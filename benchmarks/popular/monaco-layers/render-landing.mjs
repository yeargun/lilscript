function fmt(n) {
  return Number(n).toLocaleString("en-US");
}

function ratio(js, lil) {
  if (!lil) return "—";
  return `${(js / lil).toFixed(2)}×`;
}

function sign(n) {
  return n > 0 ? `+${fmt(n)}` : fmt(n);
}

function shortName(path) {
  const bits = path.split("/");
  return bits.slice(-2).join("/");
}

function folderRows(folders) {
  return (folders ?? [])
    .map((row) => {
      const lilBr = row.lil?.brotli;
      const jsBr = row.js.brotli;
      const scored = row.scoredLil ?? 0;
      return `<tr>
  <td><code>${row.key}</code></td>
  <td class="num">${fmt(row.files)}</td>
  <td class="num">${fmt(jsBr)}</td>
  <td class="num">${scored ? fmt(lilBr) : "—"}</td>
  <td class="num">${scored ? ratio(jsBr, lilBr) : "—"}</td>
  <td class="note">${scored ? `${fmt(scored)} Lil module${scored === 1 ? "" : "s"} scored` : "Lil not scored yet"}</td>
</tr>`;
    })
    .join("\n");
}

function catalogFileRows(files) {
  return (files ?? [])
    .map((row) => {
      const jsBr = row.js?.brotli;
      const lilBr = row.lil?.brotli;
      const win = jsBr != null && lilBr != null ? (lilBr <= jsBr ? "win" : "loss") : "";
      return `<tr>
  <td><code>${row.monaco}</code></td>
  <td>${row.status}</td>
  <td class="num">${jsBr == null ? "—" : fmt(jsBr)}</td>
  <td class="num ${win}">${lilBr == null ? "—" : fmt(lilBr)}</td>
  <td class="num">${jsBr != null && lilBr != null ? ratio(jsBr, lilBr) : "—"}</td>
</tr>`;
    })
    .join("\n");
}

function pairRows(pairs) {
  return pairs
    .map((row) => {
      const plugged = row.plugged ? "yes" : "no";
      const monaco = row.monacoFiles.map((f) => `<code>${shortName(f)}</code>`).join("<br>");
      const lil = row.lilFiles.map((f) => `<code>${shortName(f)}</code>`).join("<br>");
      if (!row.js) {
        return `<tr>
  <td><code>${row.id}</code></td>
  <td>${monaco}</td>
  <td>${lil}</td>
  <td>${plugged}</td>
  <td colspan="4" class="note">${row.note}</td>
</tr>`;
      }
      const win = row.delta.brotli <= 0 ? "win" : "loss";
      return `<tr>
  <td><code>${row.id}</code></td>
  <td>${monaco}</td>
  <td>${lil}</td>
  <td>${plugged}</td>
  <td class="num">${fmt(row.js.sizes.brotli)}</td>
  <td class="num">${fmt(row.lil.sizes.brotli)}</td>
  <td class="num ${win}">${sign(row.delta.brotli)}</td>
  <td class="num">${ratio(row.js.sizes.brotli, row.lil.sizes.brotli)}</td>
</tr>`;
    })
    .join("\n");
}

