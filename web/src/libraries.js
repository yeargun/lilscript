import libraryData from "./library-results.json";
import clientRuntime from "./client-runtime-results.json";
import popularData from "./popular-library-results.json";
import motionLab from "./motion-lab-results.json";
import { withBase } from "./base.js";
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
  const vite = result.surfaceArtifacts.find(
    (artifact) => artifact.id === "vite",
  );
  const rows = result.surfaceArtifacts.map((artifact) => {
    const className =
      artifact.id === "lilscript" ? ' class="lilscript-row"' : "";
    return `<tr${className}><th>${escape(artifact.label)}</th><td>${number.format(artifact.brotli)}</td><td>${number.format(artifact.gzip)}</td><td>${number.format(artifact.raw)}</td><td>${delta(artifact.brotli, vite.brotli)}</td></tr>`;
  });
  return `<table><thead><tr><th>Reusable selected API</th><th>Brotli-11 · primary</th><th>Gzip-9</th><th>Raw</th><th>vs npm/Vite</th></tr></thead><tbody>${rows.join("")}</tbody></table>`;
}

function demoTable(result) {
  const rows = result.artifacts.map((artifact) => {
    const className =
      artifact.id === "lilscript" ? ' class="lilscript-row"' : "";
    return `<tr${className}><th>${escape(artifact.label)}</th><td>${number.format(artifact.brotli)}</td><td>${number.format(artifact.gzip)}</td><td>${number.format(artifact.raw)}</td><td>${artifact.medianMs.toFixed(2)}</td></tr>`;
  });
  return `<table><thead><tr><th>Checked demo app</th><th>Brotli-11 · primary</th><th>Gzip-9</th><th>Raw</th><th>Median ms</th></tr></thead><tbody>${rows.join("")}</tbody></table>`;
}

function deployTable(result) {
  const rows = result.artifacts.map((artifact) => {
    const className =
      artifact.id === "lilscript" ? ' class="lilscript-row"' : "";
    return `<tr${className}><th>${escape(artifact.label)}</th><td>${number.format(artifact.deploy.brotli)}</td><td>${number.format(artifact.deploy.gzip)}</td><td>${number.format(artifact.deploy.raw)}</td></tr>`;
  });
  return `<table><thead><tr><th>HTML + JavaScript</th><th>Brotli-11 · primary</th><th>Gzip-9</th><th>Raw</th></tr></thead><tbody>${rows.join("")}</tbody></table>`;
}

function sizeTriplet(size) {
  if (!size || size.raw === "—") return "—";
  return `${number.format(size.brotli)} / ${number.format(size.gzip)} / ${number.format(size.raw)}`;
}

function ratio(value) {
  return value == null ? "—" : `${value.toFixed(3)}×`;
}

function popularStatus(result) {
  if (result.eligible) return "Eligible exact entrypoint";
  if (result.status === "blocked-adapter-algorithm") {
    return "Behavior match only; algorithm blocked";
  }
  if (
    result.status === "candidate-full-library" ||
    result.eligibility === "candidate"
  ) {
    return "Candidate port; not an eligibility win";
  }
  if (result.sizeGate === false && result.exactSurface)
    return "Exact entrypoint; size gate blocked";
  if (result.status === "behavior-exact-performance-blocked") {
    return "Behavior exact; performance blocked";
  }
  if (!result.exactSurface) return "Partial / candidate surface";
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
  const detailHref = `${withBase("/benchmark-detail.html")}?project=${encodeURIComponent(`popular:${result.id}`)}`;
  const examplesHref =
    result.id === "motion"
      ? ` <a class="secondary-link" href="#motion-lab-examples">Open examples</a>`
      : "";
  return `<tr${className}><th><a class="project-link" href="${detailHref}">${escape(result.project)}<i data-lucide="external-link" aria-hidden="true"></i></a><small class="table-note">${escape(popularStatus(result))}${examplesHref}</small></th><td>${sizeTriplet(result.rawJs)}</td><td>${sizeTriplet(result.terser)}</td><td>${closure}</td><td>${sizeTriplet(result.vite)}</td><td>${sizeTriplet(result.lilscript)}</td><td>${sizeTriplet(result.lilscriptVite)}</td><td>${result.vite?.brotli && result.lilscriptVite?.brotli ? `${number.format(result.lilscriptVite.brotli)} / ${number.format(result.vite.brotli)} (${delta(result.lilscriptVite.brotli, result.vite.brotli)})` : "—"}</td><td>${ratio(performance?.performance.ratio)}</td><td>${ratio(performance?.retainedMemory.ratio)}</td><td>${escape(boundary)}</td></tr>`;
}

