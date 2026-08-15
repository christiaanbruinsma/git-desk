# Git Desk

**Easy to start. Powerful enough to stay.**

Git Desk is a native GNOME Git client built with Rust, GTK4 and libadwaita. It is designed to make everyday Git workflows approachable without hiding how Git works.

## v0.9.0 scope

Git Desk includes:

- repository opening, cloning and Recent Projects;
- Changes with staging, unstaging, committing and diff inspection;
- History with commit graph, references and Inspector details;
- local and remote branches, upstream state, fetch, fast-forward pull and push;
- merge, cherry-pick and revert workflows with conflict recovery;
- stashes and tags;
- detached HEAD and diverged-upstream recovery paths;
- Git Guide with workflow filters, search, favorites, personal notes and contextual outline navigation;
- responsive left navigation and right Inspector panes;
- gettext localization baseline for Dutch, German, French, Spanish, Italian and Portuguese.

## Application identity

- Application name: **Git Desk**
- App ID: `io.github.christiaanbruinsma.GitDesk`
- Binary/package: `git-desk`
- Repository: `christiaanbruinsma/git-desk`

## Development

The normal development environment is GNOME Builder using the dedicated development manifest:

```text
io.github.christiaanbruinsma.GitDesk.Devel.yml
```

It builds the development identity `io.github.christiaanbruinsma.GitDesk.Devel` and keeps Flatpak locales in the `.Locale` extension for localization QA.

The production / standalone release manifest is:

```text
io.github.christiaanbruinsma.GitDesk.yml
```

It builds the production identity `io.github.christiaanbruinsma.GitDesk` and embeds gettext catalogs in the standalone bundle.

Run the project release checks from the project root:

```bash
./scripts/check.sh
```

See [RELEASE.md](RELEASE.md) for the full release workflow.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
