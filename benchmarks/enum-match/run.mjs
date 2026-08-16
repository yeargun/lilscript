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
const build = join(root, "target/enum-match");
const toolchainContext = "enum/match compiler gate";
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
  return canonicalCodecSizesForFile(path, "enum/match pass gate");
}
const variants = ["enum", "integer", "string"];
const results = {};
for (const codec of ["gzip", "brotli"]) {
  results[codec] = {};
  for (const variant of variants) {
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

const reference = execFileSync(
  process.execPath,
  [results.brotli.integer.artifact],
  {
    encoding: "utf8",
  },
).trimEnd();
for (const codec of ["gzip", "brotli"]) {
  for (const variant of variants) {
    const actual = execFileSync(
      process.execPath,
      [results[codec][variant].artifact],
      {
        encoding: "utf8",
      },
    ).trimEnd();
    if (actual !== reference)
      throw new Error(`${codec}/${variant} output mismatch`);
  }
}
for (const variant of variants) {
  const executable = join(build, `${variant}-native`);
  execFileSync(
    compiler,
    [join(here, `${variant}.lil`), "--target", "native", "-o", executable],
    { cwd: root, stdio: "inherit" },
  );
  const actual = execFileSync(executable, [], { encoding: "utf8" }).trimEnd();
  if (actual !== reference)
    throw new Error(`${variant}/native output mismatch`);
}

if (results.gzip.enum.gzip >= results.gzip.string.gzip) {
  throw new Error(
    "numeric enum representation did not beat string tags under gzip selection",
  );
}
if (results.brotli.enum.brotli >= results.brotli.string.brotli) {
  throw new Error(
    "numeric enum representation did not beat string tags under Brotli selection",
  );
}
if (results.gzip.enum.gzip > results.gzip.integer.gzip) {
  throw new Error(
    "enum/match regressed against the equivalent hand-written integer model under gzip selection",
  );
}
if (results.brotli.enum.brotli > results.brotli.integer.brotli) {
  throw new Error(
    "enum/match regressed against the equivalent hand-written integer model under Brotli selection",
  );
}

const samples = Object.fromEntries(
  variants.map((variant) => [variant, { time: [], memory: [] }]),
);
const sampleCount = configuredSampleCount();
for (let round = 0; round < sampleCount; round++) {
  const order = round % 2 ? [...variants].reverse() : variants;
  for (const variant of order) {
    for (const mode of ["performance", "memory"]) {
      const sample = JSON.parse(
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
      if (sample.output !== reference)
        throw new Error(`${variant}/${mode} output mismatch`);
      samples[variant][mode === "memory" ? "memory" : "time"].push(
        mode === "memory" ? sample.bytes : sample.milliseconds,
      );
    }
  }
}
const performance = requireNonInferiority(
  samples.enum.time,
  samples.integer.time,
  {
    label: "enum match/runtime versus integers",
  },
);
const retainedMemory = requireNonInferiority(
  samples.enum.memory,
  samples.integer.memory,
  {
    label: "enum match/retained memory versus integers",
  },
);
const runtime = Object.fromEntries(
  variants.map((variant) => [
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
            candidate: "enum",
            baseline: "string",
            gateMetric: "gzip",
            expectation: "lt",
          },
          {
            candidate: "enum",
            baseline: "integer",
            gateMetric: "gzip",
            expectation: "le",
          },
          {
            candidate: "enum",
            baseline: "string",
            gateMetric: "brotli",
            expectation: "lt",
          },
          {
            candidate: "enum",
            baseline: "integer",
            gateMetric: "brotli",
            expectation: "le",
          },
        ],
        diagnosticCrossMetricsMayLose: true,
      },
      codecs: canonicalCodecProvenance("enum/match report"),
      output: reference,
      sizes: results,
      runtime: {
        objective: "brotli",
        artifacts: {
          enum: "sizes.brotli.enum",
          integer: "sizes.brotli.integer",
          string: "sizes.brotli.string",
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
