export function percentageSaved(baseline, result) {
  if (!Number.isFinite(baseline) || baseline <= 0 || !Number.isFinite(result)) return 0;
  return ((baseline - result) / baseline) * 100;
}

function mean(values) {
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

export function summarizeArtifacts(artifacts) {
  if (artifacts.length === 0) return null;
  const rawTotal = artifacts.reduce((sum, artifact) => sum + artifact.raw, 0);
  const gzipTotal = artifacts.reduce((sum, artifact) => sum + artifact.gzip, 0);
  const brotliTotal = artifacts.reduce((sum, artifact) => sum + artifact.brotli, 0);
  return {
    count: artifacts.length,
    meanRaw: mean(artifacts.map((artifact) => artifact.raw)),
    meanGzipReduction: mean(artifacts.map((artifact) => percentageSaved(artifact.raw, artifact.gzip))),
    meanBrotliReduction: mean(artifacts.map((artifact) => percentageSaved(artifact.raw, artifact.brotli))),
    meanBrotliEdge: mean(artifacts.map((artifact) => percentageSaved(artifact.gzip, artifact.brotli))),
    weightedGzipReduction: percentageSaved(rawTotal, gzipTotal),
    weightedBrotliReduction: percentageSaved(rawTotal, brotliTotal),
  };
}

export function formatBytes(value) {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)} MB`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)} kB`;
  return `${Math.round(value)} B`;
}
