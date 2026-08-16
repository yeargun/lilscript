import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  canonicalCodecMeasurementsForFiles,
  canonicalCodecProvenance,
  requireCanonicalCodecRuntime,
} from "../codec-contract.mjs";
import {
  browserManifestPath,
  FIXTURES,
} from "../../lilastro/scripts/browser-fixtures.mjs";

const labRoot = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(labRoot, "../..");
const fixtureBuild = join(repoRoot, "lilastro/build/browser");
const publicRoot = join(repoRoot, "web/public/motion-lab");
const resultsOut = join(repoRoot, "web/src/motion-lab-results.json");
requireCanonicalCodecRuntime("Motion lab publication");

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const manifest = JSON.parse(readFileSync(browserManifestPath, "utf8"));
const currentCodecs = canonicalCodecProvenance("Motion lab publication report");
assert.equal(manifest.schemaVersion, 1, "Motion browser manifest schema");
assert.deepEqual(
  manifest.codecs,
  currentCodecs,
  "Motion browser manifest scorer",
);
assert.match(manifest.compiler?.sha256 ?? "", /^[a-f0-9]{64}$/u);
assert.match(manifest.config?.sha256 ?? "", /^[a-f0-9]{64}$/u);
assert.equal(manifest.config?.costModel, "brotli", "Motion artifact objective");
assert.deepEqual(
  manifest.fixtures.map(({ id }) => id),
  FIXTURES,
  "Motion browser fixture manifest coverage",
);

function laneJs(laneManifest) {
  const entries = laneManifest?.files ?? [];
  if (entries.length === 0)
    throw new Error("empty Motion fixture manifest lane");
  const paths = entries.map((entry) => resolve(fixtureBuild, entry.path));
  for (const [index, path] of paths.entries()) {
    if (!path.startsWith(`${fixtureBuild}/`)) {
      throw new Error(`Motion manifest path escapes build root: ${path}`);
    }
    const bytes = readFileSync(path);
    assert.equal(bytes.length, entries[index].bytes, `${path} byte count`);
    assert.equal(sha256(bytes), entries[index].sha256, `${path} digest`);
  }
  const javascriptEntries = entries.filter(({ javascript }) => javascript);
  const javascriptPaths = javascriptEntries.map((entry) =>
    resolve(fixtureBuild, entry.path),
  );
  const measurements = canonicalCodecMeasurementsForFiles(
    javascriptPaths,
    "Motion lab publication",
  );
  const chunks = javascriptEntries.map((entry, index) => {
    const measured = measurements[index];
    assert.deepEqual(
      entry.javascript,
      { raw: measured.raw, gzip: measured.gzip, brotli: measured.brotli },
      `${entry.path} canonical sizes`,
    );
    return {
      file: entry.path,
      sha256: entry.sha256,
      ...entry.javascript,
    };
  });
  if (chunks.length === 0)
    throw new Error("Motion fixture lane has no JavaScript");
  return {
    raw: chunks.reduce((sum, chunk) => sum + chunk.raw, 0),
    gzip: chunks.reduce((sum, chunk) => sum + chunk.gzip, 0),
    brotli: chunks.reduce((sum, chunk) => sum + chunk.brotli, 0),
    chunkCount: chunks.length,
    chunks,
  };
}

if (!existsSync(fixtureBuild)) {
  throw new Error(`missing ${fixtureBuild}; build lilastro fixtures first`);
}

rmSync(publicRoot, { recursive: true, force: true });
mkdirSync(publicRoot, { recursive: true });

const examples = [];
for (const id of FIXTURES) {
  const npmDir = join(fixtureBuild, `${id}-npm`);
  const lilDir = join(fixtureBuild, `${id}-lil`);
  if (
    !existsSync(join(npmDir, "index.html")) ||
    !existsSync(join(lilDir, "index.html"))
  ) {
    throw new Error(
      `${id}: both current npm and LilScript fixture builds are required`,
    );
  }
  cpSync(npmDir, join(publicRoot, `${id}-npm`), { recursive: true });
  cpSync(lilDir, join(publicRoot, `${id}-lil`), { recursive: true });
  const fixtureManifest = manifest.fixtures.find(
    (fixture) => fixture.id === id,
  );
  const npm = laneJs(fixtureManifest?.lanes?.npm);
  const lil = laneJs(fixtureManifest?.lanes?.lil);
  examples.push({
    id,
    title: id.replaceAll("-", " "),
    npmUrl: `/motion-lab/${id}-npm/index.html`,
    lilUrl: `/motion-lab/${id}-lil/index.html`,
    npm,
    lil,
    brotliRatio: lil.brotli / npm.brotli,
  });
  console.log(`published ${id}`);
}

const report = {
  schemaVersion: 1,
  metadata: {
    generatedAt: new Date().toISOString(),
    source: "lilastro/build/browser",
    objectiveContract: {
      gateMetric: "brotli",
      matchingArtifactOnly: true,
      crossMetricsAreDiagnostic: ["raw", "gzip"],
      chunkAccounting: "sum-of-independently-compressed-js-chunks",
    },
    codecs: currentCodecs,
    compiler: manifest.compiler,
    config: manifest.config,
    buildManifest: {
      path: "lilastro/build/browser/manifest.json",
      sha256: sha256(readFileSync(browserManifestPath)),
    },
  },
  examples,
  avgBrotliRatio:
    examples.reduce((sum, row) => sum + row.brotliRatio, 0) / examples.length,
  wins: examples.filter((row) => row.brotliRatio < 1).length,
};

writeFileSync(resultsOut, `${JSON.stringify(report, null, 2)}\n`);
console.log(`wrote ${resultsOut} (${examples.length} openable examples)`);
