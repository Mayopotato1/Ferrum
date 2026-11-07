# Ferrum v0.1.0-beta.1

First public-beta candidate; **not yet approved for publication**.

- Per-user Windows installer with Start Menu entry, independent uninstaller,
  safe running-instance checks and optional unchecked desktop/launch actions.
- Portable Windows x64 ZIP; no installation required, preferences still in AppData.
- Ferrum artwork embedded as a multi-resolution Windows icon and native window icon.
- Consistent SemVer beta metadata, static C runtime, embedded Slint assets,
  dependency notices, checksum generation and gated release automation.

The existing search engine, discovery, game/web actions, keyboard navigation,
overlay, 170 ms animation, settings, startup opt-in and cache remain protected.
The update notifier adds a small Settings section and optional in-overlay banner;
there is no overlay redesign.

Ferrum starts hidden. Press Alt+Space or the saved/fallback shortcut, then use
Settings to adjust preferences or Exit to stop it. Close earlier copies before
installation or switching between installed/portable builds. The installer never
silently kills them or enables startup. Development-path startup entries require
your explicit Settings repair after moving to the installed copy.

Earlier backend-only measurements (26.7 ms warm cached availability versus about
4.09 s without cache) are historical observations, not GUI startup guarantees.
The complete earlier GUI playtest and clean-machine beta installer tests remain
outstanding. Modrinth profiles currently open the Modrinth application only;
packaged/Xbox discovery does not guarantee every Game Pass title is launchable.

This candidate is unsigned. No trusted signature, certification or SmartScreen
exemption is claimed. Do not disable security protections. Verify SHA256SUMS.txt
against the maintainer's approved source before any manual execution.

Ferrum source code is MIT-licensed, as selected by the owner.
Publication blockers: artwork distribution confirmation and owner approval;
remaining GUI validation; manual clean-VM install/upgrade/uninstall and startup
migration tests. These files remain private owner-review candidates.

This rebuilt beta includes the Phase 1.6 GitHub update notifier, pinned to the
[official repository](https://github.com/Mayopotato1/Ferrum). Automatic checks are
Off by default; manual checks work independently. Beta and stable releases are
selected using SemVer with strict official-release URL validation. Updates only
notify and open the release page on request; no assets are downloaded or installed.
All prior local candidate packages are superseded by this rebuilt candidate.
