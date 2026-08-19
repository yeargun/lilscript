import catalog from "./benchmark-catalog.json";
import { withBase } from "./base.js";
import { renderIcons } from "./site.js";

const number = new Intl.NumberFormat("en-US");
const escape = (value) =>
  String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
const key = new URLSearchParams(location.search).get("project");
const project = catalog.projects.find((candidate) => candidate.key === key);
const root = document.querySelector("[data-project-detail]");

if (!project) {
  document.title = "Benchmark not found | LilScript";
  root.innerHTML = `<section class="comparison-intro"><p class="eyebrow">Unknown project</p><h1>That benchmark is not in the catalog.</h1><a class="primary-link" href="${withBase("/explorer.html")}">Open benchmark explorer</a></section>`;
} else {
  document.title = `${project.title} benchmark | LilScript`;
  const packages = project.packages.length
    ? `<div class="package-links">${project.packages.map((item) => `<a target="_blank" rel="noopener" href="https://www.npmjs.com/package/${encodeURIComponent(item.name)}">${escape(item.name)}${item.version ? `@${escape(item.version)}` : ""}<i data-lucide="external-link" aria-hidden="true"></i></a>`).join("")}</div>`
    : "";
  const artifacts = project.artifacts
    .map(
      (artifact) =>
        `<tr><th>${escape(artifact.label)}<small>${escape(artifact.output ?? artifact.id)}</small></th><td>${escape(artifact.tool)}</td><td>${escape(artifact.mode)}</td><td>${escape(artifact.propertyMangling)}</td><td>${number.format(artifact.brotli)}</td><td>${number.format(artifact.gzip)}</td><td>${number.format(artifact.raw)}</td>${artifact.medianMs == null ? "<td>—</td>" : `<td>${artifact.medianMs.toFixed(2)}</td>`}</tr>`,
    )
    .join("");
  const sourceSections = project.sources
    .map(
      (item, index) =>
        `<article class="source-card"><header><div><p class="eyebrow">${escape(item.language)}</p><h2>${escape(item.label)}</h2><code>${escape(item.path)}</code></div><a class="secondary-link" target="_blank" rel="noopener" href="${escape(item.url)}">Complete file<i data-lucide="external-link" aria-hidden="true"></i></a></header><pre tabindex="0"><code>${escape(item.code)}</code></pre></article>`,
    )
    .join("");
  root.innerHTML = `
    <section class="comparison-intro detail-hero"><div class="detail-badges"><span>${escape(project.category)}</span><span class="status-badge ${escape(project.status)}">${escape(project.status)}</span></div><h1>${escape(project.title)}</h1><p class="lead">${escape(project.summary)}</p>${packages}<a class="secondary-link" href="${withBase("/explorer.html")}"><i data-lucide="arrow-right" aria-hidden="true"></i>Back to all rows</a></section>
    ${project.demos?.length ? `<section class="benchmark-project"><header><p class="eyebrow">Openable examples</p><h2>Run npm and LilScript side by side</h2><p>Each link opens a built Vite fixture in a new tab.</p></header><div class="benchmark-links">${project.demos.map((demo) => `<a class="secondary-link" target="_blank" rel="noopener" href="${escape(demo.url)}">${escape(demo.label)}<i data-lucide="external-link" aria-hidden="true"></i></a>`).join("")}</div></section>` : ""}
    <section class="benchmark-project"><header><p class="eyebrow">Comparable boundary</p><h2>Why these rows can be compared</h2><p>${escape(project.fairness)}</p>${project.expected == null ? "" : `<code class="benchmark-contract">${escape(project.expected)}</code>`}${project.blockers?.length ? `<p class="comparison-note"><strong>Current blockers:</strong> ${escape(project.blockers.join(" "))}</p>` : ""}${project.exclusions?.length ? `<p class="comparison-note"><strong>Explicit exclusions:</strong> ${escape(project.exclusions.join(" · "))}</p>` : ""}</header></section>
    <section class="benchmark-project"><header><p class="eyebrow">Published artifacts</p><h2>Transport and raw bytes</h2><p>Brotli-11 is the primary column. Property-renamed rows are only valid under the boundary above; every compression cell measures its JavaScript file independently.</p></header><div class="benchmark-table-wrap"><table><thead><tr><th>Artifact</th><th>Tool</th><th>Mangling</th><th>Properties</th><th>Brotli-11 · primary</th><th>Gzip-9</th><th>Raw</th><th>Median ms</th></tr></thead><tbody>${artifacts}</tbody></table></div></section>
    <section class="source-section"><div class="section-heading"><p class="eyebrow">Source evidence</p><h2>Code used by the harness</h2><p>Previews are generated from repository files during the web build; complete-file links open GitHub.</p></div>${sourceSections || '<p class="comparison-note">This external or generated row has no local source preview. Its compatibility note and reproducible source location remain above.</p>'}</section>`;
  renderIcons(root);
}
