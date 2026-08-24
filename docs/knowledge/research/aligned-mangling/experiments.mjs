#!/usr/bin/env node
/* Scored mutations for the naming questions in this folder.

   Every variant is produced through the scope analyser and then re-analysed:
   a variant that fails `verify` is reported, never scored. Numbers are Node
   zlib Brotli 1.1.0 q11 lgwin=22 and gzip-9 — diagnostic, the same family the
   rest of the research uses, not `lilscript-codec` gates. */
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { analyze, rename, verify, RESERVED } from "./scope.mjs";
import { assign, createAllocator, adaptiveAlphabet, ALPHABETS, NgramModel, LzModel, renderScope, twoCharNames, firstUse } from "./mangle.mjs";
import { CORPORA, readCorpus, brotli, census } from "./census.mjs";
import { gzipSync } from "node:zlib";

const here = dirname(fileURLToPath(import.meta.url));

export function score(text) {
  const bytes = Buffer.from(text, "utf8");
  return { raw: bytes.length, gzip9: gzipSync(bytes, { level: 9 }).length, br11: brotli(text).length };
}

/* --- strategies ------------------------------------------------------- */

function resolveAlphabet(analysis, alphabet) {
  if (alphabet === "adaptive") return adaptiveAlphabet(analysis, { mode: "all" });
  if (alphabet === "token") return adaptiveAlphabet(analysis, { mode: "token" });
  if (alphabet === "dialect") return adaptiveAlphabet(analysis, { mode: "dialect" });
  return ALPHABETS[alphabet] || alphabet;
}

export function remangle(source, { order, alphabet }) {
  const analysis = analyze(source);
  const alpha = resolveAlphabet(analysis, alphabet);
  const mapping = assign(analysis, { order, alphabet: alpha });
  return { text: rename(analysis, mapping), analysis, mapping, alphabet: alpha.slice(0, 12) };
}

/* Dictionary words that are safe to introduce: not used anywhere in the file
   as a binding or a free name. */
function dictionaryWords(BM, analysis, { minLen = 4, maxLen = 12, count = 4000 }) {
  const used = new Set(analysis.bindings.map((b) => b.name));
  for (const name of analysis.unresolved.keys()) used.add(name);
  const dict = BM.dictionary();
  const out = [];
  for (let len = minLen; len <= maxLen && out.length < count; len++) {
    const n = dict.countFor(len);
    for (let i = 0; i < n && out.length < count; i++) {
      const word = dict.wordText(len, i);
      if (!/^[A-Za-z_$][A-Za-z0-9_$]*$/.test(word)) continue;
      if (RESERVED.has(word) || used.has(word)) continue;
      out.push(word);
    }
  }
  return out;
}

/* Rename the N bindings in a frequency band to dictionary words. */
export function dictionaryNames(source, BM, { band = "cold", limit = 400, maxCount = 3 }) {
  const analysis = analyze(source);
  const words = dictionaryWords(BM, analysis, {});
  const pool = analysis.bindings.filter((b) => b.renamable);
  const chosen = band === "cold"
    ? pool.filter((b) => b.count <= maxCount).sort((a, b) => a.count - b.count).slice(0, limit)
    : pool.sort((a, b) => b.count - a.count).slice(0, limit);
  const mapping = new Map();
  let w = 0;
  for (const binding of chosen) {
    while (w < words.length && [...mapping.values()].length && mapping.has(words[w])) w++;
    if (w >= words.length) break;
    mapping.set(binding, words[w++]);
  }
  return { text: rename(analysis, mapping), analysis, mapping, renamed: mapping.size };
}

/* Alignment as a local search.
   Start from a complete, legal assignment. Then walk the file in source
   order and, for each function, try the other names its own bindings could
   legally take, keeping whichever spelling the earlier text can copy most of.
   Nothing about the program changes; only which free letter each binding
   holds. This is the "closure 2 should call it `a` too" idea, measured. */
