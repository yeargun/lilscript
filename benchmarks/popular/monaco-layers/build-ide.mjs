import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
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
  labRoot,
  notOneToOne,
  pairs as filePairs,
} from "./file-map.mjs";
import { generateVsTree } from "./generate-vs-tree.mjs";
import {
  jsHostPlugin,
  measurePair,
  scoreProductionFile,
  tsLanguagePlugin,
} from "./measure-pairs.mjs";
import { measureCatalog } from "./measure-catalog.mjs";
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

function skipCompiledString(source, i) {
  const q = source[i];
  i += 1;
  while (i < source.length) {
    if (source[i] === "\\") {
      i += 2;
      continue;
    }
    if (source[i] === q) return i + 1;
    i += 1;
  }
  return i;
}

function skipCompiledWs(source, i) {
  while (i < source.length && /\s/.test(source[i])) i += 1;
  return i;
}

function compiledTopLevelFunctionNames(source) {
  const names = new Set();
  let i = 0;
  let depth = 0;
  while (i < source.length) {
    const c = source[i];
    if (c === "\"" || c === "'" || c === "`") {
      i = skipCompiledString(source, i);
      continue;
    }
    if (depth === 0 && source.startsWith("import", i) && (i === 0 || /[\n;]/.test(source[i - 1]))) {
      while (i < source.length && source[i] !== ";") {
        if (source[i] === "\"" || source[i] === "'" || source[i] === "`") i = skipCompiledString(source, i);
        else i += 1;
      }
      if (source[i] === ";") i += 1;
      continue;
    }
    if (c === "{") {
      depth += 1;
      i += 1;
      continue;
    }
    if (c === "}") {
      if (depth) depth -= 1;
      i += 1;
      continue;
    }
    if (depth === 0 && source.startsWith("function", i) && (i === 0 || /[\n;{}]/.test(source[i - 1]))) {
      let j = i + 8;
      j = skipCompiledWs(source, j);
      const id = /^[A-Za-z_$][\w$]*/.exec(source.slice(j));
      if (id) names.add(id[0]);
      i += 8;
      continue;
    }
    i += 1;
  }
  return names;
}

function compiledLexicalBindings(source) {
  const bindings = [];
  let i = 0;
  let depth = 0;
  while (i < source.length) {
    const c = source[i];
    if (c === "\"" || c === "'" || c === "`") {
      i = skipCompiledString(source, i);
      continue;
    }
    if (depth === 0 && source.startsWith("import", i) && (i === 0 || /[\n;]/.test(source[i - 1]))) {
      while (i < source.length && source[i] !== ";") {
        if (source[i] === "\"" || source[i] === "'" || source[i] === "`") i = skipCompiledString(source, i);
        else i += 1;
      }
      if (source[i] === ";") i += 1;
      continue;
    }
    if (depth === 0 && source.startsWith("export", i) && (i === 0 || /[\n;]/.test(source[i - 1]))) {
      while (i < source.length && source[i] !== ";") {
        if (source[i] === "\"" || source[i] === "'" || source[i] === "`") i = skipCompiledString(source, i);
        else i += 1;
      }
      if (source[i] === ";") i += 1;
      continue;
    }
    if (c === "{") {
      depth += 1;
      i += 1;
      continue;
    }
    if (c === "}") {
      if (depth) depth -= 1;
      i += 1;
      continue;
    }
    if (depth === 0) {
      const kw = /^(?:let|const|var)(?=\s|[A-Za-z_$])/.exec(source.slice(i));
      if (kw && (i === 0 || /[\n;{}]/.test(source[i - 1]))) {
        i += kw[0].length;
        i = skipCompiledWs(source, i);
        while (i < source.length) {
          const id = /^[A-Za-z_$][\w$]*/.exec(source.slice(i));
          if (!id) break;
          const name = id[0];
          const pos = i;
          i += name.length;
          i = skipCompiledWs(source, i);
          if (source[i] === "=") {
            bindings.push({ name, pos });
            i += 1;
            let exprDepth = 0;
            while (i < source.length) {
              const ch = source[i];
              if (ch === "\"" || ch === "'" || ch === "`") {
                i = skipCompiledString(source, i);
                continue;
              }
              if (ch === "{" || ch === "(" || ch === "[") exprDepth += 1;
              else if ((ch === "}" || ch === ")" || ch === "]") && exprDepth) exprDepth -= 1;
              else if (exprDepth === 0 && (ch === "," || ch === ";")) break;
              i += 1;
            }
          }
          i = skipCompiledWs(source, i);
          if (source[i] === ",") {
            i = skipCompiledWs(source, i + 1);
            continue;
          }
          break;
        }
        continue;
      }
    }
    i += 1;
  }
  return bindings;
}

