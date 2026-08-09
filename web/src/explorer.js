import catalog from "./benchmark-catalog.json";
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
const rows = catalog.projects.flatMap((project) => project.artifacts.map((artifact) => ({ project, artifact })));
const controls = {
  search: document.querySelector("[data-filter-search]"),
  category: document.querySelector("[data-filter-category]"),
  status: document.querySelector("[data-filter-status]"),
  tool: document.querySelector("[data-filter-tool]"),
  mode: document.querySelector("[data-filter-mode]"),
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
  [eligible, "eligible package rows"],
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
  if (["raw", "gzip", "brotli"].includes(key)) {
    a = left.artifact[key]; b = right.artifact[key];
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
function render() {
  const query = controls.search.value.trim().toLowerCase();
  const filtered = rows.filter((row) =>
    (!query || haystack(row).includes(query)) &&
    (!controls.category.value || row.project.category === controls.category.value) &&
    (!controls.status.value || row.project.status === controls.status.value) &&
    (!controls.tool.value || row.artifact.tool === controls.tool.value) &&
    (!controls.mode.value || row.artifact.mode === controls.mode.value)
  ).sort(compare);
  document.querySelector("[data-result-count]").textContent = `${number.format(filtered.length)} of ${number.format(rows.length)} artifact rows across ${new Set(filtered.map((row) => row.project.key)).size} projects`;
  document.querySelector("[data-explorer-rows]").innerHTML = filtered.map(({ project, artifact }) => `
    <tr>
      <th><a class="project-link" target="_blank" rel="noopener" href="/benchmark-detail.html?project=${encodeURIComponent(project.key)}">${escape(project.title)}<i data-lucide="external-link" aria-hidden="true"></i></a><small>${escape(project.packages.map((item) => item.name).join(" + ") || project.id)}</small></th>
      <td>${escape(labels[project.category] ?? project.category)}</td><td><span class="status-badge ${escape(project.status)}">${escape(project.status)}</span></td>
      <td>${escape(artifact.label)}</td><td>${escape(artifact.tool)}</td><td>${escape(artifact.mode)}</td><td>${escape(artifact.propertyMangling)}</td>
      <td class="numeric">${number.format(artifact.raw)}</td><td class="numeric">${number.format(artifact.gzip)}</td><td class="numeric">${number.format(artifact.brotli)}</td>
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
