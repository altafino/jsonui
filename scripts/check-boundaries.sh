#!/usr/bin/env bash
# eng R7: jsonc_parser only under core/document and core/security/redact.rs.
set -euo pipefail
hits=$(grep -rlnE "jsonc_parser::|use jsonc_parser|extern crate jsonc_parser" src-tauri/src | grep -v -e "src-tauri/src/core/document/" -e "src-tauri/src/core/security/redact.rs" || true)
if [ -n "$hits" ]; then echo "jsonc_parser used outside allowed modules:"; echo "$hits"; exit 1; fi
# Core must stay Tauri-free.
if grep -rn "tauri::" src-tauri/src/core >/dev/null; then echo "core/ must not depend on tauri"; grep -rn "tauri::" src-tauri/src/core; exit 1; fi
echo "boundaries ok"
