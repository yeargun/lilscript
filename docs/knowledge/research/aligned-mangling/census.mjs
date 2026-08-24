#!/usr/bin/env node
/* What does a real Brotli stream of our own artifacts actually spend bits on,
   and how much of the static dictionary does it use?

   Decodes each corpus's q11 stream with the instrumented decoder from
   ../brotli-machine and reports the census. Diagnostic numbers (Node zlib
   Brotli 1.1.0), the same family as the other research harnesses. */
import { readFileSync, writeFileSync } from "node:fs";
import { brotliCompressSync, gzipSync, constants as Z } from "node:zlib";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { loadEngine } from "../brotli-machine/engine.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const root = "/Users/yeargun/lilscript";
const BM = loadEngine();

export const CORPORA = {
  "jquery-min": "benchmarks/popular/upstream/jquery/dist/jquery.min.js",
  "jquery-src": "benchmarks/popular/upstream/jquery/dist/jquery.js",
  "jquery-lil-raw": "benchmarks/popular/ports/jquery/jquery-lilscript.raw.js",
  "jquery-lil-min": "benchmarks/popular/build/jquery-lilscript.min.js",
  "glmatrix-js-vite": "benchmarks/popular/build/gl-matrix-vite-run.mjs",
  "glmatrix-lil-vite": "benchmarks/popular/build/gl-matrix-lilscript-vite-run.mjs",
  "glmatrix-lil-raw": "benchmarks/popular/build/gl-matrix-lilscript-raw.mjs",
};

export const readCorpus = (id) => readFileSync(join(root, CORPORA[id]), "utf8");

export function brotli(text, quality = 11, lgwin = 22) {
  const bytes = Buffer.from(text, "utf8");
  return brotliCompressSync(bytes, {
    params: {
      [Z.BROTLI_PARAM_QUALITY]: quality,
      [Z.BROTLI_PARAM_LGWIN]: lgwin,
      [Z.BROTLI_PARAM_SIZE_HINT]: bytes.length,
    },
  });
}

/* Bits per channel, counting each bit once (shortest field wins). */
function channelBits(dec, byteLength) {
  const owner = new Int32Array(byteLength * 8).fill(-1);
  const order = dec.map.map((m, i) => i)
    .sort((a, b) => (dec.map[b].end - dec.map[b].start) - (dec.map[a].end - dec.map[a].start));
  for (const i of order) {
    const m = dec.map[i];
    for (let b = Math.max(0, m.start); b < Math.min(m.end, owner.length); b++) owner[b] = i;
  }
  const totals = {};
  for (let b = 0; b < owner.length; b++) {
    const kind = owner[b] >= 0 ? dec.map[owner[b]].kind : "padding";
    totals[kind] = (totals[kind] || 0) + 1;
  }
  return totals;
}

export function census(id, text) {
  const compressed = brotli(text);
  const dec = BM.decode(new Uint8Array(compressed), { trace: false });
  const c = dec.counts;
  const bits = channelBits(dec, compressed.length);
  const top = [...c.cachedWords.entries()].sort((a, b) => b[1] - a[1]);
  const dictBytesByLength = [...c.dictLengths.entries()].sort((a, b) => a[0] - b[0]);
  return {
    id,
    raw: Buffer.byteLength(text, "utf8"),
    gzip9: gzipSync(Buffer.from(text, "utf8"), { level: 9 }).length,
    br11: compressed.length,
    metablocks: c.metablocks,
    commands: c.commands,
    literalBytes: c.literals,
    copyBytes: c.copyBytes,
    copies: c.copies,
    dictRefs: c.dictRefs,
    dictBytes: c.dictBytes,
    blockSwitches: c.blockSwitches,
    implicitDistances: c.implicitDistances,
    shortDistances: c.shortDistances,
    distinctDictEntries: c.cachedWords.size,
    fullDistances: c.fullDistances,
    distinctDistances: c.distances.size,
    nearMiss: [...c.nearMiss.entries()],
    topDistances: [...c.distances.entries()].sort((a, b) => b[1] - a[1]).slice(0, 10),
    topDictWords: top.slice(0, 25),
    dictLengths: dictBytesByLength,
    bitsByChannel: bits,
    meanCopy: c.copies ? c.copyBytes / c.copies : 0,
  };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const rows = [];
  for (const id of Object.keys(CORPORA)) {
    const text = readCorpus(id);
    const r = census(id, text);
    rows.push(r);
    const pct = (n) => ((n / r.raw) * 100).toFixed(1) + "%";
    const bytesOf = (k) => Math.round((r.bitsByChannel[k] || 0) / 8);
    console.log(`\n${id}  raw ${r.raw}  br11 ${r.br11}  (${(r.br11 / r.raw * 100).toFixed(1)}% of raw)`);
    console.log(`  output from: literals ${r.literalBytes} (${pct(r.literalBytes)}) · copies ${r.copyBytes} (${pct(r.copyBytes)}) · dictionary ${r.dictBytes} (${pct(r.dictBytes)})`);
    console.log(`  commands ${r.commands}, copies ${r.copies} (mean ${r.meanCopy.toFixed(1)} bytes), dictionary refs ${r.dictRefs} over ${r.distinctDictEntries} distinct entries`);
    console.log(`  distances: implicit ${r.implicitDistances} (${(r.implicitDistances / r.commands * 100).toFixed(1)}%), short-code ${r.shortDistances} (${(r.shortDistances / r.commands * 100).toFixed(1)}%)`);
    console.log(`  stream bits: literal ${bytesOf("literal")}B · cmd ${bytesOf("cmd")}B · dist ${bytesOf("dist")}B · codes ${bytesOf("code")}B · header ${bytesOf("mb") + bytesOf("map") + bytesOf("stream")}B · block ${bytesOf("block")}B`);
    console.log(`  top dictionary entries: ${r.topDictWords.slice(0, 12).map(([w, n]) => `${JSON.stringify(w)}×${n}`).join(" ")}`);
    const nm = Object.fromEntries(r.nearMiss);
    const nmTotal = r.fullDistances || 1;
    console.log(`  full distance codes ${r.fullDistances} over ${r.distinctDistances} distinct values; distance from nearest cached: ` +
      ["0", "1-3", "4-16", "17-64", "65-256", "257-4k", ">4k"].map((k) => `${k}:${((nm[k] || 0) / nmTotal * 100).toFixed(0)}%`).join(" "));
    console.log(`  most repeated distances: ${r.topDistances.slice(0, 6).map(([d, n]) => `${d}×${n}`).join(" ")}`);
  }
  writeFileSync(join(here, "census.json"), JSON.stringify(rows, (k, v) => (v instanceof Map ? [...v] : v), 1));
  console.log("\nwrote census.json");
}
