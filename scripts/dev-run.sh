#!/usr/bin/env bash
# Run the Tauri shell against an already-built frontend.
#
# `tauri dev` starts Vite on port 1420; this script instead points the shell at
# the built `dist/` so the app can be exercised without a dev server.
set -euo pipefail

cd "$(dirname "$0")/.."

if [ ! -d dist ]; then
  pnpm build
fi

exec ./target/debug/glint "$@"
