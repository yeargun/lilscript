import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "vite";
import {
  canonicalCodecProvenance,
  canonicalCodecSizes,
} from "../../benchmarks/codec-contract.mjs";
import {
  resolveBrotliConfig,
  resolveCompilerToolchain,
} from "./evidence-toolchain.mjs";

const labRoot = dirname(fileURLToPath(import.meta.url));
const lilastroRoot = resolve(labRoot, "..");
const repoRoot = resolve(lilastroRoot, "..");
const buildRoot = join(lilastroRoot, "build");
const compilerToolchain = resolveCompilerToolchain(
  repoRoot,
  "Lilastro Motion measurement",
);
const compiler = compilerToolchain.executable;
const configuredPath = process.env.LILSCRIPT_CONFIG
  ? resolve(process.cwd(), process.env.LILSCRIPT_CONFIG)
  : join(lilastroRoot, "config/closed-world.toml");
const compilerConfiguration = resolveBrotliConfig(
  configuredPath,
  repoRoot,
  "Lilastro Motion measurement",
);
const compilerConfig = compilerConfiguration.resolvedPath;
const codecs = canonicalCodecProvenance("Lilastro Motion measurement");

mkdirSync(buildRoot, { recursive: true });

const EXAMPLES = [
  {
    id: "values-core",
    title: "Core values digest",
    inspiredBy: "popular Motion digest / Motion JS utils",
    apis: ["mix", "wrap", "stagger", "spring"],
  },
  {
    id: "spring-stagger",
    title: "Spring + stagger orchestration",
    inspiredBy: "Motion stagger docs patterns",
    apis: ["spring", "stagger", "mix"],
  },
  {
    id: "distance-geometry",
    title: "Distance helpers",
    inspiredBy: "motion distance / distance2D",
    apis: ["distance", "distance2D", "mix"],
  },
  {
    id: "animate-play",
    title: "animate + animateMini",
    inspiredBy: "dev/html/public/playwright/animate/animate-play.html",
    apis: ["animate", "animateMini", "spring"],
  },
  {
    id: "animate-scroll",
    title: "animate + scroll",
    inspiredBy: "dev/html/public/playwright/animate/animate-scroll.html",
    apis: ["animate", "animateMini", "scroll", "stagger"],
  },
];

function run(program, args, cwd = lilastroRoot) {
  const result = spawnSync(program, args, {
    cwd,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(
      `${program} ${args.join(" ")}\n${result.stdout}\n${result.stderr}`,
    );
  }
  return result.stdout.trim();
}

function metrics(code) {
  return canonicalCodecSizes(code, "Lilastro Motion measurement");
}

async function viteSize(root, name) {
  const outDir = join(buildRoot, name);
  await build({
    root,
    logLevel: "error",
    build: {
      outDir,
      emptyOutDir: true,
      minify: true,
      write: true,
      rollupOptions: { input: join(root, "index.html") },
    },
  });
  const assetsDir = join(outDir, "assets");
  const assets = readdirSync(assetsDir).filter((f) => f.endsWith(".js"));
  if (assets.length !== 1) {
    throw new Error(
      `expected one js asset in ${assetsDir}; found ${assets.length}`,
    );
  }
  return metrics(readFileSync(join(assetsDir, assets[0])));
}

function compileLil(exampleId) {
  const lilDir = join(lilastroRoot, "examples", exampleId, "lil");
  const lilMain = join(lilDir, "main.lil");
  const outJs = join(lilDir, "main.js");
  run(compiler, [
    lilMain,
    "--target",
    "js",
    "--config",
    compilerConfig,
    "-o",
    outJs,
  ]);
  writeFileSync(
    join(lilDir, "index.html"),
    `<!doctype html><html><head><meta charset="utf-8"/><title>${exampleId} lil</title></head><body><script type="module" src="./main.js"></script></body></html>\n`,
  );
  return lilDir;
}

const results = [];
for (const example of EXAMPLES) {
  console.log(`measuring ${example.id}...`);
  const npm = await viteSize(
    join(lilastroRoot, "examples", example.id, "ts"),
    `${example.id}-npm-vite`,
  );
  const lilApp = compileLil(example.id);
  const lil = await viteSize(lilApp, `${example.id}-lil-vite`);
  results.push({
    ...example,
    npm,
    lil,
    ratios: {
      raw: lil.raw / npm.raw,
      gzip: lil.gzip / npm.gzip,
      brotli: lil.brotli / npm.brotli,
    },
  });
  console.log(
    `  npm brotli ${npm.brotli} | lil brotli ${lil.brotli} | ratio ${(lil.brotli / npm.brotli).toFixed(3)}`,
  );
}

const report = {
  schemaVersion: 2,
  generatedAt: new Date().toISOString(),
  upstream: {
    repo: "https://github.com/motiondivision/motion",
    tag: "v13.0.0",
    commit: "e4029ce071bddaed2a539b861f5d9c509bea40da",
  },
  toolchain: {
    vite: "8.2.1",
    motionNpm: "13.0.0",
    node: process.version,
    minify: true,
    buildMode: "closed-world",
    compiler: compilerToolchain.evidence,
    compilerConfig: compilerConfiguration.evidence,
    codecs,
  },
  completeness: {
    claim: "DOM compile-green, not behavior-certified",
    reactOutOfScope: true,
    notes: [
      "motion-utils / motion-dom / framer-motion DOM transitive ports compile via index.lil / dom.lil",
      "Selected digest values-core matches popular verify contract",
      "animate/scroll examples measure retained API surface size; browser runtime fidelity is not asserted here",
    ],
  },
  examples: results,
};

const sizeFailures = results
  .filter((example) => example.lil.brotli >= example.npm.brotli)
  .map(
    (example) =>
      `${example.id}/brotli: ${example.lil.brotli} >= ${example.npm.brotli}`,
  );
report.verification = {
  objective: "brotli11",
  objectiveArtifact: "configured-brotli-vite-artifact",
  config: compilerConfiguration.evidence.path,
  costModel: compilerConfiguration.evidence.costModel,
  matchingArtifactOnly: true,
  crossMetricsAreDiagnostic: ["raw", "gzip9"],
  passed: sizeFailures.length === 0,
  failures: sizeFailures,
};

writeFileSync(
  join(buildRoot, "results.json"),
  `${JSON.stringify(report, null, 2)}\n`,
);
console.log(`wrote ${join(buildRoot, "results.json")}`);
if (sizeFailures.length > 0) {
  throw new Error(`LilScript size gate failed: ${sizeFailures.join(", ")}`);
}
