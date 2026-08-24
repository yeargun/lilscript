#!/usr/bin/env node
/* A closed-form cost model for the compression objective.
 *
 * The size of a Brotli stream is not a black box. For a fixed parse it is
 * exactly
 *
 *     L(x) = H + Σ_commands [ ℓ(cmd) + ℓ(dist) + Σ ℓ(literal | ctx) ]
 *
 * where every ℓ is the length of a prefix code, i.e. −log2 p of a symbol under
 * that block's own histogram. Two things make it non-linear in the program
 * text x:
 *
 *   1. the parse — which bytes become literals and which become copies — is
 *      itself chosen by the encoder;
 *   2. the code lengths depend on the histograms of the whole block, so a
 *      local edit changes the price of every symbol everywhere.
 *
 * (2) is a mean-field coupling: hold the histograms θ fixed and L becomes a
 * *linear functional* of the symbol counts, with gradient
 *
 *     ∂L/∂n_s = −log2 p_s
 *
 * That is the analytic model implemented here: parse once, count symbols, and
 * price them at their own empirical entropy. It costs one pass instead of a
 * q11 compression, and this file measures how well it ranks candidates against
 * the real codec.
 *
 * Usage: node analytic.mjs [<artifact.js> …]
 */
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { writeFileSync } from "node:fs";
import { loadEngine } from "../brotli-machine/engine.mjs";
import { brotli } from "./census.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const BM = loadEngine();
const T = BM.tables;
const CTX_LUT = BM.base64ToBytes(BM.data.contextLutBase64);

const log2 = Math.log2;

/* Shannon cost of a multiset of symbols, in bits: Σ n_s · −log2(n_s / N).
   This is the entropy bound a prefix code approaches; the real code pays a
   little more for integer lengths and for describing itself. */
function entropyBits(counts) {
  let total = 0;
  for (const n of counts.values()) total += n;
  if (!total) return { bits: 0, total: 0, distinct: 0 };
  let bits = 0;
  for (const n of counts.values()) bits += n * -log2(n / total);
  return { bits, total, distinct: counts.size };
}

/* Cost of describing one prefix code: roughly the code-length sequence, which
   Brotli itself compresses. A few bits per used symbol is close enough for
   ranking. */
const codeDescriptionBits = (distinct) => (distinct <= 4 ? 12 : 20 + distinct * 4.2);

/* Price a symbol multiset against *someone else's* histogram — the fixed-θ,
   first-order view: Σ_s n_s · (−log2 p_s) with p from the reference. This is
   ⟨∇L, δ⟩ evaluated at the baseline, and the gap between it and the full
   recomputed model is the second-order (mean-field) correction. */
function crossEntropyBits(counts, reference) {
  let refTotal = 0;
  for (const n of reference.values()) refTotal += n;
  if (!refTotal) return entropyBits(counts).bits;
  let bits = 0;
  const floor = 0.35; /* an unseen symbol still has to be paid for */
  for (const [sym, n] of counts) {
    const p = ((reference.get(sym) || 0) + floor) / (refTotal + floor * (reference.size + 1));
    bits += n * -log2(p);
  }
  return bits;
}

/* One analytic evaluation. `contextBits` selects the literal model:
   0 = order-0, 6 = the full 64-context split Brotli actually uses. */
