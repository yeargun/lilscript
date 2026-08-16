import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { canonicalCodecProvenance } from "../../benchmarks/codec-contract.mjs";

const labRoot = dirname(fileURLToPath(import.meta.url));
const lilastroRoot = resolve(labRoot, "..");
const repoRoot = resolve(lilastroRoot, "..");
const results = JSON.parse(
  readFileSync(join(lilastroRoot, "build/results.json"), "utf8"),
);
const playwrightPath = join(lilastroRoot, "build/playwright-results.json");
const playwright = existsSync(playwrightPath)
  ? JSON.parse(readFileSync(playwrightPath, "utf8"))
  : null;
const modesPath = join(lilastroRoot, "build/modes/results.json");
assert.equal(
  existsSync(modesPath),
  true,
  "Lilastro report requires fresh build-mode evidence; run npm run verify:modes",
);
const modes = JSON.parse(readFileSync(modesPath, "utf8"));

function requireCompiler(compiler, label) {
  assert.equal(typeof compiler?.path, "string", `${label} compiler path`);
  assert.equal(
    typeof compiler?.absolutePath,
    "string",
    `${label} absolute compiler path`,
  );
  assert.equal(typeof compiler?.version, "string", `${label} compiler version`);
  assert.ok(compiler.version.length > 0, `${label} compiler version`);
  assert.match(
    compiler?.sha256 ?? "",
    /^[a-f0-9]{64}$/u,
    `${label} compiler digest`,
  );
  assert.ok(Number.isSafeInteger(compiler?.bytes), `${label} compiler bytes`);
  const bytes = readFileSync(compiler.absolutePath);
  assert.equal(bytes.length, compiler.bytes, `${label} current compiler bytes`);
  assert.equal(
    createHash("sha256").update(bytes).digest("hex"),
    compiler.sha256,
    `${label} current compiler digest`,
  );
  return compiler;
}

function requireBrotliConfig(config, label) {
  assert.equal(typeof config?.path, "string", `${label} config path`);
  assert.equal(
    typeof config?.absolutePath,
    "string",
    `${label} absolute config path`,
  );
  assert.equal(config?.costModel, "brotli", `${label} config objective`);
  assert.match(
    config?.sha256 ?? "",
    /^[a-f0-9]{64}$/u,
    `${label} config digest`,
  );
  assert.ok(Number.isSafeInteger(config?.bytes), `${label} config bytes`);
  const bytes = readFileSync(config.absolutePath);
  assert.equal(bytes.length, config.bytes, `${label} current config bytes`);
  assert.equal(
    createHash("sha256").update(bytes).digest("hex"),
    config.sha256,
    `${label} current config digest`,
  );
  return config;
}

function requireCanonicalCodecs(codecs, label) {
  assert.equal(
    codecs?.implementation,
    "lilscript-codec",
    `${label} must use lilscript-codec`,
  );
  assert.equal(codecs?.schemaVersion, 1, `${label} codec schema`);
  assert.equal(
    codecs?.gzip9?.encoder,
    "upstream-stock-zlib-c",
    `${label} gzip encoder`,
  );
  assert.equal(codecs?.gzip9?.libraryVersion, "1.3.1", `${label} zlib`);
  assert.equal(
    codecs?.brotli11?.encoder,
    "official-google-brotli-c",
    `${label} Brotli encoder`,
  );
  assert.equal(codecs?.brotli11?.libraryVersion, "1.1.0", `${label} Brotli`);
  assert.match(
    codecs?.scorer?.sha256 ?? "",
    /^[a-f0-9]{64}$/,
    `${label} scorer`,
  );
  return codecs;
}

const reportCodecs = requireCanonicalCodecs(
  results.toolchain?.codecs,
  "Lilastro measurement",
);
const modeCodecs = requireCanonicalCodecs(
  modes.toolchain?.codecs,
  "Lilastro build-mode verification",
);
assert.deepEqual(
  modeCodecs,
  reportCodecs,
  "Lilastro measurement and build-mode evidence must use the same scorer binary",
);
assert.deepEqual(
  canonicalCodecProvenance("Lilastro report assembly"),
  reportCodecs,
  "Lilastro evidence must use the current canonical scorer binary",
);
const reportCompiler = requireCompiler(
  results.toolchain?.compiler,
  "Lilastro measurement",
);
const modeCompiler = requireCompiler(
  modes.toolchain?.compiler,
  "Lilastro build-mode verification",
);
assert.deepEqual(
  modeCompiler,
  reportCompiler,
  "Lilastro measurement and build-mode evidence must use the same compiler binary",
);
const measurementConfig = requireBrotliConfig(
  results.toolchain?.compilerConfig,
  "Lilastro measurement",
);
requireBrotliConfig(
  modes.toolchain?.configs?.openWorld,
  "Lilastro open-world build",
);
const closedModeConfig = requireBrotliConfig(
  modes.toolchain?.configs?.closedWorld,
  "Lilastro closed-world build",
);
assert.deepEqual(
  closedModeConfig,
  measurementConfig,
  "Lilastro measurement and closed-world evidence must use the same config bytes",
);

