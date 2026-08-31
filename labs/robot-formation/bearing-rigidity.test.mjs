import assert from "node:assert/strict";
import test from "node:test";

import {
  analyzeBearingArrows,
  reconstructShape,
  selectBearingArrows,
} from "./bearing-rigidity.mjs";

function normalizedShapeError(expected, actual) {
  const center = (points) => {
    const mean = points.reduce((sum, point) => [sum[0] + point[0], sum[1] + point[1]], [0, 0])
      .map((value) => value / points.length);
    return points.map((point) => [point[0] - mean[0], point[1] - mean[1]]);
  };
  const target = center(expected);
  const found = center(actual);
  const scale = target.reduce((sum, point) => sum + point[0] ** 2 + point[1] ** 2, 0)
    / target.reduce((sum, point, index) =>
      sum + point[0] * found[index][0] + point[1] * found[index][1], 0);
  return Math.sqrt(target.reduce((sum, point, index) => {
    const dx = point[0] - found[index][0] * scale;
    const dy = point[1] - found[index][1] * scale;
    return sum + dx * dx + dy * dy;
  }, 0) / target.length);
}

test("empty and singleton teams need no arrows", () => {
  for (const positions of [[], [[4, -2]]]) {
    const result = selectBearingArrows(positions);
    assert.equal(result.recoverable, true);
    assert.equal(result.arrowCount, 0);
    assert.equal(result.minimumProven, true);
  }
});

test("a triangle requires all three bearing arrows", () => {
  const result = selectBearingArrows([[0, 0], [4, 0], [1, 3]]);
  assert.equal(result.requiredRank, 3);
  assert.equal(result.arrowCount, 3);
  assert.equal(result.minimumProven, true);
  assert.equal(result.robustness.rank, 3);
  assert.equal(result.selectionMethod, "exact-e-optimal");
});

test("square selection is the globally strongest five-arrow basis", () => {
  const positions = [[0, 0], [2, 0], [2, 2], [0, 2]];
  const result = selectBearingArrows(positions);
  assert.equal(result.arrowCount, 5);
  assert.equal(result.combinationsEvaluated, 6);
  assert.equal(result.robustnessOptimality, "global among all minimum-cardinality subsets");

  const selected = analyzeBearingArrows(positions, result.arrows);
  assert.equal(selected.recoverable, true);
  const omittedPair = new Set(result.arrows.map(({ from, to }) => `${Math.min(from, to)}:${Math.max(from, to)}`));
  const all = [];
  for (let from = 0; from < 4; from++) {
    for (let to = from + 1; to < 4; to++) all.push({ from, to });
  }
  for (const omitted of all) {
    const alternative = all.filter((edge) => edge !== omitted);
    const metrics = analyzeBearingArrows(positions, alternative);
    assert.ok(selected.lambdaMin >= metrics.lambdaMin - 1e-12);
  }
  assert.equal(omittedPair.size, 5);
});

test("selection is invariant to candidate order and arrow direction", () => {
  const positions = [[0, 0], [3, 0], [4, 2], [1, 4], [-1, 2]];
  const candidates = [];
  for (let from = 0; from < positions.length; from++) {
    for (let to = from + 1; to < positions.length; to++) candidates.push({ from, to });
  }
  const forward = selectBearingArrows(positions, { candidates });
  const reversed = selectBearingArrows(positions, {
    candidates: candidates.slice().reverse().map(({ from, to }) => ({ from: to, to: from })),
  });
  const pairs = (result) => result.arrows
    .map(({ from, to }) => `${Math.min(from, to)}:${Math.max(from, to)}`)
    .sort();
  assert.deepEqual(pairs(forward), pairs(reversed));
  assert.ok(Math.abs(forward.robustness.lambdaMin - reversed.robustness.lambdaMin) < 1e-12);
});

test("selection and robustness are invariant to translation and scale", () => {
  const positions = [[0, 0], [3, 0], [4, 2], [1, 4], [-1, 2]];
  const transform = (scale) => positions.map(([x, y]) => [x * scale + 7, y * scale - 11]);
  const pairKeys = (result) => result.arrows.map(({ from, to }) => `${from}:${to}`).sort();
  const baseline = selectBearingArrows(positions);
  for (const scale of [1e-10, 1e8]) {
    const transformed = selectBearingArrows(transform(scale));
    assert.equal(transformed.recoverable, true);
    assert.deepEqual(pairKeys(transformed), pairKeys(baseline));
    assert.ok(Math.abs(transformed.robustness.lambdaMin / baseline.robustness.lambdaMin - 1) < 1e-7);
  }
});

test("weights steer the robust choice without changing the proven minimum", () => {
  const positions = [[0, 0], [3, 0], [3, 2], [0, 2]];
  const candidates = [];
  for (let from = 0; from < 4; from++) {
    for (let to = from + 1; to < 4; to++) {
      candidates.push({ from, to, weight: from === 0 && to === 2 ? 100 : 1 });
    }
  }
  const result = selectBearingArrows(positions, { candidates });
  assert.equal(result.arrowCount, 5);
  assert.ok(result.arrows.some(({ from, to }) => from === 0 && to === 2));
});

