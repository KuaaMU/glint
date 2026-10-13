import assert from "node:assert/strict";

export class TestRegistry {
  constructor(name = "Default Suite") {
    this.name = name;
    this.cases = [];
    this.passed = 0;
    this.failed = 0;
    this.skipped = 0;
    this.errors = [];
  }

  test(description, fn, options = {}) {
    this.cases.push({ description, fn, options });
  }

  async run(filterOptions = {}) {
    const { verbose = false, milestoneFilter = null } = filterOptions;
    console.log(`\n=== Running Suite: ${this.name} ===`);
    const startTime = Date.now();

    for (const testCase of this.cases) {
      if (milestoneFilter && testCase.options.milestone && testCase.options.milestone !== milestoneFilter) {
        this.skipped++;
        if (verbose) {
          console.log(`  [SKIP] ${testCase.description} (milestone ${testCase.options.milestone} != ${milestoneFilter})`);
        }
        continue;
      }

      try {
        const caseStart = Date.now();
        await testCase.fn();
        const duration = Date.now() - caseStart;
        this.passed++;
        if (verbose) {
          console.log(`  ✓ PASS: ${testCase.description} (${duration}ms)`);
        }
      } catch (err) {
        this.failed++;
        console.error(`  ✗ FAIL: ${testCase.description}`);
        console.error(`    Error: ${err.message}`);
        this.errors.push({ description: testCase.description, error: err });
      }
    }

    const totalDuration = Date.now() - startTime;
    console.log(
      `--- ${this.name} Summary: ${this.passed} passed, ${this.failed} failed, ${this.skipped} skipped (${totalDuration}ms) ---\n`
    );

    return {
      name: this.name,
      total: this.cases.length,
      passed: this.passed,
      failed: this.failed,
      skipped: this.skipped,
      errors: this.errors,
      duration: totalDuration,
    };
  }
}

export { assert };
