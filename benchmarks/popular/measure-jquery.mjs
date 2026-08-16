import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { build as esbuild } from "esbuild";
import {
  JQUERY_LILSCRIPT_ARTIFACT_ENV,
  JQUERY_LILSCRIPT_ARTIFACT_SHA256_ENV,
} from "./jquery-benchmark-artifact.mjs";
import { minifyJqueryBundle } from "./jquery-measurement-lanes.mjs";
import {
  canonicalCodecProvenance,
  canonicalCodecSizesForFile,
  requireCanonicalCodecRuntime,
} from "../codec-contract.mjs";

const labRoot = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(labRoot, "../..");
const compiler = join(repoRoot, "target/release/lilscript");
const buildRoot = join(labRoot, "build");
const portRoot = join(labRoot, "ports/jquery");
const compiled = join(portRoot, "jquery-measured.raw.js");
const bundledLil = join(buildRoot, "jquery-measured.bundle.js");
const esbuildMinifiedLil = join(
  buildRoot,
  "jquery-measured.esbuild.min.js",
);
const terserMinifiedLil = join(buildRoot, "jquery-measured.terser.min.js");
const oxcMinifiedLil = join(buildRoot, "jquery-measured.oxc.min.js");
const compilerConfig = join(portRoot, "lilscript.toml");
const packageJson = JSON.parse(
  readFileSync(join(labRoot, "package.json"), "utf8"),
);

