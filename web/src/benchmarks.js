import benchmarkData from "./benchmark-results.json";
import browserData from "./browser-results.json";
import comparisonSummaryData from "./comparison-summary.json";
import pairedData from "./paired-results.json";
import "./site.js";

const comparableIds = ["reference", "esbuild", "closure", "hand", "lilscript"];
const number = new Intl.NumberFormat("en-US");

function saving(delta) {
  if (Math.abs(delta) < 0.05) return "tie";
  return `${Math.abs(delta).toFixed(1)}% ${delta < 0 ? "smaller" : "larger"}`;
}

function outcome(stats) {
  return `${stats.wins} ${stats.wins === 1 ? "win" : "wins"}${stats.ties > 0 ? ` · ${stats.ties} ${stats.ties === 1 ? "tie" : "ties"}` : ""}${stats.losses > 0 ? ` · ${stats.losses} ${stats.losses === 1 ? "loss" : "losses"}` : ""}`;
}

function scaleCard({ eyebrow, value, title, body, footnote, tone = "win" }) {
  return `<article class="scale-card ${tone}"><span>${eyebrow}</span><strong>${value}</strong><h3>${title}</h3><p>${body}</p><small>${footnote}</small></article>`;
}

const overallStats = comparisonSummaryData.overall;
const pairedStats = comparisonSummaryData.smallScripts.paired;
const compilerWorkloadStats =
  comparisonSummaryData.smallScripts.compilerWorkloads;
const completePortStats = comparisonSummaryData.packages.completePorts;
const exactEntrypointStats = comparisonSummaryData.packages.exactEntrypoints;
const exactEntrypoints = comparisonSummaryData.packages.exactRows;
const frameworkRuntimeRows = comparisonSummaryData.frameworkRuntime.rows;
const marketplaceComparison = {
  lilscript: comparisonSummaryData.apps.marketplace.candidate,
  baseline: comparisonSummaryData.apps.marketplace.baseline,
};
const solidlil = comparisonSummaryData.apps.solid.candidate;
const solid = comparisonSummaryData.apps.solid.baseline;
const integratedLsx = comparisonSummaryData.apps.solidLsx;
const packageLabels = {
  nanoid: "Nano ID",
  mitt: "mitt",
  clsx: "clsx",
  "gl-matrix": "gl-matrix",
};
const tiedEntrypoints = exactEntrypoints
  .filter((result) => result.candidate === result.baseline)
  .map((result) => packageLabels[result.id] ?? result.id);
const smallerEntrypoints = exactEntrypoints
  .filter((result) => result.candidate < result.baseline)
  .map((result) => packageLabels[result.id] ?? result.id);

document.querySelector("[data-overall-verdict]").innerHTML =
  `<strong>${overallStats.wins} wins, ${overallStats.ties} ties, ${overallStats.losses} losses across ${overallStats.count} verified Brotli comparisons.</strong><p>The average case is ${saving(overallStats.mean)} and the median case is ${saving(overallStats.median)}. Combined bytes are ${saving(overallStats.aggregate)}—but the category breakdown is the honest result.</p>`;

document.querySelector("[data-overview-scores]").innerHTML = [
  [overallStats.count, "verified cases", "Behavior and scope gates passed"],
  [saving(overallStats.mean), "average case", "Each case weighted equally"],
  [
    saving(overallStats.median),
    "median case",
    "The more typical middle result",
  ],
  [
    outcome(overallStats),
    "Brotli outcomes",
    `${saving(overallStats.aggregate)} by combined bytes`,
  ],
]
  .map(
    ([value, label, note]) =>
      `<article><strong>${value}</strong><span>${label}</span><small>${note}</small></article>`,
  )
  .join("");

document.querySelector("[data-small-script-cards]").innerHTML = [
  scaleCard({
    eyebrow: "Six generated scripts",
    value: saving(pairedStats.mean),
    title: `${outcome(pairedStats)} against Closure`,
    body: `Brotli totals are ${number.format(pairedStats.candidateTotal)} B for Lilscript and ${number.format(pairedStats.baselineTotal)} B for Closure: ${saving(pairedStats.aggregate)} in aggregate.`,
    footnote: `All rows also pass the warmed 400-sample median and p95 browser confidence limit of ${browserData.regressionLimit.toFixed(2)}×.`,
  }),
  scaleCard({
    eyebrow: "Five readable workloads",
    value: saving(compilerWorkloadStats.mean),
    title: `${outcome(compilerWorkloadStats)} against Closure`,
    body: `Reactive state, events, binary memory, modules, and motion helpers total ${number.format(compilerWorkloadStats.candidateTotal)} B versus ${number.format(compilerWorkloadStats.baselineTotal)} B Brotli.`,
    footnote: `${saving(compilerWorkloadStats.aggregate)} by combined bytes. Hand-specialized JavaScript remains smaller and is shown as the optimization ceiling.`,
  }),
].join("");

