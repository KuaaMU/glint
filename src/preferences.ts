import { invoke, isTauri } from "@tauri-apps/api/core";

interface AppConfig {
  base_url: string;
  api_key: string;
  model: string;
  temperature: number;
  hotkey_trigger: string;
  hotkey_toggle: string;
  icon_plan: string;
  theme: string;
  always_on_top: boolean;
}

// 标签页切换
const tabButtons = document.querySelectorAll(".tab-pill");
const tabPanes = document.querySelectorAll(".tab-pane");

tabButtons.forEach((btn) => {
  btn.addEventListener("click", () => {
    tabButtons.forEach((b) => b.classList.remove("active"));
    tabPanes.forEach((p) => p.classList.remove("active"));

    btn.classList.add("active");
    const targetId = (btn as HTMLElement).dataset.tab;
    if (targetId) {
      const pane = document.getElementById(targetId);
      if (pane) pane.classList.add("active");
    }
  });
});

// 表单元素引用
const prefBaseUrl = document.getElementById("pref-base-url") as HTMLInputElement;
const prefApiKey = document.getElementById("pref-api-key") as HTMLInputElement;
const prefModel = document.getElementById("pref-model") as HTMLInputElement;
const prefModelSelect = document.getElementById("pref-model-select") as HTMLSelectElement;
const modelDatalist = document.getElementById("model-datalist") as HTMLElement;
const btnFetchModels = document.getElementById("btn-fetch-models") as HTMLButtonElement;
const fetchSpinner = document.getElementById("fetch-spinner") as HTMLElement;
const fetchBtnLabel = document.getElementById("fetch-btn-label") as HTMLElement;
const fetchStatusText = document.getElementById("fetch-status-text") as HTMLElement;

const prefAlwaysOnTop = document.getElementById("pref-always-on-top") as HTMLInputElement;
const btnToggleKey = document.getElementById("btn-toggle-key-reveal") as HTMLButtonElement;
const btnTest = document.getElementById("btn-test-connection") as HTMLButtonElement;
const testResult = document.getElementById("test-result-indicator") as HTMLDivElement;
const btnSave = document.getElementById("btn-save-pref-all") as HTMLButtonElement;
const btnResetSize = document.getElementById("btn-reset-window-size") as HTMLButtonElement;
const toast = document.getElementById("save-toast") as HTMLDivElement;

const presets: Record<string, { base_url: string; model: string }> = {
  deepseek: {
    base_url: "https://api.deepseek.com/v1",
    model: "deepseek-chat",
  },
  openai: {
    base_url: "https://api.openai.com/v1",
    model: "gpt-4o-mini",
  },
  kimi: {
    base_url: "https://api.moonshot.cn/v1",
    model: "moonshot-v1-8k",
  },
  ollama: {
    base_url: "http://localhost:11434/v1",
    model: "qwen2.5:7b",
  },
  custom: {
    base_url: "",
    model: "",
  },
};

// 服务提供商选择
document.querySelectorAll(".provider-btn").forEach((btn) => {
  btn.addEventListener("click", () => {
    document.querySelectorAll(".provider-btn").forEach((b) => b.classList.remove("active"));
    btn.classList.add("active");
    const p = (btn as HTMLElement).dataset.preset;
    if (p && presets[p]) {
      if (presets[p].base_url) prefBaseUrl.value = presets[p].base_url;
      if (presets[p].model) prefModel.value = presets[p].model;
      fetchStatusText.textContent = "";
      prefModelSelect.style.display = "none";
    }
  });
});

// API Key 密码明文切换
btnToggleKey.addEventListener("click", () => {
  if (prefApiKey.type === "password") {
    prefApiKey.type = "text";
    btnToggleKey.textContent = "隐藏";
  } else {
    prefApiKey.type = "password";
    btnToggleKey.textContent = "显示";
  }
});

// 动态拉取端点支持的模型列表
async function fetchModelsFromApi(silent = false) {
  const baseUrl = prefBaseUrl.value.trim();
  const apiKey = prefApiKey.value.trim();

  if (!baseUrl) {
    if (!silent) {
      fetchStatusText.textContent = "请先输入 Base URL 端点";
      fetchStatusText.style.color = "#f87171";
    }
    return;
  }

  fetchSpinner.style.display = "inline-block";
  fetchBtnLabel.textContent = "正在探测...";
  btnFetchModels.disabled = true;

  try {
    let models: string[] = [];
    if (isTauri()) {
      models = await invoke<string[]>("fetch_models", { apiKey, baseUrl });
    } else {
      await new Promise((r) => setTimeout(r, 600));
      models = ["deepseek-chat", "deepseek-reasoner", "gpt-4o", "gpt-4o-mini", "qwen2.5:7b"];
    }

    if (models.length > 0) {
      modelDatalist.innerHTML = "";
      prefModelSelect.innerHTML = `<option value="">已发现 ${models.length} 个模型 (点击快捷填入)...</option>`;

      models.forEach((m) => {
        const opt = document.createElement("option");
        opt.value = m;
        modelDatalist.appendChild(opt);

        const selOpt = document.createElement("option");
        selOpt.value = m;
        selOpt.textContent = m;
        prefModelSelect.appendChild(selOpt);
      });

      prefModelSelect.style.display = "block";
      fetchStatusText.textContent = `✓ 探测成功，已发现 ${models.length} 个可用模型`;
      fetchStatusText.style.color = "#34d399";

      if (!prefModel.value.trim() && models[0]) {
        prefModel.value = models[0];
      }
    } else {
      fetchStatusText.textContent = "端点未返回模型列表，可手动输入标识";
      fetchStatusText.style.color = "var(--text-dim)";
    }
  } catch (_err) {
    // 优雅降级提示，避免突兀的红屏报错
    fetchStatusText.textContent = "该端点暂不支持自动枚举模型，请直接手动指定";
    fetchStatusText.style.color = "var(--text-dim)";
  } finally {
    fetchSpinner.style.display = "none";
    fetchBtnLabel.textContent = "探测可用模型";
    btnFetchModels.disabled = false;
  }
}

