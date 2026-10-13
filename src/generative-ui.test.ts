import {
  renderCompareWidget,
  renderFlowWidget,
  renderMetricsWidget,
  renderCalloutWidget,
} from "./generative-ui.ts";

const assert = {
  ok(cond: unknown, msg?: string) {
    if (!cond) throw new Error(msg || "Assertion failed");
  },
  includes(actual: string, substring: string, msg?: string) {
    if (!actual.includes(substring)) {
      throw new Error(msg || `Expected string to include "${substring}", got:\n${actual}`);
    }
  },
};

console.log("=== RUNNING GENERATIVE UI WIDGET TESTS ===");

// 1. Compare Widget Test
{
  const json = JSON.stringify({
    title: "B-Tree vs LSM",
    columns: ["维度", "B-Tree", "LSM"],
    rows: [["读性能", "O(log N)", "多层SSTable"]],
  });
  const html = renderCompareWidget(json);
  assert.includes(html, "glint-widget-compare", "应包含对比组件 class");
  assert.includes(html, "B-Tree vs LSM", "应包含标题");
  assert.includes(html, "O(log N)", "应包含第一行单元格");
  assert.includes(html, "<th>维度</th>", "应渲染表头");
  console.log("✓ PASS: renderCompareWidget");
}

// 2. Flow Widget Test
{
  const json = JSON.stringify({
    title: "零拷贝流水线",
    steps: [
      { title: "磁盘读取", desc: "DMA 直传内核缓冲" },
      { title: "Socket发送", desc: "零CPU参与拷贝" },
    ],
  });
  const html = renderFlowWidget(json);
  assert.includes(html, "glint-widget-flow", "应包含流程组件 class");
  assert.includes(html, "零拷贝流水线", "应包含流程标题");
  assert.includes(html, "DMA 直传内核缓冲", "应包含步骤描述");
  console.log("✓ PASS: renderFlowWidget");
}

// 3. Metrics Widget Test
{
  const json = JSON.stringify({
    title: "QuickSort复杂度",
    items: [
      { label: "平均时间", value: "O(N log N)", tone: "cyan" },
      { label: "最差时间", value: "O(N²)", tone: "amber" },
    ],
  });
  const html = renderMetricsWidget(json);
  assert.includes(html, "glint-widget-metrics", "应包含指标组件 class");
  assert.includes(html, "O(N log N)", "应包含指标数值");
  assert.includes(html, "tone-cyan", "应包含色调 class");
  console.log("✓ PASS: renderMetricsWidget");
}

// 4. Callout Widget Test
{
  const json = JSON.stringify({
    type: "warning",
    title: "内存泄露陷阱",
    content: "避免在循环闭包中捕获外部上下文强引用。",
  });
  const html = renderCalloutWidget(json);
  assert.includes(html, "glint-widget-callout", "应包含警示组件 class");
  assert.includes(html, "callout-warning", "应包含类型 class");
  assert.includes(html, "内存泄露陷阱", "应包含警示标题");
  assert.includes(html, "循环闭包", "应包含警示内容");
  console.log("✓ PASS: renderCalloutWidget");
}

// 5. Malformed / Incomplete JSON resilience
{
  const brokenJson = '{"title": "半截';
  const html = renderCompareWidget(brokenJson);
  assert.includes(html, "glint-widget-fallback", "非法 JSON 应安全回退到 fallback，不报错");
  console.log("✓ PASS: Widget Malformed JSON resilience");
}

console.log("ALL GENERATIVE UI TESTS PASSED!");
