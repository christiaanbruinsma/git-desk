# Git Desk Release Pipeline

Git Desk uses the same tag-bound, Flatpak-first release model as the other Rust applications in the suite.

The official release flow creates four immutable assets from one Git tag:

- `git-desk-vX.Y.Z.zip`
- `git-desk-vX.Y.Z.zip.sha256`
- `git-desk-vX.Y.Z.flatpak`
- `git-desk-vX.Y.Z.flatpak.sha256`

## Manifest split

`io.github.christiaanbruinsma.GitDesk.Devel.yml` is the GNOME Builder/development manifest. It builds `io.github.christiaanbruinsma.GitDesk.Devel` with `-Dprofile=development` and exports gettext catalogs through the `.Locale` extension for local localization QA.

`io.github.christiaanbruinsma.GitDesk.yml` is the production / standalone GitHub release manifest. It builds `io.github.christiaanbruinsma.GitDesk` with the production profile and embeds all gettext catalogs in the single `.flatpak` bundle.

Both profiles use the same Git Desk source and host-Git bridge. Git Desk delegates Git commands to the host through `flatpak-spawn --host`, preserving the user's existing Git configuration and credentials.

## Before tagging

The working tree must be clean and `Cargo.lock` must contain the localization dependencies produced by the successful Builder build.

Run:

```bash
./scripts/check.sh
```

Then commit the final source, create the repository/tag, and push both `main` and the tag.

## Full release

For v0.9.0:

```bash
./release/release.sh v0.9.0
```

The pipeline:

1. requires a clean worktree and existing local tag;
2. creates a source ZIP directly from that tag;
3. tests the extracted tagged source;
4. restores a pristine copy of the tagged source;
5. builds the standalone Flatpak in an external release workspace;
6. verifies all six gettext catalogs, desktop metadata and app icon;
7. creates and verifies SHA-256 files;
8. structurally imports and audits the exact Flatpak bundle;
9. installs that exact bundle in user scope and runs acceptance checks;
10. writes a PASS/FAIL QA report;
11. publishes only after explicit `PUBLISH` confirmation;
12. downloads the published assets again and compares them byte-for-byte.

The default external workspace is:

```text
../release-work/git-desk-vX.Y.Z/
```

## Locale smoke tests

After installing the exact release bundle:

```bash
./scripts/run-locale.sh nl
./scripts/run-locale.sh de
./scripts/run-locale.sh fr
./scripts/run-locale.sh es
./scripts/run-locale.sh it
./scripts/run-locale.sh pt
```

English is the source language:

```bash
./scripts/run-locale.sh en
```

## Published release immutability

Do not overwrite a published asset just because a later rebuild differs. Normal fixes must use a new version and Git tag.
