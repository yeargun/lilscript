#!/usr/bin/env node
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const here = fileURLToPath(new URL(".", import.meta.url));
const results = JSON.parse(readFileSync(join(here, "results.json"), "utf8"));
const extra = JSON.parse(readFileSync(join(here, "extra.json"), "utf8"));

function num(n) {
  if (n == null) return "";
  return String(n);
}

function deltaClass(n) {
  if (n == null || n === 0) return "num";
  return n < 0 ? "num win" : "num lose";
}

function signed(n) {
  if (n == null) return "";
  if (n > 0) return `+${n}`;
  return String(n);
}

function corpusTable(name, block) {
  const rows = block.rows
    .map((row) => {
      const dGz = row.gzip9 - block.baseline.gzip9;
      const d5 = row.br5 - block.baseline.br5;
      return `<tr>
        <td>${row.id}</td>
        <td class="num">${row.raw}</td>
        <td class="num">${row.gzip9}</td>
        <td class="num">${row.br5}</td>
        <td class="num">${row.br11}</td>
        <td class="${deltaClass(dGz)}">${signed(dGz)}</td>
        <td class="${deltaClass(d5)}">${signed(d5)}</td>
        <td class="${deltaClass(row.dBr11)}">${signed(row.dBr11)}</td>
      </tr>`;
    })
    .join("\n");
  return `<h3 id="c-${name}">${name}</h3>
  <p class="tiny">${block.file} · baseline raw ${block.baseline.raw} · br11 ${block.baseline.br11}</p>
  <table>
    <thead><tr><th>mutation</th><th class="num">raw</th><th class="num">gzip9</th><th class="num">br5</th><th class="num">br11</th><th class="num">Δ gzip</th><th class="num">Δ br5</th><th class="num">Δ br11</th></tr></thead>
    <tbody>${rows}</tbody>
  </table>`;
}

const corpusHtml = Object.entries(results.corpora)
  .map(([name, block]) => corpusTable(name, block))
  .join("\n");

const auditRows = [
  ...Object.entries(results.audits).map(([id, a]) => ({ id, ...a })),
  ...Object.entries(extra.moreAudits).map(([id, a]) => ({ id: a.file || id, ...a })),
]
  .sort((a, b) => a.br11 - b.br11)
  .filter((row, i, all) => all.findIndex((x) => x.raw === row.raw && x.br11 === row.br11 && x.id !== row.id) >= 0 ? all.findIndex((x) => x.raw === row.raw && x.br11 === row.br11) === i : true);

const seenAudit = new Set();
const auditHtml = Object.entries(extra.moreAudits)
  .map(([, a]) => a)
  .sort((a, b) => a.br11 - b.br11)
  .filter((a) => {
    const k = `${a.raw}:${a.br11}`;
    if (seenAudit.has(k) && a.file !== "jquery-audit-current.raw.js") return false;
    seenAudit.add(k);
    return true;
  })
  .map(
    (a) => `<tr><td>${a.file}</td><td class="num">${a.raw}</td><td class="num">${a.gzip9}</td><td class="num">${a.br5}</td><td class="num">${a.br11}</td></tr>`,
  )
  .join("\n");

const surgicalHtml = Object.entries(extra.surgical)
  .map(([name, rows]) => {
    const body = rows
      .map(
        (row) => `<tr>
          <td>${row.id}</td>
          <td class="num">${row.raw}</td>
          <td class="num">${row.gzip9}</td>
          <td class="num">${row.br11}</td>
          <td class="${deltaClass(row.dGzip)}">${signed(row.dGzip)}</td>
          <td class="${deltaClass(row.dBr11)}">${signed(row.dBr11)}</td>
        </tr>`,
      )
      .join("\n");
    const top = extra.stats[name]?.topLocals?.map((x) => `${x.n} ${x.c}`).join(", ") || "";
    return `<h3>${name}</h3><p class="tiny">top locals: ${top}</p>
      <table><thead><tr><th>probe</th><th class="num">raw</th><th class="num">gzip9</th><th class="num">br11</th><th class="num">Δ gzip</th><th class="num">Δ br11</th></tr></thead><tbody>${body}</tbody></table>`;
  })
  .join("\n");

const invHtml = extra.inversions
  .map(
    (row) => `<tr><td>${row.corpus}</td><td>${row.id}</td>
      <td class="${deltaClass(row.dGz)}">${signed(row.dGz)}</td>
      <td class="${deltaClass(row.d5)}">${signed(row.d5)}</td>
      <td class="${deltaClass(row.d11)}">${signed(row.d11)}</td></tr>`,
  )
  .join("\n");

const monaco = results.monaco;
const monacoFull = monaco.fullBaseline;
const monacoRows = monaco.slice400k
  .map(
    (row) => `<tr><td>${row.id}</td><td class="num">${row.raw}</td><td class="num">${row.gzip9}</td><td class="num">${row.br11}</td><td class="${deltaClass(row.dBr11)}">${signed(row.dBr11)}</td></tr>`,
  )
  .join("\n");

