# First-public-beta publication checklist

The prepared version is **v0.1.0-beta.1**, MIT source, unsigned Windows x64.
The updater source is compile-time pinned to Mayopotato1/Ferrum, beta channel.
Automatic checking defaults Off. No release assets are downloaded by the updater.

## Completed locally

- Source, license, derived icons and update-source configuration reviewed.
- Formatting, strict release Clippy, Rust tests, 64-case relevance validation,
  packaging-policy tests and static-runtime release build run by Build-Release.ps1.
- Installer compiled only; portable/source ZIPs, notices and SHA-256 sums rebuilt.
- Compiled update-source inspection rejects old/unconfigured/fixture binaries.
- Public source allowlist and credential/local-identity scan; historical private
  development reports stay local. The artwork source remains unchanged.
- Actions references checked against their exact upstream commit IDs. Workflows
  have read-only defaults, pinned Rust/actions/Inno, locked dependencies, normalized
  ZIP metadata, and separately gated draft-only publication. CI itself has not run.

See the local PUBLICATION_PREPARATION_REPORT.md and dist/v0.1.0-beta.1 verification
files for observed results. These do not establish GUI or installer correctness.

## Required owner/manual checks

- [ ] Confirm distribution rights for Ferrum.png and derived artwork.
- [ ] Complete MANUAL_PLAYTEST.md: hidden start, hotkey/focus, search/Steam/web/@,
      updates/settings/banner/Later, animation, outside click, scaling and resource use.
- [ ] Complete clean-Windows-VM install, running-copy cancellation, upgrade,
      uninstall, startup ownership, persistence and renderer/runtime checks.
- [ ] Review unsigned-beta limitations, notices, maintenance warnings and hashes.
- [ ] Configure private vulnerability reporting and main/tag protection on GitHub.
- [ ] Configure the public-beta environment with required reviewers before enabling
      any draft-release workflow variable. Leave FERRUM_ENABLE_DRAFT_RELEASES unset.
- [ ] Confirm the exact final source commit and checksums; approve the source push
      separately from any tag, draft upload or public release.

Do not mark packaging/release-policy.json approval/manual flags true merely because
headless checks pass. If any packaged source/payload/license/notes change afterward,
archive the prior review candidate and regenerate all packages and hashes.

## Commands — only after explicit approval

From the original development directory, inspect the prepared safe main branch:

```powershell
git remote -v
git log --oneline main
git diff --stat origin/main..main
git ls-tree -r --name-only main
git status --short
```

The development working branch is main; private development history is preserved
on the local master branch. The reviewed main branch contains only the existing public README history and the
clean publication snapshot. **Never push master, --all, --mirror or backup bundles.**
The isolated publication-source checkout contains the same reviewed main commit.

After source-push approval, the intended command is:

```powershell
git push -u origin main:main
```

This preparation does not execute that command. If remote main advances first,
fetch/reconcile it without force-pushing and re-review the final tracked tree.
Wait for CI and inspect its logs. Source push is not approval to tag or release.
No tag command is provided until manual release checks and release approval exist.

## Local rebuild commands

```powershell
python -m unittest discover -s packaging/tests -v
./scripts/Build-Release.ps1
./scripts/Assert-UpdateSource.ps1 -Binary ./target/x86_64-pc-windows-msvc/release/ferrum.exe
# Archive existing dist/version and target/release-staging/version before packaging.
./scripts/release.ps1 -Binary ./target/x86_64-pc-windows-msvc/release/ferrum.exe -ExpectedTag v0.1.0-beta.1
python scripts/verify_release.py dist/v0.1.0-beta.1 --payload target/release-staging/v0.1.0-beta.1
python scripts/audit_publication.py .
```

These scripts do not publish or launch the GUI/installer. The original local
report includes installer archive extraction and exact artifact locations.
Cross-host byte reproducibility is not certified: Windows image/SDK and Python
3.12 servicing revisions can change. Repeat-build/source-snapshot comparisons
and deterministic ZIP fixture tests are evidence only for their stated scope.
