import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { relative, resolve } from "node:path";
import { build } from "vite";
import {
  canonicalCodecProvenance,
  canonicalCodecSizes,
} from "../../../benchmarks/codec-contract.mjs";
import { compilerPath } from "../tooling/compiler-path.mjs";
import { root } from "./project.mjs";

const packageRoot = resolve(root, "packages/solidlil");
const generatedRoot = resolve(
  root,
  "artifacts/generated/distribution-candidates",
);
const reportPath = resolve(root, "artifacts/distribution-selection.json");
const sourcePath = resolve(root, "apps/lilscript/src/reactive.lil");
const baseConfigPath = resolve(root, "config/open-world.toml");
const selectedCompiler = compilerPath();
const runtimeModuleId = resolve(packageRoot, "reactive.generated.js");

const sha256 = (value) => createHash("sha256").update(value).digest("hex");
const read = (path) => readFileSync(path);

function replaceOptimizationLevel(config, level) {
  const next = config.replace(
    /^optimization_level\s*=\s*\d+$/mu,
    `optimization_level = ${level}`,
  );
  assert.notEqual(next, config, "open-world optimization level replacement");
  return next;
}

function setExactLayoutLimit(config, limit) {
  const line = `function_layout_exact_limit = ${limit}`;
  if (/^function_layout_exact_limit\s*=/mu.test(config)) {
    return config.replace(/^function_layout_exact_limit\s*=.*$/mu, line);
  }
  return config.replace(
    /^function_spelling\s*=.*$/mu,
    (match) => `${match}\n${line}`,
  );
}

const baseConfig = readFileSync(baseConfigPath, "utf8");
const variants = [
  {
    id: "production-15",
    description: "Level 15 with production codec-guided candidate search",
    config: baseConfig,
    mode: "production",
    preference: 0,
  },
  {
    id: "source-15",
    description:
      "Level 15 source-shaped development output without candidate search",
    config: baseConfig,
    mode: "development",
    preference: 1,
  },
  {
    id: "production-12",
    description:
      "Level 12 production search, retaining a different phase/layout frontier",
    config: replaceOptimizationLevel(baseConfig, 12),
    mode: "production",
    preference: 2,
  },
  {
    id: "layout-18",
    description:
      "Level 15 production search with the largest exact function-layout frontier",
    config: setExactLayoutLimit(baseConfig, 18),
    mode: "production",
    preference: 3,
  },
];

const identity = {
  compilerSha256: sha256(read(selectedCompiler)),
  sourceSha256: sha256(read(sourcePath)),
  baseConfigSha256: sha256(baseConfig),
};
const identitySha256 = sha256(JSON.stringify(identity));

mkdirSync(generatedRoot, { recursive: true });

