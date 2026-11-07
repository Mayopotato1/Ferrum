param([switch]$LicenseOnly)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$policy = Get-Content -LiteralPath (Join-Path $repo 'packaging/release-policy.json') -Raw | ConvertFrom-Json
if (-not $policy.source_license -or -not (Test-Path -LiteralPath (Join-Path $repo 'LICENSE'))) { throw 'Publication blocked: the owner must choose a source license and add LICENSE; do not publish LICENSE-PENDING.txt as a license' }
if (-not $LicenseOnly -and (-not $policy.artwork_distribution_confirmed -or -not $policy.publication_approved -or -not $policy.manual_installation_validated -or -not $policy.phase_1_4_gui_validated)) { throw 'Publication blocked: artwork confirmation, owner approval and manual GUI/install/upgrade/uninstall validation remain outstanding' }
