param(
  [switch]$AutoRedact
)

$ErrorActionPreference = 'Stop'

function Write-Info($m){ Write-Host "[INFO] $m" -ForegroundColor Cyan }
function Write-Warn($m){ Write-Host "[WARN] $m" -ForegroundColor Yellow }
function Write-Err($m){ Write-Host "[ERR ] $m" -ForegroundColor Red }

# Determine staged files
$staged = git diff --cached --name-only --diff-filter=ACM | Where-Object { $_ -and -not ($_ -like 'target/*') -and -not ($_ -like 'node_modules/*') }
if (-not $staged) { exit 0 }

$rx = [ordered]@{
  'Email'            = '\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b'
  'Phone'            = '\b(?:\+?\d{1,3}[-.\s]?)?(?:\(?\d{2,4}\)?[-.\s]?)?\d{3,4}[-.\s]?\d{4}\b'
  'CCNumber'         = '\b(?:\d[ -]*?){13,19}\b'
  'US-SSN'           = '\b\d{3}-\d{2}-\d{4}\b'
  'AWS-AccessKeyId'  = '\bAKIA[0-9A-Z]{16}\b'
  'AWS-Secret'       = '\b[0-9A-Za-z/+]{40}\b'
  'Google-API'       = '\bAIza[0-9A-Za-z_\-]{35}\b'
  'JWT'              = '\beyJ[0-9A-Za-z_\-]+\.[0-9A-Za-z_\-]+\.[0-9A-Za-z_\-]+\b'
  'PrivKey'          = '-----BEGIN [A-Z ]*PRIVATE KEY-----'
}

function Test-Luhn($s){
  $digits = ($s -replace '[^0-9]','')
  if($digits.Length -lt 13 -or $digits.Length -gt 19){ return $false }
  $sum=0; $alt=$false
  for($i=$digits.Length-1; $i -ge 0; $i--){
    $d=[int]$digits[$i]
    if($alt){ $d*=2; if($d -gt 9){ $d-=9 } }
    $sum += $d; $alt = -not $alt
  }
  return ($sum % 10 -eq 0)
}

$hasBlockingFind=$false
$autoAll = $AutoRedact -or ($env:PII_AUTOREDACT -eq '1')

foreach($file in $staged){
  if (-not (Test-Path $file)) { continue }
  # Skip binary
  try {
    $bytes = [System.IO.File]::ReadAllBytes($file)
    if ($bytes -match '\x00'){ continue }
  } catch { continue }

  $text = Get-Content -Raw -LiteralPath $file
  $finds = @()
  foreach($k in $rx.Keys){
    $pattern = $rx[$k]
    $matches = [regex]::Matches($text, $pattern)
    foreach($m in $matches){
      if($k -eq 'CCNumber' -and -not (Test-Luhn $m.Value)){ continue }
      $finds += [pscustomobject]@{ Kind=$k; Value=$m.Value }
    }
  }
  if($finds.Count -eq 0){ continue }

  $isLog = ($file -like 'log/*' -or $file -like '*.log' -or $file -like '*/logs/*')
  $doRedact = $autoAll -or $isLog

  if($doRedact){
    $redacted = $text
    foreach($f in $finds){
      $mask = if($f.Kind -eq 'CCNumber'){ '[REDACTED-CC]' } elseif($f.Kind -eq 'Email'){ '[REDACTED-EMAIL]' } else { '[REDACTED]' }
      $escaped = [regex]::Escape($f.Value)
      $redacted = [regex]::Replace($redacted, $escaped, $mask)
    }
    Set-Content -NoNewline -LiteralPath $file -Value $redacted
    git add -- $file | Out-Null
    Write-Warn "PII auto-redacted in $file (${($finds | Select-Object -ExpandProperty Kind | Sort-Object -Unique) -join ', '})"
  } else {
    Write-Err "PII detected in $file:"; $finds | Format-Table -AutoSize | Out-String | Write-Host
    $hasBlockingFind = $true
  }
}

if($hasBlockingFind){
  Write-Err "Commit aborted. Remove/redact PII or set PII_AUTOREDACT=1 to auto-redact (use with care)."
  exit 1
}

exit 0