function sha256File(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function repoPath(path) {
  return relative(repoRoot, path).split(sep).join("/");
}

function run(
  program,
  args,
  { cwd = labRoot, environment = process.env } = {},
) {
  const result = spawnSync(program, args, {
    cwd,
    env: environment,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(
      `${program} ${args.join(" ")}\n${result.stdout}${result.stderr}`,
    );
  }
  return result.stdout.trim();
}

function measurementLane({ role, label, path, tool, inputArtifact = null }) {
  return {
    role,
    label,
    artifactPath: repoPath(path),
    sha256: sha256File(path),
    ...(inputArtifact === null ? {} : { inputArtifact }),
    tool,
    sizes: canonicalCodecSizesForFile(path, label),
  };
}

mkdirSync(buildRoot, { recursive: true });
requireCanonicalCodecRuntime("jQuery bundle measurement");

run(compiler, [
  join(portRoot, "entry.lil"),
  "--config",
  compilerConfig,
  "--target",
  "js-module",
  "-o",
  compiled,
]);

await esbuild({
  absWorkingDir: portRoot,
  entryPoints: [compiled],
  outfile: bundledLil,
  bundle: true,
  format: "esm",
  platform: "neutral",
  minify: false,
  write: true,
});

const bundledSource = readFileSync(bundledLil, "utf8");
const minified = await minifyJqueryBundle(
  bundledSource,
  basename(bundledLil),
);
writeFileSync(esbuildMinifiedLil, minified.esbuild);
writeFileSync(terserMinifiedLil, minified.terser);
writeFileSync(oxcMinifiedLil, minified.oxc);

const npmDistPath = join(labRoot, "node_modules/jquery/dist/jquery.js");
const npmMinPath = join(labRoot, "node_modules/jquery/dist/jquery.min.js");
const bundledArtifact = {
  path: repoPath(bundledLil),
  sha256: sha256File(bundledLil),
};
const generatedArtifact = {
  path: repoPath(compiled),
  sha256: sha256File(compiled),
};

const measurementLanes = {
  npmDevelopment: measurementLane({
    role: "baseline",
    label: "npm jquery/dist/jquery.js (published development artifact)",
    path: npmDistPath,
    tool: {
      name: "jquery",
      version: packageJson.dependencies.jquery,
      operation: "published-package-artifact",
      minifier: null,
    },
  }),
  npmMinified: measurementLane({
    role: "selected-baseline",
    label: "npm jquery/dist/jquery.min.js (published minified artifact)",
    path: npmMinPath,
    tool: {
      name: "jquery",
      version: packageJson.dependencies.jquery,
      operation: "published-package-artifact",
      minifier: "not inferred from the filename",
    },
  }),
  lilscriptBundledUnminified: measurementLane({
    role: "selected-candidate",
    label:
      "LilScript compiler-selected jQuery linked bundle (esbuild minification disabled)",
    path: bundledLil,
    inputArtifact: generatedArtifact,
    tool: {
      name: "esbuild",
      version: packageJson.devDependencies.esbuild,
      operation: "bundle",
      options: { format: "esm", platform: "neutral", minify: false },
    },
  }),
  lilscriptEsbuildMinified: measurementLane({
    role: "diagnostic-competitor",
    label: "Diagnostic: LilScript linked bundle minified by esbuild",
    path: esbuildMinifiedLil,
    inputArtifact: bundledArtifact,
    tool: {
      name: "esbuild",
      version: packageJson.devDependencies.esbuild,
      operation: "transform-minify",
      options: {
        format: "esm",
        target: "esnext",
        minify: true,
        legalComments: "none",
      },
    },
  }),
  lilscriptTerserMinified: measurementLane({
    role: "diagnostic-competitor",
    label: "Diagnostic: LilScript linked bundle minified by Terser",
    path: terserMinifiedLil,
    inputArtifact: bundledArtifact,
    tool: {
      name: "terser",
      version: packageJson.devDependencies.terser,
      operation: "minify",
      options: {
        module: true,
        compress: { passes: 3 },
        mangle: true,
        comments: false,
      },
    },
  }),
  lilscriptOxcMinified: measurementLane({
    role: "diagnostic-competitor",
    label:
      "Diagnostic: LilScript linked bundle minified by Oxc through vite.minify()",
    path: oxcMinifiedLil,
    inputArtifact: bundledArtifact,
    tool: {
      name: "Oxc",
      api: "vite.minify",
      viteVersion: packageJson.devDependencies.vite,
      rolldownVersion: packageJson.devDependencies.rolldown,
      operation: "minify",
      options: {
        module: true,
        compress: true,
        mangle: true,
        removeWhitespace: true,
        legalComments: "none",
        sourcemap: false,
      },
    },
  }),
};

const selectedCandidateKey = "lilscriptBundledUnminified";
const diagnosticCompetitorKeys = [
  "lilscriptEsbuildMinified",
  "lilscriptTerserMinified",
  "lilscriptOxcMinified",
];
const selectedCandidateArtifact = `measurementLanes.${selectedCandidateKey}`;
const selectedCandidate = measurementLanes[selectedCandidateKey];
const selectedBaseline = measurementLanes.npmMinified;
const verifyOut = run(process.execPath, [join(labRoot, "verify-jquery.mjs")], {
  environment: {
    ...process.env,
    [JQUERY_LILSCRIPT_ARTIFACT_ENV]: resolve(
      repoRoot,
      selectedCandidate.artifactPath,
    ),
    [JQUERY_LILSCRIPT_ARTIFACT_SHA256_ENV]: selectedCandidate.sha256,
  },
});

const expected = readFileSync(
  join(labRoot, "apps/jquery/expected.txt"),
  "utf8",
).trim();
const npmContract = run(process.execPath, [
  join(labRoot, "apps/jquery/js/main.js"),
]);
if (npmContract !== expected) {
  throw new Error(
    `npm development artifact contract mismatch\nnpm=${npmContract}\nexpected=${expected}`,
  );
}

const unavailable = { raw: "—", gzip: "—", brotli: "—" };
const table = {
  schemaVersion: 1,
  id: "jquery",
  project: "jQuery",
  eligibility: "candidate",
  evidenceStatus: "current-measurement",
  blockers: [
    "Full 3.7.1 LilScript surface is ported (LilScript-native selector). Not an exact published-entrypoint eligibility claim yet: Closure ADVANCED lane and representative perf/memory gates remain open.",
  ],
  verification: {
    differential: verifyOut.split("\n").filter(Boolean).join(" | "),
    measuredArtifactDifferential: {
      artifact: selectedCandidateArtifact,
      artifactPath: selectedCandidate.artifactPath,
      sha256: selectedCandidate.sha256,
      result: verifyOut.split("\n").filter(Boolean).join(" | "),
    },
    npmDevelopmentContract: npmContract,
    selectedEntrypoint: "jquery/dist/jquery.js",
  },
  closureLevel: null,
  costModel: "brotli",
  objectiveContract: {
    artifact: selectedCandidateArtifact,
    baselineArtifacts: ["measurementLanes.npmMinified"],
    gateMetric: "brotli",
    diagnosticMetrics: ["raw", "gzip"],
    diagnosticCrossMetricsMayLose: true,
    matchingArtifactOnly: true,
    scope: "ineligible candidate diagnostic; no size claim is published",
  },
  candidateSelection: {
    policy:
      "fixed single compiler-selected output at the linked library boundary; no downstream minifier selection",
    candidates: [selectedCandidateArtifact],
    stableTieOrder: [selectedCandidateArtifact],
    selectedArtifact: selectedCandidateArtifact,
    selectedBrotliBytes: selectedCandidate.sizes.brotli,
    diagnosticCompetitors: diagnosticCompetitorKeys.map(
      (key) => `measurementLanes.${key}`,
    ),
  },
  measurementLanes,
  legacyTopLevelAliases: {
    status: "deprecated compatibility aliases; measurementLanes is authoritative",
    rawJs: "measurementLanes.npmDevelopment.sizes",
    terser:
      "measurementLanes.npmMinified.sizes (legacy key; not a claim about its minifier)",
    vite:
      "measurementLanes.npmMinified.sizes (legacy baseline key; not Vite output)",
    lilscript: "measurementLanes.lilscriptBundledUnminified.sizes",
    lilscriptVite:
      "measurementLanes.lilscriptBundledUnminified.sizes (legacy key; no downstream minifier is selected)",
  },
  rawJs: measurementLanes.npmDevelopment.sizes,
  terser: measurementLanes.npmMinified.sizes,
  closure: unavailable,
  vite: measurementLanes.npmMinified.sizes,
  lilscript: measurementLanes.lilscriptBundledUnminified.sizes,
  lilscriptVite: selectedCandidate.sizes,
  libraryArtifacts: {
    npmDevelopment: measurementLanes.npmDevelopment.sizes,
    npmMinified: measurementLanes.npmMinified.sizes,
    lilscriptBundledUnminified:
      measurementLanes.lilscriptBundledUnminified.sizes,
    lilscriptEsbuildMinified: measurementLanes.lilscriptEsbuildMinified.sizes,
    lilscriptTerserMinified: measurementLanes.lilscriptTerserMinified.sizes,
    lilscriptOxcMinified: measurementLanes.lilscriptOxcMinified.sizes,
  },
  codecs: canonicalCodecProvenance("jQuery bundle report"),
  compiler: {
    path: "target/release/lilscript",
    sha256: sha256File(compiler),
    config: "benchmarks/popular/ports/jquery/lilscript.toml",
    configSha256: sha256File(compilerConfig),
    generatedArtifact,
  },
  expected,
  status: "candidate-full-library",
  packages: [{ name: "jquery", version: packageJson.dependencies.jquery }],
  entrypoint: "dist/jquery.js",
  publicRuntimeApi: ["jQuery", "$"],
  compatibilityNotes:
    "Full-library size row (not a tree-shaken app). npmDevelopment and npmMinified are the two published jQuery artifacts. The hard LilScript artifact is the compiler's single output after esbuild links its required JavaScript/TypeScript host facade with minification disabled. Esbuild, Terser, and direct vite.minify()/Oxc outputs all consume those exact linked bytes and are diagnostic competitor analysis only; they cannot replace the hard artifact. Raw and gzip are diagnostics for the Brotli-selected compiler output. Legacy top-level keys are compatibility aliases only, and this ineligible row establishes no size claim.",
  performance: null,
  exactSurface: false,
  performanceGate: null,
  sizeGate: false,
  eligible: false,
};

writeFileSync(
  join(buildRoot, "jquery-results.json"),
  JSON.stringify(table, null, 2) + "\n",
);
console.log(JSON.stringify(table, null, 2));
const brDelta =
  (selectedCandidate.sizes.brotli / selectedBaseline.sizes.brotli - 1) * 100;
console.log(
  `Brotli objective (single compiler-selected linked artifact): ${selectedCandidate.label} ${selectedCandidate.sizes.brotli} / npm published min ${selectedBaseline.sizes.brotli} (${brDelta >= 0 ? "+" : ""}${brDelta.toFixed(1)}%); downstream minifier outputs are diagnostics only; diagnostic raw LilScript bundle ${measurementLanes.lilscriptBundledUnminified.sizes.raw} / npm development ${measurementLanes.npmDevelopment.sizes.raw}`,
);
