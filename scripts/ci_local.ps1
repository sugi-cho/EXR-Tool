<#!
.SYNOPSIS
  ローカルでの簡易CIガード（fmt/clippy/build）を実行します。

.EXAMPLE
  ./scripts/ci_local.ps1
#>

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Step($name, [ScriptBlock]$block) {
  Write-Host "[STEP] $name" -ForegroundColor Cyan
  & $block
}

pushd $PSScriptRoot/..
try {
  Step 'rustup toolchain' { rustup show }

  Step 'fmt check' { cargo fmt --all -- --check }

  Step 'clippy (workspace)' { cargo clippy --workspace --all-targets -- -D warnings }

  Step 'build CLI' { cargo build -p exrtool-cli }

  Step 'build GUI' {
    Push-Location apps/exrtool-gui/src-tauri
    cargo build
    Pop-Location
  }

  Step 'feature: use_exr_crate (core only)' { cargo check -p exrtool-core --features use_exr_crate }

  Write-Host '[ OK ] all checks passed' -ForegroundColor Green
} finally {
  popd
}

