# Measures Orbit's real cost: memory and CPU of orbit.exe plus everything it started
# (WebView2 window processes, FFmpeg). Memory = "private working set", the same number
# as Task Manager's "Memory" column. CPU = average over the measuring time, as a share
# of the whole PC (like Task Manager).
#
# Run (in PowerShell, from the project folder):
#   powershell -ExecutionPolicy Bypass -File scripts\measure-orbit.ps1
#   powershell -ExecutionPolicy Bypass -File scripts\measure-orbit.ps1 -Seconds 20

param([int]$Seconds = 10)

$orbit = Get-Process orbit -ErrorAction SilentlyContinue | Sort-Object StartTime | Select-Object -First 1
if (-not $orbit) { Write-Host "Orbit is not running." -ForegroundColor Red; exit 1 }

# Orbit + all its children, grandchildren, ...
function Get-Tree([int]$root) {
  $all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId, Name
  $ids = [System.Collections.Generic.List[int]]::new(); $ids.Add($root)
  for ($i = 0; $i -lt $ids.Count; $i++) {
    foreach ($p in $all) {
      if ($p.ParentProcessId -eq $ids[$i] -and -not $ids.Contains([int]$p.ProcessId)) { $ids.Add([int]$p.ProcessId) }
    }
  }
  return $ids
}

function Get-PrivateWs([int[]]$ids) {
  $map = @{}
  Get-CimInstance Win32_PerfFormattedData_PerfProc_Process |
    Where-Object { $ids -contains [int]$_.IDProcess } |
    ForEach-Object { $map[[int]$_.IDProcess] = [double]$_.WorkingSetPrivate }
  return $map
}

$cores = [Environment]::ProcessorCount
$ids = Get-Tree $orbit.Id
$before = @{}
foreach ($id in $ids) {
  $p = Get-Process -Id $id -ErrorAction SilentlyContinue
  if ($p) { $before[$id] = $p.TotalProcessorTime.TotalSeconds }
}
Write-Host "Measuring Orbit for $Seconds seconds... (don't touch Orbit meanwhile)" -ForegroundColor DarkGray
Start-Sleep -Seconds $Seconds

$ids = Get-Tree $orbit.Id
$mem = Get-PrivateWs $ids
$rows = foreach ($id in $ids) {
  $p = Get-Process -Id $id -ErrorAction SilentlyContinue
  if (-not $p) { continue }
  $cpu = 0
  if ($before.ContainsKey($id)) {
    $cpu = ($p.TotalProcessorTime.TotalSeconds - $before[$id]) / $Seconds / $cores * 100
  }
  [pscustomobject]@{
    Process  = $p.ProcessName
    PID      = $id
    'RAM MB' = [math]::Round(($mem[$id] / 1MB), 1)
    'CPU %'  = [math]::Round($cpu, 2)
  }
}
$rows | Sort-Object 'RAM MB' -Descending | Format-Table -AutoSize
$totalRam = ($rows | Measure-Object 'RAM MB' -Sum).Sum
$totalCpu = ($rows | Measure-Object 'CPU %' -Sum).Sum
Write-Host ("TOTAL  RAM {0:N1} MB   CPU {1:N2} %   ({2} processes)" -f $totalRam, $totalCpu, $rows.Count) -ForegroundColor Cyan
