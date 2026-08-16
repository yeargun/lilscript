import { createHash } from "node:crypto";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import {
  canonicalCodecMeasurementsForFiles,
  canonicalCodecProvenance,
} from "../../benchmarks/codec-contract.mjs";

const comparison = process.argv[2];
if (!comparison) throw new Error("comparison root is required");

const digest = (path) =>
  createHash("sha256").update(readFileSync(path)).digest("hex");
const sizes = (measurement) => ({
  raw: measurement.raw,
  gzip9: measurement.gzip,
  brotli11: measurement.brotli,
});
const codecContract = (codecs) => ({
  implementation: codecs?.implementation,
  schemaVersion: codecs?.schemaVersion,
  gzip9: codecs?.gzip9,
  brotli11: codecs?.brotli11,
  nodeCodecsAreDiagnosticOnly: codecs?.nodeCodecsAreDiagnosticOnly,
});

const failures = [];
let count = 0;
for (const entry of readdirSync(join(comparison, "artifacts"), {
  withFileTypes: true,
})) {
  if (!entry.isDirectory()) continue;
  const root = join(comparison, "artifacts", entry.name);
  const report = JSON.parse(readFileSync(join(root, "report.json"), "utf8"));
  if (report.schemaVersion !== 4) {
    failures.push(`${entry.name}: expected report schema 4`);
    continue;
  }
  const paths = {
    raw: join(root, "lilscript-raw.js"),
    gzip9: join(root, "lilscript-gzip.js"),
    brotli11: join(root, "lilscript-brotli.js"),
    closure: join(root, "closure-advanced.js"),
  };
  let measured;
  try {
    measured = canonicalCodecMeasurementsForFiles(
      Object.values(paths),
      `checked comparison artifact ${entry.name}`,
    );
  } catch (error) {
    failures.push(`${entry.name}: ${error.message}`);
    continue;
  }
  const currentCodecs = canonicalCodecProvenance(
    `checked comparison artifact ${entry.name}`,
  );
  // Checked artifacts retain the scorer binary that originally measured
  // them. Its path/hash/size are necessarily platform-specific, so compare
  // the canonical encoder contract here and independently remeasure every
  // copied byte with this checkout's self-tested scorer below. The original
  // scorer identity remains required provenance; it is not expected to equal
  // a Linux, macOS, or Windows verifier binary.
  if (
    JSON.stringify(codecContract(report.codecs)) !==
    JSON.stringify(codecContract(currentCodecs))
  ) {
    failures.push(
      `${entry.name}: canonical codec contract differs from this checkout`,
    );
  }
  if (
    !/^[a-f0-9]{64}$/u.test(report.codecs?.scorer?.sha256 ?? "") ||
    !Number.isSafeInteger(report.codecs?.scorer?.bytes) ||
    report.codecs.scorer.bytes <= 0 ||
    typeof report.codecs?.scorer?.path !== "string" ||
    report.codecs.scorer.path.length === 0
  ) {
    failures.push(`${entry.name}: incomplete original scorer provenance`);
  }
  for (const [index, metric] of ["raw", "gzip9", "brotli11"].entries()) {
    const artifact = report.lilscriptArtifacts?.[metric];
    const actualSizes = sizes(measured[index]);
    if (artifact?.digest !== digest(paths[metric])) {
      failures.push(`${entry.name}/${metric}: artifact digest mismatch`);
    }
    if (JSON.stringify(artifact?.sizes) !== JSON.stringify(actualSizes)) {
      failures.push(`${entry.name}/${metric}: canonical sizes mismatch`);
    }
    if (report.lilscript?.[metric] !== actualSizes[metric]) {
      failures.push(`${entry.name}/${metric}: objective cell mismatch`);
    }
  }
  const closureSizes = sizes(measured[3]);
  if (report.closureArtifact?.digest !== digest(paths.closure)) {
    failures.push(`${entry.name}/closure: artifact digest mismatch`);
  }
  if (
    JSON.stringify(report.closureArtifact?.sizes) !==
    JSON.stringify(closureSizes)
  ) {
    failures.push(`${entry.name}/closure: canonical sizes mismatch`);
  }
  if (JSON.stringify(report.closure) !== JSON.stringify(closureSizes)) {
    failures.push(`${entry.name}/closure: report cells mismatch`);
  }
  for (const requiredDigest of [
    report.toolVersions?.lilscript?.digest,
    report.toolVersions?.closure?.digest,
    report.provenance?.configs?.raw?.digest,
    report.provenance?.configs?.gzip?.digest,
    report.provenance?.configs?.brotli?.digest,
  ]) {
    if (!/^[a-f0-9]{64}$/u.test(requiredDigest ?? "")) {
      failures.push(`${entry.name}: incomplete tool/config provenance`);
      break;
    }
  }
  count += 1;
}

if (failures.length > 0) {
  throw new Error(
    `checked comparison artifact attestation failed:\n${failures.join("\n")}`,
  );
}
console.log(
  `${count} checked comparison artifact reports are canonically attested.`,
);
