#!/usr/bin/env node
/* Builds docs/knowledge/research/brotli-machine.html: one self-contained file
   holding the RFC 7932 tables, a decoder, an encoder, and the page that drives
   them. Runs test.mjs first and refuses to write a page whose engine fails.

   Usage: node docs/knowledge/research/brotli-machine/render.mjs [--skip-tests] */
import { readFileSync, writeFileSync, readdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";
import zlib from "node:zlib";

const here = dirname(fileURLToPath(import.meta.url));
const out = join(here, "..", "brotli-machine.html");
const read = (p) => readFileSync(join(here, p), "utf8");

/* --- 1. self-tests ---------------------------------------------------- */
let tests = { pass: 0, fail: 0, streams: 0 };
if (!process.argv.includes("--skip-tests")) {
  let output;
  try {
    output = execFileSync(process.execPath, [join(here, "test.mjs")], { encoding: "utf8" });
  } catch (e) {
    process.stderr.write(e.stdout || "");
    throw new Error("engine tests failed; not writing the page");
  }
  const summary = /SUMMARY (\{.*\})/.exec(output);
  if (!summary) throw new Error("test.mjs printed no summary");
  const parsed = JSON.parse(summary[1]);
  const by = parsed.byCategory;
  tests = {
    pass: parsed.pass,
    fail: parsed.fail,
    streams: (by["decoder vs. real brotli streams"] || 0) + (by["decoder corner cases"] || 0),
    encoder: (by["encoder round trip through the real library"] || 0) +
             (by["encoder across parameter combinations"] || 0),
    fuzz: by["encoder fuzz"] || 0,
  };
  if (tests.fail) throw new Error("engine tests failed; not writing the page");
  console.log(`tests: ${tests.pass} passed`);
}

/* --- 2. samples, with reference sizes measured here ------------------- */
const sampleSource = [
  ["A web page fragment",
   "Dictionary words, a repeated attribute, and one long copy.",
   `<div class="container">\n  <p class="lead">Hello world</p>\n  <p class="lead">Hello world</p>\n</div>\n`],
  ["The alphabet, twice",
   "The smallest interesting stream: one long copy at a known distance.",
   "abcdefghijklmnopqrstuvwxyz abcdefghijklmnopqrstuvwxyz"],
  ["English prose",
   "Where context modelling earns its header bits.",
   `The compressor does not know what the text means. It knows only what it has already seen, and what the dictionary said before it saw anything at all. That is enough: the second sentence is cheaper than the first, and the third is cheaper than the second, because by then the machine has a model of what letters follow what.`],
  ["Minified JavaScript",
   "Short identifiers, repeated punctuation, and dictionary hits on keywords.",
   `function e(t,n){return t&&n?t.map(function(r){return r*n}):[]}function r(t){var n=document.createElement("div");n.className="item";n.textContent=t;return n}var i=[1,2,3,4,5];var o=e(i,3);for(var a=0;a<o.length;a++){document.body.appendChild(r(o[a]))}`],
  ["JSON records",
   "Structure that repeats exactly: the last-distance cache does the work.",
   JSON.stringify(Array.from({ length: 12 }, (_, i) => ({ id: i, name: `item ${i}`, active: i % 2 === 0, tags: ["alpha", "beta"] })), null, 1)],
  ["A stylesheet",
   "Selectors and units the static dictionary already knows.",
   `.card { display: flex; padding: 16px; border: 1px solid #38322a; }\n.card h2 { font-size: 18px; margin: 0 0 8px; }\n.card p { color: #8f877a; font-size: 14px; margin: 0; }\n.card:hover { border-color: #d8a15a; }\n`],
  ["Counting bytes",
   "No text structure at all — watch the literal codes flatten out.",
   Array.from({ length: 512 }, (_, i) => String.fromCharCode(32 + (i % 90))).join("")],
];
const samples = sampleSource.map(([name, note, text]) => {
  const buf = Buffer.from(text, "utf8");
  return {
    name, note, text,
    raw: buf.length,
    brotli11: zlib.brotliCompressSync(buf, {
      params: { [zlib.constants.BROTLI_PARAM_QUALITY]: 11, [zlib.constants.BROTLI_PARAM_SIZE_HINT]: buf.length },
    }).length,
    gzip9: zlib.gzipSync(buf, { level: 9 }).length,
  };
});

/* --- 3. bundle -------------------------------------------------------- */
const scriptFiles = readdirSync(join(here, "src")).filter((f) => /^\d\d-.*\.js$/.test(f)).sort();
const boot = scriptFiles.filter((f) => /^78-/.test(f));
const rest = scriptFiles.filter((f) => !/^78-/.test(f));

const tablesJs = read("data/tables.js");
const engineJs = rest.map((f) => read(join("src", f))).join("\n");
const buildFacts = {
  date: new Date().toISOString().slice(0, 10),
  tests,
  node: process.version,
  engineBytes: engineJs.length,
  tableBytes: tablesJs.length,
  modules: scriptFiles.length,
};

const script = [
  tablesJs,
  engineJs,
  `(function (BM) {
  BM.samples = ${JSON.stringify(samples)};
  BM.build = ${JSON.stringify(buildFacts)};
})(globalThis.BM || (globalThis.BM = {}));`,
  ...boot.map((f) => read(join("src", f))),
].join("\n");

const html = `<title>Brotli, the whole machine</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=IBM+Plex+Mono:ital,wght@0,400;0,500;0,600;1,400&family=IBM+Plex+Sans+Condensed:wght@500;600&family=IBM+Plex+Serif:ital,wght@0,400;0,600;1,400&display=swap">
<style>
${read("page.css")}
</style>
${read("page.html")}
<script>
${script.replace(/<\/script/gi, "<\\/script")}
</script>
`;

writeFileSync(out, html);
const size = html.length;
/* Record the page's own size for the provenance panel on the next build. */
console.log(`wrote ${out}`);
console.log(`  ${(size / 1024).toFixed(0)} KiB (${samples.length} samples, ${scriptFiles.length} engine/ui modules)`);
console.log(`  gzip ${(zlib.gzipSync(Buffer.from(html), { level: 9 }).length / 1024).toFixed(0)} KiB, brotli ${(zlib.brotliCompressSync(Buffer.from(html)).length / 1024).toFixed(0)} KiB`);
