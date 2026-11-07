param(
    [Parameter(Mandatory=$true)][string]$Binary,
    [Parameter(Mandatory=$true)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
$binaryPath = (Resolve-Path -LiteralPath $Binary).Path
$out = [System.IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $out | Out-Null
$runs = @()
# Fixed headless modes only: never invoke the executable without an explicit mode.
foreach ($mode in @('--version','--version','--version','--version','--version','--bench','--relevance','--idle-probe')) {
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $binaryPath
    $info.Arguments = $mode
    $info.WorkingDirectory = $out
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $info
    $clock = [System.Diagnostics.Stopwatch]::StartNew()
    [void]$process.Start()
    $peak = 0L
    while (-not $process.HasExited) {
        $process.Refresh()
        try { $peak = [Math]::Max($peak, $process.WorkingSet64) } catch {}
        if ($clock.Elapsed.TotalSeconds -gt 45) { throw 'Bounded headless probe exceeded 45 seconds' }
        Start-Sleep -Milliseconds 20
    }
    $stdout = $process.StandardOutput.ReadToEnd()
    $stderr = $process.StandardError.ReadToEnd()
    $clock.Stop()
    $runs += @{mode=$mode; exit_code=$process.ExitCode; wall_ms=$clock.Elapsed.TotalMilliseconds; cpu_ms=$process.TotalProcessorTime.TotalMilliseconds; sampled_peak_bytes=$peak; stdout=$stdout; stderr=$stderr}
    if ($process.ExitCode -ne 0) { throw $stderr }
    $process.Dispose()
}
$item = Get-Item -LiteralPath $binaryPath
@{runs=$runs; executable_bytes=$item.Length; sha256=(Get-FileHash -LiteralPath $binaryPath -Algorithm SHA256).Hash; product=$item.VersionInfo.ProductName; description=$item.VersionInfo.FileDescription; method='Fixed CLI modes, no GUI; 20ms parent sampling, observed OS cache state, first version run is warmup; CPU milliseconds are total process CPU'} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $out 'PROFILE.json')
