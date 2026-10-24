# Shared helpers for the Arch/AUR scripts. Source this file; do not execute it.
# shellcheck shell=bash

# GitHub repo that owns tags and release assets. PKGBUILD URLs must match.
GITHUB_REPO=altafino/jsonui

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

info() {
  printf '%s\n' "$*"
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "required command not found: $1 ($2)"
}

repo_root_from() {
  local scripts_dir="$1"
  cd "${scripts_dir}/.." && pwd
}

app_version() {
  python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' \
    "$1/src-tauri/tauri.conf.json"
}

package_json_version() {
  python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' \
    "$1/package.json"
}

cargo_version() {
  awk -F'"' '/^version = /{print $2; exit}' "$1/src-tauri/Cargo.toml"
}

pkgbuild_field() {
  local field="$1" file="$2"
  awk -F= -v field="$field" '$1 == field {print $2; exit}' "$file"
}

# Print the version when package.json, Cargo.toml, tauri.conf.json and both
# PKGBUILDs agree. Exit otherwise.
assert_versions() {
  local root="$1"
  local tauri cargo pkg aur bin
  tauri="$(app_version "$root")"
  cargo="$(cargo_version "$root")"
  pkg="$(package_json_version "$root")"
  aur="$(pkgbuild_field pkgver "$root/packaging/aur/configdeck/PKGBUILD")"
  bin="$(pkgbuild_field pkgver "$root/packaging/aur/configdeck-bin/PKGBUILD")"
  if [[ "$tauri" != "$cargo" || "$tauri" != "$pkg" || "$tauri" != "$aur" || "$tauri" != "$bin" ]]; then
    die "version mismatch:
  src-tauri/tauri.conf.json  ${tauri}
  src-tauri/Cargo.toml       ${cargo}
  package.json               ${pkg}
  packaging/aur/configdeck/PKGBUILD          ${aur}
  packaging/aur/configdeck-bin/PKGBUILD      ${bin}"
  fi
  [[ "$tauri" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "version ${tauri} is not x.y.z"
  printf '%s\n' "$tauri"
}

deb_filename() {
  printf 'ConfigDeck_%s_amd64.deb' "$1"
}

source_tarball_name() {
  printf 'configdeck-%s.tar.gz' "$1"
}

worktree_dirty() {
  [[ -n "$(git -C "$1" status --porcelain)" ]]
}

require_changelog_section() {
  local root="$1" ver="$2"
  if ! grep -q "^## \\[${ver}\\]" "$root/CHANGELOG.md"; then
    die "CHANGELOG.md has no '## [${ver}]' section. Rename '## [Unreleased]' when you cut the release."
  fi
}

require_clean_tree() {
  local root="$1"
  if worktree_dirty "$root"; then
    die "working tree is not clean. Commit or stash before tagging a release."
  fi
}

# Replace the published checksum line. Leaves the local-build branch alone.
update_checksum_line() {
  local file="$1" variable="$2" sum="$3"
  [[ "$sum" =~ ^[0-9a-f]{64}$ ]] || die "not a sha256: ${sum}"
  python3 - "$file" "$variable" "$sum" <<'PY'
import pathlib, re, sys
path, variable, sum_ = sys.argv[1:]
file = pathlib.Path(path)
text = file.read_text()
pattern = rf"^(\s*){re.escape(variable)}=\('([0-9a-fA-F]{{64}}|SKIP)'\)$"
repl = rf"\1{variable}=('{sum_}')"
updated, count = re.subn(pattern, repl, text, count=1, flags=re.M)
if count != 1:
    sys.exit(f"could not update {variable} in {path}")
file.write_text(updated)
PY
}

write_srcinfo() {
  local dir="$1"
  # A leaked local checksum would record the wrong source in .SRCINFO.
  (
    cd "$dir"
    env -u CONFIGDECK_LOCAL_SHA256 makepkg --printsrcinfo > .SRCINFO
  )
}
