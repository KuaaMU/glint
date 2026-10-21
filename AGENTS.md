# Glint

Desktop ambient AI companion: select text in any app and a small HUD streams an
explanation near the selection; or snip a screen region.

## Before pushing

```bash
./scripts/check.sh
```

Same sequence as CI (`.github/workflows/ci.yml`). `pnpm check` covers only the
frontend half.

## Layout

- `crates/glint-core/` — UI-agnostic engine: selection, hotkeys, screen capture,
  LLM streaming, context assembly. Test with `cargo test -p glint-core`.
- `src-tauri/` — Tauri v2 shell. Platform code lives in `src-tauri/src/platform/`;
  Linux is the mainline, Windows/macOS are compiled in CI but not shipped.
- `src/` — vanilla TypeScript UI, no framework.
- `tests/` — contract suite. Read `tests/README.md` first: it lists what this
  suite does *not* prove.

`cargo test` at the repo root runs only `glint-core` (`default-members`). The
shell's tests need `cargo test -p glint --lib`.

## Running the app

Ask before launching `./scripts/glint-x11.sh`: it maps real windows, registers
global hotkeys and can take over the pointer. For headless checks use
`cargo run -p glint-cli -- session|capture|screen|hotkeys`.

## Decisions

Read the relevant ADR in `docs/adr/` before changing an area it covers.
Platform constraints (Wayland, KGlobalAccel, window placement):
`0012-linux-native-companion-architecture.md`. Context and thread continuity:
`0013-thread-continuity-and-context-assembly.md`.

## Review rules

`CODING_STANDARDS.md`.
