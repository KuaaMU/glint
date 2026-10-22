# Test Status — Contract Baseline

**Date:** 2026-10-19
**Replaces:** the previous "115/115 passed" report, which was not reproducible.

## 1. What this suite actually is

`node tests/e2e_runner.mjs` is a **contract/unit suite**. It runs in Node,
without a desktop session, and checks source files, sanitisation behaviour,
and a few `cargo test` invocations. It does **not** exercise Wayland,
global hotkeys, PRIMARY selection, layer-shell, IME, RSS, or latency.

Runtime acceptance is a separate, manual layer: see
[`tests/RUNTIME_ACCEPTANCE.md`](tests/RUNTIME_ACCEPTANCE.md).

## 2. Current result

Run on Fedora 45 / KDE Plasma 6.7 / Wayland, after installing the GTK/WebKit
development headers:

| Suite | Total | Pass | Fail | Skip |
|---|---:|---:|---:|---:|
| Tier 1: Feature Coverage | 91 | 53 | 0 | 38 |
| Tier 2: Boundary & Corner Cases | 15 | 15 | 0 | 0 |
| Tier 3: Cross-Feature Combinations | 5 | 5 | 0 | 0 |
| Tier 4: Real-World Workloads | 4 | 4 | 0 | 0 |
| **TOTAL** | **115** | **77** | **0** | **38** |

The 38 skips are listed in section 3. Zero failures remain, but the suite is
still a **contract** suite — see section 5 for what it does not cover.

## 3. Why 38 tests are skipped

- **25 M2 Slint tests** (`F5`–`F9`): `crates/glint-hud` does not exist.
  Previously these "passed" by asserting against text in `PROJECT.md`. They
  are now marked `skip` with the reason "crates/glint-hud 未实现".
- **13 M3/M4 placeholder tests**: latency and RAM assertions that only
  compared constants, tautological tests with no runtime subject, and
  `PROJECT.md` spec checks. They are marked `skip` rather than allowed to
  report a false pass.

A skipped test is not a pass.

## 4. What is genuinely verified

- Safe Markdown escaping and 54 adversarial XSS/formatting cases.
- Unicode-safe 3000-character truncation boundaries (0/1/2999/3000/3001/50k).
- Generative UI widget schemas and malformed-JSON resilience.
- History store persistence/search/export.
- `glint-core` unit tests: **15 passed** (`cargo test -p glint-core`).
- Tauri shell unit tests: **17 passed** (`cargo test -p glint`).
- Frontend production build: `pnpm build` passes (main JS 41.5 KB, 113 ms).
- Full workspace type-check: `cargo check --workspace` passes.

## 5. What is not verified yet

- KGlobalAccel / portal global hotkey delivery on Wayland.
- PRIMARY selection capture and clipboard non-pollution.
- layer-shell placement, always-on-top, focus behaviour.
- fcitx5 Chinese IME in the follow-up input.
- Streaming first-token latency, p95 frame time, idle/streaming RSS.
- Linux screen capture: the current non-Windows `capture_screen()` returns a
  generated placeholder image, not a real screenshot.
- Windows-only behaviour (code retained, not testable on this machine).

## 6. How to reproduce

```bash
node tests/e2e_runner.mjs
pnpm test
cargo test -p glint-core
cargo check --workspace   # requires the Fedora dev headers above
```