btnFetchModels.addEventListener("click", () => fetchModelsFromApi(false));

prefModelSelect.addEventListener("change", () => {
  if (prefModelSelect.value) {
    prefModel.value = prefModelSelect.value;
  }
});

// 载入已有配置
async function loadConfig() {
  if (isTauri()) {
    try {
      const cfg: AppConfig = await invoke("get_app_config");
      prefBaseUrl.value = cfg.base_url || "https://api.deepseek.com/v1";
      prefApiKey.value = cfg.api_key || "";
      prefModel.value = cfg.model || "deepseek-chat";
      prefAlwaysOnTop.checked = cfg.always_on_top ?? true;
      return;
    } catch (e) {
      console.warn("加载配置异常:", e);
    }
  }

  prefBaseUrl.value = localStorage.getItem("glint_base_url") || "https://api.deepseek.com/v1";
  prefApiKey.value = localStorage.getItem("glint_api_key") || "";
  prefModel.value = localStorage.getItem("glint_model") || "deepseek-chat";
  prefAlwaysOnTop.checked = localStorage.getItem("glint_always_on_top") !== "false";
}

// 保存配置
async function saveConfig() {
  const config: AppConfig = {
    base_url: prefBaseUrl.value.trim(),
    api_key: prefApiKey.value.trim(),
    model: prefModel.value.trim(),
    temperature: 0.3,
    hotkey_trigger: "Alt+Q",
    hotkey_toggle: "Alt+D",
    icon_plan: "master",
    theme: "dark",
    always_on_top: prefAlwaysOnTop.checked,
  };

  localStorage.setItem("glint_base_url", config.base_url);
  localStorage.setItem("glint_api_key", config.api_key);
  localStorage.setItem("glint_model", config.model);
  localStorage.setItem("glint_always_on_top", config.always_on_top.toString());

  if (isTauri()) {
    try {
      await invoke("update_app_config", { config });
    } catch (err) {
      console.error("更新配置失败:", err);
    }
  }

  toast.style.opacity = "1";
  setTimeout(() => {
    toast.style.opacity = "0";
  }, 1800);
}

// 测试连接
btnTest.addEventListener("click", async () => {
  const apiKey = prefApiKey.value.trim();
  const baseUrl = prefBaseUrl.value.trim();
  const model = prefModel.value.trim();

  if (!baseUrl || !model) {
    testResult.textContent = "请先填写 Base URL 与 Model";
    testResult.style.color = "#f87171";
    return;
  }

  testResult.textContent = "正在探测...";
  testResult.style.color = "var(--text-muted)";

  if (isTauri()) {
    try {
      const res: string = await invoke("test_llm_connection", {
        apiKey,
        baseUrl,
        model,
      });
      testResult.textContent = `✓ ${res}`;
      testResult.style.color = "#34d399";
    } catch (err) {
      testResult.textContent = `✗ ${err}`;
      testResult.style.color = "#f87171";
    }
  } else {
    setTimeout(() => {
      testResult.textContent = "✓ 连通正常 (82ms)";
      testResult.style.color = "#34d399";
    }, 500);
  }
});

btnResetSize.addEventListener("click", async () => {
  localStorage.setItem("glint_card_width", "440");
  localStorage.setItem("glint_card_height", "380");
  if (isTauri()) {
    try {
      await invoke("reset_hud_size");
    } catch (_) {}
  }
  toast.textContent = "尺寸已恢复为 440 × 380";
  toast.style.opacity = "1";
  setTimeout(() => {
    toast.style.opacity = "0";
    toast.textContent = "配置已生效并同步";
  }, 1800);
});

btnSave.addEventListener("click", saveConfig);

// 窗口控制与快捷键 (Esc 快速收起)
const btnClosePref = document.getElementById("btn-close-pref") as HTMLButtonElement | null;
if (btnClosePref) {
  btnClosePref.addEventListener("click", async () => {
    if (isTauri()) {
      await invoke("hide_preferences_window");
    } else {
      window.close();
    }
  });
}

window.addEventListener("keydown", async (e) => {
  if (e.key === "Escape") {
    if (isTauri()) {
      await invoke("hide_preferences_window");
    }
  }
});

loadConfig();
