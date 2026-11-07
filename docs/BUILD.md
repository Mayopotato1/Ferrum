# Reproducible Windows release procedure

The source checkout remains the same Ferrum application. Release version is read
from Cargo.toml; no customer-facing phase label is used. Beta v0.1.0-beta.1 maps to
Windows numeric 0.1.0.1. In general beta.N maps to revision N (1..65534); a final
major.minor.patch uses revision 65535 so it orders after those betas. Resource,
About and CLI versions derive from the package version; installer metadata is
read back from the built executable. Update current-version test expectations
when deliberately advancing the release.

Prerequisites: Windows x64, Rust 1.99.1 MSVC with rustfmt/Clippy, installed MSVC and
Windows SDK resource compiler, Python 3.8+. Inno Setup 6.7.4 is obtained by the
pinned archive-only bootstrap if absent. No compiler installer is executed, no
machine-wide application is installed, and no elevation is requested. Tool
URLs/SHA-256 and extracted compiler-file hashes are in packaging/tools.json.
The CLI unpacker is pinned to its upstream commit; the official Inno archive and
extracted compiler must have the trusted Pyrsys B.V. Authenticode signature.

Run scripts/Build-Release.ps1. It checks formatting, strict release-target Clippy,
all headless tests, release build, --version and 64/64 --relevance from a separate
working directory. It never runs Ferrum without an explicit headless mode.

The distribution build uses explicit x86_64-pc-windows-msvc, +crt-static,
SLINT_EMBED_RESOURCES=true and release mode. Compiler/debug source paths are
remapped to generic build roots, C __FILE__ paths are mapped, and the linker uses
/Brepro. These are process-scoped environment values restored by the script.
There is no target-cpu=native, system setting change or new runtime dependency.
Output: target/x86_64-pc-windows-msvc/release/ferrum.exe. Prior developer/installed
executables are deliberately not overwritten or terminated.

Run scripts/release.ps1 to rebuild/validate and package, or pass -Binary with an
already validated executable of exactly the expected branding/version. The MIT
source license is included; publication still requires the release-policy approvals.
Output goes to dist/v0.1.0-beta.1. Existing staging/output directories are refused
rather than silently retaining stale assets/hashes; move previous candidates
aside deliberately inside the workspace before rebuilding.

The script derives the installer and portable ZIP from the same staged binary,
collects exact Windows normal/build license notices, compiles the Inno script,
exports an allowlisted clean source ZIP, creates actual SHA-256 sums and verifies
ZIP contents against staging. inspect_binary.py reads the PE import/resource
headers without loading/executing the file: x64 GUI subsystem, seven icon sizes,
version resource, VC-runtime imports and private build prefixes are checked.
The observed imports are Windows OS/API-set DLLs; GPU driver support still needs
a clean-machine manual GUI test. No compiler, OS DLL, font or source tree is a
runtime asset. Headless success alone does not prove renderer/focus behavior.

ZIP ordering and entry timestamps are normalized. Rust, dependency lockfile,
Inno compiler and workflow actions are pinned; hosted Windows SDK/image and
Python servicing revisions can still change. This is a reproducible procedure,
not an unverified claim of byte-for-byte identity across different machines.
Do a two-clean-host comparison before advertising stronger reproducibility.

Icon regeneration is optional: python scripts/generate_icon.py with Pillow 10.4.0.
It reads Ferrum.png, preserves aspect ratio and alpha, and writes only derived
ICO/RGBA/metadata files. The original must never be overwritten. The current
1364x1364 image is fully opaque; no artificial transparency or replacement logo
is introduced. CI consumes the committed derived assets without font files.

The official GitHub update identity/channel is tracked in packaging/update-source.json.
No environment setup is required. Conflicting source overrides are rejected outside
explicit debug fixture builds. Assert-UpdateSource.ps1 checks every packaged binary
headlessly for the exact repository/channel/version and fixture-off state.
