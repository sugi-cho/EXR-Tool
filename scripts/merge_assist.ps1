<#!
.SYNOPSIS
  PR を優先順でチェックアウト→ビルド確認→競合解消→マージ (-dでブランチ削除) する補助スクリプト。

.PARAMETER Numbers
  対象PR番号の配列。省略時は `gh pr list` で取得したオープンPRを新しい順に処理。

.PARAMETER Method
  GitHub側のマージ方式: merge | rebase | squash （既定: merge）

.PARAMETER Strategy
  競合時に base→head を取り込む際の優先: theirs | ours （既定: theirs）

.PARAMETER GraphQLFallback
  gh の結果が曖昧/未マージ時、GraphQL API で確定マージを試行。

.EXAMPLE
  # 明示したPRのみ処理
  ./scripts/merge_assist.ps1 -Numbers 12,15,18 -Method squash -Strategy theirs -GraphQLFallback

.EXAMPLE
  # 全オープンPRを処理
  ./scripts/merge_assist.ps1 -Method merge -Strategy theirs -GraphQLFallback
#>

param(
  [int[]]$Numbers,
  [ValidateSet('merge','rebase','squash')]
  [string]$Method = 'merge',
  [ValidateSet('theirs','ours')]
  [string]$Strategy = 'theirs',
  [switch]$GraphQLFallback
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Info($m){ Write-Host "[INFO] $m" -ForegroundColor Cyan }
function Ok($m){ Write-Host "[ OK ] $m" -ForegroundColor Green }
function Warn($m){ Write-Host "[WARN] $m" -ForegroundColor Yellow }
function Err($m){ Write-Host "[ERR ] $m" -ForegroundColor Red }

function Get-PrState($n){
  try {
    return gh pr view $n --json state,mergedAt,mergeable,mergeStateStatus --jq '{state,mergedAt,mergeable,mergeStateStatus}' | ConvertFrom-Json
  } catch { return $null }
}

function Merge-WithGraphQL($n, $title, $method){
  try {
    $id = gh pr view $n --json id --jq .id
    if (-not $id) { return $false }
    $headline = "${title} (#${n})"
    $mm = $method.ToUpper()
    $q = @"
mutation(
  $id:ID!
){
  mergePullRequest(input:{pullRequestId:$id, mergeMethod:$mm, commitHeadline:"$headline"}){
    pullRequest{ state mergedAt mergeCommit { oid } }
  }
}
"@
    $res = gh api graphql -F id=$id -f query="$q" | ConvertFrom-Json
    $st = $res.data.mergePullRequest.pullRequest.state
    return ($st -eq 'MERGED')
  } catch {
    return $false
  }
}

pushd $PSScriptRoot/..
try {
  # リポジトリ確認
  $repoRoot = (git rev-parse --show-toplevel) 2>$null
  if (-not $repoRoot) { throw 'git repo not found' }
  Set-Location $repoRoot

  # gh 認証確認
  gh auth status | Out-Null

  if (-not $Numbers -or $Numbers.Count -eq 0) {
    $listJson = gh pr list --limit 100 --json number | ConvertFrom-Json
    $Numbers = @($listJson | ForEach-Object { $_.number })
  }
  if (-not $Numbers -or $Numbers.Count -eq 0) { Warn 'No open PRs'; return }

  $results = @()
  foreach ($n in $Numbers) {
    Info "Processing PR #${n}"
    $meta = gh pr view $n --json number,title,isDraft,mergeable,mergeStateStatus,headRefName,baseRefName  | ConvertFrom-Json
    $title = $meta.title
    if ($meta.isDraft) { Warn "PR #${n} draft; skip"; $results += [pscustomobject]@{pr=$n; title=$title; action='skip(draft)'}; continue }

    # checkout
    gh pr checkout $n | Out-Null

    # quick build checks
    try {
      Info 'cargo check (cli)'
      cargo check -q -p exrtool-cli
      Push-Location 'apps/exrtool-gui/src-tauri'
      Info 'cargo check (gui)'
      cargo check -q
      Pop-Location
    } catch {
      Warn "Build check failed on PR #${n}: $($_.Exception.Message)"
    }

    # conflict handling
    $meta2 = gh pr view $n --json mergeable,mergeStateStatus,baseRefName | ConvertFrom-Json
    if (($meta2.mergeable -eq 'CONFLICTING') -or ($meta2.mergeStateStatus -eq 'DIRTY')) {
      Warn "PR #${n} has conflicts ($($meta2.mergeStateStatus)). Merging base into head with -X $Strategy"
      git fetch origin | Out-Null
      git merge --no-edit -s ort -X $Strategy "origin/$($meta2.baseRefName)" 2>&1 | Write-Host
      git push origin HEAD 2>&1 | Write-Host
    }

    # merge (gh) → 状態確認 → 必要ならGraphQLフォールバック
    $merged = $false
    $queued = $false
    $out = ''
    $out2 = ''
    $ghFlag = switch ($Method) { 'merge' { '-m' } 'rebase' { '-r' } 'squash' { '-s' } }
    try {
      $out = gh pr merge $n $ghFlag -d 2>&1
      $out | Write-Host
      $st = Get-PrState $n
      if ($st -and $st.state -eq 'MERGED') { $merged = $true }
      elseif ($out -match 'Auto-merge') { $queued = $true }
    } catch {
      Warn "Direct merge command threw: $($_.Exception.Message)"
    }
    if (-not $merged -and -not $queued) {
      Warn "Retry merge with --auto"
      try {
        $out2 = gh pr merge $n $ghFlag -d --auto 2>&1
        $out2 | Write-Host
        $st2 = Get-PrState $n
        if ($st2 -and $st2.state -eq 'MERGED') { $merged = $true } else { $queued = $true }
      } catch {
        Warn "--auto also failed: $($_.Exception.Message)"
      }
    }
    if (-not $merged -and $GraphQLFallback) {
      Warn "Trying GraphQL fallback merge"
      if (Merge-WithGraphQL $n $title $Method) { $merged = $true }
    }
    if ($merged) {
      Ok "Merged PR #${n}"
      $results += [pscustomobject]@{pr=$n; title=$title; action='merged'}
    } elseif ($queued) {
      Ok "Auto-merge queued PR #${n}"
      $results += [pscustomobject]@{pr=$n; title=$title; action='automerge'}
    } else {
      Err "Merge unresolved for PR #${n}"
      # 失敗詳細を log/error.log へ
      New-Item -Force -ItemType Directory log | Out-Null
      ("merge failed for PR #${n}\n" + ($out2 | Out-String) + ($out | Out-String)) | Set-Content -Encoding UTF8 log/error.log
      $results += [pscustomobject]@{pr=$n; title=$title; action='merge-failed'}
    }
  }

  Info 'Summary'
  $results | Format-Table -AutoSize | Out-String | Write-Host
} finally {
  popd
}