function fmt(n) {
  return new Intl.NumberFormat("en-US").format(n);
}

function pct(ratio) {
  const delta = (ratio - 1) * 100;
  const sign = delta > 0 ? "+" : "";
  return `${sign}${delta.toFixed(1)}%`;
}

function fmtRatio(ratio) {
  return `${ratio.toFixed(3)}×`;
}

function row(ex) {
  const r = ex.ratios.brotli;
  const better = r < 1;
  return `<tr>
  <td><strong>${ex.id}</strong><div class="muted">${ex.title}</div><div class="apis">${ex.apis.join(", ")}</div></td>
  <td class="num">${fmt(ex.npm.raw)}</td>
  <td class="num">${fmt(ex.npm.gzip)}</td>
  <td class="num">${fmt(ex.npm.brotli)}</td>
  <td class="num">${fmt(ex.lil.raw)}</td>
  <td class="num">${fmt(ex.lil.gzip)}</td>
  <td class="num">${fmt(ex.lil.brotli)}</td>
  <td class="num ${better ? "win" : "lose"}">${ex.ratios.brotli.toFixed(3)}× <span class="muted">${pct(r)}</span></td>
</tr>`;
}

function correctnessRows() {
  if (!playwright?.correctness?.length) {
    return `<tr><td colspan="4" class="muted">No playwright correctness results yet. Run <code>npm run playwright</code>.</td></tr>`;
  }
  return playwright.correctness
    .map((fixture) => {
      const npm = fixture.lanes.npm;
      const lil = fixture.lanes.lil;
      return `<tr>
  <td><strong>${fixture.id}</strong></td>
  <td class="${npm.ok ? "win" : "lose"}">${npm.ok ? "pass" : "fail"}<div class="muted">${npm.message}</div></td>
  <td class="${lil.ok ? "win" : "lose"}">${lil.ok ? "pass" : "fail"}<div class="muted">${lil.message}</div></td>
  <td class="muted">${(lil.pageErrors || []).concat(npm.pageErrors || []).join("; ") || "—"}</td>
</tr>`;
    })
    .join("\n");
}

function perfSections() {
  if (!playwright?.performance?.length) {
    return `<p class="muted">No statistical perf results yet.</p>`;
  }
  const method = playwright.methodology;
  const blocks = playwright.performance
    .map((entry) => {
      const rows = Object.entries(entry.comparisons)
        .map(([metric, cmp]) => {
          const cls = cmp.withinBudget ? "win" : "lose";
          return `<tr>
  <td><strong>${metric}</strong></td>
  <td class="num">${cmp.npm.mean.toFixed(3)}</td>
  <td class="num">${cmp.npm.median.toFixed(3)}</td>
  <td class="num">${cmp.npm.p95.toFixed(3)}</td>
  <td class="num">${cmp.lil.mean.toFixed(3)}</td>
  <td class="num">${cmp.lil.median.toFixed(3)}</td>
  <td class="num">${cmp.lil.p95.toFixed(3)}</td>
  <td class="num">${fmtRatio(cmp.ratio.median)}</td>
  <td class="num ${cls}">${fmtRatio(cmp.upperConfidenceRatio.median)} / ${fmtRatio(cmp.upperConfidenceRatio.p95)}</td>
</tr>`;
        })
        .join("\n");
      return `
  <h3>${entry.mode} · n=${entry.sampleCount} · warmup=${entry.warmupRounds} · order lil-first=${entry.orderCounts.lilFirst}, npm-first=${entry.orderCounts.npmFirst}</h3>
  <p class="muted">${entry.ok ? "Within non-inferiority budget" : "Outside non-inferiority budget"} (max upper ratio ${entry.maxRatio}).</p>
  <table>
    <thead>
      <tr>
        <th>Metric</th>
        <th class="num">npm mean</th>
        <th class="num">npm median</th>
        <th class="num">npm p95</th>
        <th class="num">lil mean</th>
        <th class="num">lil median</th>
        <th class="num">lil p95</th>
        <th class="num">median ratio</th>
        <th class="num">95% upper median/p95</th>
      </tr>
    </thead>
    <tbody>${rows}</tbody>
  </table>`;
    })
    .join("\n");
  return `
  <p class="lede">
    Paired rounds with <strong>randomized lane order</strong>, discarded warmup,
    then mean / median / p95 plus paired-bootstrap ${method?.confidence ?? 0.95}
    upper confidence ratios (same harness as <code>benchmarks/statistics.mjs</code>).
    Cold = fresh navigation per sample; warm = reused page + <code>__runPerfSample</code>.
  </p>
  ${blocks}`;
}

