import parity from "./solid-api-parity.json";
import lsxParity from "./solid-lsx-parity.json";
import runtime from "./client-runtime-results.json";
import "./site.js";

const number = new Intl.NumberFormat("en-US");
const escape = (value) =>
  String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");

const mount = document.querySelector("[data-solid-api-parity]");
if (mount) {
  mount.innerHTML = `<div class="section-heading"><p class="eyebrow">Exact public contract</p><h2 id="solidlil-api-title">${number.format(parity.totals.verified)} of ${number.format(parity.totals.expected)} exports verified</h2><p>Core, web, and store are counted from the pinned ${escape(parity.baseline)} package. Exporting a same-named function is only “implemented”; “verified” requires differential behavior evidence.</p></div><div class="responsibility-grid solid-parity-grid">${parity.surfaces
    .map(
      (surface) =>
        `<article><span>${escape(surface.name)}</span><h3>${number.format(surface.counts.verified)} verified / ${number.format(surface.counts.expected)}</h3><p>${number.format(surface.counts.implemented)} implemented · ${number.format(surface.counts.missing)} missing</p><details><summary>Missing and awaiting evidence</summary><p>${surface.rows
          .filter((row) => row.status !== "verified")
          .map(
            (row) =>
              `<code class="parity-export ${escape(row.status)}">${escape(row.name)}</code>`,
          )
          .join(" ")}</p></details></article>`,
    )
    .join(
      "",
    )}</div><p class="comparison-note"><strong>Strict browser API gate: ${parity.complete ? "pass" : "incomplete"}.</strong> LSX, server-target behavior, and ecosystem compatibility remain separate gates.</p>`;
}