export function analyticCost(text, { contextBits = 6, useDictionary = false, contextMode = 2, priceWith = null } = {}) {
  const bytes = BM.bytesFromLatin1(text.length === Buffer.byteLength(text) ? text : text);
  const input = new TextEncoder().encode(text);
  const ctx = {
    bytes: input, text: BM.latin1(input), dict: BM.dictionary(),
    maxBackwardDistance: (1 << 22) - 16, BM, plugins: BM.plugins, stages: {},
    params: Object.assign(BM.plugins.chooseParams({ bytes: input, text, opts: {}, BM }),
      { useDictionary, contextMode }),
  };
  const t0 = performance.now();
  const commands = BM.plugins.buildCommands(ctx);
  const parseMs = performance.now() - t0;

  const command = new Map();
  const distance = new Map();
  const literalByContext = Array.from({ length: 1 << contextBits }, () => new Map());
  const bump = (map, key) => map.set(key, (map.get(key) || 0) + 1);

  let extraBits = 0;
  let pos = 0;
  const cache = [16, 15, 11, 4];
  let cacheIdx = 0;
  for (const cmd of commands) {
    bump(command, cmd.symbol);
    const row = T.CMD_LUT[cmd.symbol];
    extraBits += row.insertExtra + row.copyExtra;
    for (let i = 0; i < cmd.insertLen; i++) {
      const p = pos + i;
      const p1 = p >= 1 ? input[p - 1] : 0;
      const p2 = p >= 2 ? input[p - 2] : 0;
      const context = contextBits
        ? (CTX_LUT[(contextMode << 9) + p1] | CTX_LUT[(contextMode << 9) + 256 + p2])
        : 0;
      bump(literalByContext[context], input[p]);
    }
    pos += cmd.insertLen;
    if (cmd.kind === "end") break;
    if (cmd.distanceCode !== 0) {
      /* price a distance by its code class: short codes are cheap, the rest
         carry their extra bits */
      const hit = cache.indexOf(cmd.distance);
      if (hit >= 0) bump(distance, "short" + hit);
      else {
        const bits = Math.max(1, 32 - Math.clz32(cmd.distance));
        bump(distance, "b" + bits);
        extraBits += Math.max(0, bits - 2);
        cache[cacheIdx & 3] = cmd.distance;
        cacheIdx++;
      }
    }
    pos += cmd.dictionary ? cmd.dictionary.produced.length : cmd.copyLen;
  }

  const cmdE = entropyBits(command);
  const distE = entropyBits(distance);
  let literalBits = 0, literalSymbols = 0, literalDistinct = 0;
  for (const counts of literalByContext) {
    const e = entropyBits(counts);
    literalBits += e.bits;
    literalSymbols += e.total;
    literalDistinct += e.distinct;
  }
  const header = codeDescriptionBits(cmdE.distinct) + codeDescriptionBits(distE.distinct) +
    literalByContext.reduce((a, c) => a + (c.size ? codeDescriptionBits(c.size) : 0), 0);

  /* first-order view: same parse, prices frozen at the reference histograms */
  let linearBits = null;
  if (priceWith) {
    linearBits = crossEntropyBits(command, priceWith.command) +
      crossEntropyBits(distance, priceWith.distance) +
      literalByContext.reduce((a, counts, i) => a + crossEntropyBits(counts, priceWith.literalByContext[i]), 0) +
      extraBits + header;
  }

  const bits = cmdE.bits + distE.bits + literalBits + extraBits + header;
  return {
    bytes: bits / 8,
    linearBytes: linearBits === null ? null : linearBits / 8,
    model: { command, distance, literalByContext },
    parts: {
      commands: cmdE.bits / 8, distances: distE.bits / 8, literals: literalBits / 8,
      extra: extraBits / 8, header: header / 8,
    },
    counts: { commands: commands.length, literals: literalSymbols, literalDistinct },
    parseMs,
  };
}

/* Correlation helpers. */
const pearson = (xs, ys) => {
  const n = xs.length;
  const mx = xs.reduce((a, b) => a + b, 0) / n, my = ys.reduce((a, b) => a + b, 0) / n;
  let sxy = 0, sxx = 0, syy = 0;
  for (let i = 0; i < n; i++) { const dx = xs[i] - mx, dy = ys[i] - my; sxy += dx * dy; sxx += dx * dx; syy += dy * dy; }
  return sxy / Math.sqrt(sxx * syy || 1);
};
const rank = (v) => {
  const idx = v.map((x, i) => [x, i]).sort((a, b) => a[0] - b[0]);
  const r = new Array(v.length);
  idx.forEach(([, i], k) => { r[i] = k; });
  return r;
};
const spearman = (xs, ys) => pearson(rank(xs), rank(ys));

