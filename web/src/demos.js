import { allDemoPairs, demoById, demoGroups, demos, resolveDemo } from "./demos-catalog.js";
import { renderIcons } from "./site.js";

const number = new Intl.NumberFormat("en-US");

function escape(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function escapeSrcdoc(html) {
  return html.replaceAll("&", "&amp;").replaceAll('"', "&quot;");
}

function bytes(value) {
  return Number.isFinite(value) ? `${number.format(value)} B` : "—";
}

function percentDelta(ratioValue) {
  if (!Number.isFinite(ratioValue)) return "—";
  if (Math.abs(ratioValue - 1) < 0.0005) return "tie";
  const percent = (ratioValue - 1) * 100;
  return `${percent < 0 ? "" : "+"}${percent.toFixed(1)}%`;
}

function times(ratioValue) {
  if (!Number.isFinite(ratioValue)) return "—";
  return `${ratioValue.toFixed(2)}×`;
}

function pairRatio(pair) {
  return pair.ratio ?? (pair.candidate.sizes?.brotli / pair.baseline.sizes?.brotli);
}

function selectedId() {
  const params = new URLSearchParams(window.location.search);
  const fromQuery = params.get("id");
  const fromHash = window.location.hash.replace(/^#/, "");
  return fromQuery || fromHash || demos[0].id;
}

function setSelected(id) {
  const url = new URL(window.location.href);
  url.searchParams.delete("id");
  url.hash = id;
  history.replaceState(null, "", url);
}

function labDocument(pair, side) {
  const pane = pair[side];
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>${escape(pane.label)}</title>
<style>
  :root { color: #18211d; background: #f5f7f6; font-family: Inter, ui-sans-serif, system-ui, sans-serif; }
  * { box-sizing: border-box; }
  body { margin: 0; min-height: 100vh; }
  header { padding: 22px 24px 8px; }
  .kicker { margin: 0 0 8px; color: #176b51; font-size: 11px; font-weight: 760; letter-spacing: 0.06em; text-transform: uppercase; }
  h1 { margin: 0; font-size: 26px; letter-spacing: -0.04em; }
  .sizes { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 1px; margin: 18px 22px; background: #d9dfdc; border: 1px solid #d9dfdc; }
  .sizes div { padding: 14px; background: #fff; }
  .sizes b { display: block; font-size: 22px; letter-spacing: -0.04em; }
  .sizes span { color: #5e6b64; font-size: 10px; text-transform: uppercase; }
  .body { margin: 0 22px 24px; color: #5e6b64; font-size: 14px; line-height: 1.6; }
</style>
</head>
<body>
  <header>
    <p class="kicker">${side === "baseline" ? "Baseline" : "LilScript"}</p>
    <h1>${escape(pane.label)}</h1>
  </header>
  <div class="sizes">
    <div><b>${bytes(pane.sizes?.brotli)}</b><span>Brotli-11</span></div>
    <div><b>${bytes(pane.sizes?.gzip)}</b><span>gzip-9</span></div>
    <div><b>${bytes(pane.sizes?.raw)}</b><span>Raw</span></div>
  </div>
  <p class="body">${escape(pair.summary)}</p>
</body>
</html>`;
}

function frameMarkup(pair, side) {
  const pane = pair[side];
  const title = `${pane.label} preview`;
  if (pane.url) {
    return `<iframe title="${escape(title)}" src="${escape(pane.url)}" sandbox="allow-scripts allow-same-origin allow-forms" loading="lazy"></iframe>`;
  }
  return `<iframe title="${escape(title)}" sandbox srcdoc="${escapeSrcdoc(labDocument(pair, side))}"></iframe>`;
}

function metricCell(label, note, candidate, baseline) {
  const ratioValue = candidate / baseline;
  return `<div>
    <span>${escape(label)}</span>
    <strong>${bytes(candidate)}</strong>
    <small>${escape(times(ratioValue))} · ${escape(percentDelta(ratioValue))} vs ${bytes(baseline)}</small>
    <em>${escape(note)}</em>
  </div>`;
}

function ratioLine(pair) {
  const ratioValue = pairRatio(pair);
  const wins =
    Number.isFinite(pair.wins) && Number.isFinite(pair.total)
      ? `${pair.wins}/${pair.total} smaller`
      : percentDelta(ratioValue);
  return `${times(ratioValue)} Brotli · ${wins}`;
}

function renderGallery(activeCardId, groupFilter) {
  const rail = document.querySelector("[data-demo-rail]");
  const visible = demos.filter((demo) => groupFilter === "all" || demo.group === groupFilter);
  const grouped = demoGroups
    .map((group) => ({
      ...group,
      items: visible.filter((demo) => demo.group === group.id),
    }))
    .filter((group) => group.items.length);
  rail.innerHTML = grouped
    .map(
      (group) => `<section>
        <h3>${escape(group.title)}</h3>
        <div class="demo-card-list">${group.items
          .map((demo) => {
            const selected = demo.id === activeCardId ? ' aria-selected="true"' : "";
            return `<button type="button" class="demo-card" data-demo-id="${escape(demo.id)}"${selected}>
              <span>${escape(demo.kicker)}</span>
              <strong>${escape(demo.title)}</strong>
              <p>${escape(demo.summary)}</p>
              <small>${escape(ratioLine(demo))}</small>
            </button>`;
          })
          .join("")}</div>
      </section>`,
    )
    .join("");
}

function renderStage(card, variant) {
  const stage = document.querySelector("[data-demo-stage]");
  const pair = variant
    ? {
        ...card,
        id: variant.id,
        title: variant.title,
        summary: variant.summary ?? card.summary,
        kind: variant.kind,
        baseline: variant.baseline,
        candidate: variant.candidate,
        source: variant.source ?? card.source,
        ratio: pairRatio(variant),
      }
    : card;
  const baseline = pair.baseline.sizes ?? {};
  const candidate = pair.candidate.sizes ?? {};
  const source = pair.source;
  const chips =
    card.variants.length > 0
      ? `<div class="demo-variants" role="tablist" aria-label="Fixtures">${card.variants
          .map((item) => {
            const active = item.id === pair.id ? " active" : "";
            return `<button type="button" class="demo-variant${active}" data-variant-id="${escape(item.id)}" role="tab" aria-selected="${item.id === pair.id}">${escape(item.title)}</button>`;
          })
          .join("")}</div>`
      : "";
  stage.hidden = false;
  stage.innerHTML = `
    <header class="demo-stage-header">
      <div>
        <p class="eyebrow">${escape(card.kicker)}</p>
        <h2>${escape(card.title)}</h2>
        <p>${escape(card.summary)}</p>
      </div>
      <div class="demo-stage-actions">
        ${
          source
            ? `<a class="demo-source" href="${escape(source.href)}" target="_blank" rel="noopener">${escape(source.label)}<i data-lucide="git-branch" aria-hidden="true"></i></a>`
            : ""
        }
      </div>
    </header>
    ${chips}
    <div class="demo-stage-frames">
      <section class="demo-frame">
        <header>
          <span>${escape(pair.baseline.label)}</span>
          ${pair.baseline.url ? `<a href="${escape(pair.baseline.url)}" target="_blank" rel="noopener">Open</a>` : "<span>Lab sheet</span>"}
        </header>
        ${frameMarkup(pair, "baseline")}
      </section>
      <section class="demo-frame">
        <header>
          <span>${escape(pair.candidate.label)}</span>
          ${pair.candidate.url ? `<a href="${escape(pair.candidate.url)}" target="_blank" rel="noopener">Open</a>` : "<span>Lab sheet</span>"}
        </header>
        ${frameMarkup(pair, "candidate")}
      </section>
    </div>
    <div class="demo-metrics" aria-label="Compressed sizes">
      ${metricCell("Brotli-11", "Gated transfer metric", candidate.brotli, baseline.brotli)}
      ${metricCell("gzip-9", "Diagnostic of the same JS", candidate.gzip, baseline.gzip)}
      ${metricCell("Raw", "Diagnostic of the same JS", candidate.raw, baseline.raw)}
    </div>
    <p class="demo-codec">${escape(card.settings.costModel)}</p>`;
  renderIcons(stage);
}

function renderFilters(active) {
  const mount = document.querySelector("[data-demo-filters]");
  const options = [{ id: "all", title: "All" }, ...demoGroups];
  mount.innerHTML = options
    .map(
      (option) =>
        `<button type="button" class="demo-filter${option.id === active ? " active" : ""}" data-group="${option.id}">${escape(option.title)}</button>`,
    )
    .join("");
}

let groupFilter = "all";

function show(id) {
  const { card, variant } = resolveDemo(id);
  const selected = variant?.id ?? card.id;
  setSelected(selected);
  renderGallery(card.id, groupFilter);
  renderStage(card, variant);
  document.querySelector("[data-demo-count]").textContent =
    `${demos.length} cards · ${allDemoPairs().filter((item) => item.kind === "visual").length} live pairs`;
}

const rail = document.querySelector("[data-demo-rail]");
rail.addEventListener("click", (event) => {
  const button = event.target.closest("[data-demo-id]");
  if (!button) return;
  show(button.dataset.demoId);
  document.querySelector("[data-demo-stage]")?.scrollIntoView({ block: "start", behavior: "smooth" });
});

document.querySelector("[data-demo-stage]").addEventListener("click", (event) => {
  const button = event.target.closest("[data-variant-id]");
  if (!button) return;
  show(button.dataset.variantId);
});

document.querySelector("[data-demo-filters]").addEventListener("click", (event) => {
  const button = event.target.closest("[data-group]");
  if (!button) return;
  groupFilter = button.dataset.group;
  renderFilters(groupFilter);
  const { card } = resolveDemo(selectedId());
  const stillVisible = groupFilter === "all" || card.group === groupFilter;
  show(stillVisible ? selectedId() : demos.find((demo) => demo.group === groupFilter).id);
});

window.addEventListener("hashchange", () => show(selectedId()));
renderFilters(groupFilter);
show(selectedId());
if (window.location.hash) {
  document.querySelector("[data-demo-stage]")?.scrollIntoView({ block: "start" });
}
renderIcons();
