import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

export const PROJECT_ROOT = path.resolve(__dirname, "..");
export const SRC_DIR = path.join(PROJECT_ROOT, "src");
export const SRC_TAURI_DIR = path.join(PROJECT_ROOT, "src-tauri");
export const CRATES_DIR = path.join(PROJECT_ROOT, "crates");
export const GLINT_CORE_DIR = path.join(CRATES_DIR, "glint-core");
export const GLINT_HUD_DIR = path.join(CRATES_DIR, "glint-hud");
export const README_PATH = path.join(PROJECT_ROOT, "README.md");
export const PROJECT_MD_PATH = path.join(PROJECT_ROOT, "PROJECT.md");
export const ORIGINAL_REQUEST_PATH = path.join(PROJECT_ROOT, ".agents", "teamwork", "ORIGINAL_REQUEST.md");

export const CONSTANTS = {
  MAX_SELECTION_CHARS: 3000,
  MAX_IDLE_RAM_MB: 15.0,
  MAX_HOTKEY_LATENCY_MS: 20.0,
  SLINT_VERSION_EXPECTED: "1.18",
  SLINT_RENDERER_EXPECTED: "renderer-femtovg",
  HOTKEYS: {
    SELECTION: "Alt + Q",
    TOGGLE: "Alt + D",
    SNIPPING: "Alt + W",
  },
};
