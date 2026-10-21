#!/usr/bin/env bash
# Capture the desktop for UI inspection, optionally cropped.
#
# Why this exists: a full 2987x3280 screenshot read into an agent's context is
# expensive, and the image is not in the same coordinate space as the window
# geometry the app reports (Spectacle may scale it). This script crops by
# *physical desktop coordinates* — the same space `Tauri` and `xrandr` use —
# and figures out the scale itself.
#
# Usage:
#   scripts/shot.sh out.png                 # whole desktop
#   scripts/shot.sh out.png X Y W H         # crop, in physical desktop pixels
#
# X/Y/W/H are exactly the values in `[Glint] snip overlay=(x,y) WxH` logs.
set -euo pipefail

if [ $# -lt 1 ]; then
  echo "usage: $0 <out.png> [x y width height]" >&2
  exit 2
fi

out=$1
shift

tmp=$(mktemp --suffix=.png)
trap 'rm -f "$tmp"' EXIT

# -b background, -n no notification, -o output path.
spectacle -b -n -o "$tmp" >/dev/null 2>&1

if [ ! -s "$tmp" ]; then
  echo "spectacle produced no image (is a desktop session running?)" >&2
  exit 1
fi

if [ $# -ne 4 ]; then
  mv "$tmp" "$out"
  trap - EXIT
  echo "$out"
  exit 0
fi

python3 - "$tmp" "$out" "$@" <<'PY'
import subprocess, sys
from PIL import Image

src, dst, x, y, w, h = sys.argv[1], sys.argv[2], *map(int, sys.argv[3:7])

# X root size is the authoritative virtual-desktop size; the captured image may
# be scaled relative to it (fractional scaling, compositor quirks).
out = subprocess.run(["xdpyinfo"], capture_output=True, text=True, check=True).stdout
line = next(l for l in out.splitlines() if "dimensions:" in l)
root_w = int(line.split()[1].split("x")[0])

img = Image.open(src)
scale = img.width / root_w

box = (
    round(x * scale),
    round(y * scale),
    round((x + w) * scale),
    round((y + h) * scale),
)
img.crop(box).save(dst)
print(f"{dst} ({img.crop(box).size[0]}x{img.crop(box).size[1]}, scale {scale:.3f})")
PY