const avgBrotli =
  results.examples.reduce((s, e) => s + e.ratios.brotli, 0) /
  results.examples.length;

const correctnessPass =
  Boolean(playwright?.correctness?.length) &&
  playwright.correctness.every(
    (fixture) => fixture.lanes.npm?.ok && fixture.lanes.lil?.ok,
  );
const perfPass =
  Boolean(playwright?.performance?.length) &&
  playwright.performance.every((entry) => entry.ok);
const modePass = Boolean(
  modes?.openWorld?.publicApiPassed &&
  modes?.closedWorld?.behaviorPassed &&
  modes?.openWorld?.size?.superior &&
  modes?.closedWorld?.size?.superior,
);
const warmPerf = playwright?.performance?.find(
  (entry) => entry.mode === "warm",
);
const coldPerf = playwright?.performance?.find(
  (entry) => entry.mode === "cold",
);
const warmSchedule = warmPerf?.comparisons?.scheduleMs;
const coldSchedule = coldPerf?.comparisons?.scheduleMs;
const warmHeap = warmPerf?.comparisons?.heapUsed;
const coldHeap = coldPerf?.comparisons?.heapUsed;

function modeRow(label, value) {
  if (!value) {
    return `<tr><th>${label}</th><td colspan="6" class="muted">pending — run <code>npm run verify:modes</code></td></tr>`;
  }
  return `<tr>
  <th>${label}</th>
  <td class="num">${fmt(value.npm.raw)}</td>
  <td class="num">${fmt(value.lil.raw)}</td>
  <td class="num">${fmtRatio(value.ratio.raw)}</td>
  <td class="num">${fmt(value.npm.brotli11)}</td>
  <td class="num">${fmt(value.lil.brotli11)}</td>
  <td class="num win">${fmtRatio(value.ratio.brotli11)}</td>
</tr>`;
}

const html = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Lilastro — Motion JS/DOM finer report</title>
  <style>
    :root {
      --bg: #0f1419;
      --panel: #171e26;
      --ink: #e8eef5;
      --muted: #8b9aab;
      --line: #2a3542;
      --accent: #5ee1a8;
      --warn: #f0b429;
      --bad: #ff7b72;
    }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      font-family: "IBM Plex Sans", "Segoe UI", sans-serif;
      background:
        radial-gradient(1200px 600px at 10% -10%, #1c3a3a 0%, transparent 55%),
        radial-gradient(900px 500px at 90% 0%, #24304a 0%, transparent 50%),
        var(--bg);
      color: var(--ink);
      line-height: 1.45;
    }
    main { max-width: 1180px; margin: 0 auto; padding: 48px 20px 80px; }
    h1 { font-size: 2rem; letter-spacing: -0.03em; margin: 0 0 8px; }
    h2 { font-size: 1.15rem; margin: 36px 0 12px; }
    h3 { font-size: 1rem; margin: 24px 0 8px; color: var(--muted); }
    .lede { color: var(--muted); max-width: 70ch; }
    .banner {
      margin: 24px 0;
      padding: 14px 16px;
      border: 1px solid var(--line);
      border-left: 4px solid var(--warn);
      background: var(--panel);
      border-radius: 8px;
    }
    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
      gap: 12px;
      margin: 20px 0 8px;
    }
    .card {
      background: var(--panel);
      border: 1px solid var(--line);
      border-radius: 10px;
      padding: 14px 16px;
    }
    .card .label { color: var(--muted); font-size: 0.8rem; text-transform: uppercase; letter-spacing: 0.06em; }
    .card .value { font-size: 1.4rem; font-variant-numeric: tabular-nums; margin-top: 4px; }
    table {
      width: 100%;
      border-collapse: collapse;
      background: var(--panel);
      border: 1px solid var(--line);
      border-radius: 10px;
      overflow: hidden;
      font-size: 0.9rem;
      margin-bottom: 12px;
    }
    th, td { padding: 10px 12px; border-bottom: 1px solid var(--line); vertical-align: top; }
    th { text-align: left; color: var(--muted); font-weight: 600; font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.05em; }
    .num { font-variant-numeric: tabular-nums; text-align: right; white-space: nowrap; }
    .win { color: var(--accent); }
    .lose { color: var(--bad); }
    .muted { color: var(--muted); font-size: 0.85rem; }
    .apis { color: var(--accent); font-size: 0.78rem; margin-top: 4px; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }
    ul { padding-left: 1.15rem; color: var(--muted); }
    code { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 0.88em; }
    a { color: var(--accent); }
    @media (max-width: 900px) {
      .grid { grid-template-columns: 1fr 1fr; }
      table { display: block; overflow-x: auto; }
    }
  </style>
