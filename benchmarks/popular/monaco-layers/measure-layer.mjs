import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
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

async function ensureDom() {
  if (globalThis.document) {
    return;
  }
  const { JSDOM } = await import("jsdom");
  const dom = new JSDOM("<!doctype html><html><body></body></html>", { pretendToBeVisual: true, url: "http://localhost/" });
  const { window } = dom;
  globalThis.window = window;
  globalThis.document = window.document;
  globalThis.HTMLElement = window.HTMLElement;
  globalThis.Node = window.Node;
  globalThis.customElements = window.customElements;
  globalThis.HTMLCanvasElement = window.HTMLCanvasElement;
  globalThis.DOMParser = window.DOMParser;
  globalThis.getComputedStyle = window.getComputedStyle.bind(window);
  globalThis.requestAnimationFrame = (fn) => setTimeout(() => fn(Date.now()), 16);
  globalThis.cancelAnimationFrame = (id) => clearTimeout(id);
  if (!window.matchMedia) {
    window.matchMedia = () => ({
      matches: false,
      media: "",
      addListener() {},
      removeListener() {},
      addEventListener() {},
      removeEventListener() {},
      dispatchEvent() { return false; },
    });
  }
  globalThis.matchMedia = window.matchMedia.bind(window);
  if (!globalThis.ResizeObserver) {
    globalThis.ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    };
  }
  if (window.HTMLCanvasElement && window.HTMLCanvasElement.prototype) {
    window.HTMLCanvasElement.prototype.getContext = function () {
      return {
        fillStyle: "",
        fillRect() {},
        clearRect() {},
        fillText() {},
        measureText() { return { width: 0 }; },
      };
    };
  }
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
  const buildRoot = join(labRoot, "build/monaco-layers", id);
  mkdirSync(buildRoot, { recursive: true });

  const upstreamPath = join(buildRoot, "upstream.js");
  const upstreamSource = await extractLayer(id);
  writeFileSync(upstreamPath, upstreamSource);

  const minified = await minifyJqueryBundle(upstreamSource, `${id}.upstream.js`);
  const jsLanes = {
    extracted: { path: upstreamPath, sizes: canonicalCodecSizesForFile(upstreamPath, `monaco-layer ${id} js extract`) },
  };
  for (const [lane, code] of Object.entries(minified)) {
    const path = join(buildRoot, `upstream.${lane}.js`);
    writeFileSync(path, code);
    jsLanes[lane] = {
      path,
      sizes: canonicalCodecSizesForFile(path, `monaco-layer ${id} js ${lane}`),
    };
  }
  const jsBest = pickBest(
    Object.fromEntries(
      Object.entries(jsLanes)
        .filter(([lane]) => lane !== "extracted")
        .map(([lane, row]) => [lane, row.sizes]),
    ),
    "brotli",
  );

  const compiledPath = join(buildRoot, "lilscript.raw.js");
  run(compiler, [
    join(labRoot, layer.lilEntry),
    "--config",
    join(here, "lilscript.toml"),
    "--target",
    "js-module",
    "-o",
    compiledPath,
  ]);
  const compiledSource = readFileSync(compiledPath, "utf8");
  const needsHost = /from\s*["'][^"']*js-host/u.test(compiledSource);
  let lilPath = compiledPath;
  if (needsHost) {
    lilPath = join(buildRoot, "lilscript.bundle.js");
    await esbuild({
      absWorkingDir: join(labRoot, "ports/monaco"),
      entryPoints: [compiledPath],
      outfile: lilPath,
      bundle: true,
      format: "esm",
      platform: "neutral",
      minify: false,
      write: true,
      plugins: [
        {
          name: "monaco-js-host",
          setup(build) {
            build.onResolve({ filter: /js-host/ }, () => ({
              path: join(labRoot, "ports/monaco/js-host.ts"),
            }));
          },
        },
      ],
    });
  }
  const lilSizes = canonicalCodecSizesForFile(lilPath, `monaco-layer ${id} lilscript`);

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
      pass: delta <= 0,
    },
    codecs: canonicalCodecProvenance(`monaco-layer ${id}`),
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
  console.log(
    `  LilScript    raw=${report.lilscript.sizes.raw} gzip=${report.lilscript.sizes.gzip} brotli=${report.lilscript.sizes.brotli}${report.lilscript.bundledHost ? " (host bundled)" : ""}`,
  );
  console.log(
    `  Brotli gate  lil ${report.gate.lilscript} / js ${report.gate.javascript} (${report.javascript.selectedBaseline.lane}) ${sign}${report.gate.delta} ${report.gate.pass ? "PASS" : "FAIL"}`,
  );
}

const ids = process.argv.slice(2).filter((arg) => arg !== "--all");
const selected = process.argv.includes("--all") || ids.length === 0
  ? layers.map((layer) => layer.id)
  : ids;

requireCanonicalCodecRuntime("monaco layer measurement");
const reports = [];
for (const id of selected) {
  reports.push(await measureOne(id));
}
for (const report of reports) {
  printReport(report);
}
const failedVerify = reports.filter((report) => report.verification !== "ok");
if (failedVerify.length) {
  throw new Error(`verification failed: ${failedVerify.map((r) => r.layer).join(", ")}`);
}
const failedSize = reports.filter((report) => !report.gate.pass);
console.log(`\nplanned ladder: ${planned.join(" → ")}`);
if (failedSize.length) {
  console.log(
    `Brotli losses (recorded, not aborting): ${failedSize.map((r) => r.layer).join(", ")}`,
  );
}
writeFileSync(join(labRoot, "build/monaco-layers/summary.json"), JSON.stringify(reports, null, 2) + "\n");