document.querySelector("[data-package-cards]").innerHTML = [
  scaleCard({
    eyebrow: "Six complete selected ports",
    value: saving(completePortStats.mean),
    title: `${outcome(completePortStats)} against the smaller control`,
    body: `Each public surface is compared with the smaller of its npm/Vite and Closure artifacts. Combined bytes are ${saving(completePortStats.aggregate)}; one large geometry package makes the aggregate less dramatic than the equal-case average.`,
    footnote:
      "Every published port passes JavaScript, emitted-C, native, behavior, runtime, and retained-memory gates.",
  }),
  scaleCard({
    eyebrow: "Four exact npm entrypoints",
    value: saving(exactEntrypointStats.mean),
    title: `${outcome(exactEntrypointStats)} against npm/Vite`,
    body: `${tiedEntrypoints.join(" and ")} tie under Brotli; ${smallerEntrypoints.join(" and ")} are smaller. Together the exact entrypoints are ${saving(exactEntrypointStats.aggregate)}.`,
    footnote:
      "This stricter view preserves the selected published ESM names and exact runtime surface, so it is the better expectation for interop-heavy code.",
    tone: "close",
  }),
].join("");

const frameworkLabels = {
  core: "Core browser API",
  store: "Store browser API",
  "web-client": "Client Web API",
  "web-full": "Full Web compatibility API",
};
document.querySelector("[data-framework-runtime-cards]").innerHTML =
  frameworkRuntimeRows
    .map((row) => {
      const difference = (row.candidate / row.baseline - 1) * 100;
      return scaleCard({
        eyebrow: frameworkLabels[row.id] ?? row.id,
        value: saving(difference),
        title: `${number.format(row.candidate)} B vs ${number.format(row.baseline)} B`,
        body: `SolidLil / official Solid under Brotli-11. The exact behavior boundary passes independently of this size result.`,
        footnote:
          row.status === "eligible"
            ? "Exact surface and strict Brotli-objective gate pass."
            : "Exact surface passes; Brotli-objective optimization remains open.",
        tone: difference <= 0 ? "win" : "loss",
      });
    })
    .join("");

const marketplaceBrotliDelta =
  (marketplaceComparison.lilscript.brotli /
    marketplaceComparison.baseline.brotli -
    1) *
  100;
const marketplaceRawDelta =
  (marketplaceComparison.lilscript.raw / marketplaceComparison.baseline.raw -
    1) *
  100;
const solidBrotliDelta = (solidlil.brotli / solid.brotli - 1) * 100;
const solidRawDelta = (solidlil.raw / solid.raw - 1) * 100;
const integratedLsxBrotliDelta = integratedLsx
  ? (integratedLsx.candidate.brotli / integratedLsx.baseline.brotli - 1) * 100
  : null;
const integratedLsxRawDelta = integratedLsx
  ? (integratedLsx.candidate.raw / integratedLsx.baseline.raw - 1) * 100
  : null;
document.querySelector("[data-app-comparisons]").innerHTML = `
  <article class="app-comparison close"><header><span>Historical single-page POC</span><h3>Lastro / Lilastro Parcel Market</h3></header><div class="app-result"><strong>${saving(marketplaceBrotliDelta)}</strong><span>archived Brotli</span></div><p>Both sides implement the same accessible marketplace, CSS, state flow, focus restoration, announcements, and fake checkout. Raw JavaScript is ${saving(marketplaceRawDelta)}; gzip ties at ${number.format(marketplaceComparison.lilscript.gzip)} B.</p><dl><div><dt>Lilastro</dt><dd>${number.format(marketplaceComparison.lilscript.brotli)} Brotli · ${number.format(marketplaceComparison.lilscript.raw)} raw</dd></div><div><dt>Astro</dt><dd>${number.format(marketplaceComparison.baseline.brotli)} Brotli · ${number.format(marketplaceComparison.baseline.raw)} raw</dd></div></dl><small>These external-document bytes predate the canonical scorer and are excluded from the verified aggregate. Rerun exact artifacts before making a current size claim.</small></article>
  ${integratedLsx ? `<article class="app-comparison ${integratedLsxBrotliDelta <= 0 ? "win" : "loss"}"><header><span>Complete client parity fixture</span><h3>SolidLil LSX client surface</h3></header><div class="app-result"><strong>${saving(integratedLsxBrotliDelta)}</strong><span>Brotli</span></div><p>The production candidate is ${saving(integratedLsxRawDelta)} raw and ${saving((integratedLsx.candidate.gzip / integratedLsx.baseline.gzip - 1) * 100)} gzip. The same sources pass differential DOM, keyed identity, branch churn, ErrorBoundary, multi-resource Suspense, events, portals, namespaces, idempotent unmount, stale-handler, and slot-release assertions.</p><dl><div><dt>SolidLil LSX + host ABI</dt><dd>${number.format(integratedLsx.candidate.brotli)} Brotli · ${number.format(integratedLsx.candidate.raw)} raw</dd></div><div><dt>Official Solid JSX</dt><dd>${number.format(integratedLsx.baseline.brotli)} Brotli · ${number.format(integratedLsx.baseline.raw)} raw</dd></div></dl><small>Verdict: complete evidence for all 21 in-scope client-rendering families. Hydration and SSR remain explicit server-coupled exclusions.</small></article>` : `<article class="app-comparison win"><header><span>Archived partial LSX application</span><h3>SolidLil LSX todolist</h3></header><div class="app-result"><strong>${saving(solidBrotliDelta)}</strong><span>Brotli</span></div><p>The served SolidLil app is ${saving(solidRawDelta)} raw and ${saving((solidlil.gzip / solid.gzip - 1) * 100)} gzip. Its archived jsdom proxy is ${((comparisonSummaryData.apps.solid.timeRatio - 1) * 100).toFixed(1)}% slower with ${((comparisonSummaryData.apps.solid.memoryRatio - 1) * 100).toFixed(1)}% more retained heap.</p><dl><div><dt>SolidLil LSX</dt><dd>${number.format(solidlil.brotli)} Brotli · ${number.format(solidlil.raw)} raw</dd></div><div><dt>Solid JSX</dt><dd>${number.format(solid.brotli)} Brotli · ${number.format(solid.raw)} raw</dd></div></dl><small>Verdict: historical app-size evidence only. The current 21/21 client fixture is verified separately; this archived row remains excluded from the current aggregate.</small></article>`}`;

