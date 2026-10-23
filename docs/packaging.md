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

Run the AppImage directly (`chmod +x` first), or the binary from
`src-tauri/target/release/configdeck`. No AUR package is provided.

## Not provided

No code signing, auto-update, Flatpak, Snap, RPM, Windows or macOS installers.
