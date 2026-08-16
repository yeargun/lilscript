import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { pathToFileURL, fileURLToPath } from "node:url";
import { build } from "vite";
import {
  canonicalCodecProvenance,
  canonicalCodecSizes,
} from "../../benchmarks/codec-contract.mjs";
import {
  resolveBrotliConfig,
  resolveCompilerToolchain,
} from "./evidence-toolchain.mjs";

const scriptRoot = dirname(fileURLToPath(import.meta.url));
const lilastroRoot = resolve(scriptRoot, "..");
const repositoryRoot = resolve(lilastroRoot, "..");
const compilerToolchain = resolveCompilerToolchain(
  repositoryRoot,
  "Lilastro build-mode verification",
);
const compiler = compilerToolchain.executable;
const buildRoot = resolve(lilastroRoot, "build/modes");
const sourceRoot = resolve(buildRoot, "generated");
const openConfiguration = resolveBrotliConfig(
  resolve(lilastroRoot, "config/open-world.toml"),
  repositoryRoot,
  "Lilastro open-world build",
);
const closedConfiguration = resolveBrotliConfig(
  process.env.LILSCRIPT_CONFIG
    ? resolve(process.cwd(), process.env.LILSCRIPT_CONFIG)
    : resolve(lilastroRoot, "config/closed-world.toml"),
  repositoryRoot,
  "Lilastro closed-world build",
);
const openConfig = openConfiguration.resolvedPath;
const closedConfig = closedConfiguration.resolvedPath;
const codecs = canonicalCodecProvenance("Lilastro build-mode verification");
const lilPublicEntry = resolve(
  repositoryRoot,
  "benchmarks/popular/ports/motion/entry.lil",
);
const npmPublicEntry = resolve(lilastroRoot, "api/motion-core.js");
const lilAppEntry = resolve(lilastroRoot, "examples/values-core/lil/main.lil");
const npmAppEntry = resolve(lilastroRoot, "examples/values-core/ts/main.js");
const publicNames = [
  "clamp",
  "distance",
  "distance2D",
  "getOriginIndex",
  "mix",
  "mixNumber",
  "spring",
  "stagger",
  "wrap",
].sort();

mkdirSync(sourceRoot, { recursive: true });

