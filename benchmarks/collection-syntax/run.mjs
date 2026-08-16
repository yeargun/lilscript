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
  median,
  quantile,
  requireNonInferiority,
} from "../statistics.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "../..");
const build = join(root, "target/collection-syntax");
const toolchainContext = "collection syntax compiler gate";
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
  return canonicalCodecSizesForFile(path, "collection syntax pass gate");
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

const expected = execFileSync(process.execPath, [join(here, "reference.js")], {
  encoding: "utf8",
}).trimEnd();
for (const codec of ["gzip", "brotli"]) {
  for (const variant of ["intrinsic", "manual"]) {
    const actual = execFileSync(
      process.execPath,
      [results[codec][variant].artifact],
      {
        encoding: "utf8",
      },
    ).trimEnd();
    if (actual !== expected)
      throw new Error(
        `${codec}/${variant} output mismatch: ${actual} != ${expected}`,
      );
  }
}
for (const variant of ["intrinsic", "manual"]) {
  const executable = join(build, `${variant}-native`);
  execFileSync(
    compiler,
    [join(here, `${variant}.lil`), "--target", "native", "-o", executable],
    {
      cwd: root,
      stdio: "inherit",
    },
  );
  const actual = execFileSync(executable, [], { encoding: "utf8" }).trimEnd();
  if (actual !== expected)
    throw new Error(
      `${variant}/native output mismatch: ${actual} != ${expected}`,
    );
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
  if (actual !== edgeExpected)
    throw new Error(
      `${label} edge output mismatch: ${actual} != ${edgeExpected}`,
    );
}

if (results.gzip.intrinsic.gzip >= results.gzip.manual.gzip) {
  throw new Error(
    "collection syntax did not reduce the gzip-selected artifact",
  );
}
if (results.brotli.intrinsic.brotli >= results.brotli.manual.brotli) {
  throw new Error(
    "collection syntax did not reduce the Brotli-selected artifact",
  );
}

const runtimeArtifacts = {
  intrinsic: results.brotli.intrinsic.artifact,
  reference: join(here, "reference.js"),
};
const samples = {
  intrinsic: { time: [], memory: [] },
  reference: { time: [], memory: [] },
};
const sampleCount = configuredSampleCount();
for (let round = 0; round < sampleCount; round++) {
  const order =
    round % 2 ? ["reference", "intrinsic"] : ["intrinsic", "reference"];
  for (const variant of order) {
    for (const mode of ["performance", "memory"]) {
      const sample = JSON.parse(
        execFileSync(
          process.execPath,
          [
            ...(mode === "memory" ? ["--expose-gc"] : []),
            join(here, "worker.mjs"),
            mode,
            runtimeArtifacts[variant],
            `${round}-${variant}-${mode}`,
          ],
          { encoding: "utf8" },
        ),
      );
      if (sample.output !== expected)
        throw new Error(`${variant}/${mode} output mismatch`);
      samples[variant][mode === "memory" ? "memory" : "time"].push(
        mode === "memory" ? sample.bytes : sample.milliseconds,
      );
    }
  }
}
const performance = requireNonInferiority(
  samples.intrinsic.time,
  samples.reference.time,
  {
    label: "collection syntax/runtime",
  },
);
const retainedMemory = requireNonInferiority(
  samples.intrinsic.memory,
  samples.reference.memory,
  { label: "collection syntax/retained memory" },
);
const runtime = Object.fromEntries(
  Object.keys(samples).map((variant) => [
    variant,
    {
      milliseconds: median(samples[variant].time),
      p95Milliseconds: quantile(samples[variant].time, 0.95),
      bytes: median(samples[variant].memory),
      p95Bytes: quantile(samples[variant].memory, 0.95),
    },
  ]),
);

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
      codecs: canonicalCodecProvenance("collection syntax report"),
      output: expected,
      sizes: results,
      runtime: {
        objective: "brotli",
        artifacts: {
          intrinsic: "sizes.brotli.intrinsic",
          reference: "reference.js",
        },
        samples: sampleCount,
        variants: runtime,
        performance,
        retainedMemory,
      },
    },
    null,
    2,
  ),
);
