#!/usr/bin/env node
/* Does the *order* of a string pool matter?

   LilScript's emit opens with a long `var` of pooled literals:
   `hg="nodeType",ig="parentNode",jg="type",…`. Those declarators are
   order-independent — literal initialisers, no side effects — so their order
   is free to the program and not free to the codec. Sorting them changes
   which strings sit next to each other, which changes what LZ77 can copy and
   which first occurrences the static dictionary can serve.

   This is a legal, cheap compiler knob: emit order of a declaration list. */
import { createRequire } from "node:module";
const require = createRequire("/Users/yeargun/lilscript/benchmarks/popular/package.json");
const acorn = require("acorn");

/* Every `var`/`let`/`const` whose declarators are all `name = <literal>`. */
export function findPools(source, { min = 6 } = {}) {
  let ast;
  try { ast = acorn.parse(source, { ecmaVersion: 2022, sourceType: "module" }); }
  catch { ast = acorn.parse(source, { ecmaVersion: 2022, sourceType: "script" }); }
  const pools = [];
  const visit = (node) => {
    if (!node || typeof node.type !== "string") return;
    if (node.type === "VariableDeclaration" && node.declarations.length >= min) {
      /* Maximal runs of `name = <literal>` declarators. A run is permutable:
         its members cannot reference each other, and every declarator outside
         the run keeps its position, so anything that reads a pooled name
         still sees it declared first. */
      const isLiteral = (d) => d.id.type === "Identifier" && d.init && d.init.type === "Literal" &&
        (typeof d.init.value === "string" || typeof d.init.value === "number");
      let run = [];
      const flush = () => {
        /* A repeated name inside the run would make the order semantic:
           the last assignment wins. Only permute runs of distinct names. */
        const names = new Set(run.map((d) => d.id.name));
        if (run.length >= min && names.size === run.length) {
          pools.push({
            node,
            start: run[0].start,
            end: run[run.length - 1].end,
            items: run.map((d) => ({ name: d.id.name, value: d.init.value, text: source.slice(d.start, d.end) })),
          });
        }
        run = [];
      };
      for (const d of node.declarations) {
        if (isLiteral(d)) run.push(d); else flush();
      }
      flush();
    }
    for (const key of Object.keys(node)) {
      if (key === "type" || key === "start" || key === "end") continue;
      const value = node[key];
      if (Array.isArray(value)) { for (const child of value) if (child && child.type) visit(child); }
      else if (value && typeof value.type === "string") visit(value);
    }
  };
  visit(ast);
  return pools;
}

const reversed = (s) => [...String(s)].reverse().join("");

export const POOL_ORDERS = {
  asIs: (items) => items,
  alphabetical: (items) => [...items].sort((a, b) => String(a.value).localeCompare(String(b.value))),
  bySuffix: (items) => [...items].sort((a, b) => reversed(a.value).localeCompare(reversed(b.value))),
  byLength: (items) => [...items].sort((a, b) => String(a.value).length - String(b.value).length ||
    String(a.value).localeCompare(String(b.value))),
  byLengthDesc: (items) => [...items].sort((a, b) => String(b.value).length - String(a.value).length ||
    String(a.value).localeCompare(String(b.value))),
  /* Greedy "longest shared affix with the previous string": the order an
     LZ77 coder would like, built one step at a time. */
  chained: (items) => {
    const remaining = [...items];
    const out = [];
    let current = remaining.shift();
    out.push(current);
    while (remaining.length) {
      let best = 0, bestScore = -1;
      for (let i = 0; i < remaining.length; i++) {
        const score = overlap(String(current.value), String(remaining[i].value));
        if (score > bestScore) { bestScore = score; best = i; }
      }
      current = remaining.splice(best, 1)[0];
      out.push(current);
    }
    return out;
  },
  /* The pool as the dictionary would like it: entries whose text the static
     dictionary can serve go first, so their copies start from the ROM. */
  dictionaryFirst: (items, dict) => {
    const score = (item) => {
      const text = String(item.value);
      const hits = dict ? dict.matchesAt(text, 0, {}) : [];
      return hits.length ? hits[0].matched : 0;
    };
    return [...items].sort((a, b) => score(b) - score(a) ||
      String(a.value).localeCompare(String(b.value)));
  },
};

