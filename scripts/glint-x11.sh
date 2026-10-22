#!/usr/bin/env bash
# Launch Glint with cursor-adjacent placement and real always-on-top.
#
# Native Wayland toplevels cannot be positioned by the client: KWin places them
# itself, so a HUD near the selection is impossible. XWayland still lets a
# client set an absolute position and `_NET_WM_STATE_ABOVE`, while hotkeys keep
# flowing through KGlobalAccel. This launcher pins the whole process to X11.
#
# Trade-off: the HUD is a normal X11 window (fractional scaling is handled by
# KWin), and the WebKitGTK transparency path differs. Set GLINT_BACKEND=wayland
# to opt back into the native compositor path.
set -euo pipefail

cd "$(dirname "$0")/.."

BIN="${GLINT_BIN:-./target/debug/glint}"

if [ "${GLINT_BACKEND:-x11}" = "wayland" ]; then
  exec "$BIN" "$@"
fi

exec env GDK_BACKEND=x11 "$BIN" "$@"
