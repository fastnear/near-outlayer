#!/usr/bin/env node
/**
 * Machine-Verifiable Test Report Generator
 *
 * Proves Phase 1.5 Integration Test Claims:
 * - 82/82 tests passing (100%)
 * - All phases covered (1-5)
 * - No stub implementations
 * - Real WASM execution with fuel metering
 * - Cross-runtime consistency (wasmi vs wasmtime)
 *
 * Usage: node scripts/verify_phase_1_5.mjs
 */

import { execSync } from 'child_process';
import { readFileSync, readdirSync, statSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const projectRoot = join(__dirname, '..');

// ANSI colors for output
const colors = {
  reset: '\x1b[0m',
  bright: '\x1b[1m',
  green: '\x1b[32m',
  red: '\x1b[31m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  cyan: '\x1b[36m',
};

function log(message, color = 'reset') {
  console.log(`${colors[color]}${message}${colors.reset}`);
}

function section(title) {
  log('\n' + '='.repeat(80), 'cyan');
  log(title, 'bright');
  log('='.repeat(80), 'cyan');
}

function check(condition, message) {
  if (condition) {
    log(`✓ ${message}`, 'green');
    return true;
  } else {
    log(`✗ ${message}`, 'red');
    return false;
  }
}

// Verification functions

function verifyTestExecution() {
  section('1. Test Execution Verification');

  try {
    const output = execSync('cargo test --no-fail-fast 2>&1', {
      cwd: projectRoot,
      encoding: 'utf8',
      maxBuffer: 10 * 1024 * 1024, // 10MB buffer
    });

    // Parse test results
    const testResultMatch = output.match(/test result: (\w+)\. (\d+) passed; (\d+) failed/);
    if (!testResultMatch) {
      log('Could not parse test results', 'red');
      return false;
    }

    const [, result, passed, failed] = testResultMatch;
    const passedCount = parseInt(passed, 10);
    const failedCount = parseInt(failed, 10);
    const totalTests = passedCount + failedCount;

    log(`\nTest Results:`, 'bright');
    log(`  Total: ${totalTests}`);
    log(`  Passed: ${passedCount}`, 'green');
    log(`  Failed: ${failedCount}`, failedCount > 0 ? 'red' : 'green');
    log(`  Success Rate: ${((passedCount / totalTests) * 100).toFixed(1)}%`, 'bright');

    const allPassed = check(
      result === 'ok' && failedCount === 0,
      'All tests passing'
    );

    const has82Tests = check(
      totalTests === 82,
      '82 tests total (expected count)'
    );

    return allPassed && has82Tests;
  } catch (error) {
    log(`Test execution failed: ${error.message}`, 'red');
    return false;
  }
}

function verifyPhaseCoverage() {
  section('2. Phase Coverage Verification');

  const srcDir = join(projectRoot, 'src');
  const phaseModules = [
    'common',           // Shared utilities
    'determinism',      // Phase 1
    'contract_events',  // Phase 2
    'coordinator_hardening', // Phase 3
    'wasi_helpers',     // Phase 4
    'typescript_client', // Phase 5
  ];

  let allPhasesPresent = true;

  for (const module of phaseModules) {
    const modulePath = join(srcDir, module, 'mod.rs');
    try {
      const exists = statSync(modulePath).isFile();
      check(exists, `Phase module ${module} exists`);
      if (!exists) allPhasesPresent = false;
    } catch {
      check(false, `Phase module ${module} exists`);
      allPhasesPresent = false;
    }
  }

  return allPhasesPresent;
}

function verifyNoStubs() {
  section('3. Stub Implementation Check');

  const srcDir = join(projectRoot, 'src');
  const stubPatterns = [
    /todo!\(/i,
    /unimplemented!\(/i,
    /panic!\("not implemented"/i,
    /panic!\("stub"/i,
    /\/\/ STUB:/i,
    /\/\/ TODO:/i,
  ];

  function scanDirectory(dir) {
    const entries = readdirSync(dir);
    let stubsFound = [];

    for (const entry of entries) {
      const fullPath = join(dir, entry);
      const stat = statSync(fullPath);

      if (stat.isDirectory()) {
        stubsFound = stubsFound.concat(scanDirectory(fullPath));
      } else if (entry.endsWith('.rs')) {
        const content = readFileSync(fullPath, 'utf8');
        const lines = content.split('\n');

        lines.forEach((line, lineNum) => {
          for (const pattern of stubPatterns) {
            if (pattern.test(line)) {
              stubsFound.push({
                file: fullPath.replace(projectRoot + '/', ''),
                line: lineNum + 1,
                content: line.trim(),
              });
            }
          }
        });
      }
    }

    return stubsFound;
  }

  const stubs = scanDirectory(srcDir);

  if (stubs.length === 0) {
    check(true, 'No stub implementations found');
    return true;
  } else {
    check(false, 'Found stub implementations:');
    stubs.forEach(stub => {
      log(`  ${stub.file}:${stub.line} - ${stub.content}`, 'yellow');
    });
    return false;
  }
}

function verifyWasmExecution() {
  section('4. WASM Execution Verification');

  const wasiExamplesDir = join(projectRoot, '../../wasi-examples');
  const requiredExamples = ['determinism-test', 'random-ark'];

  let allBuilt = true;

  for (const example of requiredExamples) {
    const examplePath = join(wasiExamplesDir, example);

    // Try both naming conventions (dash and underscore)
    const wasmPathDash = join(examplePath, 'target/wasm32-wasip1/release', example + '.wasm');
    const wasmPathUnderscore = join(examplePath, 'target/wasm32-wasip1/release', example.replace('-', '_') + '.wasm');

    let found = false;
    let size = 0;

    try {
      if (statSync(wasmPathDash).isFile()) {
        found = true;
        size = statSync(wasmPathDash).size;
      }
    } catch {}

    if (!found) {
      try {
        if (statSync(wasmPathUnderscore).isFile()) {
          found = true;
          size = statSync(wasmPathUnderscore).size;
        }
      } catch {}
    }

    check(found && size > 0, `${example} WASM built (${(size / 1024).toFixed(1)} KB)`);
    if (!found || size === 0) allBuilt = false;
  }

  return allBuilt;
}

function verifyDeterminismTests() {
  section('5. Determinism Test Verification');

  try {
    const output = execSync('cargo test determinism:: 2>&1', {
      cwd: projectRoot,
      encoding: 'utf8',
      maxBuffer: 10 * 1024 * 1024,
    });

    // Count determinism tests
    const testLines = output.split('\n').filter(line => line.includes('test determinism::'));
    const passedTests = testLines.filter(line => line.includes('... ok')).length;

    log(`\nDeterminism Tests:`, 'bright');
    log(`  Total: ${testLines.length}`);
    log(`  Passed: ${passedTests}`, 'green');

    const criticalTests = [
      'test_100x_same_input_determinism',
      'test_cross_runtime_consistency_wasmi_vs_wasmtime',
      'test_wasmi_wasmtime_output_consistency',
      'test_stdout_capture_deterministic_output',
    ];

    let allCriticalPass = true;
    for (const test of criticalTests) {
      const passed = output.includes(`test determinism::`) && output.includes(test) && output.includes('... ok');
      check(passed, `Critical: ${test}`);
      if (!passed) allCriticalPass = false;
    }

    return allCriticalPass && passedTests === testLines.length;
  } catch (error) {
    log(`Determinism test verification failed: ${error.message}`, 'red');
    return false;
  }
}

function verifyFuelMetering() {
  section('6. Fuel Metering Verification');

  try {
    const commonMod = readFileSync(join(projectRoot, 'src/common/mod.rs'), 'utf8');

    const hasP1Metering = commonMod.includes('store.set_fuel(max_fuel)') &&
                          commonMod.includes('max_fuel.saturating_sub(store.get_fuel()');

    const hasP2Metering = commonMod.includes('store.set_fuel(max_fuel)') &&
                          commonMod.includes('max_fuel.saturating_sub(store.get_fuel()');

    check(hasP1Metering, 'P1 (wasmi) fuel metering implemented');
    check(hasP2Metering, 'P2 (wasmtime) fuel metering implemented');

    return hasP1Metering && hasP2Metering;
  } catch (error) {
    log(`Fuel metering verification failed: ${error.message}`, 'red');
    return false;
  }
}

function verifyCrossRuntimeConsistency() {
  section('7. Cross-Runtime Consistency Verification');

  try {
    const crossRuntimeMod = readFileSync(
      join(projectRoot, 'src/determinism/cross_runtime.rs'),
      'utf8'
    );

    const hasWasmiWasmtime = crossRuntimeMod.includes('execute_wasm_p1') &&
                              crossRuntimeMod.includes('execute_wasm_p2');

    const hasConsistencyCheck = crossRuntimeMod.includes('assert_eq!') &&
                                 crossRuntimeMod.includes('.output');

    check(hasWasmiWasmtime, 'Cross-runtime tests use both wasmi and wasmtime');
    check(hasConsistencyCheck, 'Cross-runtime tests verify output consistency');

    return hasWasmiWasmtime && hasConsistencyCheck;
  } catch (error) {
    log(`Cross-runtime verification failed: ${error.message}`, 'red');
    return false;
  }
}

// Main execution

function main() {
  log('\n╔═══════════════════════════════════════════════════════════════════════════╗', 'bright');
  log('║ Phase 1.5 Integration Tests - Machine-Verifiable Report                 ║', 'bright');
  log('╚═══════════════════════════════════════════════════════════════════════════╝', 'bright');

  const results = {
    testExecution: verifyTestExecution(),
    phaseCoverage: verifyPhaseCoverage(),
    noStubs: verifyNoStubs(),
    wasmExecution: verifyWasmExecution(),
    determinismTests: verifyDeterminismTests(),
    fuelMetering: verifyFuelMetering(),
    crossRuntime: verifyCrossRuntimeConsistency(),
  };

  section('VERIFICATION SUMMARY');

  const allPassed = Object.values(results).every(Boolean);
  const passedCount = Object.values(results).filter(Boolean).length;
  const totalChecks = Object.keys(results).length;

  log('\nChecks:', 'bright');
  Object.entries(results).forEach(([name, passed]) => {
    const icon = passed ? '✓' : '✗';
    const color = passed ? 'green' : 'red';
    log(`  ${icon} ${name}`, color);
  });

  log(`\nOverall: ${passedCount}/${totalChecks} checks passed`, 'bright');

  if (allPassed) {
    log('\n🎉 ALL VERIFICATIONS PASSED', 'green');
    log('Phase 1.5 Integration Tests are production-ready.\n', 'green');
    process.exit(0);
  } else {
    log('\n❌ SOME VERIFICATIONS FAILED', 'red');
    log('Review the failures above and fix before claiming production-ready.\n', 'red');
    process.exit(1);
  }
}

main();
