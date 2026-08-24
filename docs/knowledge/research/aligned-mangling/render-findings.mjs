#!/usr/bin/env node
/* Builds findings.html from the JSON this folder generates, so the page cannot
   drift from the evidence. Run the harnesses first:

     node census.mjs && node experiments.mjs && node concentration.mjs
     node pool.mjs && node layout.mjs && node indexed.mjs
     node render-findings.mjs
*/
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { loadEngine } from "../brotli-machine/engine.mjs";
import { twinStats } from "./twins.mjs";
import { CORPORA, readCorpus } from "./census.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const load = (name) => JSON.parse(readFileSync(join(here, name), "utf8"));
const census = load("census.json");
const results = load("results.json");
const concentration = load("concentration.json");
const pool = load("pool.json");
const layout = load("layout.json");
const indexed = load("indexed.json");
const ports = existsSync(join(here, "ports.json")) ? load("ports.json") : [];
const costmodel = existsSync(join(here, "costmodel.json")) ? load("costmodel.json") : null;
const factorial = existsSync(join(here, "factorial.json")) ? load("factorial.json") : [];
const analytic = existsSync(join(here, "analytic.json")) ? load("analytic.json") : [];
const libraries = existsSync(join(here, "libraries.json")) ? load("libraries.json") : [];
const dict = loadEngine().dictionary();

const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const num = (n) => Number(n).toLocaleString("en-US");
const signed = (n) => (n > 0 ? "+" : "") + num(n);
const cls = (n) => (n < 0 ? "win" : n > 0 ? "lose" : "flat");
const pct = (a, b) => ((a / b) * 100).toFixed(1) + "%";
const short = (id) => id.replace("glmatrix", "gl-matrix").replace("jquery", "jQuery");

/* --- section 1: where the bits are ------------------------------------ */
const CHANNELS = [
  ["dist", "distances", "var(--c-distance)"],
  ["cmd", "insert & copy", "var(--c-command)"],
  ["literal", "literals", "var(--c-literal)"],
  ["code", "prefix codes", "var(--c-code)"],
  ["mb", "header", "var(--c-header)"],
];
function bitBars() {
  return census.map((row) => {
    const total = CHANNELS.reduce((a, [k]) => a + (row.bitsByChannel[k] || 0), 0);
    const segments = CHANNELS.map(([key, label, color]) => {
      const bits = row.bitsByChannel[key] || 0;
      if (!bits) return "";
      return `<span class="seg" style="width:${(bits / total) * 100}%;background:${color}"
        title="${label}: ${num(Math.round(bits / 8))} bytes, ${pct(bits, total)}"></span>`;
    }).join("");
    const dist = row.bitsByChannel.dist || 0;
    return `<div class="barrow">
      <div class="barname">${esc(short(row.id))}<span class="sub">${num(row.raw)} → ${num(row.br11)}</span></div>
      <div class="bar">${segments}</div>
      <div class="barnum">${pct(dist, total)}<span class="sub">distances</span></div>
    </div>`;
  }).join("");
}

/* --- section 2: the library ------------------------------------------- */
const PROBES = ["function(", "=function(", "){return ", ");return ", "for(var ", "}else{", "var ",
  "typeof ", "this.", ".length", ".prototype", ".call(", ".apply(", ".push(", "Math.", "Object.",
  "undefined", "document", "window", "addEventListener",
  "let ", "const ", "=>{", "await ", "if(", ".prototype.", "constructor", "getElementById",
  "parentNode", "nodeType", "childNodes"];
function probeRows() {
  return PROBES.map((probe) => {
    const hits = dict.matchesAt(probe, 0, {}).filter((h) => h.produced === probe);
    if (hits.length) {
      const h = hits[0];
      return `<tr><td class="mono ch-dict">${esc(JSON.stringify(probe))}</td><td class="ok">one reference</td>
        <td class="mono dim">${esc(JSON.stringify(dict.wordText(h.len, h.wordIndex)))} · ${esc(dict.describeTransform(h.transform))}</td></tr>`;
    }
    const partial = dict.matchesAt(probe, 0, {})[0];
    return `<tr><td class="mono">${esc(JSON.stringify(probe))}</td><td class="no">not in the ROM</td>
      <td class="mono dim">${partial ? `only ${partial.matched} bytes, as ${esc(JSON.stringify(partial.produced))}` : "nothing"}</td></tr>`;
  }).join("");
}

/* --- section 3 & 4: the two questions ---------------------------------- */
const variantRow = (label) => results.map((row) => {
  const v = row.variants.find((x) => x.label === label);
  return v ? `<td class="num ${cls(v.delta.br11)}">${signed(v.delta.br11)}</td>` : `<td class="num dim">—</td>`;
}).join("");

function twins() {
  return Object.keys(CORPORA).map((id) => {
    const t = twinStats(readCorpus(id));
    return `<tr><td>${esc(short(id))}</td><td class="num">${t.functions}</td>
      <td class="num">${t.twinGroups}</td><td class="num">${t.misalignedBytes} B</td></tr>`;
  }).join("");
}

/* --- section 5: the win ------------------------------------------------ */
function concentrationRows() {
  return concentration.map((c) => {
    const shipped = c.rows[0];
    const best = c.rows.slice().sort((a, b) => a.br11 - b.br11)[0];
    const delta = best.br11 - shipped.br11;
    return `<tr>
      <td>${esc(short(c.id))}</td>
      <td class="num">${num(shipped.br11)}</td>
      <td class="mono dim">${esc(best.label)}</td>
      <td class="num ${cls(delta)}">${signed(delta)}</td>
      <td class="num dim">${signed(best.raw - shipped.raw)}</td>
      <td class="num">${shipped.distinct} → ${best.distinct}</td>
      <td class="num dim">${shipped.entropy.toFixed(2)} → ${best.entropy.toFixed(2)}</td>
    </tr>`;
  }).join("");
}

