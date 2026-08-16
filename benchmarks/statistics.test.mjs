import assert from "node:assert/strict";
import test from "node:test";
import {
  configuredSampleCount,
  nonInferiorityStatistics,
  quantile,
  requireNonInferiority,
} from "./statistics.mjs";

test("uses a release-grade default and rejects undersampling", () => {
  assert.equal(configuredSampleCount({}), 401);
  assert.equal(configuredSampleCount({ LILSCRIPT_STATISTICAL_SAMPLES: "251" }), 251);
  assert.throws(
    () => configuredSampleCount({ LILSCRIPT_STATISTICAL_SAMPLES: "200" }),
    /integer >= 201/,
  );
});

test("quantiles use interpolation rather than an unstable nearest bucket", () => {
  assert.equal(quantile([0, 10, 20], 0.5), 10);
  assert.equal(quantile([0, 10, 20], 0.95), 19);
});

test("paired bootstrap is deterministic and accepts identical distributions", () => {
  const samples = Array.from({ length: 201 }, (_, index) => 100 + (index % 7));
  const first = requireNonInferiority(samples, samples, { label: "identical" });
  const second = requireNonInferiority(samples, samples, { label: "identical" });
  assert.deepEqual(first, second);
  assert.equal(first.upperConfidenceRatio.median, 1);
  assert.equal(first.upperConfidenceRatio.p95, 1);
});

test("rejects a median regression with a one-sided confidence bound", () => {
  const baseline = Array.from({ length: 201 }, (_, index) => 100 + (index % 3));
  const candidate = baseline.map((value) => value * 1.1);
  assert.throws(
    () => requireNonInferiority(candidate, baseline, { label: "median-regression" }),
    /median upper ratio/,
  );
});

test("the default non-inferiority margin rejects a four-percent regression", () => {
  const baseline = Array.from({ length: 201 }, (_, index) => 100 + (index % 3));
  const candidate = baseline.map((value) => value * 1.04);
  assert.throws(
    () => requireNonInferiority(candidate, baseline, { label: "default-margin" }),
    /1\.0300/,
  );
});

test("rejects a tail regression even when the median is unchanged", () => {
  const baseline = Array.from({ length: 201 }, () => 100);
  const candidate = baseline.map((value, index) => (index >= 185 ? value * 2 : value));
  const statistics = nonInferiorityStatistics(candidate, baseline, { label: "tail" });
  assert.equal(statistics.ratio.median, 1);
  assert.ok(statistics.ratio.p95 > 1);
  assert.throws(
    () => requireNonInferiority(candidate, baseline, { label: "tail" }),
    /p95 upper ratio/,
  );
});