function motionLabRow(example) {
  const win = example.brotliRatio != null && example.brotliRatio < 1;
  const className = win ? ' class="lilscript-row"' : "";
  const ratioText =
    example.brotliRatio == null ? "—" : `${example.brotliRatio.toFixed(3)}×`;
  return `<tr${className}><th>${escape(example.title)}<small class="table-note"><a href="${escape(example.npmUrl)}" target="_blank" rel="noopener">Open npm</a> · <a href="${escape(example.lilUrl)}" target="_blank" rel="noopener">Open LilScript</a></small></th><td>${sizeTriplet(example.npm)}</td><td>${sizeTriplet(example.lil)}</td><td>${ratioText}</td></tr>`;
}

function popularTable(results) {
  return `<table class="popular-matrix"><thead><tr><th>Project</th><th>Raw source · B/G/R</th><th>Terser · B/G/R</th><th>Closure · B/G/R</th><th>npm + Vite 8 · B/G/R</th><th>LilScript compiler · B/G/R</th><th>LilScript + Vite 8 · B/G/R</th><th>Brotli Lil / npm</th><th>Time Lil / npm</th><th>Memory Lil / npm</th><th>Compatibility boundary</th></tr></thead><tbody>${results.map(popularRow).join("")}</tbody></table>`;
}

const comparisonMetrics = {
  brotli: {
    label: "Brotli-11 bytes",
    shortLabel: "Brotli",
    value: (result, lane) => result[lane]?.brotli,
    format: (value) => `${number.format(value)} B`,
  },
  gzip: {
    label: "Gzip-9 bytes",
    shortLabel: "Gzip",
    value: (result, lane) => result[lane]?.gzip,
    format: (value) => `${number.format(value)} B`,
  },
  raw: {
    label: "Raw JavaScript bytes",
    shortLabel: "Raw",
    value: (result, lane) => result[lane]?.raw,
    format: (value) => `${number.format(value)} B`,
  },
  time: {
    label: "Median execution time",
    shortLabel: "Time",
    value: (result, lane) =>
      lane === "vite"
        ? result.performance?.performance?.npmMs
        : result.performance?.performance?.lilscriptMs,
    format: (value) => `${value.toFixed(3)} ms`,
  },
  memory: {
    label: "Retained heap",
    shortLabel: "Memory",
    value: (result, lane) =>
      lane === "vite"
        ? result.performance?.retainedMemory?.npmBytes
        : result.performance?.retainedMemory?.lilscriptBytes,
    format: (value) => `${number.format(value)} B`,
  },
  "disposed-memory": {
    label: "Post-unmount heap",
    shortLabel: "Post-unmount memory",
    value: (result, lane) =>
      lane === "vite"
        ? result.performance?.disposedMemory?.npmBytes
        : result.performance?.disposedMemory?.lilscriptBytes,
    format: (value) => `${number.format(value)} B`,
  },
};

const solidLilComparisons = clientRuntime.comparisons ?? clientRuntime.surfaces;
const solidLilDetailKey = (surface) =>
  surface.id === "lsx-client-app"
    ? "framework:solidlil-lsx"
    : surface.id === "app-vite" || surface.id === "app-closure"
      ? "framework:solidlil-application"
      : `framework:solidlil-${surface.id}`;

