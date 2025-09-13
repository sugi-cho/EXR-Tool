[CmdletBinding()]
param(
    [string]$Bundles = "msix,msi"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$projectRoot = Join-Path $PSScriptRoot "..\apps\exrtool-gui\src-tauri"
Set-Location $projectRoot

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error "cargo is not installed"
}

cargo tauri build --bundles $Bundles
