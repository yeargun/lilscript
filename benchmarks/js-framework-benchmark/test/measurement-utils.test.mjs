import assert from "node:assert/strict";
import test from "node:test";
import {
  geometricMean,
  measurementKey,
  percentile,
  shuffled,
  summarize,
} from "../scripts/measurement-utils.mjs";

test("seeded schedules are deterministic permutations", () => {
  const values = ["a", "b", "c", "d", "e"];
  const first = shuffled(values, 20260814);
  assert.deepEqual(first, shuffled(values, 20260814));
  assert.deepEqual([...first].sort(), values);
  assert.notDeepEqual(first, shuffled(values, 20260815));
});

test("measurement keys distinguish block, workload, framework, and phase", () => {
  const key = measurementKey("cpu", 3, "01_run1k", "solidlil-v0.1.0-keyed");
  assert.equal(key, "cpu:3:01_run1k:solidlil-v0.1.0-keyed");
});

test("summary statistics and bootstrap intervals are deterministic", () => {
  assert.equal(percentile([1, 2, 3, 4], 0.5), 2.5);
  assert.equal(geometricMean([1, 4]), 2);
  const first = summarize([1, 2, 3, 4, 100], 42, 500);
  const second = summarize([1, 2, 3, 4, 100], 42, 500);
  assert.deepEqual(first, second);
  assert.equal(first.median, 3);
  assert.equal(first.n, 5);
  assert.ok(first.medianCi95[0] <= first.median);
  assert.ok(first.medianCi95[1] >= first.median);
});
