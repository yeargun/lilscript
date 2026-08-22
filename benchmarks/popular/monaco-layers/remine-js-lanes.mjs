import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build as esbuild } from "esbuild";
import {
  canonicalCodecMeasurementsForFiles,
  canonicalCodecSizesForFile,
} from "../../codec-contract.mjs";
import { minifyJqueryBundle } from "../jquery-measurement-lanes.mjs";
import { labRoot } from "./file-map.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const require = createRequire(import.meta.url);
const appsRoot = join(labRoot, "apps/monaco");
const catalogJs = join(labRoot, "build/monaco-layers/catalog/js");
const laneRoot = join(labRoot, "build/monaco-layers/catalog/js-lanes");
const sizesPath = join(appsRoot, "sizes.json");

function folderOf(rel) {
  const parts = rel.split("/");
  if (parts.length < 2) return parts[0] || rel;
  if (parts[0] === "editor" && parts[1] === "contrib") {
    return parts.slice(0, 3).join("/");
  }
  return parts.slice(0, 2).join("/");
}

function catalogId(monaco) {
  return monaco.replace(/[^\w./-]+/g, "_").split("/").join("__");
}

function emptySizes() {
  return { raw: 0, gzip: 0, brotli: 0 };
}

function addSizes(into, sizes) {
  into.raw += sizes.raw;
  into.gzip += sizes.gzip;
  into.brotli += sizes.brotli;
}

function pkgVersion(name) {
  try {
    return require(`${name}/package.json`).version;
  } catch {
    return null;
  }
}

mkdirSync(laneRoot, { recursive: true });

const sizes = JSON.parse(readFileSync(sizesPath, "utf8"));
const files = sizes.coreComparison.files;
const tooling = {
  vite: pkgVersion("vite"),
  rolldown: pkgVersion("rolldown"),
  esbuild: pkgVersion("esbuild"),
  terser: pkgVersion("terser"),
  oxc: `vite@${pkgVersion("vite")} minify (Oxc, Vite 8 default)`,
};

console.log("JS minifier versions", tooling);

const prepared = [];
let i = 0;
for (const row of files) {
  i += 1;
  const id = catalogId(row.monaco);
  const bundle = join(catalogJs, `${id}.bundle.js`);
  if (!existsSync(bundle)) {
    prepared.push({ row, ok: false, reason: "missing bundle" });
    continue;
  }
  if (i % 100 === 0 || i === files.length) {
    console.log(`  minify ${i}/${files.length}`);
  }
  try {
    const source = readFileSync(bundle, "utf8");
    if (source.length === 0) {
      prepared.push({ row, ok: false, reason: "empty bundle" });
      continue;
    }
    const minified = await minifyJqueryBundle(source, row.monaco);
    const paths = {};
    for (const [lane, code] of Object.entries(minified)) {
      const out = join(laneRoot, `${id}.${lane}.js`);
      writeFileSync(out, code);
      paths[lane] = out;
    }
    prepared.push({ row, ok: true, paths });
  } catch (err) {
    console.error(`  minify ${row.monaco}: ${err.message.split("\n")[0]}`);
    prepared.push({ row, ok: false, reason: err.message });
  }
}

const lanes = ["oxc", "esbuild", "terser"];
const measured = {};
for (const lane of lanes) {
  const paths = prepared.filter((item) => item.ok).map((item) => item.paths[lane]);
  console.log(`scoring ${lane} (${paths.length} files)…`);
  measured[lane] = canonicalCodecMeasurementsForFiles(paths, `monaco catalog JS ${lane}`);
}

const byMonaco = new Map();
let mi = 0;
for (const item of prepared) {
  if (!item.ok) continue;
  const lanesSizes = {};
  for (const lane of lanes) {
    const sizesRow = measured[lane][mi];
    lanesSizes[lane] = { raw: sizesRow.raw, gzip: sizesRow.gzip, brotli: sizesRow.brotli };
  }
  byMonaco.set(item.row.monaco, lanesSizes);
  mi += 1;
}

const nextFiles = files.map((row) => {
  const lanesSizes = byMonaco.get(row.monaco);
  if (!lanesSizes) return row;
  return {
    ...row,
    js: lanesSizes.oxc,
    jsLanes: lanesSizes,
    jsMinifier: "vite/oxc",
  };
});