function sanitizeCompiledModule(source) {
  const fnNames = compiledTopLevelFunctionNames(source);
  const bindings = compiledLexicalBindings(source);
  const taken = new Set([...fnNames, ...bindings.map((b) => b.name)]);
  const collisions = bindings.filter((b) => fnNames.has(b.name));
  collisions.sort((a, b) => b.pos - a.pos);
  let out = source;
  for (const binding of collisions) {
    let next = `${binding.name}$`;
    let n = 1;
    while (taken.has(next)) {
      n += 1;
      next = `${binding.name}$${n}`;
    }
    taken.add(next);
    const exportAt = Math.max(out.lastIndexOf("export {"), out.lastIndexOf("export{"));
    const before = out.slice(0, binding.pos);
    const midEnd = exportAt > binding.pos ? exportAt : out.length;
    const ident = new RegExp(`(?<![A-Za-z0-9_$])${binding.name}(?![A-Za-z0-9_$])`, "g");
    const mid = out.slice(binding.pos, midEnd).replace(ident, next);
    const after = out.slice(midEnd);
    out = before + mid + after;
  }
  return out;
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

if (process.argv.includes("--runtime-only")) {
  const entryRaw = join(measureOut, "entry.raw.js");
  console.log("compiling entry.lil (runtime only)…");
  run(compiler, [
    join(labRoot, "ports/monaco/entry.lil"),
    "--config",
    join(labRoot, "ports/monaco/lilscript.toml"),
    "--target",
    "js-module",
    "-o",
    entryRaw,
  ]);
  writeFileSync(entryRaw, sanitizeCompiledModule(readFileSync(entryRaw, "utf8")));
  const previous = readFileSync(join(lilOutDir, "ide.js"), "utf8");
  const catalogAt = previous.indexOf("\nglobalThis.__lilMonacoCatalog");
  console.log("bundling LilScript monaco IDE…");
  await esbuild({
    ...monacoBundle,
    entryPoints: [join(lilOutDir, "ide-entry.js")],
    outfile: join(lilOutDir, "ide.js"),
    plugins: [jsHostPlugin, tsLanguagePlugin],
  });
  if (catalogAt >= 0) {
    writeFileSync(join(lilOutDir, "ide.js"), `${readFileSync(join(lilOutDir, "ide.js"), "utf8")}${previous.slice(catalogAt)}`);
  }
  const jsTs = join(jsOutDir, "ts.worker.js");
  const lilTs = join(lilOutDir, "ts.worker.js");
  if (!existsSync(jsTs)) {
    await esbuild({
      absWorkingDir: labRoot,
      entryPoints: [join(labRoot, "node_modules/monaco-editor/esm/vs/language/typescript/ts.worker.js")],
      outfile: jsTs,
      bundle: true,
      format: "iife",
      platform: "browser",
      minify: true,
      write: true,
      logOverride: { "import-is-undefined": "silent", "empty-import-meta": "silent" },
      loader: { ".css": "empty", ".ttf": "empty" },
    });
  }
  copyFileSync(jsTs, lilTs);
  writeFileSync(
    join(lilOutDir, "monaco-env.js"),
    `self.MonacoEnvironment = {
  getWorker(_id, label) {
    const file =
      label === "json"
        ? "json.worker.js"
        : label === "css" || label === "scss" || label === "less"
          ? "css.worker.js"
          : label === "html" || label === "handlebars" || label === "razor"
            ? "html.worker.js"
            : label === "typescript" || label === "javascript"
              ? "ts.worker.js"
              : "editor.worker.js";
    return new Worker(file, { name: label });
  },
};
`,
  );
  console.log("runtime-only IDE ready", join(lilOutDir, "ide.js"));
  process.exit(0);
}

requireCanonicalCodecRuntime("monaco production IDE");

console.log("generating 1:1 vs/ catalog…");
const vsCatalog = generateVsTree();
console.log(`  ${vsCatalog.ported} implemented, ${vsCatalog.shim} shim, ${vsCatalog.thin} thin, ${vsCatalog.stub} stub`);

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
writeFileSync(entryRaw, sanitizeCompiledModule(readFileSync(entryRaw, "utf8")));

console.log("measuring monaco-editor-core catalog (994 JS files + scored Lil)…");
const coreComparison = await measureCatalog(vsCatalog, join(measureOut, "catalog"));
console.log(
  `  catalog js br=${coreComparison.totals.js.brotli.toLocaleString("en-US")}  lil modules=${coreComparison.scoredLil}  br=${coreComparison.totals.lil ? coreComparison.totals.lil.brotli.toLocaleString("en-US") : "—"}`,
);

console.log("bundling LilScript monaco IDE…");
await esbuild({
  ...monacoBundle,
  entryPoints: [join(lilOutDir, "ide-entry.js")],
  outfile: join(lilOutDir, "ide.js"),
  plugins: [jsHostPlugin, tsLanguagePlugin],
});
const catalogPack = join(measureOut, "catalog-pack-entry.js");
writeFileSync(
  join(lilOutDir, "ide.js"),
  `${readFileSync(join(lilOutDir, "ide.js"), "utf8")}\n${readFileSync(catalogPack, "utf8")}`,
);

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

const lilWorkers = [
  ["editor.worker.js", "workers/editor.worker.lil", "handleEditorWorker"],
  ["json.worker.js", "workers/json.worker.lil", "handleJsonWorker"],
  ["css.worker.js", "workers/css.worker.lil", "handleCssWorker"],
  ["html.worker.js", "workers/html.worker.lil", "handleHtmlWorker"],
];
const lilWorkerSizes = [];
for (const [name, lilRel, handler] of lilWorkers) {
  console.log("compiling Lil", name);
  const raw = join(measureOut, name.replace(".js", ".raw.js"));
  run(compiler, [
    join(labRoot, "ports/monaco", lilRel),
    "--config",
    join(labRoot, "ports/monaco/lilscript.toml"),
    "--target",
    "js-module",
    "-o",
    raw,
  ]);
  const boot = join(measureOut, name.replace(".js", ".boot.js"));
  writeFileSync(
    boot,
    `import * as lil from ${JSON.stringify(raw)};
self.onmessage = (e) => {
  const msg = e.data || {};
  const method = String(msg.method || msg.type || "ping");
  const uri = String(msg.uri || "");
  const value = String(msg.value || msg.prefix || "");
  let res = "pong";
  try {
    const fn = lil.${handler};
    if (typeof fn === "function") res = fn(method, uri, value);
  } catch (err) {
    self.postMessage({ id: msg.id ?? msg.req, err: String(err) });
    return;
  }
  self.postMessage({ id: msg.id ?? msg.req, res });
};
`,
  );
  const out = join(lilOutDir, name);
  await esbuild({
    absWorkingDir: labRoot,
    entryPoints: [boot],
    outfile: out,
    bundle: true,
    format: "iife",
    platform: "browser",
    minify: true,
    write: true,
    plugins: [jsHostPlugin],
    logOverride: { "import-is-undefined": "silent" },
  });
  lilWorkerSizes.push({
    name,
    sizes: scoreProductionFile(out, `lilscript worker ${name}`),
  });
}

copyFileSync(join(jsOutDir, "ts.worker.js"), join(lilOutDir, "ts.worker.js"));
lilWorkerSizes.push({
  name: "ts.worker.js",
  sizes: scoreProductionFile(join(lilOutDir, "ts.worker.js"), "microsoft ts.worker on lil page"),
});

writeFileSync(
  join(lilOutDir, "monaco-env.js"),
  `self.MonacoEnvironment = {
  getWorker(_id, label) {
    const file =
      label === "json"
        ? "json.worker.js"
        : label === "css" || label === "scss" || label === "less"
          ? "css.worker.js"
          : label === "html" || label === "handlebars" || label === "razor"
            ? "html.worker.js"
            : label === "typescript" || label === "javascript"
              ? "ts.worker.js"
              : "editor.worker.js";
    return new Worker(file, { name: label });
  },
};
`,
);

console.log("scoring production artifacts…");
const jsIde = scoreProductionFile(join(jsOutDir, "ide.js"), "monaco js ide.js");
const lilIde = scoreProductionFile(join(lilOutDir, "ide.js"), "lilscript monaco ide.js");
const cssPath = join(labRoot, "node_modules/monaco-editor/min/vs/editor/editor.main.css");
const cssSizes = scoreProductionFile(cssPath, "monaco editor.main.css");
const lilCssPath = existsSync(join(lilOutDir, "monaco.css"))
  ? join(lilOutDir, "monaco.css")
  : join(appsRoot, "lil-editor.css");
const lilCss = scoreProductionFile(lilCssPath, "lilscript monaco.css (official editor.main + generated themes + extras)");
const workersRaw = workerSizes.reduce((n, row) => n + row.sizes.raw, 0);
const workersBrotli = workerSizes.reduce((n, row) => n + row.sizes.brotli, 0);
const workersGzip = workerSizes.reduce((n, row) => n + row.sizes.gzip, 0);
const lilWorkersRaw = lilWorkerSizes.reduce((n, row) => n + row.sizes.raw, 0);
const lilWorkersBrotli = lilWorkerSizes.reduce((n, row) => n + row.sizes.brotli, 0);
const lilWorkersGzip = lilWorkerSizes.reduce((n, row) => n + row.sizes.gzip, 0);

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

console.log("writing sizes.json…");
const provenance = canonicalCodecProvenance("monaco production IDE");
const sizes = {
  versions: {
    monacoEditor: monacoEditorVersion,
    monacoEditorCore: monacoEditorCoreVersion,
    vscodeCommit: vscodeCommitId,
  },
  protocol: {
    productionJs: "esbuild minify of monaco-editor ESM + workers, then lilscript-codec gzip-9 / brotli-11",
    productionLil: "lilscript compile of entry.lil runtime + 992 monaco-editor-core catalog modules (js-module keepers retain exported class methods), official Microsoft ts.worker, tree-shaken js-host once, workbench, esbuild minify, lilscript-codec",
    moduleJs: "listed monaco-editor-core files only; other monaco imports external; esbuild minify; lilscript-codec",
    moduleLil: "independently compiled catalog .lil with js-module keepers (exported class methods retained); js-host left external per file; lilscript-codec",
    moduleJsMinifiers: "esbuild and terser; best Brotli wins",
  },
  codec: {
    implementation: provenance.implementation,
    gzip: provenance.gzip9.libraryVersion,
    brotli: provenance.brotli11.libraryVersion,
  },
  coreJsFiles: vsCatalog.coreCount,
  catalog: {
    mapped: vsCatalog.mapped,
    ported: vsCatalog.ported,
    shim: vsCatalog.shim,
    thin: vsCatalog.thin,
    stub: vsCatalog.stub,
    extern: vsCatalog.extern,
    remaining: vsCatalog.remaining,
    workers: vsCatalog.workers.length,
  },
  plugged: ["entire LilScript editor (entry.lil) plus monaco-editor-core catalog pack — no monaco-editor JS"],
  production: {
    js: { ide: jsIde, workers: { raw: workersRaw, gzip: workersGzip, brotli: workersBrotli }, css: cssSizes },
    lil: {
      ide: lilIde,
      workers: { raw: lilWorkersRaw, gzip: lilWorkersGzip, brotli: lilWorkersBrotli },
      css: lilCss,
    },
    workers: { raw: workersRaw, gzip: workersGzip, brotli: workersBrotli, files: workerSizes },
    lilWorkers: { raw: lilWorkersRaw, gzip: lilWorkersGzip, brotli: lilWorkersBrotli, files: lilWorkerSizes },
    css: cssSizes,
  },
  pairs: pairReports,
  coreComparison,
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
console.log(`  lil workers    raw=${fmt(lilWorkersRaw)}  gzip=${fmt(lilWorkersGzip)}  br=${fmt(lilWorkersBrotli)}`);
console.log(`  catalog        ${vsCatalog.ported} implemented / ${vsCatalog.shim} shim / ${vsCatalog.thin} thin / ${vsCatalog.stub} stub`);
for (const row of pairReports) {
  if (!row.js) {
    console.log(`  pair ${row.id}  (not scored — ${row.note})`);
    continue;
  }
  console.log(
    `  pair ${row.id}  js br=${fmt(row.js.sizes.brotli)} (${row.js.lane})  lil br=${fmt(row.lil.sizes.brotli)}  Δ=${row.delta.brotli}`,
  );
}
