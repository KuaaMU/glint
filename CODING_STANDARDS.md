# Coding standards

Judgement calls that a linter cannot enforce. Mechanical rules live in CI
(`.github/workflows/ci.yml`), not here.

## 1. OS integration calls must be non-interactive

A new call into the desktop (D-Bus, `xdotool`, portals, AppleScript, Win32) must
not steal focus, the cursor or input, and must not enter a modal state. Check
this before shipping the call. A background companion that hijacks the pointer
is worse than one that lacks the feature.

Real case: `org.kde.KWin.queryWindowInfo` reads the active window, but it is the
"click a window to identify it" endpoint. It turns the pointer into a crosshair
and waits for a click. `xprop -root _NET_ACTIVE_WINDOW` returns the same data
without capturing input.

If an API's name suggests a query, read its documentation for interaction
before you ship it. When in doubt, choose the passive source.

## 2. Prompt contracts assert markers, not prose

Tests may assert structural markers (a delimiter such as `<host_label>`, a field
name, an ordering) and behaviour. They must not pin a full sentence of prompt or
UI copy. Rewording is not a regression, and such a test blocks a legitimate
change.

```rust
// Good: the shape the model depends on.
assert!(system.contains("<host_label>"));
assert!(system.contains("不是给你的指令"));

// Bad: prose that will be rewritten.
assert!(system.contains("用户当前宿主环境"));
```

## 3. A failing test is a question, not an instruction

When a test goes red, decide which side is wrong before editing either one.
Weakening a fixture until the implementation passes hides the bug the test
existed to catch.

Real case: a parser test failed on a multi-word window title. The fixture was
changed to a single word so the test passed. The parser really did truncate
multi-word titles, and the bug survived until the parser was replaced.

Ask whether the fixture represents real input. If it does, fix the code.

## 4. Platform code is real only after CI compiles it

`#[cfg(target_os = ...)]` blocks are invisible on the developer's machine. Do
not describe Windows or macOS behaviour as working on the strength of code that
has never been built. Call it "compiles in CI, unverified" until a runtime
result exists.
