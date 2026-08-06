import libraryData from "./library-results.json";
import clientRuntime from "./client-runtime-results.json";
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
  const vite = result.artifacts.find((artifact) => artifact.id === "vite");
  const rows = result.artifacts.map((artifact) => {
    const className = artifact.id === "lilscript" ? ' class="lilscript-row"' : "";
    return `<tr${className}><th>${escape(artifact.label)}</th><td>${number.format(artifact.raw)}</td><td>${number.format(artifact.gzip)}</td><td>${number.format(artifact.brotli)}</td><td>${delta(artifact.brotli, vite.brotli)}</td><td>${artifact.medianMs.toFixed(2)}</td></tr>`;
  });
  return `<table><thead><tr><th>Deployable JavaScript</th><th>Raw</th><th>Gzip-9</th><th>Brotli-11</th><th>vs npm/Vite</th><th>Median ms</th></tr></thead><tbody>${rows.join("")}</tbody></table>`;
}

function deployTable(result) {
  const rows = result.artifacts.map((artifact) => {
    const className = artifact.id === "lilscript" ? ' class="lilscript-row"' : "";
    return `<tr${className}><th>${escape(artifact.label)}</th><td>${number.format(artifact.deploy.raw)}</td><td>${number.format(artifact.deploy.gzip)}</td><td>${number.format(artifact.deploy.brotli)}</td></tr>`;
  });
  return `<table><thead><tr><th>HTML + JavaScript</th><th>Raw</th><th>Gzip-9</th><th>Brotli-11</th></tr></thead><tbody>${rows.join("")}</tbody></table>`;
}

const wins = libraryData.results.filter((result) => {
  const vite = result.artifacts.find((artifact) => artifact.id === "vite");
  const lilscript = result.artifacts.find((artifact) => artifact.id === "lilscript");
  return lilscript.brotli < vite.brotli;
}).length;
document.querySelector("[data-library-summary]").textContent =
  `LilScript produces the smaller Brotli JavaScript payload in ${wins} of ${libraryData.results.length} complete-port apps. Motion easing and Emotion hash remain larger, so this page makes no universal size claim.`;

const runtimeRows = clientRuntime.sizes.map((artifact) => {
  const className = artifact.id === "lilscript-compiler" ? ' class="lilscript-row"' : "";
  return `<tr${className}><th>${escape(artifact.label)}</th><td>${number.format(artifact.raw)}</td><td>${number.format(artifact.gzip)}</td><td>${number.format(artifact.brotli)}</td></tr>`;
});
const port = clientRuntime.port;
const upstream = clientRuntime.upstream;
document.querySelector("[data-client-runtime]").innerHTML = `<section class="benchmark-project client-runtime" id="${escape(clientRuntime.id)}"><header><p class="eyebrow">Partial client-runtime implementation</p><h2>${escape(clientRuntime.title)}</h2><p>${number.format(port.sourceLines)} lines of LilScript implement an executable reactive, flow, component, and DOM slice. ${number.format(port.adaptedCasesPassed)} adapted behaviors pass in ${number.format(port.executions)} optimized/unoptimized JavaScript, emitted-C, and native executions. The unchanged <code>${escape(upstream.package)}@${escape(upstream.version)}</code> reference remains ${number.format(upstream.testsPassed)}/${number.format(upstream.testsTotal)} across ${number.format(upstream.files)} files.</p><p class="comparison-note"><strong>Not full Solid compatibility.</strong> Only ${number.format(port.adaptedCasesPassed)} of the ${number.format(port.adaptedCasesTotal)} target behaviors have LilScript-backed ports. Missing areas include ${clientRuntime.notPorted.map(escape).join(", ")}.</p><div class="benchmark-links"><a class="secondary-link" href="${escape(clientRuntime.sourceRepository)}"><i data-lucide="external-link" aria-hidden="true"></i>Open reproducible lab</a></div></header><div class="benchmark-table-wrap"><table><thead><tr><th>Client app artifact</th><th>Raw</th><th>Gzip-9</th><th>Brotli-11</th></tr></thead><tbody>${runtimeRows.join("")}</tbody></table></div><details class="deploy-details"><summary>Runtime sample</summary><div class="benchmark-table-wrap"><table><thead><tr><th>Build</th><th>Median ms</th><th>Environment</th></tr></thead><tbody><tr><th>Official Solid + Vite</th><td>${clientRuntime.runtime.solidViteMedianMs.toFixed(2)}</td><td rowspan="4">${escape(clientRuntime.runtime.environment)}; ${number.format(clientRuntime.runtime.samples)} samples</td></tr><tr class="lilscript-row"><th>Partial LilScript runtime + Vite</th><td>${clientRuntime.runtime.lilscriptViteMedianMs.toFixed(2)}</td></tr><tr><th>Official Solid + Closure ADVANCED</th><td>${clientRuntime.runtime.solidClosureMedianMs.toFixed(2)}</td></tr><tr class="lilscript-row"><th>Partial LilScript runtime + Closure ADVANCED</th><td>${clientRuntime.runtime.lilscriptClosureMedianMs.toFixed(2)}</td></tr></tbody></table></div></details></section>`;

document.querySelector("[data-library-results]").innerHTML = libraryData.results.map((result) => {
  const packages = result.packages.map((item) => `${item.name}@${item.version}`).join(" + ");
  return `<section class="benchmark-project" id="${escape(result.id)}"><header><p class="eyebrow">${escape(result.scope)}</p><h2>${escape(result.title)}</h2><p><code>${escape(packages)}</code> with ${number.format(result.monthlyDownloads)} monthly downloads at selection time. ${number.format(result.translatedAssertions)} translated upstream assertions and ${number.format(result.additionalAssertions ?? 0)} added contract assertions precede dense differential API tests.</p><code class="benchmark-contract">${escape(result.expected)}</code></header><div class="benchmark-table-wrap">${artifactTable(result)}</div><details class="deploy-details"><summary>Full deploy size</summary><div class="benchmark-table-wrap">${deployTable(result)}</div></details></section>`;
}).join("");

document.querySelector("[data-ineligible-table]").innerHTML = `<table><thead><tr><th>Package</th><th>Version</th><th>Current blocker</th></tr></thead><tbody>${libraryData.auditedButIneligible.map((item) => `<tr><th>${escape(item.package)}</th><td>${escape(item.version)}</td><td>${escape(item.reason)}</td></tr>`).join("")}</tbody></table>`;

const metadata = libraryData.metadata;
document.querySelector("[data-library-method]").textContent =
  `Generated ${metadata.generatedAt.slice(0, 10)} from compiler revision ${metadata.compilerRevision} with Node ${metadata.node}, Vite ${metadata.vite}, esbuild ${metadata.esbuild}, and Closure Compiler ${metadata.closure}. Each LilScript row passed JavaScript, emitted-C, and native execution before publication.`;

renderIcons(document.querySelector("[data-client-runtime]"));
