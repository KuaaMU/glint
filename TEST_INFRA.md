# Test Infrastructure Specification (TEST_INFRA)

## 1. Testing Philosophy & Principles

Glint's End-to-End (E2E) testing framework adopts a **requirement-driven, opaque-box test architecture** derived strictly from `ORIGINAL_REQUEST.md` and `PROJECT.md § Feature Inventory` across both the Hardened Web Track and the Pure Native Vector Lens (Slint) Track.

### Core Principles
1. **Opaque-Box Seam Testing**: Tests verify observable external behaviors, security invariants, data structures, and interface contracts rather than private implementation details.
2. **Authoritative Derivation**: Expected outputs are derived directly from the normative specifications in `ORIGINAL_REQUEST.md` and `PROJECT.md`, never from recomputing or echoing implementation logic (zero tautological tests).
3. **Four-Tier Systematic Stratification**:
   - **Tier 1 (Feature Coverage)**: At least 5 distinct test cases per feature covering all 18 features across Milestones M1, M2, M3, and M4 (minimum 90 feature tests).
   - **Tier 2 (Boundary & Corner Cases)**: String length boundaries (0, 1, 2999, 3000, 3001, 50000 chars), Unicode surrogate pairs, complex emojis, diverse XSS injection payloads, process RAM limits (< 15MB), and hotkey latency thresholds (< 20ms).
   - **Tier 3 (Cross-Feature Combinations)**: Multi-system pipelines (e.g., selection capture + 3000-char truncation + streaming typewriter; hotkey wake-up + foreground window title injection + multi-turn follow-up).
   - **Tier 4 (Real-World Workloads)**: Multi-language code snippets with nested markdown, stress toggle loops, large conversational histories, and dual-track concurrent execution.
4. **Progressive Testability & Zero False Blockers**: The test runner validates features progressively against current milestone deliverables and defined interface contracts (`PROJECT.md § Interface Contracts`), reporting clear pass/fail/pending statuses while supporting both milestone-specific runs and unified complete validation.
5. **Zero External Runtime Test Dependencies**: The runner operates entirely on standard Node.js (v22+) built-in modules (`node:test`, `node:assert/strict`) and standard Cargo commands (`cargo test`), ensuring deterministic execution without requiring unbundled npm test frameworks.

---

## 2. Feature Inventory Matrix

| # | Feature Name | Milestone | Scope / Seam | Tier 1 Cases | Key Verification Criteria & Invariants |
|---|---|---|---|---|---|
| **F1** | Safe Markdown Parser | M1 | `src/safe-markdown.ts` | 5+ | Entity escaping (`&`, `<`, `>`, `"`, `'`), code blocks (` ``` `), inline code (`` ` ``), bold (`**`), unclosed streaming code blocks |
| **F2** | Eliminate `innerHTML` | M1 | `src/main.ts` | 5+ | Zero naked `innerHTML` writes in `src/main.ts`; DOM replacement via `replaceChildren` / `textContent` |
| **F3** | 3000-char Truncation | M1 | `src/safe-markdown.ts` & UI | 5+ | Precise truncation at 3000 Unicode chars; original length tracking; UI warning capsule trigger |
| **F4** | README Alignment | M1 | `README.md` | 5+ | Windows documented as Tier-1 production; macOS/Linux documented as experimental; `Alt + W` shortcut documented |
| **F5** | Slint Workspace & Crate | M2 | `Cargo.toml`, `crates/glint-hud` | 5+ | Valid Cargo workspace member; Slint 1.18 dependency with `renderer-femtovg`; `build.rs` compiles `ui/hud.slint` |
| **F6** | Obsidian Glass Texture | M2 | `crates/glint-hud/ui/hud.slint` | 5+ | `no-frame: true`, dark crystal glass background (`#0d1117e8`), DWM acrylic blur integration |
| **F7** | Precision Vector Lines | M2 | `crates/glint-hud/ui/hud.slint` | 5+ | 1px horizontal gradient light blade, 0.5px hairline craftsmanship, razor borders |
| **F8** | Breathing Glow Animation | M2 | `crates/glint-hud/ui/hud.slint` | 5+ | Hardware-accelerated continuous breathing glow property (`glow_pulse`), smooth transitions |
| **F9** | Zero Web Runtime | M2 | Native HUD Binary Artifact | 5+ | Zero dependency on WebView2, HTML, CSS, or Node.js runtime in `glint-hud` binary |
| **F10** | Clipboard Engine Reuse | M3 | `src-tauri/src/capture.rs` | 5+ | `GetClipboardSequenceNumber` atomic polling, modifier release synthesis, zero clipboard mutex lock contention |
| **F11** | Window Title Awareness | M3 | `src-tauri/src/platform/windows.rs` | 5+ | `GetForegroundWindow` + `GetWindowTextW` integration; injection into LLM prompt envelope |
| **F12** | Streaming LLM Channel | M3 | `src-tauri/src/llm.rs` & `core_bridge` | 5+ | Decoupled `StreamSink` channel interface, SSE token parsing, offline fallback engine |
| **F13** | Hotkey HUD Summon | M3 | `src-tauri/src/platform/windows.rs` | 5+ | Global `Alt + Q`, `Alt + D`, `Alt + W` message loop; window presentation latency < 20ms |
| **F14** | Typewriter Text Display | M3 | `crates/glint-hud/ui/hud.slint` | 5+ | Incremental token accumulation, live preview update, auto-scroll responsiveness |
| **F15** | Multi-Turn Follow-up | M3 | `crates/glint-hud/src/session.rs` | 5+ | Interactive `TextInput`, user query appending, conversational history payload structure |
| **F16** | Idle RAM < 15MB Verification | M4 | Process Monitoring / PSAPI | 5+ | Idle resident working set strictly < 15.0 MB verified via Win32 PSAPI / OS metrics |
| **F17** | XSS Injection Defense Tests | M4 | Sanitizer & Stream Renderer | 5+ | 100% defense against `<script>`, `<img onerror>`, `<svg onload>`, iframe, and DOM event injections |
| **F18** | Dual-Track Non-Interference | M4 | Workspace & Build System | 5+ | All 14 existing `cargo test` pass; `npm run build` succeeds; no build collision between Tauri and Slint |

