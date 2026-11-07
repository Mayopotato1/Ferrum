# Ferrum v0.1.0-beta.1 installation

This is an unsigned Windows x64 beta candidate. The maintainer has not completed
the clean-machine installation/upgrade/uninstall or remaining earlier GUI tests.
Source code is MIT-licensed. Public distribution still requires artwork rights
confirmation, manual validation and owner approval.

## Requirements

Windows 10 x64 version 1809 or later, or Windows 11 x64. A working OpenGL driver
is needed for the normal Slint renderer. For a manual software fallback, set
SLINT_BACKEND=winit-software only in the environment used for your launch; that
path uses opaque rendering and no opening zoom. Do not change system graphics
or security settings. The release binary statically links its Microsoft C runtime;
no Rust compiler, source tree or separately installed VC++ runtime is required.
Actual clean-machine GUI behavior remains a manual release test.

## Installer

1. Obtain Ferrum-v0.1.0-beta.1-windows-x64-setup.exe and SHA256SUMS.txt from the
   maintainer's approved release source. Compare its SHA-256 before running it.
2. Close every running Ferrum / earlier launcher yourself using Exit. Setup
   checks the shared mutex and refuses to proceed while a participating copy runs.
   It never kills Ferrum, games, Explorer or another application.
3. Run Setup manually. It needs no administrator privileges and installs by
   default to %LOCALAPPDATA%\Programs\Ferrum. Keep the stable installed path for
   future upgrades. Setup creates a per-user Start Menu shortcut and uninstaller.
4. Desktop shortcut and final Launch Ferrum are optional and unchecked by default.
   Setup never enables Windows startup, creates services/tasks/drivers, adds
   firewall rules, or changes antivirus exclusions.
5. Ferrum always starts hidden, including an explicitly selected final launch.
   Press Alt+Space or your saved/fallback shortcut. Use Settings for preferences
   and Exit to terminate. The original installer is not needed to keep it working.

Unsigned beta assets may receive SmartScreen/reputation warnings. Do not bypass
or disable security protections on the maintainer's instructions. If uncertain,
cancel and wait for an approved/signed release or contact the maintainer.

## Upgrade and startup migration

An upgrade uses the same installer AppId and installed directory. Close Ferrum
before proceeding. Cancelling the running-instance prompt leaves the application
alone. Files are not queued for forced replacement/reboot; the application is a
single binary with embedded assets, and Setup never silently terminates it.

Preferences/custom aliases remain in
%LOCALAPPDATA%\SpotlightLauncherPrototype\config.json. The adjacent private
ferrum-index-v1.json cache is preserved, but app-version changes invalidate its
contents and may require one normal discovery pass before a warm cached start.

Setup does not migrate existing startup values. If a development/portable entry
points elsewhere, open the installed Ferrum's Settings and explicitly enable
Launch Ferrum on Windows startup to approve repair/migration. Its native code
validates the two known Ferrum/legacy names and refuses unfamiliar payloads.
Future upgrades at the same installed path keep a valid user-enabled command
working without rewriting it. Startup remains opt-in and off by default.

## Uninstall

Close Ferrum yourself, then use Windows Settings > Installed Apps > Ferrum, or
run the installed uninstaller manually. Ferrum does not have to be functional to
uninstall. The uninstaller is independent of the application executable.

It removes installer-owned files, shortcuts and installation metadata. In the
64-bit current-user Run key, only the Ferrum and legacy product values whose
entire command exactly equals the quoted executable at THIS installed path are
removed (case-insensitive Windows path comparison). A portable/development
target, extra arguments, expanded-variable command or unrecognized payload is
left untouched. Ambiguous/stale foreign commands require your explicit review.
If deletion of a verified own value is denied, uninstall reports it before its
file-removal step; permissions/failure behavior still needs the manual VM test.

Settings, aliases, cache and diagnostics in AppData are preserved. No recursive
AppData deletion or data-removal option is supplied in this beta. Personal
documents, downloads, Steam libraries and Modrinth profiles are never uninstall
targets. Installer file logs, not broad wildcard deletion, determine program files.

## Portable ZIP

Extract Ferrum-v0.1.0-beta.1-windows-x64-portable.zip and run ferrum.exe manually.
It requires no installation but still uses the same AppData preferences/cache.
It creates no startup entry unless you explicitly enable the preference. Do not
run portable and installed copies together: the shared mutex admits one instance.
To remove a portable copy, close it yourself and delete its extracted program
folder; if you enabled its startup entry, disable that preference first.
