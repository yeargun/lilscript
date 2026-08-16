import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const webRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const summary = JSON.parse(
  readFileSync(
    join(webRoot, "../comparison/algorithms/summary.json"),
    "utf8",
  ),
);
const cases = summary.rows.map((row) => {
  const winner = row.winners.brotli11;
  const baseline = row.baselineCandidates.find(
    (candidate) => candidate.id === winner.candidate,
  );
  const lil = row.lilscript.brotli11;
  return {
    id: row.id,
    title: row.title,
    hypothesis: row.hypothesis,
    passed: row.passed,
    tier: row.tier,
    baseline: {
      id: winner.candidate,
      tool: winner.tool,
      raw: baseline.sizes.raw,
      gzip: baseline.sizes.gzip9,
      brotli: baseline.sizes.brotli11,
    },
    lilscript: {
      raw: lil.sizes.raw,
      gzip: lil.sizes.gzip9,
      brotli: lil.sizes.brotli11,
    },
    config: lil.config,
  };
});

const out = {
  generatedAt: new Date().toISOString(),
  gate: summary.gate,
  cases,
};
writeFileSync(
  join(webRoot, "src/algorithm-demo-results.json"),
  `${JSON.stringify(out, null, 2)}\n`,
);
console.log(`wrote ${cases.length} algorithm demo rows`);
