#!/usr/bin/env node
/* The same measurements, on the three ports that matter: jquerylil, markedlil
   and solidlil.

   The corpora in census.mjs are benchmark artifacts inside this checkout. These
   are the real packages: what those projects actually publish. For each one we
   ask the two questions this folder answered on jQuery — is there a cheaper
   legal naming, and does the string pool want a different order — and then
   score the winner with the gate codec.

   Usage: node ports.mjs [--only <substring>] */
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";
import { analyze, rename } from "./scope.mjs";
import { assign, adaptiveAlphabet, ALPHABETS } from "./mangle.mjs";
import { nameStats } from "./concentration.mjs";
import { reorderPools, findPools, POOL_ORDERS } from "./pool.mjs";
import { brotli, census } from "./census.mjs";
import { twinStats } from "./twins.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const CODEC = "/Users/yeargun/lilscript/target/release/lilscript-codec";

export const PORTS = {
  /* jquerylil — the published package build */
  "jquerylil-raw": "/Users/yeargun/jquerylil/dist/jquery.raw.js",
  "jquerylil-esm": "/Users/yeargun/jquerylil/dist/jquery.esm.js",
  /* markedlil — compiler emit and the two tuned configs */
  "markedlil-raw": "/Users/yeargun/markedlil/dist/marked.raw.js",
  "markedlil-bytes": "/Users/yeargun/markedlil/dist/marked.bytes.js",
  "markedlil-gzip": "/Users/yeargun/markedlil/dist/marked.gzip.js",
  "markedlil-esm": "/Users/yeargun/markedlil/dist/marked.esm.js",
  /* solidlil — the compiler's reactive core, and the bundles that ship it */
  "solidlil-reactive": "/Users/yeargun/lilscript/labs/solid-client/packages/solidlil/reactive.generated.js",
  "solidlil-core": "/Users/yeargun/lilscript/labs/solid-client/artifacts/generated/solidlil-core-open.js",
  "solidlil-lsx-vite": "/Users/yeargun/lilscript/labs/solid-client/artifacts/generated/solidlil-lsx-vite.js",
  "solidlil-web": "/Users/yeargun/lilscript/labs/solid-client/artifacts/generated/solidlil-web.js",
  /* the JavaScript sides of the same pairs, for context */
  "solid-core (js)": "/Users/yeargun/lilscript/labs/solid-client/artifacts/generated/solid-core-open.js",
  "solid-lsx-vite (js)": "/Users/yeargun/lilscript/labs/solid-client/artifacts/generated/solid-lsx-vite.js",
  "solid-web (js)": "/Users/yeargun/lilscript/labs/solid-client/artifacts/generated/solid-web.js",
};

const score = (text) => ({
  raw: Buffer.byteLength(text, "utf8"),
  gzip9: gzipSync(Buffer.from(text, "utf8"), { level: 9 }).length,
  br11: brotli(text).length,
});

