#!/usr/bin/env bash
# Tag a ConfigDeck release. --push publishes the tag; GitHub Actions uploads
# the .deb and AppImage. This script does not push the current branch.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=aur-common.sh
source "${here}/aur-common.sh"
root="$(repo_root_from "$here")"

usage() {
  cat <<'EOF'
Usage: scripts/release.sh [--check|--tag|--push]

  --check   Verify that versions match, CHANGELOG has this version, and the
            tree is clean (default).
  --tag     Also create annotated tag v<version> on HEAD.
  --push    Create that tag if needed and push it to origin. The Release
            workflow then publishes ConfigDeck_<version>_amd64.deb and the
            AppImage. The branch itself is not pushed.

After the workflow is green:
  scripts/aur-publish.sh --push
EOF
}

mode=check
while [[ $# -gt 0 ]]; do
  case "$1" in
    --check) mode=check ;;
    --tag) mode=tag ;;
    --push) mode=push ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown argument: $1 (try --help)" ;;
  esac
  shift
done

require_cmd git "git"
require_cmd python3 "python"

ver="$(assert_versions "$root")"
require_changelog_section "$root" "$ver"
require_clean_tree "$root"

head_commit="$(git -C "$root" rev-parse --short HEAD)"
info "version ${ver} at ${head_commit} ($(git -C "$root" log -1 --format=%s))"

if [[ "$mode" == check ]]; then
  info "checks passed. Tag with: scripts/release.sh --tag"
  exit 0
fi

tag="v${ver}"
if git -C "$root" rev-parse -q --verify "refs/tags/${tag}" >/dev/null; then
  tagged="$(git -C "$root" rev-parse "${tag}^{}")"
  current="$(git -C "$root" rev-parse HEAD)"
  if [[ "$tagged" != "$current" ]]; then
    die "tag ${tag} exists and points at a different commit"
  fi
  info "tag ${tag} already points at HEAD"
else
  git -C "$root" tag -a "$tag" -m "ConfigDeck ${ver}"
  info "created tag ${tag}"
fi

if [[ "$mode" == tag ]]; then
  info "tag is local. Publish it with: scripts/release.sh --push"
  exit 0
fi

remote_tag="$(git -C "$root" ls-remote --tags origin "refs/tags/${tag}")"
if [[ -n "$remote_tag" ]]; then
  remote="$(awk '{print $1}' <<<"$remote_tag")"
  # Annotated tags show up as the tag object; peel via ls-remote ^{}.
  peeled="$(git -C "$root" ls-remote --tags origin "refs/tags/${tag}^{}" | awk '{print $1}')"
  current="$(git -C "$root" rev-parse HEAD)"
  if [[ -n "$peeled" && "$peeled" != "$current" ]]; then
    die "origin already has ${tag} pointing at ${peeled}, not HEAD"
  fi
  if [[ -z "$peeled" && "$remote" != "$current" ]]; then
    die "origin already has ${tag} pointing at ${remote}, not HEAD"
  fi
  info "origin already has ${tag} at HEAD"
else
  git -C "$root" push origin "$tag"
  info "pushed ${tag}"
fi

info "The Release workflow uploads the .deb and AppImage."
info "It does not push branch $(git -C "$root" branch --show-current)."
info "When that workflow is green, publish the AUR repos with: scripts/aur-publish.sh --push"
