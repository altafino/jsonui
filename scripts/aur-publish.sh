#!/usr/bin/env bash
# Fill AUR checksums from a published tag and, with --push, update the AUR git repos.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=aur-common.sh
source "${here}/aur-common.sh"
root="$(repo_root_from "$here")"

usage() {
  cat <<'EOF'
Usage: scripts/aur-publish.sh [--push] [--package configdeck|configdeck-bin|all]

  Download the Git tag archive (and, for configdeck-bin, the GitHub release
  .deb), write sha256sums into the PKGBUILDs, and regenerate .SRCINFO.

  --push    Commit and push the AUR repositories (ssh://aur@aur.archlinux.org).
            Create each empty repo on the AUR first, and add your SSH key:
              https://aur.archlinux.org/submit
              https://aur.archlinux.org/account

  Refuses --push while the GitHub repository is private or license() is still
  LicenseRef-unlicensed. Other users cannot build a private repository, and
  the AUR requires a license that allows redistribution.

  Does not commit the checksum update in this repository. Review the PKGBUILD
  diff and commit it yourself.
EOF
}

do_push=0
which_pkg=all

while [[ $# -gt 0 ]]; do
  case "$1" in
    --push) do_push=1; shift ;;
    --package)
      [[ $# -ge 2 ]] || die "--package needs configdeck, configdeck-bin, or all"
      which_pkg="$2"
      shift 2
      ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown argument: $1 (try --help)" ;;
  esac
done

case "$which_pkg" in
  all|configdeck|configdeck-bin) ;;
  *) die "--package must be configdeck, configdeck-bin, or all" ;;
esac

require_cmd curl "curl"
require_cmd gh "GitHub CLI"
require_cmd git "git"
require_cmd makepkg "base-devel"
require_cmd python3 "python"
require_cmd sha256sum "coreutils"

gh auth status >/dev/null 2>&1 || die "gh is not logged in. Run: gh auth login"

ver="$(assert_versions "$root")"
aur_pkgbuild="${root}/packaging/aur/configdeck/PKGBUILD"
bin_pkgbuild="${root}/packaging/aur/configdeck-bin/PKGBUILD"

visibility="$(gh repo view "$GITHUB_REPO" --json visibility --jq .visibility)"
if [[ "$visibility" != "PUBLIC" ]]; then
  die "GitHub repo ${GITHUB_REPO} is ${visibility}. Make it public before publishing to the AUR; other users cannot download a private tag archive."
fi

license_unset=0
if grep -q "LicenseRef-unlicensed" "$aur_pkgbuild" "$bin_pkgbuild"; then
  license_unset=1
fi
if [[ ! -f "${root}/LICENSE" && ! -f "${root}/LICENSE.md" ]]; then
  license_unset=1
fi
if [[ "$license_unset" -eq 1 ]]; then
  if [[ "$do_push" -eq 1 ]]; then
    die "add a LICENSE file and set license() in both PKGBUILDs to its SPDX id before --push.
The AUR redistributes the build recipe; the upstream project needs a license that allows it."
  fi
  info "warning: no SPDX license yet. Checksums can be updated; --push will refuse."
fi

if [[ -z "$(git -C "$root" ls-remote --tags origin "refs/tags/v${ver}")" ]]; then
  die "tag v${ver} is not on origin. Run: scripts/release.sh --push"
fi

grep -q "github.com/${GITHUB_REPO}" "$aur_pkgbuild" \
  || die "source PKGBUILD url does not match ${GITHUB_REPO}"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

want_source=0
want_bin=0
[[ "$which_pkg" == all || "$which_pkg" == configdeck ]] && want_source=1
[[ "$which_pkg" == all || "$which_pkg" == configdeck-bin ]] && want_bin=1

if [[ "$want_source" -eq 1 ]]; then
  archive_url="https://github.com/${GITHUB_REPO}/archive/refs/tags/v${ver}.tar.gz"
  info "downloading ${archive_url}"
  curl -fL --retry 3 -o "${tmp}/src.tar.gz" "$archive_url"
  # The checksum must be of the URL makepkg fetches, not of `git archive`.
  if ! grep -qxF "jsonui-${ver}/src-tauri/Cargo.toml" < <(tar -tzf "${tmp}/src.tar.gz"); then
    die "tag archive does not contain jsonui-${ver}/ (repo name and tag prefix are part of the PKGBUILD)"
  fi
  sum="$(sha256sum "${tmp}/src.tar.gz" | awk '{print $1}')"
  update_checksum_line "$aur_pkgbuild" sha256sums "$sum"
  write_srcinfo "${root}/packaging/aur/configdeck"
  info "configdeck ${ver} sha256 ${sum}"
fi

if [[ "$want_bin" -eq 1 ]]; then
  asset="$(deb_filename "$ver")"
  if ! gh release view "v${ver}" --repo "$GITHUB_REPO" >/dev/null 2>&1; then
    die "GitHub release v${ver} does not exist yet. Wait for the Release workflow, then re-run."
  fi
  gh release download "v${ver}" --repo "$GITHUB_REPO" --pattern "$asset" --dir "$tmp" --clobber
  [[ -f "${tmp}/${asset}" ]] || die "release v${ver} has no ${asset}. The Release workflow uploads it."
  sum="$(sha256sum "${tmp}/${asset}" | awk '{print $1}')"
  update_checksum_line "$bin_pkgbuild" sha256sums_x86_64 "$sum"
  write_srcinfo "${root}/packaging/aur/configdeck-bin"
  info "configdeck-bin ${ver} sha256 ${sum}"
fi

info "PKGBUILD checksums updated in this checkout. Commit them when you are ready."

if [[ "$do_push" -eq 0 ]]; then
  info "AUR repos were not pushed. Re-run with --push after the license and SSH key are in place."
  exit 0
fi

cache="${XDG_CACHE_HOME:-$HOME/.cache}/configdeck/aur"
mkdir -p "$cache"

push_aur_repo() {
  local name="$1"
  shift
  local remote="ssh://aur@aur.archlinux.org/${name}.git"
  local dest="${cache}/${name}"
  local rel
  rel="$(pkgbuild_field pkgrel "${root}/packaging/aur/${name}/PKGBUILD")"
  if [[ ! -d "${dest}/.git" ]]; then
    git clone "$remote" "$dest" \
      || die "could not clone ${remote}. Create the empty AUR package '${name}' at https://aur.archlinux.org/submit and add your SSH key."
  else
    git -C "$dest" pull --ff-only
  fi
  local f
  for f in "$@"; do
    cp -L "$f" "${dest}/$(basename "$f")"
  done
  git -C "$dest" add PKGBUILD .SRCINFO
  if [[ "$name" == configdeck-bin ]]; then
    git -C "$dest" add configdeck.desktop LICENSE
  fi
  if git -C "$dest" diff --cached --quiet; then
    info "${name}: AUR repo already matches"
    return 0
  fi
  git -C "$dest" commit -m "$(cat <<EOF
upgpkg: ${name} ${ver}-${rel}

EOF
)"
  git -C "$dest" push
  info "pushed ${name} ${ver}-${rel}"
}

if [[ "$want_source" -eq 1 ]]; then
  push_aur_repo configdeck \
    "${root}/packaging/aur/configdeck/PKGBUILD" \
    "${root}/packaging/aur/configdeck/.SRCINFO"
fi
if [[ "$want_bin" -eq 1 ]]; then
  push_aur_repo configdeck-bin \
    "${root}/packaging/aur/configdeck-bin/PKGBUILD" \
    "${root}/packaging/aur/configdeck-bin/.SRCINFO" \
    "${root}/packaging/linux/configdeck.desktop" \
    "${root}/LICENSE"
fi
