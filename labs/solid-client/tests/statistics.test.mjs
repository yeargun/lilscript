import { describe, expect, test } from "vitest";
import {
  balancedRandomOrders,
  pairedDifference,
  pairedRatio,
  sampleSummary,
} from "../scripts/statistics.mjs";

describe("benchmark statistics", () => {
  test("randomized complete blocks are reproducible and position-balanced", () => {
    const labels = ["a", "b", "c", "d"];
    const first = balancedRandomOrders(labels, labels.length * 2, "seed");
    const second = balancedRandomOrders(labels, labels.length * 2, "seed");
    expect(first).toEqual(second);
    expect(first).not.toEqual(
      Array.from({ length: labels.length * 2 }, () => labels),
    );
    for (const label of labels) {
      const positions = labels.map(
        (_, position) =>
          first.filter((order) => order[position] === label).length,
      );
      expect(new Set(positions)).toEqual(new Set([2]));
    }
  });

  test("paired ratios expose confidence, direction, and non-inferiority", () => {
    const baseline = [10, 11, 9, 10.5, 9.5, 10.2, 9.8, 10.1];
    const candidate = baseline.map((value) => value * 0.9);
    const result = pairedRatio(baseline, candidate, {
      bootstrapIterations: 2_000,
      seed: "ratio-test",
    });
    expect(result.pointEstimate).toBeCloseTo(0.9, 10);
    expect(result.confidenceInterval.upper95).toBeLessThan(1);
    expect(result.statisticallySuperior).toBe(true);
    expect(result.nonInferior).toBe(true);
  });

  test("paired differences use an absolute noise allowance", () => {
    const result = pairedDifference([100, 110, 90], [108, 116, 99], {
      bootstrapIterations: 2_000,
      nonInferiorityMargin: 16,
      seed: "difference-test",
    });
    expect(result.pointEstimate).toBeCloseTo(23 / 3, 10);
    expect(result.nonInferior).toBe(true);
  });

  test("summaries retain every observation", () => {
    expect(sampleSummary([1, 2, 3, 100])).toMatchObject({
      count: 4,
      minimum: 1,
      maximum: 100,
      median: 2.5,
    });
  });
});
