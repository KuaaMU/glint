# Linux Runtime Acceptance Checklist

Target: Fedora 45, KDE Plasma 6.7, Wayland. Every item is manual or measured on
the real session. A green contract suite does not satisfy any item here.

## Measured evidence (2026-10-21)

Environment: `XDG_SESSION_TYPE=wayland`, `XDG_CURRENT_DESKTOP=KDE`,
`WAYLAND_DISPLAY=wayland-0`, `xdg-desktop-portal-kde 6.7.5`.

| Item | Command | Result |
|---|---|---|
| Session detection | `glint-cli session` | `wayland`, `kde: true` |
| PRIMARY capture | `printf 'probe' \| wl-copy --primary` then `glint-cli capture` | `source: WaylandPrimary`, exact text returned |
| Clipboard non-pollution | `glint-cli capture` | no clipboard write; only `wl-paste --primary` equivalent |
| KGlobalAccel registration | `glint-cli hotkeys` | component `com.glint.Glint`, 3 actions, Qt keys `0x8000051`, `0x8000057`, `0xA000044` |
| Hotkey signal delivery | `busctl ... invokeShortcut s selection-capture` | `[hotkey] SelectionCapture` printed |
| Unregister on exit | Ctrl-C | all three actions removed; component gone |
| Offline state | `glint-cli explain` with no key | explicit `未配置 API Key` error, no canned answer |
| Cargo workspace | `cargo check --workspace` | passes |
| Core tests | `cargo test -p glint-core` | 23 passed, 1 ignored (live probe) |

### Known limitation found during P1

`com.glint.desktop` is rejected as a KGlobalAccel component id: kglobalacceld
treats any `*.desktop` component name as a desktop-file service action and
silently drops it when no matching file exists in
`$XDG_DATA_DIRS/kglobalaccel`. Glint therefore registers as
`com.glint.Glint`. The reverse-DNS application id stays `com.glint.desktop`
for the portal.

## Prerequisites

```bash
sudo dnf install -y gtk3-devel webkit2gtk4.1-devel dbus-devel \
  gtk-layer-shell-devel libappindicator-gtk3-devel librsvg2-devel \
  openssl-devel pkgconf-pkg-config wayland-devel libxkbcommon-devel \
  libxcb-devel mesa-libGL-devel mesa-libEGL-devel fontconfig-devel \
  freetype-devel
```

## A. Trigger

- [ ] KGlobalAccel registers `Glint` under System Settings → Shortcuts.
- [ ] `Alt+Q` fires while a native Wayland app (Kate, Firefox, Chromium) has focus.
- [ ] `Alt+Q` fires while an XWayland app has focus.
- [ ] Rebinding the shortcut in System Settings takes effect without restart.
- [ ] A conflicting shortcut is reported in the UI, not silently ignored.
- [ ] Portal fallback works on a non-KDE Wayland compositor (if available).

## B. Selection capture

- [ ] Select text in Kate / Firefox / a PDF reader; `Alt+Q` captures it.
- [ ] Select text in a terminal; capture either succeeds or reports a clear
  "selection unavailable" state (no stale previous text).
- [ ] Clipboard content is **byte-identical** before and after capture.
- [ ] PRIMARY is read before the HUD takes focus.
- [ ] A 50 KB selection is capped before it reaches the prompt.
- [ ] No `xdotool`/`wtype`/`ydotool` process is spawned.

## C. HUD surface

- [ ] Window is borderless, translucent (or opaque fallback), and above normal windows.
- [ ] Window appears on the monitor containing the pointer, not always the primary.
- [ ] Window does not steal focus from the reading app until the user clicks the input.
- [ ] Escape hides the HUD and returns focus to the previous app.
- [ ] Multi-monitor and fractional scaling produce correct placement and size.

## D. Streaming and follow-up

- [ ] First token appears in under 1.5 s on a warm connection.
- [ ] Markdown headings, lists, bold, inline code, and fenced code blocks render.
- [ ] An unclosed code fence during streaming does not flicker or leak raw tags.
- [ ] Code block is highlighted after the closing fence, not per token.
- [ ] Follow-up input accepts 200 Chinese characters through fcitx5 with zero
  dropped characters and the preedit at the caret.
- [ ] `Enter` submits; `Shift+Enter` inserts a newline (or the documented choice).
- [ ] Stop/cancel works during streaming and leaves no toast.
- [ ] Offline mode shows an explicit offline state, never canned model-like text.

## E. Output quality and privacy

- [ ] API key is read from KWallet/Secret Service, not plaintext config.
- [ ] The route chip states local vs cloud and the provider name.
- [ ] Secrets (`sk-`, `AKIA`, `ghp_`, JWTs) are redacted before egress.
- [ ] History persists in SQLite and search returns prior sessions.
- [ ] Export produces valid Markdown.

## F. Measurements

Record the command and output for each:

- [ ] Idle PSS after 10 minutes: `ps -o pid,rss,comm -C glint` (RSS; note PSS via `smem -P glint` if available).
- [ ] Streaming PSS with a 2k-token response.
- [ ] Cold start to first frame (instrument `Instant::now()` at process start and at first paint).
- [ ] Hotkey-to-first-frame latency (log timestamps around activation).
- [ ] p95 frame time during streaming (Slint frame callback or WebKit inspector).
- [ ] TTFT p50/p95 over 20 requests.
- [ ] Cache hit rate from provider usage fields.

## G. Regression gate

- [ ] `cargo check --workspace` passes.
- [ ] `cargo test -p glint-core` passes.
- [ ] `pnpm build` passes.
- [ ] `node tests/e2e_runner.mjs` has zero unexpected failures; every skip has a reason.
