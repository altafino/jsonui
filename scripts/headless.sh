#!/usr/bin/env bash
# Run a command on a throwaway X display so test windows never land on the desktop.
# Uses xvfb-run when installed (CI), otherwise a temporary Xvnc server.
set -eu
if command -v xvfb-run >/dev/null; then exec xvfb-run -a "$@"; fi
command -v Xvnc >/dev/null || {
  echo "Need xvfb-run or Xvnc for a headless display: sudo pacman -S xorg-server-xvfb" >&2
  exit 1
}
d=77
while [ -e "/tmp/.X11-unix/X$d" ]; do d=$((d + 1)); done
Xvnc ":$d" -SecurityTypes None -geometry 1600x1000 -rfbport $((5900 + d)) -localhost >/dev/null 2>&1 &
xpid=$!
trap 'kill "$xpid" 2>/dev/null' EXIT
for _ in $(seq 50); do [ -S "/tmp/.X11-unix/X$d" ] && break; sleep 0.1; done
DISPLAY=":$d" WAYLAND_DISPLAY= "$@"
