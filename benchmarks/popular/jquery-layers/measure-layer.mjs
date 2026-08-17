import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { build as esbuild } from "esbuild";
import {
  canonicalCodecProvenance,
  canonicalCodecSizesForFile,
  requireCanonicalCodecRuntime,
} from "../../codec-contract.mjs";
import { minifyJqueryBundle } from "../jquery-measurement-lanes.mjs";
import { layerById, layers, planned } from "./catalog.mjs";
import { extractLayer } from "./extract-upstream.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const labRoot = resolve(here, "..");
const repoRoot = resolve(labRoot, "../..");
const compiler = process.env.LILSCRIPT
  ? resolve(process.cwd(), process.env.LILSCRIPT)
  : join(repoRoot, "target/release/lilscript");
const upstreamRoot = join(labRoot, "node_modules/jquery/src");

async function ensureDom() {
  if (globalThis.document) {
    return;
  }
  const { JSDOM } = await import("jsdom");
  const dom = new JSDOM("<!doctype html><html><body></body></html>");
  globalThis.window = dom.window;
  globalThis.document = dom.window.document;
}

function run(program, args) {
  const result = spawnSync(program, args, {
    cwd: labRoot,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(`${program} ${args.join(" ")}\n${result.stdout}${result.stderr}`);
  }
  return result.stdout.trim();
}

function pickBest(sizesByLane, metric) {
  let winner = null;
  for (const [lane, sizes] of Object.entries(sizesByLane)) {
    if (!winner || sizes[metric] < winner.sizes[metric]) {
      winner = { lane, sizes };
    }
  }
  return winner;
}

async function measureOne(id) {
  const layer = layerById(id);
  const buildRoot = join(labRoot, "build/jquery-layers", id);
  mkdirSync(buildRoot, { recursive: true });

  const upstreamPath = join(buildRoot, "upstream.js");
  const upstreamSource = extractLayer(id, upstreamRoot);
  writeFileSync(upstreamPath, upstreamSource);

  const minified = await minifyJqueryBundle(upstreamSource, `${id}.upstream.js`);
  const jsLanes = {
    extracted: { path: upstreamPath, sizes: canonicalCodecSizesForFile(upstreamPath, `jquery-layer ${id} js extract`) },
  };
  for (const [lane, code] of Object.entries(minified)) {
    const path = join(buildRoot, `upstream.${lane}.js`);
    writeFileSync(path, code);
    jsLanes[lane] = {
      path,
      sizes: canonicalCodecSizesForFile(path, `jquery-layer ${id} js ${lane}`),
    };
  }
  if (layer.officialMin) {
    const officialPath = join(labRoot, layer.officialMin);
    jsLanes.official = {
      path: officialPath,
      sizes: canonicalCodecSizesForFile(officialPath, `jquery-layer ${id} official min`),
    };
  }
  const jsBest = layer.officialMin
    ? { lane: "official", sizes: jsLanes.official.sizes }
    : pickBest(
        Object.fromEntries(
          Object.entries(jsLanes)
            .filter(([lane]) => lane !== "extracted" && lane !== "official")
            .map(([lane, row]) => [lane, row.sizes]),
        ),
        "brotli",
      );

  const compiledPath = join(buildRoot, "lilscript.raw.js");
  const layerConfig = join(here, `lilscript.${id}.toml`);
  run(compiler, [
    join(labRoot, layer.lilEntry),
    "--config",
    existsSync(layerConfig) ? layerConfig : join(here, "lilscript.toml"),
    "--target",
    "js-module",
    "-o",
    compiledPath,
  ]);
  const compiledSource = readFileSync(compiledPath, "utf8");
  const needsHost = /from\s+["'][^"']*js-host/u.test(compiledSource);
  let lilPath = compiledPath;
  if (needsHost) {
    lilPath = join(buildRoot, "lilscript.bundle.js");
    await esbuild({
      absWorkingDir: join(labRoot, "ports/jquery"),
      entryPoints: [compiledPath],
      outfile: lilPath,
      bundle: true,
      format: "esm",
      platform: "neutral",
      minify: false,
      write: true,
    });
  }
  const lilSizes = canonicalCodecSizesForFile(lilPath, `jquery-layer ${id} lilscript`);

  await ensureDom();
  const verifyModule = await import(pathToFileURL(join(labRoot, layer.verify)).href);
  if (typeof verifyModule.verify !== "function") {
    throw new Error(`${layer.verify} must export verify(lil, js)`);
  }
  const lilModule = await import(`${pathToFileURL(lilPath).href}?t=${Date.now()}`);
  const jsModule = await import(`${pathToFileURL(upstreamPath).href}?t=${Date.now()}`);
  await verifyModule.verify(lilModule, jsModule);

  const delta = lilSizes.brotli - jsBest.sizes.brotli;
  const report = {
    layer: id,
    title: layer.title,
    exports: layer.exports,
    verification: "ok",
    lilscript: {
      path: lilPath.slice(repoRoot.length + 1),
      bundledHost: needsHost,
      sizes: lilSizes,
    },
    javascript: {
      extracted: jsLanes.extracted.sizes,
      terser: jsLanes.terser.sizes,
      oxc: jsLanes.oxc.sizes,
      esbuild: jsLanes.esbuild.sizes,
      ...(jsLanes.official ? { official: jsLanes.official.sizes } : {}),
      selectedBaseline: {
        lane: jsBest.lane,
        sizes: jsBest.sizes,
      },
    },
    gate: {
      metric: "brotli",
      lilscript: lilSizes.brotli,
      javascript: jsBest.sizes.brotli,
      delta,
      pass: layer.officialMin ? delta < -32 : delta <= 0,
    },
    codecs: canonicalCodecProvenance(`jquery-layer ${id}`),
  };
  writeFileSync(join(buildRoot, "report.json"), JSON.stringify(report, null, 2) + "\n");
  return report;
}

function printReport(report) {
  const sign = report.gate.delta > 0 ? "+" : "";
  console.log(`${report.layer}: ${report.title}`);
  console.log(
    `  JS extract   raw=${report.javascript.extracted.raw} gzip=${report.javascript.extracted.gzip} brotli=${report.javascript.extracted.brotli}`,
  );
  console.log(
    `  JS terser    raw=${report.javascript.terser.raw} gzip=${report.javascript.terser.gzip} brotli=${report.javascript.terser.brotli}`,
  );
  console.log(
    `  JS oxc       raw=${report.javascript.oxc.raw} gzip=${report.javascript.oxc.gzip} brotli=${report.javascript.oxc.brotli}`,
  );
  console.log(
    `  JS esbuild   raw=${report.javascript.esbuild.raw} gzip=${report.javascript.esbuild.gzip} brotli=${report.javascript.esbuild.brotli}`,
  );
  if (report.javascript.official) {
    console.log(
      `  JS official  raw=${report.javascript.official.raw} gzip=${report.javascript.official.gzip} brotli=${report.javascript.official.brotli}`,
    );
  }
  console.log(
    `  LilScript    raw=${report.lilscript.sizes.raw} gzip=${report.lilscript.sizes.gzip} brotli=${report.lilscript.sizes.brotli}${report.lilscript.bundledHost ? " (host bundled)" : ""}`,
  );
  console.log(
    `  Brotli gate  lil ${report.gate.lilscript} / js ${report.gate.javascript} (${jsLane(report)}) ${sign}${report.gate.delta} ${report.gate.pass ? "PASS" : "FAIL"}`,
  );
}

function jsLane(report) {
  return report.javascript.selectedBaseline.lane;
}

const ids = process.argv.slice(2).filter((arg) => arg !== "--all");
const selected = process.argv.includes("--all") || ids.length === 0
  ? layers.map((layer) => layer.id)
  : ids;

requireCanonicalCodecRuntime("jquery layer measurement");
const reports = [];
for (const id of selected) {
  reports.push(await measureOne(id));
}
for (const report of reports) {
  printReport(report);
}
const failed = reports.filter((report) => !report.gate.pass);
if (failed.length) {
  console.log(`\nplanned ladder: ${planned.join(" → ")}`);
  throw new Error(
    `${failed.map((report) => report.layer).join(", ")} lost Brotli to the extracted JS baseline`,
  );
}
console.log(`\nplanned ladder: ${planned.join(" → ")}`);