function resultFor(name) {
  const result = benchmarkData.results.find(
    (candidate) => candidate.name === name,
  );
  if (!result) throw new Error(`Missing benchmark result: ${name}`);
  return result;
}

function artifactFor(result, id) {
  const artifact = result.artifacts.find((candidate) => candidate.id === id);
  if (!artifact) throw new Error(`Missing ${id} artifact for ${result.name}`);
  return artifact;
}

function compilerTable(result) {
  const closure = artifactFor(result, "closure");
  const rows = result.artifacts.map((artifact) => {
    const delta = ((artifact.brotli / closure.brotli - 1) * 100).toFixed(1);
    const deltaLabel = `${Number(delta) > 0 ? "+" : ""}${delta}%`;
    const className =
      artifact.id === "lilscript" ? ' class="lilscript-row"' : "";
    return `<tr${className}><th>${artifact.label}</th><td>${number.format(artifact.brotli)}</td><td>${number.format(artifact.gzip)}</td><td>${number.format(artifact.raw)}</td><td>${deltaLabel}</td><td>${artifact.medianMs.toFixed(2)}</td></tr>`;
  });
  return `<table><thead><tr><th>Comparable artifact</th><th>Brotli-11 · primary</th><th>Gzip-9</th><th>Raw</th><th>vs Closure</th><th>Median ms</th></tr></thead><tbody>${rows.join("")}</tbody></table>`;
}

function ecosystemTable(result) {
  if (!result.ecosystem) return "";
  const ecosystem = result.ecosystem;
  return `<div class="ecosystem-context"><p><strong>${ecosystem.label}</strong> is a context-only Vite production build. It uses a different library implementation and is excluded from compiler deltas and totals.</p><code class="benchmark-contract">${ecosystem.expected}</code><div class="benchmark-table-wrap"><table><thead><tr><th>Production assets</th><th>Brotli-11 · primary</th><th>Gzip-9</th><th>Raw</th><th>Median ms</th></tr></thead><tbody><tr><th>${ecosystem.files.join("<br>")}</th><td>${number.format(ecosystem.brotli)}</td><td>${number.format(ecosystem.gzip)}</td><td>${number.format(ecosystem.raw)}</td><td>${ecosystem.medianMs.toFixed(2)}</td></tr></tbody></table></div></div>`;
}

function corpusRows() {
  return comparableIds.map((id) => {
    const artifacts = benchmarkData.results.map((result) =>
      artifactFor(result, id),
    );
    const closureTimes = benchmarkData.results.map(
      (result) => artifactFor(result, "closure").medianMs,
    );
    const totals = artifacts.reduce(
      (sum, artifact) => ({
        raw: sum.raw + artifact.raw,
        gzip: sum.gzip + artifact.gzip,
        brotli: sum.brotli + artifact.brotli,
      }),
      { raw: 0, gzip: 0, brotli: 0 },
    );
    const runtimeRatio = Math.exp(
      artifacts.reduce(
        (sum, artifact, index) =>
          sum + Math.log(artifact.medianMs / closureTimes[index]),
        0,
      ) / artifacts.length,
    );
    return { id, label: artifacts[0].label, ...totals, runtimeRatio };
  });
}