const selectableResults = [
  ...popularData.results,
  ...solidLilComparisons.map((surface) => ({
    id: `solidlil-${surface.id}`,
    project: `SolidLil ${surface.title}`,
    detailKey: solidLilDetailKey(surface),
    exactSurface:
      (surface.exactExports || surface.contractVerified) &&
      surface.behaviorEquivalent,
    eligible: surface.status === "eligible",
    status:
      surface.status === "optimization-gap"
        ? "behavior-exact-performance-blocked"
        : surface.status,
    vite: surface.artifacts.find(({ id }) => id === "solid"),
    lilscriptVite: surface.artifacts.find(({ id }) => id === "solidlil"),
  })),
  ...(clientRuntime.lsxApplication &&
  !solidLilComparisons.some(({ id }) => id === "lsx-client-app")
    ? [
        {
          id: "solidlil-lsx",
          project: "SolidLil integrated LSX parity fixture",
          detailKey: "framework:solidlil-lsx",
          exactSurface: false,
          eligible: false,
          status: clientRuntime.lsxApplication.status,
          vite: clientRuntime.lsxApplication.artifacts.find(
            ({ id }) => id === "solid-lsx-vite",
          ),
          lilscriptVite: clientRuntime.lsxApplication.artifacts.find(
            ({ id }) => id === "solidlil-lsx-vite",
          ),
          performance: clientRuntime.lsxApplication.performance
            ? {
                performance: {
                  npmMs:
                    clientRuntime.lsxApplication.performance.medians.solidLsx,
                  lilscriptMs:
                    clientRuntime.lsxApplication.performance.medians
                      .solidlilLsx,
                  ratio: clientRuntime.lsxApplication.performance.ratio,
                },
                retainedMemory: {
                  npmBytes:
                    clientRuntime.lsxApplication.performance.retainedMemory
                      .solidLsx,
                  lilscriptBytes:
                    clientRuntime.lsxApplication.performance.retainedMemory
                      .solidlilLsx,
                  ratio: clientRuntime.lsxApplication.performance.memoryRatio,
                },
                disposedMemory: {
                  npmBytes:
                    clientRuntime.lsxApplication.performance.disposedMemory
                      .solidLsx,
                  lilscriptBytes:
                    clientRuntime.lsxApplication.performance.disposedMemory
                      .solidlilLsx,
                  ratio:
                    clientRuntime.lsxApplication.performance
                      .disposedMemoryRatio,
                },
              }
            : null,
        },
      ]
    : []),
];

function metricPair(result, metricName) {
  const metric = comparisonMetrics[metricName];
  const baseline = metric.value(result, "vite");
  const candidate = metric.value(result, "lilscriptVite");
  return Number.isFinite(baseline) && Number.isFinite(candidate)
    ? { baseline, candidate, ratio: candidate / baseline }
    : null;
}

