<#
ExrTool bootstrap (Windows, PowerShell 5+)
目的: Rust/Tauri開発環境を最短で構築

実行: 右クリック→PowerShellで実行（管理者推奨）
  Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass
  .\scripts\bootstrap_windows.ps1
#>

function Write-Info($msg) { Write-Host "[INFO] $msg" -ForegroundColor Cyan }
function Write-Ok($msg) { Write-Host "[ OK ] $msg" -ForegroundColor Green }
function Write-Warn($msg) { Write-Host "[WARN] $msg" -ForegroundColor Yellow }
function Write-Err($msg) { Write-Host "[ERR ] $msg" -ForegroundColor Red }

$ErrorActionPreference = 'Stop'

Write-Info "Bootstrap start"

# 1) winget availability
if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
  Write-Err "winget が見つかりません。Microsoft Storeから 'App Installer' をインストールしてください。"
  exit 1
}

# 2) Git
if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
  Write-Info "Git をインストール"
  winget install -e --id Git.Git -h --accept-package-agreements --accept-source-agreements
} else { Write-Ok "Git OK" }

# 3) Rustup + toolchain（PATH 未反映にも対応）
#   winget インストール直後は現セッションに PATH が反映されない場合があるため、
#   明示的に ~/.cargo/bin を PATH に追加し、フルパス実行にフォールバックする。
function Resolve-CargoBin($exeName) {
  $cmd = Get-Command $exeName -ErrorAction SilentlyContinue
  if ($cmd) { return $cmd.Source }
  $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
  $candidate = Join-Path $cargoBin ($exeName + ".exe")
  if (Test-Path $candidate) { return $candidate }
  return $null
}

if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
  Write-Info "Rustup をインストール"
  winget install -e --id Rustlang.Rustup -h --accept-package-agreements --accept-source-agreements
  # セッション PATH を更新（存在すれば前段に追加）
  $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
  if (Test-Path $cargoBin) { $env:PATH = "$cargoBin;$env:PATH" }
}

$rustupPath = Resolve-CargoBin "rustup"
if (-not $rustupPath) {
  # インストール直後の遅延に備えて短時間リトライ
  for ($i=0; $i -lt 10 -and -not $rustupPath; $i++) {
    Start-Sleep -Milliseconds 500
    $rustupPath = Resolve-CargoBin "rustup"
  }
}
if (-not $rustupPath) {
  Write-Err "rustup が見つかりません。PowerShell を再起動して再実行してください。"
  exit 1
}

Write-Info "Rust toolchain を確認"
& $rustupPath toolchain install stable
& $rustupPath default stable

# 4) VS Build Tools (C++ + Windows SDK)
Write-Info "MSVC ビルドツールを確認"
if (-not (Get-Command cl.exe -ErrorAction SilentlyContinue)) {
  Write-Warn "cl.exe が見つかりません。Visual Studio 2022 Build Tools を導入します。"
  winget install -e --id Microsoft.VisualStudio.2022.BuildTools --override "--add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --passive --norestart"
} else { Write-Ok "MSVC OK" }

# 5) WebView2 runtime
Write-Info "WebView2 runtime を確認"
winget list --id Microsoft.EdgeWebView2Runtime -q | Out-Null
if ($LASTEXITCODE -ne 0) {
  winget install -e --id Microsoft.EdgeWebView2Runtime -h --accept-package-agreements --accept-source-agreements
} else { Write-Ok "WebView2 OK" }

# 6) tauri-cli (v1 系)（cargo の PATH 未反映にも対応）
Write-Info "tauri-cli をインストール/更新 (v1 系)"
$cargoPath = Resolve-CargoBin "cargo"
if (-not $cargoPath) {
  # 念のためリトライ
  for ($i=0; $i -lt 10 -and -not $cargoPath; $i++) {
    Start-Sleep -Milliseconds 500
    $cargoPath = Resolve-CargoBin "cargo"
  }
}
if (-not $cargoPath) {
  Write-Err "cargo が見つかりません。PowerShell を再起動して再実行してください。"
  exit 1
}
& $cargoPath install tauri-cli --version ^1 --locked

Write-Ok "Bootstrap 完了。次のコマンドを実行してください:"
Write-Host "  cd apps/exrtool-gui/src-tauri" -ForegroundColor Gray
Write-Host "  cargo tauri dev" -ForegroundColor Gray