test("disconnected candidate graph is reported rather than approximated", () => {
  const result = selectBearingArrows(
    [[0, 0], [2, 0], [0, 2], [2, 2]],
    { candidates: [[0, 1], [2, 3]] },
  );
  assert.equal(result.recoverable, false);
  assert.ok(result.maximumRank < result.requiredRank);
  assert.match(result.reason, /do not span/);
});

test("collinear formations are correctly recognized as bearing-degenerate", () => {
  const result = selectBearingArrows([[0, 0], [1, 0], [3, 0], [6, 0]]);
  assert.equal(result.recoverable, false);
  assert.equal(result.requiredRank, 5);
  assert.ok(result.maximumRank < 5);
});

test("coincident candidate endpoints are excluded with diagnostics", () => {
  const result = selectBearingArrows([[0, 0], [0, 0], [2, 1]]);
  assert.equal(result.recoverable, false);
  assert.equal(result.unusableCandidateCount, 1);
});

test("invalid arrow sets fail loudly", () => {
  const positions = [[0, 0], [1, 0], [0, 1]];
  assert.throws(() => selectBearingArrows(positions, { candidates: [[0, 0]] }), /self-arrow/);
  assert.throws(
    () => selectBearingArrows(positions, { candidates: [[0, 1], [1, 0]] }),
    /duplicates/,
  );
  assert.throws(
    () => selectBearingArrows(positions, { candidates: [{ from: 0, to: 1, weight: 0 }] }),
    /positive/,
  );
  assert.throws(
    () => selectBearingArrows(positions, {
      candidates: [{ from: 0, to: 1, noiseStdDev: -1 }],
    }),
    /noiseStdDev must be positive/,
  );
  assert.throws(() => analyzeBearingArrows(positions, [], { rankTolerance: 0 }), /positive/);
  assert.throws(
    () => reconstructShape(3, [], { rankTolerance: Number.NaN }),
    /must be finite/,
  );
});

test("larger formations use a deterministic minimum-cardinality local optimum", () => {
  const positions = Array.from({ length: 8 }, (_, index) => {
    const angle = index * 0.71;
    return [Math.cos(angle) * (3 + index / 4), Math.sin(angle) * (2 + index / 5)];
  });
  const first = selectBearingArrows(positions, { exactCombinationLimit: 100 });
  const second = selectBearingArrows(positions, { exactCombinationLimit: 100 });
  assert.equal(first.selectionMethod, "rank-pivot-plus-exchange");
  assert.equal(first.arrowCount, 13);
  assert.equal(first.minimumProven, true);
  assert.deepEqual(first.arrows, second.arrows);
  assert.equal(first.robustness.rank, 13);
  assert.equal(first.exchangeConverged, true);
  assert.equal(first.robustnessOptimality, "one-arrow-exchange local optimum");
});

test("a capped heuristic does not claim local optimality", () => {
  const positions = Array.from({ length: 8 }, (_, index) => [
    Math.cos(index * 0.71) * (3 + index / 4),
    Math.sin(index * 0.71) * (2 + index / 5),
  ]);
  const result = selectBearingArrows(positions, {
    exactCombinationLimit: 100,
    maxExchangePasses: 0,
  });
  assert.equal(result.exchangeConverged, false);
  assert.equal(result.robustnessOptimality, "bounded one-arrow-exchange search");
});

test("exact bearings reconstruct shape up to translation and positive scale", () => {
  const positions = [[-2, -1], [2, -1], [3, 2], [0, 4], [-3, 2]];
  const selection = selectBearingArrows(positions);
  const reconstruction = reconstructShape(positions.length, selection.arrows);
  assert.ok(normalizedShapeError(positions, reconstruction.points) < 1e-7);
  assert.ok(reconstruction.spectralGap > 1e-6);
  for (const arrow of selection.arrows) {
    const dx = reconstruction.points[arrow.to][0] - reconstruction.points[arrow.from][0];
    const dy = reconstruction.points[arrow.to][1] - reconstruction.points[arrow.from][1];
    assert.ok(dx * arrow.bearing[0] + dy * arrow.bearing[1] > 0);
  }
});

test("reconstruction rejects underdetermined and direction-inconsistent bearings", () => {
  assert.throws(() => reconstructShape(3, []), /at least 3/);
  assert.throws(() => reconstructShape(3, [
    { from: 0, to: 1, bearing: [1, 0] },
    { from: 0, to: 2, bearing: [0, 1] },
    { from: 1, to: 2, bearing: [1, -1] },
  ]), /directed bearings are inconsistent/);
});

test("reconstruction remains stable under deterministic bearing noise", () => {
  const positions = [[-3, -1], [1, -2], [4, 0], [3, 4], [-1, 5], [-4, 2]];
  const selection = selectBearingArrows(positions);
  const noisy = selection.arrows.map((arrow, index) => {
    const angle = Math.atan2(arrow.bearing[1], arrow.bearing[0])
      + Math.sin(index * 1.7 + 0.2) * 0.004;
    return { ...arrow, bearing: [Math.cos(angle), Math.sin(angle)] };
  });
  const reconstruction = reconstructShape(positions.length, noisy);
  assert.ok(normalizedShapeError(positions, reconstruction.points) < 0.08);
  assert.ok(reconstruction.spectralGap > reconstruction.residualEigenvalue * 100);
});
