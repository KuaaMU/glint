# Glint

A high-performance, system-wide ambient companion that brings instant clarity to complex jargon, concepts, and AI outputs across any desktop application.

## Language

**Trigger Capsule**:
A lightweight, non-intrusive floating pill appearing near the cursor after selection to offer immediate actions.
_Avoid_: Floating tooltip, selection menu, hover button

**Companion HUD**:
The anchored, clean floating card that streams direct, focused explanations and conversational follow-ups.
_Avoid_: Modal window, popup dialog, sidebar drawer

**Context Sensor**:
The background subsystem responsible for extracting host application metadata, surrounding paragraphs, or local screen regions.
_Avoid_: Text grabber, inspector, scraper

**Context Envelope**:
The unified structured payload containing the selected token, surrounding paragraph, host process, and window title passed to the explanation engine.
_Avoid_: Prompt context, raw selection, payload data

**Zero-latency Cache**:
The local embedded store providing instant (<10ms) responses for previously encountered or high-frequency technical jargon.
_Avoid_: Local storage, query cache, memory pool

**Snip Query**:
The modality where non-selectable text or visual assets (diagrams, math equations, terminal output) are captured via an instant region crop.
_Avoid_: Screenshot tool, OCR window

**Cognitive Alignment**:
The process of dynamically calibrating explanations to the user's mental model and depth preference, progressively elevating comprehension without visual clutter.
_Avoid_: Prompt tuning, temperature adjustment

**Adaptor Protocol**:
The standardized contract implemented by host application integrations to extract structured context and clean tokens.
_Avoid_: App hook, window scraper, integration driver

**OCR Provider**:
The pluggable abstraction interface providing fast optical text recognition across operating systems and backends.
_Avoid_: OCR library, image reader

**Global Trigger Engine**:
The background OS listener monitoring system-wide hotkeys and mouse selection gestures to awaken the Companion HUD without window focus theft.
_Avoid_: Keylogger, hotkey manager, global hooker
