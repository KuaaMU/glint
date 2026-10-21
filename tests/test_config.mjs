import path from "node:path";
import { execFileSync } from "node:child_process";
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

/**
 * Whether the Tauri shell can be compiled here.
 *
 * The shell needs GTK/WebKit headers on Linux. CI's frontend job runs without
 * them, so contract tests that shell out to `cargo test` in src-tauri would
 * fail for a reason unrelated to the code under test. Those tests skip with a
 * reason instead of reporting a false failure (or a false pass).
 *
 * Override with GLINT_SHELL_BUILDABLE=1 or =0.
 */
function detectShellBuildable() {
  if (process.env.GLINT_SHELL_BUILDABLE === "1") return true;
  if (process.env.GLINT_SHELL_BUILDABLE === "0") return false;
  if (process.platform !== "linux") return true;
  try {
    execFileSync("pkg-config", ["--exists", "webkit2gtk-4.1"], { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}

export const SHELL_BUILDABLE = detectShellBuildable();

export const SKIP_SHELL = SHELL_BUILDABLE
  ? undefined
  : "src-tauri 需要 GTK/WebKit 开发头文件，当前环境无法编译（CI 前端 job）";

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
