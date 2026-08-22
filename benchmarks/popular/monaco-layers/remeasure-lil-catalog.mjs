import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { canonicalCodecMeasurementsForFiles } from "../../codec-contract.mjs";
import { labRoot, lilPath, portsRoot } from "./file-map.mjs";
import { runCompiler } from "./measure-pairs.mjs";
import { writeKeepFile } from "./catalog-keep.mjs";

function isHeavyLil(src) {
  return /from ["'][^"']*(?:editor\/view|editor\/standalone|text-model|monaco-api|editor\/commands|contrib\/runtime|contrib\/popular)["']/.test(src);
}

function addSizes(into, sizes) {
  into.raw += sizes.raw;
  into.gzip += sizes.gzip;
  into.brotli += sizes.brotli;
}

function emptySizes() {
  return { raw: 0, gzip: 0, brotli: 0 };
}

const sizesPath = join(labRoot, "apps/monaco/sizes.json");
const sizes = JSON.parse(readFileSync(sizesPath, "utf8"));
const keepRoot = join(labRoot, "build/monaco-layers/catalog/keep");
const lilDir = join(labRoot, "build/monaco-layers/catalog/lil");
mkdirSync(lilDir, { recursive: true });

const artifacts = [];
let i = 0;
const files = sizes.coreComparison.files;
for (const row of files) {
  i += 1;
  if (i % 50 === 0 || i === files.length) {
    console.log(`  lil ${i}/${files.length} kept=${artifacts.filter((a) => a.ok).length}`);
  }
  if (row.status !== "ported" || !row.lilPath) {
    artifacts.push({ row, ok: false });
    continue;
  }
  const src = readFileSync(join(portsRoot, row.lilPath), "utf8");
  if (isHeavyLil(src)) {
    artifacts.push({ row, ok: false, reason: "heavy" });
    continue;
  }
  const id = row.monaco.replace(/[^\w./-]+/g, "_").split("/").join("__");
  const outDir = join(lilDir, id);
  mkdirSync(outDir, { recursive: true });
  const compiledPath = join(outDir, "lilscript.raw.js");
  const lilAbs = lilPath(row.lilPath);
  const keepAbs = join(keepRoot, row.lilPath.replace(/\.lil$/, ".keep.lil"));
  const keep = writeKeepFile(src, lilAbs, keepAbs);
  try {
    runCompiler(keep || lilAbs, compiledPath);
    artifacts.push({ row, artifact: compiledPath, ok: true });
  } catch {
    if (!keep) {
      artifacts.push({ row, ok: false });
      continue;
    }
    try {
      runCompiler(lilAbs, compiledPath);
      artifacts.push({ row, artifact: compiledPath, ok: true });
    } catch {
      artifacts.push({ row, ok: false });
    }
  }
}

const measured = artifacts.filter((item) => item.ok).length
  ? canonicalCodecMeasurementsForFiles(
    artifacts.filter((item) => item.ok).map((item) => item.artifact),
    "monaco catalog Lil keepers",
  )
  : [];

let mi = 0;
const byMonaco = new Map();
for (const item of artifacts) {
  if (!item.ok) continue;
  const sizesRow = measured[mi];
  byMonaco.set(item.row.monaco, {
    path: item.row.lilPath,
    raw: sizesRow.raw,
    gzip: sizesRow.gzip,
    brotli: sizesRow.brotli,
    unique: item.row.lil?.unique !== false,
  });
  mi += 1;
}

for (const row of files) {
  const lil = byMonaco.get(row.monaco);
  if (lil) row.lil = lil;
}

const totals = {
  ...sizes.coreComparison.totals,
  lil: emptySizes(),
  scoredLil: 0,
};
const grouped = new Map();
for (const row of files) {
  const key = row.folder;
  const list = grouped.get(key) ?? [];
  list.push(row);
  grouped.set(key, list);
  if (row.lil && row.lil.unique !== false) {
    addSizes(totals.lil, row.lil);
    totals.scoredLil += 1;
  }
}

const folders = [];
for (const [key, rows] of grouped) {
  const folder = sizes.coreComparison.folders.find((row) => row.key === key) ?? {
    key,
    files: rows.length,
    js: emptySizes(),
    jsLanes: { oxc: emptySizes(), esbuild: emptySizes(), terser: emptySizes() },
  };
  folder.files = rows.length;
  folder.scoredLil = rows.filter((row) => row.lil && row.lil.unique !== false).length;
  folder.lil = emptySizes();
  let lilCount = 0;
  for (const row of rows) {
    if (row.lil && row.lil.unique !== false) {
      addSizes(folder.lil, row.lil);
      lilCount += 1;
    }
  }
  if (!lilCount) folder.lil = null;
  folders.push(folder);
}
folders.sort((a, b) => (b.js?.brotli ?? 0) - (a.js?.brotli ?? 0));

sizes.coreComparison.totals = totals;
sizes.coreComparison.folders = folders;
sizes.coreComparison.protocol.lil =
  "independently compiled .lil as js-module; keepers retain exported class constructors/methods whose types are in-file or imported; private callees stay if reachable; js-host external; relative .lil imports are linked";
writeFileSync(sizesPath, `${JSON.stringify(sizes, null, 2)}\n`);
console.log("catalog Lil Brotli", totals.lil.brotli);
console.log("wrote", sizesPath);
