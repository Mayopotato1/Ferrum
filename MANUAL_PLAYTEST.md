# Ferrum v0.1.0-beta.1 manual release checklist

Status: **Not performed**. No Ferrum GUI or installer was executed during this
packaging phase. Headless tests cannot establish focus, appearance, graphics
compatibility, installation or uninstall correctness. Earlier phases had user
playtests; the complete Phase 1.4 GUI check and this beta checklist remain open.
Record Windows version, DPI, results and failures before approving publication.
Use a disposable Windows VM for installation, upgrade and uninstall checks.
Do not change your everyday startup registration merely to test this candidate.

## Package and installation (clean VM)

- Verify SHA256SUMS.txt; inspect unsigned publisher/SmartScreen status using normal
  Windows security controls. Do not disable protection to complete a test.
- Install as a standard user without elevation. Confirm the Ferrum name, beta
  version, icon, LocalAppData Programs destination and Installed Apps entry.
- Confirm Start Menu shortcut; desktop shortcut and final Launch are unchecked.
- Finish without Launch: no Ferrum process, startup entry, service, scheduled task
  or firewall rule should be created. Installer can be deleted after installation.
- Launch manually: starts hidden, no console/window flash. Open with Alt+Space or
  configured fallback. Check focus, outside-click dismissal, Escape, reopening,
  explicit Exit, no normal taskbar/Alt+Tab presence or permanent topmost window.
- Check original artwork, opening animation, opacity and Settings About/version
  and Slint license credits. Type during animation; test repeated open/close.
- Check multiple monitors and 100/125/150% scaling, taskbar work areas and display
  disconnection. Check a clean machine's graphics driver/software fallback.

## Preserved behavior

- Search installed Windows apps, files/folders, Steam, Modrinth and Xbox entries.
  Manually activate representative valid targets. Verify icons and responsiveness.
- Check ordinary web search, Shift+Enter, safe direct URLs, @ discovery, custom
  aliases, Tab/Enter completion, empty/unknown aliases and Unicode encoding.
- Change search engine, shortcut and opacity; restart manually and check settings
  persistence. Do not claim historical backend timings describe GUI startup.
- Launch a second installed/portable copy: one existing background instance only.
  Confirm failures/conflicting hotkeys are reported and cleanup permits restart.

## Startup, upgrade and cancellation (VM only)

- Startup is disabled by default. Enable only from Settings; verify its quoted
  executable command targets the installed copy, and Windows login starts hidden.
- Development/portable startup migration: inspect the old target, then explicitly
  disable the owned old entry and enable the installed entry via Settings. Verify
  user-approved repair and no unrelated startup values are changed.
- Attempt reinstall/upgrade while Ferrum runs: clear close/retry/cancel instruction,
  no forced termination, no partial payload replacement, no restart/reboot.
- Cancel with Ferrum still running: existing installation remains functional.
- Exit Ferrum yourself, then retry upgrade. Same installed path, config, custom
  aliases, cache and user-enabled startup command should survive; no auto-launch.
- Test a race where Ferrum is reopened before file replacement; expect a safe
  retry/cancel path, never forced closure or restartreplace.

## Uninstall (VM only)

- Running Ferrum blocks uninstall with safe close instructions. Cancel preserves
  the application. Exit manually, then uninstall from Installed Apps.
- Confirm installed binaries, logged shortcuts and uninstall metadata are removed.
  Preferences, custom aliases, cache and personal content must remain.
- An exact quoted HKCU startup command for this installed executable (Ferrum or
  recognized legacy value name) is removed. A portable/development command,
  extra arguments or unrelated value must remain. Test both 64-bit registry view
  ownership and failures without granting elevated privileges.
- Uninstall remains possible while Ferrum is already stopped or cannot start.

## Portable distribution

- Extract to a different folder, without the source tree or Rust installed.
  Launch manually and test the same functions. Settings/cache are in AppData;
  this is no-install distribution, not fully self-contained storage.
- Startup is never registered automatically. Avoid installed/portable copies
  running together; close Ferrum explicitly before moving/removing its folder.

Approval record: tester / date / environment / failures / decision: **pending**.

## Phase 1.6 update notifier — user-run checks only

Development did not open Ferrum, a browser, a game or an installer. First test the
new private candidate, not the older Phase 1.5 packages. The rebuilt candidate's
source is pinned to Mayopotato1/Ferrum; automatic checking remains Off. See docs/UPDATE_CHECKER.md for isolated debug
fixture build steps; the fixture mode cannot be built into release binaries.

1. Exit the currently running Ferrum yourself before testing another copy. Start
   the candidate manually: it must start hidden. Alt+Space must open promptly.
2. Open Settings. Updates must show installed version 0.1.0-beta.1, automatic
   checking Off on a fresh isolated profile, and clear update status.
   Check for updates now must work even with automatic Off; no releases means up
   to date only after a successful response, while offline/rate-limit errors must
   report inability to check.
3. Use the separate debug fixture/profile instructions in docs/UPDATE_CHECKER.md
   to demonstrate Checking then a newer beta, with no HTTP. Keep typing/opening
   the overlay while checking; verify it remains responsive. A missing fixture
   or malformed JSON must report failure without a crash or an up-to-date claim.
4. On the next overlay opening after a new fixture release, inspect the subtle
   banner. Search input/results must remain usable; type, use arrow keys, Enter,
   Shift+Enter, @ autocomplete, Escape, hotkey, outside click and animation.
5. Click Later. Reopen repeatedly and restart the fixture build: the same release
   must remain dismissed. Change the fixture tag and matching URL to a newer
   beta.N, then check again: that new release may notify once more.
6. Headless tests prepare View update's exact canonical URL without opening it.
   Do not navigate to the synthetic fixture repository. Once a controlled release exists in Mayopotato1/Ferrum, personally verify View update
   and Settings' Open release page use its exact official HTTPS GitHub URL.
7. With the configured official source, test offline/slow/rate-limited
   responses and automatic opt-in. Verify no focus changes/Windows notifications,
   no repeated checks on Settings/overlay opening, and no checks after disabling.
8. Verify Steam launching, applications, @ aliases, web search and opening/hiding
   still work. Observe idle CPU and memory personally; headless measurements do
   not establish actual GUI performance. No asset or installer should download or
   execute, and no startup setting should change during update tests.
9. Restore the original process environment after fixture tests. Keep the fixture
   build and its isolated profile out of distribution. Remaining installer,
   startup ownership, licensing and public-release checks above still apply.
