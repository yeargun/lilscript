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
const build = join(root, "target/web-number");
const toolchainContext = "web number compiler gate";
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
  return canonicalCodecSizesForFile(path, "web number pass gate");
}
const results = {};
for (const codec of ["gzip", "brotli"]) {
  results[codec] = {};
  for (const variant of ["number", "int"]) {
    const artifact = join(build, `${codec}-${variant}.mjs`);
    execFileSync(
      compiler,
      [
        join(here, `${variant}.lil`),
        "--target",
        "js-module",
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
  [results.brotli.int.artifact],
  {
    encoding: "utf8",
  },
).trimEnd();
for (const codec of ["gzip", "brotli"]) {
  for (const variant of ["number", "int"]) {
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
for (const variant of ["number", "int"]) {
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
  if (actual !== reference)
    throw new Error(`${variant}/native output mismatch`);
}
if (results.gzip.number.gzip >= results.gzip.int.gzip) {
  throw new Error("number representation did not reduce gzip-selected output");
}
if (results.brotli.number.brotli >= results.brotli.int.brotli) {
  throw new Error(
    "number representation did not reduce Brotli-selected output",
  );
}

const samples = {
  number: { time: [], memory: [] },
  int: { time: [], memory: [] },
};
const sampleCount = configuredSampleCount();
for (let round = 0; round < sampleCount; round++) {
  for (const variant of round % 2 ? ["int", "number"] : ["number", "int"]) {
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
  samples.number.time,
  samples.int.time,
  {
    label: "web number/runtime",
  },
);
const retainedMemory = requireNonInferiority(
  samples.number.memory,
  samples.int.memory,
  {
    label: "web number/retained memory",
  },
);
const runtime = {
  samples: sampleCount,
  numberMs: performance.candidate.median,
  intMs: performance.baseline.median,
  numberP95Ms: performance.candidate.p95,
  intP95Ms: performance.baseline.p95,
  numberBytes: retainedMemory.candidate.median,
  intBytes: retainedMemory.baseline.median,
  numberP95Bytes: retainedMemory.candidate.p95,
  intP95Bytes: retainedMemory.baseline.p95,
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
            candidate: "number",
            baseline: "int",
            gateMetric: "gzip",
            expectation: "lt",
          },
          {
            candidate: "number",
            baseline: "int",
            gateMetric: "brotli",
            expectation: "lt",
          },
        ],
        diagnosticCrossMetricsMayLose: true,
      },
      codecs: canonicalCodecProvenance("web number report"),
      output: reference,
      sizes: results,
      runtime: {
        objective: "brotli",
        artifacts: {
          number: "sizes.brotli.number",
          int: "sizes.brotli.int",
        },
        ...runtime,
      },
    },
    null,
    2,
  ),
);