function runCompiler(variant, configPath, outputPath) {
  const args = [
    sourcePath,
    "--target",
    "js-module",
    "--config",
    configPath,
    "--mode",
    variant.mode,
    "-o",
    outputPath,
  ];
  const result = spawnSync(selectedCompiler, args, {
    cwd: root,
    encoding: "utf8",
    env: process.env,
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(
      `${selectedCompiler} ${args.join(" ")}\n${result.stdout ?? ""}${result.stderr ?? ""}`,
    );
  }
}

function candidatePaths(variant) {
  return {
    config: resolve(generatedRoot, `${variant.id}.toml`),
    metadata: resolve(generatedRoot, `${variant.id}.json`),
    runtime: resolve(generatedRoot, `${variant.id}.runtime.js`),
  };
}

function candidateFingerprint(variant) {
  return sha256(
    JSON.stringify({
      ...identity,
      configSha256: sha256(variant.config),
      mode: variant.mode,
    }),
  );
}

function ensureCandidate(variant) {
  const paths = candidatePaths(variant);
  const fingerprint = candidateFingerprint(variant);
  let metadata = null;
  try {
    metadata = JSON.parse(readFileSync(paths.metadata, "utf8"));
  } catch {
    // A missing or interrupted cache entry is regenerated below.
  }
  if (
    metadata?.fingerprint !== fingerprint ||
    metadata?.runtimeSha256 !==
      (() => {
        try {
          return sha256(read(paths.runtime));
        } catch {
          return null;
        }
      })()
  ) {
    writeFileSync(paths.config, variant.config);
    runCompiler(variant, paths.config, paths.runtime);
    metadata = {
      id: variant.id,
      description: variant.description,
      mode: variant.mode,
      fingerprint,
      configSha256: sha256(variant.config),
      runtimeSha256: sha256(read(paths.runtime)),
      runtimeRawBytes: read(paths.runtime).byteLength,
    };
    writeFileSync(paths.metadata, `${JSON.stringify(metadata, null, 2)}\n`);
  }
  return { ...variant, ...paths, metadata };
}

async function bundleCandidate(entry, candidate, { clientOnly }) {
  const runtimeCode = readFileSync(candidate.runtime, "utf8");
  const result = await build({
    configFile: false,
    root,
    logLevel: "error",
    plugins: [
      {
        name: "solidlil-final-artifact-runtime",
        enforce: "pre",
        resolveId(source, importer) {
          if (
            source === "./reactive.generated.js" &&
            importer?.startsWith(packageRoot)
          ) {
            // Preserve the real module identity. Module IDs can perturb chunk
            // ordering and identifier allocation, which would make the outer
            // comparison measure the harness rather than the candidate.
            return runtimeModuleId;
          }
          return null;
        },
        load(id) {
          return id === runtimeModuleId ? runtimeCode : null;
        },
      },
    ],
    resolve: {
      conditions: ["browser", "module", "import", "default"],
    },
    define: clientOnly
      ? { "import.meta.env.SOLIDLIL_CLIENT_ONLY": "true" }
      : undefined,
    build: {
      target: "es2022",
      minify: "oxc",
      write: false,
      lib: { entry, formats: ["es"], fileName: "bundle" },
      rolldownOptions: { output: { codeSplitting: false } },
    },
  });
  const outputs = Array.isArray(result)
    ? result.flatMap((item) => item.output)
    : result.output;
  const chunks = outputs.filter((item) => item.type === "chunk");
  assert.equal(chunks.length, 1, `${entry} should emit one JavaScript chunk`);
  return `${chunks[0].code.trim()}\n`;
}

function sizes(code, label) {
  const measured = canonicalCodecSizes(code, label);
  return {
    brotli11: measured.brotli,
    gzip9: measured.gzip,
    raw: measured.raw,
  };
}

function readReport() {
  try {
    const report = JSON.parse(readFileSync(reportPath, "utf8"));
    if (report.identitySha256 === identitySha256) return report;
  } catch {
    // Start a fresh evidence ledger when no compatible report exists.
  }
  return {
    schemaVersion: 1,
    generatedAt: null,
    objectiveContract: {
      gateMetric: "brotli11",
      matchingArtifactOnly: true,
      selectionStage: "final-tree-shaken-minified-chunk",
      crossMetricsAreDiagnostic: ["raw", "gzip9"],
    },
    identity,
    identitySha256,
    compiler: {
      path: relative(root, selectedCompiler) || selectedCompiler,
      sha256: identity.compilerSha256,
    },
    codecs: canonicalCodecProvenance("SolidLil distribution selection"),
    candidates: {},
    targets: {},
  };
}

/**
 * Select the runtime representation against the complete deliverable chunk.
 *
 * The compiler cannot see which exports a later JavaScript bundler tree-shakes
 * from a reusable package. This bounded outer loop keeps all candidates
 * behavior-equivalent and lets canonical Brotli-11 judge the artifact users
 * actually download, including minifier spelling and function ordering.
 */
export async function selectSolidLilDistribution({
  clientOnly = false,
  entry,
  output,
  target,
}) {
  const candidates = variants.map(ensureCandidate);
  const evaluated = [];
  for (const candidate of candidates) {
    const code = await bundleCandidate(entry, candidate, { clientOnly });
    const artifactPath = resolve(generatedRoot, `${target}.${candidate.id}.js`);
    writeFileSync(artifactPath, code);
    evaluated.push({
      id: candidate.id,
      description: candidate.description,
      mode: candidate.mode,
      preference: candidate.preference,
      code,
      artifactPath,
      artifactSha256: sha256(code),
      runtimeSha256: candidate.metadata.runtimeSha256,
      sizes: sizes(code, `SolidLil ${target} candidate selection`),
    });
  }
  evaluated.sort(
    (left, right) =>
      left.sizes.brotli11 - right.sizes.brotli11 ||
      left.sizes.raw - right.sizes.raw ||
      left.preference - right.preference,
  );
  const winner = evaluated[0];
  writeFileSync(output, winner.code);

  const report = readReport();
  for (const candidate of candidates) {
    report.candidates[candidate.id] = {
      description: candidate.description,
      mode: candidate.mode,
      config: relative(root, candidate.config),
      configSha256: candidate.metadata.configSha256,
      runtime: relative(root, candidate.runtime),
      runtimeSha256: candidate.metadata.runtimeSha256,
      runtimeRawBytes: candidate.metadata.runtimeRawBytes,
    };
  }
  report.generatedAt = new Date().toISOString();
  report.targets[target] = {
    entry: relative(root, entry),
    output: relative(root, output),
    clientOnly,
    winner: winner.id,
    sizes: winner.sizes,
    artifactSha256: winner.artifactSha256,
    candidates: Object.fromEntries(
      evaluated
        .sort((left, right) => left.id.localeCompare(right.id))
        .map((candidate) => [
          candidate.id,
          {
            artifact: relative(root, candidate.artifactPath),
            artifactSha256: candidate.artifactSha256,
            runtimeSha256: candidate.runtimeSha256,
            sizes: candidate.sizes,
          },
        ]),
    ),
  };
  writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  return {
    code: winner.code,
    selection: report.targets[target],
  };
}

/** Bundle a behavior harness with the exact runtime chosen for its public row. */
export async function bundleSolidLilCandidate({
  candidateId,
  clientOnly = false,
  entry,
  output,
}) {
  const candidate = variants
    .map(ensureCandidate)
    .find(({ id }) => id === candidateId);
  assert.ok(
    candidate,
    `unknown SolidLil distribution candidate: ${candidateId}`,
  );
  const code = await bundleCandidate(entry, candidate, { clientOnly });
  writeFileSync(output, code);
  return code;
}
