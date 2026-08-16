import {
  canonicalCodecProvenance,
  canonicalCodecSizesForFile,
  requireCanonicalCodecRuntime,
} from "./codec-contract.mjs";

requireCanonicalCodecRuntime("legacy Closure comparison");

function measure(path) {
  return canonicalCodecSizesForFile(path, "legacy Closure comparison artifact");
}

const args = process.argv.slice(2);
const rows = [];
let objectiveGate = false;
if (args[0] === "--objective") {
  const [_, label, rawPath, gzipPath, brotliPath, baselineLabel, baselinePath] = args;
  if (!baselinePath || args.length !== 7) {
    throw new Error("--objective expects label raw.js gzip.js brotli.js baseline-label baseline.js");
  }
  const objectiveArtifacts = {
    raw: measure(rawPath),
    gzip: measure(gzipPath),
    brotli: measure(brotliPath),
  };
  rows.push({
    compiler: `${label} objective builds`,
    raw: objectiveArtifacts.raw.raw,
    gzip: objectiveArtifacts.gzip.gzip,
    brotli: objectiveArtifacts.brotli.brotli,
  });
  rows.push({ compiler: baselineLabel, ...measure(baselinePath) });
  objectiveGate = true;
  console.log(
    "LilScript cells are independent raw/gzip/Brotli objective builds; each cell is comparable only to the matching baseline metric.",
  );
} else {
  for (let index = 0; index < args.length; index += 2) {
    rows.push({ compiler: args[index], ...measure(args[index + 1]) });
  }
}

console.log("| Compiler | Raw | Gzip-9 | Brotli-11 |");
console.log("| --- | ---: | ---: | ---: |");
for (const row of rows) {
  console.log(`| ${row.compiler} | ${row.raw} | ${row.gzip} | ${row.brotli} |`);
}

if (objectiveGate) {
  const [lilscript, baseline] = rows;
  const failures = ["raw", "gzip", "brotli"].filter(
    (metric) => lilscript[metric] > baseline[metric],
  );
  if (failures.length > 0) {
    throw new Error(
      `objective size gate failed: ${failures.map((metric) =>
        `${metric} ${lilscript[metric]} > ${baseline[metric]}`
      ).join(", ")}`,
    );
  }
}

console.log(`Codec provenance: ${JSON.stringify(canonicalCodecProvenance())}`);
