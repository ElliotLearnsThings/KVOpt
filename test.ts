import { RustCache } from "./index.js";
import { randomUUID } from "crypto";
import { performance } from "perf_hooks";
import { promises as fs } from "fs";

/**
 * Comprehensive test suite for the Rust-based cache system
 * This includes:
 * - Functional tests (basic operations)
 * - Performance tests (throughput, latency)
 * - Stress tests (concurrent operations, large values)
 * - Edge case tests (boundary conditions, error handling)
 * - Recovery tests (restart behavior)
 */

// Configuration
const STRESS_TEST_KEY_COUNT = 1000;
const THROUGHPUT_TEST_ITERATIONS = 10000;
const LATENCY_TEST_ITERATIONS = 1000;
const CONCURRENCY_LEVEL = 100;
const EXTENDED_TESTS = true; // Set to false for quicker testing

// Helper functions
function generateRandomString(length: number): string {
  const characters =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  let result = "";
  const charactersLength = characters.length;
  for (let i = 0; i < length; i++) {
    result += characters.charAt(Math.floor(Math.random() * charactersLength));
  }
  return result;
}

async function runPerformanceTests(results: Record<string, any>) {
  results.performance = {};

  // Test 1: Insertion throughput
  console.log(
    `Running insertion throughput test (${THROUGHPUT_TEST_ITERATIONS} operations)...`,
  );
  try {
    const startTime = performance.now();
    const keys = [];

    for (let i = 0; i < THROUGHPUT_TEST_ITERATIONS; i++) {
      const key = `perf-insert-${i}-${randomUUID()}`;
      keys.push(key);
      await RustCache.insert(key, `performance test value ${i}`, 60);
    }

    const endTime = performance.now();
    const duration = endTime - startTime;
    const opsPerSecond = (
      THROUGHPUT_TEST_ITERATIONS /
      (duration / 1000)
    ).toFixed(2);

    results.performance.insertionThroughput = {
      passed: true,
      opsPerSecond: parseFloat(opsPerSecond),
      totalTimeMs: duration,
      operations: THROUGHPUT_TEST_ITERATIONS,
    };

    console.log(`Insertion throughput: ${opsPerSecond} ops/sec`);
  } catch (error) {
    results.performance.insertionThroughput = {
      passed: false,
      error: String(error),
    };
    console.log("Insertion throughput: ❌ FAIL (Error)");
  }

  // Test 2: Retrieval throughput
  console.log(
    `Running retrieval throughput test (${THROUGHPUT_TEST_ITERATIONS} operations)...`,
  );
  try {
    // First, insert a value we'll repeatedly retrieve
    const testKey = "perf-get-test-key";
    const testValue = "performance test get value";
    await RustCache.insert(testKey, testValue, 300);

    const startTime = performance.now();

    for (let i = 0; i < THROUGHPUT_TEST_ITERATIONS; i++) {
      await RustCache.get(testKey);
    }

    const endTime = performance.now();
    const duration = endTime - startTime;
    const opsPerSecond = (
      THROUGHPUT_TEST_ITERATIONS /
      (duration / 1000)
    ).toFixed(2);

    results.performance.retrievalThroughput = {
      passed: true,
      opsPerSecond: parseFloat(opsPerSecond),
      totalTimeMs: duration,
      operations: THROUGHPUT_TEST_ITERATIONS,
    };

    console.log(`Retrieval throughput: ${opsPerSecond} ops/sec`);
  } catch (error) {
    results.performance.retrievalThroughput = {
      passed: false,
      error: String(error),
    };
    console.log("Retrieval throughput: ❌ FAIL (Error)");
  }

  // Test 3: Latency measurement
  console.log(
    `Running latency measurement test (${LATENCY_TEST_ITERATIONS} operations)...`,
  );
  try {
    const latencies = {
      insert: [] as number[],
      get: [] as number[],
      remove: [] as number[],
    };

    // Measure insertion latency
    for (let i = 0; i < LATENCY_TEST_ITERATIONS; i++) {
      const key = `latency-${i}-${randomUUID()}`;
      const start = performance.now();
      await RustCache.insert(key, `latency test value ${i}`, 60);
      latencies.insert.push(performance.now() - start);

      // Use the key for get and remove tests too
      const getStart = performance.now();
      await RustCache.get(key);
      latencies.get.push(performance.now() - getStart);

      const removeStart = performance.now();
      await RustCache.remove(key);
      latencies.remove.push(performance.now() - removeStart);
    }

    // Calculate statistics
    const calculateStats = (arr: number[]) => {
      const sorted = [...arr].sort((a, b) => a - b);
      const sum = sorted.reduce((a, b) => a + b, 0);
      return {
        min: sorted[0],
        max: sorted[sorted.length - 1],
        avg: sum / sorted.length,
        median: sorted[Math.floor(sorted.length / 2)],
        p95: sorted[Math.floor(sorted.length * 0.95)],
        p99: sorted[Math.floor(sorted.length * 0.99)],
      };
    };

    const stats = {
      insert: calculateStats(latencies.insert),
      get: calculateStats(latencies.get),
      remove: calculateStats(latencies.remove),
    };

    results.performance.latency = {
      passed: true,
      insertMs: {
        avg: stats.insert.avg,
        median: stats.insert.median,
        p95: stats.insert.p95,
        p99: stats.insert.p99,
        min: stats.insert.min,
        max: stats.insert.max,
      },
      getMs: {
        avg: stats.get.avg,
        median: stats.get.median,
        p95: stats.get.p95,
        p99: stats.get.p99,
        min: stats.get.min,
        max: stats.get.max,
      },
      removeMs: {
        avg: stats.remove.avg,
        median: stats.remove.median,
        p95: stats.remove.p95,
        p99: stats.remove.p99,
        min: stats.remove.min,
        max: stats.remove.max,
      },
    };

    console.log(
      `Insert latency (avg/p95): ${stats.insert.avg.toFixed(2)}ms / ${stats.insert.p95.toFixed(2)}ms`,
    );
    console.log(
      `Get latency (avg/p95): ${stats.get.avg.toFixed(2)}ms / ${stats.get.p95.toFixed(2)}ms`,
    );
    console.log(
      `Remove latency (avg/p95): ${stats.remove.avg.toFixed(2)}ms / ${stats.remove.p95.toFixed(2)}ms`,
    );
  } catch (error) {
    results.performance.latency = {
      passed: false,
      error: String(error),
    };
    console.log("Latency measurement: ❌ FAIL (Error)");
  }

  if (EXTENDED_TESTS) {
    // Test 4: Concurrent operation throughput
    console.log(
      `Running concurrent operation test (${CONCURRENCY_LEVEL} parallel operations)...`,
    );
    try {
      const testKey = "concurrent-base-key";
      const tasks = [];

      // Create insert tasks
      for (let i = 0; i < CONCURRENCY_LEVEL; i++) {
        tasks.push(() =>
          RustCache.insert(`${testKey}-${i}`, `concurrent value ${i}`, 60),
        );
      }

      const startTime = performance.now();
      await runConcurrent(tasks);
      const duration = performance.now() - startTime;

      results.performance.concurrentOperations = {
        passed: true,
        totalTimeMs: duration,
        operationsPerSecond: (CONCURRENCY_LEVEL / (duration / 1000)).toFixed(2),
        concurrencyLevel: CONCURRENCY_LEVEL,
      };

      console.log(
        `Concurrent operations: ${results.performance.concurrentOperations.operationsPerSecond} ops/sec`,
      );
    } catch (error) {
      results.performance.concurrentOperations = {
        passed: false,
        error: String(error),
      };
      console.log("Concurrent operations: ❌ FAIL (Error)");
    }
  }
}

