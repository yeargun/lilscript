import { mkdirSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build as esbuild } from "esbuild";
import {
  canonicalCodecProvenance,
  requireCanonicalCodecRuntime,
} from "../../codec-contract.mjs";
import {
  monacoEditorCoreVersion,
  monacoEditorVersion,
  vscodeCommitId,
} from "./catalog.mjs";
import {
  countJsFiles,
  coreEsm,
  labRoot,
  notOneToOne,
  pairs as filePairs,
} from "./file-map.mjs";
import {
  jsHostPlugin,
  measurePair,
  scoreProductionFile,
} from "./measure-pairs.mjs";
import { renderLanding } from "./render-landing.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(labRoot, "../..");
const compiler = process.env.LILSCRIPT
  ? resolve(process.cwd(), process.env.LILSCRIPT)
  : join(repoRoot, "target/release/lilscript");
const measureOut = join(labRoot, "build/monaco-layers");
const appsRoot = join(labRoot, "apps/monaco");
const lilOutDir = join(appsRoot, "lil");
const jsOutDir = join(appsRoot, "js");

function run(program, args) {
  const result = spawnSync(program, args, {
    cwd: labRoot,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(`${program} ${args.join(" ")}\n${result.stdout}${result.stderr}`);
  }
}

const monacoBundle = {
  absWorkingDir: labRoot,
  bundle: true,
  format: "esm",
  platform: "browser",
  minify: true,
  write: true,
  logOverride: {
    "import-is-undefined": "silent",
    "empty-import-meta": "silent",
  },
  loader: {
    ".ttf": "file",
    ".woff": "file",
    ".woff2": "file",
    ".css": "empty",
  },
};

mkdirSync(measureOut, { recursive: true });
mkdirSync(lilOutDir, { recursive: true });
mkdirSync(jsOutDir, { recursive: true });

requireCanonicalCodecRuntime("monaco production IDE");

const entryRaw = join(measureOut, "entry.raw.js");
console.log("compiling entry.lil (full LilScript monaco)…");
run(compiler, [
  join(labRoot, "ports/monaco/entry.lil"),
  "--config",
  join(labRoot, "ports/monaco/lilscript.toml"),
  "--target",
  "js-module",
  "-o",
  entryRaw,
]);

console.log("bundling LilScript monaco IDE…");
await esbuild({
  ...monacoBundle,
  entryPoints: [join(lilOutDir, "ide-entry.js")],
  outfile: join(lilOutDir, "ide.js"),
  plugins: [jsHostPlugin],
});

console.log("bundling npm monaco-editor IDE…");
await esbuild({
  ...monacoBundle,
  entryPoints: [join(jsOutDir, "ide-entry.js")],
  outfile: join(jsOutDir, "ide.js"),
  splitting: false,
});

const workers = [
  ["editor.worker.js", join(labRoot, "node_modules/monaco-editor/esm/vs/editor/editor.worker.js")],
  ["json.worker.js", join(labRoot, "node_modules/monaco-editor/esm/vs/language/json/json.worker.js")],
  ["css.worker.js", join(labRoot, "node_modules/monaco-editor/esm/vs/language/css/css.worker.js")],
  ["html.worker.js", join(labRoot, "node_modules/monaco-editor/esm/vs/language/html/html.worker.js")],
  ["ts.worker.js", join(labRoot, "node_modules/monaco-editor/esm/vs/language/typescript/ts.worker.js")],
];

const workerSizes = [];
for (const [name, entry] of workers) {
  console.log("bundling", name);
  const jsWorker = join(jsOutDir, name);
  await esbuild({
    absWorkingDir: labRoot,
    entryPoints: [entry],
    outfile: jsWorker,
    bundle: true,
    format: "iife",
    platform: "browser",
    minify: true,
    write: true,
    logOverride: {
      "import-is-undefined": "silent",
      "empty-import-meta": "silent",
    },
    loader: { ".css": "empty", ".ttf": "empty" },
  });
  workerSizes.push({
    name,
    sizes: scoreProductionFile(jsWorker, `monaco worker ${name}`),
  });
}

console.log("scoring production artifacts…");
const jsIde = scoreProductionFile(join(jsOutDir, "ide.js"), "monaco js ide.js");
const lilIde = scoreProductionFile(join(lilOutDir, "ide.js"), "lilscript monaco ide.js");
const cssPath = join(labRoot, "node_modules/monaco-editor/min/vs/editor/editor.main.css");
const cssSizes = scoreProductionFile(cssPath, "monaco editor.main.css");
const lilCss = scoreProductionFile(join(appsRoot, "lil-editor.css"), "lilscript editor css");
const workersRaw = workerSizes.reduce((n, row) => n + row.sizes.raw, 0);
const workersBrotli = workerSizes.reduce((n, row) => n + row.sizes.brotli, 0);
const workersGzip = workerSizes.reduce((n, row) => n + row.sizes.gzip, 0);
const empty = { raw: 0, gzip: 0, brotli: 0 };

const pairReports = [];
for (const pair of filePairs) {
  if (!pair.measure) {
    pairReports.push({
      id: pair.id,
      title: pair.title,
      plugged: pair.plugged,
      monacoFiles: pair.monacoFiles,
      lilFiles: pair.lilFiles,
      note: pair.note,
    });
    continue;
  }
  console.log("measuring pair", pair.id);
  try {
    pairReports.push(await measurePair(pair, join(measureOut, "pairs", pair.id)));
  } catch (err) {
    console.error(`pair ${pair.id} failed:`, err.message);
    pairReports.push({
      id: pair.id,
      title: pair.title,
      plugged: pair.plugged,
      monacoFiles: pair.monacoFiles,
      lilFiles: pair.lilFiles,
      note: `measure failed: ${err.message}`,
    });
  }
}

const provenance = canonicalCodecProvenance("monaco production IDE");
const sizes = {
  versions: {
    monacoEditor: monacoEditorVersion,
    monacoEditorCore: monacoEditorCoreVersion,
    vscodeCommit: vscodeCommitId,
  },
  protocol: {
    productionJs: "esbuild minify of monaco-editor ESM + workers, then lilscript-codec gzip-9 / brotli-11",
    productionLil: "lilscript compile of ports/monaco/entry.lil + js-host + monaco.d.ts facade + workbench, esbuild minify, lilscript-codec",
    moduleJs: "listed monaco-editor-core files only; other monaco imports external; best of esbuild/terser minify; lilscript-codec",
    moduleLil: "lilscript compiler output (already mangled); js-host tree-shaken when imported; lilscript-codec",
    moduleJsMinifiers: "esbuild and terser; best Brotli wins",
  },
  codec: {
    implementation: provenance.implementation,
    gzip: provenance.gzip9.libraryVersion,
    brotli: provenance.brotli11.libraryVersion,
  },
  coreJsFiles: countJsFiles(coreEsm),
  plugged: ["entire LilScript editor (entry.lil) — no monaco-editor JS"],
  production: {
    js: { ide: jsIde, workers: { raw: workersRaw, gzip: workersGzip, brotli: workersBrotli }, css: cssSizes },
    lil: { ide: lilIde, workers: empty, css: lilCss },
    workers: { raw: workersRaw, gzip: workersGzip, brotli: workersBrotli, files: workerSizes },
    css: cssSizes,
  },
  pairs: pairReports,
  notOneToOne,
};

writeFileSync(join(appsRoot, "sizes.json"), JSON.stringify(sizes, null, 2) + "\n");
writeFileSync(join(appsRoot, "index.html"), renderLanding(sizes));

function fmt(n) {
  return n.toLocaleString("en-US");
}

console.log("IDE bundles ready");
console.log("  Lil", join(lilOutDir, "ide.js"));
console.log("  JS ", join(jsOutDir, "ide.js"));
console.log("production lilscript-codec");
console.log(`  ide.js lil     raw=${fmt(lilIde.raw)}  gzip=${fmt(lilIde.gzip)}  br=${fmt(lilIde.brotli)}`);
console.log(`  ide.js js      raw=${fmt(jsIde.raw)}  gzip=${fmt(jsIde.gzip)}  br=${fmt(jsIde.brotli)}`);
console.log(`  js workers     raw=${fmt(workersRaw)}  gzip=${fmt(workersGzip)}  br=${fmt(workersBrotli)}`);
for (const row of pairReports) {
  if (!row.js) {
    console.log(`  pair ${row.id}  (not scored — ${row.note})`);
    continue;
  }
  console.log(
    `  pair ${row.id}  js br=${fmt(row.js.sizes.brotli)} (${row.js.lane})  lil br=${fmt(row.lil.sizes.brotli)}  Δ=${row.delta.brotli}`,
  );
}