---

## 3. Test Suite Layout & File Organization

The test infrastructure is structured under `F:\glint\tests/`:

```
F:\glint\tests/
├── e2e_runner.mjs               # Unified test orchestrator & CLI runner
├── test_config.mjs              # Shared configuration, paths, and environment constants
├── helpers/
│   ├── contract_oracle.mjs      # Authoritative specification oracles & contract checks
│   ├── test_assert.mjs          # Assertion helpers & invariant validators
│   └── mock_fixtures.mjs        # Standard fixtures (XSS payloads, sample code, long texts)
├── tier1_features.test.mjs      # Tier 1: Feature Coverage (F1 through F18, >= 5 cases each)
├── tier2_boundaries.test.mjs    # Tier 2: Boundary & Corner Cases (lengths, emojis, XSS, RAM, latency)
├── tier3_combinations.test.mjs  # Tier 3: Cross-Feature Combinations & Interaction Pipelines
└── tier4_workloads.test.mjs     # Tier 4: Real-World Workloads (complex code, multi-turn dialogues)
```

---

## 4. Runner Invocation & CLI Usage

### Running All Test Suites
```bash
node tests/e2e_runner.mjs
```
*Expected behavior*: Executes Tiers 1-4 across all 18 features, prints human-readable test summaries, and exits with code `0` when all suites pass.

### Filtering by Tier
```bash
node tests/e2e_runner.mjs --tier=1    # Run Tier 1 Feature Coverage
node tests/e2e_runner.mjs --tier=2    # Run Tier 2 Boundary & Corner Cases
node tests/e2e_runner.mjs --tier=3    # Run Tier 3 Cross-Feature Combinations
node tests/e2e_runner.mjs --tier=4    # Run Tier 4 Real-World Workloads
```

### Filtering by Milestone
```bash
node tests/e2e_runner.mjs --milestone=M1   # Run M1 Safety & Typography tests
node tests/e2e_runner.mjs --milestone=M2   # Run M2 Slint HUD Architecture tests
node tests/e2e_runner.mjs --milestone=M3   # Run M3 Core Engine & Integration tests
node tests/e2e_runner.mjs --milestone=M4   # Run M4 E2E & Performance tests
```

### Verbose Mode
```bash
node tests/e2e_runner.mjs --verbose
```

---

## 5. Pass/Fail Semantics & Exit Codes

1. **Exit Code 0 (Success)**:
   - Every executed test case across the targeted tiers passed without assertion errors or unhandled exceptions.
   - All critical security invariants (zero XSS, 3000-char truncation, zero innerHTML writes) verified.
   - All performance and resource thresholds (< 15MB RAM, < 20ms hotkey response) satisfied.
2. **Exit Code 1 (Failure)**:
   - One or more test assertions failed.
   - A critical invariant was violated (e.g. unescaped HTML tag detected, truncation exceeded, build regression).
   - An uncaught exception or fatal error was encountered.

---

## 6. Progressive Testability & Invariant Verification

To satisfy progressive testability during parallel milestone development:
1. When target source files (e.g., `src/safe-markdown.ts`, `crates/glint-hud`) are present in the filesystem, tests directly load, parse, and verify the production artifacts.
2. If a planned milestone crate is not yet materialized on disk, the test suite evaluates the authoritative interface contracts defined in `PROJECT.md § Interface Contracts` and validates baseline non-interference with existing operational crates (`src-tauri` cargo tests and Vite builds).
3. The unified runner outputs a structured report detailing:
   - Total test cases executed.
   - Pass / Fail breakdown per tier and feature.
   - Verified system invariants.
