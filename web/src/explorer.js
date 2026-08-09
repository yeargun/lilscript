import catalog from "./benchmark-catalog.json";
import { formatBytes, percentageSaved, summarizeArtifacts } from "./benchmark-metrics.js";
import { renderIcons } from "./site.js";

const number = new Intl.NumberFormat("en-US");
const labels = {
  "real-app": "Real application",
  mangling: "Mangling stress",
  "compiler-app": "Compiler application",
  "complete-library": "Complete npm API",
  "popular-library": "Popular package audit",
  "generated-pair": "Generated source pair",
};
const escape = (value) => String(value ?? "")
  .replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
const percent = (value) => `${value.toFixed(1)}%`;
const rateLabel = (baseline, result) => {
  const rate = percentageSaved(baseline, result);
  return rate >= 0 ? `${percent(rate)} smaller` : `${percent(Math.abs(rate))} larger`;
};
const rows = catalog.projects.flatMap((project, projectIndex) => project.artifacts.map((artifact, artifactIndex) => ({ project, artifact, projectIndex, artifactIndex })));
const controls = {
  search: document.querySelector("[data-filter-search]"),
  category: document.querySelector("[data-filter-category]"),
  status: document.querySelector("[data-filter-status]"),
  tool: document.querySelector("[data-filter-tool]"),
  mode: document.querySelector("[data-filter-mode]"),
  view: document.querySelector("[data-column-view]"),
  sort: document.querySelector("[data-sort]"),
  direction: document.querySelector("[data-sort-direction]"),
};
let ascending = true;

function option(value, label = value) {
  return `<option value="${escape(value)}">${escape(label)}</option>`;
}
function values(getter) {
  return [...new Set(rows.map(getter).filter(Boolean))].sort((a, b) => a.localeCompare(b));
}
controls.category.insertAdjacentHTML("beforeend", values(({ project }) => project.category).map((value) => option(value, labels[value] ?? value)).join(""));
controls.status.insertAdjacentHTML("beforeend", values(({ project }) => project.status).map((value) => option(value)).join(""));
controls.tool.insertAdjacentHTML("beforeend", values(({ artifact }) => artifact.tool).map((value) => option(value)).join(""));
controls.mode.insertAdjacentHTML("beforeend", values(({ artifact }) => artifact.mode).map((value) => option(value)).join(""));

const eligible = catalog.projects.filter((project) => project.status === "eligible").length;
document.querySelector("[data-catalog-summary]").innerHTML = [
  [catalog.metadata.projectCount, "projects"],
  [catalog.metadata.artifactCount, "artifact lanes"],
  [eligible, "eligible projects"],
  [catalog.metadata.versions.vite, "Vite version"],
].map(([value, label]) => `<article><strong>${escape(value)}</strong><span>${escape(label)}</span></article>`).join("");
document.querySelector("[data-definitions]").innerHTML = Object.entries(catalog.definitions)
  .map(([term, definition]) => `<article><h3>${escape(term)}</h3><p>${escape(definition)}</p></article>`).join("");

