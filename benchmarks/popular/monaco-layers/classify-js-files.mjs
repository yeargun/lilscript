import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { coreEsm, labRoot } from "./file-map.mjs";

function stripComments(src) {
  return src.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/.*$/gm, "");
}

function classifyJs(src) {
  const text = stripComments(src).replace(/^\s*import\s+['"][^'"]+\.css['"]\s*;?\s*$/gm, "").trim();
  if (!text) return "css-or-empty";
  const lines = text.split(/\n/).map((line) => line.trim()).filter(Boolean);
  const code = lines.filter(
    (line) => !/^import\s/.test(line) && !/^export\s+\{[^}]+\}\s+from\s/.test(line) && !/^export\s+\*\s+from\s/.test(line),
  );
  if (code.length === 0) return "reexport-or-imports-only";
  return "runtime";
}

const sizesPath = join(labRoot, "apps/monaco/sizes.json");
const sizes = JSON.parse(readFileSync(sizesPath, "utf8"));
const counts = { runtime: 0, "css-or-empty": 0, "reexport-or-imports-only": 0 };
for (const row of sizes.coreComparison.files) {
  const kind = classifyJs(readFileSync(join(coreEsm, row.monaco), "utf8"));
  row.jsKind = kind;
  counts[kind] += 1;
}
sizes.coreComparison.protocol.jsKind =
  "runtime = file has its own JS; css-or-empty / reexport-or-imports-only are not comparable to a Lil implementation of that widget";
writeFileSync(sizesPath, `${JSON.stringify(sizes, null, 2)}\n`);
console.log(counts);
console.log("wrote", sizesPath);
