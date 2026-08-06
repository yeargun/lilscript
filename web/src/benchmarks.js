import benchmarkData from "./benchmark-results.json";
import "./site.js";

const comparableIds = ["reference", "esbuild", "closure", "hand", "lilscript"];
const number = new Intl.NumberFormat("en-US");

function resultFor(name) {
  const result = benchmarkData.results.find((candidate) => candidate.name === name);
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
    const className = artifact.id === "lilscript" ? ' class="lilscript-row"' : "";
    return `<tr${className}><th>${artifact.label}</th><td>${number.format(artifact.raw)}</td><td>${number.format(artifact.gzip)}</td><td>${number.format(artifact.brotli)}</td><td>${deltaLabel}</td><td>${artifact.medianMs.toFixed(2)}</td></tr>`;
  });
  return `<table><thead><tr><th>Comparable artifact</th><th>Raw</th><th>Gzip-9</th><th>Brotli-11</th><th>vs Closure</th><th>Median ms</th></tr></thead><tbody>${rows.join("")}</tbody></table>`;
}

function ecosystemTable(result) {
  if (!result.ecosystem) return "";
  const ecosystem = result.ecosystem;
  return `<div class="ecosystem-context"><p><strong>${ecosystem.label}</strong> is a context-only Vite production build. It uses a different library implementation and is excluded from compiler deltas and totals.</p><code class="benchmark-contract">${ecosystem.expected}</code><div class="benchmark-table-wrap"><table><thead><tr><th>Production assets</th><th>Raw</th><th>Gzip-9</th><th>Brotli-11</th><th>Median ms</th></tr></thead><tbody><tr><th>${ecosystem.files.join("<br>")}</th><td>${number.format(ecosystem.raw)}</td><td>${number.format(ecosystem.gzip)}</td><td>${number.format(ecosystem.brotli)}</td><td>${ecosystem.medianMs.toFixed(2)}</td></tr></tbody></table></div></div>`;
}

function corpusRows() {
  return comparableIds.map((id) => {
    const artifacts = benchmarkData.results.map((result) => artifactFor(result, id));
    const closureTimes = benchmarkData.results.map((result) => artifactFor(result, "closure").medianMs);
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
        (sum, artifact, index) => sum + Math.log(artifact.medianMs / closureTimes[index]),
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
  `Across the comparable five-workload corpus, LilScript is ${comparisonPhrase(lilscriptTotal.raw, closureTotal.raw, "raw output")}, ${comparisonPhrase(lilscriptTotal.gzip, closureTotal.gzip, "gzip")}, and ${comparisonPhrase(lilscriptTotal.brotli, closureTotal.brotli, "Brotli")}. Hand-specialized JavaScript remains the optimization oracle.`;

document.querySelector("[data-corpus-table]").innerHTML = `<table><thead><tr><th>Comparable artifact</th><th>Raw</th><th>Gzip-9</th><th>Brotli-11</th><th>Runtime ratio</th></tr></thead><tbody>${totals.map((row) => `<tr${row.id === "lilscript" ? ' class="lilscript-row"' : ""}><th>${row.label}</th><td>${number.format(row.raw)}</td><td>${number.format(row.gzip)}</td><td>${number.format(row.brotli)}</td><td>${row.runtimeRatio.toFixed(3)}x</td></tr>`).join("")}</tbody></table>`;

for (const container of document.querySelectorAll("[data-compiler-table]")) {
  const result = resultFor(container.dataset.compilerTable);
  container.innerHTML = compilerTable(result);
  const ecosystem = document.querySelector(`[data-ecosystem="${result.name}"]`);
  if (ecosystem) ecosystem.innerHTML = ecosystemTable(result);
}

const metadata = benchmarkData.metadata;
document.querySelector("[data-benchmark-method]").textContent =
  `Measured on ${metadata.generatedAt.slice(0, 10)} from compiler revision ${metadata.compilerRevision} with Node ${metadata.node}, Vite ${metadata.vite}, esbuild ${metadata.esbuild}, and Closure Compiler ${metadata.closure}. Runtime is the median of ${metadata.samples} cache-busted module evaluations in one dedicated process per artifact after ${metadata.warmups} warmups; process startup is excluded.`;
