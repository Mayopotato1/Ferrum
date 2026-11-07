param([Parameter(Mandatory=$true)][string]$Binary)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$source = Get-Content -LiteralPath (Join-Path $repo 'packaging/update-source.json') -Raw | ConvertFrom-Json
if (-not $source.configured) { throw 'Official release packaging requires the pinned update source' }
$info = New-Object System.Diagnostics.ProcessStartInfo
$info.FileName = (Resolve-Path -LiteralPath $Binary).Path
$info.Arguments = '--update-source'
$info.UseShellExecute = $false; $info.CreateNoWindow = $true
$info.RedirectStandardOutput = $true; $info.RedirectStandardError = $true
$process = New-Object System.Diagnostics.Process; $process.StartInfo = $info
try {
    [void]$process.Start()
    $stdout = $process.StandardOutput.ReadToEnd(); $stderr = $process.StandardError.ReadToEnd()
    $process.WaitForExit()
    if ($process.ExitCode -ne 0) { throw "Headless source inspection failed: $stderr" }
    $actual = $stdout | ConvertFrom-Json
    $expected = "https://api.github.com/repos/$($source.owner)/$($source.repository)/releases?per_page=100"
    $manifest = Get-Content -LiteralPath (Join-Path $repo 'Cargo.toml') -Raw
    $version = [regex]::Match($manifest, '(?m)^version = "([^"]+)"').Groups[1].Value
    if (-not $actual.configured -or $actual.local_fixture -or $actual.api_url -cne $expected -or $actual.channel -cne $source.channel -or $actual.version -cne $version) {
        throw 'Binary update source/channel/version does not match pinned release configuration'
    }
    Write-Output "Validated compiled update source: $expected ($($actual.channel), $($actual.version), fixture off)"
} finally { $process.Dispose() }
