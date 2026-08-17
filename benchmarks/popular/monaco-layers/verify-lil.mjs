import { spawnSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { build as esbuild } from "esbuild";
import { JSDOM } from "jsdom";
import { layerById, layers } from "./catalog.mjs";
import { extractLayer } from "./extract-upstream.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const labRoot = resolve(here, "..");
const repoRoot = resolve(labRoot, "../..");
const compiler = join(repoRoot, "target/release/lilscript");

function ensureDom() {
  if (globalThis.document) {
    return;
  }
  const { window } = new JSDOM("<!doctype html><html><body></body></html>", {
    pretendToBeVisual: true,
    url: "http://localhost/",
  });
  globalThis.window = window;
  globalThis.document = window.document;
  globalThis.HTMLElement = window.HTMLElement;
  globalThis.Node = window.Node;
  globalThis.HTMLCanvasElement = window.HTMLCanvasElement;
  globalThis.DOMParser = window.DOMParser;
  globalThis.getComputedStyle = window.getComputedStyle.bind(window);
  globalThis.requestAnimationFrame = (fn) => setTimeout(() => fn(Date.now()), 16);
  globalThis.cancelAnimationFrame = (id) => clearTimeout(id);
  if (window.HTMLCanvasElement?.prototype) {
    window.HTMLCanvasElement.prototype.getContext = function () {
      return {
        fillStyle: "",
        fillRect() {},
        clearRect() {},
        fillText() {},
        measureText() {
          return { width: 0 };
        },
      };
    };
  }
}

function compileLayer(id) {
  const layer = layerById(id);
  const buildRoot = join(labRoot, "build/monaco-layers", id);
  mkdirSync(buildRoot, { recursive: true });
  const compiledPath = join(buildRoot, "lilscript.raw.js");
  const result = spawnSync(
    compiler,
    [
      join(labRoot, layer.lilEntry),
      "--config",
      join(here, "lilscript.toml"),
      "--target",
      "js-module",
      "-o",
      compiledPath,
    ],
    { cwd: labRoot, encoding: "utf8", maxBuffer: 32 * 1024 * 1024 },
  );
  if (result.status !== 0) {
    throw new Error(`${id} compile failed\n${result.stdout}${result.stderr}`);
  }
  return { layer, buildRoot, compiledPath };
}

async function bundleHost(compiledPath, buildRoot) {
  const lilPath = join(buildRoot, "lilscript.bundle.js");
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
          build.onResolve({ filter: /(^|\/)js-host(\.ts)?$/ }, () => ({
            path: join(labRoot, "ports/monaco/js-host.ts"),
          }));
        },
      },
    ],
  });
  return lilPath;
}

const needsJs = new Set(["base-lifecycle", "core-types", "piece-tree"]);
const needsDom = new Set([
  "view-render",
  "input-commands",
  "standalone-api",
  "monarch-popular",
  "popular-contrib",
  "remaining-contrib",
  "remaining-monarch",
  "json-css-html-ls",
]);

const ids = process.argv.slice(2);
const selected = ids.length ? ids : layers.map((layer) => layer.id);

for (const id of selected) {
  const started = Date.now();
  const { layer, buildRoot, compiledPath } = compileLayer(id);
  const lilPath = await bundleHost(compiledPath, buildRoot);
  if (needsDom.has(id)) {
    ensureDom();
  }
  const verifyModule = await import(`${pathToFileURL(join(labRoot, layer.verify)).href}?t=${Date.now()}`);
  const lil = await import(`${pathToFileURL(lilPath).href}?t=${Date.now()}`);
  if (needsJs.has(id)) {
    const upstreamPath = join(buildRoot, "upstream.js");
    const { writeFileSync } = await import("node:fs");
    writeFileSync(upstreamPath, await extractLayer(id));
    const js = await import(`${pathToFileURL(upstreamPath).href}?t=${Date.now()}`);
    await verifyModule.verify(lil, js);
  } else {
    await verifyModule.verify(lil);
  }
  console.log(`ok ${id} ${Date.now() - started}ms`);
}