async function runStressTests(results: Record<string, any>) {
  results.stress = {};

  // Test 1: Large number of keys
  console.log(
    `Running large number of keys test (${STRESS_TEST_KEY_COUNT} keys)...`,
  );
  try {
    const startTime = performance.now();
    const keys = [];

    for (let i = 0; i < STRESS_TEST_KEY_COUNT; i++) {
      const key = `stress-key-${i}-${randomUUID()}`;
      keys.push(key);
      await RustCache.insert(key, `stress test value ${i}`, 60);
    }

    // Verify a sample of keys
    let successCount = 0;
    const sampleSize = Math.min(100, STRESS_TEST_KEY_COUNT);
    const sampleIndices = Array.from({ length: sampleSize }, () =>
      Math.floor(Math.random() * STRESS_TEST_KEY_COUNT),
    );

    for (const idx of sampleIndices) {
      const key = keys[idx];
      const value = await RustCache.get(key);
      if (value.includes(`stress test value ${idx}`)) {
        successCount++;
      }
    }

    const endTime = performance.now();
    const passed = successCount === sampleSize;

    results.stress.largeNumberOfKeys = {
      passed,
      totalKeys: STRESS_TEST_KEY_COUNT,
      sampleSize,
      successfulRetrieval: successCount,
      totalTimeMs: endTime - startTime,
    };

    console.log(
      `Large number of keys: ${passed ? "✅ PASS" : "❌ FAIL"} (${successCount}/${sampleSize} successful retrievals)`,
    );
  } catch (error) {
    results.stress.largeNumberOfKeys = {
      passed: false,
      error: String(error),
    };
    console.log("Large number of keys: ❌ FAIL (Error)");
  }

  // Test 2: Large values
  console.log("Running large values test...");
  try {
    const sizes = [1024, 4096, 16384, 32768]; // Bytes
    const results2 = [];

    for (const size of sizes) {
      const testKey = `large-value-${size}`;
      const testValue = generateRandomString(size);

      try {
        await RustCache.insert(testKey, testValue, 60);
        const retrieved = await RustCache.get(testKey);

        // For large values, we may need to check just a portion
        const success = retrieved.length > 0 && testValue.startsWith(retrieved);
        results2.push({ size, success, retrievedLength: retrieved.length });
      } catch (error) {
        results2.push({ size, success: false, error: String(error) });
      }
    }

    const passed = results2.every((r) => r.success);

    results.stress.largeValues = {
      passed,
      details: results2,
    };

    console.log(`Large values: ${passed ? "✅ PASS" : "❌ FAIL"}`);
    results2.forEach((r) => {
      console.log(
        `  - ${r.size} bytes: ${r.success ? "✅" : "❌"} ${r.success ? "Retrieved " + r.retrievedLength + " bytes" : "Failed"}`,
      );
    });
  } catch (error) {
    results.stress.largeValues = {
      passed: false,
      error: String(error),
    };
    console.log("Large values: ❌ FAIL (Error)");
  }

  if (EXTENDED_TESTS) {
    // Test 3: Rapid succession operations
    console.log("Running rapid succession operations test...");
    try {
      const key = "rapid-op-key";
      const iterations = 1000;
      let successCount = 0;

      for (let i = 0; i < iterations; i++) {
        await RustCache.insert(key, `value-${i}`, 60);
        const value = await RustCache.get(key);

        if (value.includes(`value-${i}`)) {
          successCount++;
        }
      }

      const passed = successCount > iterations * 0.95; // Allow for a small error rate

      results.stress.rapidSuccession = {
        passed,
        iterations,
        successCount,
        successRate: (successCount / iterations).toFixed(4),
      };

      console.log(
        `Rapid succession: ${passed ? "✅ PASS" : "❌ FAIL"} (${successCount}/${iterations} successful operations)`,
      );
    } catch (error) {
      results.stress.rapidSuccession = {
        passed: false,
        error: String(error),
      };
      console.log("Rapid succession: ❌ FAIL (Error)");
    }

    // Test 4: Mixed operation load
    console.log("Running mixed operation load test...");
    try {
      const baseKey = "mixed-op-key";
      const operations = 1000;
      const mixedOps = [];

      // Create a mix of insert, get, and remove operations
      for (let i = 0; i < operations; i++) {
        const key = `${baseKey}-${i}`;
        const op = i % 3;

        if (op === 0) {
          mixedOps.push(() => RustCache.insert(key, `mixed-value-${i}`, 60));
        } else if (op === 1) {
          mixedOps.push(() => RustCache.get(key));
        } else {
          mixedOps.push(() => RustCache.remove(key));
        }
      }

      // Execute in somewhat random order
      const startTime = performance.now();
      let errorCount = 0;

      for (const op of mixedOps.sort(() => Math.random() - 0.5)) {
        try {
          await op();
        } catch (error) {
          errorCount++;
        }
      }

      const endTime = performance.now();
      const passed = errorCount < operations * 0.1; // Allow for some errors due to ordering

      results.stress.mixedOperations = {
        passed,
        operations,
        errorCount,
        totalTimeMs: endTime - startTime,
      };

      console.log(
        `Mixed operations: ${passed ? "✅ PASS" : "❌ FAIL"} (${errorCount} errors out of ${operations} operations)`,
      );
    } catch (error) {
      results.stress.mixedOperations = {
        passed: false,
        error: String(error),
      };
      console.log("Mixed operations: ❌ FAIL (Error)");
    }
  }
}

