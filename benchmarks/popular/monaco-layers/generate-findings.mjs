import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { monacoEditorCoreVersion, monacoEditorVersion, planned, vscodeCommitId } from "./catalog.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const labRoot = join(here, "..");
const summaryPath = join(labRoot, "build/monaco-layers/summary.json");
const demoSizesPath = join(labRoot, "build/monaco-layers/demo-sizes.json");
const outPath = join(labRoot, "apps/monaco/findings.html");

function fmt(n) {
  return Number(n).toLocaleString("en-US");
}

function deltaClass(delta) {
  return delta <= 0 ? "win" : "loss";
}

function sign(n) {
  return n > 0 ? `+${fmt(n)}` : fmt(n);
}

const reports = JSON.parse(readFileSync(summaryPath, "utf8"));
let demo = null;
try {
  demo = JSON.parse(readFileSync(demoSizesPath, "utf8"));
} catch {
  demo = null;
}

const codecs = reports[0]?.codecs ?? {};
const honest = new Set(["base-lifecycle", "core-types", "piece-tree", "text-model"]);
const viewish = new Set(["view-render", "input-commands"]);

const rows = reports
  .map((report) => {
    const lil = report.lilscript.sizes;
    const js = report.javascript.selectedBaseline;
    const extracted = report.javascript.extracted;
    const fairness = honest.has(report.layer)
      ? "matching extract"
      : viewish.has(report.layer)
        ? "view graph extract"
        : "editor.api kitchen-sink";
    return `<tr>
      <td><code>${report.layer}</code></td>
      <td>${fairness}</td>
      <td class="num">${fmt(lil.raw)}</td>
      <td class="num">${fmt(lil.gzip)}</td>
      <td class="num">${fmt(lil.brotli)}</td>
      <td class="num">${fmt(extracted.raw)}</td>
      <td class="num">${fmt(js.sizes.raw)}</td>
      <td class="num">${fmt(js.sizes.gzip)}</td>
      <td class="num">${fmt(js.sizes.brotli)}</td>
      <td>${js.lane}</td>
      <td class="num ${deltaClass(report.gate.delta)}">${sign(report.gate.delta)}</td>
      <td>${report.gate.pass ? "PASS" : "FAIL"}</td>
    </tr>`;
  })
  .join("\n");

const demoBlock = demo
  ? `<h2>Integrated demo artifacts</h2>
<p>Closed-app compile of the paired demo (<code>demo-entry.lil</code> + host bundle) versus the monaco-editor-core <code>editor.api</code> bundle used by the JS page. CSS and TTF are not in these JS totals.</p>
<table>
  <thead>
    <tr>
      <th>Artifact</th>
      <th>Raw</th>
      <th>Gzip-9</th>
      <th>Brotli-11</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <td>LilScript demo (app mangle, host bundled)</td>
      <td class="num">${fmt(demo.lilscript.raw)}</td>
      <td class="num">${fmt(demo.lilscript.gzip)}</td>
      <td class="num">${fmt(demo.lilscript.brotli)}</td>
    </tr>
    <tr>
      <td>monaco-editor-core editor.api (esbuild minify)</td>
      <td class="num">${fmt(demo.javascript.raw)}</td>
      <td class="num">${fmt(demo.javascript.gzip)}</td>
      <td class="num">${fmt(demo.javascript.brotli)}</td>
    </tr>
  </tbody>
</table>`
  : `<p class="note">Integrated demo codec row appears after <code>node monaco-layers/build-apps.mjs</code>.</p>`;

const html = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Monaco Editor LilScript port — findings</title>
  <style>
    :root {
      --ink: #1b1712;
      --paper: #f4efe4;
      --rule: #d7cbb6;
      --muted: #6d6254;
      --win: #1f6b3a;
      --loss: #9b2c2c;
      --accent: #b5471b;
    }
    * { box-sizing: border-box; }
    html { background: var(--paper); color: var(--ink); }
    body {
      margin: 0 auto;
      max-width: 1100px;
      padding: 2.5rem 1.25rem 4rem;
      font: 17px/1.5 "Iowan Old Style", "Palatino Linotype", Palatino, serif;
    }
    h1, h2, h3 { font-family: "Trebuchet MS", "Gill Sans", sans-serif; letter-spacing: -0.02em; }
    h1 { font-size: 2.1rem; margin: 0 0 0.4rem; }
    .kicker { color: var(--accent); font: 700 0.78rem/1 "Trebuchet MS", sans-serif; text-transform: uppercase; letter-spacing: 0.14em; }
    p { max-width: 70ch; }
    code, .num { font-family: "IBM Plex Mono", "ui-monospace", Menlo, monospace; font-size: 0.86em; }
    table { width: 100%; border-collapse: collapse; font-size: 0.82rem; margin: 1.2rem 0 2rem; }
    th, td { border-bottom: 1px solid var(--rule); padding: 0.45rem 0.4rem; text-align: left; vertical-align: top; }
    th { font-family: "Trebuchet MS", sans-serif; font-weight: 700; color: var(--muted); }
    .num { text-align: right; white-space: nowrap; }
    .win { color: var(--win); }
    .loss { color: var(--loss); }
    ul { max-width: 75ch; }
    .note { color: var(--muted); }
    footer { margin-top: 3rem; color: var(--muted); font-size: 0.9rem; }
  </style>
