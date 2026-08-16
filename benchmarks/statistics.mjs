const DEFAULT_SAMPLE_COUNT = 401;
const MINIMUM_SAMPLE_COUNT = 201;
const DEFAULT_BOOTSTRAP_SAMPLES = 10000;

function assertFiniteSamples(values, label) {
  if (!Array.isArray(values) || values.length < MINIMUM_SAMPLE_COUNT) {
    throw new Error(
      `${label} needs at least ${MINIMUM_SAMPLE_COUNT} samples; received ${values?.length ?? 0}`,
    );
  }
  for (const value of values) {
    if (!Number.isFinite(value) || value < 0) {
      throw new Error(`${label} contains an invalid sample: ${value}`);
    }
  }
}

export function configuredSampleCount(environment = process.env) {
  const raw = environment.LILSCRIPT_STATISTICAL_SAMPLES;
  const count = raw === undefined ? DEFAULT_SAMPLE_COUNT : Number(raw);
  if (!Number.isSafeInteger(count) || count < MINIMUM_SAMPLE_COUNT) {
    throw new Error(
      `LILSCRIPT_STATISTICAL_SAMPLES must be an integer >= ${MINIMUM_SAMPLE_COUNT}`,
    );
  }
  return count;
}

export function quantile(values, probability) {
  if (values.length === 0 || probability < 0 || probability > 1) {
    throw new Error("quantile needs samples and a probability in [0, 1]");
  }
  const sorted = [...values].sort((left, right) => left - right);
  const position = (sorted.length - 1) * probability;
  const lower = Math.floor(position);
  const upper = Math.ceil(position);
  const weight = position - lower;
  return sorted[lower] * (1 - weight) + sorted[upper] * weight;
}

export function median(values) {
  return quantile(values, 0.5);
}

function ratio(candidate, baseline) {
  if (baseline === 0) return candidate === 0 ? 1 : Number.POSITIVE_INFINITY;
  return candidate / baseline;
}

function seedFor(label) {
  let seed = 0x811c9dc5;
  for (const character of label) {
    seed ^= character.codePointAt(0);
    seed = Math.imul(seed, 0x01000193);
  }
  return seed >>> 0;
}

function randomGenerator(seed) {
  let state = seed || 0x6d2b79f5;
  return () => {
    state = Math.imul(state ^ (state >>> 15), state | 1);
    state ^= state + Math.imul(state ^ (state >>> 7), state | 61);
    return ((state ^ (state >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * Computes deterministic paired-bootstrap upper confidence bounds. Samples at
 * the same index are one alternating benchmark round, so resampling the index
 * preserves round-level machine load instead of pretending every process was
 * measured under unrelated conditions.
 */
export function nonInferiorityStatistics(
  candidate,
  baseline,
  {
    label = "benchmark",
    confidence = 0.95,
    bootstrapSamples = DEFAULT_BOOTSTRAP_SAMPLES,
  } = {},
) {
  assertFiniteSamples(candidate, `${label} candidate`);
  assertFiniteSamples(baseline, `${label} baseline`);
  if (candidate.length !== baseline.length) {
    throw new Error(`${label} needs equal paired sample counts`);
  }
  if (!(confidence > 0.5 && confidence < 1)) {
    throw new Error(`${label} confidence must be between 0.5 and 1`);
  }
  if (!Number.isSafeInteger(bootstrapSamples) || bootstrapSamples < 1000) {
    throw new Error(`${label} needs at least 1000 bootstrap resamples`);
  }

  const candidateMedian = median(candidate);
  const baselineMedian = median(baseline);
  const candidateP95 = quantile(candidate, 0.95);
  const baselineP95 = quantile(baseline, 0.95);
  const medianRatios = [];
  const p95Ratios = [];
  const random = randomGenerator(seedFor(label));
  for (let iteration = 0; iteration < bootstrapSamples; iteration += 1) {
    const candidateResample = [];
    const baselineResample = [];
    for (let index = 0; index < candidate.length; index += 1) {
      const selected = Math.floor(random() * candidate.length);
      candidateResample.push(candidate[selected]);
      baselineResample.push(baseline[selected]);
    }
    medianRatios.push(ratio(median(candidateResample), median(baselineResample)));
    p95Ratios.push(
      ratio(quantile(candidateResample, 0.95), quantile(baselineResample, 0.95)),
    );
  }

  return {
    samples: candidate.length,
    confidence,
    bootstrapSamples,
    candidate: { median: candidateMedian, p95: candidateP95 },
    baseline: { median: baselineMedian, p95: baselineP95 },
    ratio: {
      median: ratio(candidateMedian, baselineMedian),
      p95: ratio(candidateP95, baselineP95),
    },
    upperConfidenceRatio: {
      median: quantile(medianRatios, confidence),
      p95: quantile(p95Ratios, confidence),
    },
  };
}

export function requireNonInferiority(
  candidate,
  baseline,
  { label = "benchmark", maxRatio = 1.03, ...options } = {},
) {
  const statistics = nonInferiorityStatistics(candidate, baseline, { label, ...options });
  const failures = [];
  for (const metric of ["median", "p95"]) {
    const upper = statistics.upperConfidenceRatio[metric];
    if (upper > maxRatio) {
      failures.push(`${metric} upper ratio ${upper.toFixed(4)} > ${maxRatio.toFixed(4)}`);
    }
  }
  if (failures.length > 0) {
    throw new Error(
      `${label} failed statistical non-inferiority: ${failures.join(", ")}; `
      + `candidate median/p95 ${statistics.candidate.median}/${statistics.candidate.p95}, `
      + `baseline ${statistics.baseline.median}/${statistics.baseline.p95}, `
      + `point ratios ${statistics.ratio.median.toFixed(4)}/${statistics.ratio.p95.toFixed(4)}`,
    );
  }
  return { ...statistics, maxRatio };
}
