import { createServer } from "node:http";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { extname, join, normalize, resolve } from "node:path";
import {
  canonicalCodecMeasurementsForFiles,
  canonicalCodecProvenance,
} from "../../benchmarks/codec-contract.mjs";
import {
  browserBuildRoot as buildRoot,
  buildBrowserFixtures,
  FIXTURES,
} from "./browser-fixtures.mjs";

const port = Number(process.env.PORT ?? 5177);
const rebuild = process.env.REBUILD !== "0";

function contentType(path) {
  switch (extname(path)) {
    case ".js":
      return "text/javascript;charset=utf-8";
    case ".css":
      return "text/css;charset=utf-8";
    case ".svg":
      return "image/svg+xml";
    case ".json":
      return "application/json;charset=utf-8";
    default:
      return "text/html;charset=utf-8";
  }
}

function laneJsMetrics(laneDir) {
  const assetsDir = join(laneDir, "assets");
  if (!existsSync(assetsDir)) return null;
  const files = readdirSync(assetsDir)
    .filter((name) => name.endsWith(".js"))
    .sort()
    .map((name) => join(assetsDir, name));
  if (files.length === 0) return null;
  const measurements = canonicalCodecMeasurementsForFiles(
    files,
    "Lilastro browser lab measurement",
  );
  return measurements.reduce(
    (total, measured) => ({
      raw: total.raw + measured.raw,
      gzip: total.gzip + measured.gzip,
      brotli: total.brotli + measured.brotli,
    }),
    { raw: 0, gzip: 0, brotli: 0 },
  );
}

function fmtBytes(n) {
  if (n == null) return "—";
  if (n < 1024) return `${n} B`;
  return `${(n / 1024).toFixed(1)} KB`;
}

function fmtRatio(lil, npm) {
  if (lil == null || npm == null || npm === 0) return "—";
  const ratio = lil / npm;
  const cls = ratio < 1 ? "win" : ratio > 1 ? "lose" : "";
  const delta = Math.abs((1 - ratio) * 100).toFixed(0);
  const hint = ratio < 1 ? ` (−${delta}%)` : ratio > 1 ? ` (+${delta}%)` : "";
  return `<span class="${cls}">${ratio.toFixed(2)}×${hint}</span>`;
}

function sizeCell(npm, lil, key) {
  return `<td class="num">${fmtBytes(npm?.[key])}</td>
      <td class="num">${fmtBytes(lil?.[key])}</td>
      <td class="num">${fmtRatio(lil?.[key], npm?.[key])}</td>`;
}

