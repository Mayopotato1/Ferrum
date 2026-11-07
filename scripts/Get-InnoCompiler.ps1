param([switch]$Offline)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$pins = Get-Content -LiteralPath (Join-Path $repo 'packaging/tools.json') -Raw | ConvertFrom-Json
$tools = Join-Path $repo 'target/tools'
$compiler = Join-Path $tools 'inno-6.7.4-extracted/{app}/ISCC.exe'

function Download-Verified($Url, $Destination, $Hash) {
    if (-not (Test-Path -LiteralPath $Destination)) {
        if ($Offline) { throw "Missing pinned tool in offline mode: $Destination" }
        New-Item -ItemType Directory -Force -Path (Split-Path $Destination -Parent) | Out-Null
        Invoke-WebRequest -UseBasicParsing -Uri $Url -OutFile $Destination
    }
    if ((Get-FileHash -LiteralPath $Destination -Algorithm SHA256).Hash.ToLower() -ne $Hash) { throw "Tool hash mismatch: $Destination" }
}

if (-not (Test-Path -LiteralPath $compiler)) {
    $archive = Join-Path $tools 'innosetup-6.7.4.exe'
    Download-Verified $pins.inno.url $archive $pins.inno.sha256
    $signature = Get-AuthenticodeSignature -LiteralPath $archive
    if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'Pyrsys B\.V\.') { throw 'Official compiler archive signature is not trusted' }
    $zip = Join-Path $tools 'innounp-2.71.3.zip'
    Download-Verified $pins.unpacker.url $zip $pins.unpacker.sha256
    $unpackerDir = Join-Path $tools 'innounp-2.71.3'
    # The pinned upstream ZIP contains the CLI unpacker, not a GUI installer.
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zipFile = [IO.Compression.ZipFile]::OpenRead($zip)
    try {
        $destinationRoot = [IO.Path]::GetFullPath($unpackerDir) + [IO.Path]::DirectorySeparatorChar
        foreach ($entry in $zipFile.Entries) {
            $destination = [IO.Path]::GetFullPath((Join-Path $unpackerDir $entry.FullName))
            if (-not $destination.StartsWith($destinationRoot, [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe tool archive path' }
        }
    } finally { $zipFile.Dispose() }
    if (-not (Test-Path -LiteralPath $unpackerDir)) { [IO.Compression.ZipFile]::ExtractToDirectory($zip, $unpackerDir) }
    & (Join-Path $unpackerDir 'innounp.exe') -x -y ('-d' + (Join-Path $tools 'inno-6.7.4-extracted')) $archive | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Compiler archive extraction failed' }
}
$compilerDir = Split-Path $compiler -Parent
foreach ($property in $pins.inno.compiler_files.PSObject.Properties) {
    $path = Join-Path $compilerDir $property.Name
    if (-not (Test-Path -LiteralPath $path) -or (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLower() -ne $property.Value) { throw "Pinned compiler file differs: $($property.Name)" }
}
$signature = Get-AuthenticodeSignature -LiteralPath $compiler
if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'Pyrsys B\.V\.') { throw 'Extracted compiler signature is not trusted' }
# Return only the console compiler path. Never execute a setup installer.
$compiler
