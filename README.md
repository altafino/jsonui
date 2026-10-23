# ConfigDeck

A local-first desktop editor that turns JSON and JSONC configuration files
into forms, and saves them without disturbing anything you did not change.
Claude Code settings are the first built-in preset; any JSON file works.

- **Non-destructive:** edits change only the touched tokens; comments,
  ordering, line endings, BOM and unknown keys are preserved. Opening and
  saving an unchanged file is a no-op.
- **Safe saving:** a backup before every write, atomic replace, and a
  fingerprint recheck so changes made by other programs (for example Claude
  Code adding a permission rule) are never overwritten silently.
- **Offline:** no network access; schemas are bundled or chosen locally.

Built with Tauri 2 (Rust core) and React + TypeScript.

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

| Task | Command |
|---|---|
| Rust tests (core + flows; regenerates TS contract types) | `cd src-tauri && cargo test --lib --tests` |
| Rust lint / format | `cargo clippy --all-targets -- -D warnings`, `cargo fmt` |
| Architecture rules | `pnpm check:boundaries` |
| Frontend type check / unit tests | `pnpm typecheck`, `pnpm test` |
| UI flows (browser, mocked backend) | `pnpm test:e2e` |
| Real-app journeys (WebdriverIO + tauri-driver) | `pnpm build:app` once, then `pnpm test:wdio` (needs `cargo install tauri-driver --locked`, `WebKitWebDriver`, and an X display: `xvfb-run -a pnpm test:wdio`; see `tests/wdio/`) |

Layout: `src-tauri/src/core/` is the Tauri-free engine (document, patch,
persistence, backup, registry, schema, presets, security, watcher);
`src-tauri/src/commands/` is the IPC layer; `src/` is the UI. The IPC
contract is in `docs/contract.md`; decisions in `docs/decisions.md`; the full
build specification with review ledgers in `docs/spec/`.

## Limitations

See `docs/known-limitations.md`.
