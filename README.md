# Ferrum

<img src="Ferrum.png" alt="Ferrum icon" width="96">

**v0.1.0-beta.2 - owner-review release candidate**

Ferrum is a native Rust Windows launcher for applications, installed games, local
filenames and explicit website searches. It stays hidden until you summon its
floating search palette with your keyboard shortcut.

The first public release is not published yet. Core behavior received earlier
manual playtests; the complete cached-start GUI playtest and clean-machine beta
installer tests remain outstanding. Artwork distribution confirmation and owner
release approval also remain outstanding. See [release preparation](docs/RELEASING.md).

<img width="1919" height="1079" alt="image" src="https://github.com/user-attachments/assets/8897979f-c2e1-4445-974b-b810f0a8fca8" />


## Features

- In-memory Nucleo matching, targeted typo correction and local/web result ranking.
- Native Windows apps, Steam games, Modrinth profiles and packaged/Xbox entries.
- Desktop, Documents and Downloads filename/folder search within bounded limits.
- Discoverable `@` website commands, general web search, `?` mode and Shift+Enter.
- Safe direct HTTP/HTTPS URLs; nothing is searched online just because you type.
- Rounded floating overlay, 170 ms entrance, async icons and adjustable opacity.
- Always-hidden startup, single-instance guard and explicit Exit.
- Settings for provider, global shortcut, opacity and opt-in Windows startup.
- Local validated metadata cache with background refresh and activation checks.

## Windows requirements

Windows 10 x64 1809 or later / Windows 11 x64. A working OpenGL driver is required
for normal rendering; a software fallback is available. This release statically
links its Microsoft C runtime and embeds app assets. End users do not need Rust,
Visual Studio, a source checkout or a separate Visual C++ redistributable.
Clean-machine GUI compatibility still requires the maintainer's manual test.

## Install or use the portable ZIP

Once approved, use the repository's Releases page to obtain these matching files:

- `Ferrum-v0.1.0-beta.2-windows-x64-setup.exe`
- `Ferrum-v0.1.0-beta.2-windows-x64-portable.zip`
- `SHA256SUMS.txt` and `RELEASE_NOTES.md`

Verify SHA-256 against the approved release channel before manual execution.
This beta is unsigned; no Microsoft certification, trusted publisher signature
or SmartScreen exemption is claimed. Do not disable security protections.

Close earlier copies yourself. The installer is per-user, defaults to
`%LOCALAPPDATA%\Programs\Ferrum`, and creates a Start Menu entry and uninstaller.
It never enables startup or kills running apps. Desktop shortcut and final
Launch Ferrum are optional and unchecked. You do not need to retain the installer.

The ZIP needs no installation: extract it into a writable folder and manually run
`ferrum.exe`. **Portable still stores settings/cache in AppData**, not beside the
executable. Installed, portable and participating earlier builds share one
instance identity; do not run them together. See [installation and removal](docs/INSTALLATION.md).

## Open, search, configure and exit

**Ferrum always starts hidden.** Press **Alt+Space**, or your saved/fallback
shortcut. It does not show a normal application window merely because you opened
the executable. Indexing runs in the background; a first or invalidated-cache
start can take several seconds before the full index is ready.

Type to search. Up/Down selects; Enter activates. Escape or clicking outside
hides the palette. Use its **Settings** button to edit preferences and **Exit**
(or Settings > Exit application) to stop the background process. Closing Settings
hides only that window. Refresh updates discovery on its existing worker.

Supported shortcuts: Alt+Space, Ctrl+Alt+Space, Ctrl+Shift+Space, Alt+Shift+Space.
Changes preserve the old registration when the new one fails. Actual shortcut
status appears in Settings and the initial `hotkey-status.txt` beside config.
If every shortcut conflicts, close the existing process yourself and use
`ferrum.exe --settings` explicitly for recovery. A second invocation exits quietly;
it cannot remotely open Settings in an already-running instance.

General searches use Google, DuckDuckGo, Bing or Brave as selected in Settings.
Shift+Enter forces general web search for ordinary input; `? words` is explicit
web mode. `@` lists configured commands; type a prefix, select, then Tab/Enter
completes an alias without launching an empty search. Defaults: `@yt`, `@wiki`,
`@google`, `@reddit`, `@steam`. Complete `@` searches keep their own providers.
Recognized HTTP(S)/domain input offers direct opening. Unknown/unsafe protocols
are rejected; no general shell execution is inferred from text.