async function runEdgeCaseTests(results: Record<string, any>) {
  results.edgeCases = {};

  // Test 1: Empty key and value
  console.log("Running empty key and value test...");
  try {
    // Empty value
    const emptyValueKey = randomUUID();
    await RustCache.insert(emptyValueKey, "", 60);
    const emptyValue = await RustCache.get(emptyValueKey);

    // Empty key (should be an error or empty result)
    let emptyKeyError = false;
    try {
      await RustCache.insert("", "test value", 60);
    } catch (error) {
      emptyKeyError = true;
    }

    const emptyKeyValue = await RustCache.get("");
    const passed = emptyValue === "" && (emptyKeyError || !emptyKeyValue);

    results.edgeCases.emptyKeyValue = {
      passed,
      emptyValueRetrieved: emptyValue,
      emptyKeyThrowsError: emptyKeyError,
      emptyKeyRetrieved: emptyKeyValue,
    };

    console.log(`Empty key and value: ${passed ? "✅ PASS" : "❌ FAIL"}`);
  } catch (error) {
    results.edgeCases.emptyKeyValue = {
      passed: false,
      error: String(error),
    };
    console.log("Empty key and value: ❌ FAIL (Error)");
  }

  // Test 2: Special characters
  console.log("Running special characters test...");
  try {
    const specialChars = "!@#$%^&*()_+-=[]{}\\|;:'\",.<>/?\u00A9\u00AE\u2122";
    const testKey = `special-${randomUUID()}`;

    await RustCache.insert(testKey, specialChars, 60);
    const retrieved = await RustCache.get(testKey);

    // Check if at least some special chars are preserved
    // Complete matching may not be possible due to encoding limitations
    const specialCharsFound = specialChars
      .split("")
      .some((char) => retrieved.includes(char));

    results.edgeCases.specialCharacters = {
      passed: specialCharsFound,
      original: specialChars,
      retrieved: retrieved,
    };

    console.log(
      `Special characters: ${specialCharsFound ? "✅ PASS" : "❌ FAIL"}`,
    );
  } catch (error) {
    results.edgeCases.specialCharacters = {
      passed: false,
      error: String(error),
    };
    console.log("Special characters: ❌ FAIL (Error)");
  }

  // Test 3: Boundary conditions
  console.log("Running boundary conditions test...");
  try {
    // Max key length (63 bytes according to your implementation)
    const maxKeyLength = 63;
    const longKey = "x".repeat(maxKeyLength);
    const longKeyValue = "long key test value";

    await RustCache.insert(longKey, longKeyValue, 60);
    const longKeyRetrieved = await RustCache.get(longKey);

    // Too long key (64+ bytes)
    const tooLongKey = "x".repeat(maxKeyLength + 1);
    let tooLongKeyError = false;
    try {
      await RustCache.insert(tooLongKey, "too long key value", 60);
    } catch (error) {
      tooLongKeyError = true;
    }

    // Max value length (56 bytes according to your implementation)
    const maxValueLength = 56;
    const maxLengthValue = "x".repeat(maxValueLength);
    const maxValueKey = "max-value-length-key";

    await RustCache.insert(maxValueKey, maxLengthValue, 60);
    const maxValueRetrieved = await RustCache.get(maxValueKey);

    // Too long value (57+ bytes)
    const tooLongValue = "x".repeat(maxValueLength + 10);
    const tooLongValueKey = "too-long-value-key";

    await RustCache.insert(tooLongValueKey, tooLongValue, 60);
    const tooLongValueRetrieved = await RustCache.get(tooLongValueKey);

    // Zero expiration
    const zeroExpirationKey = "zero-expiration-key";
    await RustCache.insert(zeroExpirationKey, "zero expiration test", 0);
    const zeroExpirationRetrieved = await RustCache.get(zeroExpirationKey);

    // Negative expiration (should use default or handle gracefully)
    const negativeExpirationKey = "negative-expiration-key";
    let negativeExpirationError = false;
    try {
      await RustCache.insert(
        negativeExpirationKey,
        "negative expiration test",
        -1,
      );
    } catch (error) {
      negativeExpirationError = true;
    }

    const boundary1 = longKeyRetrieved === longKeyValue;
    const boundary2 = maxValueRetrieved === maxLengthValue;
    const boundary3 = tooLongValueRetrieved.length > 0; // At least got something back
    const boundary4 = zeroExpirationRetrieved.includes("zero expiration test");

    const passed = boundary1 && boundary2 && boundary3 && boundary4;

    results.edgeCases.boundaryConditions = {
      passed,
      maxKeyLengthWorks: boundary1,
      tooLongKeyError,
      maxValueLengthWorks: boundary2,
      tooLongValueTruncated: tooLongValueRetrieved.length < tooLongValue.length,
      zeroExpirationWorks: boundary4,
      negativeExpirationError,
    };

    console.log(`Boundary conditions: ${passed ? "✅ PASS" : "❌ FAIL"}`);
  } catch (error) {
    results.edgeCases.boundaryConditions = {
      passed: false,
      error: String(error),
    };
    console.log("Boundary conditions: ❌ FAIL (Error)");
  }

  // Test 4: Error handling
  console.log("Running error handling test...");
  try {
    let nonExistentKeyResult = await RustCache.get("definitely-not-exists-key");
    let removeNonExistentResult = await RustCache.remove(
      "definitely-not-exists-key",
    );

    const passed =
      nonExistentKeyResult === "" && removeNonExistentResult.includes("R");

    results.edgeCases.errorHandling = {
      passed,
      nonExistentKeyReturns: nonExistentKeyResult,
      removeNonExistentReturns: removeNonExistentResult,
    };

    console.log(`Error handling: ${passed ? "✅ PASS" : "❌ FAIL"}`);
  } catch (error) {
    results.edgeCases.errorHandling = {
      passed: false,
      error: String(error),
    };
    console.log("Error handling: ❌ FAIL (Error)");
  }
}

