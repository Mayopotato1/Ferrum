# Owner approval and GitHub release

The owner created https://github.com/Mayopotato1/Ferrum. Local preparation
configures origin and prepares main; it does not push, tag or publish.
All current assets are private review candidates, unsigned and not yet approved.

Before publication:

1. The owner selected MIT for Ferrum source. LICENSE, Cargo.toml and release
   policy record that decision. The original artwork is user-provided and reused
   as requested; confirm distribution rights/terms before setting
   artwork_distribution_confirmed. No font files are redistributed.
2. Complete the remaining earlier cached-start GUI checks and clean Windows VM
   install, reinstall/upgrade, running-copy cancel, uninstall, startup ownership,
   persistence, icon and full application regressions in MANUAL_PLAYTEST.md.
   Only then set the corresponding validated flags true. Do not infer these from
   headless tests or successful installer compilation.
3. Review notices/dependency maintenance findings and unsigned-beta risk. Set
   publication_approved only after actual owner approval. Rebuild artifacts and
   regenerate checksums whenever payload, license, notes or signing changes.
4. Verify final package extraction/contents and exact SHA256SUMS.txt hashes.

## Publishable source without private development history

The existing development repository has historical local paths/reports. Do NOT
blindly push that history. The generated Ferrum-v0.1.0-beta.2-source.zip is an
allowlisted source tree: code, tests, resources, packaging, workflows and current
documentation; no .git, private reports, target outputs, caches or user config.
The local publication-source directory is a separate audited checkout of the
existing public main history (its README-only commits are retained). Its new
publication commit contains only this allowlisted tree. The same safe main branch
is imported into the development repository. Private development history is
retained on master; the working branch is switched to the audited main after
verifying that their current source trees match.
Never push master, --all, --mirror or private backup bundles. After explicit owner
approval, use only `git push -u origin main:main` from the development repository.
That is a normal fast-forward from the existing remote main; do not force-push.

The source identity is pinned in packaging/update-source.json. Both build and
packaging validate the embedded --update-source output. Confirm GitHub private
vulnerability reporting and branch/tag/environment rules separately. No credential
is embedded. See PUBLICATION_CHECKLIST.md for the remaining approval steps.

## CI and draft releases

.github/workflows/ci.yml validates pull requests/main with contents:read and no
publish step. .github/workflows/release.yml builds manually or on v* tags and
requires a selected license before packaging. Tag and Cargo versions must match.
The files use JSON syntax, which is valid YAML, allowing standard-library policy
validation without a YAML parser. Action SHAs/ref metadata are recorded in
packaging/actions.lock.json; updates require owner review, not floating versions.

The separate contents:write job can run only for a tag push when the repository
variable FERRUM_ENABLE_DRAFT_RELEASES equals true. It also uses the public-beta
GitHub environment and checks every release-policy approval flag. Before enabling
it, configure that environment with required reviewers and restricted deployment
tags, and protect version tags against untrusted creation/modification. An
unconfigured GitHub environment does not enforce approval by itself: leave the
variable unset until its protection is verified. No PR gets publishing rights.

The job verifies artifact checksums and creates/updates only a DRAFT PRERELEASE;
it refuses to replace an already-public release. It never makes a release public.
The owner reviews the intended assets/notes and manually publishes later. GitHub
Actions has not been run against a repository during local preparation.

## Legitimate code signing later

No certificate was provided, bought, generated or used. Current app/setup assets
are genuinely unsigned; no trusted-publisher claim is made. Do not bypass Windows
security or use a self-signed certificate as public trust.

To sign later, use an owner-authorized real certificate/signing environment to
sign the validated payload first. Pass that already valid signed executable via
release.ps1 -Binary and supply -InnoSignCommand with the authorized signing command.
The SignedRelease Inno branch signs Setup and its generated uninstaller through
Inno's SignTool support. Never merely post-sign Setup and assume its uninstaller
is signed. No signing credentials are embedded in scripts/workflows; provision
any future secret only in the protected publishing environment with explicit
owner authorization. The signing branch remains untested without a real cert.
Repackage and regenerate all hashes after signing; no Rust source rebuild is
required solely for signing, although the installer/ZIP necessarily change.
