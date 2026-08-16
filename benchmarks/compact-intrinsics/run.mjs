import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  canonicalCodecProvenance,
  canonicalCodecSizesForFile,
  requireExistingLilscriptToolchain,
  requirePairedLilscriptOverrides,
} from "../codec-contract.mjs";
import {
  configuredSampleCount,
  requireNonInferiority,
} from "../statistics.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "../..");
const build = join(root, "target/compact-intrinsics");
const toolchainContext = "compact intrinsic compiler gate";
const { compilerOverride, codecOverride } =
  requirePairedLilscriptOverrides(toolchainContext);
const executableSuffix = process.platform === "win32" ? ".exe" : "";
const compiler = compilerOverride
  ? resolve(process.cwd(), compilerOverride)
  : join(root, `target/release/lilscript${executableSuffix}`);
const codecScorer = codecOverride
  ? resolve(process.cwd(), codecOverride)
  : join(root, `target/release/lilscript-codec${executableSuffix}`);
const cargo = process.env.CARGO ?? "cargo";

if (compilerOverride) {
  requireExistingLilscriptToolchain(toolchainContext, compiler, codecScorer);
} else if (!existsSync(compiler) || !existsSync(codecScorer)) {
  execFileSync(
    cargo,
    ["build", "--release", "--bin", "lilscript", "--bin", "lilscript-codec"],
    {
      cwd: root,
      stdio: "inherit",
    },
  );
}
mkdirSync(build, { recursive: true });

function sizes(path) {
  return canonicalCodecSizesForFile(path, "compact intrinsic pass gate");
}

const results = {};
for (const codec of ["gzip", "brotli"]) {
  results[codec] = {};
  for (const variant of ["intrinsic", "manual"]) {
    const artifact = join(build, `${codec}-${variant}.js`);
    execFileSync(
      compiler,
      [
        join(here, `${variant}.lil`),
        "--config",
        join(here, `${codec}.toml`),
        "--mode",
        "production",
        "-o",
        artifact,
      ],
      { cwd: root, stdio: "inherit" },
    );
    results[codec][variant] = { artifact, ...sizes(artifact) };
  }
}

const referenceOutput = execFileSync(
  process.execPath,
  [results.brotli.manual.artifact],
  {
    encoding: "utf8",
  },
).trimEnd();
for (const codec of ["gzip", "brotli"]) {
  for (const variant of ["intrinsic", "manual"]) {
    const output = execFileSync(
      process.execPath,
      [results[codec][variant].artifact],
      {
        encoding: "utf8",
      },
    ).trimEnd();
    if (output !== referenceOutput) {
      throw new Error(
        `${codec}/${variant} behavior mismatch: ${output} != ${referenceOutput}`,
      );
    }
  }
}

const edgeJs = join(build, "edge-cases.js");
const edgeNative = join(build, "edge-cases-native");
execFileSync(
  compiler,
  [
    join(here, "edge-cases.lil"),
    "--target",
    "js",
    "--mode",
    "development",
    "-o",
    edgeJs,
  ],
  { cwd: root, stdio: "inherit" },
);
execFileSync(
  compiler,
  [join(here, "edge-cases.lil"), "--target", "native", "-o", edgeNative],
  { cwd: root, stdio: "inherit" },
);
const edgeExpected = readFileSync(
  join(here, "edge-cases.expected"),
  "utf8",
).trimEnd();
for (const [label, executable, args] of [
  ["JavaScript", process.execPath, [edgeJs]],
  ["native", edgeNative, []],
]) {
  const actual = execFileSync(executable, args, { encoding: "utf8" }).trimEnd();
  if (actual !== edgeExpected) {
    throw new Error(
      `${label} edge-case output mismatch: ${actual} != ${edgeExpected}`,
    );
  }
}

if (results.gzip.intrinsic.gzip >= results.gzip.manual.gzip) {
  throw new Error(
    "native intrinsics did not reduce the gzip-selected artifact",
  );
}
if (results.brotli.intrinsic.brotli >= results.brotli.manual.brotli) {
  throw new Error(
    "native intrinsics did not reduce the Brotli-selected artifact",
  );
}

const samples = {
  intrinsic: { time: [], memory: [] },
  manual: { time: [], memory: [] },
};
const sampleCount = configuredSampleCount();
for (let round = 0; round < sampleCount; round++) {
  for (const variant of round % 2
    ? ["manual", "intrinsic"]
    : ["intrinsic", "manual"]) {
    for (const mode of ["performance", "memory"]) {
      const value = JSON.parse(
        execFileSync(
          process.execPath,
          [
            ...(mode === "memory" ? ["--expose-gc"] : []),
            join(here, "worker.mjs"),
            mode,
            results.brotli[variant].artifact,
            `${round}-${variant}-${mode}`,
          ],
          { encoding: "utf8" },
        ),
      );
      if (value.output !== referenceOutput) {
        throw new Error(`${variant}/${mode} workload output mismatch`);
      }
      samples[variant][mode === "memory" ? "memory" : "time"].push(
        mode === "memory" ? value.bytes : value.milliseconds,
      );
    }
  }
}

const performance = requireNonInferiority(
  samples.intrinsic.time,
  samples.manual.time,
  {
    label: "compact intrinsics/runtime",
  },
);
const retainedMemory = requireNonInferiority(
  samples.intrinsic.memory,
  samples.manual.memory,
  { label: "compact intrinsics/retained memory" },
);
const runtime = {
  samples: sampleCount,
  intrinsicMs: performance.candidate.median,
  manualMs: performance.baseline.median,
  intrinsicP95Ms: performance.candidate.p95,
  manualP95Ms: performance.baseline.p95,
  intrinsicBytes: retainedMemory.candidate.median,
  manualBytes: retainedMemory.baseline.median,
  intrinsicP95Bytes: retainedMemory.candidate.p95,
  manualP95Bytes: retainedMemory.baseline.p95,
  performance,
  retainedMemory,
};

console.log(
  JSON.stringify(
    {
      schemaVersion: 1,
      objectiveContract: {
        artifactMetricMapping: {
          gzip: {
            artifacts: "sizes.gzip.*",
            config: "gzip.toml",
            gateMetric: "gzip",
            diagnosticMetrics: ["raw", "brotli"],
          },
          brotli: {
            artifacts: "sizes.brotli.*",
            config: "brotli.toml",
            gateMetric: "brotli",
            diagnosticMetrics: ["raw", "gzip"],
          },
        },
        gates: [
          {
            candidate: "intrinsic",
            baseline: "manual",
            gateMetric: "gzip",
            expectation: "lt",
          },
          {
            candidate: "intrinsic",
            baseline: "manual",
            gateMetric: "brotli",
            expectation: "lt",
          },
        ],
        diagnosticCrossMetricsMayLose: true,
      },
      codecs: canonicalCodecProvenance("compact intrinsics report"),
      output: referenceOutput,
      sizes: results,
      runtime: {
        objective: "brotli",
        artifacts: {
          intrinsic: "sizes.brotli.intrinsic",
          manual: "sizes.brotli.manual",
        },
        ...runtime,
      },
    },
    null,
    2,
  ),
);
