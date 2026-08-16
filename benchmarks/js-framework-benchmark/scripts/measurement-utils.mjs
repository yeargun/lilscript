export function hashString(value) {
  let hash = 2166136261;
  for (const character of value) {
    hash ^= character.codePointAt(0);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

export function seededRandom(seed) {
  let state = seed >>> 0 || 0x6d2b79f5;
  return () => {
    state += 0x6d2b79f5;
    let value = state;
    value = Math.imul(value ^ (value >>> 15), value | 1);
    value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
    return ((value ^ (value >>> 14)) >>> 0) / 4294967296;
  };
}

export function shuffled(values, seed) {
  const result = [...values];
  const random = seededRandom(seed);
  for (let index = result.length - 1; index > 0; index -= 1) {
    const selected = Math.floor(random() * (index + 1));
    [result[index], result[selected]] = [result[selected], result[index]];
  }
  return result;
}

export function measurementKey(phase, block, workload, framework) {
  return `${phase}:${block}:${workload}:${framework}`;
}

export function percentile(values, probability) {
  if (values.length === 0) return null;
  const sorted = [...values].sort((left, right) => left - right);
  const position = (sorted.length - 1) * probability;
  const lower = Math.floor(position);
  const upper = Math.ceil(position);
  if (lower === upper) return sorted[lower];
  return sorted[lower] + (sorted[upper] - sorted[lower]) * (position - lower);
}

export function geometricMean(values) {
  if (values.length === 0 || values.some((value) => value <= 0)) return null;
  return Math.exp(values.reduce((sum, value) => sum + Math.log(value), 0) / values.length);
}

export function summarize(values, seed = 1, bootstrapIterations = 2_000) {
  if (values.length === 0) return null;
  const mean = values.reduce((sum, value) => sum + value, 0) / values.length;
  const variance =
    values.reduce((sum, value) => sum + (value - mean) ** 2, 0) /
    Math.max(1, values.length - 1);
  const median = percentile(values, 0.5);
  const random = seededRandom(seed);
  const bootstrapMedians = [];
  if (values.length > 1) {
    for (let iteration = 0; iteration < bootstrapIterations; iteration += 1) {
      const sample = [];
      for (let index = 0; index < values.length; index += 1) {
        sample.push(values[Math.floor(random() * values.length)]);
      }
      bootstrapMedians.push(percentile(sample, 0.5));
    }
  }
  return {
    n: values.length,
    min: Math.min(...values),
    max: Math.max(...values),
    mean,
    median,
    p95: percentile(values, 0.95),
    standardDeviation: Math.sqrt(variance),
    medianCi95:
      bootstrapMedians.length > 0
        ? [percentile(bootstrapMedians, 0.025), percentile(bootstrapMedians, 0.975)]
        : [median, median],
  };
}
