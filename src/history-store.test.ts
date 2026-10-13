import { HistoryStore, type GlintSessionRecord } from "./history-store.ts";

const memoryStore = new Map<string, string>();
(globalThis as unknown as { localStorage: unknown }).localStorage = {
  getItem: (key: string) => memoryStore.get(key) || null,
  setItem: (key: string, val: string) => memoryStore.set(key, val),
  removeItem: (key: string) => memoryStore.delete(key),
  clear: () => memoryStore.clear(),
};

const assert = {
  strictEqual<T>(actual: T, expected: T, msg?: string) {
    if (actual !== expected) {
      throw new Error(msg || `Expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
    }
  },
  ok(cond: unknown, msg?: string) {
    if (!cond) throw new Error(msg || "Assertion failed");
  },
};

console.log("=== RUNNING HISTORY STORE TESTS ===");

HistoryStore.clear();
assert.strictEqual(HistoryStore.getAll().length, 0, "清空后长度应为 0");

const session1: GlintSessionRecord = {
  id: "sess_1",
  createdAt: 1000,
  updatedAt: 1000,
  hostTitle: "VS Code - main.rs",
  queryText: "tokio::sync::mpsc",
  previewSnippet: "多生产者单消费者异步信道",
  messages: [
    { role: "user", content: "tokio::sync::mpsc" },
    { role: "assistant", content: "多生产者单消费者异步信道，基于有界队列实现背压控制。" },
  ],
};

const session2: GlintSessionRecord = {
  id: "sess_2",
  createdAt: 2000,
  updatedAt: 2000,
  hostTitle: "Chrome - Bilibili",
  queryText: "零拷贝与 mmap",
  previewSnippet: "通过内核缓冲区转移数据",
  messages: [
    { role: "user", content: "零拷贝与 mmap" },
    { role: "assistant", content: "零拷贝消除了用户空间往返拷贝开销。" },
  ],
};

HistoryStore.save(session1);
HistoryStore.save(session2);

const all = HistoryStore.getAll();
assert.strictEqual(all.length, 2, "应存有两条记录");
assert.strictEqual(all[0].id, "sess_2", "最新更新的会话应排在首位");

// 搜索测试
const searchRes1 = HistoryStore.search("tokio");
assert.strictEqual(searchRes1.length, 1, "应精准搜出 tokio 会话");
assert.strictEqual(searchRes1[0].id, "sess_1");

const searchRes2 = HistoryStore.search("bilibili");
assert.strictEqual(searchRes2.length, 1, "应支持按宿主窗口搜索");

// Markdown 导出测试
const md = HistoryStore.exportToMarkdown();
assert.ok(md.includes("# Glint 认知透视伴生笔记"), "Markdown 应包含主标题");
assert.ok(md.includes("tokio::sync::mpsc"), "Markdown 应包含会话1内容");
assert.ok(md.includes("零拷贝与 mmap"), "Markdown 应包含会话2内容");

// 单条删除测试
HistoryStore.delete("sess_1");
assert.strictEqual(HistoryStore.getAll().length, 1, "删除后应剩余1条");
assert.strictEqual(HistoryStore.getById("sess_1"), undefined, "sess_1 应已被删除");

console.log("ALL HISTORY STORE TESTS PASSED!");
