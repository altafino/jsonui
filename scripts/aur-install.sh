#!/usr/bin/env bash
# Build an Arch/Manjaro package from this checkout.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=aur-common.sh
source "${here}/aur-common.sh"
root="$(repo_root_from "$here")"

usage() {
  cat <<'EOF'
Usage: scripts/aur-install.sh [--deb] [--install] [--dirty]

  Build an Arch package from this checkout.

  --deb       Package the local Tauri .deb as configdeck-bin (no compile).
              Looks in src-tauri/target/release/bundle/deb/.
  --install   Install after building (makepkg -si, uses sudo).
  --dirty     Package uncommitted files. Without this, a dirty tree is
              refused so a source package matches HEAD.

The package file is left in packaging/build/. Install it with:
  sudo pacman -U packaging/build/<name>.pkg.tar.zst
EOF
}

mode=source
do_install=0
dirty=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --deb) mode=deb ;;
    --install) do_install=1 ;;
    --dirty) dirty=1 ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown argument: $1 (try --help)" ;;
  esac
  shift
done

require_cmd git "git"
require_cmd python3 "python"
require_cmd makepkg "sudo pacman -S base-devel"
require_cmd sha256sum "coreutils"
require_cmd tar "tar"
require_cmd bsdtar "libarchive (pacman depends on it)"

ver="$(assert_versions "$root")"

make_source_tarball() {
  local dest="$1"
  local list stage bytes
  list="$(mktemp)"
  stage="$(mktemp -d)"
  if [[ "$dirty" -eq 1 ]]; then
    git -C "$root" ls-files -z --cached --others --exclude-standard >"$list"
  else
    git -C "$root" ls-files -z >"$list"
  fi
  python3 - "$list" "$stage/files" "$root" <<'PY'
import pathlib, sys
src, dst, root = sys.argv[1:]
base = pathlib.Path(root)
raw = pathlib.Path(src).read_bytes().split(b"\0")
prefixes = (
    "src-tauri/target/",
    "packaging/build/",
    "node_modules/",
    "dist/",
    "test-results/",
    "playwright-report/",
)
exact = tuple(p.rstrip("/") for p in prefixes)
kept = []
for item in raw:
    if not item:
        continue
    text = item.decode()
    if text.startswith(prefixes) or text in exact:
        continue
    if not (base / text).is_file():
        continue
    kept.append(item)
if not kept:
    sys.exit("source file list is empty")
pathlib.Path(dst).write_bytes(b"\0".join(kept) + b"\0")
PY
  mkdir -p "${stage}/jsonui-${ver}"
  tar --null -C "$root" -T "$stage/files" -cf - | tar -C "${stage}/jsonui-${ver}" -xf -
  tar -C "$stage" -czf "$dest" "jsonui-${ver}"
  rm -rf "$stage"
  rm -f "$list"
  bytes="$(stat -c%s "$dest")"
  if (( bytes > 80000000 )); then
    die "source tarball is ${bytes} bytes; build output was probably included"
  fi
  # Read the listing once. `tar | grep -q` under pipefail dies with SIGPIPE
  # as soon as grep finds a match.
  local listing need
  listing="$(tar -tzf "$dest")"
  for need in \
    "jsonui-${ver}/LICENSE" \
    "jsonui-${ver}/packaging/linux/configdeck.desktop" \
    "jsonui-${ver}/pnpm-lock.yaml" \
    "jsonui-${ver}/src-tauri/Cargo.lock" \
    "jsonui-${ver}/src-tauri/tauri.conf.json"
  do
    grep -qxF "$need" <<<"$listing" || die "tarball is missing ${need}"
  done
}

run_makepkg() {
  local workdir="$1"
  local -a args
  args=(-fs --noconfirm --nocheck)
  if [[ "$do_install" -eq 1 ]]; then
    args+=(-i)
  fi
  (
    cd "$workdir"
    # PKGDEST/SRCDEST after -- override makepkg.conf.
    CONFIGDECK_LOCAL_SHA256="$2" makepkg "${args[@]}" -- \
      PKGDEST="$workdir" SRCDEST="$workdir"
  )
  local pkg
  pkg="$(find "$workdir" -maxdepth 1 -name '*.pkg.tar.zst' -print -quit)"
  [[ -n "$pkg" ]] || die "makepkg finished without a package in ${workdir}"
  info "package: ${pkg}"
  if [[ "$do_install" -eq 0 ]]; then
    info "install: sudo pacman -U ${pkg}"
  fi
}

if [[ "$mode" == source ]]; then
  if worktree_dirty "$root" && [[ "$dirty" -eq 0 ]]; then
    die "working tree has uncommitted changes, so the package would not match HEAD.
Commit them, or re-run with --dirty to package the working tree."
  fi
  if [[ "$dirty" -eq 1 ]] && worktree_dirty "$root"; then
    info "packaging the working tree, including uncommitted files"
  fi
  require_cmd npm "sudo pacman -S nodejs"
  require_cmd rustc "sudo pacman -S rust"
  require_cmd cargo "sudo pacman -S rust"
  require_cmd pkg-config "base-devel"
  pkg-config --exists webkit2gtk-4.1 \
    || die "missing webkit2gtk-4.1. Install: sudo pacman -S --needed webkit2gtk-4.1 base-devel openssl librsvg"
  workdir="${root}/packaging/build/configdeck"
  rm -rf "$workdir"
  mkdir -p "$workdir"
  tarball="${workdir}/$(source_tarball_name "$ver")"
  info "packing source ${ver}"
  make_source_tarball "$tarball"
  cp "${root}/packaging/aur/configdeck/PKGBUILD" "${workdir}/PKGBUILD"
  sum="$(sha256sum "$tarball" | awk '{print $1}')"
  info "building configdeck ${ver} from source (Rust release build, several minutes)"
  run_makepkg "$workdir" "$sum"
elif [[ "$mode" == deb ]]; then
  shopt -s nullglob
  debs=("${root}/src-tauri/target/release/bundle/deb/"*.deb)
  shopt -u nullglob
  [[ ${#debs[@]} -gt 0 ]] || die "no .deb in src-tauri/target/release/bundle/deb. Run: pnpm tauri build"
  [[ ${#debs[@]} -eq 1 ]] || die "expected one .deb, found ${#debs[@]}"
  workdir="${root}/packaging/build/configdeck-bin"
  rm -rf "$workdir"
  mkdir -p "$workdir"
  cp "${debs[0]}" "${workdir}/$(deb_filename "$ver")"
  cp -L "${root}/packaging/linux/configdeck.desktop" "${workdir}/configdeck.desktop"
  cp -L "${root}/LICENSE" "${workdir}/LICENSE"
  cp "${root}/packaging/aur/configdeck-bin/PKGBUILD" "${workdir}/PKGBUILD"
  sum="$(sha256sum "${workdir}/$(deb_filename "$ver")" | awk '{print $1}')"
  info "packaging $(deb_filename "$ver") as configdeck-bin ${ver}"
  run_makepkg "$workdir" "$sum"
else
  die "unknown mode: ${mode}"
fi
