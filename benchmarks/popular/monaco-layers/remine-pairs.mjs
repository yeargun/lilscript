import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { canonicalCodecSizesForFile } from "../../codec-contract.mjs";
import { minifyJqueryBundle } from "../jquery-measurement-lanes.mjs";
import { labRoot } from "./file-map.mjs";

const sizesPath = join(labRoot, "apps/monaco/sizes.json");
const pairsRoot = join(labRoot, "build/monaco-layers/pairs");
const sizes = JSON.parse(readFileSync(sizesPath, "utf8"));

for (const pair of sizes.pairs ?? []) {
  const bundle = join(pairsRoot, pair.id, "js/javascript.bundle.js");
  if (!existsSync(bundle)) continue;
  const source = readFileSync(bundle, "utf8");
  if (!source) continue;
  const minified = await minifyJqueryBundle(source, pair.id);
  const lanes = {};
  let best = null;
  for (const [lane, code] of Object.entries(minified)) {
    const out = join(pairsRoot, pair.id, "js/minify", `${pair.id}.${lane}.js`);
    writeFileSync(out, code);
    const scored = canonicalCodecSizesForFile(out, `pair ${pair.id} ${lane}`);
    lanes[lane] = scored;
    if (!best || scored.brotli < best.sizes.brotli) best = { lane, sizes: scored };
  }
  pair.js = {
    ...pair.js,
    lane: best.lane,
    sizes: best.sizes,
    lanes,
  };
  if (pair.lil?.sizes) {
    pair.delta = { brotli: pair.lil.sizes.brotli - best.sizes.brotli };
  }
  console.log(
    `${pair.id.padEnd(16)} oxc=${lanes.oxc.brotli} esbuild=${lanes.esbuild.brotli} terser=${lanes.terser.brotli} best=${best.lane}`,
  );
}

const empty = { raw: 0, gzip: 0, brotli: 0 };
const lanes = ["oxc", "esbuild", "terser"];
for (const row of sizes.coreComparison.files) {
  if (row.jsLanes) continue;
  row.js = empty;
  row.jsLanes = { oxc: empty, esbuild: empty, terser: empty };
  row.jsMinifier = "vite/oxc";
}

function add(into, sizesRow) {
  into.raw += sizesRow.raw;
  into.gzip += sizesRow.gzip;
  into.brotli += sizesRow.brotli;
}

const totals = sizes.coreComparison.totals;
totals.js = { raw: 0, gzip: 0, brotli: 0 };
totals.jsLanes = { oxc: { raw: 0, gzip: 0, brotli: 0 }, esbuild: { raw: 0, gzip: 0, brotli: 0 }, terser: { raw: 0, gzip: 0, brotli: 0 } };
for (const row of sizes.coreComparison.files) {
  if (row.js) add(totals.js, row.js);
  if (row.jsLanes) {
    for (const lane of lanes) add(totals.jsLanes[lane], row.jsLanes[lane]);
  }
}
totals.js = { ...totals.jsLanes.oxc };

writeFileSync(sizesPath, `${JSON.stringify(sizes, null, 2)}\n`);
console.log("catalog totals", totals.jsLanes);
console.log("wrote", sizesPath);