function run(program, args, cwd = lilastroRoot) {
  const result = spawnSync(program, args, {
    cwd,
    encoding: "utf8",
    env: process.env,
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(
      `${program} ${args.join(" ")}\n${result.stdout ?? ""}${result.stderr ?? ""}`,
    );
  }
  return result.stdout.trim();
}

function compile(input, output, { config, target }) {
  run(compiler, [input, "--target", target, "--config", config, "-o", output]);
}

async function bundle(entry, output) {
  const result = await build({
    configFile: false,
    root: lilastroRoot,
    logLevel: "error",
    build: {
      target: "es2022",
      minify: "oxc",
      write: false,
      lib: {
        entry,
        formats: ["es"],
        fileName: "bundle",
      },
      rolldownOptions: {
        output: { codeSplitting: false },
      },
    },
  });
  const outputs = Array.isArray(result)
    ? result.flatMap((item) => item.output)
    : result.output;
  const chunks = outputs.filter((item) => item.type === "chunk");
  assert.equal(
    chunks.length,
    1,
    `${entry} should produce one JavaScript chunk`,
  );
  mkdirSync(dirname(output), { recursive: true });
  writeFileSync(output, `${chunks[0].code.trim()}\n`);
  return chunks[0].code;
}

function size(code) {
  const measured = canonicalCodecSizes(
    code,
    "Lilastro build-mode verification",
  );
  return {
    raw: measured.raw,
    gzip9: measured.gzip,
    brotli11: measured.brotli,
  };
}

function sizeComparison(npm, lil) {
  return {
    npm,
    lil,
    ratio: Object.fromEntries(
      Object.keys(npm).map((metric) => [metric, lil[metric] / npm[metric]]),
    ),
    // Both LilScript configs select Brotli. Raw and gzip are diagnostics for
    // these exact deploy artifacts, not cross-objective release gates.
    superior: lil.brotli11 < npm.brotli11,
  };
}

async function importFresh(path) {
  const url = pathToFileURL(path);
  url.searchParams.set("build", `${Date.now()}-${Math.random()}`);
  return import(url.href);
}

function round(value) {
  return Number(value.toFixed(9));
}

function publicApiDigest(api) {
  const delay = api.stagger(0.125, { startDelay: 0.25, from: "center" });
  const generator = api.spring({
    keyframes: [0, 100],
    stiffness: 170,
    damping: 26,
    mass: 1,
  });
  return {
    clamp: [-2, 4, 12].map((value) => api.clamp(0, 10, value)),
    distance: api.distance(-4, 7),
    distance2D: round(api.distance2D({ x: -1, y: 2 }, { x: 5, y: 10 })),
    mix: [0, 0.25, 1].map((progress) => round(api.mix(-20, 60, progress))),
    mixNumber: round(api.mixNumber(2, 10, 0.375)),
    origin: [
      api.getOriginIndex("first", 8),
      api.getOriginIndex("center", 8),
      api.getOriginIndex("last", 8),
    ],
    stagger: [0, 1, 3, 7].map((index) => round(delay(index, 8))),
    spring: [0, 80, 160, 320, 640].map((time) => {
      const value = generator.next(time);
      return { done: value.done, value: round(value.value) };
    }),
    wrap: [-13, -1, 0, 9, 21].map((value) => api.wrap(0, 10, value)),
  };
}

function assertPublicModule(module, label) {
  assert.deepEqual(
    Object.keys(module).sort(),
    publicNames,
    `${label} public exports`,
  );
  for (const name of publicNames) {
    assert.equal(
      typeof module[name],
      "function",
      `${label}.${name} must be callable`,
    );
  }
  const generator = module.spring({ keyframes: [0, 1] });
  assert.equal(
    typeof generator.next,
    "function",
    `${label} must preserve generator.next`,
  );
}

function formatBytes(value) {
  return new Intl.NumberFormat("en-US").format(value);
}

function deltaCell(ratio) {
  const delta = (ratio - 1) * 100;
  const prefix = delta > 0 ? "+" : "";
  return `<td class="${delta < 0 ? "win" : "tradeoff"}">${prefix}${delta.toFixed(1)}%</td>`;
}

function htmlReport(report) {
  const row = (label, comparison) => `<tr>
    <th scope="row">${label}</th>
    <td>${formatBytes(comparison.npm.raw)}</td>
    <td>${formatBytes(comparison.lil.raw)}</td>
    ${deltaCell(comparison.ratio.raw)}
    <td>${formatBytes(comparison.npm.gzip9)}</td>
    <td>${formatBytes(comparison.lil.gzip9)}</td>
    ${deltaCell(comparison.ratio.gzip9)}
    <td>${formatBytes(comparison.npm.brotli11)}</td>
    <td>${formatBytes(comparison.lil.brotli11)}</td>
    ${deltaCell(comparison.ratio.brotli11)}
  </tr>`;
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="description" content="Verified open-world and closed-world Motion build comparison for LilScript.">
  <title>Lilastro Motion build-mode verification</title>
  <style>
    :root { color-scheme: dark; --bg:#0b1116; --panel:#121b23; --line:#263541; --ink:#edf6f4; --muted:#9bb0ad; --mint:#5ce1b9; --amber:#ffc86b; }
    * { box-sizing: border-box; }
    body { margin:0; color:var(--ink); background:radial-gradient(circle at 12% 0,#123d39 0,transparent 34rem),var(--bg); font:15px/1.55 Inter,ui-sans-serif,system-ui,sans-serif; }
    main { width:min(1120px,calc(100% - 32px)); margin:auto; padding:64px 0 88px; }
    h1,h2 { letter-spacing:-.035em; } h1 { max-width:14ch; margin:8px 0 16px; font-size:clamp(2.4rem,7vw,5.4rem); line-height:.96; }
    h2 { margin:0 0 10px; font-size:1.35rem; } p { color:var(--muted); max-width:74ch; }
    .eyebrow { color:var(--mint); font:700 .75rem/1 ui-monospace,monospace; letter-spacing:.14em; text-transform:uppercase; }
    .verdict { display:grid; grid-template-columns:repeat(3,1fr); gap:12px; margin:32px 0; }
    .card,.mode { border:1px solid var(--line); background:color-mix(in srgb,var(--panel) 92%,transparent); border-radius:16px; padding:20px; }
    .card strong { display:block; margin-top:4px; color:var(--mint); font-size:1.5rem; } .card span { color:var(--muted); }
    .modes { display:grid; grid-template-columns:1fr 1fr; gap:16px; margin:20px 0 36px; }
    code { color:var(--amber); font:inherit; font-family:ui-monospace,monospace; }
    .table-wrap { overflow:auto; border:1px solid var(--line); border-radius:16px; }
    table { width:100%; min-width:900px; border-collapse:collapse; background:var(--panel); font-variant-numeric:tabular-nums; }
    th,td { padding:12px 14px; border-bottom:1px solid var(--line); text-align:right; white-space:nowrap; }
    th:first-child { text-align:left; } thead th { color:var(--muted); font-size:.72rem; letter-spacing:.08em; text-transform:uppercase; }
    .win { color:var(--mint); font-weight:700; }
    .tradeoff { color:var(--amber); font-weight:700; }
    .note { margin-top:24px; padding-left:16px; border-left:3px solid var(--amber); }
    @media (max-width:760px) { main { padding-top:40px; } .verdict,.modes { grid-template-columns:1fr; } }
  </style>
</head>
<body><main>
  <p class="eyebrow">Motion 13 · ABI-aware evidence</p>
  <h1>Two build worlds. Both verified.</h1>
  <p>The reusable build keeps all ${report.openWorld.exports.length} selected public names and the public <code>spring().next</code> method. The application build links the consumer first, mangles owned names, and exposes no public library surface.</p>
  <section class="verdict" aria-label="Verification summary">
    <div class="card"><span>Public API</span><strong>pass</strong></div>
    <div class="card"><span>Closed app behavior</span><strong>pass</strong></div>
    <div class="card"><span>Brotli targets won</span><strong>2 / 2</strong></div>
  </section>
  <section class="modes">
    <article class="mode"><h2>Open world</h2><p><code>js-module</code>, stable exports, stable fields reachable through exported aggregates. Private identifiers and safe owned fields still compress.</p></article>
    <article class="mode"><h2>Closed world</h2><p>The call site is linked before emission. Export names and public aggregate fields may be mangled or erased because no JavaScript consumer observes them.</p></article>
  </section>
  <h2>Equivalent retained surfaces</h2>
  <p>Bytes are one Vite 8 / Oxc ESM chunk. Gzip uses level 9; Brotli uses quality 11.</p>
  <div class="table-wrap"><table>
    <thead><tr><th rowspan="2">Build</th><th colspan="3">Raw</th><th colspan="3">Gzip-9</th><th colspan="3">Brotli-11</th></tr><tr><th>npm</th><th>Lil</th><th>saving</th><th>npm</th><th>Lil</th><th>saving</th><th>npm</th><th>Lil</th><th>saving</th></tr></thead>
    <tbody>${row("Open-world core API", report.openWorld.size)}${row("Closed-world values app", report.closedWorld.size)}</tbody>
  </table></div>
  <p class="note"><strong>Scope:</strong> this proves the selected nine-export core slice and the values-core consumer. It does not certify Motion’s full DOM export inventory; browser CSSOM, timing, and heap gates remain separate.</p>
</main></body></html>\n`;
}

const generated = {
  openLil: resolve(sourceRoot, "motion-open.mjs"),
  closedMangledLil: resolve(sourceRoot, "motion-closed-exports.mjs"),
  closedAppLil: resolve(sourceRoot, "values-core-closed.mjs"),
};
compile(lilPublicEntry, generated.openLil, {
  config: openConfig,
  target: "js-module",
});
compile(lilPublicEntry, generated.closedMangledLil, {
  config: closedConfig,
  target: "js-module",
});
compile(lilAppEntry, generated.closedAppLil, {
  config: closedConfig,
  target: "js",
});

const outputs = {
  openNpm: resolve(buildRoot, "open-world/npm.mjs"),
  openLil: resolve(buildRoot, "open-world/lilscript.mjs"),
  closedNpm: resolve(buildRoot, "closed-world/npm.mjs"),
  closedLil: resolve(buildRoot, "closed-world/lilscript.mjs"),
};
const [openNpmCode, openLilCode, closedNpmCode, closedLilCode] =
  await Promise.all([
    bundle(npmPublicEntry, outputs.openNpm),
    bundle(generated.openLil, outputs.openLil),
    bundle(npmAppEntry, outputs.closedNpm),
    bundle(generated.closedAppLil, outputs.closedLil),
  ]);

const [npmPublic, lilPublic, mangledPublic] = await Promise.all([
  importFresh(outputs.openNpm),
  importFresh(outputs.openLil),
  importFresh(generated.closedMangledLil),
]);
assertPublicModule(npmPublic, "motion npm");
assertPublicModule(lilPublic, "LilScript Motion");
assert.deepEqual(publicApiDigest(lilPublic), publicApiDigest(npmPublic));
const mangledNames = Object.keys(mangledPublic).sort();
assert.equal(mangledNames.length, publicNames.length, "closed export count");
assert.equal(
  mangledNames.filter((name) => publicNames.includes(name)).length,
  0,
  "closed-world config must mangle every selected public export",
);

const npmStdout = run(process.execPath, [outputs.closedNpm]);
const lilStdout = run(process.execPath, [outputs.closedLil]);
assert.equal(lilStdout, npmStdout, "closed-world values-core behavior");

const report = {
  schemaVersion: 2,
  generatedAt: new Date().toISOString(),
  toolchain: {
    node: process.version,
    vite: "8.2.1",
    motion: "13.0.0",
    compiler: compilerToolchain.evidence,
    configs: {
      openWorld: openConfiguration.evidence,
      closedWorld: closedConfiguration.evidence,
    },
    codecs,
  },
  openWorld: {
    config: openConfiguration.evidence,
    exports: publicNames,
    apiDigest: publicApiDigest(lilPublic),
    publicApiPassed: true,
    size: sizeComparison(size(openNpmCode), size(openLilCode)),
  },
  closedWorld: {
    config: closedConfiguration.evidence,
    sourceExports: publicNames,
    emittedExports: mangledNames,
    exportsMangled: true,
    behaviorOutput: lilStdout,
    behaviorPassed: true,
    size: sizeComparison(size(closedNpmCode), size(closedLilCode)),
  },
  verification: {
    objective: "brotli11",
    objectiveArtifact: "configured-brotli-vite-artifact",
    configs: {
      openWorld: openConfiguration.evidence.path,
      closedWorld: closedConfiguration.evidence.path,
    },
    matchingArtifactOnly: true,
    crossMetricsAreDiagnostic: ["raw", "gzip9"],
  },
};
assert.equal(
  report.openWorld.size.superior,
  true,
  "open-world Brotli target must win Brotli-11",
);
assert.equal(
  report.closedWorld.size.superior,
  true,
  "closed-world Brotli target must win Brotli-11",
);

writeFileSync(
  resolve(buildRoot, "results.json"),
  `${JSON.stringify(report, null, 2)}\n`,
);
writeFileSync(resolve(buildRoot, "report.html"), htmlReport(report));
console.log(
  `open br ${report.openWorld.size.lil.brotli11}/${report.openWorld.size.npm.brotli11}; ` +
    `closed br ${report.closedWorld.size.lil.brotli11}/${report.closedWorld.size.npm.brotli11}`,
);
console.log(`verified public exports: ${publicNames.join(", ")}`);
console.log(`wrote ${resolve(buildRoot, "results.json")}`);
