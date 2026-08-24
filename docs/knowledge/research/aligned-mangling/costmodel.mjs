#!/usr/bin/env node
/* markedlil compiles one source tree four times, changing a single knob each
   time. Score all of them with the gate codec — and run all 680 CommonMark and
   GFM spec cases through each, because a size comparison between builds that
   do not compute the same thing is not a size comparison.

   That check is why this file exists: `cost_model = "raw"` produces the
   smallest artifact of the family and fails two spec cases the others pass. */
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const CODEC = "/Users/yeargun/lilscript/target/release/lilscript-codec";
const DIST = "/Users/yeargun/markedlil/dist";
const WORK = "/private/tmp/claude-501/-Users-yeargun-lilscript/68d4d12f-89ad-4d08-8494-1336a12a22e8/scratchpad";

export const VARIANTS = [
  ["marked.esm.js", "what the package publishes"],
  ["marked.raw.js", 'cost_model = "brotli"'],
  ["marked.gzip.js", 'cost_model = "gzip"'],
  ["marked.closed.js", 'cost_model = "brotli", extern_fields = false'],
  ["marked.bytes.js", 'cost_model = "raw"'],
];

const SPECS = [
  "/Users/yeargun/markedlil/test/specs/commonmark.0.31.2.json",
  "/Users/yeargun/markedlil/test/specs/gfm.0.29.json",
];

function specCases() {
  const cases = [];
  for (const spec of SPECS.filter(existsSync)) {
    for (const c of JSON.parse(readFileSync(spec, "utf8"))) {
      const markdown = c.markdown ?? c.md ?? c.input;
      if (typeof markdown !== "string") continue;
      cases.push({ markdown, html: c.html, options: c.options, example: c.example,
                   section: c.section, spec: spec.split("/").pop().replace(".json", "") });
    }
  }
  return cases;
}

async function parseAll(file, cases, tag) {
  /* copy to .mjs so node treats it as a module wherever it runs */
  const copy = join(WORK, `costmodel-${tag}.mjs`);
  writeFileSync(copy, readFileSync(file, "utf8"));
  const module = await import(pathToFileURL(copy).href + `?v=${Math.random()}`);
  const parse = module.parse || module.marked || module.default;
  return cases.map((c) => {
    try { return String(parse(c.markdown, c.options || undefined)); }
    catch (e) { return `THREW ${e && e.message}`; }
  });
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const files = VARIANTS.filter(([f]) => existsSync(join(DIST, f)));
  const codec = JSON.parse(execFileSync(CODEC, ["--json", ...files.map(([f]) => join(DIST, f))],
    { encoding: "utf8" }));
  const cases = specCases();
  const outputs = {};
  let i = 0;
  for (const [file] of files) outputs[file] = await parseAll(join(DIST, file), cases, i++);

  const rows = files.map(([file, knob]) => {
    const sizes = codec.artifacts.find((a) => a.path.endsWith("/" + file));
    const wrong = [];
    for (let k = 0; k < cases.length; k++) {
      if (cases[k].html !== undefined && outputs[file][k] !== cases[k].html) wrong.push(k);
    }
    return { file, knob, raw: sizes.raw, gzip9: sizes.gzip9, br11: sizes.brotli11,
             specFailures: wrong.length, failing: wrong };
  });

  /* Which builds compute the same thing? */
  const groups = [];
  for (const row of rows) {
    const match = groups.find((g) => outputs[g.files[0]].every((v, k) => v === outputs[row.file][k]));
    if (match) match.files.push(row.file);
    else groups.push({ files: [row.file] });
  }
  const baseline = groups.sort((a, b) => b.files.length - a.files.length)[0];
  const agreeing = new Set(baseline.files);

  console.log(`${cases.length} spec cases, ${rows.length} builds\n`);
  console.log("build".padEnd(20) + "knob".padEnd(44) + "raw".padStart(7) + "gzip".padStart(8) +
    "brotli".padStart(8) + "  spec failures");
  for (const row of rows) {
    console.log(row.file.padEnd(20) + row.knob.padEnd(44) + String(row.raw).padStart(7) +
      String(row.gzip9).padStart(8) + String(row.br11).padStart(8) +
      `  ${row.specFailures}${agreeing.has(row.file) ? "" : "  ← computes something else"}`);
  }
  const others = rows.filter((r) => !agreeing.has(r.file));
  for (const row of others) {
    const extra = row.failing.filter((k) => {
      const peer = rows.find((r) => agreeing.has(r.file));
      return !peer.failing.includes(k);
    });
    console.log(`\n${row.file} fails ${extra.length} case(s) the others pass:`);
    for (const k of extra.slice(0, 5)) {
      console.log(`  ${cases[k].spec} #${cases[k].example} (${cases[k].section})`);
      console.log(`     markdown: ${JSON.stringify(cases[k].markdown.slice(0, 70))}`);
      console.log(`     expected: ${JSON.stringify((cases[k].html || "").slice(0, 90))}`);
      console.log(`     got:      ${JSON.stringify(outputs[row.file][k].slice(0, 90))}`);
    }
  }
  const correct = rows.filter((r) => agreeing.has(r.file)).sort((a, b) => a.br11 - b.br11);
  console.log(`\nsmallest build that computes the same thing as the rest: ${correct[0].file} at ${correct[0].br11} Brotli` +
    `; the package publishes ${rows[0].file} at ${rows[0].br11} (+${rows[0].br11 - correct[0].br11})`);

  writeFileSync(join(here, "costmodel.json"), JSON.stringify({
    codec: codec.codecs, cases: cases.length, rows,
    agreeing: [...agreeing],
    divergent: others.map((r) => ({
      file: r.file, knob: r.knob,
      extraFailures: r.failing.filter((k) => !rows.find((x) => agreeing.has(x.file)).failing.includes(k))
        .map((k) => ({ spec: cases[k].spec, example: cases[k].example, section: cases[k].section,
                       markdown: cases[k].markdown, expected: cases[k].html, got: outputs[r.file][k] })),
    })),
  }, null, 1));
  console.log("\nwrote costmodel.json");
}
