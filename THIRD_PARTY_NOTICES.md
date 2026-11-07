# Third-party licensing

Ferrum source code is MIT-licensed, as selected by the owner. Artwork distribution
rights/terms remain subject to owner confirmation. No blanket claim that all
dependencies are MIT-licensed is made.

The release script generates a locked Windows normal/build dependency inventory
and copies upstream license/copyright/notice files into the package's licenses
directory. Missing license texts are supplied from pinned upstream commits under
packaging/license-overrides; hashes and source URLs are recorded there. Build-time
components are included conservatively, while inactive optional/Linux-only graph
entries are excluded using Cargo's actual Windows dependency tree.

Slint 1.18.1 is used under its Royalty-free Desktop, Mobile, and Web Applications
License 2.0, with the AboutSlint attribution widget in Settings / About and its full
license in the package. This is separate from the owner's Ferrum source-license
choice. Nucleo-matcher is unmodified MPL-2.0; the notice inventory links the exact
crate's Source Code Form. SQLite's engine is public domain; its wrapper retains
its own license. Rust standard-library MIT/Apache notices are included. The
generated installer uses Inno Setup's engine under its included license.

No development-machine font files, SDK tools or OS DLLs are redistributed.
The user-supplied Ferrum.png is raster artwork; only derived icons are embedded
in distributables, and the original is retained unchanged in the source snapshot.

This automated inventory is an engineering compliance aid, not an assertion that
all legal/publication decisions are complete. Resolve LICENSE-PENDING.txt and
the release checklist before publishing.