function haystack(row) {
  return [row.project.title, row.project.id, row.project.summary, row.project.category, row.project.status, row.artifact.label, row.artifact.tool, ...row.project.packages.map((item) => item.name)].join(" ").toLowerCase();
}
function compare(left, right) {
  const key = controls.sort.value;
  let a;
  let b;
  if (key === "core") {
    return (ascending ? 1 : -1) * ((left.projectIndex - right.projectIndex) || (left.artifactIndex - right.artifactIndex));
  } else if (["raw", "gzip", "brotli"].includes(key)) {
    a = left.artifact[key]; b = right.artifact[key];
  } else if (key === "gzip-rate") {
    a = percentageSaved(left.artifact.raw, left.artifact.gzip); b = percentageSaved(right.artifact.raw, right.artifact.gzip);
  } else if (key === "brotli-rate") {
    a = percentageSaved(left.artifact.raw, left.artifact.brotli); b = percentageSaved(right.artifact.raw, right.artifact.brotli);
  } else if (key === "tool") {
    a = left.artifact.tool; b = right.artifact.tool;
  } else if (key === "category") {
    a = left.project.category; b = right.project.category;
  } else {
    a = left.project.title; b = right.project.title;
  }
  const order = typeof a === "number" ? a - b : String(a).localeCompare(String(b));
  return (ascending ? 1 : -1) * (order || left.artifact.label.localeCompare(right.artifact.label));
}
function renderAggregates(filtered) {
  const summary = summarizeArtifacts(filtered.map(({ artifact }) => artifact));
  const cards = summary ? [
    [formatBytes(summary.meanRaw), "mean raw output", "Arithmetic mean before transport compression"],
    [percent(summary.meanGzipReduction), "average saved by gzip-9", "Mean of each lane's raw-to-gzip reduction"],
    [percent(summary.meanBrotliReduction), "average saved by Brotli-11", "Mean of each lane's raw-to-Brotli reduction"],
    [percent(summary.meanBrotliEdge), "average Brotli edge", "Mean reduction from gzip bytes to Brotli bytes"],
  ] : [
    ["—", "mean raw output", "No matching artifacts"],
    ["—", "average saved by gzip-9", "No matching artifacts"],
    ["—", "average saved by Brotli-11", "No matching artifacts"],
    ["—", "average Brotli edge", "No matching artifacts"],
  ];
  document.querySelector("[data-aggregate-summary]").innerHTML = cards
    .map(([value, label, detail]) => `<article><strong>${escape(value)}</strong><span>${escape(label)}</span><small>${escape(detail)}</small></article>`).join("");
  document.querySelector("[data-aggregate-note]").innerHTML = summary
    ? `<strong>Two honest summaries:</strong> equal-row averages give every visible lane one vote. Byte-weighted totals let larger bundles count more; across this selection, gzip saves <strong>${percent(summary.weightedGzipReduction)}</strong> and Brotli saves <strong>${percent(summary.weightedBrotliReduction)}</strong>. Neither is a cross-project compiler score.`
    : "No artifacts match the current filters, so no average is calculated.";
}
function render() {
  document.querySelector(".explorer-main").classList.toggle("show-all-columns", controls.view.value === "full");
  const query = controls.search.value.trim().toLowerCase();
  const filtered = rows.filter((row) =>
    (!query || haystack(row).includes(query)) &&
    (!controls.category.value || row.project.category === controls.category.value) &&
    (!controls.status.value || row.project.status === controls.status.value) &&
    (!controls.tool.value || row.artifact.tool === controls.tool.value) &&
    (!controls.mode.value || row.artifact.mode === controls.mode.value)
  ).sort(compare);
  renderAggregates(filtered);
  document.querySelector("[data-result-count]").textContent = `${number.format(filtered.length)} of ${number.format(rows.length)} artifact rows across ${new Set(filtered.map((row) => row.project.key)).size} projects`;
  document.querySelector("[data-explorer-rows]").innerHTML = filtered.map(({ project, artifact }) => `
    <tr>
      <th class="project-cell"><a class="project-link" target="_blank" rel="noopener" href="/benchmark-detail.html?project=${encodeURIComponent(project.key)}">${escape(project.title)}<i data-lucide="external-link" aria-hidden="true"></i></a><span class="project-meta"><span>${escape(labels[project.category] ?? project.category)}</span><span class="status-badge ${escape(project.status)}">${escape(project.status)}</span></span><small>${escape(project.packages.map((item) => item.name).join(" + ") || project.id)}</small></th>
      <td class="optional-column optional-cell" data-label="Category">${escape(labels[project.category] ?? project.category)}</td><td class="optional-column optional-cell" data-label="Status"><span class="status-badge ${escape(project.status)}">${escape(project.status)}</span></td>
      <td class="artifact-cell" data-label="Artifact">${escape(artifact.label)}<small>${escape(artifact.tool)}</small></td><td class="optional-column optional-cell" data-label="Tool">${escape(artifact.tool)}</td><td class="mangling-cell" data-label="Mangling">${escape(artifact.mode)}<small>properties: ${escape(artifact.propertyMangling)}</small></td><td class="optional-column optional-cell" data-label="Properties">${escape(artifact.propertyMangling)}</td>
      <td class="numeric metric-cell" data-label="Raw"><span class="metric-value">${number.format(artifact.raw)}</span><small class="metric-rate">baseline</small></td><td class="numeric metric-cell" data-label="Gzip-9"><span class="metric-value">${number.format(artifact.gzip)}</span><small class="metric-rate">${rateLabel(artifact.raw, artifact.gzip)}</small></td><td class="numeric metric-cell" data-label="Brotli-11"><span class="metric-value">${number.format(artifact.brotli)}</span><small class="metric-rate">${rateLabel(artifact.raw, artifact.brotli)}</small></td>
    </tr>`).join("") || '<tr><td colspan="10" class="empty-table">No artifact matches these filters.</td></tr>';
  renderIcons(document.querySelector("[data-explorer-rows]"));
}
for (const control of Object.values(controls).filter((control) => control !== controls.direction)) control.addEventListener("input", render);
controls.direction.addEventListener("click", () => {
  ascending = !ascending;
  controls.direction.textContent = ascending ? "Ascending" : "Descending";
  render();
});
render();
