import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  canonicalCodecProvenance,
  canonicalCodecSizesForFile,
  requireExistingLilscriptToolchain,
  requirePairedLilscriptOverrides,
} from "../../benchmarks/codec-contract.mjs";
import {
  assertEffortConfig,
  assertSampledEffortFrontier,
  parseSelectionExplanation,
} from "./contract.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, "../..");
const executableSuffix = process.platform === "win32" ? ".exe" : "";
const context = "optimization-level frontier";
const { compilerOverride, codecOverride } =
  requirePairedLilscriptOverrides(context);
const compiler = compilerOverride
  ? resolve(process.cwd(), compilerOverride)
  : join(repo, `target/release/lilscript${executableSuffix}`);
const codec = codecOverride
  ? resolve(process.cwd(), codecOverride)
  : join(repo, `target/release/lilscript-codec${executableSuffix}`);
const buildRoot = join(repo, "target/comparison-effort");

const levels = [
  { level: 0, retainedCandidateCap: 1 },
  { level: 3, retainedCandidateCap: 16 },
  { level: 6, retainedCandidateCap: 64 },
  { level: 9, retainedCandidateCap: 384 },
  { level: 12, retainedCandidateCap: 768 },
  { level: 15, retainedCandidateCap: 1536 },
];
const cases = [
  { id: "raw-struct", objective: "raw", case: "aggregates/struct-point" },
  { id: "raw-control", objective: "raw", case: "control/if-chain" },
  {
    id: "gzip-shared-helper",
    objective: "gzip",
    case: "functions/identical-helpers",
  },
  { id: "gzip-string-pool", objective: "gzip", case: "strings/pool" },
  {
    id: "brotli-search-pressure",
    objective: "brotli",
    case: "wins/optimizer-pressure",
  },
  {
    id: "brotli-closure",
    objective: "brotli",
    case: "functions/closure-capture",
  },
];

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function run(program, args, options = {}) {
  const result = spawnSync(program, args, {
    cwd: repo,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
    timeout: 180_000,
    ...options,
  });
  if (result.status !== 0) {
    throw new Error(
      `${program} ${args.join(" ")}\n${result.error?.message ?? ""}\n` +
        `${result.stdout ?? ""}${result.stderr ?? ""}`,
    );
  }
  return result;
}

function prepareToolchain() {
  if (compilerOverride) {
    requireExistingLilscriptToolchain(context, compiler, codec);
    return;
  }
  run(process.env.CARGO ?? "cargo", [
    "build",
    "--release",
    "--bin",
    "lilscript",
    "--bin",
    "lilscript-codec",
  ]);
}

function compilerProvenance() {
  if (!existsSync(compiler)) {
    throw new Error(
      `${context}: compiler does not exist after preparation: ${compiler}`,
    );
  }
  const version = run(compiler, ["--version"]).stdout.trim();
  const relativePath = relative(repo, compiler);
  return {
    source: compilerOverride ? "LILSCRIPT" : "cargo-build-release",
    path: relativePath.startsWith("..") ? compiler : relativePath,
    version,
    sha256: sha256(readFileSync(compiler)),
  };
}

function objectiveBytes(sizes, objective) {
  return sizes[objective];
}

function configFor(objective, level) {
  const basePath = join(repo, `comparison/cases/configs/${objective}.toml`);
  const base = readFileSync(basePath, "utf8");
  assertEffortConfig(base, {
    label: basePath,
    objective,
    level: 15,
  });
  const configured = base.replace(
    /^optimization_level\s*=\s*15\s*$/m,
    `optimization_level = ${level}`,
  );
  if (configured === base && level !== 15) {
    throw new Error(`${basePath} does not declare optimization_level = 15`);
  }
  assertEffortConfig(configured, {
    label: `${basePath} at level ${level}`,
    objective,
    level,
  });
  return configured;
}

prepareToolchain();
const compilerIdentity = compilerProvenance();
rmSync(buildRoot, { recursive: true, force: true });
mkdirSync(buildRoot, { recursive: true });

