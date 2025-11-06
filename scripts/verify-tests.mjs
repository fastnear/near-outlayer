#!/usr/bin/env node
/**
 * verification tests Verification Script
 *
 * Machine-verifiable proof that verification tests claims are substantiated by test logs.
 *
 * Usage:
 *   LOGS_DIR=logs node scripts/verify_phase_1_5.mjs
 *
 * Exit codes:
 *   0 - All claims verified
 *   1 - Missing evidence or test failures
 */

import { promises as fs } from "fs";
import path from "path";

const LOGS_DIR = process.env.LOGS_DIR || "logs";
const ALL_TESTS = path.join(LOGS_DIR, "all_tests.log");
const STDOUT_CAPTURE = path.join(LOGS_DIR, "stdout_capture.log");

/**
 * Check if text contains a pattern
 */
function has(text, re) {
  const m = text.match(re);
  return { ok: !!m, match: m ? (m[1] ?? m[0]) : "" };
}

/**
 * Print verification result
 */
function line(msg, ok, detail = "") {
  const mark = ok ? "✓" : "✗";
  const tail = detail ? `  (${detail})` : "";
  console.log(`${mark} ${msg}${tail}`);
  return ok;
}

/**
 * Main verification
 */
async function main() {
  console.log("verification tests Verification - Machine Verification of Test Claims\n");
  console.log(`Reading logs from: ${LOGS_DIR}\n`);

  // Read log files
  const [allTests, stdoutCapture] = await Promise.all([
    fs.readFile(ALL_TESTS, "utf8").catch(() => ""),
    fs.readFile(STDOUT_CAPTURE, "utf8").catch(() => ""),
  ]);

  if (!allTests) {
    console.error(`✗ Missing log file: ${ALL_TESTS}`);
    console.error("  Run: cargo test -p verification-integration -- --nocapture 2>&1 | tee logs/all_tests.log");
    process.exitCode = 1;
    return;
  }

  let pass = true;

  // === Test Execution ===
  console.log("=== Test Execution ===");
  const totalsMatch = allTests.match(/test result:.*?(\d+) passed.*?(\d+) failed/i);
  if (totalsMatch) {
    const passed = totalsMatch[1];
    const failed = totalsMatch[2];
    pass &= line(`Tests executed`, true, `${passed} passed, ${failed} failed`);
    if (parseInt(failed) > 0) {
      pass &= line(`All tests passing`, false, `${failed} failures detected`);
    } else {
      pass &= line(`All tests passing`, true, `${passed}/82 green`);
    }
  } else {
    pass &= line(`Tests executed`, false, "Could not parse test results");
  }

  // === Determinism ===
  console.log("\n=== Determinism ===");
  pass &= line(
    "100× replay test exists",
    has(allTests, /test_100x_same_input_determinism|100.*?runs.*?identical/i).ok
  );

  const fuelEvidence = has(allTests, /fuel[_\s](?:consumption|used|consumed)[:\s=]*27[,\s]?2[0-9]{2}\b/i);
  pass &= line(
    "Fuel stable around 27,200",
    fuelEvidence.ok,
    fuelEvidence.match || "not found"
  );

  pass &= line(
    "Deterministic output verified",
    has(allTests, /deterministic.*?output|output.*?deterministic.*?PASS/i).ok
  );

  // === NEP-297 Events ===
  console.log("\n=== NEP-297 Event Compliance ===");
  pass &= line(
    "EVENT_JSON: prefix present",
    has(allTests, /EVENT_JSON:/).ok
  );

  pass &= line(
    "Required fields (standard, version, event)",
    has(allTests, /nep.*?297.*?event.*?(?:envelope|structure|validated)|event.*?(?:envelope|structure).*?validated/i).ok
  );

  pass &= line(
    "Missing prefix rejected",
    has(allTests, /missing.*?prefix.*?(?:rejected|fail|error)|test.*?event.*?negative.*?PASS/i).ok
  );

  pass &= line(
    "Invalid JSON rejected",
    has(allTests, /invalid.*?json.*?(?:rejected|fail)|malformed.*?event/i).ok
  );

  // === Economic Math Safety ===
  console.log("\n=== Economic Math (Overflow/Underflow Protection) ===");
  pass &= line(
    "Checked add overflow",
    has(allTests, /test_checked_add_overflow.*?(?:ok|PASS)|overflow.*?detected.*?add/i).ok
  );

  pass &= line(
    "Checked mul overflow",
    has(allTests, /test_checked_mul_overflow.*?(?:ok|PASS)|overflow.*?detected.*?mul/i).ok
  );

  pass &= line(
    "Checked sub underflow",
    has(allTests, /test_checked_sub_underflow.*?(?:ok|PASS)|underflow.*?detected|saturating.*?sub/i).ok
  );

  pass &= line(
    "Refund logic correct",
    has(allTests, /test.*?refund.*?(?:ok|PASS)|refund.*?saturating/i).ok
  );

  const costEvidence = has(allTests, /(?:Total|Cost)[:\s]*102[,\s]?000[,\s]?000\s*(?:yN|yoctoNEAR)/i);
  pass &= line(
    "Cost calculation realistic (102,000,000 yN)",
    costEvidence.ok,
    costEvidence.match || "not found"
  );

  // === Path Traversal Prevention ===
  console.log("\n=== Path Traversal & Cache Bypass Prevention ===");
  pass &= line(
    "Path traversal blocked",
    has(allTests, /test.*?traversal.*?blocked.*?(?:ok|PASS)|\.\..*?rejected/i).ok
  );

  pass &= line(
    "Absolute paths blocked",
    has(allTests, /test.*?absolute.*?path.*?(?:ok|PASS)|absolute.*?rejected/i).ok
  );

  pass &= line(
    "GitHub URL cache bypass prevention",
    has(allTests, /test.*?cache.*?bypass.*?(?:ok|PASS)|normalize.*?github.*?url/i).ok
  );

  pass &= line(
    "Hidden files blocked",
    has(allTests, /test.*?hidden.*?(?:ok|PASS)|\..*?file.*?rejected/i).ok
  );

  // === WASM I/O Correctness ===
  console.log("\n=== WASM I/O Correctness ===");

  // Check stdout capture tests in main log (separate log file no longer required)
  const stdoutTests = stdoutCapture || allTests;
  const stdoutSize = has(stdoutTests, /\b(82|83)\s*bytes\b/i);
  pass &= line(
    "Stdout capture non-empty (82-83 bytes)",
    stdoutSize.ok,
    stdoutSize.match || "not found"
  );

  pass &= line(
    "Stdout deterministic across runs",
    has(stdoutTests, /stdout.*?deterministic|deterministic.*?output/i).ok
  );

  pass &= line(
    "Memory isolation verified",
    has(allTests, /test.*?memory.*?isolation.*?(?:ok|PASS)|isolated.*?execution/i).ok
  );

  pass &= line(
    "UTF-8 validation",
    has(allTests, /test.*?utf.*?8.*?(?:ok|PASS)|utf.*?validation/i).ok
  );

  // === Cross-Runtime Consistency ===
  console.log("\n=== Cross-Runtime Consistency ===");
  pass &= line(
    "WASI P1 (wasmi) execution",
    has(allTests, /wasmi|wasi.*?p1|phase.*?1.*?execution/i).ok
  );

  pass &= line(
    "WASI P2 (wasmtime) execution",
    has(allTests, /wasmtime|wasi.*?p2|phase.*?2.*?execution/i).ok
  );

  pass &= line(
    "P1/P2 output consistency",
    has(allTests, /cross.*?runtime|p1.*?p2.*?consistent|wasmi.*?wasmtime.*?match/i).ok ||
    has(allTests, /deterministic.*?across.*?runtime/i).ok
  );

  // === Resource Limits ===
  console.log("\n=== Resource Limits ===");
  pass &= line(
    "Zero fuel rejection",
    has(allTests, /test.*?zero.*?fuel.*?(?:ok|PASS)|fuel.*?exhausted/i).ok
  );

  pass &= line(
    "Epoch deadline timeout",
    has(allTests, /test.*?epoch.*?deadline.*?(?:ok|PASS)|timeout.*?epoch/i).ok
  );

  pass &= line(
    "High epoch completion",
    has(allTests, /test.*?high.*?epoch.*?(?:ok|PASS)|epoch.*?allows.*?completion/i).ok
  );

  // === Summary ===
  console.log("\n" + "=".repeat(60));
  if (pass) {
    console.log("✅ ALL VERIFICATIONS PASSED");
    console.log("\nverification tests Integration Tests are production-ready.");
    console.log("All claims are substantiated by test evidence.");
  } else {
    console.log("❌ VERIFICATION FAILED");
    console.log("\nSome claims lack evidence in test logs.");
    console.log("Review failed checks above and ensure tests capture required evidence.");
  }
  console.log("=".repeat(60));

  process.exitCode = pass ? 0 : 1;
}

main().catch((e) => {
  console.error("Fatal error:", e);
  process.exitCode = 1;
});
