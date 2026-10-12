# Global Trigger Engine via Dedicated Win32 Message Thread

We implement global desktop triggers using a dedicated Win32 worker thread running `RegisterHotKey` (`Alt + Q` / `Alt + D`) and optional low-level mouse hook (`WH_MOUSE_LL`). When triggered, it captures the active selection via guarded clipboard emulation, samples cursor coordinates, clamps the HUD window within monitor bounds, and awakens the UI. We do this to ensure zero CPU polling and instant (<10ms) activation across arbitrary Windows applications without interfering with normal window focus.
