import { writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { root } from "./project.mjs";

const strict = process.argv.includes("--strict");
const official = {
  core: await import(
    pathToFileURL(resolve(root, "node_modules/solid-js/dist/solid.js")).href
  ),
  web: await import(
    pathToFileURL(resolve(root, "node_modules/solid-js/web/dist/web.js")).href
  ),
  store: await import(
    pathToFileURL(resolve(root, "node_modules/solid-js/store/dist/store.js"))
      .href
  ),
};
const candidate = {
  core: await import(
    pathToFileURL(resolve(root, "packages/solidlil/index.js")).href
  ),
  web: await import(
    pathToFileURL(resolve(root, "packages/solidlil/web.js")).href
  ),
  store: await import(
    pathToFileURL(resolve(root, "packages/solidlil/store.js")).href
  ),
};
const verified = {
  core: new Set(
    Object.keys(
      await import(pathToFileURL(resolve(root, "api/solidlil-core.js")).href),
    ),
  ),
  web: new Set(
    Object.keys(
      await import(pathToFileURL(resolve(root, "api/solidlil-web.js")).href),
    ),
  ),
  store: new Set(
    Object.keys(
      await import(pathToFileURL(resolve(root, "api/solidlil-store.js")).href),
    ),
  ),
};

function auditSurface(name) {
  const expected = Object.keys(official[name]).sort();
  const implemented = Object.keys(candidate[name]).sort();
  const expectedSet = new Set(expected);
  const implementedSet = new Set(implemented);
  const rows = expected.map((exportName) => {
    // solid-js/web reexports its active solid-js condition. A direct Node file
    // import resolves server.js, while this audit targets browser ESM, so
    // shared Web exports use the pinned browser-core value.
    const reference =
      name === "web" && exportName === "effect"
        ? official.core.createRenderEffect
        : name === "web" && exportName in official.core
          ? official.core[exportName]
          : official[name][exportName];
    const implementation = candidate[name][exportName];
    const typeMatches =
      implementedSet.has(exportName) &&
      typeof implementation === typeof reference;
    const arityMatches =
      typeof reference !== "function" ||
      implementation.length === reference.length;
    const contractMatches = typeMatches && arityMatches;
    return {
      name: exportName,
      status: !implementedSet.has(exportName)
        ? "missing"
        : !contractMatches
          ? "contract-mismatch"
          : verified[name].has(exportName)
            ? "verified"
            : "implemented-unverified",
      type: typeof reference,
      candidateType: typeof implementation,
      arity: typeof reference === "function" ? reference.length : null,
      candidateArity:
        typeof implementation === "function" ? implementation.length : null,
      contractMatches,
    };
  });
  const extra = implemented.filter(
    (exportName) => !expectedSet.has(exportName),
  );
  const counts = {
    expected: expected.length,
    implemented: rows.filter((row) => row.status !== "missing").length,
    verified: rows.filter((row) => row.status === "verified").length,
    missing: rows.filter((row) => row.status === "missing").length,
    contractMismatches: rows.filter((row) => row.status === "contract-mismatch")
      .length,
    extra: extra.length,
  };
  return {
    name,
    counts,
    exportParity:
      counts.implemented === counts.expected &&
      counts.extra === 0 &&
      counts.contractMismatches === 0,
    behaviorParity: counts.verified === counts.expected,
    complete:
      counts.implemented === counts.expected &&
      counts.verified === counts.expected &&
      counts.extra === 0 &&
      counts.contractMismatches === 0,
    rows,
    extra,
  };
}

const surfaces = ["core", "web", "store"].map(auditSurface);
const totals = Object.fromEntries(
  [
    "expected",
    "implemented",
    "verified",
    "missing",
    "contractMismatches",
    "extra",
  ].map((metric) => [
    metric,
    surfaces.reduce((sum, surface) => sum + surface.counts[metric], 0),
  ]),
);
const report = {
  generatedAt: new Date().toISOString(),
  baseline: "solid-js@1.9.13 browser ESM exports",
  definition:
    "Complete means exact exports, matching browser value types and function arities, plus differential behavior evidence for every export.",
  complete: surfaces.every((surface) => surface.complete),
  totals,
  surfaces,
};

function markdown() {
  const summary = surfaces
    .map(
      (surface) =>
        `| ${surface.name} | ${surface.counts.expected} | ${surface.counts.implemented} | ${surface.counts.verified} | ${surface.counts.missing} | ${surface.counts.contractMismatches} | ${surface.complete ? "pass" : "incomplete"} |`,
    )
    .join("\n");
  const details = surfaces
    .map((surface) => {
      const missing = surface.rows
        .filter((row) => row.status === "missing")
        .map((row) => `\`${row.name}\``)
        .join(", ");
      const unverified = surface.rows
        .filter((row) => row.status === "implemented-unverified")
        .map((row) => `\`${row.name}\``)
        .join(", ");
      const mismatched = surface.rows
        .filter((row) => row.status === "contract-mismatch")
        .map(
          (row) =>
            `\`${row.name}\` (type ${row.candidateType}/${row.type}, arity ${row.candidateArity}/${row.arity})`,
        )
        .join(", ");
      return `## ${surface.name}\n\n- Missing: ${missing || "none"}\n- Type/arity mismatches: ${mismatched || "none"}\n- Implemented but not yet differentially verified: ${unverified || "none"}\n`;
    })
    .join("\n");
  return `# SolidLil API parity\n\nBaseline: ${report.baseline}. ${report.definition}\n\n| Surface | Expected | Implemented | Verified | Missing | Type/arity mismatches | Gate |\n| --- | ---: | ---: | ---: | ---: | ---: | --- |\n${summary}\n\nOverall exact-parity gate: **${report.complete ? "pass" : "incomplete"}**.\n\n${details}`;
}

function html() {
  const cards = surfaces
    .map(
      (surface) => `<article class="surface">
        <div class="surface-head"><div><p class="label">${surface.name}</p><h2>${surface.counts.verified}<span> / ${surface.counts.expected}</span></h2></div><span class="state ${surface.complete ? "pass" : "pending"}">${surface.complete ? "complete" : "incomplete"}</span></div>
        <div class="meter" aria-label="${surface.name}: ${surface.counts.verified} of ${surface.counts.expected} exports verified"><i style="width:${(surface.counts.verified / surface.counts.expected) * 100}%"></i></div>
        <p>${surface.counts.implemented} implemented · ${surface.counts.missing} missing · ${surface.counts.contractMismatches} type/arity mismatches · ${surface.counts.implemented - surface.counts.verified} awaiting differential evidence</p>
        <details><summary>Export ledger</summary><div class="exports">${surface.rows.map((row) => `<code data-status="${row.status}">${row.name}<small>${row.status.replace("implemented-unverified", "needs evidence")}</small></code>`).join("")}</div></details>
      </article>`,
    )
    .join("");
  return `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="description" content="Exact Solid and SolidLil public API and behavior parity ledger."><title>SolidLil API parity</title><style>
:root{color-scheme:light;--ink:#14201c;--muted:#65706b;--paper:#f4f2ea;--panel:#fffefa;--line:#d5dad4;--green:#067a58;--amber:#a05a12;--red:#a13a31}*{box-sizing:border-box}body{margin:0;background:radial-gradient(circle at 85% 0,#d9f2e7,transparent 34rem),var(--paper);color:var(--ink);font:16px/1.5 Inter,system-ui,sans-serif}main{width:min(1120px,calc(100% - 32px));margin:auto;padding:64px 0 96px}.eyebrow,.label{color:var(--green);font:750 .73rem/1 ui-monospace,monospace;letter-spacing:.14em;text-transform:uppercase}h1{max-width:13ch;margin:.16em 0;font-size:clamp(2.7rem,7vw,5.6rem);line-height:.93;letter-spacing:-.06em}.lead{max-width:67ch;color:var(--muted);font-size:1.08rem}.summary{display:flex;gap:12px;flex-wrap:wrap;margin:28px 0 42px}.summary span{padding:10px 14px;border:1px solid var(--line);border-radius:999px;background:var(--panel);font-variant-numeric:tabular-nums}.grid{display:grid;grid-template-columns:repeat(3,1fr);gap:16px}.surface{min-width:0;padding:22px;border:1px solid var(--line);border-radius:20px;background:var(--panel);box-shadow:0 18px 55px #2648390c}.surface-head{display:flex;align-items:start;justify-content:space-between;gap:12px}.surface h2{margin:.2rem 0 1rem;font-size:2.3rem;letter-spacing:-.05em}.surface h2 span{color:var(--muted);font-size:1rem}.state{padding:6px 9px;border-radius:999px;font:700 .68rem/1 ui-monospace,monospace;text-transform:uppercase}.state.pending{background:#fff1df;color:var(--amber)}.state.pass{background:#dff5eb;color:var(--green)}.meter{height:8px;overflow:hidden;border-radius:999px;background:#e3e7e2}.meter i{display:block;height:100%;background:var(--green)}.surface>p{min-height:3em;color:var(--muted)}details{margin-top:18px;border-top:1px solid var(--line);padding-top:15px}summary{cursor:pointer;font-weight:700}.exports{display:grid;gap:6px;margin-top:13px;max-height:390px;overflow:auto}.exports code{display:flex;justify-content:space-between;gap:8px;padding:8px;border-radius:8px;background:#f4f5f1;font-size:.76rem}.exports code[data-status=verified]{border-left:3px solid var(--green)}.exports code[data-status=implemented-unverified]{border-left:3px solid var(--amber)}.exports code[data-status=contract-mismatch],.exports code[data-status=missing]{border-left:3px solid var(--red)}small{color:var(--muted)}.note{max-width:74ch;margin-top:32px;padding:18px 20px;border-left:3px solid var(--green);background:#ffffff80;color:var(--muted)}@media(max-width:850px){.grid{grid-template-columns:1fr}.surface>p{min-height:0}}@media(max-width:520px){main{padding-top:40px}.exports code{display:block}.exports small{display:block;margin-top:3px}}
</style></head><body><main><p class="eyebrow">Solid 1.9.13 · executable contract</p><h1>Parity is a gate, not a slogan.</h1><p class="lead">Every public core, web, and store export is counted. “Verified” requires matching browser value types and function arities plus differential behavior evidence against Solid; merely exporting a name is not enough.</p><div class="summary"><span>${totals.verified} verified</span><span>${totals.implemented} implemented</span><span>${totals.expected} expected</span><span>${totals.missing} missing</span><span>${totals.contractMismatches} type/arity mismatches</span></div><section class="grid">${cards}</section><p class="note"><strong>Current gate: ${report.complete ? "pass" : "incomplete"}.</strong> Size, performance, and memory wins are reported only for behavior-equivalent surfaces with an exact runtime contract.</p></main></body></html>\n`;
}

writeFileSync(
  resolve(root, "artifacts/api-parity.json"),
  `${JSON.stringify(report, null, 2)}\n`,
);
writeFileSync(
  resolve(root, "../../web/src/solid-api-parity.json"),
  `${JSON.stringify(report, null, 2)}\n`,
);
writeFileSync(resolve(root, "artifacts/api-parity.md"), markdown());
writeFileSync(resolve(root, "artifacts/api-parity.html"), html());
console.log(
  `SolidLil API parity: ${totals.verified}/${totals.expected} verified, ${totals.implemented}/${totals.expected} implemented (${report.complete ? "complete" : "incomplete"}).`,
);
if (strict && !report.complete) process.exitCode = 1;
