# Glint Test Layers

Glint has two distinct test layers. Do not use one to claim the other.

## 1. Contract / unit suite (`node tests/e2e_runner.mjs`)

- Runs in Node, with no desktop session and no Rust GUI toolchain.
- Checks source contracts (safe Markdown, XSS escaping, truncation, widget
  schemas, history store, file/config layout) and a few `cargo test` invocations.
- **It cannot prove** that a global hotkey fires, that PRIMARY selection is
  read, that a layer-shell window is positioned correctly, that IME works, or
  that RSS/latency targets are met.
- Tests whose target does not exist yet are marked `skip` with an explicit
  reason. A skipped test is not a pass.

## 2. Runtime acceptance (`tests/RUNTIME_ACCEPTANCE.md`)

- Runs on the real Fedora 45 + KDE Plasma Wayland machine.
- Measures the actual integration: KGlobalAccel/portal hotkeys, PRIMARY
  capture, layer-shell placement, streaming first token, IME, clipboard
  non-pollution, memory, and first-frame latency.
- This is the only layer that can support a "Linux v0.2 works" claim.

## Commands

```bash
# Everything CI runs, in the same order (use this before pushing)
./scripts/check.sh

# Contract suite
node tests/e2e_runner.mjs

# Frontend unit/adversarial tests
pnpm test

# Core crate tests (no GTK/WebKit required)
cargo test -p glint-core

# Shell tests (the root `cargo test` skips these: default-members is glint-core)
cargo test -p glint --lib

# Full workspace type-check (requires GTK/WebKit dev headers)
cargo check --workspace
```
