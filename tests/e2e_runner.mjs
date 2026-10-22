#!/usr/bin/env node

/**
 * Glint Unified End-to-End (E2E) Test Runner
 *
 * Runs the 4-tier systematic testing framework derived from
 * ORIGINAL_REQUEST.md and PROJECT.md § Feature Inventory (Features 1-18).
 *
 * Usage:
 *   node tests/e2e_runner.mjs
 *   node tests/e2e_runner.mjs --tier=1,2
 *   node tests/e2e_runner.mjs --milestone=M1
 *   node tests/e2e_runner.mjs --verbose
 */

import process from "node:process";
import { tier1Suite } from "./tier1_features.test.mjs";
import { tier2Suite } from "./tier2_boundaries.test.mjs";
import { tier3Suite } from "./tier3_combinations.test.mjs";
import { tier4Suite } from "./tier4_workloads.test.mjs";

function parseArgs() {
  const args = process.argv.slice(2);
  const options = {
    tiers: [1, 2, 3, 4],
    milestone: null,
    verbose: false,
    help: false,
  };

  for (const arg of args) {
    if (arg === "--help" || arg === "-h") {
      options.help = true;
    } else if (arg === "--verbose" || arg === "-v") {
      options.verbose = true;
    } else if (arg.startsWith("--tier=")) {
      const tierVal = arg.slice("--tier=".length);
      options.tiers = tierVal.split(",").map((t) => parseInt(t.trim(), 10));
    } else if (arg.startsWith("--milestone=")) {
      options.milestone = arg.slice("--milestone=".length).trim();
    }
  }

  return options;
}

function printHelp() {
  console.log(`
Glint E2E Unified Test Runner
-----------------------------
Usage:
  node tests/e2e_runner.mjs [options]

Options:
  --tier=<1,2,3,4>     Specify comma-separated tiers to run (default: 1,2,3,4)
  --milestone=<M1..M4> Filter test cases by milestone tag (e.g. M1, M2, M3, M4)
  --verbose, -v        Display verbose test execution logs
  --help, -h           Show this help message
`);
}

async function main() {
  const options = parseArgs();

  if (options.help) {
    printHelp();
    process.exit(0);
  }

  console.log("\n========================================================");
  console.log("       GLINT E2E TEST RUNNER - 4-TIER SUITE");
  console.log("========================================================");
  console.log(`Config: Tiers=[${options.tiers.join(", ")}], MilestoneFilter=${options.milestone || "ALL"}, Verbose=${options.verbose}\n`);

  const suitesToRun = [];
  if (options.tiers.includes(1)) suitesToRun.push(tier1Suite);
  if (options.tiers.includes(2)) suitesToRun.push(tier2Suite);
  if (options.tiers.includes(3)) suitesToRun.push(tier3Suite);
  if (options.tiers.includes(4)) suitesToRun.push(tier4Suite);

  const results = [];
  let totalTests = 0;
  let totalPassed = 0;
  let totalFailed = 0;
  let totalSkipped = 0;
  const runnerStart = Date.now();

  for (const suite of suitesToRun) {
    const res = await suite.run({
      verbose: options.verbose,
      milestoneFilter: options.milestone,
    });
    results.push(res);
    totalTests += res.total;
    totalPassed += res.passed;
    totalFailed += res.failed;
    totalSkipped += res.skipped;
  }

  const overallDuration = Date.now() - runnerStart;

  // Print Summary Table
  console.log("\n========================================================");
  console.log("                E2E TEST EXECUTION SUMMARY");
  console.log("========================================================");
  console.log(
    "| Suite / Tier                          | Total | Pass | Fail | Skip | Time (ms) |"
  );
  console.log(
    "|---------------------------------------|-------|------|------|------|-----------|"
  );

  for (const r of results) {
    const namePadded = r.name.padEnd(37, " ");
    const tot = String(r.total).padStart(5, " ");
    const pas = String(r.passed).padStart(4, " ");
    const fai = String(r.failed).padStart(4, " ");
    const ski = String(r.skipped).padStart(4, " ");
    const dur = String(r.duration).padStart(9, " ");
    console.log(`| ${namePadded} | ${tot} | ${pas} | ${fai} | ${ski} | ${dur} |`);
  }

  console.log(
    "|---------------------------------------|-------|------|------|------|-----------|"
  );
  const totStr = String(totalTests).padStart(5, " ");
  const pasStr = String(totalPassed).padStart(4, " ");
  const faiStr = String(totalFailed).padStart(4, " ");
  const skiStr = String(totalSkipped).padStart(4, " ");
  const durStr = String(overallDuration).padStart(9, " ");
  console.log(
    `| TOTAL                                 | ${totStr} | ${pasStr} | ${faiStr} | ${skiStr} | ${durStr} |`
  );
  console.log("========================================================\n");

  // These are static contract checks, not runtime acceptance tests.
  // Runtime behavior (hotkey delivery, PRIMARY capture, layer-shell placement,
  // IME, first-frame latency, RSS) is tracked in tests/RUNTIME_ACCEPTANCE.md.
  console.log("Contract-test note:");
  console.log("  - This runner checks source/config contracts and unit behaviour only.");
  console.log("  - It does NOT prove runtime Wayland integration or memory/latency targets.");
  console.log(`  - Result: ${totalPassed} passed, ${totalFailed} failed, ${totalSkipped} skipped.\n`);

  if (totalFailed > 0) {
    console.error(`💥 TEST SUITE FAILED: ${totalFailed} test(s) failed.`);
    process.exit(1);
  } else {
    console.log(
      `🎉 CONTRACT SUITE GREEN (${totalPassed} passed, ${totalSkipped} skipped, ${totalTests} registered).`
    );
    process.exit(0);
  }
}

main().catch((err) => {
  console.error("Fatal runner exception:", err);
  process.exit(1);
});
