#!/usr/bin/env node
/* Is there a cheap number that predicts which naming a codec will prefer?

   For every corpus, generate many legal namings, then compare each one's
   compressed size against two candidate proxies:
     - the total length of names in the text (what a raw-size mangler
       minimises);
     - the entropy of the name-usage distribution (how concentrated the
       spellings are).
   A proxy that ranks candidates the same way the codec does can filter a
   beam for almost nothing. */
import { writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";
import { analyze, rename } from "./scope.mjs";
import { assign, adaptiveAlphabet, ALPHABETS } from "./mangle.mjs";
import { CORPORA, readCorpus, brotli } from "./census.mjs";

const here = dirname(fileURLToPath(import.meta.url));

/* Shannon entropy of "which name is this occurrence", plus the raw cost. */
export function nameStats(analysis, mapping) {
  const counts = new Map();
  let nameBytes = 0;
  for (const binding of analysis.bindings) {
    if (!binding.renamable) continue;
    const name = mapping.get(binding) || binding.name;
    counts.set(name, (counts.get(name) || 0) + binding.count);
    nameBytes += name.length * binding.count;
  }
  const total = [...counts.values()].reduce((a, b) => a + b, 0) || 1;
  let entropy = 0;
  for (const c of counts.values()) { const p = c / total; entropy -= p * Math.log2(p); }
  const sorted = [...counts.values()].sort((a, b) => b - a);
  const top5 = sorted.slice(0, 5).reduce((a, b) => a + b, 0);
  return {
    distinct: counts.size, nameBytes, entropy,
    top5share: top5 / total,
    /* bits the names alone would cost an order-0 coder */
    nameBits: entropy * total,
  };
}

/* A spread of legal namings: alphabets crossed with orders, plus a few
   deliberately bad ones so the correlation has range. */
function* strategies(analysis) {
  const alphabets = {
    abc: ALPHABETS.abc,
    etn: ALPHABETS.etn,
    adaptive: adaptiveAlphabet(analysis, { mode: "all" }),
    dialect: adaptiveAlphabet(analysis, { mode: "dialect" }),
    hostile: "qxzjkvwyubpgfmhdclsrntioaeQXZJKVWYUBPGFMHDCLSRNTIOAE$_",
    reversed: [...ALPHABETS.abc].reverse().join(""),
  };
  for (const [aName, alphabet] of Object.entries(alphabets)) {
    for (const order of ["frequency", "firstUse", "source"]) {
      yield { label: `${order}/${aName}`, options: { order, alphabet } };
    }
  }
}

/* Pearson correlation. */
const corr = (xs, ys) => {
  const n = xs.length;
  const mx = xs.reduce((a, b) => a + b, 0) / n;
  const my = ys.reduce((a, b) => a + b, 0) / n;
  let sxy = 0, sxx = 0, syy = 0;
  for (let i = 0; i < n; i++) { const dx = xs[i] - mx, dy = ys[i] - my; sxy += dx * dy; sxx += dx * dx; syy += dy * dy; }
  return sxy / Math.sqrt(sxx * syy || 1);
};

if (import.meta.url === `file://${process.argv[1]}`) {
  const out = [];
  for (const id of Object.keys(CORPORA)) {
    const source = readCorpus(id);
    const analysis = analyze(source);
    const rows = [];
    /* the artifact as shipped is one of the candidates */
    const shipped = nameStats(analysis, new Map());
    rows.push({ label: "as shipped", ...shipped, raw: Buffer.byteLength(source),
      br11: brotli(source).length, gzip9: gzipSync(Buffer.from(source)).length });
    for (const strategy of strategies(analysis)) {
      const mapping = assign(analysis, strategy.options);
      const text = rename(analysis, mapping);
      const stats = nameStats(analysis, mapping);
      rows.push({ label: strategy.label, ...stats, raw: Buffer.byteLength(text),
        br11: brotli(text).length, gzip9: gzipSync(Buffer.from(text)).length });
    }
    const br = rows.map((r) => r.br11);
    const report = {
      id,
      rows,
      correlation: {
        nameBytes: corr(rows.map((r) => r.nameBytes), br),
        entropy: corr(rows.map((r) => r.entropy), br),
        nameBits: corr(rows.map((r) => r.nameBits), br),
        distinct: corr(rows.map((r) => r.distinct), br),
        top5share: corr(rows.map((r) => r.top5share), br),
        rawSize: corr(rows.map((r) => r.raw), br),
      },
    };
    out.push(report);
    const best = rows.slice().sort((a, b) => a.br11 - b.br11)[0];
    const shippedRow = rows[0];
    console.log(`\n${id}`);
    console.log(`  ${rows.length} legal namings scored. best ${best.label} at ${best.br11} br11 ` +
      `(${best.br11 - shippedRow.br11 >= 0 ? "+" : ""}${best.br11 - shippedRow.br11} vs shipped, raw ${best.raw - shippedRow.raw >= 0 ? "+" : ""}${best.raw - shippedRow.raw})`);
    console.log(`  correlation with br11:  name bytes ${report.correlation.nameBytes.toFixed(3)}` +
      `   name entropy ${report.correlation.entropy.toFixed(3)}` +
      `   entropy x uses ${report.correlation.nameBits.toFixed(3)}` +
      `   distinct names ${report.correlation.distinct.toFixed(3)}`);
    const table = rows.slice().sort((a, b) => a.br11 - b.br11).slice(0, 5);
    for (const r of table) {
      console.log(`    ${r.label.padEnd(20)} br11 ${String(r.br11).padStart(6)}  raw ${String(r.raw).padStart(7)}` +
        `  distinct ${String(r.distinct).padStart(4)}  entropy ${r.entropy.toFixed(3)}  top5 ${(r.top5share * 100).toFixed(1)}%`);
    }
  }
  writeFileSync(join(here, "concentration.json"), JSON.stringify(out, null, 1));
  const all = out.flatMap((r) => r.rows.map((row) => ({ ...row, id: r.id })));
  console.log(`\nwrote concentration.json (${all.length} scored namings)`);
}