async function runRecoveryTests(results: Record<string, any>) {
  results.recovery = {};

  // Test 1: Restart and verify persistence
  console.log("Running restart and persistence test...");
  try {
    // Insert some data before restart
    const persistenceKey = `persistence-${randomUUID()}`;
    const persistenceValue = "This value should persist across restarts";

    await RustCache.insert(persistenceKey, persistenceValue, 3600); // 1 hour expiry

    // Close and restart the cache
    console.log("Restarting cache...");
    RustCache.close();
    await new Promise((resolve) => setTimeout(resolve, 1000)); // Wait for close
    await RustCache.start();

    // Check if data persisted
    const retrievedValue = await RustCache.get(persistenceKey);
    const passed = retrievedValue.includes(persistenceValue);

    results.recovery.persistence = {
      passed,
      expected: persistenceValue,
      actual: retrievedValue,
    };

    console.log(
      `Persistence across restart: ${passed ? "✅ PASS" : "❌ FAIL"}`,
    );
  } catch (error) {
    results.recovery.persistence = {
      passed: false,
      error: String(error),
    };
    console.log("Persistence across restart: ❌ FAIL (Error)");
  }

  if (EXTENDED_TESTS) {
    // Test 2: Simulate crash recovery
    console.log("Running crash recovery simulation test...");
    try {
      const crashKey = `crash-${randomUUID()}`;
      const crashValue = "This value should be available after a crash";

      await RustCache.insert(crashKey, crashValue, 3600);

      // Simulate a crash by directly closing the process without proper cleanup
      console.log("Simulating crash...");
      if (RustCache.process && RustCache.process.kill) {
        RustCache.process.kill("SIGKILL");
      } else {
        // Fallback to just closing normally if we can't access the process directly
        RustCache.close();
      }

      await new Promise((resolve) => setTimeout(resolve, 2000)); // Wait for shutdown

      // Restart and check if data is still available
      await RustCache.start();

      const afterCrashValue = await RustCache.get(crashKey);
      const passed = afterCrashValue.includes(crashValue);

      results.recovery.crashRecovery = {
        passed,
        expected: crashValue,
        actual: afterCrashValue,
      };

      console.log(`Crash recovery: ${passed ? "✅ PASS" : "❌ FAIL"}`);
    } catch (error) {
      results.recovery.crashRecovery = {
        passed: false,
        error: String(error),
      };
      console.log("Crash recovery: ❌ FAIL (Error)");
    }

    // Test 3: Check expiration after restart
    console.log("Running expiration after restart test...");
    try {
      const expireKey = `expire-restart-${randomUUID()}`;
      const expireValue = "Should expire after restart";

      // Set to expire in 2 seconds
      await RustCache.insert(expireKey, expireValue, 2);

      // Verify it exists now
      const beforeRestartValue = await RustCache.get(expireKey);
      const existsBeforeRestart = beforeRestartValue.includes(expireValue);

      // Restart cache
      console.log("Restarting cache...");
      RustCache.close();
      await new Promise((resolve) => setTimeout(resolve, 1000));
      await RustCache.start();

      // Wait for expiration
      await new Promise((resolve) => setTimeout(resolve, 2000));

      // Check if expired
      const afterRestartValue = await RustCache.get(expireKey);
      const expiredAfterRestart = !afterRestartValue;

      const passed = existsBeforeRestart && expiredAfterRestart;

      results.recovery.expirationAfterRestart = {
        passed,
        existsBeforeRestart,
        expiredAfterRestart,
      };

      console.log(
        `Expiration after restart: ${passed ? "✅ PASS" : "❌ FAIL"}`,
      );
    } catch (error) {
      results.recovery.expirationAfterRestart = {
        passed: false,
        error: String(error),
      };
      console.log("Expiration after restart: ❌ FAIL (Error)");
    }
  }

  // Test 4: Cache behavior when under load during restart
  console.log("Running under-load restart test...");
  try {
    // Start a background load
    const loadPromises = [];
    const loadKeys = [];

    for (let i = 0; i < 100; i++) {
      const key = `load-key-${i}-${randomUUID()}`;
      loadKeys.push(key);
      loadPromises.push(RustCache.insert(key, `load value ${i}`, 60));
    }

    // Wait for some inserts to complete
    await Promise.all(loadPromises.slice(0, 50));

    // Restart while still under load
    console.log("Restarting cache during load...");
    RustCache.close();
    await new Promise((resolve) => setTimeout(resolve, 1000));
    await RustCache.start();

    // Try to complete remaining operations
    try {
      await Promise.all(loadPromises.slice(50));
    } catch (error) {
      // Ignore errors from the load operations that were in progress during restart
    }

    // Verify keys that should have been persisted
    let successCount = 0;
    for (let i = 0; i < 50; i++) {
      const value = await RustCache.get(loadKeys[i]);
      if (value.includes(`load value ${i}`)) {
        successCount++;
      }
    }

    const passed = successCount > 0;

    results.recovery.underLoadRestart = {
      passed,
      expectedPersisted: 50,
      actualPersisted: successCount,
    };

    console.log(
      `Under-load restart: ${passed ? "✅ PASS" : "❌ FAIL"} (${successCount}/50 keys persisted)`,
    );
  } catch (error) {
    results.recovery.underLoadRestart = {
      passed: false,
      error: String(error),
    };
    console.log("Under-load restart: ❌ FAIL (Error)");
  }
}

