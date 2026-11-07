Ferrum v0.1.0-beta.2 - Windows x64 portable

Extract the whole ZIP into a writable folder, then run ferrum.exe yourself.
Ferrum always starts hidden. Press Alt+Space (or the saved/fallback shortcut),
then use Settings to edit preferences or Exit to stop the background process.

Portable means NO INSTALLATION REQUIRED, not self-contained user data.
Settings and private search metadata are stored in
%LOCALAPPDATA%\SpotlightLauncherPrototype for compatibility with earlier builds.
No Windows startup entry is created unless you explicitly enable it in Settings.

Do not run installed and portable/development copies simultaneously. They share
one instance identity. Close the existing copy yourself before switching.
Moving a startup-enabled portable executable requires explicit repair in Settings.

Requires Windows 10 x64 1809 or later / Windows 11 x64, a working OpenGL driver
for the normal renderer, or the documented software fallback. No Rust toolchain
or Visual C++ redistributable is required by this static-CRT build.

This unsigned beta may trigger Windows reputation warnings. Verify the SHA-256
checksum against the approved release source. Do not disable Windows protections.
GUI, clean-machine install/upgrade/uninstall and remaining previous-phase GUI
checks must be completed by the maintainer before public release.

Read LICENSE.txt, THIRD_PARTY_NOTICES.txt and INSTALLATION.txt before distribution.
Ferrum source code is MIT-licensed. This is still a private review candidate:
artwork distribution confirmation and manual release approval remain pending.
