# Rust and Tauri 2.0 Desktop Architecture

We are building Glint using a Rust core with a Tauri 2.0 (WebView2) presentation layer instead of Electron or pure native Win32/WPF. This gives us sub-30MB baseline memory consumption, sub-10ms system-level Windows API hooks, and instant window reveal without the multi-hundred-megabyte overhead of Chromium.
