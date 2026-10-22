/**
 * Glint Session History Store
 *
 * Provides persistent local session history, fuzzy search, session restoration,
 * and Markdown export for reading companions.
 */

export type ContentPart =
  | { type: "text"; text: string }
  | { type: "image_url"; image_url: { url: string } };

export type MessageContent = string | ContentPart[];

export interface ChatMessage {
  role: "system" | "user" | "assistant";
  content: MessageContent;
}

export interface ToolCallPayload {
  name: string;
  args: string;
  status: string;
  result?: string | null;
}

export interface GlintSessionRecord {
  id: string;
  createdAt: number;
  updatedAt: number;
  hostTitle?: string | null;
  queryText: string;
  isMultimodal?: boolean;
  imageDataUrl?: string;
  previewSnippet: string;
  messages: ChatMessage[];
  /**
   * Thread anchor (`context-key`) the session belongs to. Turns that share an
   * anchor are the same reading thread even across HUD reopenings, so a later
   * selection can genuinely "continue" the earlier one.
   */
  contextKey?: string;
  /**
   * Document digest inside that app, from `glint_core::HostIdentity`. Only the
   * digest is stored, never the raw window title.
   */
  docKey?: string | null;
}

const STORAGE_KEY = "glint_session_history_v1";
const MAX_SESSIONS = 100;

export class HistoryStore {
  /**
   * 读取全部存储的会话（按最新更新时间倒序排序）
   */
  static getAll(): GlintSessionRecord[] {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (!raw) return [];
      const list: GlintSessionRecord[] = JSON.parse(raw);
      if (!Array.isArray(list)) return [];
      return list.sort((a, b) => b.updatedAt - a.updatedAt);
    } catch {
      return [];
    }
  }

  /**
   * 保存或更新会话
   */
  static save(record: GlintSessionRecord): void {
    try {
      const list = this.getAll();
      const existingIdx = list.findIndex((item) => item.id === record.id);

      if (existingIdx !== -1) {
        list[existingIdx] = {
          ...record,
          updatedAt: Date.now(),
        };
      } else {
        list.unshift({
          ...record,
          updatedAt: record.updatedAt || Date.now(),
        });
      }

      // 保留最近 MAX_SESSIONS 条，防止溢出
      if (list.length > MAX_SESSIONS) {
        list.length = MAX_SESSIONS;
      }

      localStorage.setItem(STORAGE_KEY, JSON.stringify(list));
    } catch (e) {
      console.warn("保存会话历史失败:", e);
    }
  }

  /**
   * 根据 ID 获取单个会话
   */
  static getById(id: string): GlintSessionRecord | undefined {
    const list = this.getAll();
    return list.find((item) => item.id === id);
  }

  /**
   * 模糊搜索历史记录
   */
  static search(keyword: string): GlintSessionRecord[] {
    const trimmed = keyword.trim().toLowerCase();
    if (!trimmed) return this.getAll();

    return this.getAll().filter((item) => {
      const matchQuery = item.queryText.toLowerCase().includes(trimmed);
      const matchHost = (item.hostTitle || "").toLowerCase().includes(trimmed);
      const matchSnippet = item.previewSnippet.toLowerCase().includes(trimmed);
      return matchQuery || matchHost || matchSnippet;
    });
  }

  /**
   * 删除指定会话
   */
  static delete(id: string): void {
    try {
      const list = this.getAll().filter((item) => item.id !== id);
      localStorage.setItem(STORAGE_KEY, JSON.stringify(list));
    } catch (e) {
      console.warn("删除会话失败:", e);
    }
  }

  /**
   * 清空所有历史会话
   */
  static clear(): void {
    try {
      localStorage.removeItem(STORAGE_KEY);
    } catch (e) {
      console.warn("清空历史记录失败:", e);
    }
  }

  /**
   * 将会话导出为规范 Markdown 笔记
   */
  static exportToMarkdown(sessions?: GlintSessionRecord[]): string {
    const list = sessions || this.getAll();
    if (list.length === 0) {
      return "# Glint 认知透视伴生笔记\n\n暂无历史会话记录。";
    }

    const lines: string[] = [
      "# Glint 认知透视伴生笔记",
      `*导出时间: ${new Date().toLocaleString()} · 共 ${list.length} 条会话*`,
      "",
      "---",
      "",
    ];

    for (const session of list) {
      const dateStr = new Date(session.createdAt).toLocaleString();
      const hostStr = session.hostTitle ? ` · 宿主: \`${session.hostTitle}\`` : "";
      lines.push(`## ${session.queryText}`);
      lines.push(`> 记录时间: ${dateStr}${hostStr}`);
      lines.push("");

      for (const msg of session.messages) {
        if (msg.role === "user") {
          const text = typeof msg.content === "string" ? msg.content : "【多模态视窗标注】";
          lines.push(`**用户提问 / 选词**: ${text}`);
          lines.push("");
        } else if (msg.role === "assistant") {
          const text = typeof msg.content === "string" ? msg.content : "";
          lines.push(`**Glint 透视解答**:`);
          lines.push("");
          lines.push(text);
          lines.push("");
        }
      }

      lines.push("---");
      lines.push("");
    }

    return lines.join("\n");
  }
}
