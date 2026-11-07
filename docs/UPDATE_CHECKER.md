# GitHub update notifications

The official source is pinned in `packaging/update-source.json`:
`Mayopotato1/Ferrum`, beta channel, configured. Automatic checks default **Off**.
Manual Check for updates now works while automatic checking is Off. A successful
empty releases list means up to date as of that check; failures never prove it.
No controlled Ferrum release has been published by local preparation.

## Build configuration and validation

Normal builds read the tracked source file at compile time. They reject conflicting
FERRUM_UPDATE_OWNER/REPO/CHANNEL environment values. No runtime configuration,
HTTP response or installed user's environment can replace the source. Both the
release build and packaging scripts invoke Assert-UpdateSource.ps1, which runs the
headless --update-source command and refuses an unconfigured, fixture-enabled,
wrong-version/channel/repository binary. GitHub Actions uses the same source file
and build script; no repository variable or secret is needed for update checks.

```powershell
./scripts/Build-Release.ps1
./scripts/Assert-UpdateSource.ps1 -Binary ./target/x86_64-pc-windows-msvc/release/ferrum.exe
./scripts/release.ps1 -Binary ./target/x86_64-pc-windows-msvc/release/ferrum.exe -ExpectedTag v0.1.0-beta.1
```

Beta accepts newer beta.N and stable versions. Stable distribution builds should
set the tracked channel to stable before building their stable package version;
stable then excludes prereleases. Comparison uses SemVer precedence and ignores
build metadata. Drafts, invalid records and unpublished releases are ignored.
An intentionally unconfigured development build can set configured:false in the
source file, but official packaging refuses it. Do not commit a fixture identity.

## Network, privacy and scheduling

Only the GitHub releases-list endpoint is used, with at most 100 records/1 MiB.
The first bounded page can omit an applicable older-listed release. Native TLS
uses Windows certificate validation; HTTPS only, no redirects, three-second
connect/eight-second total timeout, no idle connection retained. Responses cannot
change the configured owner/repository. Remote release URLs must exactly match
https://github.com/Mayopotato1/Ferrum/releases/tag/VALIDATED_TAG. The browser action
reconstructs the canonical URL and runs only after View update/Open release page.

One worker waits for commands or the due time, independently of search. Automatic
checks wait at least ten seconds after initialization and at least 24 hours between
attempts. Manual checks bypass that interval but respect failure/rate-limit backoff.
403/429 honor Retry-After (seconds or HTTP date) and X-RateLimit-Reset, minimum one
hour. Other failure backoff grows from ten minutes to 24 hours. Clock rollback
rebases the wait conservatively. No requests occur on keystrokes or Settings/overlay
opening. No ETag complexity: unsolicited 304 is failure. Known validated releases
can remain visible after a network failure, separately from last-success time.

The automatic preference, attempt/success times, backoff, validated release and
per-version Later dismissal use the existing atomic settings document. Unknown
ordinary preferences are retained. No token is needed; no telemetry, index entries,
filenames, queries or preferences are uploaded. Nothing downloads release assets,
installs updates, replaces binaries, enables startup or requires administrator access.

## Isolated user-run fixture playtest

Development/packaging never launches the GUI. The user can explicitly build a
separate debug-only local-file transport. Exit any running Ferrum yourself first.
Use a fresh terminal and isolated profile; never enable startup during this test:

```powershell
$originalLocalAppData = $env:LOCALAPPDATA
$env:LOCALAPPDATA = Join-Path $PWD 'target/update-fixture-profile'
$env:CARGO_TARGET_DIR = Join-Path $PWD 'target/update-fixture-build'
$env:FERRUM_UPDATE_OWNER = 'fixture-owner'
$env:FERRUM_UPDATE_REPO = 'fixture-repo'
$env:FERRUM_UPDATE_CHANNEL = 'beta'
$env:FERRUM_UPDATE_FIXTURE_PATH = Join-Path $PWD 'tests/fixtures/update-newer.json'
cargo build --features update-fixture
# Only the user executes this GUI command, after reading MANUAL_PLAYTEST.md:
& "$env:CARGO_TARGET_DIR/debug/ferrum.exe" --settings
```

The fixture identity is synthetic. This feature reads local JSON instead of HTTP.
Do not browse to its synthetic page; headless tests prepare the canonical browser
action without opening it. A missing file or invalid JSON demonstrates failure;
[] demonstrates a successful empty response. A newer beta.N tag and matching URL
can demonstrate another notification. Check manually with automatic Off, then open
the overlay to see the banner. Later should persist across reopening/restart.

Release and non-debug builds reject the fixture feature. After personally quitting
this debug build, restore LOCALAPPDATA, remove all process-local fixture/build
variables, or close the terminal. Public production settings cannot activate it.

Official references: [release list](https://docs.github.com/en/rest/releases/releases#list-releases)
and [rate limits](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api).
