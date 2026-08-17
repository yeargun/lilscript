import { mkdirSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build as esbuild } from "esbuild";

const here = dirname(fileURLToPath(import.meta.url));
const labRoot = resolve(here, "..");
const repoRoot = resolve(labRoot, "../..");
const compiler = process.env.LILSCRIPT
  ? resolve(process.cwd(), process.env.LILSCRIPT)
  : join(repoRoot, "target/release/lilscript");
const measureOut = join(labRoot, "build/monaco-layers");
const lilOutDir = join(labRoot, "apps/monaco/lil");
const jsOutDir = join(labRoot, "apps/monaco/js");

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

const jsHostPlugin = {
  name: "monaco-js-host",
  setup(build) {
    build.onResolve({ filter: /js-host/ }, () => ({
      path: join(labRoot, "ports/monaco/js-host.ts"),
    }));
  },
};

mkdirSync(measureOut, { recursive: true });
mkdirSync(lilOutDir, { recursive: true });
mkdirSync(jsOutDir, { recursive: true });

const compiledPath = join(measureOut, "entry.raw.js");
console.log("compiling entry.lil…");
run(compiler, [
  join(labRoot, "ports/monaco/entry.lil"),
  "--config",
  join(labRoot, "ports/monaco/lilscript.toml"),
  "--target",
  "js-module",
  "-o",
  compiledPath,
]);

console.log("bundling LilScript IDE…");
await esbuild({
  absWorkingDir: labRoot,
  entryPoints: [join(lilOutDir, "ide-entry.js")],
  outfile: join(lilOutDir, "ide.js"),
  bundle: true,
  format: "esm",
  platform: "browser",
  minify: false,
  write: true,
  plugins: [jsHostPlugin],
});

console.log("bundling monaco-editor IDE…");
await esbuild({
  absWorkingDir: labRoot,
  entryPoints: [join(jsOutDir, "ide-entry.js")],
  outfile: join(jsOutDir, "ide.js"),
  bundle: true,
  format: "esm",
  platform: "browser",
  minify: true,
  splitting: false,
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
});

const workers = [
  ["editor.worker.js", join(labRoot, "node_modules/monaco-editor/esm/vs/editor/editor.worker.js")],
  ["json.worker.js", join(labRoot, "node_modules/monaco-editor/esm/vs/language/json/json.worker.js")],
  ["css.worker.js", join(labRoot, "node_modules/monaco-editor/esm/vs/language/css/css.worker.js")],
  ["html.worker.js", join(labRoot, "node_modules/monaco-editor/esm/vs/language/html/html.worker.js")],
  ["ts.worker.js", join(labRoot, "node_modules/monaco-editor/esm/vs/language/typescript/ts.worker.js")],
];

for (const [name, entry] of workers) {
  console.log("bundling", name);
  await esbuild({
    absWorkingDir: labRoot,
    entryPoints: [entry],
    outfile: join(jsOutDir, name),
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
}

console.log("IDE bundles ready");
console.log("  Lil", join(lilOutDir, "ide.js"));
console.log("  JS ", join(jsOutDir, "ide.js"));
