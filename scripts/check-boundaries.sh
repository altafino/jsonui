#!/usr/bin/env bash
# eng R7: jsonc_parser only under core/document and core/security/redact.rs.
set -euo pipefail
hits=$(grep -rlnE "jsonc_parser::|use jsonc_parser|extern crate jsonc_parser" src-tauri/src | grep -v -e "src-tauri/src/core/document/" -e "src-tauri/src/core/security/redact.rs" || true)
if [ -n "$hits" ]; then echo "jsonc_parser used outside allowed modules:"; echo "$hits"; exit 1; fi
# Core must stay Tauri-free.
if grep -rn "tauri::" src-tauri/src/core >/dev/null; then echo "core/ must not depend on tauri"; grep -rn "tauri::" src-tauri/src/core; exit 1; fi
echo "boundaries ok"
# eng R6 / A20: no Tauri command accepts a filesystem path from the webview.
bad=$(python3 - <<'PY'
import re, pathlib
hits = []
for f in pathlib.Path("src-tauri/src/commands").rglob("*.rs"):
    src = f.read_text()
    for m in re.finditer(r"#\[tauri::command\]\s*pub\s+(?:async\s+)?fn\s+(\w+)[^(]*\((.*?)\)\s*->", src, re.S):
        params = m.group(2)
        if re.search(r"\bPathBuf\b|&Path\b|\b\w*path\w*\s*:", params, re.I):
            hits.append(f"{f}:{m.group(1)}")
print("\n".join(hits))
PY
)
if [ -n "$bad" ]; then echo "commands taking a path argument (eng R6):"; echo "$bad"; exit 1; fi
echo "command signatures ok"