## Settings, startup and uninstall

Preferences remain at `%LOCALAPPDATA%\SpotlightLauncherPrototype\config.json`
for backward compatibility. Custom aliases/providers/hotkeys/opacity are preserved.
Missing fields receive defaults; malformed JSON is retained rather than replaced.
External config edits require restart. Background opacity is 85-100%, default 92%;
unsupported/software rendering reports effective 100% instead of pretending alpha
was applied. Software mode keeps its established no-zoom fallback.

Launch on Windows startup is off by default and controlled only in Settings.
Neither Setup nor ordinary Ferrum startup enables or migrates registration.
After moving from a development/portable executable, explicitly enable the setting
in the installed copy to approve repair of a recognized own entry. Stable-path
upgrades keep a valid user-enabled registration working. Uninstall removes only
an exact verified startup command for that installed copy, preserving commands
for other copies and all AppData preferences. See [installation details](docs/INSTALLATION.md).

## Privacy

Ferrum locally indexes app/game metadata and names/paths of supported files in
bounded common folders. It does not index document contents, require an account,
add telemetry or send your index/query text to servers while typing. The local
`ferrum-index-v1.json` cache contains private metadata, not file contents or search
history. Parameterized shortcuts and Modrinth profiles are fresh-only; cache
activation always uses current typed-action validation. Cache invalidation by
version/config/age falls back to ordinary discovery.

Only activating a website result asks your default browser to open a URL and send
search words to that selected provider/site, subject to its own policies. Local
launch actions ask Windows or the relevant installed launcher to open the target.
Optional opt-in aggregate diagnostics contain no query text or filenames. Do not
attach private caches, config, discovery snapshots or unredacted logs to issues.

## Integration and beta limitations

- Steam discovery depends on readable installed-library metadata; Steam must be installed.
- Modrinth profile results currently open the discovered Modrinth application only.
- Packaged/Xbox discovery does not guarantee that every Game Pass title is launchable.
- Bounded folder scanning may omit entries; remote/reparse/offline/recall files are skipped.
- Some cached metadata can be stale until refresh; invalid actions show an error.
- Pre-mutex prototypes cannot coordinate automatically; close them yourself.
- Fullscreen focus, display scaling, GPU behavior and real sign-in execution need manual checks.
- This beta has no auto-updater, service, driver, installer-enabled startup or account system.

## Build and release from source

See [BUILD.md](docs/BUILD.md) for the pinned MSVC release recipe and runtime checks.
The desktop source requires Windows, Rust/MSVC and Windows SDK tools. Release
scripts use Rust 1.99.1, Cargo.lock, static CRT, embedded Slint resources and source
path remapping. Python 3.8+ supports packaging helpers; existing derived icons are
tracked, so Pillow is needed only if you deliberately regenerate them.

```powershell
./scripts/Build-Release.ps1
./scripts/release.ps1
```

The owner selected the MIT source license; it is included in each package.
Packaging does not approve publication: artwork distribution confirmation and
manual release validation remain required. No script launches the
Ferrum GUI or its installer. See [RELEASING.md](docs/RELEASING.md) for clean source
export, protected draft-only GitHub automation, signing and approval requirements.

## License and notices

**Ferrum source code is licensed under [MIT](LICENSE)**, as selected by the owner.
Artwork distribution rights still require owner confirmation before publication.
Third-party code has its own licenses; see [notices](THIRD_PARTY_NOTICES.md).
Slint uses its royalty-free desktop license with the AboutSlint widget in Settings.
No development-machine font files are redistributed. The original user-provided
Ferrum.png is retained unchanged; derived icons are used for application branding.

For security/private reporting and remaining owner setup, see [SECURITY.md](SECURITY.md).

## Optional update notifications

[Official source and releases](https://github.com/Mayopotato1/Ferrum).
Automatic update checks default Off. Settings provides a manual check even while
Off; opting in allows approximately daily requests to GitHub's public Releases API.
These requests send no index, filename, query or settings data. A compatible newer
beta/stable release can show a small banner on the next overlay opening. View update
opens its validated official release page for manual download/installation. Later
dismisses that release. There is no automatic download or installer. See
[update configuration and isolated fixtures](docs/UPDATE_CHECKER.md).
