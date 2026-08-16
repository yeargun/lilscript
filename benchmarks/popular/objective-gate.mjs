export function objectiveSizeGate(row) {
  if (!row.vite || !row.closure || !row.lilscriptVite) return null;
  const metric = row.costModel ?? "brotli";
  const candidate = row.lilscriptVite[metric];
  const baselines = [row.vite[metric], row.closure[metric]];
  if (![candidate, ...baselines].every(Number.isFinite)) return null;
  return candidate <= Math.min(...baselines);
}
