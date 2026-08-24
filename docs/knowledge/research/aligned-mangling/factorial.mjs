#!/usr/bin/env node
/* Do transform families add up, or do they interact?

   Every heuristic in a compiler's search assumes something about this. A beam
   that scores families independently and then stacks the winners is assuming
   the objective is close to additive. The playbook already noticed it is not
   ("stacks are not additive"), but nobody put a number on it.

   This runs a full 2^k factorial: every combination of k independently
   applicable families, scored with the real codec. From those responses it
   computes main effects, every pairwise interaction, and how much of the
   variance an additive model explains. That is the difference between "greedy
   stacking is fine" and "you must search the product".

   Usage: node factorial.mjs <artifact.js> [--codec] */
import { readFileSync, writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { gzipSync } from "node:zlib";
import { analyze, rename } from "./scope.mjs";
import { assign, adaptiveAlphabet, ALPHABETS } from "./mangle.mjs";
import { mergeDeclarations, forToWhile, outlineMemberCalls } from "./families.mjs";
import { reorderPools, findPools } from "./pool.mjs";
import { brotli } from "./census.mjs";

/* Each factor is a pure text -> text transform, applied in a fixed order so
   that a combination is well defined. */
export function factorsFor(source, { groupNaming = false } = {}) {
  const factors = [];
  const analysis = analyze(source, { renameModuleTopLevel: true });
  const renamable = analysis.bindings.filter((b) => b.renamable).length;

  const remangle = (order, alphabet) => (text) => {
    const a = analyze(text, { renameModuleTopLevel: true });
    const alpha = alphabet === "dialect" ? adaptiveAlphabet(a, { mode: "dialect" })
      : alphabet === "adaptive" ? adaptiveAlphabet(a, { mode: "all" })
      : ALPHABETS[alphabet] || alphabet;
    return rename(a, assign(a, { order, alphabet: alpha }));
  };

  if (renamable > 20 && groupNaming) {
    /* One factor with four levels: naming is a single decision, not two
       independent switches that overwrite each other. */
    factors.push({
      key: "M", name: "naming",
      levels: [
        { label: "as shipped", apply: (t) => t },
        { label: "frequency/dialect", apply: remangle("frequency", "dialect") },
        { label: "first-use/abc", apply: remangle("firstUse", "abc") },
        { label: "first-use/dialect", apply: remangle("firstUse", "dialect") },
      ],
    });
  } else if (renamable > 20) {
    factors.push({ key: "N", name: "rename: dialect alphabet", apply: remangle("frequency", "dialect") });
    factors.push({ key: "O", name: "rename: first-use order", apply: remangle("firstUse", "abc") });
  }
  factors.push({ key: "D", name: "merge adjacent declarations", apply: (t) => mergeDeclarations(t).text });
  factors.push({ key: "W", name: "for(;t;) -> while(t)", apply: (t) => forToWhile(t).text });
  factors.push({ key: "C", name: "outline .slice/.exec/.replace", apply: (t) => outlineMemberCalls(t).text });
  if (findPools(source).length) {
    factors.push({ key: "P", name: "pool order: by reversed string", apply: (t) => reorderPools(t, "bySuffix").text });
  }
  return factors;
}

const score = (text) => ({
  raw: Buffer.byteLength(text),
  gzip9: gzipSync(Buffer.from(text), { level: 9 }).length,
  br11: brotli(text).length,
});

/* A design point is one level per factor. Binary factors have levels
   [off, on]; a grouped factor carries its own list. */
export const levelsOf = (f) => f.levels || [{ label: "off", apply: (t) => t }, { label: "on", apply: f.apply }];
export const gridSize = (factors) => factors.reduce((n, f) => n * levelsOf(f).length, 1);
export function pointAt(factors, index) {
  const point = [];
  let rest = index;
  for (const f of factors) {
    const levels = levelsOf(f);
    point.push(rest % levels.length);
    rest = Math.floor(rest / levels.length);
  }
  return point;
}
export function build(source, factors, index) {
  const point = pointAt(factors, index);
  let text = source;
  factors.forEach((f, i) => { text = levelsOf(f)[point[i]].apply(text); });
  return text;
}

/* Effects for a general grid: the effect of a factor level is its mean
   response minus the grand mean; the interaction of two factors is the
   variance their cell means carry beyond both main effects. */
export function gridEffects(responses, factors) {
  const n = responses.length;
  const grand = responses.reduce((a, b) => a + b, 0) / n;
  const points = responses.map((_, i) => pointAt(factors, i));
  const main = factors.map((f, i) => {
    const levels = levelsOf(f);
    return levels.map((lv, l) => {
      const rows = responses.filter((_, m) => points[m][i] === l);
      return { label: lv.label, effect: rows.reduce((a, b) => a + b, 0) / rows.length - grand };
    });
  });
  const inter = [];
  for (let i = 0; i < factors.length; i++) {
    for (let j = i + 1; j < factors.length; j++) {
      let ss = 0;
      const li = levelsOf(factors[i]).length, lj = levelsOf(factors[j]).length;
      for (let a = 0; a < li; a++) for (let b = 0; b < lj; b++) {
        const rows = responses.filter((_, m) => points[m][i] === a && points[m][j] === b);
        if (!rows.length) continue;
        const cell = rows.reduce((x, y) => x + y, 0) / rows.length;
        const residual = cell - grand - main[i][a].effect - main[j][b].effect;
        ss += rows.length * residual * residual;
      }
      inter.push({ i, j, ss });
    }
  }
  /* additive fit over the whole grid */
  let ssTotal = 0, ssResidual = 0, worst = { index: 0, error: 0 };
  responses.forEach((y, m) => {
    let predicted = grand;
    factors.forEach((f, i) => { predicted += main[i][points[m][i]].effect; });
    const error = y - predicted;
    ssTotal += (y - grand) ** 2;
    ssResidual += error * error;
    if (Math.abs(error) > Math.abs(worst.error)) worst = { index: m, error };
  });
  return { grand, main, inter, r2: ssTotal ? 1 - ssResidual / ssTotal : 1,
           rmse: Math.sqrt(ssResidual / n), worst };
}

/* Yates-style effects from a full factorial: the main effect of factor i is
   the mean response with it on minus the mean with it off; the interaction of
   i and j is half the difference of factor i's effect at j's two levels. */
export function effects(responses, k) {
  const main = [];
  for (let i = 0; i < k; i++) {
    let on = 0, off = 0, n = 0;
    for (let m = 0; m < responses.length; m++) {
      if (m & (1 << i)) on += responses[m]; else off += responses[m];
      n++;
    }
    main.push((on - off) / (n / 2));
  }
  const inter = [];
  for (let i = 0; i < k; i++) {
    for (let j = i + 1; j < k; j++) {
      let sum = 0;
      for (let m = 0; m < responses.length; m++) {
        const si = (m & (1 << i)) ? 1 : -1;
        const sj = (m & (1 << j)) ? 1 : -1;
        sum += si * sj * responses[m];
      }
      inter.push({ i, j, value: sum / (responses.length / 2) });
    }
  }
  return { main, inter };
}

/* How well does "sum the main effects" predict the real response? */
export function additiveFit(responses, k, main) {
  const mean = responses.reduce((a, b) => a + b, 0) / responses.length;
  let ssTotal = 0, ssResidual = 0, worst = { mask: 0, error: 0 };
  for (let m = 0; m < responses.length; m++) {
    let predicted = mean;
    for (let i = 0; i < k; i++) predicted += ((m & (1 << i)) ? 0.5 : -0.5) * main[i];
    const error = responses[m] - predicted;
    ssTotal += (responses[m] - mean) ** 2;
    ssResidual += error ** 2;
    if (Math.abs(error) > Math.abs(worst.error)) worst = { mask: m, error };
  }
  return { r2: ssTotal ? 1 - ssResidual / ssTotal : 1, rmse: Math.sqrt(ssResidual / responses.length), worst };
}

/* Run one design and report it. */
export function runDesign(source, factors, base) {
  const n = gridSize(factors);
  const rows = [];
  for (let index = 0; index < n; index++) {
    let text;
    try { text = build(source, factors, index); } catch (e) { rows.push(null); continue; }
    rows.push({ index, ...score(text), text });
  }
  const responses = rows.map((r) => (r ? r.br11 : base.br11));
  const stats = gridEffects(responses, factors);
  const ok = rows.filter(Boolean);
  const best = ok.slice().sort((a, b) => a.br11 - b.br11)[0];
  /* greedy: pick each factor's best level independently */
  let greedyIndex = 0, place = 1;
  factors.forEach((f, i) => {
    const levels = levelsOf(f);
    let bestLevel = 0, bestEffect = Infinity;
    stats.main[i].forEach((lv, l) => { if (lv.effect < bestEffect) { bestEffect = lv.effect; bestLevel = l; } });
    greedyIndex += bestLevel * place;
    place *= levels.length;
  });
  return { rows, responses, stats, best, greedy: rows[greedyIndex], greedyIndex, n };
}

const describe = (factors, index) => pointAt(factors, index)
  .map((l, i) => `${factors[i].key}=${levelsOf(factors[i])[l].label}`).join(" ");

const DEFAULT_TARGETS = [
  ["jquerylil-esm", "/Users/yeargun/jquerylil/dist/jquery.esm.js"],
  ["jquery-lil-raw", "/Users/yeargun/lilscript/benchmarks/popular/ports/jquery/jquery-lilscript.raw.js"],
  ["solidlil-reactive", "/Users/yeargun/lilscript/labs/solid-client/packages/solidlil/reactive.generated.js"],
  ["markedlil-raw", "/Users/yeargun/markedlil/dist/marked.raw.js"],
];

if (import.meta.url === `file://${process.argv[1]}`) {
  const arg = process.argv[2];
  const targets = arg && !arg.startsWith("--") ? [[arg.split("/").pop(), arg]] : DEFAULT_TARGETS;
  const report = [];
  for (const [id, file] of targets) {
  const source = readFileSync(file, "utf8");
  const base = score(source);
  console.log(`\n${id}  ${base.raw} raw / ${base.gzip9} gzip / ${base.br11} br11\n`);
  const entry = { id, file, base, designs: {} };

  for (const grouped of [false, true]) {
    const factors = factorsFor(source, { groupNaming: grouped });
    const n = gridSize(factors);
    const label = grouped ? "naming as ONE factor with 4 levels" : "naming as TWO independent switches";
    const { stats, best, greedy, greedyIndex, rows: runRows } = runDesign(source, factors, base);
    console.log(`── ${label}: ${factors.length} factors, ${n} design points`);
    console.log(`   additive model R² ${stats.r2.toFixed(4)}   RMSE ${stats.rmse.toFixed(1)} bytes`);
    const worstPoint = describe(factors, stats.worst.index);
    console.log(`   worst-predicted point off by ${stats.worst.error.toFixed(1)} bytes  (${worstPoint})`);
    const ranked = stats.inter.slice().sort((a, b) => b.ss - a.ss);
    const totalSS = ranked.reduce((a, x) => a + x.ss, 0) || 1;
    console.log("   interaction budget:");
    for (const it of ranked.slice(0, 4)) {
      console.log(`     ${factors[it.i].key}×${factors[it.j].key}  ${((it.ss / totalSS) * 100).toFixed(1)}% of all interaction variance`);
    }
    console.log(`   best of ${n}: ${best.br11} (${best.br11 - base.br11})   greedy: ${greedy.br11} (${greedy.br11 - base.br11})` +
      `   greedy leaves ${greedy.br11 - best.br11}`);
    console.log(`   best point: ${describe(factors, best.index)}`);
    /* what "stack the independently measured deltas" would have predicted */
    const singles = factors.map((f, i) => {
      const levels = levelsOf(f);
      let bestLevel = 0, bestSize = Infinity;
      for (let l = 1; l < levels.length; l++) {
        let idx = 0, place = 1;
        factors.forEach((g, gi) => { if (gi === i) idx += l * place; place *= levelsOf(g).length; });
        const size = runRows[idx] ? runRows[idx].br11 : base.br11;
        if (size < bestSize) { bestSize = size; bestLevel = l; }
      }
      return bestLevel ? bestSize - base.br11 : 0;
    });
    const stacked = base.br11 + singles.filter((d) => d < 0).reduce((a, b) => a + b, 0);
    console.log(`   stacking individual deltas predicts ${stacked}; the truth is ${greedy.br11} (off by ${greedy.br11 - stacked})\n`);
    entry.designs[grouped ? "grouped" : "split"] = {
      factors: factors.map((f) => ({ key: f.key, name: f.name, levels: levelsOf(f).length })),
      points: n, r2: stats.r2, rmse: stats.rmse,
      worstError: stats.worst.error,
      topInteractions: ranked.slice(0, 4).map((it) => ({
        pair: `${factors[it.i].key}×${factors[it.j].key}`, share: it.ss / totalSS })),
      best: { br11: best.br11, delta: best.br11 - base.br11, point: describe(factors, best.index) },
      greedy: { br11: greedy.br11, delta: greedy.br11 - base.br11, leaves: greedy.br11 - best.br11 },
      stackedPrediction: stacked, stackingError: greedy.br11 - stacked,
    };
  }
  report.push(entry);
  }
  const { writeFileSync: write } = await import("node:fs");
  const { dirname, join } = await import("node:path");
  const { fileURLToPath } = await import("node:url");
  write(join(dirname(fileURLToPath(import.meta.url)), "factorial.json"), JSON.stringify(report, null, 1));
  console.log("wrote factorial.json");
}
