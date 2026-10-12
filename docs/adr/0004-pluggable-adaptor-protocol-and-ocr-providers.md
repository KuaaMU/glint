# Standardized Adaptor Protocol and Pluggable OCR Providers

We decouple application-specific context extraction and visual text recognition into standardized Rust trait interfaces (`AppAdaptor` and `OcrProvider`) with lightweight built-in defaults (Windows Terminal, VS Code, Browser, and Windows.Media.Ocr). We do this to ensure Glint remains sub-30MB and lightning-fast out of the box, while remaining easily extensible across diverse developer toolchains and cross-platform operating systems.
