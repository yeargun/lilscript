import assert from "node:assert/strict";

function hashSeed(seed) {
  let hash = 2166136261;
  for (const character of String(seed)) {
    hash ^= character.codePointAt(0);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

export function createRandom(seed) {
  let state = hashSeed(seed) || 0x6d2b79f5;
  return () => {
    state += 0x6d2b79f5;
    let value = state;
    value = Math.imul(value ^ (value >>> 15), value | 1);
    value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
    return ((value ^ (value >>> 14)) >>> 0) / 4_294_967_296;
  };
}

function shuffle(values, random) {
  const result = [...values];
  for (let index = result.length - 1; index > 0; index -= 1) {
    const selected = Math.floor(random() * (index + 1));
    [result[index], result[selected]] = [result[selected], result[index]];
  }
  return result;
}

/**
 * Deterministic randomized complete blocks with position and carryover balance.
 * Every full 2n block cycle contains rotations of a shuffled order and its
 * reverse, so each variant appears equally often in every run position and
 * pairwise precedence is balanced.
 */
export function balancedRandomOrders(labels, blocks, seed) {
  assert.ok(Number.isInteger(blocks) && blocks > 0, "positive block count");
  assert.ok(labels.length > 1, "at least two benchmark variants");
  assert.equal(new Set(labels).size, labels.length, "unique benchmark labels");
  const random = createRandom(seed);
  const orders = [];
  while (orders.length < blocks) {
    const base = shuffle(labels, random);
    const reversed = [...base].reverse();
    const cycle = [];
    for (const source of [base, reversed]) {
      for (let offset = 0; offset < source.length; offset += 1) {
        cycle.push([...source.slice(offset), ...source.slice(0, offset)]);
      }
    }
    for (const order of shuffle(cycle, random)) {
      if (orders.length === blocks) break;
      orders.push(order);
    }
  }
  return orders;
}

export function quantile(values, probability) {
  assert.ok(values.length > 0, "quantile requires samples");
  assert.ok(probability >= 0 && probability <= 1, "valid probability");
  const sorted = [...values].sort((left, right) => left - right);
  const position = (sorted.length - 1) * probability;
  const lower = Math.floor(position);
  const weight = position - lower;
  return sorted[lower + 1] === undefined
    ? sorted[lower]
    : sorted[lower] + weight * (sorted[lower + 1] - sorted[lower]);
}

export function median(values) {
  return quantile(values, 0.5);
}

function mean(values) {
  return values.reduce((total, value) => total + value, 0) / values.length;
}

function standardDeviation(values) {
  if (values.length < 2) return 0;
  const center = mean(values);
  return Math.sqrt(
    values.reduce((total, value) => total + (value - center) ** 2, 0) /
      (values.length - 1),
  );
}

export function sampleSummary(values) {
  assert.ok(values.length > 0, "summary requires samples");
  assert.ok(values.every(Number.isFinite), "summary samples must be finite");
  const center = median(values);
  return {
    count: values.length,
    mean: mean(values),
    median: center,
    standardDeviation: standardDeviation(values),
    medianAbsoluteDeviation: median(
      values.map((value) => Math.abs(value - center)),
    ),
    minimum: Math.min(...values),
    maximum: Math.max(...values),
    q1: quantile(values, 0.25),
    q3: quantile(values, 0.75),
  };
}

function bootstrap(values, iterations, seed, statistic) {
  const random = createRandom(seed);
  const estimates = [];
  for (let iteration = 0; iteration < iterations; iteration += 1) {
    const resample = [];
    for (let index = 0; index < values.length; index += 1) {
      resample.push(values[Math.floor(random() * values.length)]);
    }
    estimates.push(statistic(resample));
  }
  return {
    lower95: quantile(estimates, 0.025),
    upper95: quantile(estimates, 0.975),
  };
}

function pairedPermutationPValue(effects, iterations, seed) {
  const observed = Math.abs(mean(effects));
  const random = createRandom(seed);
  let extreme = 0;
  for (let iteration = 0; iteration < iterations; iteration += 1) {
    const permuted = effects.map((effect) =>
      random() < 0.5 ? effect : -effect,
    );
    if (Math.abs(mean(permuted)) >= observed) extreme += 1;
  }
  return (extreme + 1) / (iterations + 1);
}

function assertPairs(baseline, candidate) {
  assert.equal(candidate.length, baseline.length, "paired sample count");
  assert.ok(baseline.length > 1, "paired comparison requires two samples");
  assert.ok(
    [...baseline, ...candidate].every(Number.isFinite),
    "paired samples must be finite",
  );
}

export function pairedRatio(
  baseline,
  candidate,
  {
    bootstrapIterations = 10_000,
    nonInferiorityMargin = 1.03,
    seed = "paired-ratio",
  } = {},
) {
  assertPairs(baseline, candidate);
  assert.ok(
    baseline.every((value) => value > 0),
    "positive baselines",
  );
  assert.ok(
    candidate.every((value) => value >= 0),
    "non-negative candidates",
  );
  const ratios = candidate.map((value, index) => value / baseline[index]);
  const logRatios = ratios.map((ratio) => Math.log(Math.max(ratio, 1e-300)));
  const pointEstimate = Math.exp(mean(logRatios));
  const confidenceInterval = bootstrap(
    logRatios,
    bootstrapIterations,
    `${seed}:bootstrap`,
    (values) => Math.exp(mean(values)),
  );
  const deviation = standardDeviation(logRatios);
  return {
    method:
      "paired geometric-mean ratio with deterministic percentile bootstrap",
    pointEstimate,
    medianRatio: median(ratios),
    percentChange: (pointEstimate - 1) * 100,
    confidenceInterval,
    confidenceLevel: 0.95,
    bootstrapIterations,
    permutationPValue: pairedPermutationPValue(
      logRatios,
      bootstrapIterations,
      `${seed}:permutation`,
    ),
    pairedLogEffectSize: deviation === 0 ? null : mean(logRatios) / deviation,
    wins: ratios.filter((ratio) => ratio < 1).length,
    ties: ratios.filter((ratio) => ratio === 1).length,
    losses: ratios.filter((ratio) => ratio > 1).length,
    pointEstimateImproved: pointEstimate < 1,
    statisticallySuperior: confidenceInterval.upper95 < 1,
    nonInferiorityMargin,
    nonInferior: confidenceInterval.upper95 <= nonInferiorityMargin,
    ratios,
  };
}

export function pairedDifference(
  baseline,
  candidate,
  {
    bootstrapIterations = 10_000,
    nonInferiorityMargin = 0,
    seed = "paired-difference",
  } = {},
) {
  assertPairs(baseline, candidate);
  const differences = candidate.map((value, index) => value - baseline[index]);
  const pointEstimate = mean(differences);
  const confidenceInterval = bootstrap(
    differences,
    bootstrapIterations,
    `${seed}:bootstrap`,
    mean,
  );
  const deviation = standardDeviation(differences);
  return {
    method: "paired mean difference with deterministic percentile bootstrap",
    pointEstimate,
    medianDifference: median(differences),
    confidenceInterval,
    confidenceLevel: 0.95,
    bootstrapIterations,
    permutationPValue: pairedPermutationPValue(
      differences,
      bootstrapIterations,
      `${seed}:permutation`,
    ),
    pairedEffectSize: deviation === 0 ? null : pointEstimate / deviation,
    wins: differences.filter((value) => value < 0).length,
    ties: differences.filter((value) => value === 0).length,
    losses: differences.filter((value) => value > 0).length,
    pointEstimateImproved: pointEstimate < 0,
    statisticallySuperior: confidenceInterval.upper95 < 0,
    nonInferiorityMargin,
    nonInferior: confidenceInterval.upper95 <= nonInferiorityMargin,
    differences,
  };
}