if (import.meta.url === `file://${process.argv[1]}`) {
  const { factorsFor, build, gridSize } = await import("./factorial.mjs");
  const targets = process.argv.slice(2).filter((a) => !a.startsWith("--"));
  const files = targets.length ? targets : [
    "/Users/yeargun/lilscript/labs/solid-client/packages/solidlil/reactive.generated.js",
    "/Users/yeargun/markedlil/dist/marked.raw.js",
    "/Users/yeargun/jquerylil/dist/jquery.esm.js",
  ];
  const report = [];
  for (const file of files) {
    const source = readFileSync(file, "utf8");
    const factors = factorsFor(source, { groupNaming: true });
    const n = Math.min(gridSize(factors), 32);
    const real = [], model = [], linear = [];
    let baselineModel = null;
    let modelMs = 0, realMs = 0;
    for (let i = 0; i < n; i++) {
      let text;
      try { text = build(source, factors, i); } catch { continue; }
      let t = performance.now();
      const r = brotli(text).length;
      realMs += performance.now() - t;
      t = performance.now();
      const m = analyticCost(text);
      modelMs += performance.now() - t;
      real.push(r);
      model.push(m.bytes);
      if (i === 0) baselineModel = m.model;
      else linear.push(analyticCost(text, { priceWith: baselineModel }).linearBytes);
      if (i === 0) linear.push(m.bytes);
    }
    const argmin = (v) => v.indexOf(Math.min(...v));
    const scale = real.reduce((a, b) => a + b, 0) / model.reduce((a, b) => a + b, 0);
    const errors = real.map((r, i) => model[i] * scale - r);
    const mae = errors.reduce((a, e) => a + Math.abs(e), 0) / errors.length;
    const row = {
      file: file.split("/").pop(), points: real.length,
      pearson: pearson(model, real), spearman: spearman(model, real),
      picksSameWinner: argmin(model) === argmin(real),
      winnerPenalty: real[argmin(model)] - Math.min(...real),
      scale, maeAfterScale: mae,
      msPerEvalModel: modelMs / real.length, msPerEvalReal: realMs / real.length,
    };
    report.push(row);
    console.log(`\n${row.file}  (${row.points} design points)`);
    console.log(`  correlation with real Brotli: pearson ${row.pearson.toFixed(4)}  spearman ${row.spearman.toFixed(4)}`);
    console.log(`  picks the same winner: ${row.picksSameWinner ? "yes" : `no — costs ${row.winnerPenalty} bytes`}`);
    console.log(`  after one global scale factor (${row.scale.toFixed(4)}): mean absolute error ${row.maeAfterScale.toFixed(1)} bytes`);
    console.log(`  cost per evaluation: model ${row.msPerEvalModel.toFixed(1)} ms vs codec ${row.msPerEvalReal.toFixed(1)} ms` +
      `  (${(row.msPerEvalReal / row.msPerEvalModel).toFixed(1)}× )`);
    /* how much of each change the first-order (frozen-price) term explains */
    if (linear.length === real.length) {
      const dReal = real.map((r) => r - real[0]);
      const dFull = model.map((m) => (m - model[0]) * row.scale);
      const dLinear = linear.map((l) => (l - linear[0]) * row.scale);
      const err = (pred) => pred.reduce((a, p, i) => a + Math.abs(p - dReal[i]), 0) / pred.length;
      row.firstOrder = { pearson: pearson(dLinear, dReal), mae: err(dLinear) };
      row.fullModel = { pearson: pearson(dFull, dReal), mae: err(dFull) };
      console.log(`  predicting the *change* from the baseline:`);
      console.log(`     first order (prices frozen at the baseline): r ${row.firstOrder.pearson.toFixed(4)}, mean error ${row.firstOrder.mae.toFixed(1)} B`);
      console.log(`     full model  (prices recomputed):             r ${row.fullModel.pearson.toFixed(4)}, mean error ${row.fullModel.mae.toFixed(1)} B`);
    }
  }
  writeFileSync(join(here, "analytic.json"), JSON.stringify(report, null, 1));
  console.log("\nwrote analytic.json");
}
