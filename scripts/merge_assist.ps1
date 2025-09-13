<#!
.SYNOPSIS
  PR を優先順でチェックアウト→ビルド確認→競合解消→マージ (-dでブランチ削除) する補助スクリプト。

.PARAMETER Numbers
  対象PR番号の配列。省略時は `gh pr list` で取得したオープンPRを新しい順に処理。

.EXAMPLE
  # 明示したPRのみ処理
  ./scripts/merge_assist.ps1 -Numbers 12,15,18

.EXAMPLE
  # 全オープンPRを処理
  ./scripts/merge_assist.ps1
#>

param(
  [int[]]$Numbers
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Info($m){ Write-Host "[INFO] $m" -ForegroundColor Cyan }
function Ok($m){ Write-Host "[ OK ] $m" -ForegroundColor Green }
function Warn($m){ Write-Host "[WARN] $m" -ForegroundColor Yellow }
function Err($m){ Write-Host "[ERR ] $m" -ForegroundColor Red }

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
    Info "Processing PR #$n"
    $meta = gh pr view $n --json number,title,isDraft,mergeable,mergeStateStatus,headRefName,baseRefName  | ConvertFrom-Json
    $title = $meta.title
    if ($meta.isDraft) { Warn "PR #$n draft; skip"; $results += [pscustomobject]@{pr=$n; title=$title; action='skip(draft)'}; continue }

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
      Warn "Build check failed on PR #$n: $($_.Exception.Message)"
    }

    # conflict handling
    $meta2 = gh pr view $n --json mergeable,mergeStateStatus,baseRefName | ConvertFrom-Json
    if (($meta2.mergeable -eq 'CONFLICTING') -or ($meta2.mergeStateStatus -eq 'DIRTY')) {
      Warn "PR #$n has conflicts ($($meta2.mergeStateStatus)). Merging base into head with -X theirs"
      git fetch origin | Out-Null
      git merge --no-edit -s ort -X theirs "origin/$($meta2.baseRefName)" 2>&1 | Write-Host
      git push origin HEAD 2>&1 | Write-Host
    }

    # merge
    try {
      gh pr merge $n -m -d 2>&1 | Write-Host
      Ok "Merged PR #$n"
      $results += [pscustomobject]@{pr=$n; title=$title; action='merged'}
    } catch {
      Warn "Direct merge failed; retry with --auto"
      try {
        gh pr merge $n -m -d --auto 2>&1 | Write-Host
        Ok "Auto-merge queued PR #$n"
        $results += [pscustomobject]@{pr=$n; title=$title; action='automerge'}
      } catch {
        Err "Merge failed for PR #$n: $($_.Exception.Message)"
        $results += [pscustomobject]@{pr=$n; title=$title; action='merge-failed'}
      }
    }
  }

  Info 'Summary'
  $results | Format-Table -AutoSize | Out-String | Write-Host
} finally {
  popd
}