export function alignedMangle(source, {
  alphabet = "etn", order = "frequency", candidates = 8, minMatch = 6, passes = 1, model: modelKind = "lz",
  objective = "bits",
} = {}) {
  const analysis = analyze(source);
  const allocator = createAllocator(analysis);
  const alpha = resolveAlphabet(analysis, alphabet);
  const mapping = assign(analysis, { order, alphabet: alpha });
  const names = twoCharNames(alpha);

  const functionScopes = analysis.scopes
    .filter((s) => s.kind === "function" && s.node && s.node.start !== undefined)
    .sort((a, b) => a.node.start - b.node.start);

  let improvedScopes = 0, improvedBindings = 0;
  for (let pass = 0; pass < passes; pass++) {
    const model = modelKind === "lz" ? new LzModel(minMatch) : new NgramModel(8);
    const add = (text) => (model.add ? model.add(text) : null);
    /* higher is better in every objective */
    const scoreOf = (text) => (modelKind !== "lz" ? model.score(text)
      : objective === "bits" ? -model.estimateBits(text) : model.covered(text));
    let cursor = 0;
    for (const scope of functionScopes) {
      if (scope.node.start > cursor) { add(renderRange(analysis, mapping, cursor, scope.node.start)); cursor = scope.node.start; }
      const bindings = [...scope.bindings.values()].filter((b) => b.renamable);
      if (bindings.length) {
        bindings.sort((a, b) => b.count - a.count || firstUse(a) - firstUse(b));
        const base = allocator.scopeForbidden(scope, mapping);
        for (const binding of bindings) {
          const forbidden = new Set(base);
          for (const other of bindings) {
            if (other === binding) continue;
            forbidden.add(other.name);
            forbidden.add(mapping.get(other) || other.name);
          }
          for (const name of allocator.shadowed(binding, mapping)) forbidden.add(name);
          const current = mapping.get(binding);
          const options = [current, ...names.filter((n) => !forbidden.has(n) && n !== current).slice(0, candidates)]
            .filter(Boolean);
          if (options.length <= 1) continue;
          let best = current, bestScore = -1;
          const overlay = new Map();
          for (const name of options) {
            if (name.length > (current || "").length) continue; /* never grow raw */
            overlay.set(binding, name);
            const score = scoreOf(renderScope(analysis, scope, mapping, overlay));
            if (score > bestScore) { bestScore = score; best = name; }
          }
          if (best !== current) { mapping.set(binding, best); improvedBindings++; }
        }
        improvedScopes++;
      }
      const rendered = renderScope(analysis, scope, mapping);
      add(rendered);
      cursor = Math.max(cursor, scope.node.end);
    }
  }
  return { text: rename(analysis, mapping), analysis, mapping, improvedScopes, improvedBindings };
}

/* Text of a source range with the current mapping applied. */
function renderRange(analysis, mapping, start, end) {
  const edits = [];
  for (const binding of analysis.bindings) {
    const name = mapping.get(binding);
    if (!name || name === binding.name) continue;
    for (const ref of binding.references) {
      if (ref.start >= start && ref.end <= end) {
        edits.push({ start: ref.start, end: ref.end,
          text: binding.shorthandNodes.has(ref) ? `${binding.name}: ${name}` : name });
      }
    }
  }
  edits.sort((a, b) => a.start - b.start);
  let out = "", cursor = start;
  for (const edit of edits) {
    if (edit.start < cursor) continue;
    out += analysis.source.slice(cursor, edit.start) + edit.text;
    cursor = edit.end;
  }
  return out + analysis.source.slice(cursor, end);
}

/* --- shape statistics -------------------------------------------------- */
/* How many function bodies are byte-identical to another one? That is the
   quantity the alignment idea is trying to move. */
export function shapeStats(text) {
  const analysis = analyze(text);
  const bodies = new Map();
  let total = 0;
  for (const scope of analysis.scopes) {
    if (scope.kind !== "function") continue;
    const body = text.slice(scope.node.start, scope.node.end);
    if (body.length < 40) continue;
    total++;
    bodies.set(body, (bodies.get(body) || 0) + 1);
  }
  let duplicates = 0, duplicateBytes = 0;
  for (const [body, n] of bodies) if (n > 1) { duplicates += n - 1; duplicateBytes += (n - 1) * body.length; }
  return { functions: total, distinct: bodies.size, duplicates, duplicateBytes };
}

