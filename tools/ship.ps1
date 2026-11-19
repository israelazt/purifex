# Commits and pushes only if everything is green: format, lints, tests (guards included).
# Usage: powershell -File tools/ship.ps1 -Message "type(scope): what changed" [-NoPush]
param(
    [Parameter(Mandatory = $true)][string]$Message,
    [switch]$NoPush
)
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
& powershell -File tools/check.ps1
if ($LASTEXITCODE -ne 0) { Write-Error 'check failed: nothing committed'; exit 1 }
git add -A
git commit -m $Message
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
if (-not $NoPush) { git push }
