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
const testResult = document.getElementById("test-result-indicator") as HTMLSpanElement;
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

document.querySelectorAll(".preset-chip").forEach((btn) => {
  btn.addEventListener("click", () => {
    document.querySelectorAll(".preset-chip").forEach((b) => b.classList.remove("active"));
    btn.classList.add("active");
    const p = (btn as HTMLElement).dataset.preset;
    if (p && presets[p]) {
      if (presets[p].base_url) prefBaseUrl.value = presets[p].base_url;
      if (presets[p].model) prefModel.value = presets[p].model;
      if (prefApiKey.value.trim() || prefBaseUrl.value.includes("localhost")) {
        triggerSoftAutoFetch();
      }
    }
  });
});

// API Key 密码框显隐切换
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
      fetchStatusText.textContent = "请先输入 Base URL 端点地址";
      fetchStatusText.style.color = "#EF4444";
    }
    return;
  }

  fetchSpinner.style.display = "inline";
  fetchBtnLabel.textContent = "探测中...";
  btnFetchModels.disabled = true;
  if (!silent) {
    fetchStatusText.textContent = "正在向端点探测可用模型列表...";
    fetchStatusText.style.color = "var(--text-muted)";
  }

  try {
    let models: string[] = [];
    if (isTauri()) {
      models = await invoke<string[]>("fetch_models", { apiKey, baseUrl });
    } else {
      // 前端环境模拟
      await new Promise((r) => setTimeout(r, 600));
      models = [
        "deepseek-chat",
        "deepseek-reasoner",
        "gpt-4o",
        "gpt-4o-mini",
        "qwen2.5:7b",
        "moonshot-v1-8k",
      ];
    }

    if (models.length > 0) {
      // 填充 datalist 供自动补全
      modelDatalist.innerHTML = "";
      // 填充 select 供用户快捷点选
      prefModelSelect.innerHTML = '<option value="">-- 选择拉取的模型 (' + models.length + '个) --</option>';

      models.forEach((m) => {
        const optData = document.createElement("option");
        optData.value = m;
        modelDatalist.appendChild(optData);

        const optSelect = document.createElement("option");
        optSelect.value = m;
        optSelect.textContent = m;
        if (m === prefModel.value.trim()) {
          optSelect.selected = true;
        }
        prefModelSelect.appendChild(optSelect);
      });

      prefModelSelect.style.display = "block";
      fetchStatusText.textContent = `✓ 成功拉取 ${models.length} 个模型 (可通过下拉菜单直接点选)`;
      fetchStatusText.style.color = "#10B981";

      // 若当前输入框为空，自动填充首个推荐模型
      if (!prefModel.value.trim() && models[0]) {
        prefModel.value = models[0];
      }
    } else {
      fetchStatusText.textContent = "未发现任何模型，请检查端点返回数据";
      fetchStatusText.style.color = "#F59E0B";
    }
  } catch (err) {
    fetchStatusText.textContent = `✗ 拉取失败: ${err}`;
    fetchStatusText.style.color = "#EF4444";
  } finally {
    fetchSpinner.style.display = "none";
    fetchBtnLabel.textContent = "🔄 自动拉取模型";
    btnFetchModels.disabled = false;
  }
}

// 绑定模型拉取事件
btnFetchModels.addEventListener("click", () => fetchModelsFromApi(false));

// 下拉列表选择同步至输入框
prefModelSelect.addEventListener("change", () => {
  if (prefModelSelect.value) {
    prefModel.value = prefModelSelect.value;
  }
});

// 当 Base URL 或 API Key 失去焦点且两者非空时，温和自动拉取一次
let autoFetchDebounceTimer: any = null;
function triggerSoftAutoFetch() {
  clearTimeout(autoFetchDebounceTimer);
  autoFetchDebounceTimer = setTimeout(() => {
    if (prefBaseUrl.value.trim() && (prefApiKey.value.trim() || prefBaseUrl.value.includes("localhost"))) {
      fetchModelsFromApi(true);
    }
  }, 800);
}

prefBaseUrl.addEventListener("blur", triggerSoftAutoFetch);
prefApiKey.addEventListener("blur", triggerSoftAutoFetch);

// 载入已有配置
async function loadConfig() {
  if (isTauri()) {
    try {
      const cfg: AppConfig = await invoke("get_app_config");
      prefBaseUrl.value = cfg.base_url || "https://api.deepseek.com/v1";
      prefApiKey.value = cfg.api_key || "";
      prefModel.value = cfg.model || "deepseek-chat";
      prefAlwaysOnTop.checked = cfg.always_on_top ?? true;
      if (prefBaseUrl.value.trim() && (prefApiKey.value.trim() || prefBaseUrl.value.includes("localhost"))) {
        fetchModelsFromApi(true);
      }
      return;
    } catch (e) {
      console.warn("加载 Tauri 配置异常，回退至 localStorage:", e);
    }
  }

  prefBaseUrl.value = localStorage.getItem("glint_base_url") || "https://api.deepseek.com/v1";
  prefApiKey.value = localStorage.getItem("glint_api_key") || "";
  prefModel.value = localStorage.getItem("glint_model") || "deepseek-chat";
  prefAlwaysOnTop.checked = localStorage.getItem("glint_always_on_top") !== "false";
  if (prefBaseUrl.value.trim() && (prefApiKey.value.trim() || prefBaseUrl.value.includes("localhost"))) {
    fetchModelsFromApi(true);
  }
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
  }, 2200);
}

// 测试连接
btnTest.addEventListener("click", async () => {
  const apiKey = prefApiKey.value.trim();
  const baseUrl = prefBaseUrl.value.trim();
  const model = prefModel.value.trim();

  if (!baseUrl || !model) {
    testResult.textContent = "请先填写 Base URL 和 Model";
    testResult.style.color = "#EF4444";
    return;
  }

  testResult.textContent = "正在探测连接与响应延迟...";
  testResult.style.color = "var(--text-muted)";

  if (isTauri()) {
    try {
      const res: string = await invoke("test_llm_connection", {
        apiKey,
        baseUrl,
        model,
      });
      testResult.textContent = `✓ ${res}`;
      testResult.style.color = "#10B981";
    } catch (err) {
      testResult.textContent = `✗ 连接失败: ${err}`;
      testResult.style.color = "#EF4444";
    }
  } else {
    // 纯前端模拟
    setTimeout(() => {
      testResult.textContent = "✓ 模拟连接成功 (延迟 85ms)";
      testResult.style.color = "#10B981";
    }, 600);
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
  toast.textContent = "卡片尺寸已重置为 440 × 380";
  toast.style.opacity = "1";
  setTimeout(() => {
    toast.style.opacity = "0";
    toast.textContent = "配置已保存并同步至伴生卡片";
  }, 2000);
});

btnSave.addEventListener("click", saveConfig);

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