function overlap(a, b) {
  let prefix = 0;
  while (prefix < a.length && prefix < b.length && a[prefix] === b[prefix]) prefix++;
  let suffix = 0;
  while (suffix < a.length && suffix < b.length && a[a.length - 1 - suffix] === b[b.length - 1 - suffix]) suffix++;
  return Math.max(prefix, suffix);
}

export function reorderPools(source, orderName, dict) {
  const pools = findPools(source);
  if (!pools.length) return { text: source, pools: 0, items: 0 };
  const order = POOL_ORDERS[orderName];
  const edits = [];
  let items = 0;
  for (const pool of pools) {
    const sorted = order(pool.items, dict);
    items += sorted.length;
    edits.push({ start: pool.start, end: pool.end, text: sorted.map((i) => i.text).join(",") });
  }
  edits.sort((a, b) => a.start - b.start);
  let out = "", cursor = 0;
  for (const edit of edits) {
    out += source.slice(cursor, edit.start) + edit.text;
    cursor = edit.end;
  }
  return { text: out + source.slice(cursor), pools: pools.length, items };
}

/* A permutation of pool declarators is legal by construction — literal
   initialisers, distinct names, nothing else moved — and this proves that is
   all that happened: canonicalise both texts by sorting every pool's
   declarators and require them to be byte-identical. */
export function verifyReorder(before, after) {
  const canon = (text) => {
    const pools = findPools(text);
    if (!pools.length) return text;
    const edits = pools.map((pool) => ({
      start: pool.start, end: pool.end,
      text: pool.items.slice().sort((a, b) => (a.text < b.text ? -1 : a.text > b.text ? 1 : 0))
        .map((i) => i.text).join(","),
    })).sort((a, b) => a.start - b.start);
    let out = "", cursor = 0;
    for (const edit of edits) { out += text.slice(cursor, edit.start) + edit.text; cursor = edit.end; }
    return out + text.slice(cursor);
  };
  const a = canon(before), b = canon(after);
  return a === b ? { ok: true } : { ok: false, why: "the change is not only a permutation of pool declarators" };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const { writeFileSync } = await import("node:fs");
  const { dirname, join } = await import("node:path");
  const { fileURLToPath } = await import("node:url");
  const here = dirname(fileURLToPath(import.meta.url));
  const rows = [];
  const { CORPORA, readCorpus, brotli } = await import("./census.mjs");
  const { gzipSync } = await import("node:zlib");
  const { loadEngine } = await import("../brotli-machine/engine.mjs");
  const dict = loadEngine().dictionary();
  const score = (t) => ({
    raw: Buffer.byteLength(t), gzip9: gzipSync(Buffer.from(t), { level: 9 }).length, br11: brotli(t).length,
  });
  for (const id of Object.keys(CORPORA)) {
    const source = readCorpus(id);
    const pools = findPools(source);
    if (!pools.length) { console.log(`${id.padEnd(18)} no literal pool`); continue; }
    const base = score(source);
    const biggest = pools.slice().sort((a, b) => b.items.length - a.items.length)[0];
    console.log(`\n${id}  ${pools.length} pool declaration(s), largest ${biggest.items.length} entries, br11 ${base.br11}`);
    console.log(`  first entries: ${biggest.items.slice(0, 6).map((i) => JSON.stringify(i.value)).join(" ")}`);
    for (const name of Object.keys(POOL_ORDERS)) {
      const { text } = reorderPools(source, name, dict);
      const s = score(text);
      const d = (k) => { const v = s[k] - base[k]; return (v > 0 ? "+" : "") + v; };
      console.log(`  ${name.padEnd(16)} raw ${d("raw").padStart(6)}  gzip ${d("gzip9").padStart(6)}  br11 ${d("br11").padStart(6)}`);
      rows.push({ id, order: name, entries: biggest.items.length, base,
        delta: { raw: s.raw - base.raw, gzip9: s.gzip9 - base.gzip9, br11: s.br11 - base.br11 } });
    }
  }
  writeFileSync(join(here, "pool.json"), JSON.stringify(rows, null, 1));
  console.log("\nwrote pool.json");
}
