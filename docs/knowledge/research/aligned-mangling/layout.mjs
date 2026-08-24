#!/usr/bin/env node
/* Emission order as a codec knob.

   The census says distance codes are the largest single consumer of bits in
   these streams (47–65% on the jQuery family), and that a fifth to a third of
   full distance codes land within 4–64 bytes of a distance the decoder
   already has cached. Both numbers are about *where* things sit relative to
   each other, which emission order controls for free.

   Function declarations in one body are hoisted, so their textual order does
   not change the program. This permutes them among the slots they already
   occupy — never across a statement, never when two share a name. */
import { createRequire } from "node:module";
const require = createRequire("/Users/yeargun/lilscript/benchmarks/popular/package.json");
const acorn = require("acorn");

/* Groups of sibling function declarations that may be permuted together. */
export function findGroups(source, { min = 3 } = {}) {
  let ast;
  try { ast = acorn.parse(source, { ecmaVersion: 2022, sourceType: "module" }); }
  catch { ast = acorn.parse(source, { ecmaVersion: 2022, sourceType: "script" }); }
  const groups = [];
  const consider = (body) => {
    const declarations = body.filter((n) => n.type === "FunctionDeclaration" && n.id);
    if (declarations.length < min) return;
    const names = new Set(declarations.map((d) => d.id.name));
    if (names.size !== declarations.length) return; /* duplicate names: order decides */
    groups.push(declarations.map((d) => ({ start: d.start, end: d.end, name: d.id.name })));
  };
  const visit = (node) => {
    if (!node || typeof node.type !== "string") return;
    if (node.type === "Program") consider(node.body);
    if (node.type === "BlockStatement" && node.body) consider(node.body);
    for (const key of Object.keys(node)) {
      if (key === "type" || key === "start" || key === "end") continue;
      const value = node[key];
      if (Array.isArray(value)) { for (const child of value) if (child && child.type) visit(child); }
      else if (value && typeof value.type === "string") visit(value);
    }
  };
  visit(ast);
  return groups;
}

/* Shingle set of a text, for similarity. */
function shingles(text, k = 8, stride = 4) {
  const set = new Set();
  for (let i = 0; i + k <= text.length; i += stride) set.add(text.slice(i, i + k));
  return set;
}
function similarity(a, b) {
  const [small, large] = a.size < b.size ? [a, b] : [b, a];
  let shared = 0;
  for (const s of small) if (large.has(s)) shared++;
  return shared / (small.size || 1);
}

export const LAYOUTS = {
  asIs: (items) => items,
  /* Nearest-neighbour chain: each function follows the one it looks most
     like, so a copy has the shortest distance to its source. */
  similarity: (items) => {
    const withShingles = items.map((i) => ({ ...i, sh: shingles(i.text) }));
    const remaining = withShingles.slice(1);
    const out = [withShingles[0]];
    let current = out[0];
    while (remaining.length) {
      let best = 0, bestScore = -1;
      for (let i = 0; i < remaining.length; i++) {
        const score = similarity(current.sh, remaining[i].sh);
        if (score > bestScore) { bestScore = score; best = i; }
      }
      current = remaining.splice(best, 1)[0];
      out.push(current);
    }
    return out;
  },
  /* Same idea, but equal-size runs first: equal strides make repeated
     distances, which is what the last-distance cache rewards. */
  sizeThenSimilarity: (items) => {
    const buckets = new Map();
    for (const item of items) {
      const bucket = Math.round(item.text.length / 64);
      if (!buckets.has(bucket)) buckets.set(bucket, []);
      buckets.get(bucket).push(item);
    }
    const out = [];
    for (const key of [...buckets.keys()].sort((a, b) => a - b)) {
      out.push(...LAYOUTS.similarity(buckets.get(key)));
    }
    return out;
  },
  byLength: (items) => [...items].sort((a, b) => a.text.length - b.text.length),
  byName: (items) => [...items].sort((a, b) => a.name.localeCompare(b.name)),
  reversed: (items) => [...items].reverse(),
};

export function relayout(source, orderName, { min = 3 } = {}) {
  const groups = findGroups(source, { min });
  if (!groups.length) return { text: source, groups: 0, moved: 0 };
  const order = LAYOUTS[orderName];
  const edits = [];
  let moved = 0;
  for (const group of groups) {
    const items = group.map((g) => ({ ...g, text: source.slice(g.start, g.end) }));
    const sorted = order(items);
    for (let i = 0; i < group.length; i++) {
      if (sorted[i].start !== group[i].start) moved++;
      edits.push({ start: group[i].start, end: group[i].end, text: sorted[i].text });
    }
  }
  edits.sort((a, b) => a.start - b.start);
  let out = "", cursor = 0;
  for (const edit of edits) {
    if (edit.start < cursor) continue;
    out += source.slice(cursor, edit.start) + edit.text;
    cursor = edit.end;
  }
  return { text: out + source.slice(cursor), groups: groups.length, moved };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const { writeFileSync } = await import("node:fs");
  const { dirname, join } = await import("node:path");
  const { fileURLToPath } = await import("node:url");
  const here = dirname(fileURLToPath(import.meta.url));
  const rows = [];
  const { CORPORA, readCorpus, brotli, census } = await import("./census.mjs");
  const { gzipSync } = await import("node:zlib");
  const score = (t) => ({
    raw: Buffer.byteLength(t), gzip9: gzipSync(Buffer.from(t), { level: 9 }).length, br11: brotli(t).length,
  });
  for (const id of Object.keys(CORPORA)) {
    const source = readCorpus(id);
    const groups = findGroups(source);
    const total = groups.reduce((a, g) => a + g.length, 0);
    if (!total) { console.log(`${id.padEnd(18)} no permutable function-declaration group`); continue; }
    const base = score(source);
    const baseCensus = census(id, source);
    console.log(`\n${id}  ${groups.length} group(s), ${total} hoisted function declarations, br11 ${base.br11}`);
    for (const name of Object.keys(LAYOUTS)) {
      const { text, moved } = relayout(source, name);
      const s = score(text);
      let c;
      try { c = census(id, text); } catch { c = null; }
      const d = (k) => { const v = s[k] - base[k]; return (v > 0 ? "+" : "") + v; };
      rows.push({ id, order: name, moved, base,
        delta: { raw: s.raw - base.raw, gzip9: s.gzip9 - base.gzip9, br11: s.br11 - base.br11 },
        implicitPct: c ? (c.implicitDistances / c.commands) * 100 : null,
        baseImplicitPct: (baseCensus.implicitDistances / baseCensus.commands) * 100,
        distBytes: c ? Math.round((c.bitsByChannel.dist || 0) / 8) : null,
        baseDistBytes: Math.round((baseCensus.bitsByChannel.dist || 0) / 8) });
      console.log(`  ${name.padEnd(20)} raw ${d("raw").padStart(6)}  gzip ${d("gzip9").padStart(7)}  br11 ${d("br11").padStart(7)}  moved ${String(moved).padStart(4)}` +
        (c ? `  implicit ${(c.implicitDistances / c.commands * 100).toFixed(1)}% (was ${(baseCensus.implicitDistances / baseCensus.commands * 100).toFixed(1)}%)` +
             `  distBytes ${Math.round((c.bitsByChannel.dist || 0) / 8)} (was ${Math.round((baseCensus.bitsByChannel.dist || 0) / 8)})` : ""));
    }
  }
  writeFileSync(join(here, "layout.json"), JSON.stringify(rows, null, 1));
  console.log("\nwrote layout.json");
}
