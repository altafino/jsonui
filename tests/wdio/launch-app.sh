#!/usr/bin/env bash
# Launched by WebKitWebDriver (via tauri-driver) in place of the app binary.
# Isolates ConfigDeck's own state from the real user's: $1 = sandbox dir, $2 = real binary.
set -eu
sb="$1"; bin="$2"; shift 2
export HOME="$sb/home" XDG_CONFIG_HOME="$sb/xdg-config" XDG_DATA_HOME="$sb/xdg-data" XDG_CACHE_HOME="$sb/xdg-cache"
export CONFIGDECK_TEST_PICKS="$sb/picks.txt"
# Force X11 (XWayland locally, Xvfb in CI): WebKitGTK under a Wayland session crashes with protocol errors.
export GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1
exec "$bin" "$@"
