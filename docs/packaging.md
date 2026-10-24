# Packaging

`pnpm tauri build` produces, under `src-tauri/target/release/bundle/`:

- `appimage/ConfigDeck_<version>_amd64.AppImage`: self-contained, runs on most
  distributions with a glibc at least as new as the build machine's.
- `deb/ConfigDeck_<version>_amd64.deb`: for Debian/Ubuntu (`sudo apt install ./…deb`).

The targets and the bundled resource (`fixtures/feature-flags.json`, the sample file)
are set in `src-tauri/tauri.conf.json`.

## Portability

An AppImage links against the build machine's glibc. Built on Manjaro (rolling), it
will not start on older distributions. The CI workflow (`.github/workflows/ci.yml`)
builds on ubuntu-22.04 (glibc 2.35) and uploads both bundles as artifacts; use those
for distribution.

## Manjaro / Arch

Two packages, installed with `yay`, `paru`, or `pamac build`:

| Package | What it does |
| --- | --- |
| `configdeck` | Builds the tagged source against system WebKitGTK. This is the package to publish on the AUR. |
| `configdeck-bin` | Installs the Ubuntu 22.04 `.deb` from the GitHub release. No compile. x86_64 only. |

They conflict. The `.deb` is built on glibc 2.35, so it runs on Manjaro.

From a git checkout, without the AUR:

```sh
make aur-install          # compile and install
make aur                  # package only; sudo pacman -U packaging/build/configdeck/*.pkg.tar.zst
make aur-deb              # wrap the local .deb (needs pnpm tauri build first)
```

`AUR_ARGS=--dirty` includes uncommitted files. A clean tree is required otherwise, so the package matches HEAD.

Recipes live in `packaging/aur/`. `scripts/aur-srcinfo.sh` regenerates `.SRCINFO`.

### Cut a release and publish to the AUR

The project is Apache-2.0 (`LICENSE`). `github.com/altafino/jsonui` has to be public so other people can download the tag archive and the release `.deb`.

Before the first AUR push, create empty AUR repos named `configdeck` and `configdeck-bin`, and add an SSH key at the AUR account page.

Then, with versions already matching in `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, and both PKGBUILDs, and a `## [x.y.z]` section in `CHANGELOG.md`:

```sh
scripts/release.sh --check
scripts/release.sh --push          # tag only; does not push the branch
# wait until the Release workflow has uploaded the .deb
scripts/aur-publish.sh             # writes sha256sums; review the diff
scripts/aur-publish.sh --push      # commits those recipes on the AUR
```

`scripts/release.sh --push` starts `.github/workflows/release.yml`, which builds on Ubuntu 22.04 and attaches `ConfigDeck_<version>_amd64.deb`, the AppImage, and `SHA256SUMS` to the GitHub release. `scripts/aur-publish.sh` checksums the public tag archive and that `.deb`. Commit the updated PKGBUILDs and `.SRCINFO` in this repository afterwards.

The source package installs pnpm 10.12.3 itself. Arch's pnpm 11 rejects `pnpm-lock.yaml` under its release-age policy. The build uses distro `rust` and links WebKitGTK 4.1. `options=('!lto')` keeps makepkg's `-flto` out of the C flags; the Cargo release profile already enables LTO.

## Not provided

No code signing, auto-update, Flatpak, Snap, RPM, Windows or macOS installers.