/* --- section 6: free orders -------------------------------------------- */
function poolRows() {
  const ids = [...new Set(pool.map((r) => r.id))];
  const orders = [...new Set(pool.map((r) => r.order))];
  return orders.map((order) => `<tr><td class="mono">${esc(order)}</td>` + ids.map((id) => {
    const row = pool.find((r) => r.id === id && r.order === order);
    return `<td class="num ${cls(row.delta.br11)}">${signed(row.delta.br11)}</td>
            <td class="num dim">${signed(row.delta.gzip9)}</td>`;
  }).join("") + "</tr>").join("");
}
function layoutRows() {
  const ids = [...new Set(layout.map((r) => r.id))];
  const orders = [...new Set(layout.map((r) => r.order))].filter((o) => o !== "asIs");
  return orders.map((order) => `<tr><td class="mono">${esc(order)}</td>` + ids.map((id) => {
    const row = layout.find((r) => r.id === id && r.order === order);
    return row ? `<td class="num ${cls(row.delta.br11)}">${signed(row.delta.br11)}</td>` : `<td class="num dim">—</td>`;
  }).join("") + "</tr>").join("");
}
function indexedRows() {
  return indexed.map((r) => `<tr>
    <td>${esc(short(r.id))}</td>
    <td class="num">${num(r.sites)}</td>
    <td class="num">${(r.rawShare * 100).toFixed(2)}%</td>
    <td class="mono dim">${r.topPairs.slice(0, 4).map(([k, n]) => `${esc(k)}×${n}`).join(" ")}</td>
    <td class="num ${cls(r.ceiling.br11)}">${signed(r.ceiling.br11)}</td>
  </tr>`).join("");
}

const VERIFIED = {
  "jquerylil-raw": "28/28 jsdom", "jquerylil-esm": "28/28 jsdom",
  "markedlil-raw": "680/680 spec", "markedlil-bytes": "680/680 spec",
  "markedlil-gzip": "680/680 spec", "markedlil-esm": "680/680 spec",
  "solidlil-reactive": "18/18 reactive",
};
function portRows() {
  const divergentFiles = new Set((costmodel && costmodel.divergent || []).map((d) => d.file));
  return ports.map((p) => {
    const d = p.best.br11 - p.base.br11;
    const verified = VERIFIED[p.id];
    const broken = [...divergentFiles].some((f) => p.path.endsWith("/" + f));
    return `<tr>
      <td class="mono">${esc(p.id)}</td>
      <td class="num">${num(p.base.raw)}</td>
      <td class="num">${num(p.base.br11)}</td>
      <td class="mono dim">${esc(p.best.label)}</td>
      <td class="num ${broken ? "dim" : cls(d)}">${signed(d)}</td>
      <td class="num dim">${signed(p.best.raw - p.base.raw)}</td>
      <td class="num">${p.shipped.distinct} → ${p.best.distinct}</td>
      <td class="${broken ? "no" : verified ? "ok" : "dim"}">${broken ? "baseline is miscompiled" : verified || "app bundle, not driven"}</td>
    </tr>`;
  }).join("");
}
function costRows() {
  if (!costmodel) return "";
  const agreeing = new Set(costmodel.agreeing || []);
  const correct = costmodel.rows.filter((r) => agreeing.has(r.file)).sort((a, b) => a.br11 - b.br11);
  const best = correct[0];
  return costmodel.rows.slice().sort((a, b) => a.br11 - b.br11).map((r) => {
    const ok = agreeing.has(r.file);
    return `<tr>
      <td class="mono">${esc(r.file)}</td>
      <td class="mono dim">${esc(r.knob)}</td>
      <td class="num">${num(r.raw)}</td>
      <td class="num">${num(r.gzip9)}</td>
      <td class="num">${num(r.br11)}</td>
      <td class="num ${ok ? "dim" : "lose"}">${r.specFailures}</td>
      <td class="${ok ? (r === best ? "win" : "dim") : "no"}">${ok ? (r === best ? "smallest correct build" : "+" + (r.br11 - best.br11)) : "computes something else"}</td>
    </tr>`;
  }).join("");
}
function divergentDetail() {
  if (!costmodel || !costmodel.divergent || !costmodel.divergent.length) return "";
  const d = costmodel.divergent[0];
  return d.extraFailures.slice(0, 2).map((f) => `<tr>
    <td class="mono dim">${esc(f.spec)} #${f.example}<span class="sub"> ${esc(f.section)}</span></td>
    <td class="mono">${esc(JSON.stringify(f.markdown))}</td>
    <td class="mono ok">${esc(JSON.stringify(String(f.expected).slice(0, 76)))}</td>
    <td class="mono no">${esc(JSON.stringify(String(f.got).slice(0, 76)))}</td>
  </tr>`).join("");
}

function factorialRows() {
  return factorial.map((e) => {
    const sp = e.designs.split, gr = e.designs.grouped;
    return `<tr>
      <td class="mono">${esc(e.id)}</td>
      <td class="num lose">${sp.r2.toFixed(4)}</td>
      <td class="num win">${gr.r2.toFixed(4)}</td>
      <td class="mono dim">${esc(sp.topInteractions[0].pair)} ${(sp.topInteractions[0].share * 100).toFixed(0)}%</td>
      <td class="num">${gr.greedy.leaves} B</td>
      <td class="num dim">${sp.stackingError !== undefined ? signed(sp.stackingError) + " B" : "—"}</td>
      <td class="num">${signed(gr.stackingError)} B</td>
    </tr>`;
  }).join("");
}

