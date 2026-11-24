# Compares what Purifex proposes against what Fex Sonda really left (docs/TEST_APP_PLAN.md, T.5).
#   tools/sonda.ps1 ciclo      install + uninstall (as administrator), then `purifex apps traces`.
#   tools/sonda.ps1 comparar   crosses that output with `fex-sonda status --json`.
# It removes nothing: `fex-sonda purge` does that.
param([Parameter(Mandatory = $true)][ValidateSet('ciclo', 'comparar')][string]$Orden)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$out = Join-Path $root 'target\sonda'
$traces = Join-Path $out 'traces.txt'
$sonda = Join-Path $root 'target\debug\fex-sonda.exe'
$purifex = Join-Path $root 'target\debug\purifex.exe'

function Test-Elevated {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    ([Security.Principal.WindowsPrincipal]$id).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

# The place of a line of `apps traces` («• C:\x  203 MB», «• HKCU\k  [valor: v]  –»).
function Get-Proposed([string]$Text) {
    $Text -split "`r?`n" | Where-Object { $_.StartsWith([string][char]0x2022) } |
        ForEach-Object { ($_.Substring(2) -split '  ', 2)[0].Trim() }
}

# The same place, an ancestor of it (the whole folder or key goes) or something inside it.
function Test-Related([string]$Where, [string]$Place) {
    $a = $Where.TrimEnd('\').ToLowerInvariant()
    $b = $Place.TrimEnd('\').ToLowerInvariant()
    ($a -eq $b) -or $a.StartsWith($b + '\') -or $b.StartsWith($a + '\')
}

function Invoke-Ciclo {
    if (-not (Test-Elevated)) { throw 'ciclo necesita una consola de administrador (install escribe en HKLM).' }
    Push-Location $root
    try {
        cargo build -q -p fex-sonda -p purifex
        if ($LASTEXITCODE -ne 0) { throw 'la compilación falló' }
    } finally { Pop-Location }
    & $sonda install
    if ($LASTEXITCODE -ne 0) { throw 'install falló' }
    & $sonda uninstall
    if ($LASTEXITCODE -ne 0) { throw 'uninstall falló' }
    New-Item -ItemType Directory -Force $out | Out-Null
    & $purifex apps traces 'Fex Sonda' | Out-File -Encoding utf8 $traces
    Write-Host "Restos propuestos por Purifex guardados en $traces"
}

function Invoke-Comparar {
    if (-not (Test-Path $traces)) { throw "falta ${traces}: ejecuta antes 'tools/sonda.ps1 ciclo'." }
    $proposed = @(Get-Proposed (Get-Content -Raw -Encoding utf8 $traces))
    $status = & $sonda status --json | ConvertFrom-Json
    $rows = foreach ($item in $status.items) {
        $found = $proposed | Where-Object { Test-Related $item.where $_ } | Select-Object -First 1
        $ok = switch ($item.role) {
            'leftover' { $item.present -and $found }
            'control' { -not $found }
            default { $true }
        }
        [pscustomobject]@{
            Id = $item.id; Rol = $item.role; Presente = $item.present
            Propuesto = [bool]$found; Veredicto = $(if ($ok) { 'bien' } else { 'FALLA' })
        }
    }
    $rows | Format-Table -AutoSize
    $failed = @($rows | Where-Object Veredicto -eq 'FALLA')
    if ($failed.Count -gt 0) {
        Write-Host ('Fallan: ' + (($failed | ForEach-Object Id) -join ', ')) -ForegroundColor Red
        exit 1
    }
    Write-Host 'Todos los restos propuestos y ningún control.' -ForegroundColor Green
}

switch ($Orden) {
    'ciclo' { Invoke-Ciclo }
    'comparar' { Invoke-Comparar }
}
