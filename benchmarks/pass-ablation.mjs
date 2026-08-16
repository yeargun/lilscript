import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  canonicalCodecProvenance,
  canonicalCodecSizesForFile,
  requireExistingLilscriptToolchain,
  requirePairedLilscriptOverrides,
} from "./codec-contract.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const { compilerOverride, codecOverride } = requirePairedLilscriptOverrides(
  "compiler pass-ablation",
);
const executableSuffix = process.platform === "win32" ? ".exe" : "";
const compiler = compilerOverride
  ? resolve(process.cwd(), compilerOverride)
  : join(root, `target/release/lilscript${executableSuffix}`);
const codec = codecOverride
  ? resolve(process.cwd(), codecOverride)
  : join(root, `target/release/lilscript-codec${executableSuffix}`);
const cargo =
  process.env.CARGO ?? join(process.env.HOME ?? "", ".cargo/bin/cargo");

function command(executable, args, capture = false) {
  return execFileSync(executable, args, {
    cwd: root,
    encoding: "utf8",
    stdio: capture ? ["ignore", "pipe", "pipe"] : "inherit",
  });
}

function sizes(path) {
  return canonicalCodecSizesForFile(path, "compiler pass-ablation gate");
}

export function objectiveRelationPasses(
  enabled,
  disabled,
  gateMetric,
  expectation,
) {
  return expectation === "lt"
    ? enabled[gateMetric] < disabled[gateMetric]
    : enabled[gateMetric] <= disabled[gateMetric];
}

export function declaredCostModel(configText) {
  return (
    configText.match(/^cost_model\s*=\s*["'](raw|gzip|brotli)["']\s*$/m)?.[1] ??
    null
  );
}

function prepareCompilerAndCodec() {
  if (compilerOverride) {
    requireExistingLilscriptToolchain(
      "compiler pass-ablation",
      compiler,
      codec,
    );
    return;
  }
  if (!existsSync(compiler) || !existsSync(codec)) {
    command(cargo, [
      "build",
      "--release",
      "--bin",
      "lilscript",
      "--bin",
      "lilscript-codec",
    ]);
  }
}

export function runPassAblation({
  id,
  source,
  expected,
  variants,
  gateMetric,
  expectation = "lt",
}) {
  if (!["raw", "gzip", "brotli"].includes(gateMetric)) {
    throw new Error(`${id} must declare gateMetric as raw, gzip, or brotli`);
  }
  if (!["lt", "le"].includes(expectation)) {
    throw new Error(`${id} must declare expectation as lt or le`);
  }
  prepareCompilerAndCodec();
  const build = join(root, "target/pass-ablation", id);
  const expectedOutput = readFileSync(join(root, expected), "utf8").trimEnd();
  mkdirSync(build, { recursive: true });

  const results = [];
  for (const [label, config, file] of variants) {
    const configPath = join(root, config);
    const configuredObjective = declaredCostModel(
      readFileSync(configPath, "utf8"),
    );
    if (configuredObjective !== gateMetric) {
      throw new Error(
        `${id}/${label}: ${config} must explicitly declare javascript.cost_model = ` +
          `${JSON.stringify(gateMetric)}; found ${JSON.stringify(configuredObjective)}`,
      );
    }
    const output = join(build, file);
    command(compiler, [
      join(root, source),
      "--config",
      configPath,
      "-o",
      output,
    ]);
    const actual = command(process.execPath, [output], true).trimEnd();
    if (actual !== expectedOutput) {
      throw new Error(
        `${label} output mismatch\nexpected:\n${expectedOutput}\nactual:\n${actual}`,
      );
    }
    results.push({ label, ...sizes(output) });
  }

  const [enabled, disabled] = results;
  if (!objectiveRelationPasses(enabled, disabled, gateMetric, expectation)) {
    throw new Error(
      `${id} failed its ${gateMetric} ${expectation} objective: ` +
        `${enabled[gateMetric]} versus ${disabled[gateMetric]}`,
    );
  }

  const diagnosticMetrics = ["raw", "gzip", "brotli"].filter(
    (metric) => metric !== gateMetric,
  );
  const report = {
    schemaVersion: 1,
    id,
    objectiveContract: {
      artifactMetricMapping: {
        [gateMetric]: {
          artifacts: "sizes.*",
          configs: Object.fromEntries(
            variants.map(([label, config]) => [label, config]),
          ),
          gateMetric,
          diagnosticMetrics,
        },
      },
      gates: [
        {
          candidate: enabled.label,
          baseline: disabled.label,
          gateMetric,
          expectation,
        },
      ],
      diagnosticCrossMetricsMayLose: true,
    },
    codecs: canonicalCodecProvenance(`${id} pass-ablation report`),
    output: expectedOutput,
    sizes: results,
  };
  console.log(JSON.stringify(report, null, 2));
  return report;
}
