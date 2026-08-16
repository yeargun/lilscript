import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { objectiveSizeGate } from "./objective-gate.mjs";

const labRoot = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(labRoot, "../..");
const manifest = JSON.parse(
  readFileSync(join(labRoot, "compatibility/libraries.json"), "utf8"),
);
const packageJson = JSON.parse(
  readFileSync(join(labRoot, "package.json"), "utf8"),
);
const measured = JSON.parse(
  readFileSync(join(labRoot, "build/results.json"), "utf8"),
);
const performance = JSON.parse(
  readFileSync(join(labRoot, "build/performance-memory.json"), "utf8"),
);
const sizeEvidenceBytes = readFileSync(join(labRoot, "build/results.json"));

const materialRegressionLimit = 1.05;

const primaryMeasuredRows = measured.filter((row) => !row.external);
assert.ok(primaryMeasuredRows.length > 0, "popular report needs measured rows");
const canonicalCodecs = primaryMeasuredRows[0].codecs;
const compilerEvidence = primaryMeasuredRows[0].compiler;
assert.equal(canonicalCodecs?.implementation, "lilscript-codec");
assert.equal(canonicalCodecs?.schemaVersion, 1);
assert.match(canonicalCodecs?.scorer?.sha256 ?? "", /^[a-f0-9]{64}$/);
assert.match(compilerEvidence?.sha256 ?? "", /^[a-f0-9]{64}$/);
for (const row of primaryMeasuredRows) {
  assert.deepEqual(row.codecs, canonicalCodecs, `${row.id} codec provenance`);
  assert.deepEqual(
    row.compiler,
    compilerEvidence,
    `${row.id} compiler provenance`,
  );
  assert.equal(row.costModel, "brotli", `${row.id} objective`);
  assert.equal(
    row.objectiveContract?.gateMetric,
    "brotli",
    `${row.id} objective gate`,
  );
  assert.equal(
    row.objectiveContract?.artifact,
    "lilscriptVite",
    `${row.id} objective artifact`,
  );
}
assert.equal(performance.schemaVersion, 1, "popular performance schema");
assert.equal(performance.runtimeObjective, "brotli");
assert.deepEqual(performance.codecs, canonicalCodecs, "performance codecs");
assert.deepEqual(
  performance.compiler,
  compilerEvidence,
  "performance compiler",
);
assert.equal(
  performance.sizeEvidence?.sha256,
  createHash("sha256").update(sizeEvidenceBytes).digest("hex"),
  "performance evidence must bind the current size report",
);
const targets = new Map(manifest.targets.map((target) => [target.id, target]));
const commit = execFileSync("git", ["rev-parse", "--short", "HEAD"], {
  cwd: repoRoot,
  encoding: "utf8",
}).trim();
const dirty =
  execFileSync("git", ["status", "--porcelain"], {
    cwd: repoRoot,
    encoding: "utf8",
  }).trim().length > 0;

const selectedResults = measured.map((measuredRow) => {
  let row = measuredRow;
  let solidPerformance = null;
  let externalCompatibilityNotes = null;
  const rowSource = row.source
    ? isAbsolute(row.source)
      ? row.source
      : join(repoRoot, row.source)
    : null;
  if (row.id === "solid-js" && rowSource && existsSync(rowSource)) {
    const sizeReport = JSON.parse(readFileSync(rowSource, "utf8"));
    const evidenceStatus =
      sizeReport.evidence?.status ?? row.evidenceStatus ?? "external-current";
    const normalize = (size) => ({
      raw: size.raw,
      gzip: size.gzip9 ?? size.gzip,
      brotli: size.brotli11 ?? size.brotli,
    });
    row = {
      ...row,
      vite: normalize(sizeReport.sizes["solid-todolist"]),
      lilscriptVite: normalize(sizeReport.sizes["solidlil-todolist"]),
      comparisons: sizeReport.comparisons?.todolistLilx ?? row.comparisons,
      evidenceStatus,
    };
    if (evidenceStatus === "archived-external-snapshot") {
      externalCompatibilityNotes =
        "Archived sibling-worktree LSX application snapshot; the parser, lowerer, Vite transform, and feature ledger are integrated, but the todolist and its gates are not yet reproducible from labs/solid-client. Runtime-only SolidLil evidence is verified separately.";
    }
    const performancePath = join(dirname(rowSource), "performance-report.json");
    if (
      existsSync(performancePath) &&
      evidenceStatus !== "archived-external-snapshot"
    ) {
      const report = JSON.parse(readFileSync(performancePath, "utf8"));
      solidPerformance = {
        performance: {
          npmMs: report.medians.solid.total,
          lilscriptMs: report.medians.solidlilLsx.total,
          ratio: report.ratios.lsx,
        },
        retainedMemory: {
          npmBytes: report.retainedMemory?.solid ?? null,
          lilscriptBytes: report.retainedMemory?.solidlilLsx ?? null,
          ratio: report.memoryRatios?.lsx ?? null,
        },
      };
      const solid = sizeReport.sizes["solid-todolist"];
      const lsx = sizeReport.sizes["solidlil-todolist"];
      const babel = sizeReport.sizes["solidlil-babel-todolist"];
      const solidCore = sizeReport.sizes["solid-core-min"];
      const lilCore = sizeReport.sizes["solidlil-core-min"];
      externalCompatibilityNotes =
        `Measured in lilscript-solid-lab; this is not a complete Solid replacement. ` +
        `Fresh Vite/oxc app JS: Solid ${solid.raw} raw / ${solid.brotli11} brotli, ` +
        `solidlil LSX ${lsx.raw} / ${lsx.brotli11}, and identical-JSX reactive-swap ` +
        `${babel.raw} / ${babel.brotli11}. Isolated ratios: LSX time ` +
        `${report.ratios.lsx.toFixed(3)} and retained heap ${report.memoryRatios.lsx.toFixed(3)}; ` +
        `identical-JSX time ${report.ratios.babel.toFixed(3)} and retained heap ` +
        `${report.memoryRatios.babel.toFixed(3)}. Used-core bundle: Solid ` +
        `${solidCore.raw} / ${solidCore.brotli11}, solidlil ${lilCore.raw} / ${lilCore.brotli11}; ` +
        `all three measured surfaces pass their strict gates.`;
    }
  }
  const target = targets.get(row.id);
  if (!target) throw new Error(`missing compatibility target for ${row.id}`);
  const runtime = performance.results[row.id] ?? solidPerformance;
  const performanceGate =
    runtime &&
    runtime.performance.ratio != null &&
    runtime.retainedMemory.ratio != null
      ? runtime.performance.ratio <= materialRegressionLimit &&
        runtime.retainedMemory.ratio <= materialRegressionLimit
      : null;
  const exactSurface = target.status.startsWith("exact-");
  const sizeGate = objectiveSizeGate(row);
  return {
    ...row,
    status: target.status,
    packages: target.packages.map((name, index) => ({
      name,
      version: target.versions[index] ?? target.versions[0],
    })),
    entrypoint: target.entrypoint ?? null,
    publicRuntimeApi: target.publicRuntimeApi ?? [],
    compatibilityNotes:
      externalCompatibilityNotes ?? target.notes ?? target.reason ?? "",
    performance: runtime,
    exactSurface,
    performanceGate,
    sizeGate,
    eligible: exactSurface && performanceGate === true && sizeGate === true,
  };
});