</head>
<body>
  <p class="kicker">Popular-lab findings</p>
  <h1>Monaco Editor 0.56 in LilScript</h1>
  <p>
    This is a popular-lab experiment, not a product rewrite of VS Code.
    Public <code>monaco.d.ts</code> names and the real algorithms stay;
    internals are structs, classes, and explicit services so the compiler can mangle and dissolve them.
    Numbers are from <code>lilscript-codec</code> (stock zlib ${codecs.gzip9?.libraryVersion ?? "1.3.1"} /
    official Brotli C ${codecs.brotli11?.libraryVersion ?? "1.1.0"}), never Node <code>zlib</code>.
  </p>

  <h2>What was ported</h2>
  <ul>
    <li>Pins: monaco-editor ${monacoEditorVersion}, monaco-editor-core ${monacoEditorCoreVersion}, vscode <code>${vscodeCommitId.slice(0, 7)}</code>.</li>
    <li>Algorithms kept: piece tree + red-black metadata, prefix-sum line starts, decoration interval tree, Monarch lexer, Myers diff.</li>
    <li>Public surface: <code>create</code>, <code>createModel</code>, <code>applyEdits</code>, undo/redo, <code>deltaDecorations</code>, <code>findMatches</code>, <code>defineTheme</code>/<code>setTheme</code>, layout, dispose, diff editor.</li>
    <li>Popular languages: JS/TS/JSON/HTML/CSS/Python/Markdown, then 75 remaining Monarch ids.</li>
    <li>Popular contrib: find, indent folding, brackets, hover, suggest, snippets, comments, goto, sticky, word highlight, links, Myers diff.</li>
    <li>JSON/CSS/HTML adapters without rewriting <code>tsc</code>.</li>
  </ul>
  <p>Layer ladder: <code>${planned.join(" → ")}</code></p>

  <h2>Per-layer JS size</h2>
  <p>
    Gate is Brotli-11 of the LilScript compiler output versus the best eligible JS minifier
    (esbuild / Terser / Oxc) of that layer’s monaco-editor-core extract.
    Raw and gzip are diagnostics. CSS/TTF are excluded.
  </p>
  <table>
    <thead>
      <tr>
        <th>Layer</th>
        <th>JS extract fairness</th>
        <th>Lil raw</th>
        <th>Lil gzip</th>
        <th>Lil Brotli</th>
        <th>JS extract raw</th>
        <th>JS min raw</th>
        <th>JS gzip</th>
        <th>JS Brotli</th>
        <th>JS lane</th>
        <th>Brotli Δ</th>
        <th>Gate</th>
      </tr>
    </thead>
    <tbody>
      ${rows}
    </tbody>
  </table>

  ${demoBlock}

  <h2>Where LilScript won</h2>
  <ul>
    <li><strong>core-types / piece-tree / text-model</strong> are the honest rows. Fixed <code>Pos</code>/<code>Rng</code>/<code>Sel</code> layout and a closed-world piece tree beat TS-erased objects plus the VS Code buffer graph.</li>
    <li>No instantiation service or decorator DI: services are typed fields, so property mangling and scalar replacement can fire.</li>
    <li><code>JsValue</code> stays at the <code>create(options)</code> bag and DOM host. Methods are exported as functions because <code>mangle.properties = true</code>.</li>
  </ul>

  <h2>Where the comparison overstates, or LilScript lost ground</h2>
  <ul>
    <li><strong>view-render / input-commands</strong> extract monaco’s <code>editor/browser/view.js</code> graph (GPU view zones, whitespace, cursors). The port is a viewport, textarea, and minimap canvas. The Brotli gap is real for this subset, not proof that every view part was ported.</li>
    <li><strong>standalone-api and later</strong> extracts often pull <code>editor.api.js</code> because language <code>conf</code> imports <code>IndentAction</code>. That is the npm kitchen-sink for <code>monaco.editor.create</code>, while LilScript is the functioning subset. Size deltas there overstate a full-product replacement.</li>
    <li>Host facade and known-host lowering still cost bytes. <code>createElement</code> had to be spelled <code>domCreateElement</code> after known-host lowering emitted an unbound call.</li>
    <li>CSS and editor fonts are side artifacts. Workers are not ported; tokenize on the main thread.</li>
    <li>Remaining Monarch languages are keyword lexers generated from monaco-editor definitions, not a line-by-line Monarch dump of every upstream tokenizer.</li>
  </ul>

  <h2>Non-goals</h2>
  <ul>
    <li>Microsoft TypeScript compiler / <code>ts.worker</code></li>
    <li>VS Code workbench</li>
    <li>TextMate / Oniguruma grammars</li>
  </ul>

  <p>
    Paired demos: <a href="./lil/index.html">LilScript editor</a> ·
    <a href="./js/index.html">monaco-editor-core editor</a>.
    Port notes: <a href="../../ports/monaco/STATUS.md">STATUS.md</a>.
  </p>
  <footer>
    Encoder: ${codecs.implementation ?? "lilscript-codec"};
    gzip ${codecs.gzip9?.encoder ?? "upstream-stock-zlib-c"} ${codecs.gzip9?.libraryVersion ?? "1.3.1"};
    Brotli ${codecs.brotli11?.encoder ?? "official-google-brotli-c"} ${codecs.brotli11?.libraryVersion ?? "1.1.0"} q11 lgwin22.
    Node compressors are diagnostic only.
  </footer>
</body>
</html>
`;

writeFileSync(outPath, html);
console.log(`wrote ${outPath}`);
