# Dependency and runtime review

Reviewed 2026-11-07 against the locked dependency graph. The HTTP/native-TLS dependencies added in Phase 1.6 are included in this scan. See audit-summary.json for the Cargo.lock hash,
RustSec database commit and exact advisory IDs.

cargo-audit 0.22.2 found **zero known vulnerability advisories** in Cargo.lock
using RustSec database commit 309392b18f6b17022f82c701f300433126901068
(updated 2026-11-06). This is a dated advisory check, not a security certification.
The six informational maintenance notices cover bincode, pin-utils, ttf-parser,
zbus_names, zvariant and zvariant_derive. Only pin-utils 0.1.1 is in the actual
Windows normal/build dependency graph; it is unmaintained (RUSTSEC-2026-0441).
The other five occur in lockfile alternatives outside this Windows build.
Dependency modernization is deferred to a separate, tested change.

The release notices collector uses cargo tree for the actual Windows normal/build
graph: 298 third-party packages, including build-time components conservatively.
Declared licenses, upstream source URLs and available license/copyright/notice
texts are included in each binary distribution. Pinned upstream license overrides
cover crates whose published archive lacks the full license text. The collector
fails if a required notice remains missing. See THIRD_PARTY_NOTICES.md for Slint,
MPL-2.0, SQLite and toolchain obligations. Ferrum source is MIT-licensed;
third-party license obligations remain separate.

The x64 MSVC release links the C runtime statically and embeds Slint resources.
PE import inspection found only Windows system/API-set DLLs, including OpenGL,
DirectWrite and DWM; no separately deployed VC runtime DLLs. No DLLs or fonts
are bundled. A compatible Windows graphics driver remains necessary. Resource
inspection verifies all seven icon sizes and version metadata. These static
checks do not prove GUI operation on a clean Windows machine: complete the
manual checklist, including a fresh VM without Rust/VC redistributable installed.

Upstream security data: https://github.com/RustSec/advisory-db
Slint licensing: https://slint.dev/license/royalty-free
Inno Setup: https://jrsoftware.org/
