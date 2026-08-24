#!/usr/bin/env node
/* How much of a file is "the same function spelled differently"?

   For every function, replace each locally-bound identifier with a positional
   token (first occurrence order) and keep everything else. Two functions with
   the same canonical form are the same code up to naming — the case where
   closure 2 could have called it `a` too. The gap between "canonically the
   same" and "byte-identical" is the headroom a naming pass could take. */
import { analyze } from "./scope.mjs";

export function twinStats(source, { minBody = 40 } = {}) {
  const analysis = analyze(source);
  const groups = new Map();
  for (const scope of analysis.scopes) {
    if (scope.kind !== "function" || !scope.node) continue;
    const { start, end } = scope.node;
    if (end - start < minBody) continue;
    const own = new Map(); /* binding -> positional token */
    const edits = [];
    const walk = (s) => {
      for (const binding of s.bindings.values()) {
        for (const ref of binding.references) {
          if (ref.start < start || ref.end > end) continue;
          edits.push({ start: ref.start, end: ref.end, binding });
        }
      }
      for (const child of s.children) walk(child);
    };
    walk(scope);
    /* References to bindings declared outside keep their spelling: they are
       shared context, not this function's choice. */
    edits.sort((a, b) => a.start - b.start);
    let canonical = "", cursor = start;
    for (const edit of edits) {
      if (edit.start < cursor) continue;
      if (!own.has(edit.binding)) own.set(edit.binding, `#${own.size}`);
      canonical += source.slice(cursor, edit.start) + own.get(edit.binding);
      cursor = edit.end;
    }
    canonical += source.slice(cursor, end);
    const text = source.slice(start, end);
    if (!groups.has(canonical)) groups.set(canonical, { spellings: new Map(), count: 0, size: end - start });
    const group = groups.get(canonical);
    group.count++;
    group.spellings.set(text, (group.spellings.get(text) || 0) + 1);
  }

  let functions = 0, twinFunctions = 0, twinGroups = 0, misalignedFunctions = 0;
  let twinBytes = 0, misalignedBytes = 0, identicalBytes = 0;
  const examples = [];
  for (const group of groups.values()) {
    functions += group.count;
    if (group.count < 2) continue;
    twinGroups++;
    twinFunctions += group.count;
    twinBytes += group.size * group.count;
    /* The most common spelling is free; the rest are the tax. */
    const best = Math.max(...group.spellings.values());
    const off = group.count - best;
    misalignedFunctions += off;
    misalignedBytes += off * group.size;
    identicalBytes += (best - 1) * group.size;
    if (off > 0 && examples.length < 12) {
      examples.push({
        size: group.size, count: group.count, spellings: group.spellings.size,
        sample: [...group.spellings.keys()].slice(0, 2).map((s) => s.slice(0, 110)),
      });
    }
  }
  return {
    functions, twinGroups, twinFunctions, twinBytes,
    misalignedFunctions, misalignedBytes, identicalBytes, examples,
  };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const { CORPORA, readCorpus } = await import("./census.mjs");
  for (const id of Object.keys(CORPORA)) {
    const source = readCorpus(id);
    const t = twinStats(source);
    const pct = ((t.misalignedBytes / Buffer.byteLength(source)) * 100).toFixed(2);
    console.log(
      `${id.padEnd(18)} functions ${String(t.functions).padStart(5)}  twin groups ${String(t.twinGroups).padStart(4)}` +
      `  functions in them ${String(t.twinFunctions).padStart(4)}  already identical ${String(t.identicalBytes).padStart(6)}B` +
      `  differ only in names ${String(t.misalignedFunctions).padStart(4)} fns / ${String(t.misalignedBytes).padStart(6)}B (${pct}% of raw)`);
    if (t.examples.length) {
      const e = t.examples[0];
      console.log(`    e.g. ${e.count} copies of a ${e.size}-byte function in ${e.spellings} spellings:`);
      for (const s of e.sample) console.log(`         ${JSON.stringify(s)}`);
    }
  }
}
