import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { compileLilPair } from "./measure-pairs.mjs";
import { labRoot, pairs } from "./file-map.mjs";

const sizesPath = join(labRoot, "apps/monaco/sizes.json");
const sizes = JSON.parse(readFileSync(sizesPath, "utf8"));
const byId = new Map(pairs.map((pair) => [pair.id, pair]));

for (const row of sizes.pairs ?? []) {
  if (!row.lil) continue;
  const pair = byId.get(row.id);
  if (!pair) continue;
  const lil = await compileLilPair(pair, join(labRoot, "build/monaco-layers/pairs", pair.id, "lil"));
  row.lil = {
    path: lil.path.replace(`${labRoot}/`, "benchmarks/popular/"),
    bundledHost: lil.bundledHost,
    sizes: lil.sizes,
    keeper: true,
  };
  if (row.js?.sizes) {
    row.delta = { brotli: lil.sizes.brotli - row.js.sizes.brotli };
  }
  const jsOxc = row.js?.lanes?.oxc?.brotli ?? row.js?.sizes?.brotli;
  console.log(
    `${pair.id.padEnd(16)} lil=${lil.sizes.brotli} jsOxc=${jsOxc} ratio=${
      jsOxc ? (lil.sizes.brotli / jsOxc).toFixed(2) : "—"
    }×`,
  );
}

writeFileSync(sizesPath, `${JSON.stringify(sizes, null, 2)}\n`);
console.log("wrote", sizesPath);
