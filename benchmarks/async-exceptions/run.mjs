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
const build = join(root, "target/async-exceptions");
const toolchainContext = "async/exception compiler gate";
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
  return canonicalCodecSizesForFile(path, "async/exception pass gate");
}

const artifacts = {};
for (const codec of ["gzip", "brotli"]) {
  artifacts[codec] = {};
  for (const variant of ["on", "off"]) {
    const artifact = join(build, `${codec}-${variant}.js`);
    execFileSync(
      compiler,
      [
        join(here, "workload.lil"),
        "--config",
        join(here, `${codec}-${variant}.toml`),
        "--mode",
        "production",
        "-o",
        artifact,
      ],
      { cwd: root, stdio: "inherit" },
    );
    artifacts[codec][variant] = { artifact, ...sizes(artifact) };
  }
}

const referencePath = join(here, "reference.js");
const expected = execFileSync(process.execPath, [referencePath], {
  encoding: "utf8",
}).trimEnd();
for (const codec of ["gzip", "brotli"]) {
  for (const variant of ["on", "off"]) {
    const actual = execFileSync(
      process.execPath,
      [artifacts[codec][variant].artifact],
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

const edgeArtifact = join(build, "edge-cases.js");
execFileSync(
  compiler,
  [
    join(here, "edge-cases.lil"),
    "--config",
    join(here, "brotli-on.toml"),
    "--mode",
    "production",
    "-o",
    edgeArtifact,
  ],
  { cwd: root, stdio: "inherit" },
);
const edgeExpected = readFileSync(
  join(here, "edge-cases.expected"),
  "utf8",
).trimEnd();
const edgeActual = execFileSync(process.execPath, [edgeArtifact], {
  encoding: "utf8",
}).trimEnd();
if (edgeActual !== edgeExpected) {
  throw new Error(`edge output mismatch:\n${edgeActual}\n!=\n${edgeExpected}`);
}

if (artifacts.gzip.on.gzip >= artifacts.gzip.off.gzip) {
  throw new Error(
    "unused catch binding elision did not reduce the gzip-selected artifact",
  );
}
if (artifacts.brotli.on.brotli >= artifacts.brotli.off.brotli) {
  throw new Error(
    "unused catch binding elision did not reduce the Brotli-selected artifact",
  );
}

const runtimeArtifacts = {
  on: artifacts.brotli.on.artifact,
  off: artifacts.brotli.off.artifact,
  reference: referencePath,
};
const samples = Object.fromEntries(
  Object.keys(runtimeArtifacts).map((variant) => [
    variant,
    { time: [], memory: [] },
  ]),
);
const sampleCount = configuredSampleCount();
for (let round = 0; round < sampleCount; round++) {
  const order =
    round % 2 ? ["reference", "off", "on"] : ["on", "off", "reference"];
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
const comparisons = Object.fromEntries(
  ["off", "reference"].map((baseline) => [
    baseline,
    {
      performance: requireNonInferiority(
        samples.on.time,
        samples[baseline].time,
        {
          label: `async exceptions/runtime versus ${baseline}`,
        },
      ),
      retainedMemory: requireNonInferiority(
        samples.on.memory,
        samples[baseline].memory,
        {
          label: `async exceptions/retained memory versus ${baseline}`,
        },
      ),
    },
  ]),
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
            config: "gzip-{on,off}.toml",
            gateMetric: "gzip",
            diagnosticMetrics: ["raw", "brotli"],
          },
          brotli: {
            artifacts: "sizes.brotli.*",
            config: "brotli-{on,off}.toml",
            gateMetric: "brotli",
            diagnosticMetrics: ["raw", "gzip"],
          },
        },
        gates: [
          {
            candidate: "on",
            baseline: "off",
            gateMetric: "gzip",
            expectation: "lt",
          },
          {
            candidate: "on",
            baseline: "off",
            gateMetric: "brotli",
            expectation: "lt",
          },
        ],
        diagnosticCrossMetricsMayLose: true,
      },
      codecs: canonicalCodecProvenance("async/exception report"),
      output: expected,
      sizes: artifacts,
      runtime: {
        objective: "brotli",
        artifacts: {
          on: "sizes.brotli.on",
          off: "sizes.brotli.off",
          reference: "reference.js",
        },
        samples: sampleCount,
        variants: runtime,
        comparisons,
      },
    },
    null,
    2,
  ),
);