const runtimeMount = document.querySelector("[data-solid-runtime-results]");
if (runtimeMount) {
  const mode = runtime.buildModes;
  const allSurfaces = runtime.comparisons ?? [
    ...runtime.surfaces,
    ...(runtime.closedWorldSurfaces ?? []),
  ];
  const strictWinners = allSurfaces.filter(
    ({ status }) => status === "eligible",
  ).length;
  const openCount = allSurfaces.filter(
    ({ boundary }) => boundary === "open-world-distribution",
  ).length;
  const closedCount = allSurfaces.filter(
    ({ boundary }) => boundary === "closed-world-application",
  ).length;
  const candidateTargets = runtime.distributionOptimization?.targets ?? {};
  runtimeMount.innerHTML = `<div class="section-heading"><p class="eyebrow">Brotli-first · both deployment boundaries</p><h2 id="solidlil-runtime-title">${strictWinners} of ${allSurfaces.length} comparisons pass every gate</h2><p>A published library is normally <strong>open world</strong>: unknown future consumers must be able to call documented exports, although internal names still shrink. An application bundle is <strong>closed world</strong>: the complete graph is known, so tree shaking may erase unused public APIs and rename everything that remains. SolidLil applications primarily use the second boundary, so both are first-class rows below.</p></div><div class="comparator-controls solid-runtime-controls"><label><span>Metric</span><select data-solid-runtime-metric><option value="brotli">Brotli-11 bytes · objective</option><option value="gzip">Gzip-9 bytes · diagnostic</option><option value="raw">Raw JavaScript bytes · diagnostic</option></select></label><label><span>Boundary</span><select data-solid-runtime-boundary><option value="all">All boundaries</option><option value="open-world-distribution">Open-world distributions</option><option value="closed-world-application">Closed-world applications</option></select></label></div><div class="comparator-actions" aria-label="Runtime surface selection"><button type="button" data-solid-runtime-client>Select client distributions</button><button type="button" data-solid-runtime-apps>Select applications</button><button type="button" data-solid-runtime-all>Select all</button><button type="button" data-solid-runtime-clear>Clear</button></div><fieldset class="comparator-picker solid-runtime-picker"><legend>Surfaces and applications in comparison</legend><div data-solid-runtime-picker></div></fieldset><p class="comparator-total" data-solid-runtime-summary aria-live="polite"></p><div class="compact-table-wrap" data-solid-runtime-table></div><div class="responsibility-grid"><article><span>Open-world distribution</span><h3>${number.format(openCount)} reusable entries</h3><p>${number.format(mode.openWorld.publicExports)} Core exports remain stable and callable. Exact export and differential behavior parity gate every distribution row.</p></article><article><span>Closed-world application</span><h3>${number.format(closedCount)} whole-program rows</h3><p>Unused public functions disappear; reachable names may be mangled. Observable UI behavior, teardown, randomized CPU, and retained heap replace export count as the contract.</p></article><article><span>Final-artifact selection</span><h3>${number.format(Object.keys(candidateTargets).length)} targets scored after tree shaking</h3><p>Compiler representations are compared only after the actual entry is tree-shaken and minified. Canonical Brotli-11 chooses the winner; candidate hashes stay auditable.</p></article><article><span>Candidate suite</span><h3>${number.format(runtime.upstream.candidateTestsPassed)}/${number.format(runtime.upstream.candidateTestsTotal)} unchanged tests</h3><p>Core, Web, and Store imports resolve to SolidLil across ${number.format(runtime.upstream.files)} pinned files before size is credited.</p></article></div><p class="comparison-note"><strong>Eligibility is boundary-aware and objective-specific:</strong> the appropriate behavior contract, statistically non-degraded CPU/RAM, and fewer Brotli-11 bytes from the Brotli-target artifact. Raw and gzip remain diagnostics rather than extra gates.</p>`;

  const metrics = {
    brotli: "Brotli-11",
    gzip: "Gzip-9",
    raw: "Raw JavaScript",
  };
  const selected = new Set(allSurfaces.map(({ id }) => id));
  const metricSelect = runtimeMount.querySelector(
    "[data-solid-runtime-metric]",
  );
  const boundarySelect = runtimeMount.querySelector(
    "[data-solid-runtime-boundary]",
  );
  const picker = runtimeMount.querySelector("[data-solid-runtime-picker]");
  const summary = runtimeMount.querySelector("[data-solid-runtime-summary]");
  const table = runtimeMount.querySelector("[data-solid-runtime-table]");

  const delta = (candidate, baseline) => {
    if (candidate === baseline) return "tie";
    const percent = Math.abs((candidate / baseline - 1) * 100).toFixed(2);
    return `${percent}% ${candidate < baseline ? "smaller" : "larger"}`;
  };
  const boundaryLabel = (boundary) =>
    boundary === "open-world-distribution"
      ? "open-world distribution"
      : boundary === "closed-world-application"
        ? "closed-world application"
        : boundary;

  const contractLabel = (surface) =>
    Number.isFinite(surface.exportCount)
      ? `${number.format(surface.exportCount)} public exports`
      : "whole-program observable contract";

  function renderRuntimeComparison() {
    const metric = metricSelect.value;
    const boundary = boundarySelect.value;
    const inBoundary = allSurfaces.filter(
      (surface) => boundary === "all" || surface.boundary === boundary,
    );
    const visible = inBoundary.filter(({ id }) => selected.has(id));
    picker.innerHTML = inBoundary
      .map(
        (surface) =>
          `<label class="library-choice"><input type="checkbox" value="${escape(surface.id)}" ${selected.has(surface.id) ? "checked" : ""}><span><strong>${escape(surface.title)}</strong><small>${escape(contractLabel(surface))} · ${escape(boundaryLabel(surface.boundary))}</small></span></label>`,
      )
      .join("");
    const wins = visible.filter((surface) => {
      const solid = surface.artifacts.find(({ id }) => id === "solid");
      const solidlil = surface.artifacts.find(({ id }) => id === "solidlil");
      return solidlil[metric] < solid[metric];
    }).length;
    summary.textContent = visible.length
      ? `${wins} of ${visible.length} selected comparisons are smaller for ${metrics[metric]}. Rows are alternative deliverables, so their bytes are not summed into a fictitious deployment total.`
      : "Select at least one comparison in this boundary.";
    table.innerHTML = visible.length
      ? `<table><thead><tr><th>Exact surface and boundary</th><th>Official Solid</th><th>SolidLil</th><th>${metrics[metric]} result</th><th>Declared objective</th></tr></thead><tbody>${visible
          .map((surface) => {
            const solid = surface.artifacts.find(({ id }) => id === "solid");
            const solidlil = surface.artifacts.find(
              ({ id }) => id === "solidlil",
            );
            const winsMetric = solidlil[metric] < solid[metric];
            return `<tr${winsMetric ? ' class="lilscript-row"' : ""}><th>${escape(surface.title)}<small>${escape(contractLabel(surface))} · ${escape(surface.scope)}</small></th><td>${number.format(solid[metric])} B</td><td>${number.format(solidlil[metric])} B</td><td>${delta(solidlil[metric], solid[metric])}</td><td>${surface.status === "eligible" ? "all gates pass" : escape(surface.status)}</td></tr>`;
          })
          .join("")}</tbody></table>`
      : "";
  }

  picker.addEventListener("change", (event) => {
    if (!(event.target instanceof HTMLInputElement)) return;
    if (event.target.checked) selected.add(event.target.value);
    else selected.delete(event.target.value);
    renderRuntimeComparison();
  });
  metricSelect.addEventListener("change", renderRuntimeComparison);
  boundarySelect.addEventListener("change", renderRuntimeComparison);
  runtimeMount
    .querySelector("[data-solid-runtime-client]")
    .addEventListener("click", () => {
      selected.clear();
      for (const id of ["core", "store", "web-client"]) selected.add(id);
      boundarySelect.value = "open-world-distribution";
      renderRuntimeComparison();
    });
  runtimeMount
    .querySelector("[data-solid-runtime-apps]")
    .addEventListener("click", () => {
      selected.clear();
      for (const surface of allSurfaces) {
        if (surface.boundary === "closed-world-application") {
          selected.add(surface.id);
        }
      }
      boundarySelect.value = "closed-world-application";
      renderRuntimeComparison();
    });
  runtimeMount
    .querySelector("[data-solid-runtime-all]")
    .addEventListener("click", () => {
      for (const { id } of allSurfaces) selected.add(id);
      boundarySelect.value = "all";
      renderRuntimeComparison();
    });
  runtimeMount
    .querySelector("[data-solid-runtime-clear]")
    .addEventListener("click", () => {
      selected.clear();
      renderRuntimeComparison();
    });
  renderRuntimeComparison();
}