const totals = {
  key: "monaco-editor-core",
  files: nextFiles.length,
  scoredLil: sizes.coreComparison.totals.scoredLil,
  js: emptySizes(),
  lil: sizes.coreComparison.totals.lil,
  jsLanes: { oxc: emptySizes(), esbuild: emptySizes(), terser: emptySizes() },
};
const grouped = new Map();
for (const row of nextFiles) {
  const list = grouped.get(row.folder ?? folderOf(row.monaco)) ?? [];
  list.push(row);
  grouped.set(row.folder ?? folderOf(row.monaco), list);
  if (row.js) addSizes(totals.js, row.js);
  if (row.jsLanes) {
    for (const lane of lanes) addSizes(totals.jsLanes[lane], row.jsLanes[lane]);
  }
}

const folders = [];
for (const [key, rows] of grouped) {
  const folder = {
    key,
    files: rows.length,
    scoredLil: rows.filter((row) => row.lil && row.lil.unique !== false).length,
    js: emptySizes(),
    lil: emptySizes(),
    jsLanes: { oxc: emptySizes(), esbuild: emptySizes(), terser: emptySizes() },
    jsMinifier: "vite/oxc",
  };
  let lilCount = 0;
  for (const row of rows) {
    if (row.js) addSizes(folder.js, row.js);
    if (row.jsLanes) {
      for (const lane of lanes) addSizes(folder.jsLanes[lane], row.jsLanes[lane]);
    }
    if (row.lil && row.lil.unique !== false) {
      addSizes(folder.lil, row.lil);
      lilCount += 1;
    }
  }
  if (!lilCount) folder.lil = null;
  folders.push(folder);
}
folders.sort((a, b) => b.js.brotli - a.js.brotli);

console.log("rebundling unminified JS ide…");
const rawIde = join(labRoot, "build/monaco-layers/js-ide.raw.js");
await esbuild({
  absWorkingDir: labRoot,
  entryPoints: [join(appsRoot, "js/ide-entry.js")],
  outfile: rawIde,
  bundle: true,
  format: "esm",
  platform: "browser",
  minify: false,
  write: true,
  logOverride: { "import-is-undefined": "silent", "empty-import-meta": "silent" },
  loader: { ".ttf": "file", ".woff": "file", ".woff2": "file", ".css": "empty" },
});
const ideSource = readFileSync(rawIde, "utf8");
const ideMinified = await minifyJqueryBundle(ideSource, "ide.js");
const productionLanes = {};
for (const [lane, code] of Object.entries(ideMinified)) {
  const out = join(labRoot, "build/monaco-layers", `js-ide.${lane}.js`);
  writeFileSync(out, code);
  productionLanes[lane] = canonicalCodecSizesForFile(out, `monaco js ide.js ${lane}`);
}

sizes.coreComparison = {
  ...sizes.coreComparison,
  protocol: {
    js: "each monaco-editor-core ESM file; other monaco imports external; Vite 8 Oxc / esbuild / Terser minify; headline is Vite/Oxc; lilscript-codec gzip-9 / brotli-11",
    lil: sizes.coreComparison.protocol.lil,
    jsMinifiers: "vite/oxc (Vite 8 default), esbuild, terser; scored separately",
  },
  tooling,
  totals: {
    ...totals,
    scoredJs: byMonaco.size,
  },
  folders,
  files: nextFiles,
};

sizes.production.jsMinifiers = productionLanes;
sizes.production.js.ide = productionLanes.oxc;
sizes.protocol.productionJs =
  "Vite 8 Oxc minify of monaco-editor ESM workbench (esbuild and Terser lanes also scored), then lilscript-codec gzip-9 / brotli-11";
sizes.protocol.moduleJs =
  "listed monaco-editor-core files only; other monaco imports external; Vite 8 Oxc / esbuild / Terser; headline Vite/Oxc; lilscript-codec";
sizes.protocol.moduleJsMinifiers = "vite/oxc, esbuild, terser; all three scored";

writeFileSync(sizesPath, `${JSON.stringify(sizes, null, 2)}\n`);
console.log("catalog JS Brotli");
for (const lane of lanes) {
  console.log(`  ${lane.padEnd(8)} ${totals.jsLanes[lane].brotli.toLocaleString("en-US")}`);
}
console.log("ide.js Brotli");
for (const lane of lanes) {
  console.log(`  ${lane.padEnd(8)} ${productionLanes[lane].brotli.toLocaleString("en-US")}`);
}
console.log("wrote", sizesPath);