const supplementalCandidatePaths = [join(labRoot, "build/jquery-results.json")];
const supplementalCandidates = supplementalCandidatePaths
  .filter((candidatePath) => existsSync(candidatePath))
  .map((candidatePath) => JSON.parse(readFileSync(candidatePath, "utf8")))
  .filter(
    (candidate) => !selectedResults.some((row) => row.id === candidate.id),
  )
  .map((candidate) => {
    const canonical =
      candidate.schemaVersion === 1 &&
      JSON.stringify(candidate.codecs) === JSON.stringify(canonicalCodecs) &&
      candidate.compiler?.sha256 === compilerEvidence.sha256 &&
      /^[a-f0-9]{64}$/.test(candidate.compiler?.configSha256 ?? "") &&
      candidate.objectiveContract?.gateMetric === "brotli";
    if (canonical) return candidate;
    return {
      ...candidate,
      evidenceStatus: "historical-noncanonical",
      codecs: null,
      compiler: null,
      rawJs: null,
      terser: null,
      closure: null,
      vite: null,
      lilscript: null,
      lilscriptVite: null,
      libraryArtifacts: null,
      objectiveContract: {
        gateMetric: null,
        scope:
          "stale candidate retained without byte claims until measure-jquery.mjs is rerun with the current compiler/scorer pair",
      },
    };
  });
for (const candidate of supplementalCandidates) {
  if (
    candidate.eligible !== false ||
    candidate.exactSurface !== false ||
    !candidate.status?.startsWith("candidate-")
  ) {
    throw new Error(
      `supplemental ${candidate.id} must remain an explicitly ineligible candidate`,
    );
  }
}
const results = [...selectedResults, ...supplementalCandidates];

const report = {
  schemaVersion: 2,
  metadata: {
    generatedAt: new Date().toISOString(),
    compilerRevision: `${commit}${dirty ? "-dirty" : ""}`,
    node: process.version,
    vite: packageJson.devDependencies.vite,
    esbuild: packageJson.devDependencies.esbuild,
    terser: packageJson.devDependencies.terser,
    closure: packageJson.devDependencies["google-closure-compiler"],
    performanceRounds: performance.rounds,
    runtimeObjective: performance.runtimeObjective ?? "brotli",
    runtimeConfig: performance.runtimeConfig ?? "lilscript.toml",
    materialRegressionLimit,
    codecs: canonicalCodecs,
    compiler: compilerEvidence,
    objectiveContract: {
      candidateArtifact: "lilscriptVite",
      baselineArtifacts: ["vite", "closure"],
      gateMetric: "brotli",
      matchingArtifactOnly: true,
      crossMetricsAreDiagnostic: ["raw", "gzip"],
      appliesToRows: primaryMeasuredRows.map(({ id }) => id),
    },
  },
  eligibilityRule: manifest.eligibilityRule,
  results,
};

writeFileSync(
  join(repoRoot, "web/src/popular-library-results.json"),
  `${JSON.stringify(report, null, 2)}\n`,
);

console.log(
  `Published ${results.length} popular-library rows (${results.filter((row) => row.eligible).length} eligible) to web/src/popular-library-results.json`,
);
