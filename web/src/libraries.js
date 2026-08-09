import libraryData from "./library-results.json";
import clientRuntime from "./client-runtime-results.json";
import popularData from "./popular-library-results.json";
import { renderIcons } from "./site.js";

const number = new Intl.NumberFormat("en-US");

function escape(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function delta(value, baseline) {
  const percentage = (value / baseline - 1) * 100;
  return `${percentage > 0 ? "+" : ""}${percentage.toFixed(1)}%`;
}

function artifactTable(result) {
  const vite = result.surfaceArtifacts.find((artifact) => artifact.id === "vite");
  const rows = result.surfaceArtifacts.map((artifact) => {
    const className = artifact.id === "lilscript" ? ' class="lilscript-row"' : "";
    return `<tr${className}><th>${escape(artifact.label)}</th><td>${number.format(artifact.raw)}</td><td>${number.format(artifact.gzip)}</td><td>${number.format(artifact.brotli)}</td><td>${delta(artifact.brotli, vite.brotli)}</td></tr>`;
  });
  return `<table><thead><tr><th>Reusable selected API</th><th>Raw</th><th>Gzip-9</th><th>Brotli-11</th><th>vs npm/Vite</th></tr></thead><tbody>${rows.join("")}</tbody></table>`;
}

function demoTable(result) {
  const rows = result.artifacts.map((artifact) => {
    const className = artifact.id === "lilscript" ? ' class="lilscript-row"' : "";
    return `<tr${className}><th>${escape(artifact.label)}</th><td>${number.format(artifact.raw)}</td><td>${number.format(artifact.gzip)}</td><td>${number.format(artifact.brotli)}</td><td>${artifact.medianMs.toFixed(2)}</td></tr>`;
  });
  return `<table><thead><tr><th>Checked demo app</th><th>Raw</th><th>Gzip-9</th><th>Brotli-11</th><th>Median ms</th></tr></thead><tbody>${rows.join("")}</tbody></table>`;
}

function deployTable(result) {
  const rows = result.artifacts.map((artifact) => {
    const className = artifact.id === "lilscript" ? ' class="lilscript-row"' : "";
    return `<tr${className}><th>${escape(artifact.label)}</th><td>${number.format(artifact.deploy.raw)}</td><td>${number.format(artifact.deploy.gzip)}</td><td>${number.format(artifact.deploy.brotli)}</td></tr>`;
  });
  return `<table><thead><tr><th>HTML + JavaScript</th><th>Raw</th><th>Gzip-9</th><th>Brotli-11</th></tr></thead><tbody>${rows.join("")}</tbody></table>`;
}

function sizeTriplet(size) {
  if (!size || size.raw === "—") return "—";
  return `${number.format(size.raw)} / ${number.format(size.gzip)} / ${number.format(size.brotli)}`;
}

function ratio(value) {
  return value == null ? "—" : `${value.toFixed(3)}×`;
}

function popularStatus(result) {
  if (result.eligible) return "Eligible exact entrypoint";
  if (result.status === "blocked-adapter-algorithm") {
    return "Behavior match only; algorithm blocked";
  }
  if (result.sizeGate === false) return "Exact entrypoint; size gate blocked";
  if (result.status === "behavior-exact-performance-blocked") {
    return "Behavior exact; performance blocked";
  }
  return "Exact entrypoint; resource gate blocked";
}

function popularRow(result) {
  const performance = result.performance;
  const className = result.eligible ? ' class="lilscript-row"' : "";
  const closure = result.closureLevel
    ? `${escape(result.closureLevel)}: ${sizeTriplet(result.closure)}`
    : "—";
  const boundary = result.blockers?.length
    ? result.blockers.join(" ")
    : result.compatibilityNotes;
  return `<tr${className}><th>${escape(result.project)}<small class="table-note">${escape(popularStatus(result))}</small></th><td>${sizeTriplet(result.rawJs)}</td><td>${sizeTriplet(result.terser)}</td><td>${closure}</td><td>${sizeTriplet(result.vite)}</td><td>${sizeTriplet(result.lilscript)}</td><td>${sizeTriplet(result.lilscriptVite)}</td><td>${result.vite?.brotli && result.lilscriptVite?.brotli ? `${number.format(result.lilscriptVite.brotli)} / ${number.format(result.vite.brotli)} (${delta(result.lilscriptVite.brotli, result.vite.brotli)})` : "—"}</td><td>${ratio(performance?.performance.ratio)}</td><td>${ratio(performance?.retainedMemory.ratio)}</td><td>${escape(boundary)}</td></tr>`;
}

function popularTable(results) {
  return `<table class="popular-matrix"><thead><tr><th>Project</th><th>Raw JS</th><th>Terser</th><th>Closure (actual level)</th><th>npm + Vite 8</th><th>LilScript compiler</th><th>LilScript + Vite 8</th><th>Brotli Lil / npm</th><th>Time Lil / npm</th><th>Memory Lil / npm</th><th>Compatibility boundary</th></tr></thead><tbody>${results.map(popularRow).join("")}</tbody></table>`;
}

const wins = libraryData.results.filter((result) => {
  const vite = result.surfaceArtifacts.find((artifact) => artifact.id === "vite");
  const lilscript = result.surfaceArtifacts.find((artifact) => artifact.id === "lilscript");
  return lilscript.brotli < vite.brotli;
}).length;
document.querySelector("[data-library-summary]").textContent =
  `LilScript produces the smaller reusable-API Brotli payload in ${wins} of ${libraryData.results.length} eligible complete ports. Eligibility also requires non-larger raw and configured-codec output than Closure ADVANCED plus the runtime and retained-memory gates.`;

const eligiblePopular = popularData.results.filter((result) => result.eligible);
const exactPopular = popularData.results.filter((result) => result.exactSurface);
const blockedPopular = exactPopular.filter((result) => !result.eligible);
document.querySelector("[data-popular-summary]").textContent =
  `${eligiblePopular.length} of ${exactPopular.length} exact selected entrypoints currently clear behavior and algorithm parity, non-larger raw/selected-codec output than both npm/Vite 8 and public-API-preserving Closure ADVANCED, and ≤${((popularData.metadata.materialRegressionLimit - 1) * 100).toFixed(0)}% time/retained-memory gates. Gzip and Brotli are both reported even when only the configured transport codec is the optimization objective. The other requested libraries stay off this comparison until their selected entrypoints are complete.`;
document.querySelector("[data-popular-eligible]").innerHTML = eligiblePopular.length
  ? popularTable(eligiblePopular)
  : "<p>No requested package currently clears every publication gate.</p>";
document.querySelector("[data-popular-blocked]").innerHTML = blockedPopular.length
  ? popularTable(blockedPopular)
  : "<p>No exact entrypoint is currently blocked.</p>";
document.querySelector("[data-popular-method]").textContent =
  `Generated ${popularData.metadata.generatedAt.slice(0, 10)} from compiler revision ${popularData.metadata.compilerRevision} with Node ${popularData.metadata.node}, Vite ${popularData.metadata.vite}, Terser ${popularData.metadata.terser}, and Closure Compiler ${popularData.metadata.closure}. Size cells are raw / gzip-9 / Brotli-11 bytes; Closure's actual compilation level is shown per row.`;

const runtimeRows = clientRuntime.sizes.map((artifact) => {
  const className = artifact.id.startsWith("solidlil") ? ' class="lilscript-row"' : "";
  return `<tr${className}><th>${escape(artifact.label)}</th><td>${number.format(artifact.raw)}</td><td>${number.format(artifact.gzip)}</td><td>${number.format(artifact.brotli)}</td></tr>`;
});
const port = clientRuntime.port;
const upstream = clientRuntime.upstream;
document.querySelector("[data-client-runtime]").innerHTML = `<section class="benchmark-project client-runtime" id="${escape(clientRuntime.id)}"><header><p class="eyebrow">Partial client-runtime implementation</p><h2>${escape(clientRuntime.title)}</h2><p>${number.format(port.sourceLines)} lines of LilScript implement an executable reactive, flow, component, and DOM slice. ${number.format(port.adaptedCasesPassed)} adapted behaviors pass in ${number.format(port.executions)} optimized/unoptimized JavaScript, emitted-C, and native executions. The unchanged <code>${escape(upstream.package)}@${escape(upstream.version)}</code> reference remains ${number.format(upstream.testsPassed)}/${number.format(upstream.testsTotal)} across ${number.format(upstream.files)} files.</p><p class="comparison-note"><strong>Not full Solid compatibility.</strong> Only ${number.format(port.adaptedCasesPassed)} of the ${number.format(port.adaptedCasesTotal)} target behaviors have LilScript-backed ports. Missing areas include ${clientRuntime.notPorted.map(escape).join(", ")}.</p><div class="benchmark-links"><a class="secondary-link" href="${escape(clientRuntime.sourceRepository)}"><i data-lucide="external-link" aria-hidden="true"></i>Open reproducible lab</a></div></header><div class="benchmark-table-wrap"><table><thead><tr><th>Client app artifact</th><th>Raw</th><th>Gzip-9</th><th>Brotli-11</th></tr></thead><tbody>${runtimeRows.join("")}</tbody></table></div><details class="deploy-details"><summary>Interaction and retained-heap sample</summary><div class="benchmark-table-wrap"><table><thead><tr><th>Build</th><th>Median ms</th><th>Time ratio</th><th>Retained bytes</th><th>Memory ratio</th></tr></thead><tbody><tr><th>Official Solid JSX</th><td>${clientRuntime.runtime.solidMedianMs.toFixed(2)}</td><td>1.000×</td><td>${number.format(clientRuntime.runtime.solidRetainedBytes)}</td><td>1.000×</td></tr><tr class="lilscript-row"><th>solidlil LSX</th><td>${clientRuntime.runtime.lsxMedianMs.toFixed(2)}</td><td>${clientRuntime.runtime.lsxTimeRatio.toFixed(3)}×</td><td>${number.format(clientRuntime.runtime.lsxRetainedBytes)}</td><td>${clientRuntime.runtime.lsxMemoryRatio.toFixed(3)}×</td></tr><tr class="lilscript-row"><th>Identical-JSX solidlil</th><td>${clientRuntime.runtime.babelMedianMs.toFixed(2)}</td><td>${clientRuntime.runtime.babelTimeRatio.toFixed(3)}×</td><td>${number.format(clientRuntime.runtime.babelRetainedBytes)}</td><td>${clientRuntime.runtime.babelMemoryRatio.toFixed(3)}×</td></tr></tbody></table></div><p>${escape(clientRuntime.runtime.environment)}; ${number.format(clientRuntime.runtime.samples)} time samples and ${number.format(clientRuntime.runtime.memorySamples)} isolated retained-heap samples. This is a Node/jsdom regression proxy, not a browser-performance claim.</p></details></section>`;

document.querySelector("[data-library-results]").innerHTML = libraryData.results.map((result) => {
  const packages = result.packages.map((item) => `${item.name}@${item.version}`).join(" + ");
  return `<section class="benchmark-project" id="${escape(result.id)}"><header><p class="eyebrow">${escape(result.scope)}</p><h2>${escape(result.title)}</h2><p><code>${escape(packages)}</code> with ${number.format(result.monthlyDownloads)} monthly downloads at selection time. ${number.format(result.translatedAssertions)} translated upstream assertions and ${number.format(result.additionalAssertions ?? 0)} added contract assertions precede dense differential API tests.</p><code class="benchmark-contract">${escape(result.expected)}</code></header><div class="benchmark-table-wrap">${artifactTable(result)}</div><details class="deploy-details"><summary>Checked demo-app diagnostics</summary><div class="benchmark-table-wrap">${demoTable(result)}</div></details><details class="deploy-details"><summary>Full demo deploy size</summary><div class="benchmark-table-wrap">${deployTable(result)}</div></details></section>`;
}).join("");

document.querySelector("[data-ineligible-table]").innerHTML = `<table><thead><tr><th>Package</th><th>Version</th><th>Current blocker</th></tr></thead><tbody>${libraryData.auditedButIneligible.map((item) => `<tr><th>${escape(item.package)}</th><td>${escape(item.version)}</td><td>${escape(item.reason)}</td></tr>`).join("")}</tbody></table>`;

const metadata = libraryData.metadata;
document.querySelector("[data-library-method]").textContent =
  `Generated ${metadata.generatedAt.slice(0, 10)} from compiler revision ${metadata.compilerRevision} with Node ${metadata.node}, Vite ${metadata.vite}, esbuild ${metadata.esbuild}, and Closure Compiler ${metadata.closure}. Size eligibility uses reusable selected APIs; demo-app specialization is diagnostic only. Each LilScript row passed JavaScript, emitted-C, and native execution before publication.`;

renderIcons(document.querySelector("[data-client-runtime]"));