/* --- runner ------------------------------------------------------------ */
if (import.meta.url === `file://${process.argv[1]}`) {
  const { loadEngine } = await import("../brotli-machine/engine.mjs");
  const BM = loadEngine();
  const only = process.argv.slice(2).filter((a) => !a.startsWith("--"));
  const ids = only.length ? only : Object.keys(CORPORA);
  const results = [];

  for (const id of ids) {
    const source = readCorpus(id);
    const base = score(source);
    const baseCensus = census(id, source);
    console.log(`\n=== ${id}  raw ${base.raw}  gzip ${base.gzip9}  br11 ${base.br11}`);
    const row = { id, base, baseCensus: summarize(baseCensus), variants: [] };

    const variants = [
      ["remangle frequency abc", () => remangle(source, { order: "frequency", alphabet: "abc" })],
      ["remangle frequency etn", () => remangle(source, { order: "frequency", alphabet: "etn" })],
      ["remangle first-use abc", () => remangle(source, { order: "firstUse", alphabet: "abc" })],
      ["remangle first-use etn", () => remangle(source, { order: "firstUse", alphabet: "etn" })],
      ["remangle source-order abc", () => remangle(source, { order: "source", alphabet: "abc" })],
      ["remangle frequency adaptive-all", () => remangle(source, { order: "frequency", alphabet: "adaptive" })],
      ["remangle frequency adaptive-token", () => remangle(source, { order: "frequency", alphabet: "token" })],
      ["remangle frequency dialect", () => remangle(source, { order: "frequency", alphabet: "dialect" })],
      ["aligned, coverage objective etn", () => alignedMangle(source, { alphabet: "etn", order: "frequency", objective: "coverage" })],
      ["aligned, bits objective etn", () => alignedMangle(source, { alphabet: "etn", order: "frequency", objective: "bits" })],
      ["aligned, bits objective dialect", () => alignedMangle(source, { alphabet: "dialect", order: "frequency", objective: "bits" })],
      ["dictionary words, cold (<=3 uses)", () => dictionaryNames(source, BM, { band: "cold", limit: 400, maxCount: 3 })],
      ["dictionary words, hot (top 400)", () => dictionaryNames(source, BM, { band: "hot", limit: 400 })],
    ];

    for (const [label, run] of variants) {
      const t0 = Date.now();
      let produced;
      try { produced = run(); } catch (e) { console.log(`  ${label.padEnd(34)} FAILED ${e.message}`); continue; }
      const text = produced.text;
      const check = verify(produced.analysis, text, produced.mapping);
      const s = score(text);
      const shapes = shapeStats(text);
      const c = summarize(census(id, text));
      const d = (k) => {
        const delta = s[k] - base[k];
        return (delta > 0 ? "+" : "") + delta;
      };
      console.log(`  ${label.padEnd(34)} raw ${d("raw").padStart(7)} gzip ${d("gzip9").padStart(7)} br11 ${d("br11").padStart(7)}` +
        `  ${check.ok ? "legal" : "ILLEGAL: " + check.why}  dupFns ${shapes.duplicates} (${shapes.duplicateBytes}B)  implicitDist ${(c.implicitPct).toFixed(1)}%  ${(Date.now() - t0)}ms`);
      row.variants.push({ label, score: s, delta: { raw: s.raw - base.raw, gzip9: s.gzip9 - base.gzip9, br11: s.br11 - base.br11 }, legal: check.ok, why: check.why || null, shapes, census: c });
    }
    const baseShapes = shapeStats(source);
    row.baseShapes = baseShapes;
    console.log(`  baseline duplicate function bodies: ${baseShapes.duplicates} of ${baseShapes.functions} (${baseShapes.duplicateBytes} bytes), implicit distances ${row.baseCensus.implicitPct.toFixed(1)}%`);
    results.push(row);
  }
  writeFileSync(join(here, "results.json"), JSON.stringify(results, null, 1));
  console.log("\nwrote results.json");
}

function summarize(c) {
  return {
    br11: c.br11, commands: c.commands, literalBytes: c.literalBytes, copyBytes: c.copyBytes,
    dictBytes: c.dictBytes, dictRefs: c.dictRefs, copies: c.copies, meanCopy: c.meanCopy,
    implicitPct: (c.implicitDistances / Math.max(1, c.commands)) * 100,
    shortPct: (c.shortDistances / Math.max(1, c.commands)) * 100,
    distBits: c.bitsByChannel.dist || 0, litBits: c.bitsByChannel.literal || 0,
    cmdBits: c.bitsByChannel.cmd || 0, codeBits: c.bitsByChannel.code || 0,
  };
}