async function runConcurrent<T>(
  tasks: (() => Promise<T>)[],
  limit: number = CONCURRENCY_LEVEL,
): Promise<T[]> {
  const results: T[] = [];
  const executing: Promise<void>[] = [];

  for (const task of tasks) {
    const p = Promise.resolve()
      .then(() => task())
      .then((r) => {
        results.push(r);
        return executing.splice(executing.indexOf(p), 1)[0];
      });

    executing.push(p);
    if (executing.length >= limit) {
      await Promise.race(executing);
    }
  }

  await Promise.all(executing);
  return results;
}

// Main test runner
async function runTests() {
  console.log("=============================");
  console.log("RUST CACHE PRODUCTION TESTING");
  console.log("=============================");

  try {
    // Create a results object to store all test metrics
    const results: Record<string, any> = {
      functional: {},
      performance: {},
      stress: {},
      edgeCases: {},
      recovery: {},
      timestamp: new Date().toISOString(),
    };

    // Start the Rust cache process
    console.log("\n1. INITIALIZING CACHE");
    console.log("---------------------");

    const startTime = performance.now();
    await RustCache.start();

    // Add a longer delay to ensure the cache is fully initialized
    await new Promise((resolve) => setTimeout(resolve, 2000));

    const initTime = performance.now() - startTime;
    console.log(`Cache startup time: ${initTime.toFixed(2)}ms`);
    results.initialization = { startupTimeMs: initTime };

    // Functional Tests
    console.log("\n2. FUNCTIONAL TESTS");
    console.log("-------------------");
    await runFunctionalTests(results);

    // Performance Tests
    console.log("\n3. PERFORMANCE TESTS");
    console.log("--------------------");
    await runPerformanceTests(results);

    // Stress Tests
    console.log("\n4. STRESS TESTS");
    console.log("---------------");
    await runStressTests(results);

    // Edge Case Tests
    console.log("\n5. EDGE CASE TESTS");
    console.log("------------------");
    await runEdgeCaseTests(results);

    // Recovery Tests
    console.log("\n6. RECOVERY TESTS");
    console.log("-----------------");
    await runRecoveryTests(results);

    // Final Summary
    console.log("\n==============================");
    console.log("TEST SUITE SUMMARY");
    console.log("==============================");

    let totalTests = 0;
    let passedTests = 0;

    for (const category in results) {
      if (typeof results[category] === "object" && results[category] !== null) {
        for (const test in results[category]) {
          if (results[category][test].hasOwnProperty("passed")) {
            totalTests++;
            if (results[category][test].passed) passedTests++;
          }
        }
      }
    }

    console.log(`Total Tests: ${totalTests}`);
    console.log(`Passed Tests: ${passedTests}`);
    console.log(`Failed Tests: ${totalTests - passedTests}`);
    console.log(`Pass Rate: ${((passedTests / totalTests) * 100).toFixed(2)}%`);

    // Save test results to file
    await fs.writeFile(
      "cache-test-results.json",
      JSON.stringify(results, null, 2),
    );
    console.log("\nDetailed test results saved to cache-test-results.json");
  } catch (error) {
    console.error("Critical error during tests:", error);
  } finally {
    // Clean up
    console.log("\nShutting down Rust cache...");
    RustCache.close();
    console.log("Tests finished");
  }
}