function initLibraryComparator() {
  const root = document.querySelector("[data-library-comparator]");
  if (!root) return;
  const metricSelect = root.querySelector("[data-compare-metric]");
  const scopeSelect = root.querySelector("[data-compare-scope]");
  const sortSelect = root.querySelector("[data-compare-sort]");
  const searchInput = root.querySelector("[data-compare-search]");
  const picker = root.querySelector("[data-compare-picker]");
  const results = root.querySelector("[data-compare-results]");
  const total = root.querySelector("[data-compare-total]");
  const params = new URLSearchParams(location.search);
  const validMetric = comparisonMetrics[params.get("metric")]
    ? params.get("metric")
    : "brotli";
  const validScopes = new Set([
    "all",
    "exact",
    "candidate",
    "eligible",
    "regression",
  ]);
  const validSorts = new Set(["saving", "name", "candidate", "baseline"]);
  metricSelect.value = validMetric;
  scopeSelect.value = validScopes.has(params.get("scope"))
    ? params.get("scope")
    : "all";
  sortSelect.value = validSorts.has(params.get("sort"))
    ? params.get("sort")
    : "saving";
  const selected = new Set(
    params.has("libs")
      ? params.get("libs").split(",").filter(Boolean)
      : selectableResults.map((result) => result.id),
  );

  function visibleRows() {
    const metric = metricSelect.value;
    const scope = scopeSelect.value;
    const query = searchInput.value.trim().toLocaleLowerCase();
    return selectableResults.filter((result) => {
      const pair = metricPair(result, metric);
      const scopeMatch =
        scope === "all" ||
        (scope === "exact" && result.exactSurface) ||
        (scope === "candidate" && !result.exactSurface) ||
        (scope === "eligible" && result.eligible) ||
        (scope === "regression" && pair && pair.ratio > 1);
      return (
        scopeMatch &&
        (!query ||
          `${result.project} ${result.id}`.toLocaleLowerCase().includes(query))
      );
    });
  }

  function syncUrl() {
    const next = new URL(location.href);
    next.searchParams.set("metric", metricSelect.value);
    next.searchParams.set("scope", scopeSelect.value);
    next.searchParams.set("sort", sortSelect.value);
    next.searchParams.set("libs", [...selected].sort().join(","));
    history.replaceState(
      null,
      "",
      `${next.pathname}${next.search}${next.hash}`,
    );
  }

  function renderPicker(visible) {
    const metric = comparisonMetrics[metricSelect.value];
    picker.innerHTML = visible.length
      ? visible
          .map((result) => {
            const available = metricPair(result, metricSelect.value) !== null;
            const boundary = result.exactSurface ? "exact" : "candidate";
            return `<label class="library-choice${available ? "" : " unavailable"}"><input type="checkbox" value="${escape(result.id)}" ${selected.has(result.id) ? "checked" : ""} ${available ? "" : "disabled"}><span><strong>${escape(result.project)}</strong><small>${boundary}${available ? "" : ` · no ${escape(metric.shortLabel.toLowerCase())} sample`}</small></span></label>`;
          })
          .join("")
      : '<p class="comparator-empty">No libraries match this filter.</p>';
  }

  function renderResults(visible) {
    const metric = comparisonMetrics[metricSelect.value];
    const compared = visible
      .filter((result) => selected.has(result.id))
      .map((result) => ({
        result,
        pair: metricPair(result, metricSelect.value),
      }))
      .filter((item) => item.pair !== null);
    const sort = sortSelect.value;
    compared.sort((left, right) => {
      if (sort === "name")
        return left.result.project.localeCompare(right.result.project);
      if (sort === "candidate")
        return left.pair.candidate - right.pair.candidate;
      if (sort === "baseline") return left.pair.baseline - right.pair.baseline;
      return left.pair.ratio - right.pair.ratio;
    });
    const baselineTotal = compared.reduce(
      (sum, item) => sum + item.pair.baseline,
      0,
    );
    const candidateTotal = compared.reduce(
      (sum, item) => sum + item.pair.candidate,
      0,
    );
    const wins = compared.filter((item) => item.pair.ratio <= 1).length;
    total.textContent = compared.length
      ? `${wins} of ${compared.length} selected rows are no larger/slower; combined ${delta(candidateTotal, baselineTotal)}.`
      : "Select at least one library with data for this metric.";
    results.innerHTML = compared.length
      ? `<div class="comparator-result-head"><span>Library and evidence</span><span>${escape(metric.label)} · lower is better</span></div>${compared
          .map(({ result, pair }) => {
            const max = Math.max(pair.baseline, pair.candidate);
            const outcome =
              pair.ratio < 1 ? "win" : pair.ratio > 1 ? "loss" : "tie";
            const status = result.exactSurface
              ? "exact API"
              : "candidate / partial";
            const href = `/benchmark-detail.html?project=${encodeURIComponent(result.detailKey ?? `popular:${result.id}`)}`;
            return `<article class="comparator-result ${outcome}"><div class="comparator-result-title"><div><a href="${href}">${escape(result.project)}</a><span>${status}</span></div><strong>${delta(pair.candidate, pair.baseline)}</strong></div><div class="comparison-bars"><div><span>Official / npm</span><i><b style="width:${(pair.baseline / max) * 100}%"></b></i><em>${metric.format(pair.baseline)}</em></div><div><span>LilScript candidate</span><i><b style="width:${(pair.candidate / max) * 100}%"></b></i><em>${metric.format(pair.candidate)}</em></div></div></article>`;
          })
          .join("")}`
      : '<p class="comparator-empty">Nothing selected with comparable data. Use the library checkboxes or change the evidence filter.</p>';
  }

  function render() {
    const visible = visibleRows();
    renderPicker(visible);
    renderResults(visible);
    syncUrl();
  }

  picker.addEventListener("change", (event) => {
    if (!(event.target instanceof HTMLInputElement)) return;
    if (event.target.checked) selected.add(event.target.value);
    else selected.delete(event.target.value);
    render();
  });
  for (const control of [metricSelect, scopeSelect, sortSelect]) {
    control.addEventListener("change", render);
  }
  searchInput.addEventListener("input", render);
  root.querySelector("[data-select-visible]").addEventListener("click", () => {
    for (const result of visibleRows()) {
      if (metricPair(result, metricSelect.value)) selected.add(result.id);
    }
    render();
  });
  root.querySelector("[data-select-winners]").addEventListener("click", () => {
    selected.clear();
    for (const result of visibleRows()) {
      const pair = metricPair(result, metricSelect.value);
      if (pair?.ratio <= 1) selected.add(result.id);
    }
    render();
  });
  root.querySelector("[data-clear-selection]").addEventListener("click", () => {
    selected.clear();
    render();
  });
  render();
}