const rows = [];
for (const fixture of cases) {
  const sourceRoot = join(repo, "comparison/cases/canonical", fixture.case);
  const source = join(sourceRoot, "main.lil");
  const reference = join(sourceRoot, "main.js");
  if (!existsSync(source) || !existsSync(reference)) {
    throw new Error(`${fixture.id}: missing paired source under ${sourceRoot}`);
  }
  const sourceDigest = sha256(readFileSync(source));
  const referenceDigest = sha256(readFileSync(reference));
  const expected = run(process.execPath, [reference]).stdout;
  const expectedStdoutDigest = sha256(expected);
  for (const { level, retainedCandidateCap } of levels) {
    const stem = `${fixture.id}-level-${level}`;
    const config = join(buildRoot, `${stem}.toml`);
    const artifact = join(buildRoot, `${stem}.js`);
    const configText = configFor(fixture.objective, level);
    writeFileSync(config, configText);
    const compileStarted = process.hrtime.bigint();
    const compiled = run(compiler, [
      source,
      "--config",
      config,
      "--target",
      "js",
      "--mode",
      "production",
      "--explain",
      "json",
      "-o",
      artifact,
    ]);
    const processWallTimeMicros =
      Number(process.hrtime.bigint() - compileStarted) / 1_000;
    const actual = run(process.execPath, [artifact]).stdout;
    if (actual !== expected) {
      throw new Error(
        `${stem}: semantic mismatch\nexpected:\n${expected}\nactual:\n${actual}`,
      );
    }
    const selection = parseSelectionExplanation(compiled.stderr, stem);
    if (selection.codec !== fixture.objective) {
      throw new Error(
        `${stem}: selected ${selection.codec}, expected ${fixture.objective}`,
      );
    }
    if (selection.candidates_evaluated > retainedCandidateCap) {
      throw new Error(
        `${stem}: evaluated ${selection.candidates_evaluated} candidates ` +
          `above retained level cap ${retainedCandidateCap}`,
      );
    }
    const sizes = canonicalCodecSizesForFile(artifact, stem);
    const selectedBytes = objectiveBytes(sizes, fixture.objective);
    if (selection.transfer_bytes !== selectedBytes) {
      throw new Error(
        `${stem}: compiler selected ${selection.transfer_bytes} bytes, ` +
          `scorer measured ${selectedBytes}`,
      );
    }
    rows.push({
      ...fixture,
      level,
      retainedCandidateCap,
      survivingCandidatesEvaluated: selection.candidates_evaluated,
      compilerReportedWallTimeMicros: selection.compiler_time_micros,
      processWallTimeMicros,
      selectedBytes,
      sizes,
      artifactDigest: sha256(readFileSync(artifact)),
      sourceDigest,
      referenceDigest,
      expectedStdoutDigest,
      configDigest: sha256(configText),
    });
  }
}

const frontiers = [];
for (const fixture of cases) {
  const points = rows.filter((row) => row.id === fixture.id);
  assertSampledEffortFrontier(points, {
    label: fixture.id,
    objective: fixture.objective,
  });
  const baseline = points.find((point) => point.level === 0);
  const maximum = points.find((point) => point.level === 15);
  const bestLowerEffort = points
    .filter((point) => point.level < 15)
    .reduce((best, point) =>
      point.selectedBytes < best.selectedBytes ? point : best,
    );
  frontiers.push({
    ...fixture,
    levelZeroBytes: baseline.selectedBytes,
    levelFifteenBytes: maximum.selectedBytes,
    bestLowerEffort: {
      level: bestLowerEffort.level,
      bytes: bestLowerEffort.selectedBytes,
    },
    maximumRetainsBestLowerEffort:
      maximum.selectedBytes <= bestLowerEffort.selectedBytes,
  });
}

const deterministicRows = rows.map(
  ({ compilerReportedWallTimeMicros: _, processWallTimeMicros: __, ...row }) =>
    row,
);
const deterministicResultsDigest = sha256(
  JSON.stringify({
    levels,
    cases,
    compilerDigest: compilerIdentity.sha256,
    rows: deterministicRows,
  }),
);
if (sha256(readFileSync(compiler)) !== compilerIdentity.sha256) {
  throw new Error(`${context}: compiler binary changed during the run`);
}

const report = {
  schemaVersion: 3,
  contract: {
    levels,
    sampledSelectedObjectiveMustNotRegressFromBestLowerMeasuredLevel: true,
    sampledFixtureCount: cases.length,
    wallTimesAreDiagnosticOnly: true,
    reportedSurvivingCandidateCapsAreHard: true,
    candidateCountMeaning:
      "deduplicated surviving scored artifacts, not total proposals or codec calls",
  },
  codecs: canonicalCodecProvenance(context),
  provenance: {
    compiler: compilerIdentity,
    runnerDigest: sha256(readFileSync(fileURLToPath(import.meta.url))),
    contractDigest: sha256(readFileSync(join(here, "contract.mjs"))),
    deterministicResultsDigest,
    deterministicResultsExclude: [
      "rows[].compilerReportedWallTimeMicros",
      "rows[].processWallTimeMicros",
    ],
  },
  cases,
  frontiers,
  rows,
};
writeFileSync(
  join(buildRoot, "summary.json"),
  `${JSON.stringify(report, null, 2)}\n`,
);
console.log(JSON.stringify({ frontiers, rows: rows.length }, null, 2));
