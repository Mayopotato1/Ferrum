param(
    [string]$Binary,
    [string]$ExpectedTag,
    [switch]$AllowPrivateCandidate,
    [string]$InnoSignCommand
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$manifest = Get-Content -LiteralPath (Join-Path $repo 'Cargo.toml') -Raw
$version = [regex]::Match($manifest, '(?m)^version = "([^"]+)"').Groups[1].Value
if (-not $version) { throw 'Cannot read Cargo package version' }
if ($ExpectedTag -and $ExpectedTag -cne ('v' + $version)) { throw 'Release tag does not match Cargo.toml' }
if (-not $AllowPrivateCandidate) { & (Join-Path $PSScriptRoot 'Assert-ReleaseReady.ps1') -LicenseOnly }
if (-not $Binary) { $Binary = & (Join-Path $PSScriptRoot 'Build-Release.ps1') }
$binaryPath = (Resolve-Path -LiteralPath $Binary).Path
$metadata = (Get-Item -LiteralPath $binaryPath).VersionInfo
if ($metadata.ProductName -cne 'Ferrum' -or $metadata.ProductVersion -cne $version -or $metadata.OriginalFilename -cne 'ferrum.exe') { throw 'Executable branding/version does not match release' }
& (Join-Path $PSScriptRoot 'Assert-UpdateSource.ps1') -Binary $binaryPath | Out-Host
$dist = Join-Path $repo ('dist/v' + $version)
$stage = Join-Path $repo ('target/release-staging/v' + $version)
if (Test-Path -LiteralPath $dist) { throw 'Release directory exists. Move the prior candidate aside explicitly before rebuilding; never reuse stale checksums.' }
if (Test-Path -LiteralPath $stage) { throw 'Staging directory exists. Move the prior candidate aside explicitly before rebuilding.' }
New-Item -ItemType Directory -Force -Path $stage,$dist | Out-Null
Copy-Item -LiteralPath $binaryPath -Destination (Join-Path $stage 'ferrum.exe')
if ($InnoSignCommand -and (Get-AuthenticodeSignature -LiteralPath $binaryPath).Status -ne 'Valid') { throw 'Signing mode requires an already legitimately signed payload; no self-signed trust is assumed' }
& python (Join-Path $PSScriptRoot 'inspect_binary.py') (Join-Path $stage 'ferrum.exe') --output (Join-Path $dist 'BINARY_INSPECTION.json')
if ($LASTEXITCODE -ne 0) { throw 'Binary runtime/privacy/icon inspection failed' }
& python (Join-Path $PSScriptRoot 'generate_notices.py') (Join-Path $stage 'licenses')
if ($LASTEXITCODE -ne 0) { throw 'Third-party license collection is incomplete' }
$license = if (Test-Path -LiteralPath (Join-Path $repo 'LICENSE')) { Join-Path $repo 'LICENSE' } else { Join-Path $repo 'LICENSE-PENDING.txt' }
Copy-Item -LiteralPath $license -Destination (Join-Path $stage 'LICENSE.txt')
Copy-Item -LiteralPath (Join-Path $repo 'packaging/portable-README.txt') -Destination (Join-Path $stage 'README.txt')
Copy-Item -LiteralPath (Join-Path $repo 'docs/INSTALLATION.md') -Destination (Join-Path $stage 'INSTALLATION.txt')
Copy-Item -LiteralPath (Join-Path $repo 'RELEASE_NOTES.md') -Destination (Join-Path $dist 'RELEASE_NOTES.md')
$compiler = & (Join-Path $PSScriptRoot 'Get-InnoCompiler.ps1')
$arguments = @('/Qp', ('/DAppVersion=' + $version), ('/DNumericVersion=' + $metadata.FileVersion), ('/DPayloadDir=' + $stage), ('/DReleaseDir=' + $dist))
if ($InnoSignCommand) { $arguments += '/DSignedRelease'; $arguments += ('/Sferrum=' + $InnoSignCommand) }
$arguments += Join-Path $repo 'packaging/windows/ferrum.iss'
& $compiler @arguments | Out-Host
if ($LASTEXITCODE -ne 0) { throw 'Inno Setup compilation failed; no installer was executed' }
$zip = Join-Path $dist ('Ferrum-v' + $version + '-windows-x64-portable.zip')
& python (Join-Path $PSScriptRoot 'make_zip.py') $stage $zip
if ($LASTEXITCODE -ne 0) { throw 'Portable ZIP creation failed' }
& python (Join-Path $PSScriptRoot 'export_source.py') $repo (Join-Path $dist ('Ferrum-v' + $version + '-source.zip'))
if ($LASTEXITCODE -ne 0) { throw 'Clean source export failed' }
$assets = @("Ferrum-v${version}-windows-x64-setup.exe", "Ferrum-v${version}-windows-x64-portable.zip", "Ferrum-v${version}-source.zip", 'RELEASE_NOTES.md')
$lines = foreach ($name in $assets) { $hash = (Get-FileHash -LiteralPath (Join-Path $dist $name) -Algorithm SHA256).Hash.ToLower(); "$hash  $name" }
[IO.File]::WriteAllLines((Join-Path $dist 'SHA256SUMS.txt'), $lines, (New-Object Text.UTF8Encoding($false)))
& python (Join-Path $PSScriptRoot 'verify_release.py') $dist --payload $stage
if ($LASTEXITCODE -ne 0) { throw 'Release artifact verification failed' }
Write-Host 'Prepared release assets only. No Ferrum installer, GUI, startup write, tag or publication was performed.'
$dist