initLibraryComparator();

const wins = libraryData.results.filter((result) => {
  const vite = result.surfaceArtifacts.find(
    (artifact) => artifact.id === "vite",
  );
  const lilscript = result.surfaceArtifacts.find(
    (artifact) => artifact.id === "lilscript",
  );
  return lilscript.brotli < vite.brotli;
}).length;
document.querySelector("[data-library-summary]").textContent =
  `LilScript produces the smaller reusable-API Brotli payload in ${wins} of ${libraryData.results.length} eligible complete ports. Eligibility also requires non-larger raw and configured-codec output than Closure ADVANCED plus the runtime and retained-memory gates.`;

const eligiblePopular = popularData.results.filter((result) => result.eligible);
const exactPopular = popularData.results.filter(
  (result) => result.exactSurface,
);
const blockedPopular = exactPopular.filter((result) => !result.eligible);
const candidatePopular = popularData.results.filter(
  (result) =>
    !result.exactSurface &&
    result.lilscriptVite &&
    result.lilscriptVite.raw !== "—",
);
document.querySelector("[data-popular-summary]").textContent =
  `${eligiblePopular.length} of ${exactPopular.length} exact selected entrypoints currently clear behavior and algorithm parity, a non-larger Brotli-11 cell from the Brotli-selected artifact than both npm/Vite 8 and public-API-preserving Closure ADVANCED, and ≤${((popularData.metadata.materialRegressionLimit - 1) * 100).toFixed(0)}% time/retained-memory gates. Gzip-9 and raw are diagnostics for that artifact and may lose. Candidate / partial ports remain separately labeled and are not eligibility wins.`;
document.querySelector("[data-popular-eligible]").innerHTML =
  eligiblePopular.length
    ? popularTable(eligiblePopular)
    : "<p>No requested package currently clears every publication gate.</p>";
document.querySelector("[data-popular-blocked]").innerHTML =
  blockedPopular.length
    ? popularTable(blockedPopular)
    : "<p>No exact entrypoint is currently blocked.</p>";
const candidateMount = document.querySelector("[data-popular-candidates]");
if (candidateMount) {
  candidateMount.innerHTML = candidatePopular.length
    ? popularTable(candidatePopular)
    : "<p>No candidate ports with measured sizes.</p>";
}
document.querySelector("[data-popular-method]").textContent =
  `Generated ${popularData.metadata.generatedAt.slice(0, 10)} from compiler revision ${popularData.metadata.compilerRevision} with Node ${popularData.metadata.node}, Vite ${popularData.metadata.vite}, Terser ${popularData.metadata.terser}, and Closure Compiler ${popularData.metadata.closure}. Size cells are Brotli-11 / gzip-9 / raw bytes; Closure's actual compilation level is shown per row.`;

