# ConfigDeck

A local-first desktop editor that turns JSON, JSONC, YAML and TOML
configuration files into forms, and saves them without disturbing anything you
did not change. Claude Code settings are the first built-in preset; any file in
those formats works, and a JSON Schema can describe any of them.

- **Non-destructive:** edits change only the touched tokens; comments,
  ordering, line endings, BOM and unknown keys are preserved. Opening and
  saving an unchanged file is a no-op.
- **Safe saving:** a backup before every write, atomic replace, and a
  fingerprint recheck so changes made by other programs (for example Claude
  Code adding a permission rule) are never overwritten silently.
- **Offline:** no network access; schemas are bundled or chosen locally.

Built with Tauri 2 (Rust core) and React + TypeScript.

## Install on Manjaro / Arch

From this checkout:

```sh
make aur-install
```

That compiles a native package and installs it (`sudo`). `make aur` only builds the package; `sudo pacman -U packaging/build/configdeck/*.pkg.tar.zst` installs it. `make aur-deb` packages an already built `.deb` without compiling.

After a release is on the AUR:

```sh
yay -S configdeck       # compile the tagged source
yay -S configdeck-bin   # install the release build
```

`pamac build configdeck` and `paru -S configdeck` are the same packages. Publishing steps are in `docs/packaging.md`.

## Run on Manjaro / Arch

Prerequisites (from the Tauri v2 Linux guide):

```sh
sudo pacman -S --needed webkit2gtk-4.1 base-devel curl wget file openssl \
  appmenu-gtk-module libappindicator-gtk3 librsvg xdotool
```

Plus Rust stable (`rustup`), Node.js 20+ and pnpm 10.

```sh
pnpm install
pnpm tauri dev        # run the app
pnpm tauri build      # AppImage + .deb in src-tauri/target/release/bundle/
```

A build made on Manjaro needs a glibc at least as new as Manjaro's. For a
portable AppImage use the CI artifact built on Ubuntu 22.04
(`.github/workflows/ci.yml`). See `docs/packaging.md`.

## Develop

Run `make` to list every target. The usual loop:

```sh
make doctor   # check toolchain and system libraries
make setup    # JS deps, Playwright Chromium, tauri-driver
make dev      # app with hot reload (make ui: browser + mock backend)
make test     # Rust + UI unit tests
make check    # everything CI runs: lint, unit, Playwright, real-app journeys
make bundle   # AppImage + .deb
```

`make wdio` drives the real app on a throwaway X display (`xvfb-run`, or `Xvnc`
when that is missing), so no windows open on your desktop. Plain `pnpm`/`cargo`
commands still work; the Makefile only wraps them.

Formats plug in behind `core/document/syntax.rs`: a format implements six
primitive edits on its own lossless tree (`yaml/` on yaml-edit, `toml/` on
toml_edit), and a shared op layer gives every format the same semantics.

Layout: `src-tauri/src/core/` is the Tauri-free engine (document, patch,
persistence, backup, registry, schema, presets, security, watcher);
`src-tauri/src/commands/` is the IPC layer; `src/` is the UI. The IPC
contract is in `docs/contract.md`; decisions in `docs/decisions.md`; the full
build specification with review ledgers in `docs/spec/`.

## Limitations

See `docs/known-limitations.md`.

## License

Copyright 2026 Stefan. Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).
