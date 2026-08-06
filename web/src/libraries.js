import libraryData from "./library-results.json";
import "./site.js";

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

document.querySelector("[data-library-results]").innerHTML = libraryData.results.map((result) => {
  const packages = result.packages.map((item) => `${item.name}@${item.version}`).join(" + ");
  return `<section class="benchmark-project" id="${escape(result.id)}"><header><p class="eyebrow">${escape(result.scope)}</p><h2>${escape(result.title)}</h2><p><code>${escape(packages)}</code> with ${number.format(result.monthlyDownloads)} monthly downloads at selection time. ${number.format(result.translatedAssertions)} translated upstream assertions and ${number.format(result.additionalAssertions ?? 0)} added contract assertions precede dense differential API tests.</p><code class="benchmark-contract">${escape(result.expected)}</code></header><div class="benchmark-table-wrap">${artifactTable(result)}</div><details class="deploy-details"><summary>Full deploy size</summary><div class="benchmark-table-wrap">${deployTable(result)}</div></details></section>`;
}).join("");

document.querySelector("[data-ineligible-table]").innerHTML = `<table><thead><tr><th>Package</th><th>Version</th><th>Current blocker</th></tr></thead><tbody>${libraryData.auditedButIneligible.map((item) => `<tr><th>${escape(item.package)}</th><td>${escape(item.version)}</td><td>${escape(item.reason)}</td></tr>`).join("")}</tbody></table>`;

const metadata = libraryData.metadata;
document.querySelector("[data-library-method]").textContent =
  `Generated ${metadata.generatedAt.slice(0, 10)} from compiler revision ${metadata.compilerRevision} with Node ${metadata.node}, Vite ${metadata.vite}, esbuild ${metadata.esbuild}, and Closure Compiler ${metadata.closure}. Each LilScript row passed JavaScript, emitted-C, and native execution before publication.`;