const motionSummary = document.querySelector("[data-motion-lab-summary]");
const motionMount = document.querySelector("[data-motion-lab-examples]");
if (motionSummary && motionMount) {
  const avg = motionLab.avgBrotliRatio;
  motionSummary.textContent = `${motionLab.wins} of ${motionLab.examples.length} call-site fixtures are smaller under Brotli than npm motion@13 (avg ${avg.toFixed(3)}×). Open them as dual iframes in Demos, or npm and LilScript below.`;
  motionMount.innerHTML = `<table class="popular-matrix"><thead><tr><th>Example</th><th>npm Vite 8</th><th>LilScript Vite 8</th><th>Brotli Lil / npm</th></tr></thead><tbody>${motionLab.examples.map(motionLabRow).join("")}</tbody></table>`;
}

const upstream = clientRuntime.upstream;
const surfaceRows = solidLilComparisons
  .map((surface) => {
    const solid = surface.artifacts.find(({ id }) => id === "solid");
    const solidlil = surface.artifacts.find(({ id }) => id === "solidlil");
    const className =
      surface.status === "eligible" ? ' class="lilscript-row"' : "";
    const contract = Number.isFinite(surface.exportCount)
      ? `${number.format(surface.exportCount)} exact exports`
      : "whole-program observable contract";
    return `<tr${className}><th><a class="project-link" href="${withBase("/benchmark-detail.html")}?project=${encodeURIComponent(solidLilDetailKey(surface))}">${escape(surface.title)}<i data-lucide="external-link" aria-hidden="true"></i></a><small class="table-note">${escape(contract)} · ${escape(surface.boundary)} · ${escape(surface.status)}</small></th><td>${number.format(solid.brotli)} / ${number.format(solidlil.brotli)}</td><td>${number.format(solid.gzip)} / ${number.format(solidlil.gzip)}</td><td>${number.format(solid.raw)} / ${number.format(solidlil.raw)}</td><td>${delta(solidlil.brotli, solid.brotli)}</td></tr>`;
  })
  .join("");
const app = clientRuntime.appSnapshot;
const appRows = app.sizes
  .map((artifact) => {
    const className = artifact.id.startsWith("solidlil")
      ? ' class="lilscript-row"'
      : "";
    return `<tr${className}><th>${escape(artifact.label)}</th><td>${number.format(artifact.brotli)}</td><td>${number.format(artifact.gzip)}</td><td>${number.format(artifact.raw)}</td></tr>`;
  })
  .join("");
