#!/usr/bin/env bash
# Regenerate packaging/aur/*/.SRCINFO from the PKGBUILDs.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=aur-common.sh
source "${here}/aur-common.sh"
root="$(repo_root_from "$here")"

require_cmd makepkg "pacman / base-devel"
require_cmd python3 "python"
assert_versions "$root" >/dev/null

write_srcinfo "${root}/packaging/aur/configdeck"
write_srcinfo "${root}/packaging/aur/configdeck-bin"
info "updated packaging/aur/configdeck/.SRCINFO"
info "updated packaging/aur/configdeck-bin/.SRCINFO"