/* Gate codec, for the rows that matter. */
export function gateScore(paths) {
  if (!existsSync(CODEC)) return null;
  const out = execFileSync(CODEC, ["--json", ...paths], { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
  return JSON.parse(out).artifacts;
}

/* Every legal naming this folder knows how to generate. */
function* namings(analysis) {
  const alphabets = {
    abc: ALPHABETS.abc,
    etn: ALPHABETS.etn,
    adaptive: adaptiveAlphabet(analysis, { mode: "all" }),
    dialect: adaptiveAlphabet(analysis, { mode: "dialect" }),
    reversed: [...ALPHABETS.abc].reverse().join(""),
  };
  for (const [name, alphabet] of Object.entries(alphabets)) {
    for (const order of ["frequency", "firstUse", "source"]) {
      yield { label: `${order}/${name}`, options: { order, alphabet } };
    }
  }
}

export function analysePort(id, path, { renameModuleTopLevel = true } = {}) {
  const source = readFileSync(path, "utf8");
  const base = score(source);
  const analysis = analyze(source, { renameModuleTopLevel });
  const renamable = analysis.bindings.filter((b) => b.renamable).length;
  const shipped = nameStats(analysis, new Map());

  const rows = [];
  for (const naming of namings(analysis)) {
    const mapping = assign(analysis, naming.options);
    const text = rename(analysis, mapping);
    const stats = nameStats(analysis, mapping);
    rows.push({ label: naming.label, ...score(text), distinct: stats.distinct, entropy: stats.entropy, text });
  }
  rows.sort((a, b) => a.br11 - b.br11);
  const best = rows[0];

  const pools = findPools(source);
  const poolRows = [];
  if (pools.length) {
    for (const order of Object.keys(POOL_ORDERS)) {
      const { text } = reorderPools(source, order);
      poolRows.push({ order, ...score(text) });
    }
    poolRows.sort((a, b) => a.br11 - b.br11);
  }

  return {
    id, path, base, renameModuleTopLevel,
    bindings: analysis.bindings.length, renamable,
    shipped: { distinct: shipped.distinct, entropy: shipped.entropy, nameBytes: shipped.nameBytes },
    best: { label: best.label, raw: best.raw, gzip9: best.gzip9, br11: best.br11,
            distinct: best.distinct, entropy: best.entropy },
    bestText: best.text,
    namings: rows.map(({ text, ...rest }) => rest),
    pools: pools.length,
    poolEntries: pools.reduce((a, p) => a + p.items.length, 0),
    poolBest: poolRows[0] || null,
    poolRows,
  };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const flag = process.argv.indexOf("--only");
  const filter = flag >= 0 ? process.argv[flag + 1] : null;
  const out = [];
  for (const [id, path] of Object.entries(PORTS)) {
    if (filter && !id.includes(filter)) continue;
    if (!existsSync(path)) { console.log(`${id.padEnd(22)} MISSING ${path}`); continue; }
    let row;
    try { row = analysePort(id, path); }
    catch (e) { console.log(`${id.padEnd(22)} FAILED ${e.message}`); continue; }
    const dBr = row.best.br11 - row.base.br11;
    const dRaw = row.best.raw - row.base.raw;
    console.log(`\n${id}  ${row.base.raw} raw / ${row.base.gzip9} gzip / ${row.base.br11} br11` +
      `   ${row.renamable} renamable bindings, ${row.shipped.distinct} distinct names, entropy ${row.shipped.entropy.toFixed(2)}`);
    console.log(`  best legal naming: ${row.best.label.padEnd(18)} br11 ${dBr >= 0 ? "+" : ""}${dBr}` +
      `  raw ${dRaw >= 0 ? "+" : ""}${dRaw}  names ${row.shipped.distinct} → ${row.best.distinct}` +
      `  entropy ${row.shipped.entropy.toFixed(2)} → ${row.best.entropy.toFixed(2)}`);
    for (const n of row.namings.slice(0, 4)) {
      console.log(`      ${n.label.padEnd(18)} br11 ${String(n.br11 - row.base.br11).padStart(6)}  raw ${String(n.raw - row.base.raw).padStart(6)}  distinct ${String(n.distinct).padStart(4)}`);
    }
    if (row.pools) {
      const p = row.poolBest;
      console.log(`  string pool: ${row.poolEntries} entries in ${row.pools} run(s); best order ${p.order} br11 ${p.br11 - row.base.br11}  gzip ${p.gzip9 - row.base.gzip9}`);
    } else {
      console.log("  string pool: none");
    }
    const c = census(id, readFileSync(path, "utf8"));
    console.log(`  census: literals ${((c.literalBytes / c.raw) * 100).toFixed(1)}%  copies ${((c.copyBytes / c.raw) * 100).toFixed(1)}%` +
      `  dictionary ${((c.dictBytes / c.raw) * 100).toFixed(2)}% (${c.dictRefs} refs, ${c.dictRefs - c.distinctDictEntries} reused)` +
      `  implicit distances ${((c.implicitDistances / c.commands) * 100).toFixed(1)}%` +
      `  distance bits ${((c.bitsByChannel.dist || 0) / (c.br11 * 8) * 100).toFixed(0)}% of stream`);
    const t = twinStats(readFileSync(path, "utf8"));
    console.log(`  twins up to renaming: ${t.twinGroups} group(s), ${t.misalignedBytes} bytes differing only in names`);
    out.push({ ...row, bestText: undefined, census: {
      literalBytes: c.literalBytes, copyBytes: c.copyBytes, dictBytes: c.dictBytes, dictRefs: c.dictRefs,
      reusedDictEntries: c.dictRefs - c.distinctDictEntries, commands: c.commands,
      implicitPct: (c.implicitDistances / c.commands) * 100,
      distBits: c.bitsByChannel.dist || 0, litBits: c.bitsByChannel.literal || 0,
      cmdBits: c.bitsByChannel.cmd || 0,
    }, twins: { groups: t.twinGroups, misalignedBytes: t.misalignedBytes } });
  }
  writeFileSync(join(here, "ports.json"), JSON.stringify(out, null, 1));
  console.log("\nwrote ports.json");
}
