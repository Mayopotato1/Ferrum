param()
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$pins = Get-Content -LiteralPath (Join-Path $repo 'packaging/tools.json') -Raw | ConvertFrom-Json
if ((rustc --version) -notmatch ('^rustc ' + [regex]::Escape($pins.rust_toolchain) + ' ')) { throw "Release builds require Rust $($pins.rust_toolchain); install it explicitly or use the CI workflow" }
$previousDirectory = Get-Location
$names = @('RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','SLINT_EMBED_RESOURCES','CFLAGS')
$saved = @{}
foreach ($name in $names) { $saved[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
try {
    Set-Location -LiteralPath $repo
    $cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
    # Most specific remapping comes last. Never persist machine/user environment changes.
    $flags = @("--remap-path-prefix=$env:USERPROFILE=/build-user", "--remap-path-prefix=$cargoHome=/cargo", "--remap-path-prefix=$repo=/src/ferrum", '-C', 'target-feature=+crt-static', '-C', 'link-arg=/Brepro')
    $env:RUSTFLAGS = ''
    $env:CARGO_ENCODED_RUSTFLAGS = $flags -join [char]31
    $env:SLINT_EMBED_RESOURCES = 'true'
    $env:CFLAGS = '"/pathmap:' + $env:USERPROFILE + '=/build-user" "/pathmap:' + $cargoHome + '=/cargo" "/pathmap:' + $repo + '=/src/ferrum"'
    & cargo fmt --check | Out-Host
    if ($LASTEXITCODE -ne 0) { throw 'Formatting failed' }
    & cargo clippy --release --locked --target x86_64-pc-windows-msvc --all-targets -- -D warnings | Out-Host
    if ($LASTEXITCODE -ne 0) { throw 'Strict Clippy failed' }
    & cargo test --release --locked --target x86_64-pc-windows-msvc | Out-Host
    if ($LASTEXITCODE -ne 0) { throw 'Headless tests failed' }
    & cargo build --release --locked --target x86_64-pc-windows-msvc | Out-Host
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
    $targetRoot = if ($env:CARGO_TARGET_DIR) { [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR) } else { Join-Path $repo 'target' }
    $exe = Join-Path $targetRoot 'x86_64-pc-windows-msvc/release/ferrum.exe'
    $validation = Join-Path $repo 'target/release-validation'
    New-Item -ItemType Directory -Force -Path $validation | Out-Null
    foreach ($mode in @('--version','--relevance')) {
        $info = New-Object System.Diagnostics.ProcessStartInfo
        $info.FileName = $exe; $info.Arguments = $mode; $info.WorkingDirectory = $validation
        $info.UseShellExecute = $false; $info.CreateNoWindow = $true
        $info.RedirectStandardOutput = $true; $info.RedirectStandardError = $true
        $process = New-Object System.Diagnostics.Process; $process.StartInfo = $info
        [void]$process.Start()
        $stdout = $process.StandardOutput.ReadToEnd(); $stderr = $process.StandardError.ReadToEnd()
        $process.WaitForExit()
        if ($process.ExitCode -ne 0) { throw "$mode failed: $stderr" }
        if ($mode -eq '--relevance' -and $stdout -notmatch 'Top-1 64/64; Top-3 64/64') { throw 'Relevance regression' }
        Write-Host $stdout.Trim()
        $process.Dispose()
    }
    & (Join-Path $PSScriptRoot "Assert-UpdateSource.ps1") -Binary $exe | Out-Host
    # Keep prior development/installed executables untouched, including running copies.
    $exe
} finally {
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $saved[$name], 'Process') }
    Set-Location -LiteralPath $previousDirectory
}