const lifecycle = clientRuntime.lifecycle;
document.querySelector("[data-client-runtime]").innerHTML =
  `<section class="benchmark-project client-runtime" id="${escape(clientRuntime.id)}"><header><p class="eyebrow">Integrated exact runtime evidence</p><h2>${escape(clientRuntime.title)}</h2><p><code>${escape(upstream.package)}@${escape(upstream.version)}</code> resolves ${number.format(upstream.candidateTestsPassed)}/${number.format(upstream.candidateTestsTotal)} unchanged upstream tests to SolidLil across ${number.format(upstream.files)} files. The complete public browser inventory is ${number.format(clientRuntime.apiParity.verified)}/${number.format(clientRuntime.apiParity.expected)} verified exports.</p><p class="comparison-note"><strong>Runtime parity and LSX parity are separate.</strong> Core, Store, and the declared client Web target pass their Brotli-objective gates. The full 73-export compatibility row remains behavior-exact but is an explicit optimization gap because it includes server/hydration-facing exports outside the client target. Raw and gzip are diagnostics for the Brotli-selected artifacts. LSX remains ${number.format(clientRuntime.lsx.counts.loweringVerified)}/${number.format(clientRuntime.lsx.counts.expected)} lowering families and ${number.format(clientRuntime.lsx.counts.runtimeVerified)}/${number.format(clientRuntime.lsx.counts.expected)} integrated runtime families.</p><div class="benchmark-links"><a class="secondary-link" href="/solidlil.html#runtime-comparison"><i data-lucide="arrow-right" aria-hidden="true"></i>Open selectable SolidLil comparison</a><a class="secondary-link" href="/explorer.html?category=framework-runtime"><i data-lucide="search" aria-hidden="true"></i>Filter exact surfaces</a></div></header><div class="benchmark-table-wrap"><table><thead><tr><th>Distribution or application boundary</th><th>Solid / SolidLil Brotli-11 · objective</th><th>Gzip-9 · diagnostic</th><th>Raw · diagnostic</th><th>Brotli delta</th></tr></thead><tbody>${surfaceRows}</tbody></table></div><p class="comparison-note"><strong>Bundle boundary:</strong> open-world rows preserve documented export names for unknown consumers; closed-world application rows know the entire graph and may tree-shake or mangle those names. They are alternatives, not additive bytes.</p><details class="deploy-details"><summary>Ownership, unmount, and Playwright memory gate</summary><p>${number.format(lifecycle.cycles)} root cycles, ${number.format(lifecycle.collectionCycles)} keyed/indexed collection cycles, and ${number.format(lifecycle.resourceCycles)} late resource resolutions match Solid. SolidLil returns all ${number.format(lifecycle.slots.owners)} owner and ${number.format(lifecycle.slots.effects)} effect slots with zero pending effects; randomized Chromium CPU/RAM eligibility is <strong>${lifecycle.repeatedMemoryEligibility ? "pass" : "pending fresh benchmark"}</strong>.</p></details><details class="deploy-details"><summary>Archived LSX size snapshot</summary><p>${escape(app.notes)}</p><div class="benchmark-table-wrap"><table><thead><tr><th>Client app artifact</th><th>Brotli-11 · primary</th><th>Gzip-9</th><th>Raw</th></tr></thead><tbody>${appRows}</tbody></table></div><p>This legacy size-only snapshot is excluded from current behavior and performance evidence.</p></details></section>`;

document.querySelector("[data-library-results]").innerHTML = libraryData.results
  .map((result) => {
    const packages = result.packages
      .map((item) => `${item.name}@${item.version}`)
      .join(" + ");
    return `<section class="benchmark-project" id="${escape(result.id)}"><header><p class="eyebrow">${escape(result.scope)}</p><h2>${escape(result.title)}</h2><p><code>${escape(packages)}</code> with ${number.format(result.monthlyDownloads)} monthly downloads at selection time. ${number.format(result.translatedAssertions)} translated upstream assertions and ${number.format(result.additionalAssertions ?? 0)} added contract assertions precede dense differential API tests.</p><code class="benchmark-contract">${escape(result.expected)}</code></header><div class="benchmark-table-wrap">${artifactTable(result)}</div><details class="deploy-details"><summary>Checked demo-app diagnostics</summary><div class="benchmark-table-wrap">${demoTable(result)}</div></details><details class="deploy-details"><summary>Full demo deploy size</summary><div class="benchmark-table-wrap">${deployTable(result)}</div></details></section>`;
  })
  .join("");

document.querySelector("[data-ineligible-table]").innerHTML =
  `<table><thead><tr><th>Package</th><th>Version</th><th>Current blocker</th></tr></thead><tbody>${libraryData.auditedButIneligible.map((item) => `<tr><th>${escape(item.package)}</th><td>${escape(item.version)}</td><td>${escape(item.reason)}</td></tr>`).join("")}</tbody></table>`;

const metadata = libraryData.metadata;
document.querySelector("[data-library-method]").textContent =
  `Generated ${metadata.generatedAt.slice(0, 10)} from compiler revision ${metadata.compilerRevision} with Node ${metadata.node}, Vite ${metadata.vite}, esbuild ${metadata.esbuild}, and Closure Compiler ${metadata.closure}. Size eligibility uses reusable selected APIs; demo-app specialization is diagnostic only. Each LilScript row passed JavaScript, emitted-C, and native execution before publication.`;

renderIcons(document.querySelector("[data-client-runtime]"));
renderIcons(document.querySelector("[data-popular-eligible]"));
renderIcons(document.querySelector("[data-popular-blocked]"));
renderIcons(document.querySelector("[data-popular-candidates]"));
renderIcons(document.querySelector("[data-motion-lab-examples]"));
