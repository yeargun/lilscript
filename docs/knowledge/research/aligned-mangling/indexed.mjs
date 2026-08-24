#!/usr/bin/env node
/* The `a[1]=…; a[2]=…` question, measured.

   If two closures each walk an array with constant indices, does it pay to
   have them agree on the array's name so that `a[1]` recurs instead of
   `a[1]` and `b[1]`? Two numbers answer it:

     - how much of the file is `name[constant]` at all, and how much of that
       is already a repeated (name, index) pair;
     - the illegal ceiling: give every indexed receiver the same name and see
       what the codec pays. Nothing legal can beat that number.

   The ceiling is a gravity probe in the sense this repository already uses:
   it is not a candidate, it bounds a family. */
import { createRequire } from "node:module";
import { gzipSync } from "node:zlib";
const require = createRequire("/Users/yeargun/lilscript/benchmarks/popular/package.json");
const acorn = require("acorn");

export function indexedStats(source) {
  let ast;
  try { ast = acorn.parse(source, { ecmaVersion: 2022, sourceType: "module" }); }
  catch { ast = acorn.parse(source, { ecmaVersion: 2022, sourceType: "script" }); }
  const sites = [];
  const visit = (node) => {
    if (!node || typeof node.type !== "string") return;
    if (node.type === "MemberExpression" && node.computed &&
        node.object.type === "Identifier" && node.property.type === "Literal" &&
        typeof node.property.value === "number") {
      sites.push({
        start: node.object.start, end: node.object.end,
        name: node.object.name, index: node.property.value,
        text: source.slice(node.start, node.end),
      });
    }
    for (const key of Object.keys(node)) {
      if (key === "type" || key === "start" || key === "end") continue;
      const value = node[key];
      if (Array.isArray(value)) { for (const child of value) if (child && child.type) visit(child); }
      else if (value && typeof value.type === "string") visit(value);
    }
  };
  visit(ast);
  const pairs = new Map();
  const names = new Map();
  const indices = new Map();
  for (const site of sites) {
    const key = `${site.name}[${site.index}]`;
    pairs.set(key, (pairs.get(key) || 0) + 1);
    names.set(site.name, (names.get(site.name) || 0) + 1);
    indices.set(site.index, (indices.get(site.index) || 0) + 1);
  }
  const repeated = [...pairs.values()].filter((n) => n > 1);
  return {
    sites: sites.length,
    bytes: sites.reduce((a, s) => a + s.text.length, 0),
    distinctPairs: pairs.size,
    repeatedPairs: repeated.length,
    occurrencesInRepeatedPairs: repeated.reduce((a, b) => a + b, 0),
    distinctReceivers: names.size,
    topPairs: [...pairs.entries()].sort((a, b) => b[1] - a[1]).slice(0, 8),
    topIndices: [...indices.entries()].sort((a, b) => b[1] - a[1]).slice(0, 8),
    sitesForCollapse: sites,
  };
}

/* Illegal ceiling: every indexed receiver becomes one name. */
export function collapseReceivers(source, stats, name = "Z") {
  const edits = [...stats.sitesForCollapse].sort((a, b) => a.start - b.start);
  let out = "", cursor = 0;
  for (const edit of edits) {
    if (edit.start < cursor) continue;
    out += source.slice(cursor, edit.start) + name;
    cursor = edit.end;
  }
  return out + source.slice(cursor);
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const { writeFileSync } = await import("node:fs");
  const { dirname, join } = await import("node:path");
  const { fileURLToPath } = await import("node:url");
  const here = dirname(fileURLToPath(import.meta.url));
  const rows = [];
  const { CORPORA, readCorpus, brotli } = await import("./census.mjs");
  for (const id of Object.keys(CORPORA)) {
    const source = readCorpus(id);
    const stats = indexedStats(source);
    const base = { raw: Buffer.byteLength(source), br11: brotli(source).length,
      gzip9: gzipSync(Buffer.from(source), { level: 9 }).length };
    console.log(`\n${id}  ${stats.sites} \`name[constant]\` sites (${stats.bytes} bytes, ` +
      `${((stats.bytes / base.raw) * 100).toFixed(2)}% of raw) over ${stats.distinctReceivers} receivers`);
    if (!stats.sites) continue;
    console.log(`  ${stats.repeatedPairs} of ${stats.distinctPairs} (name, index) pairs repeat, covering ${stats.occurrencesInRepeatedPairs} sites`);
    console.log(`  most repeated: ${stats.topPairs.map(([k, n]) => `${k}×${n}`).join(" ")}`);
    console.log(`  most used indices: ${stats.topIndices.map(([k, n]) => `[${k}]×${n}`).join(" ")}`);
    const collapsed = collapseReceivers(source, stats);
    const c = { raw: Buffer.byteLength(collapsed), br11: brotli(collapsed).length,
      gzip9: gzipSync(Buffer.from(collapsed), { level: 9 }).length };
    const d = (a, b) => (a - b > 0 ? "+" : "") + (a - b);
    console.log(`  illegal ceiling, every receiver renamed to one letter: raw ${d(c.raw, base.raw)}  ` +
      `gzip ${d(c.gzip9, base.gzip9)}  br11 ${d(c.br11, base.br11)}  (baseline br11 ${base.br11})`);
    rows.push({ id, sites: stats.sites, bytes: stats.bytes, rawShare: stats.bytes / base.raw,
      distinctPairs: stats.distinctPairs, repeatedPairs: stats.repeatedPairs,
      topPairs: stats.topPairs, base,
      ceiling: { raw: c.raw - base.raw, gzip9: c.gzip9 - base.gzip9, br11: c.br11 - base.br11 } });
  }
  writeFileSync(join(here, "indexed.json"), JSON.stringify(rows, null, 1));
  console.log("\nwrote indexed.json");
}