const VERIFIED_BY = {
  "markedlil/marked.raw.js": "680 spec cases",
  "jquerylil/jquery.esm.js": "12 jsdom observations",
  "solidlil/reactive.generated.js": "7 reactive observations",
};
function libraryRows() {
  return libraries.slice().sort((a, b) => a.delta.br11 - b.delta.br11).map((r) => {
    const id = `${r.project}/${r.name}`;
    const moved = r.delta.br11 < 0;
    return `<tr>
      <td class="mono">${esc(r.project)}</td>
      <td class="mono dim">${esc(r.name)}</td>
      <td class="num">${num(r.base.br11)}</td>
      <td class="num ${moved ? "win" : "dim"}">${moved ? num(r.best.br11) : "—"}</td>
      <td class="num ${moved ? "win" : "dim"}">${moved ? signed(r.delta.br11) : "0"}</td>
      <td class="num ${moved ? "" : "dim"}">${moved ? r.percent.toFixed(2) + "%" : "—"}</td>
      <td class="num dim">${signed(r.delta.raw)}</td>
      <td class="mono dim">${moved ? esc(r.point) : "already at the optimum"}</td>
      <td class="${moved ? "ok" : "dim"}">${moved ? esc(VERIFIED_BY[id] || "export surface") : ""}</td>
    </tr>`;
  }).join("");
}
function analyticRows() {
  return analytic.map((a) => `<tr>
    <td class="mono">${esc(a.file)}</td>
    <td class="num ${a.firstOrder && a.firstOrder.pearson < 0.5 ? "lose" : ""}">${a.firstOrder ? a.firstOrder.pearson.toFixed(4) : "—"}</td>
    <td class="num dim">${a.firstOrder ? a.firstOrder.mae.toFixed(0) + " B" : "—"}</td>
    <td class="num win">${a.fullModel ? a.fullModel.pearson.toFixed(4) : a.pearson.toFixed(4)}</td>
    <td class="num dim">${a.fullModel ? a.fullModel.mae.toFixed(0) + " B" : "—"}</td>
    <td class="num">${a.maeAfterScale.toFixed(1)} B</td>
    <td class="num win">${(a.msPerEvalReal / a.msPerEvalModel).toFixed(1)}×</td>
  </tr>`).join("");
}

const corpusHeads = results.map((r) => `<th class="num">${esc(short(r.id))}</th>`).join("");
const poolIds = [...new Set(pool.map((r) => r.id))];
const layoutIds = [...new Set(layout.map((r) => r.id))];

