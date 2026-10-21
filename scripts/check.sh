#!/usr/bin/env bash
# One command that runs everything CI runs, in the same order.
#
# CI (.github/workflows/ci.yml) is the authority; this script exists so the
# same failures surface locally before a push. Keep the two in step: if you add
# a job there, add its command here.
set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> cargo fmt --check"
cargo fmt --check

echo "==> cargo clippy (core)"
cargo clippy -p glint-core --all-targets -- -D warnings

# Cross-target lint for glint-core. Without this, Windows/macOS-only code is
# only linted by CI, one platform per run, after a push.
for target in x86_64-pc-windows-msvc aarch64-apple-darwin; do
  if rustup target list --installed | grep -qx "$target"; then
    echo "==> cargo clippy (core, $target)"
    cargo clippy -p glint-core --target "$target" --all-targets -- -D warnings
  fi
done

echo "==> cargo test (core)"
cargo test -p glint-core

echo "==> cargo clippy (shell)"
cargo clippy -p glint --all-targets -- -D warnings

echo "==> cargo test (shell)"
cargo test -p glint --lib

echo "==> pnpm test (frontend units)"
pnpm test

echo "==> contract suite"
node tests/e2e_runner.mjs

echo "==> production build"
pnpm build

echo "All checks passed."