</head>
<body>
<main>
  <h1>Lilastro · Motion JS/DOM finer report</h1>
  <p class="lede">
    Vite&nbsp;8 sizes plus Playwright CSSOM correctness and statistical
    DOM/CSSOM/memory gates for npm <code>motion@${results.toolchain.motionNpm}</code>
    vs LilScript <code>benchmarks/popular/ports/motion</code>
    (<a href="${results.upstream.repo}">motiondivision/motion</a> ${results.upstream.tag}).
  </p>

  <div class="banner">
    <strong>Completeness:</strong> ${results.completeness.claim}.
    React / Vue entrypoints remain out of scope. Size rows are tree-shaken
    retained API surface; Playwright rows are the behavioral/perf evidence.
  </div>

  <div class="grid">
    <div class="card">
      <div class="label">ABI modes</div>
      <div class="value ${modePass ? "win" : "lose"}">${modePass ? "pass" : "fail/pending"}</div>
    </div>
    <div class="card">
      <div class="label">Size examples</div>
      <div class="value">${results.examples.length}</div>
    </div>
    <div class="card">
      <div class="label">Avg Brotli lil/npm</div>
      <div class="value ${avgBrotli < 1 ? "win" : "lose"}">${avgBrotli.toFixed(3)}×</div>
    </div>
    <div class="card">
      <div class="label">Playwright CSSOM</div>
      <div class="value ${correctnessPass ? "win" : "lose"}">${correctnessPass ? "pass" : "fail/pending"}</div>
    </div>
    <div class="card">
      <div class="label">Perf gate</div>
      <div class="value ${perfPass ? "win" : "lose"}">${perfPass ? "pass" : "fail/pending"}</div>
    </div>
  </div>

  <h2>Open-world and closed-world contracts</h2>
  <p class="lede">
    Open world preserves the selected reusable ESM API and public aggregate
    fields. Closed world links the consumer first, then permits every
    LilScript-owned name to be mangled or erased. Both lanes must match npm
    behavior and beat it for the configured Brotli-11 objective. Raw and
    gzip-9 are diagnostics for those same Brotli-target artifacts.
  </p>
  <table>
    <thead><tr><th>Mode</th><th class="num">npm raw</th><th class="num">lil raw</th><th class="num">raw ratio</th><th class="num">npm br</th><th class="num">lil br</th><th class="num">br ratio</th></tr></thead>
    <tbody>
      ${modeRow("Open-world core API", modes?.openWorld?.size)}
      ${modeRow("Closed-world values app", modes?.closedWorld?.size)}
    </tbody>
  </table>
  <p class="muted">Open-world export names: ${modes?.openWorld?.exports?.map((name) => `<code>${name}</code>`).join(", ") ?? "pending"}. Closed-world diagnostic exports are all renamed.</p>

  <h2>Per-example Vite 8 sizes (bytes)</h2>
  <table>
    <thead>
      <tr>
        <th>Example</th>
        <th class="num">npm raw</th>
        <th class="num">npm gzip</th>
        <th class="num">npm br</th>
        <th class="num">lil raw</th>
        <th class="num">lil gzip</th>
        <th class="num">lil br</th>
        <th class="num">br ratio</th>
      </tr>
    </thead>
    <tbody>
      ${results.examples.map(row).join("\n")}
    </tbody>
  </table>

  <h2>Playwright CSSOM correctness</h2>
  <table>
    <thead>
      <tr>
        <th>Fixture</th>
        <th>npm</th>
        <th>lil</th>
        <th>page errors</th>
      </tr>
    </thead>
    <tbody>
      ${correctnessRows()}
    </tbody>
  </table>

  <h2>Statistical DOM / CSSOM / memory perf</h2>
  ${perfSections()}

  <h2>Drift root-cause findings</h2>
  <p>
    The first complete run failed rather than being rounded into a pass. Inspection separated
    harness defects from missing Motion control paths and cold-start allocation overhead.
  </p>
  <table>
    <thead><tr><th>Finding</th><th>Effect measured</th><th>Status</th></tr></thead>
    <tbody>
      <tr>
        <td>Correctness waited for Playwright <code>networkidle</code>, adding 500&nbsp;ms after module
        evaluation and sampling the 350&nbsp;ms stagger after it had ended.</td>
        <td>Both npm and LilScript falsely failed the mid-animation cascade check.</td>
        <td>fixed — animation-start fixtures now continue at document <code>load</code></td>
      </tr>
      <tr>
        <td>The hybrid <code>animate()</code> path treated an explicit CSS <code>transform</code> as
        a JS MotionValue, while the mini controller lacked timeline playback controls.</td>
        <td>One hybrid target did not animate and two scroll-linked WAAPI lanes stayed at zero.</td>
        <td>fixed — native transforms route through WAAPI and mini controls expose timeline time</td>
      </tr>
      <tr>
        <td>Every inner mini animation eagerly allocated completion Promises, callback closures,
        keyframe copies, and dynamic method wrappers even when the returned group was ignored.</td>
        <td>Cold scheduling initially missed the non-inferiority confidence bound.</td>
        <td>fixed — completion is lazy, active-map cleanup is direct, and internals use typed dispatch</td>
      </tr>
      <tr>
        <td>CDP heap was sampled without forcing collection.</td>
        <td>Warm results depended on V8's incidental GC timing rather than retained library state.</td>
        <td>fixed — every retained-heap sample follows <code>HeapProfiler.collectGarbage</code></td>
      </tr>
    </tbody>
  </table>
  <p>
    Final 192-element scheduling ratios are
    <strong>${warmSchedule ? fmtRatio(warmSchedule.ratio.median) : "pending"}</strong> warm and
    <strong>${coldSchedule ? fmtRatio(coldSchedule.ratio.median) : "pending"}</strong> cold.
    Forced-GC retained-heap ratios are
    <strong>${warmHeap ? fmtRatio(warmHeap.ratio.median) : "pending"}</strong> warm and
    <strong>${coldHeap ? fmtRatio(coldHeap.ratio.median) : "pending"}</strong> cold.
    All paired-bootstrap upper bounds remain within the ${playwright?.methodology?.maxRatio ?? 1.15}&times;
    non-inferiority budget.
  </p>

  <h2>Methodology</h2>
  <ul>
    <li>Size lane: Vite <code>${results.toolchain.vite}</code>, minify on, Node ${results.toolchain.node}, LilScript mode <code>${results.toolchain.buildMode ?? "unrecorded"}</code>.</li>
    <li>LilScript compiler: <code>${reportCompiler.path}</code>, <code>${reportCompiler.version}</code>, SHA-256 <code>${reportCompiler.sha256}</code>; closed-world config <code>${measurementConfig.path}</code>, SHA-256 <code>${measurementConfig.sha256}</code>.</li>
    <li>Canonical transfer scorer: upstream zlib <code>${reportCodecs.gzip9.libraryVersion}</code> at level ${reportCodecs.gzip9.level}, official Google Brotli <code>${reportCodecs.brotli11.libraryVersion}</code> at quality ${reportCodecs.brotli11.quality} / lgwin ${reportCodecs.brotli11.lgwin}; scorer SHA-256 <code>${reportCodecs.scorer.sha256}</code>.</li>
    <li>Correctness: ten paired animation, CSS-variable, stagger, spring, scroll, gesture, viewport, resize, and MotionValue fixtures.</li>
    <li>Perf: 192 elements × two animated properties, randomized lil/npm order, ${playwright?.methodology?.warmupRounds ?? 8} warmup discarded, n=${playwright?.methodology?.sampleCount ?? "≥201"}, mean/median/p95, paired-bootstrap 95% upper ratios via <code>benchmarks/statistics.mjs</code>.</li>
    <li>Cold vs warm measured separately; retained heap is Chromium CDP <code>JSHeapUsedSize</code> after explicit collection.</li>
  </ul>
</main>
</body>
</html>
`;

const out = join(repoRoot, "report-motion-finer.html");
writeFileSync(out, html);
console.log(`wrote ${out}`);