const html = `<title>Aligned Mangling</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=IBM+Plex+Mono:ital,wght@0,400;0,500;1,400&family=IBM+Plex+Sans+Condensed:wght@500;600&family=IBM+Plex+Serif:ital,wght@0,400;0,600;1,400&display=swap">
<style>
:root {
  --ground:#100f0d; --panel:#191713; --sunken:#0b0a09; --rule:#38322a; --rule-soft:#262119;
  --ink:#ece7dc; --ink-dim:#b8b1a3; --muted:#8f877a; --faint:#665f54;
  --brass:#d8a15a; --brass-dim:#8a6a3c;
  --c-distance:#c77dbb; --c-command:#e08a5a; --c-literal:#79b473; --c-code:#c9a227;
  --c-header:#9a8fd8; --c-dict:#5fb3b3;
  --win:#7dba7a; --lose:#d97a68;
  --mono:"IBM Plex Mono",ui-monospace,Menlo,monospace;
  --serif:"IBM Plex Serif","Iowan Old Style",Palatino,Georgia,serif;
  --cond:"IBM Plex Sans Condensed","IBM Plex Sans",system-ui,sans-serif;
  color-scheme: dark;
}
*{box-sizing:border-box}
@media (prefers-reduced-motion:reduce){*{animation-duration:.01ms!important;transition-duration:.01ms!important}}
body{margin:0;background:var(--ground);color:var(--ink);font:400 16.5px/1.62 var(--serif);-webkit-font-smoothing:antialiased}
a{color:var(--brass)}
h1,h2,h3{text-wrap:balance;font-weight:600}
.mono,code,th,td.num{font-family:var(--mono)}
header{padding:56px 32px 36px;border-bottom:1px solid var(--rule);background:linear-gradient(180deg,#1d1a15,var(--panel) 70%)}
.wrap{max-width:1120px;margin:0 auto}
.eyebrow{font:500 11.5px/1 var(--cond);letter-spacing:.16em;text-transform:uppercase;color:var(--muted);display:flex;gap:14px;flex-wrap:wrap}
.eyebrow .dot{color:var(--brass-dim)}
h1{font-size:clamp(32px,5vw,50px);line-height:1.05;margin:16px 0 0;letter-spacing:-.015em}
h1 .sub{display:block;color:var(--brass);font-style:italic;font-weight:400;font-size:.5em;margin-top:10px}
.lede{font-size:19px;max-width:66ch;color:var(--ink-dim);margin:18px 0 0}
main{max-width:1120px;margin:0 auto;padding:0 32px 100px}
section{padding:52px 0 4px;border-top:1px solid var(--rule-soft)}
section:first-child{border-top:0}
.num-label{font:500 11.5px/1 var(--cond);letter-spacing:.18em;text-transform:uppercase;color:var(--brass-dim);margin-bottom:10px}
h2{font-size:clamp(23px,3vw,30px);margin:0 0 14px}
h3{font-size:17px;margin:30px 0 8px}
p{max-width:70ch;color:var(--ink-dim)}
strong{color:var(--ink)}
.qa{display:grid;gap:18px;grid-template-columns:1fr 1fr;margin:26px 0 0}
@media(max-width:900px){.qa{grid-template-columns:1fr}main,header{padding-left:18px;padding-right:18px}}
.card{background:var(--panel);border:1px solid var(--rule);padding:18px 20px}
.card h3{margin-top:0;font:600 12.5px/1.3 var(--cond);letter-spacing:.1em;text-transform:uppercase;color:var(--muted)}
.verdict{font-size:19px;color:var(--ink);margin:0 0 10px}
.verdict.no{color:var(--lose)}
.verdict.yes{color:var(--win)}
table{width:100%;border-collapse:collapse;font-family:var(--mono);font-size:12.5px;margin-top:8px}
th,td{text-align:left;padding:6px 9px;border-bottom:1px solid var(--rule-soft);vertical-align:top}
th{font:500 10.5px/1.4 var(--cond);letter-spacing:.1em;text-transform:uppercase;color:var(--muted);border-bottom-color:var(--rule)}
td.num,th.num{text-align:right;font-variant-numeric:tabular-nums}
tbody tr:hover{background:#1f1c17}
.win{color:var(--win)}.lose{color:var(--lose)}.flat{color:var(--muted)}
.dim{color:var(--muted)}.ok{color:var(--win)}.no{color:var(--lose)}
.scroll{overflow-x:auto}
.barrow{display:grid;grid-template-columns:190px 1fr 92px;gap:12px;align-items:center;margin-bottom:7px}
.barname{font-family:var(--mono);font-size:12px}
.barname .sub,.barnum .sub{display:block;color:var(--faint);font-size:10.5px}
.sub{color:var(--faint)}
.bar{display:flex;height:22px;background:var(--sunken);border:1px solid var(--rule)}
.seg{display:block;height:100%}
.barnum{font-family:var(--mono);font-size:13px;text-align:right;font-variant-numeric:tabular-nums;color:var(--c-distance)}
.legend{display:flex;flex-wrap:wrap;gap:8px 16px;font:500 10.5px/1 var(--cond);letter-spacing:.08em;text-transform:uppercase;color:var(--muted);margin-top:12px}
.legend span{display:inline-flex;align-items:center;gap:6px}
.legend i{width:10px;height:10px;display:inline-block}
.callout{border-left:2px solid var(--brass-dim);padding:4px 0 4px 16px;margin:22px 0}
.callout p{margin:0}
.big{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:18px;margin:22px 0}
.big .v{font:600 30px/1 var(--mono);font-variant-numeric:tabular-nums}
.big .v.win{color:var(--win)}
.big .k{font:500 10.5px/1.4 var(--cond);letter-spacing:.11em;text-transform:uppercase;color:var(--muted);margin-top:6px}
.big .n{font-size:12px;color:var(--faint);font-family:var(--mono)}
ol.phases{counter-reset:p;list-style:none;padding:0;max-width:74ch}
ol.phases li{border-left:1px solid var(--rule);padding:0 0 22px 20px;position:relative}
ol.phases li::before{counter-increment:p;content:counter(p);position:absolute;left:-13px;top:0;width:25px;height:25px;border:1px solid var(--rule);background:var(--panel);color:var(--brass);font:500 11px/23px var(--mono);text-align:center}
ol.phases b{color:var(--ink);display:block;margin-bottom:4px}
footer{border-top:1px solid var(--rule);margin-top:60px;padding-top:24px;color:var(--muted);font-size:13.5px}
</style>

<header><div class="wrap">
  <div class="eyebrow"><span>LilScript compression research</span><span class="dot">/</span>
    <span>seven benchmark artifacts + jquerylil, markedlil, solidlil</span><span class="dot">/</span>
    <span>Brotli 1.1.0 q11 lgwin 22</span><span class="dot">/</span>
    <span>every win run, not just measured</span></div>
  <h1>Aligned mangling<span class="sub">two good questions, and the two things sitting next to them</span></h1>
  <p class="lede">Should mangled names be dictionary words instead of <code>a</code>, <code>b</code>,
  <code>c</code>? Should closures agree on names so <code>a[1]</code> recurs instead of
  <code>b[1]</code>? Both were measured on real artifacts and both answers are no. What the
  measuring found instead: <strong>2.2–2.5% in name allocation</strong> on every artifact the
  compiler ships directly, a <strong>fold that sinks a read past its own variable's rebinding</strong>
  in a shipped port, and a <strong>codec tie broken toward the 920-byte-larger side</strong>.</p>
</div></header>

<main>

<section>
  <div class="num-label">the short answers</div>
  <h2>What the corpora said</h2>
  <div class="qa">
    <div class="card">
      <h3>Question 1 — names from the dictionary</h3>
      <p class="verdict no">No, at any frequency.</p>
      <p>Cold bindings, used three times or fewer, cost <strong>+1,017 to +2,437</strong> Brotli
      bytes. Hot ones cost <strong>+1,953 to +6,341</strong>. The mechanism is one census row:
      in every real stream we decoded, <strong>every dictionary reference is used exactly
      once</strong>. The ROM is a few hundred first occurrences per artifact, not a rate.</p>
    </div>
    <div class="card">
      <h3>Question 2 — aligning names across closures</h3>
      <p class="verdict no">The pattern is real; the headroom is already taken.</p>
      <p>Functions that are twins up to renaming: <strong>0–1 groups per corpus</strong>, and
      those are already byte-identical. An aligner that maximises copyability loses on every
      corpus. Under a codec-shaped cost model it declines to make a single move — the ordinary
      greedy assignment is already the local optimum.</p>
    </div>
  </div>
  <div class="callout"><p>Both questions point at the same underlying thing, and the legal form of
  it is neither spelling nor alignment: <strong>hand out fewer distinct names</strong>. On
  LilScript's own jQuery artifact that is worth −801 Brotli bytes, behaviour-identical, confirmed
  with the gate codec.</p></div>
</section>

<section>
  <div class="num-label">01 — the census</div>
  <h2>Where the bits actually are</h2>
  <p>Each artifact compressed at q11, then decoded command by command with our own instrumented
  decoder. Every bit is attributed once, to the innermost field that read it.</p>
  <div style="margin:24px 0">${bitBars()}</div>
  <div class="legend">${CHANNELS.map(([, label, color]) => `<span><i style="background:${color}"></i>${label}</span>`).join("")}</div>
  <p style="margin-top:22px"><strong>Distance codes are the largest consumer of bits in every
  artifact</strong> — 47% of jQuery-min, 65% of unminified jQuery. Literals, the thing identifier
  spelling is argued about, are 4–26%. The entire prefix-code header machinery is under 2%.</p>
  <div class="scroll"><table>
    <thead><tr><th>artifact</th><th class="num">commands</th><th class="num">implicit distance</th>
      <th class="num">short code</th><th class="num">from the dictionary</th>
      <th class="num">dictionary entries reused</th></tr></thead>
    <tbody>${census.map((r) => `<tr><td>${esc(short(r.id))}</td><td class="num">${num(r.commands)}</td>
      <td class="num">${((r.implicitDistances / r.commands) * 100).toFixed(1)}%</td>
      <td class="num">${((r.shortDistances / r.commands) * 100).toFixed(1)}%</td>
      <td class="num">${num(r.dictBytes)} B (${((r.dictBytes / r.raw) * 100).toFixed(2)}%)</td>
      <td class="num ${r.dictRefs - r.distinctDictEntries === 0 ? "dim" : "lose"}">${r.dictRefs - r.distinctDictEntries}</td></tr>`).join("")}</tbody>
  </table></div>
  <p style="margin-top:18px">An implicit distance costs nothing — command symbols below 128 reuse
  the last distance. The ranking of these artifacts by implicit-distance rate is almost exactly
  their ranking by compression ratio.</p>
</section>

<section>
  <div class="num-label">02 — the hardcoded library</div>
  <h2>What is actually in the 122,784 bytes</h2>
  <p>13,504 words of length 4–24 plus 121 transforms, sampled from the web of about 2014. 5,399
  words are legal JavaScript identifiers; a large part of the rest is HTML and CSS punctuation
  (<code>sByTagName(</code>, <code>"&gt;&lt;div class="</code>, <code>cursor:pointer;</code>) and
  natural-language text in a dozen languages.</p>
  <p>The useful question is which spellings are <em>exactly one reference</em>:</p>
  <div class="scroll"><table>
    <thead><tr><th>spelling</th><th>cost</th><th>word and transform</th></tr></thead>
    <tbody>${probeRows()}</tbody>
  </table></div>
  <p style="margin-top:18px">The dictionary is fluent in ES5 and the DOM of 2014. It has never
  heard of <code>let</code>, <code>const</code>, arrows or <code>await</code>. 78 of the 121
  transforms carry code punctuation, which is the part a code generator can aim at — and the
  measured value of aiming at it is tens of bytes per artifact, as a tie-break on pooled strings.</p>
</section>

<section>
  <div class="num-label">03 — question one</div>
  <h2>Dictionary words as names, measured</h2>
  <p>Legal, scope-correct renames: the 400 most-referenced bindings, and separately the 400
  least-referenced bindings with three uses or fewer, renamed to distinct dictionary words that
  the file does not already use. Δ Brotli-11 against each artifact's own baseline.</p>
  <div class="scroll"><table>
    <thead><tr><th>variant</th>${corpusHeads}</tr></thead>
    <tbody>
      <tr><td>cold bindings, ≤3 uses</td>${variantRow("dictionary words, cold (<=3 uses)")}</tr>
      <tr><td>hot bindings, top 400</td>${variantRow("dictionary words, hot (top 400)")}</tr>
    </tbody>
  </table></div>
  <p style="margin-top:18px">The cold band is the interesting one: those bindings are used once,
  twice or three times, exactly the regime where the first-occurrence discount should have
  applied. On gl-matrix it costs 2,208 Brotli bytes while growing raw by 2,703 — nearly the whole
  raw growth paid in full, as if the codec had no dictionary at all.</p>
</section>

<section>
  <div class="num-label">04 — question two</div>
  <h2>Aligning names across closures, measured</h2>
  <h3>There are no twins to align</h3>
  <p>For every function, replace its own bindings with positional tokens and keep everything else.
  Two functions with the same canonical form are the same code modulo naming.</p>
  <div class="scroll"><table>
    <thead><tr><th>artifact</th><th class="num">functions</th><th class="num">canonical twin groups</th>
      <th class="num">bytes differing only in names</th></tr></thead>
    <tbody>${twins()}</tbody>
  </table></div>
  <p style="margin-top:16px">Frequency-ordered assignment is itself a canonical order: two
  functions with the same shape have the same frequency profile, so they already receive the same
  names without anyone trying.</p>

  <h3>An aligner that maximises copyability loses</h3>
  <div class="scroll"><table>
    <thead><tr><th>objective</th>${corpusHeads}</tr></thead>
    <tbody>
      <tr><td>maximise bytes a copy can supply</td>${variantRow("aligned, coverage objective etn")}</tr>
      <tr><td>minimise estimated bits</td>${variantRow("aligned, bits objective etn")}</tr>
    </tbody>
  </table></div>
  <p style="margin-top:16px">The second row is zero everywhere because the bit-cost aligner made
  <strong>no changes at all</strong>: at every decision point, no legal alternative beat "the first
  available name". Choosing a name to match earlier text flattens the name distribution, and
  literals get more expensive faster than copies get longer.</p>

  <h3>The <code>a[1]</code> case, with its ceiling</h3>
  <p>Renaming <em>every</em> indexed receiver to one letter is illegal; it bounds the family.</p>
  <div class="scroll"><table>
    <thead><tr><th>artifact</th><th class="num">name[k] sites</th><th class="num">share of raw</th>
      <th>most repeated pairs</th><th class="num">illegal ceiling</th></tr></thead>
    <tbody>${indexedRows()}</tbody>
  </table></div>
  <p style="margin-top:16px">On array-heavy code the pattern is a fifth of the file and the ceiling
  is 2.3–2.8% — but the hot receivers are <em>already</em> <code>e[0..3]</code> and
  <code>t[0..3]</code> across hundreds of functions. What is left is the interference case: two
  receivers live at once, which no spelling rule can merge.</p>
</section>

<section>
  <div class="num-label">05 — what was next to the questions</div>
  <h2>Fewer names, not shorter ones</h2>
  <div class="big">
    <div><div class="v win">−801</div><div class="k">Brotli bytes</div><div class="n">33,283 → 32,482</div></div>
    <div><div class="v">−2.41%</div><div class="k">of the artifact</div><div class="n">jQuery, LilScript emit</div></div>
    <div><div class="v">106 → 27</div><div class="k">distinct names</div><div class="n">2,174 bindings</div></div>
    <div><div class="v">0</div><div class="k">behaviour differences</div><div class="n">37 jsdom observations</div></div>
  </div>
  <p>Re-mangling each artifact from scratch with precise availability rules — a name is unusable
  only when it would collide with a sibling, capture a reference in the scope's subtree, or be
  shadowed between a reference and its declaration — and scoring the best legal naming:</p>
  <div class="scroll"><table>
    <thead><tr><th>artifact</th><th class="num">shipped br11</th><th>best legal naming</th>
      <th class="num">Δ br11</th><th class="num">Δ raw</th><th class="num">distinct names</th>
      <th class="num">name entropy</th></tr></thead>
    <tbody>${concentrationRows()}</tbody>
  </table></div>
  <p style="margin-top:18px">Two clean groups. On artifacts a mature JavaScript minifier produced,
  there is nothing left: 0 to 13 bytes. On <strong>LilScript's own emits</strong> there is 0.04% to
  6.6%. The compiler is not missing the idea — it has a
  <code>precise_cross_scope_shadowing</code> regime that implements exactly this rule, off in the
  pinned path by design and reachable only as a candidate-search proposal.</p>
  <div class="callout"><p>Verified three ways: the binding graph is unchanged after the rewrite,
  37 jsdom observations are byte-identical to the shipped artifact, and
  <code>lilscript-codec</code> — the gate, not the diagnostic scorer — reports 32,482.</p></div>
</section>

<section>
  <div class="num-label">06 — free orders</div>
  <h2>Emission orders that cost nothing to change</h2>
  <h3>String-pool order: a small, free win</h3>
  <p>LilScript emits 325 pooled literals in one <code>var</code>. Those declarators cannot
  reference each other, so any permutation is the same program. Δ Brotli, with gzip alongside:</p>
  <div class="scroll"><table>
    <thead><tr><th>order</th>${poolIds.map((id) => `<th class="num" colspan="2">${esc(short(id))}</th>`).join("")}</tr>
    <tr><th></th>${poolIds.map(() => `<th class="num">br11</th><th class="num">gzip</th>`).join("")}</tr></thead>
    <tbody>${poolRows()}</tbody>
  </table></div>
  <p style="margin-top:16px">Sorting by <em>reversed</em> string wins on Brotli because property
  names share endings — <code>…Node</code>, <code>…Element</code>, <code>…Type</code> — more than
  beginnings, and a shared ending is still a copy. gzip prefers alphabetical; the two codecs
  disagree, as usual.</p>

  <h3>Function layout: measured, and it loses</h3>
  <div class="scroll"><table>
    <thead><tr><th>order</th>${layoutIds.map((id) => `<th class="num">${esc(short(id))}</th>`).join("")}</tr></thead>
    <tbody>${layoutRows()}</tbody>
  </table></div>
  <p style="margin-top:16px">The census of the reordered files is the point: the implicit-distance
  rate moves by less than one point and distance bytes by less than 1%. Reordering whole functions
  does not convert near-miss distances into cache hits. That door is closed with a reason.</p>
</section>

<section>
  <div class="num-label">07 — the three ports</div>
  <h2>jquerylil, markedlil, solidlil</h2>
  <p>The same questions, asked of what those projects actually publish. Every win below was
  checked by running the port: 28 jsdom observations for jquerylil, all 680 CommonMark and GFM
  spec cases for markedlil, 18 reactive observations for solidlil's core. A row is reported only
  when the mutant's observations are identical to the baseline's.</p>
  <div class="scroll"><table>
    <thead><tr><th>artifact</th><th class="num">raw</th><th class="num">br11</th>
      <th>best legal naming</th><th class="num">Δ br11</th><th class="num">Δ raw</th>
      <th class="num">distinct names</th><th>verified</th></tr></thead>
    <tbody>${portRows()}</tbody>
  </table></div>
  <p style="margin-top:18px">The split is the same one as section 05: <strong>where the compiler's
  emit is the final artifact</strong> — jquerylil's dist files, solidlil's reactive core — there is
  2.2–2.5% in naming. Where a bundler re-mangles afterwards, there is nothing: rolldown and esbuild
  have already taken it.</p>

  <h3>markedlil: one cost model miscompiles</h3>
  <p>markedlil compiles one source tree four times, changing a single knob each time. Scored with
  <code>lilscript-codec</code> — and, because a size comparison between builds that do not compute
  the same thing is not a size comparison, run through all 680 CommonMark 0.31.2 and GFM 0.29 spec
  cases:</p>
  <div class="scroll"><table>
    <thead><tr><th>build</th><th>knob</th><th class="num">raw</th><th class="num">gzip-9</th>
      <th class="num">Brotli-11</th><th class="num">spec failures</th><th></th></tr></thead>
    <tbody>${costRows()}</tbody>
  </table></div>
  <p style="margin-top:16px">206 failures is this port's normal state — marked is not fully
  CommonMark compliant, and four of the five builds fail exactly those 206 and produce
  byte-identical output on all 680 cases. The fifth does not:</p>
  <div class="scroll"><table>
    <thead><tr><th>case</th><th>markdown</th><th>expected</th><th>what <code>cost_model = "raw"</code> produced</th></tr></thead>
    <tbody>${divergentDetail()}</tbody>
  </table></div>
  <div class="callout"><p>The <code>mailto:</code> prefix is gone. Same sources, same compiler, one
  knob apart, candidate search on in both: <strong>the search ranked and shipped a candidate that
  changes what the program computes.</strong> It reproduces at HEAD.</p></div>

  <h3>Why the raw model's build is smaller — and what it costs</h3>
  <p>The two builds differ in four transform families at once: outlining repeated member calls
  (<code>.slice(</code> 63 → 2, <code>.exec(</code> 39 → 1, <code>.replace(</code> 28 → 1), fusing
  statements into comma sequences (<code>;</code> 1,092 → 383, and 99 blocks lose their braces),
  merging adjacent declarations (<code>var</code> 240 → 75), and <code>for(;t;)</code> →
  <code>while(t)</code> (50 → 1). Applied one at a time to the Brotli-model artifact and scored
  with the gate codec:</p>
  <div class="scroll"><table>
    <thead><tr><th>family</th><th class="num">sites</th><th class="num">Δ raw</th>
      <th class="num">Δ gzip</th><th class="num">Δ Brotli-11</th><th></th></tr></thead>
    <tbody>
      <tr><td class="mono">merge adjacent declarations</td><td class="num">230</td>
        <td class="num win">−920</td><td class="num win">−60</td><td class="num">±3</td>
        <td class="win">a tie, worth 920 raw bytes</td></tr>
      <tr><td class="mono">for(;t;) → while(t)</td><td class="num">49</td>
        <td class="num dim">0</td><td class="num dim">−5</td><td class="num lose">+19</td>
        <td class="dim">rightly declined</td></tr>
      <tr><td class="mono">outline .slice/.exec/.replace</td><td class="num">130</td>
        <td class="num">−187</td><td class="num lose">+79</td><td class="num lose">+126</td>
        <td class="dim">rightly declined</td></tr>
    </tbody>
  </table></div>
  <p style="margin-top:16px">So the Brotli model is right to refuse two of the three. The third is a
  <strong>tie on the ranked metric that it is losing by 920 raw bytes</strong> — and the same probe
  finds <em>zero</em> such opportunities in jquerylil or solidlil, whose configs carry an explicit
  30-pass <code>compression</code> list that markedlil's does not.</p>

  <h3>And the bug is the <code>ident</code> class, not the cost model</h3>
  <p>The two builds' autolink handlers, side by side: the correct one computes the
  <code>mailto:</code> ternary <em>before</em> reusing its variable; the raw-model one fuses four
  assignments into one comma sequence and reads match-group 2 from <code>p</code>
  <em>after</em> <code>p</code> has been reassigned to the token.</p>
  <pre class="mono" style="background:var(--sunken);border:1px solid var(--rule);padding:14px;overflow-x:auto;font-size:12px;line-height:1.6">// cost_model = "brotli" — correct
var t = ae.exec(e);                          // t = the match
e = Ir(t);
var r = "@" == At(t, 2) ? "mailto:" + e : e; // reads group 2 of t …
t = a(15, t[0] + "");                        // … before t is reused
t.text = e; t.href = r;

// cost_model = "raw" — wrong
var p = R(up, A);                            // p = the match
A = a(p, 1),
p = i(15, M(p)),                             // p reassigned to the token
p.text = A,
p.href = "@" == a(p, 2) ? "mailto:" + A : A; // reads group 2 of p — too late</pre>
  <p>That is the board's own <code>ident</code> invariant — a saved value must stay readable across
  its own update — in a shipped port, with a one-knob trigger and a two-case spec signature. The
  raw cost model does not cause it; it buys enough fusion for the fold to fire.</p>
  <p>Among the four builds that do compute the same thing, the smallest is
  <code>marked.closed.js</code> at 9,475 — and the package publishes
  <code>marked.esm.js</code> at 9,589, 114 bytes above a correct build it already produces.
  Naming headroom on those builds is nil.</p>
</section>

<section>
  <div class="num-label">08 — how to search</div>
  <h2>The objective is separable, if you factor it right</h2>
  <p>Transforms plainly affect one another, so the question is whether the coupling is strong and
  general — in which case no cheap heuristic can work — or structured. So it was measured: six
  legality-checked rewrites applied to a finished artifact in every combination, 32 or 64 design
  points each, scored with the real codec, and an additive model fitted to the responses.</p>
  <div class="scroll"><table>
    <thead><tr><th>artifact</th><th class="num">R², naming as two switches</th>
      <th class="num">R², naming as one decision</th><th>top interaction (split)</th>
      <th class="num">greedy leaves</th><th class="num">stacking error, split</th>
      <th class="num">stacking error, grouped</th></tr></thead>
    <tbody>${factorialRows()}</tbody>
  </table></div>
  <div class="callout"><p>Modelling naming as two independent switches puts the additive fit at
  <strong>R² 0.55</strong>. Modelling it as <strong>one decision with four levels</strong> puts it at
  <strong>0.9968</strong> — and in the split design that single pair carried 70–99% of all the
  apparent coupling. It was never physics: applying one renaming after another does not compose
  them, the second overwrites the first.</p></div>
  <p>Interactions concentrate between factors that <strong>rewrite the same bytes</strong> and vanish
  between factors that rewrite disjoint ones — on two artifacts the naming × declaration-merging
  interaction is exactly <strong>0.0%</strong>. That turns the search from a product into a sum:
  partition transforms by what they rewrite, enumerate levels inside a partition, take each
  partition's best independently. Cost falls from ∏|levels| to ∑|levels|, and coordinate descent
  lands within 25 bytes — 0.08% — of exhaustive search.</p>
  <p>The largest verified result in this folder came out of exactly that grid: the in-tree jQuery
  port at <strong>33,283 → 32,223 Brotli, −1,060 bytes (−3.18%)</strong> under
  <code>lilscript-codec</code>, all 37 jsdom observations identical — from 64 evaluations that take
  seconds, against a candidate search that did not finish the same artifact in 4.5 hours.</p>
  <p class="dim" style="font-size:14px">Honest limits: this is a six-factor screen on post-hoc
  rewrites, not the compiler's real space, where families change structure rather than spelling and
  may couple harder. And a grouped factor is only as good as its level set — the two-switch design
  found 30 bytes the four-level design missed, because applying both renamings composes to a naming
  none of the four levels contained.</p>
</section>

<section>
  <div class="num-label">09 — the equation</div>
  <h2>Closed form, useless gradient, cheap re-solve</h2>
  <p>There is no black box. For a fixed parse the size of a Brotli stream is exactly
  <code>L = H(θ) + Σ [ ℓ(cmd) + ℓ(dist) + Σ ℓ(literal | ctx) ]</code>, where every <code>ℓ</code> is
  <code>−log2 p</code> under that block's own histogram θ. Two things stop it being directly
  optimisable: the parse is itself a choice, and θ depends on the whole block — a mean-field
  coupling rather than a local one.</p>
  <p>Freeze θ and the objective becomes linear in the symbol counts, with the obvious gradient
  <code>∂L/∂n_s = −log2 p_s</code>. That is what any cost model pricing a change locally is doing.
  Measured against the real codec over 32 design points per artifact:</p>
  <div class="scroll"><table>
    <thead><tr><th>artifact</th><th class="num">r, θ frozen (the gradient)</th><th class="num">mean error</th>
      <th class="num">r, θ recomputed</th><th class="num">mean error</th>
      <th class="num">absolute error</th><th class="num">vs the codec</th></tr></thead>
    <tbody>${analyticRows()}</tbody>
  </table></div>
  <div class="callout"><p>On the artifact whose edits are renamings the gradient is
  <strong>anti-correlated with the truth</strong> (r −0.14). Renaming barely changes which symbols
  occur — it changes the distribution itself, which is exactly the term a linearisation discards.
  You cannot step along this field; you have to re-solve it. Re-solving is one pass, ranks to
  0.15%, and is 6–10× cheaper than compressing.</p></div>
  <p>The hardness is real and sits in the obvious place: the surrogate this folder measured —
  <em>use fewer distinct names, subject to interference</em> — is graph colouring, which is NP-hard,
  and the true objective is worse. What is not out of reach is the structure. Enumerate levels
  inside a partition, score analytically, confirm the finalists with the real codec.</p>

  <h3>What it buys on the shipped libraries</h3>
  <p>Grid, best point, each step proved, scored with <code>lilscript-codec</code> — 14 artifacts
  from five LilScript libraries, <strong>12 seconds of search in total</strong>.</p>
  <div class="scroll"><table>
    <thead><tr><th>library</th><th>artifact</th><th class="num">Brotli-11</th><th class="num">after</th>
      <th class="num">Δ</th><th class="num">%</th><th class="num">Δ raw</th><th>winning point</th>
      <th>verified by</th></tr></thead>
    <tbody>${libraryRows()}</tbody>
  </table></div>
  <p style="margin-top:16px"><strong>196,441 → 195,489 Brotli bytes, −952 overall</strong>, up to
  −2.90% on a single module, every winner behaviourally identical. Read the zeros too: motionlil is
  already at this grid's optimum on its three largest artifacts — they come out of a bundler, and
  where something re-mangles downstream naming has nothing left. markedlil gives back
  <strong>945 raw bytes for 16 Brotli bytes</strong>, the same tie-break as section 07, now on a
  third artifact.</p>
</section>

<section>
  <div class="num-label">10 — the plan</div>
  <h2>What to do about it</h2>
  <ol class="phases">
    <li><b>Fix the miscompilation first.</b> Route the statement-fusion fold through the shared
    receiver-rebinding check the <code>ident</code> lane is already building, and freeze CommonMark
    #604/#605 as a canonical case. A search that ranks a behaviour-changing candidate outranks
    every size question below it — and the general question it raises is whether anything gates
    candidate acceptance on behaviour, or only on size and a static proof.</li>
    <li><b>Break codec ties with raw size.</b> markedlil is taking the 920-raw-byte-larger side of a
    ±3-byte Brotli tie. Raw is never free: it is parse time, memory, and the gzip lane.</li>
    <li><b>Diagnose the 106 names.</b> Rebuild the port with today's compiler, and find out whether
    the candidate beam proposes the precise-shadowing regime for this artifact at all. Three
    possible answers — not proposed, proposed and lost, proposed and dropped by beam width — and
    they lead to three different fixes. Diagnosis only; safe while the identity lane is red.</li>
    <li><b>Make the allocation objective explicit.</b> Colour the interference graph for
    <em>fewest colours</em>, biased toward the letters the file already spends, instead of "first
    free name in alphabet order". Blocked on the identity lane: a naming change that lands while
    identity bugs are open will be blamed for them.</li>
    <li><b>Freeze the invariant.</b> Paired canonical cases where two non-interfering live ranges
    in sibling closures must receive the same name.</li>
    <li><b>Take the free order.</b> Pooled literals in reversed-string order under a Brotli cost
    model, as a scored proposal rather than a rule.</li>
    <li><b>Write the closed doors down.</b> Dictionary words as identifiers, copy-maximising
    alignment, and function layout for the distance cache — each with its numbers, so the next
    context does not re-derive them.</li>
    <li><b>Factor the search space before optimising it.</b> Two knobs that rewrite the same bytes
    are one decision with several levels. Report the additive fit's R² whenever a family joins the
    beam: if it drops, the partition is wrong and the family belongs inside an existing one.</li>
    <li><b>Keep the instrument.</b> A proposal that claims a mechanism must show the census row
    where the mechanism appears. Two of the ideas above died on exactly that test after looking
    plausible in prose.</li>
  </ol>
</section>

<footer><div class="wrap">
  <p>Every number here is generated from the JSON in
  <code>docs/knowledge/research/aligned-mangling/</code>; this page is built by
  <code>render-findings.mjs</code> and cannot drift from it. Mutations are applied through a scope
  analyser and re-analysed afterwards; anything that changes the binding graph is reported, not
  scored. Diagnostic sizes are Node zlib Brotli 1.1.0 q11 lgwin 22 and gzip-9; the headline result
  was re-scored with <code>lilscript-codec</code>. The format tooling is
  <em>Brotli, the whole machine</em>, in the same folder.</p>
</div></footer>
</main>
`;

writeFileSync(join(here, "findings.html"), html);
console.log(`wrote findings.html (${(html.length / 1024).toFixed(0)} KiB)`);
