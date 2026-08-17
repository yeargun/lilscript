import { mkdirSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { build as esbuild } from "esbuild";
import {
  canonicalCodecSizesForFile,
  requireCanonicalCodecRuntime,
} from "../../codec-contract.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const labRoot = resolve(here, "..");
const repoRoot = resolve(labRoot, "../..");
const compiler = process.env.LILSCRIPT
  ? resolve(process.cwd(), process.env.LILSCRIPT)
  : join(repoRoot, "target/release/lilscript");
const appsRoot = join(labRoot, "apps/monaco");
const lilOutDir = join(appsRoot, "lil");
const jsOutDir = join(appsRoot, "js/build");
const measureOut = join(labRoot, "build/monaco-layers");

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

const emptyCssPlugin = {
  name: "empty-assets",
  setup(build) {
    build.onLoad({ filter: /\.(ttf|woff2?|svg)$/ }, () => ({ contents: "", loader: "text" }));
  },
};

const jsHostPlugin = {
  name: "monaco-js-host",
  setup(build) {
    build.onResolve({ filter: /(^|\/)js-host(\.ts)?$/ }, () => ({
      path: join(labRoot, "ports/monaco/js-host.ts"),
    }));
  },
};

mkdirSync(lilOutDir, { recursive: true });
mkdirSync(jsOutDir, { recursive: true });
mkdirSync(measureOut, { recursive: true });

const compiledPath = join(measureOut, "demo-entry.raw.js");
run(compiler, [
  join(labRoot, "ports/monaco/demo-entry.lil"),
  "--config",
  join(labRoot, "ports/monaco/lilscript.toml"),
  "--target",
  "js-module",
  "-o",
  compiledPath,
]);

const lilBundle = join(lilOutDir, "main.js");
await esbuild({
  absWorkingDir: join(labRoot, "ports/monaco"),
  entryPoints: [compiledPath],
  outfile: lilBundle,
  bundle: true,
  format: "esm",
  platform: "neutral",
  minify: false,
  write: true,
  plugins: [jsHostPlugin],
});

const appCompiledPath = join(measureOut, "demo-entry.app.js");
run(compiler, [
  join(labRoot, "ports/monaco/demo-entry.lil"),
  "--config",
  join(labRoot, "ports/monaco/lilscript.app.toml"),
  "--target",
  "js-module",
  "-o",
  appCompiledPath,
]);
const appBundle = join(measureOut, "demo-entry.app.bundle.js");
await esbuild({
  absWorkingDir: join(labRoot, "ports/monaco"),
  entryPoints: [appCompiledPath],
  outfile: appBundle,
  bundle: true,
  format: "esm",
  platform: "neutral",
  minify: false,
  write: true,
  plugins: [jsHostPlugin],
});

const jsEntry = join(jsOutDir, "entry.js");
writeFileSync(
  jsEntry,
  `import { editor } from "monaco-editor-core/esm/vs/editor/editor.api.js";

const SAMPLE = "function hello(name) {\\n  const msg = \\"hi \\" + name;\\n  return msg;\\n}\\n";

export function runDemo(editorHost, diffHost, logEl) {
  const ed = editor.create(editorHost, {
    value: SAMPLE,
    language: "javascript",
    theme: "vs-dark",
    lineNumbers: "on",
    minimap: { enabled: true },
    automaticLayout: true,
  });
  ed.setPosition({ lineNumber: 1, column: 23 });
  ed.trigger("keyboard", "type", { text: "!" });
  ed.trigger("keyboard", "undo", null);
  editor.setTheme("vs-dark");
  ed.layout();
  const found = ed.getModel().findMatches("msg", true, false, true, null, true);
  ed.setPosition({ lineNumber: 2, column: 10 });
  const highlights = ed.getModel().findMatches("msg", true, false, true, null, true);
  const word = ed.getModel().getWordAtPosition(ed.getPosition())?.word ?? "";
  ed.setPosition({ lineNumber: 2, column: 1 });
  ed.trigger("keyboard", "editor.action.commentLine", null);
  const diff = editor.createDiffEditor(diffHost, { automaticLayout: true, renderSideBySide: true });
  diff.setModel({
    original: editor.createModel("a\\nb\\nc\\n", "plaintext"),
    modified: editor.createModel("a\\nx\\nc\\n", "plaintext"),
  });
  const changes = diff.getLineChanges() ?? [];
  logEl.textContent =
    "value=" + ed.getValue().length +
    " matches=" + found.length +
    " folds=n/a" +
    " highlights=" + highlights.length +
    " hover=" + word +
    " diffs=" + changes.length;
}

export { editor };
`,
);

await esbuild({
  absWorkingDir: labRoot,
  entryPoints: [jsEntry],
  outfile: join(jsOutDir, "main.js"),
  bundle: true,
  format: "esm",
  platform: "browser",
  minify: true,
  write: true,
  logOverride: {
    "import-is-undefined": "silent",
    "empty-import-meta": "silent",
  },
  plugins: [emptyCssPlugin],
  loader: { ".css": "empty", ".ttf": "empty" },
});

await esbuild({
  absWorkingDir: labRoot,
  entryPoints: [join(labRoot, "node_modules/monaco-editor-core/esm/vs/editor/common/services/editorWebWorkerMain.js")],
  outfile: join(jsOutDir, "editor.worker.js"),
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

const { JSDOM } = await import("jsdom");
const dom = new JSDOM("<!doctype html><html><body><div id='editor'></div><div id='diff'></div><div id='log'></div></body></html>", {
  pretendToBeVisual: true,
  url: "http://localhost/",
});
globalThis.window = dom.window;
globalThis.document = dom.window.document;
globalThis.HTMLElement = dom.window.HTMLElement;
globalThis.HTMLCanvasElement = dom.window.HTMLCanvasElement;
globalThis.requestAnimationFrame = (fn) => setTimeout(() => fn(Date.now()), 16);
if (dom.window.HTMLCanvasElement?.prototype) {
  dom.window.HTMLCanvasElement.prototype.getContext = function () {
    return { fillStyle: "", fillRect() {}, clearRect() {}, fillText() {}, measureText() { return { width: 0 }; } };
  };
}
const lilMod = await import(`${pathToFileURL(lilBundle).href}?t=${Date.now()}`);
lilMod.runDemo(document.getElementById("editor"), document.getElementById("diff"), document.getElementById("log"));
const log = document.getElementById("log").textContent;
if (!/^value=\d+ matches=\d+ folds=\d+ highlights=\d+ hover=.* diffs=\d+$/.test(log)) {
  throw new Error(`demo log contract failed: ${log}`);
}
if (!/matches=[1-9]/.test(log) || !/folds=[1-9]/.test(log) || !/highlights=[1-9]/.test(log) || !/diffs=[1-9]/.test(log) || !/hover=msg/.test(log)) {
  throw new Error(`demo expected find/fold/highlight/diff/hover work, got ${log}`);
}
writeFileSync(join(appsRoot, "expected.txt"), `${log}\n`);
console.log("demo smoke", log);

requireCanonicalCodecRuntime("monaco demo measurement");
const demoSizes = {
  lilscript: canonicalCodecSizesForFile(appBundle, "monaco demo lilscript app"),
  javascript: canonicalCodecSizesForFile(join(jsOutDir, "main.js"), "monaco demo js editor.api"),
};
writeFileSync(join(measureOut, "demo-sizes.json"), JSON.stringify(demoSizes, null, 2) + "\n");
console.log("lil demo", demoSizes.lilscript);
console.log("js demo", demoSizes.javascript);

run(process.execPath, [join(here, "generate-findings.mjs")]);