const lsxMount = document.querySelector("[data-solid-lsx-parity]");
if (lsxMount) {
  const excluded = lsxParity.features.filter(
    (feature) =>
      feature.lowering === "excluded" || feature.runtime === "excluded",
  );
  lsxMount.innerHTML = `<div class="section-heading"><p class="eyebrow">LSX client feature gate</p><h2 id="solidlil-lsx-title">${number.format(lsxParity.counts.loweringVerified)} of ${number.format(lsxParity.counts.expected)} client families verified</h2><p>The parser and lowerer live in the monorepo and reject unsupported syntax explicitly. Integrated runtime evidence covers all ${number.format(lsxParity.counts.runtimeVerified)} in-scope client families; the inventory also keeps ${number.format(excluded.length)} server-coupled exclusions visible.</p></div><details class="parity-ledger"><summary>Open the complete LSX boundary ledger</summary><div>${lsxParity.features
    .map(
      (feature) =>
        `<article><strong>${escape(feature.label)}</strong><span>lowering: ${escape(feature.lowering)} · runtime: ${escape(feature.runtime)}</span><p>${escape(feature.notes)}</p></article>`,
    )
    .join(
      "",
    )}</div></details><p class="comparison-note"><strong>Strict client LSX gate: ${lsxParity.complete ? "pass" : "incomplete"}.</strong> ${number.format(excluded.length)} server-coupled families are explicitly excluded: ${excluded.map(({ label }) => escape(label)).join(" and ")}.</p>`;
}

