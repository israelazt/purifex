# Measures whether the window of Purifex keeps answering (docs/BETTER_THAN_REVO_PLAN.md, M.1):
# every 15 ms it asks the window to answer an empty message and times it.
#   tools/respond.ps1                                  an open purifex.exe, for 120 s
#   tools/respond.ps1 -Launch "ui --find-traces X"     starts it (target\release) and measures
#   -Seconds N   how long    -Csv path   every wait    -Name purifex   which process
# It writes nothing in the repository.
param(
    [string]$Name = 'purifex',
    [string]$Launch,
    [int]$Seconds = 120,
    [string]$Csv
)

$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class Probe {
    [DllImport("user32.dll")] public static extern IntPtr SendMessageTimeout(
        IntPtr hwnd, uint msg, IntPtr w, IntPtr l, uint flags, uint timeout, out IntPtr result);
    [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr hwnd);
}
'@

function Wait-Window([System.Diagnostics.Process]$Process) {
    $until = (Get-Date).AddSeconds(15)
    while ((Get-Date) -lt $until) {
        $Process.Refresh()
        if ($Process.HasExited) { throw 'el programa terminó antes de enseñar su ventana' }
        if ($Process.MainWindowHandle -ne [IntPtr]::Zero) { return $Process.MainWindowHandle }
        Start-Sleep -Milliseconds 10
    }
    throw 'no apareció ninguna ventana en 15 s'
}

if ($Launch) {
    $exe = Join-Path (Split-Path $PSScriptRoot -Parent) 'target\release\purifex.exe'
    if (-not (Test-Path $exe)) { throw "falta ${exe}: cargo build --release -p purifex" }
    $process = Start-Process -FilePath $exe -ArgumentList $Launch -PassThru
} else {
    $process = Get-Process -Name $Name -ErrorAction SilentlyContinue |
        Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } | Select-Object -First 1
    if (-not $process) { throw "no hay ningún proceso '$Name' con ventana: ábrelo o usa -Launch" }
}
$window = Wait-Window $process

$waits = New-Object System.Collections.Generic.List[object]
$cpuBefore = $process.TotalProcessorTime.TotalMilliseconds
$clock = [System.Diagnostics.Stopwatch]::StartNew()
$one = [System.Diagnostics.Stopwatch]::new()
$result = [IntPtr]::Zero
while ($clock.Elapsed.TotalSeconds -lt $Seconds -and [Probe]::IsWindow($window)) {
    $at = $clock.ElapsedMilliseconds
    $one.Restart()
    # WM_NULL (0), SMTO_ABORTIFHUNG (2): answered only when the window reads its messages.
    $null = [Probe]::SendMessageTimeout($window, 0, [IntPtr]::Zero, [IntPtr]::Zero, 2, 10000, [ref]$result)
    $one.Stop()
    $waits.Add([pscustomobject]@{ AtMs = $at; WaitMs = [math]::Round($one.Elapsed.TotalMilliseconds, 1) })
    Start-Sleep -Milliseconds 15
}
$elapsed = $clock.Elapsed.TotalMilliseconds
$process.Refresh()
$cpu = if ($process.HasExited) { $null } else { $process.TotalProcessorTime.TotalMilliseconds - $cpuBefore }
if ($Launch -and -not $process.HasExited) { Stop-Process -Id $process.Id -Force }

$slow = @($waits | Where-Object { $_.WaitMs -gt 100 })
"preguntas: $($waits.Count) en $([math]::Round($elapsed / 1000, 1)) s"
"esperas de más de 100 ms: $($slow.Count)"
$slow | Select-Object -First 20 | ForEach-Object { "  a los $($_.AtMs) ms: $($_.WaitMs) ms" }
'las 5 peores:'
$waits | Sort-Object WaitMs -Descending | Select-Object -First 5 |
    ForEach-Object { "  a los $($_.AtMs) ms: $($_.WaitMs) ms" }
if ($null -ne $cpu) { "CPU: $([math]::Round(100 * $cpu / $elapsed, 1)) % de un núcleo" }
else { 'CPU: el proceso terminó antes de medirla' }
if ($Csv) { $waits | Export-Csv -NoTypeInformation -Path $Csv; "todas las esperas en $Csv" }
