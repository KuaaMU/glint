# Cross-Platform Architecture and Native Platform Bridges

We design Glint as a cross-platform application (Windows, macOS, Linux) with a modular platform bridge layer in Rust. We implement OS-specific accessibility and OCR interfaces (Windows: UIA + Windows.Media.Ocr; macOS: Accessibility API + Apple Vision Framework; Linux: AT-SPI + Wayland/X11 protocols + native/ONNX OCR). This ensures Glint achieves zero-overhead native integration on Windows first while preserving zero architectural rewrite for macOS and Linux distribution.