function comparisonPhrase(value, baseline, metric) {
  const difference = Math.abs(value - baseline);
  if (value === baseline) return `ties Closure for ${metric}`;
  return `${number.format(difference)} bytes ${value < baseline ? "smaller" : "larger"} under ${metric}`;
}

const totals = corpusRows();
const closureTotal = totals.find((row) => row.id === "closure");
const lilscriptTotal = totals.find((row) => row.id === "lilscript");
document.querySelector("[data-corpus-summary]").textContent =
  `Across the comparable five-workload corpus, LilScript is ${comparisonPhrase(lilscriptTotal.brotli, closureTotal.brotli, "Brotli")}, ${comparisonPhrase(lilscriptTotal.gzip, closureTotal.gzip, "gzip")}, and ${comparisonPhrase(lilscriptTotal.raw, closureTotal.raw, "raw output")}. Hand-specialized JavaScript remains the optimization oracle.`;

document.querySelector("[data-corpus-table]").innerHTML =
  `<table><thead><tr><th>Comparable artifact</th><th>Brotli-11 · primary</th><th>Gzip-9</th><th>Raw</th><th>Runtime ratio</th></tr></thead><tbody>${totals.map((row) => `<tr${row.id === "lilscript" ? ' class="lilscript-row"' : ""}><th>${row.label}</th><td>${number.format(row.brotli)}</td><td>${number.format(row.gzip)}</td><td>${number.format(row.raw)}</td><td>${row.runtimeRatio.toFixed(3)}x</td></tr>`).join("")}</tbody></table>`;

for (const container of document.querySelectorAll("[data-compiler-table]")) {
  const result = resultFor(container.dataset.compilerTable);
  container.innerHTML = compilerTable(result);
  const ecosystem = document.querySelector(`[data-ecosystem="${result.name}"]`);
  if (ecosystem) ecosystem.innerHTML = ecosystemTable(result);
}

const metadata = benchmarkData.metadata;
document.querySelector("[data-benchmark-method]").textContent =
  `Measured on ${metadata.generatedAt.slice(0, 10)} from compiler revision ${metadata.compilerRevision} with Node ${metadata.node}, Vite ${metadata.vite}, esbuild ${metadata.esbuild}, and Closure Compiler ${metadata.closure}. Runtime is the median of ${metadata.samples} cache-busted module evaluations in one dedicated process per artifact after ${metadata.warmups} warmups; process startup is excluded.`;

const pairedRows = pairedData.results.map((result) => {
  const runtime = browserData.results.find(
    (candidate) => candidate.id === result.id,
  );
  if (!runtime) throw new Error(`Missing browser runtime result: ${result.id}`);
  const confidence = Number.isFinite(runtime.p95Upper95Ratio)
    ? `${runtime.upper95Ratio.toFixed(3)}x / ${runtime.p95Upper95Ratio.toFixed(3)}x`
    : `${runtime.upper95Ratio.toFixed(3)}x / pending refresh`;
  return `<tr><th>${result.id}</th><td>${number.format(result.lilscript.brotli)} / ${number.format(result.closure.brotli)}</td><td>${number.format(result.lilscript.gzip)} / ${number.format(result.closure.gzip)}</td><td>${number.format(result.lilscript.raw)} / ${number.format(result.closure.raw)}</td><td>${confidence}</td><td><code>${result.contract.replaceAll("\n", " / ")}</code></td></tr>`;
});
document.querySelector("[data-paired-table]").innerHTML =
  `<table><thead><tr><th>Generated workload</th><th>Brotli L/C · primary</th><th>Gzip L/C</th><th>Raw L/C</th><th>Median / p95 upper 95%</th><th>Contract</th></tr></thead><tbody>${pairedRows.join("")}</tbody></table>`;
const browserEvidenceCurrent =
  browserData.schemaVersion === 2 &&
  browserData.results.every((result) =>
    Number.isFinite(result.p95Upper95Ratio),
  );
document.querySelector("[data-paired-method]").textContent =
  browserEvidenceCurrent
    ? `Generated ${pairedData.generatedAt.slice(0, 10)} from one neutral workload schema. Every row passed Closure JavaScript, LilScript JavaScript, emitted C, and native execution before its per-case ${pairedData.costModel ?? "brotli"} size gate; raw and gzip remain visible. ${browserData.browser} then measured ${browserData.results[0]?.samples ?? 0} warmed alternating samples; median and p95 runtime distributions each require a bootstrap upper confidence ratio at or below ${browserData.regressionLimit.toFixed(2)}x.`
    : "The checked browser snapshot predates the current 400-sample median-and-p95 contract. Its legacy median column is shown for context, while p95 evidence is explicitly pending a fresh canonical paired/browser run and is excluded from current release claims.";