function indexHtml() {
  const codecs = canonicalCodecProvenance("Lilastro browser lab measurement");
  const measured = FIXTURES.map((id) => {
    const npmDir = join(buildRoot, `${id}-npm`);
    const lilDir = join(buildRoot, `${id}-lil`);
    const npmExists = existsSync(join(npmDir, "index.html"));
    const lilExists = existsSync(join(lilDir, "index.html"));
    const npm = npmExists ? laneJsMetrics(npmDir) : null;
    const lil = lilExists ? laneJsMetrics(lilDir) : null;
    return { id, npmExists, lilExists, npm, lil };
  });
  const rows = measured
    .map(
      ({ id, npmExists, lilExists, npm, lil }) => `<tr>
      <td><code>${id}</code></td>
      <td>${npmExists ? `<a href="/${id}-npm/">npm</a>` : "missing"}</td>
      <td>${lilExists ? `<a href="/${id}-lil/">lil</a>` : "missing"}</td>
      ${sizeCell(npm, lil, "raw")}
      ${sizeCell(npm, lil, "gzip")}
      ${sizeCell(npm, lil, "brotli")}
    </tr>`,
    )
    .join("\n");

  const sized = measured.filter((row) => row.npm && row.lil);

  const avg = (key) => {
    if (sized.length === 0) return null;
    const sum = sized.reduce(
      (acc, row) => acc + row.lil[key] / row.npm[key],
      0,
    );
    return sum / sized.length;
  };
  const avgBrotli = avg("brotli");
  const summary =
    avgBrotli == null
      ? ""
      : `<p class="summary">Avg lil/npm brotli across ${sized.length} fixtures: <strong class="${avgBrotli < 1 ? "win" : "lose"}">${avgBrotli.toFixed(2)}×</strong> (Vite 8 minify on).</p>`;

  return `<!doctype html>
<html>
<head>
  <meta charset="utf-8" />
  <title>Motion lab — lil vs npm</title>
  <style>
    :root { color-scheme: light; }
    body {
      margin: 0;
      font: 15px/1.45 "IBM Plex Sans", "Segoe UI", sans-serif;
      background:
        radial-gradient(1200px 600px at 10% -10%, #d9ecff 0%, transparent 55%),
        radial-gradient(900px 500px at 100% 0%, #ffe8d6 0%, transparent 50%),
        #f7f5f1;
      color: #1c1a17;
    }
    main { max-width: 1180px; margin: 0 auto; padding: 48px 24px 80px; }
    h1 { font: 700 40px/1.1 "Fraunces", Georgia, serif; margin: 0 0 8px; }
    p { margin: 0 0 20px; color: #524a42; max-width: 48rem; }
    .summary { margin-bottom: 28px; }
    .scroll { overflow-x: auto; border: 1px solid #e4ddd4; background: rgba(255,255,255,.72); }
    table { width: 100%; border-collapse: collapse; min-width: 980px; }
    th, td { text-align: left; padding: 10px 12px; border-bottom: 1px solid #e4ddd4; vertical-align: middle; }
    th { font-size: 11px; letter-spacing: .04em; text-transform: uppercase; color: #7a7168; white-space: nowrap; }
    th.group { text-align: center; border-bottom: 0; padding-bottom: 2px; color: #524a42; }
    th.num, td.num {
      text-align: right;
      font-variant-numeric: tabular-nums;
      font-family: "IBM Plex Mono", ui-monospace, monospace;
      font-size: 12.5px;
      white-space: nowrap;
    }
    a { color: #0b5fff; text-decoration: none; font-weight: 600; }
    a:hover { text-decoration: underline; }
    code { font-family: "IBM Plex Mono", ui-monospace, monospace; font-size: 13px; }
    .win { color: #0f7a3a; font-weight: 700; }
    .lose { color: #b42318; font-weight: 700; }
  </style>
</head>
<body>
  <main>
    <h1>Motion lab</h1>
    <p>Side-by-side npm <code>motion@13</code> vs LilScript port. Open both lanes for the same fixture and compare behavior. Bundle sizes are Vite&nbsp;8 minified JS (raw / gzip / brotli) for fixtures that <em>call</em> the APIs; multiple chunks are encoded independently and summed.</p>
    ${summary}
    <p class="summary">Sizes are Vite&nbsp;8 minified call-site bundles with LilScript <code>size-first</code> app mangling. Scored by bundled zlib ${codecs.gzip9.libraryVersion} and Google Brotli ${codecs.brotli11.libraryVersion}. Slim DOM <code>animate/element</code> entry (sequence/SVG/projection/spring split out unless needed).</p>
    <div class="scroll">
    <table>
      <thead>
        <tr>
          <th rowspan="2">Fixture</th>
          <th rowspan="2">npm</th>
          <th rowspan="2">lil</th>
          <th class="group" colspan="3">raw</th>
          <th class="group" colspan="3">gzip</th>
          <th class="group" colspan="3">brotli</th>
        </tr>
        <tr>
          <th class="num">npm</th><th class="num">lil</th><th class="num">ratio</th>
          <th class="num">npm</th><th class="num">lil</th><th class="num">ratio</th>
          <th class="num">npm</th><th class="num">lil</th><th class="num">ratio</th>
        </tr>
      </thead>
      <tbody>${rows}</tbody>
    </table>
    </div>
  </main>
</body>
</html>`;
}

await buildBrowserFixtures({ rebuild });

const server = createServer((request, response) => {
  const url = new URL(request.url ?? "/", `http://127.0.0.1:${port}`);
  if (url.pathname === "/" || url.pathname === "/index.html") {
    response.writeHead(200, {
      "content-type": "text/html;charset=utf-8",
      "cache-control": "no-store",
    });
    response.end(indexHtml());
    return;
  }

  const parts = url.pathname.split("/").filter(Boolean);
  const lane = parts[0];
  const root = join(buildRoot, lane);
  if (!existsSync(root) || !statSync(root).isDirectory()) {
    response.writeHead(404).end(`unknown lane: ${lane}`);
    return;
  }
  const rel = parts.slice(1).join("/") || "index.html";
  const path = resolve(root, normalize(rel));
  if (!path.startsWith(root)) {
    response.writeHead(403).end();
    return;
  }
  try {
    const content = readFileSync(path);
    response.writeHead(200, {
      "content-type": contentType(path),
      "cache-control": "no-store",
    });
    response.end(content);
  } catch {
    response.writeHead(404).end("not found");
  }
});

server.listen(port, "127.0.0.1", () => {
  const lanes = existsSync(buildRoot)
    ? readdirSync(buildRoot).filter((name) =>
        existsSync(join(buildRoot, name, "index.html")),
      )
    : [];
  console.log(`Motion lab at http://127.0.0.1:${port}/`);
  console.log(`Serving ${lanes.length} lanes from ${buildRoot}`);
});