export function renderLanding(doc) {
  const p = doc.production;
  const jsWorkers = p.workers?.files ?? [];
  const lilWorkers = p.lilWorkers?.files ?? [];
  const tsJs = jsWorkers.find((row) => row.name === "ts.worker.js");
  const tsLil = lilWorkers.find((row) => row.name === "ts.worker.js");
  const jsWorkersNoTs = {
    raw: p.js.workers.raw - (tsJs?.sizes.raw ?? 0),
    brotli: p.js.workers.brotli - (tsJs?.sizes.brotli ?? 0),
  };
  const lilWorkersNoTs = {
    raw: p.lil.workers.raw - (tsLil?.sizes.raw ?? 0),
    brotli: p.lil.workers.brotli - (tsLil?.sizes.brotli ?? 0),
  };
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>monaco-editor 0.56 — JS vs LilScript</title>
  <style>
    * { box-sizing: border-box; }
    html, body {
      margin: 0;
      background: #1e1e1e;
      color: #cccccc;
      font: 14px/1.5 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }
    body { overflow: auto; padding: 32px 20px 64px; }
    main { max-width: 1080px; margin: 0 auto; }
    table.catalog { font-size: 12px; }
    table.catalog td { padding: 4px 6px; }
    h1 { font-size: 22px; font-weight: 600; color: #fff; margin: 0 0 8px; }
    h2 { font-size: 16px; color: #fff; margin: 28px 0 10px; }
    p, li { max-width: 78ch; }
    a { color: #9cdcfe; }
    code { font-family: ui-monospace, Menlo, monospace; font-size: 0.88em; }
    .kicker { color: #ce9178; font-size: 12px; letter-spacing: 0.08em; text-transform: uppercase; }
    .cards { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; margin: 18px 0 8px; }
    .cards a {
      display: block;
      background: #252526;
      border: 1px solid #3c3c3c;
      padding: 16px 18px;
      text-decoration: none;
      color: #9cdcfe;
    }
    .cards a:hover { border-color: #007acc; }
    .cards strong { display: block; color: #fff; font-size: 15px; margin-bottom: 4px; }
    .cards span { color: #bbbbbb; font-size: 13px; }
    table { width: 100%; border-collapse: collapse; font-size: 13px; margin: 8px 0 16px; }
    th, td { border-bottom: 1px solid #333; padding: 7px 8px; text-align: left; vertical-align: top; }
    th { color: #888; font-weight: 600; }
    .num { text-align: right; font-family: ui-monospace, Menlo, monospace; white-space: nowrap; }
    .win { color: #89d185; }
    .loss { color: #f48771; }
    .note { color: #888; font-size: 12px; }
    .warn { border: 1px solid #9d6b2f; background: #2a2116; padding: 10px 12px; }
  </style>
</head>
<body>
<main>
  <p class="kicker">Production build · Brotli vs Brotli</p>
  <h1>monaco-editor 0.56: JavaScript vs LilScript</h1>
  <p>
    Two production editors. The JS page is npm <code>monaco-editor@${doc.versions.monacoEditor}</code>.
    The LilScript page is compiled from <code>ports/monaco/entry.lil</code> — the editor is LilScript, not monaco-editor with a few files swapped.
  </p>
  <div class="cards">
    <a href="./lil/">
      <strong>Open LilScript monaco</strong>
      <span>compiled Lil editor + catalog · ide.js Brotli ${fmt(p.lil.ide.brotli)}</span>
    </a>
    <a href="./js/">
      <strong>Open JS monaco-editor</strong>
      <span>npm monaco-editor · ide.js Brotli ${fmt(p.js.ide.brotli)}</span>
    </a>
  </div>

  <h2>How both production builds are made</h2>
  <p>
    JS: esbuild minify of <code>monaco-editor</code> ESM plus its workers, then
    <code>lilscript-codec</code> Brotli-11 / gzip-9 (stock zlib ${doc.codec.gzip} /
    official Brotli C ${doc.codec.brotli}).
    Lil: LilScript compiler of the runtime editor plus the monaco-editor-core catalog (992 modules compiled as reusable libraries so class methods stay in the artifact), one js-host, same workbench chrome, esbuild minify, same codec.
    The HTTP server sends Brotli-11. Lil language hosts remain compiled LilScript for JSON/CSS/HTML/editor workers. Both pages load the same Microsoft <code>ts.worker</code> (<code>typescriptServices.js</code>) so TypeScript and Solid TSX use the real language service.
  </p>
  <table>
    <thead>
      <tr>
        <th>Artifact</th>
        <th>JS page raw</th>
        <th>JS page Brotli</th>
        <th>Lil page raw</th>
        <th>Lil page Brotli</th>
      </tr>
    </thead>
    <tbody>
      <tr>
        <td><code>ide.js</code> (JS monaco-editor / Lil runtime + catalog)</td>
        <td class="num">${fmt(p.js.ide.raw)}</td>
        <td class="num">${fmt(p.js.ide.brotli)}</td>
        <td class="num">${fmt(p.lil.ide.raw)}</td>
        <td class="num">${fmt(p.lil.ide.brotli)}</td>
      </tr>
      <tr>
        <td>Workers except <code>ts.worker</code></td>
        <td class="num">${fmt(jsWorkersNoTs.raw)}</td>
        <td class="num">${fmt(jsWorkersNoTs.brotli)}</td>
        <td class="num">${fmt(lilWorkersNoTs.raw)}</td>
        <td class="num">${fmt(lilWorkersNoTs.brotli)}</td>
      </tr>
      <tr>
        <td><code>ts.worker</code> (same Microsoft <code>typescriptServices.js</code> on both pages)</td>
        <td class="num">${fmt(tsJs?.sizes.raw ?? 0)}</td>
        <td class="num">${fmt(tsJs?.sizes.brotli ?? 0)}</td>
        <td class="num">${fmt(tsLil?.sizes.raw ?? 0)}</td>
        <td class="num">${fmt(tsLil?.sizes.brotli ?? 0)}</td>
      </tr>
      <tr>
        <td>JS + workers / Lil editor + workers, excluding tsc</td>
        <td class="num">${fmt(p.js.ide.raw + jsWorkersNoTs.raw)}</td>
        <td class="num">${fmt(p.js.ide.brotli + jsWorkersNoTs.brotli)}</td>
        <td class="num">${fmt(p.lil.ide.raw + lilWorkersNoTs.raw)}</td>
        <td class="num">${fmt(p.lil.ide.brotli + lilWorkersNoTs.brotli)}</td>
      </tr>
      <tr>
        <td>Editor CSS</td>
        <td class="num">${fmt(p.js.css.raw)}</td>
        <td class="num">${fmt(p.js.css.brotli)}</td>
        <td class="num">${fmt(p.lil.css.raw)}</td>
        <td class="num">${fmt(p.lil.css.brotli)}</td>
      </tr>
    </tbody>
  </table>
  <p class="note">
    monaco-editor-core ships ${fmt(doc.coreJsFiles)} <code>.js</code> files. Catalog:
    <strong>${fmt(doc.catalog?.ported ?? 0)} implemented</strong>,
    ${fmt(doc.catalog?.shim ?? 0)} shims to those implementations,
    ${fmt(doc.catalog?.thin ?? 0)} thin view bindings,
    ${fmt(doc.catalog?.stub ?? 0)} stubs,
    ${fmt(doc.catalog?.extern ?? 0)} externs
    (${fmt(doc.catalog?.mapped ?? doc.coreJsFiles)} mapped paths).
    Implemented files are independently compiled with js-module keepers (exported class methods retained) and scored Brotli-for-Brotli against the matching monaco JS module.
    The Lil <code>ide.js</code> bundles those catalog modules with the running editor.
  </p>

  <h2>monaco files already in LilScript</h2>
  <p>
    Each scored row is that monaco-editor-core file versus the LilScript file, both minified-equivalent, both Brotli-11.
    The Lil <em>page</em> is the whole compiled editor, not a plug of one file into npm monaco.
  </p>
  <p class="warn">
    The old piece-tree “5×” compared Lil’s tree to a <code>PieceTreeTextBufferBuilder</code> extract that also pulled
    <code>model.js</code> and <code>textModelSearch.js</code>. Fair piece-tree is <code>pieceTreeBase.js</code> + <code>rbTreeBase.js</code> only.
  </p>
  <table>
    <thead>
      <tr>
        <th>Pair</th>
        <th>monaco JS</th>
        <th>LilScript</th>
        <th>In IDE</th>
        <th>JS Brotli</th>
        <th>Lil Brotli</th>
        <th>Δ</th>
        <th>JS / Lil</th>
      </tr>
    </thead>
    <tbody>
      ${pairRows(doc.pairs)}
    </tbody>
  </table>
  <p class="note">
    Unscored rows still have Lil sources; they are not a complete API match of the monaco file yet.
    Microsoft <code>tsc</code> is not rewritten in LilScript. The served Lil page uses the official <code>ts.worker.js</code> so diagnostics, complete, hover, and Solid TSX match JS monaco.
  </p>

  <h2>Full monaco-editor-core comparison (${fmt(doc.coreJsFiles)} files)</h2>
  <p>
    Every core JS file is esbuild-minified with other monaco imports left external, then scored with the same Brotli-11 codec as production.
    Lil columns are filled for independently compiled implementations with keepers so class methods are not DCE'd; the two vendored externs (<code>marked</code>, <code>dompurify</code>) stay external.
    Folder Brotli is the sum of those JS files; Lil folder totals count each implementation once.
  </p>
  <table>
    <thead>
      <tr>
        <th>Folder</th>
        <th>Files</th>
        <th>JS Brotli</th>
        <th>Lil Brotli</th>
        <th>JS / Lil</th>
        <th></th>
      </tr>
    </thead>
    <tbody>
      ${folderRows(doc.coreComparison?.folders)}
      <tr>
        <td><strong>all ${fmt(doc.coreJsFiles)}</strong></td>
        <td class="num">${fmt(doc.coreComparison?.totals?.files ?? doc.coreJsFiles)}</td>
        <td class="num">${fmt(doc.coreComparison?.totals?.js?.brotli ?? 0)}</td>
        <td class="num">${doc.coreComparison?.totals?.lil ? fmt(doc.coreComparison.totals.lil.brotli) : "—"}</td>
        <td class="num">${doc.coreComparison?.totals?.lil ? ratio(doc.coreComparison.totals.js.brotli, doc.coreComparison.totals.lil.brotli) : "—"}</td>
        <td class="note">${fmt(doc.coreComparison?.scoredLil ?? 0)} unique Lil modules scored</td>
      </tr>
    </tbody>
  </table>
  <table class="catalog">
    <thead>
      <tr>
        <th>monaco-editor-core file</th>
        <th>Status</th>
        <th>JS Brotli</th>
        <th>Lil Brotli</th>
        <th>JS / Lil</th>
      </tr>
    </thead>
    <tbody>
      ${catalogFileRows(doc.coreComparison?.files)}
    </tbody>
  </table>

  <h2>Toward a vscode / monaco fork</h2>
  <p>
    The Lil page is a LilScript editor, including the public <code>monaco</code> object and the monaco-editor-core catalog pack. monaco-editor-core is catalogued file-for-file (992 Lil ports, 2 externs). The JS page stays stock monaco so you can compare behavior and Brotli.
  </p>
  <p class="note">
    Encoder: ${doc.codec.implementation}; gzip ${doc.codec.gzip}; Brotli ${doc.codec.brotli} q11.
    Node zlib is not used for these tables. Raw JSON: <a href="./sizes.json">sizes.json</a>.
  </p>
</main>
</body>
</html>
`;
}
