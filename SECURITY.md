# Security and private reports

Ferrum is an unsigned beta candidate. Do not disable Windows security protections
to run it. Verify releases and checksums through a trusted maintainer channel.
Checksum integrity is not a code-signing or publisher-trust certificate.

Do not publish private filename indexes, configuration, diagnostics or discovery
snapshots in issues. Redact paths, queries, credentials and personal identifiers.
The official repository is https://github.com/Mayopotato1/Ferrum. The owner must
configure GitHub private vulnerability reporting before public publication; no
private-reporting setup is claimed by local preparation.

The checked lockfile received a RustSec scan during packaging. No known
vulnerability advisories were found in that snapshot. Informational maintenance
warnings are documented in docs/DEPENDENCIES.md; this is not a guarantee of safety
or a substitute for keeping dependency review current. The current lockfile includes Phase 1.6 HTTP/native-TLS dependencies.
