# Glint

Desktop ambient AI companion. Select text in any app and a small HUD streams an
explanation near the selection. It can also snip a screen region.

## Before pushing

```bash
./scripts/check.sh
```

This runs the same sequence as CI (`.github/workflows/ci.yml`). `pnpm check`
covers only the frontend half.

## Layout

- `crates/glint-core/` holds the UI-agnostic engine: selection, hotkeys, screen
  capture, LLM streaming, context assembly. Test with `cargo test -p glint-core`.
- `src-tauri/` is the Tauri v2 shell. Platform code lives in
  `src-tauri/src/platform/`. Linux is the mainline; Windows and macOS are
  compiled in CI but not shipped.
- `src/` holds the vanilla TypeScript UI. It uses no framework.
- `tests/` holds the contract suite. Read `tests/README.md` first. It lists what
  this suite does not prove.

`cargo test` at the repo root runs only `glint-core` (`default-members`). The
shell's tests need `cargo test -p glint --lib`.

## Running the app

Ask before launching `./scripts/glint-x11.sh`. It maps real windows, registers
global hotkeys and can take over the pointer. For headless checks use
`cargo run -p glint-cli -- session|capture|screen|hotkeys`.

## Decisions

Read the relevant ADR in `docs/adr/` before changing an area it covers. Platform
constraints (Wayland, KGlobalAccel, window placement) are in
`0012-linux-native-companion-architecture.md`. The context and thread model is in
`0013-thread-continuity-and-context-assembly.md`.

## Review rules

`CODING_STANDARDS.md`.