const html = `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Brotli global-mangle lab</title>
<style>
:root {
  --bg: #12110f; --bg2: #1b1916; --bg3: #24211c; --ink: #ece7dc;
  --muted: #9a9283; --line: #3a342b; --accent: #d8a15a;
  --win: #7dba7a; --lose: #d37a6a;
  --mono: "IBM Plex Mono", ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  --sans: "Iowan Old Style", "Palatino Linotype", Palatino, Georgia, serif;
}
* { box-sizing: border-box; }
html { scroll-behavior: smooth; }
body { margin: 0; background: var(--bg); color: var(--ink); font: 17px/1.5 var(--sans); }
a { color: var(--accent); }
code, pre, table { font-family: var(--mono); }
header { padding: 48px 28px 28px; border-bottom: 1px solid var(--line); background: var(--bg2); }
header p { max-width: 72ch; color: var(--muted); }
nav {
  display: flex; flex-wrap: wrap; gap: 12px 18px; padding: 14px 28px;
  border-bottom: 1px solid var(--line); position: sticky; top: 0;
  background: #12110fef; z-index: 2; font-size: 13px; font-family: var(--mono);
}
nav a { text-decoration: none; }
main { padding: 28px; max-width: 1240px; }
section { margin: 0 0 56px; }
h1 { font-size: 34px; line-height: 1.15; margin: 0 0 12px; }
h2 { font-size: 24px; margin: 0 0 12px; }
h3 { font-size: 18px; margin: 28px 0 8px; }
p, li { max-width: 74ch; }
.lede { font-size: 20px; max-width: 68ch; color: var(--ink); }
.tiny { font-size: 12px; color: var(--muted); }
table { width: 100%; border-collapse: collapse; font-size: 12px; }
th, td { border-bottom: 1px solid var(--line); padding: 6px 8px; text-align: left; vertical-align: top; }
th { color: var(--muted); font-weight: 500; }
.num { text-align: right; font-variant-numeric: tabular-nums; }
.win { color: var(--win); }
.lose { color: var(--lose); }
.pill {
  display: inline-block; border: 1px solid var(--line); padding: 1px 7px;
  font-family: var(--mono); font-size: 11px; color: var(--muted);
}
.callout { border-left: 3px solid var(--accent); padding: 2px 0 2px 14px; margin: 16px 0; }
</style>
</head>
<body>
<header>
  <div class="pill">diagnostic Node zlib · brotli 1.1.0 generic q11 lgwin=22 · gzip-9 · not lilscript-codec</div>
  <h1>Global Brotli mangling lab</h1>
  <p class="lede">Hundred-kilobyte jQuery / gl-matrix / Monaco mutations. Green is smaller than that file’s baseline. Several probes are semantically illegal. Playbook starts at <a href="README.md">README.md</a>.</p>
</header>
<nav>
  <a href="00-thesis.md">thesis</a>
  <a href="#corpora">mutations</a>
  <a href="#surgical">color merge</a>
  <a href="#inversions">inversions</a>
  <a href="#audits">audits</a>
  <a href="#monaco">monaco</a>
  <a href="11-playbook.md">playbook</a>
</nav>
<main>
<section>
  <p class="callout">The large legal-looking lever is cross-scope reuse. The large <em>ugly</em> lever is merging the hottest local into <code>e</code>/<code>t</code> (often illegal). ROM words as names lose. gzip and q11 disagree on layout.</p>
  <p class="tiny">Generated ${results.generatedAt} · harness ${results.elapsedMs} ms · ${extra.inversions.length} ranking inversions</p>
</section>
<section id="corpora">
  <h2>24 mutations × 9 corpora</h2>
  ${corpusHtml}
</section>
<section id="surgical">
  <h2>Surgical: one hottest name</h2>
  <p>Usually illegal if the target letter is already a local in the same scope. Raw unchanged.</p>
  ${surgicalHtml}
</section>
<section id="inversions">
  <h2>gzip / q5 / q11 sign disagreements</h2>
  <table>
    <thead><tr><th>corpus</th><th>mutation</th><th class="num">Δ gzip</th><th class="num">Δ br5</th><th class="num">Δ br11</th></tr></thead>
    <tbody>${invHtml}</tbody>
  </table>
</section>
<section id="audits">
  <h2>In-tree jQuery audit emits</h2>
  <p class="tiny">Duplicate byte-identical files collapsed to the first name. <code>audit-slim</code> is a smaller program.</p>
  <table>
    <thead><tr><th>file</th><th class="num">raw</th><th class="num">gzip9</th><th class="num">br5</th><th class="num">br11</th></tr></thead>
    <tbody>${auditHtml}</tbody>
  </table>
</section>
<section id="monaco">
  <h2>Monaco LilScript IDE</h2>
  <p>Full file raw ${num(monacoFull.raw)} · gzip ${num(monacoFull.gzip9)} · br11 <strong>${num(monacoFull.br11)}</strong> (q5 not scored). Prefix mutations below (last <code>;\\n</code> before 400k = ${monaco.slice400k[0].raw} bytes).</p>
  <table>
    <thead><tr><th>mutation</th><th class="num">raw</th><th class="num">gzip9</th><th class="num">br11</th><th class="num">Δ br11</th></tr></thead>
    <tbody>${monacoRows}</tbody>
  </table>
  <p class="tiny">Independent chunks of jquery-lil-raw: whole ${results.chunks["jquery-lil-raw-whole"].br11} · 64k ${results.chunks["jquery-lil-raw-64k"].br11} · 32k ${results.chunks["jquery-lil-raw-32k"].br11}</p>
</section>
</main>
</body>
</html>
`;

writeFileSync(join(here, "lab.html"), html);
console.log("wrote lab.html", html.length);