const lsxSizeMount = document.querySelector("[data-solid-lsx-size]");
if (lsxSizeMount && runtime.lsxApplication) {
  const baseline = runtime.lsxApplication.artifacts.find(
    ({ id }) => id === "solid-lsx-vite",
  );
  const candidate = runtime.lsxApplication.artifacts.find(
    ({ id }) => id === "solidlil-lsx-vite",
  );
  const brotliDelta = (candidate.brotli / baseline.brotli - 1) * 100;
  const performance = runtime.lsxApplication.performance;
  const ratioWithConfidence = (metric) =>
    `${metric.comparison.pointEstimate.toFixed(3)}× [${metric.comparison.confidenceInterval.lower95.toFixed(3)}, ${metric.comparison.confidenceInterval.upper95.toFixed(3)}]`;
  const resourceRows = performance?.statistics
    ? (() => {
        const cpu = performance.statistics.metrics;
        const memory = performance.memoryStatistics.phases;
        const rows = [
          ["Cold mount wall", cpu.coldWall, "ms"],
          ["Warm loop wall", cpu.warmWall, "ms"],
          ["Warm loop CPU", cpu.warmCpu, "ms"],
          ["Cold retained heap", memory.cold.heapUsed, "B"],
          ["Live retained heap", memory.live.heapUsed, "B"],
          ["Post-unmount heap", memory.disposed.heapUsed, "B"],
        ];
        return `<div class="compact-table-wrap"><table><thead><tr><th>Randomized paired resource metric · lower is better</th><th>Official Solid median</th><th>SolidLil median</th><th>Ratio [95% CI]</th></tr></thead><tbody>${rows
          .map(
            ([label, metric, unit]) =>
              `<tr${metric.comparison.pointEstimate < 1 ? ' class="lilscript-row"' : ""}><th>${label}</th><td>${unit === "B" ? number.format(Math.round(metric.baseline.median)) : metric.baseline.median.toFixed(3)} ${unit}</td><td>${unit === "B" ? number.format(Math.round(metric.candidate.median)) : metric.candidate.median.toFixed(3)} ${unit}</td><td>${ratioWithConfidence(metric)}</td></tr>`,
          )
          .join("")}</tbody></table></div>`;
      })()
    : performance
      ? `<div class="compact-table-wrap"><table><thead><tr><th>Resource metric · lower is better</th><th>Official Solid</th><th>SolidLil</th><th>Ratio</th></tr></thead><tbody><tr><th>Median interaction</th><td>${performance.medians.solidLsx.toFixed(3)} ms</td><td>${performance.medians.solidlilLsx.toFixed(3)} ms</td><td>${performance.ratio.toFixed(3)}×</td></tr><tr class="lilscript-row"><th>Live retained heap</th><td>${number.format(performance.retainedMemory.solidLsx)} B</td><td>${number.format(performance.retainedMemory.solidlilLsx)} B</td><td>${performance.memoryRatio.toFixed(3)}×</td></tr><tr class="lilscript-row"><th>Post-unmount heap</th><td>${number.format(performance.disposedMemory.solidLsx)} B</td><td>${number.format(performance.disposedMemory.solidlilLsx)} B</td><td>${performance.disposedMemoryRatio.toFixed(3)}×</td></tr></tbody></table></div>`
      : "";
  lsxSizeMount.innerHTML = `<div class="section-heading"><p class="eyebrow">Current integrated production build</p><h2 id="solidlil-lsx-size-title">SolidLil is ${Math.abs(brotliDelta).toFixed(1)}% ${brotliDelta <= 0 ? "smaller" : "larger"} under Brotli-11</h2><p>This is the same complete client-only LSX fixture used by the differential behavior, resource, and teardown gates. The candidate includes its production DOM host ABI; both lanes are closed-world Vite 8/Oxc application builds.</p></div><div class="compact-table-wrap"><table><thead><tr><th>Artifact</th><th>Brotli-11 · primary</th><th>Gzip-9</th><th>Raw</th></tr></thead><tbody><tr><th>${escape(baseline.label)}</th><td>${number.format(baseline.brotli)} B</td><td>${number.format(baseline.gzip)} B</td><td>${number.format(baseline.raw)} B</td></tr><tr${candidate.brotli <= baseline.brotli ? ' class="lilscript-row"' : ""}><th>${escape(candidate.label)}</th><td>${number.format(candidate.brotli)} B</td><td>${number.format(candidate.gzip)} B</td><td>${number.format(candidate.raw)} B</td></tr></tbody></table></div>${resourceRows}<p class="comparison-note"><strong>Complete client LSX resource gate: ${runtime.lsxApplication.resourceEligible ? "pass" : "fail or unavailable"}.</strong> Hydration and SSR remain separately excluded. <strong>Scope:</strong> ${escape(runtime.lsxApplication.scope)}</p>`;
}