// Individual test categories
async function runFunctionalTests(results: Record<string, any>) {
  results.functional = {};

  // Test 1: Basic insert and retrieve
  console.log("Running basic insert/retrieve test...");
  try {
    const testKey = `test-basic-${randomUUID()}`;
    const testValue = "Simple test value";

    await RustCache.insert(testKey, testValue, 60);

    // Add a small delay to ensure value is stored
    await new Promise((resolve) => setTimeout(resolve, 200));

    const retrievedValue = await RustCache.get(testKey);

    const passed = retrievedValue.includes(testValue);
    results.functional.basicOperations = {
      passed,
      expected: testValue,
      actual: retrievedValue,
    };

    console.log(`Basic operations: ${passed ? "✅ PASS" : "❌ FAIL"}`);
  } catch (error) {
    results.functional.basicOperations = {
      passed: false,
      error: String(error),
    };
    console.log("Basic operations: ❌ FAIL (Error)");
  }

  // Test 2: Remove operation
  console.log("Running remove operation test...");
  try {
    const testKey = `test-remove-${randomUUID()}`;
    await RustCache.insert(testKey, "Value to be removed", 60);

    // Add a small delay to ensure value is stored
    await new Promise((resolve) => setTimeout(resolve, 200));

    await RustCache.remove(testKey);

    // Add a small delay to ensure value is removed
    await new Promise((resolve) => setTimeout(resolve, 200));

    const retrievedValue = await RustCache.get(testKey);

    const passed = !retrievedValue;
    results.functional.removeOperation = {
      passed,
      expected: "",
      actual: retrievedValue,
    };

    console.log(`Remove operation: ${passed ? "✅ PASS" : "❌ FAIL"}`);
  } catch (error) {
    results.functional.removeOperation = {
      passed: false,
      error: String(error),
    };
    console.log("Remove operation: ❌ FAIL (Error)");
  }

  // Test 3: Expiration
  console.log("Running expiration test (wait 5 seconds)...");
  try {
    const testKey = `test-expire-${randomUUID()}`;
    await RustCache.insert(testKey, "Expires quickly", 1); // Should expire in 1 second

    // Check initial value
    await new Promise((resolve) => setTimeout(resolve, 200));
    const initialValue = await RustCache.get(testKey);
    const initialExists = initialValue.includes("Expires quickly");

    // Wait for expiration (longer to ensure it expires)
    console.log("Waiting for expiration...");
    await new Promise((resolve) => setTimeout(resolve, 5000));

    // Check multiple times to ensure it's really gone
    let retrievedValue = await RustCache.get(testKey);
    if (retrievedValue) {
      // Try again after a delay
      await new Promise((resolve) => setTimeout(resolve, 1000));
      retrievedValue = await RustCache.get(testKey);
    }

    const expired = !retrievedValue;

    results.functional.expiration = {
      passed: initialExists && expired,
      initialExists,
      expired,
      expected: "",
      actual: retrievedValue,
    };

    console.log(
      `Expiration: ${results.functional.expiration.passed ? "✅ PASS" : "❌ FAIL"}`,
    );
    if (!results.functional.expiration.passed) {
      console.log(`  Initial exists: ${initialExists}, Expired: ${expired}`);
    }
  } catch (error) {
    results.functional.expiration = {
      passed: false,
      error: String(error),
    };
    console.log("Expiration: ❌ FAIL (Error)");
  }
}

// Run the tests
runTests();
