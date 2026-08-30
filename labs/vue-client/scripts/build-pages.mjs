import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  buildInventory,
  inventoryPath,
  projectRoot,
  scopePath,
  serializeInventory,
} from "./audit-scope.mjs";
import { evaluateCompletion, sha256 } from "./check-complete.mjs";
import { sourceParityPath } from "./audit-source-parity.mjs";

const webRoot = resolve(projectRoot, "web");
const reportPaths = {
  sourceParity: sourceParityPath,
  compatibility: resolve(projectRoot, "artifacts/compatibility-report.json"),
  size: resolve(projectRoot, "artifacts/project-size-report.json"),
  performance: resolve(projectRoot, "artifacts/performance-report.json"),
};

function readOptionalReport(path) {
  if (!existsSync(path)) return { bytes: null, report: null };
  const bytes = readFileSync(path);
  try {
    return { bytes, report: JSON.parse(bytes) };
  } catch (error) {
    return { bytes, report: { invalidJson: error.message } };
  }
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function statusClass(status) {
  return status === "complete" || status === "passed" ? "pass" : "open";
}

function reportReference(path, entry) {
  if (!entry.bytes) return { path: relative(projectRoot, path), present: false };
  return {
    path: relative(projectRoot, path),
    present: true,
    sha256: sha256(entry.bytes),
    schemaVersion: entry.report?.schemaVersion ?? null,
  };
}

function pageHtml(scope, inventory, evidence) {
  const packageRows = scope.packages
    .map((entry) => {
      const audited = inventory.packages.find(({ name }) => name === entry.name);
      return `<tr><td><code>${escapeHtml(entry.name)}</code></td><td>${audited.sourceFiles.length}</td><td>${audited.testFiles.length}</td><td>${audited.publicExports.length}</td><td><span class="status ${statusClass(entry.status)}">${escapeHtml(entry.status)}</span></td></tr>`;
    })
    .join("\n");
  const blockerItems = evidence.blockers
    .map((blocker) => `<li>${escapeHtml(blocker)}</li>`)
    .join("\n");
  const reportRows = Object.entries(evidence.reports)
    .map(
      ([name, report]) =>
        `<tr><td>${escapeHtml(name)}</td><td>${report.present ? `<code>${escapeHtml(report.sha256.slice(0, 12))}</code>` : "absent"}</td><td>${report.present ? "present" : "pending"}</td></tr>`,
    )
    .join("\n");
  const gateRows = Object.entries(scope.gates)
    .map(
      ([name, value]) =>
        `<tr><td>${escapeHtml(name)}</td><td><span class="status ${statusClass(value)}">${escapeHtml(value)}</span></td></tr>`,
    )
    .join("\n");
  const bundleScenarioRows = scope.bundleScenarios
    .map(
      (entry) => {
        const measured = evidence.sizeScenarios.find(({ id }) => id === entry.id);
        const sizes = measured
          ? `${measured.candidateBrotli11.toLocaleString()} / ${measured.upstreamBrotli11.toLocaleString()} B`
          : "pending";
        const delta = measured ? `${measured.deltaBrotli11 >= 0 ? "+" : ""}${measured.deltaBrotli11.toLocaleString()} B` : "-";
        return `<tr><td><code>${escapeHtml(entry.id)}</code></td><td>${entry.completionRequired ? "required" : "diagnostic"}</td><td>${escapeHtml(entry.description)}</td><td>${sizes}</td><td>${delta}</td><td><span class="status ${statusClass(entry.status)}">${escapeHtml(entry.status)}</span></td></tr>`;
      },
    )
    .join("\n");
  const performanceRows = evidence.performanceWorkloads
    .map((entry) => `<tr><td><code>${escapeHtml(entry.id)}</code></td><td>${escapeHtml(entry.category)}</td><td>${entry.ratio.toFixed(3)}x</td><td>${entry.upper95.toFixed(3)}x</td><td>${entry.margin.toFixed(2)}x</td><td><span class="status ${entry.passed ? "pass" : "open"}">${entry.passed ? "passed" : "failed"}</span></td></tr>`)
    .join("\n");
  const artifactRows = evidence.runtimeOnlyModules
    .map((entry) => `<tr><td><code>${escapeHtml(entry.name)}</code></td><td>${entry.candidateBytes.toLocaleString()} B</td><td>${entry.upstreamBytes.toLocaleString()} B</td><td>+${(entry.candidateBytes - entry.upstreamBytes).toLocaleString()} B</td></tr>`)
    .join("\n");
  const headline = evidence.complete
    ? "Complete evidence set"
    : "Compatibility and project evidence are incomplete";
  const summary = evidence.complete
    ? "Every scoped gate is backed by the machine-readable evidence listed below."
    : "No full Vue compatibility, required-project size win, or performance win is claimed while these gates remain open. A diagnostic result does not satisfy the final project-size gate.";

  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="description" content="Machine-backed evidence for the VueLil 3.5.42 compatibility laboratory.">
  <title>VueLil evidence</title>
  <link rel="stylesheet" href="./styles.css">
</head>
<body>
  <header class="hero">
    <p class="eyebrow">VueLil laboratory / pinned audit</p>
    <h1>${headline}</h1>
    <p class="lede">${summary}</p>
    <div class="pin"><span>Vue ${escapeHtml(inventory.upstream.version)}</span><code>${escapeHtml(inventory.upstream.revision)}</code></div>
  </header>
  <main>
    <section class="summary-grid" aria-label="Audit summary">
      <article><strong>${inventory.totals.packages}</strong><span>scoped packages</span></article>
      <article><strong>${evidence.sourceParity.satisfied}/${inventory.totals.sourceFiles}</strong><span>verified source files</span></article>
      <article><strong>${scope.gates.candidatePassed}/${inventory.totals.upstreamTestFiles + inventory.totals.declarationTestFiles}</strong><span>test files passed</span></article>
      <article><strong>${inventory.totals.publicExports}</strong><span>export names audited</span></article>
    </section>
    <section>
      <div class="section-heading"><p>Current truth</p><h2>Open gates</h2></div>
      ${evidence.complete ? '<p class="notice pass-note">No open gates.</p>' : `<ol class="blockers">${blockerItems}</ol>`}
    </section>
    <section>
      <div class="section-heading"><p>Coverage</p><h2>Packages from the pinned tree</h2></div>
      <div class="table-wrap"><table><thead><tr><th>Package</th><th>Source</th><th>Tests</th><th>Exports</th><th>Scope status</th></tr></thead><tbody>${packageRows}</tbody></table></div>
    </section>
    <section>
      <div class="section-heading"><p>Project bundles</p><h2>Paired application scenarios</h2></div>
      <p class="section-copy">Each scenario builds the same application source against upstream Vue and VueLil with identical production settings. Published library bundles are not completion evidence.</p>
      <div class="table-wrap"><table><thead><tr><th>Scenario</th><th>Role</th><th>Application</th><th>Brotli candidate / Vue</th><th>Delta</th><th>Status</th></tr></thead><tbody>${bundleScenarioRows}</tbody></table></div>
    </section>
    <section>
      <div class="section-heading"><p>Runtime cost</p><h2>Paired performance confidence bounds</h2></div>
      <p class="section-copy">Ratios are candidate duration divided by Vue duration. Passing requires the one-sided 95% upper bound to stay at or below the configured non-inferiority margin.</p>
      <div class="table-wrap"><table><thead><tr><th>Workload</th><th>Category</th><th>Point ratio</th><th>Upper 95%</th><th>Margin</th><th>Status</th></tr></thead><tbody>${performanceRows}</tbody></table></div>
    </section>
    <section>
      <div class="section-heading"><p>Bundle anatomy</p><h2>Why the runtime-only bundle is larger</h2></div>
      <p class="section-copy">All ${evidence.sourceParity.mapped} runtime sources are explicitly written in LilScript; ${evidence.sourceParity.declarationOnly} type-only files use audited declaration-only handling. Seven JavaScript adapters expose host primitives only and their retained bytes are counted. Production compilation enables identifier and property mangling while preserving public export names. The current diagnostic still retains substantially more generated runtime code, primarily dynamic <code>JsValue</code> access, host-boundary calls, module initialization, and compatibility paths.</p>
      <div class="table-wrap"><table><thead><tr><th>Module family</th><th>VueLil rendered</th><th>Vue rendered</th><th>Raw delta</th></tr></thead><tbody>${artifactRows}</tbody></table></div>
      <p class="notice">The next valid measurement must consume one reusable, open-world mangled production package. Module audits reject scenario-specific candidate paths, and Vite performs downstream application tree shaking. The checked-in report predates this stricter requirement and remains diagnostic.</p>
    </section>
    <section class="split">
      <div><div class="section-heading"><p>Declared state</p><h2>Scope gates</h2></div><table><thead><tr><th>Gate</th><th>Value</th></tr></thead><tbody>${gateRows}</tbody></table></div>
      <div><div class="section-heading"><p>Backing files</p><h2>Evidence reports</h2></div><table><thead><tr><th>Report</th><th>SHA-256</th><th>State</th></tr></thead><tbody>${reportRows}</tbody></table></div>
    </section>
    <section class="contract">
      <div class="section-heading"><p>Completion rule</p><h2>No partial credit</h2></div>
      <p>${escapeHtml(scope.claimRule)}</p>
      <p>Inspect the complete source-derived inventory in <a href="./evidence.json">evidence.json</a>. Generated presentation text is not itself a benchmark or compatibility result.</p>
      <p>Read <a href="./brotli-explained.html">Why VueLil is larger after Brotli</a> for a visual source-to-bundle explanation, or inspect the <a href="https://github.com/yeargun/vuelil/blob/main/artifacts/brotli-regression-report.md">machine-oriented report</a>.</p>
      <p>Use the concrete <a href="./size-migration-plan.html">Before → After size migration plan</a> for source, build, and future language changes.</p>
      <p>Open the exact report-pinned <a href="./bundles/">minified Vue and VueLil JavaScript bundles</a> side by side.</p>
    </section>
  </main>
  <footer>Static evidence generated only from <code>compatibility/scope.json</code>, <code>compatibility/inventory.json</code>, and identified machine reports.</footer>
</body>
</html>
`;
}

const styles = `:root {
  color-scheme: dark;
  --ink: #e9f6ef;
  --muted: #9db4aa;
  --line: #29473b;
  --panel: #10271f;
  --accent: #6ee7a8;
  --warning: #ffc96b;
  background: #07130f;
  color: var(--ink);
  font-family: ui-monospace, "SFMono-Regular", Consolas, monospace;
}
* { box-sizing: border-box; }
body { margin: 0; background: radial-gradient(circle at 80% 0, #153d2d 0, transparent 32rem), #07130f; }
.hero, main, footer { width: min(1120px, calc(100% - 2rem)); margin-inline: auto; }
.hero { padding: 5rem 0 3rem; border-bottom: 1px solid var(--line); }
.eyebrow, .section-heading p { margin: 0 0 .55rem; color: var(--accent); font-size: .75rem; letter-spacing: .16em; text-transform: uppercase; }
h1 { max-width: 850px; margin: 0; font-family: Georgia, serif; font-size: clamp(2.8rem, 8vw, 6.8rem); font-weight: 400; line-height: .94; letter-spacing: -.055em; }
.lede { max-width: 760px; margin: 1.6rem 0; color: var(--muted); font-family: system-ui, sans-serif; font-size: 1.08rem; line-height: 1.7; }
.pin { display: flex; flex-wrap: wrap; gap: .7rem 1.5rem; color: var(--muted); }
main { padding: 1rem 0 5rem; }
section { padding: 3rem 0; border-bottom: 1px solid var(--line); }
.summary-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 1px; padding: 1px; background: var(--line); }
.summary-grid article { min-height: 150px; padding: 1.25rem; background: var(--panel); display: flex; flex-direction: column; justify-content: space-between; }
.summary-grid strong { color: var(--accent); font-family: Georgia, serif; font-size: 3rem; font-weight: 400; }
.summary-grid span { color: var(--muted); font-size: .78rem; text-transform: uppercase; }
.section-heading { margin-bottom: 1.5rem; }
h2 { margin: 0; font-family: Georgia, serif; font-size: clamp(1.8rem, 4vw, 3rem); font-weight: 400; }
.blockers { margin: 0; padding: 0; list-style-position: inside; columns: 2; column-gap: 2rem; }
.blockers li { break-inside: avoid; padding: .65rem 0; color: var(--muted); font-family: system-ui, sans-serif; border-bottom: 1px dotted var(--line); }
.table-wrap { overflow-x: auto; }
table { width: 100%; border-collapse: collapse; font-size: .82rem; }
th, td { padding: .85rem .7rem; border-bottom: 1px solid var(--line); text-align: left; }
th { color: var(--muted); font-size: .7rem; letter-spacing: .08em; text-transform: uppercase; }
.status { color: var(--warning); }
.status.pass, .pass-note { color: var(--accent); }
.split { display: grid; grid-template-columns: 1fr 1fr; gap: 3rem; }
.contract p { max-width: 850px; color: var(--muted); font-family: system-ui, sans-serif; line-height: 1.75; }
.section-copy { max-width: 850px; color: var(--muted); font-family: system-ui, sans-serif; line-height: 1.7; }
.notice { max-width: 850px; margin-top: 1.5rem; padding: 1rem; border-left: 3px solid var(--warning); background: var(--panel); color: var(--muted); font-family: system-ui, sans-serif; line-height: 1.6; }
a { color: var(--accent); }
footer { padding: 2rem 0 4rem; color: var(--muted); font-size: .72rem; line-height: 1.7; }
@media (max-width: 720px) {
  .hero { padding-top: 3rem; }
  .summary-grid { grid-template-columns: 1fr 1fr; }
  .blockers { columns: 1; }
  .split { grid-template-columns: 1fr; }
}
`;

export function buildPages() {
  const scopeBytes = readFileSync(scopePath);
  const inventoryBytes = readFileSync(inventoryPath);
  const scope = JSON.parse(scopeBytes);
  const inventory = JSON.parse(inventoryBytes);
  if (serializeInventory(buildInventory()) !== inventoryBytes.toString()) {
    throw new Error("compatibility/inventory.json is stale; run npm run audit:scope");
  }
  const reports = Object.fromEntries(
    Object.entries(reportPaths).map(([name, path]) => [name, readOptionalReport(path)]),
  );
  const result = evaluateCompletion({
    scope,
    inventory,
    sourceParity: reports.sourceParity.report,
    scopeDigest: sha256(scopeBytes),
    inventoryDigest: sha256(inventoryBytes),
    evidence: Object.fromEntries(
      Object.entries(reports).map(([name, entry]) => [name, entry.report]),
    ),
    requirePagesEvidence: false,
  });
  const evidence = {
    schemaVersion: 1,
    complete: result.complete,
    upstream: inventory.upstream,
    scope: {
      path: relative(projectRoot, scopePath),
      sha256: sha256(scopeBytes),
    },
    inventory: {
      path: relative(projectRoot, inventoryPath),
      sha256: sha256(inventoryBytes),
      totals: inventory.totals,
    },
    reports: Object.fromEntries(
      Object.entries(reports).map(([name, entry]) => [
        name,
        reportReference(reportPaths[name], entry),
      ]),
    ),
    sourceParity: {
      satisfied: reports.sourceParity.report?.totals?.satisfiedFiles ?? 0,
      required: reports.sourceParity.report?.totals?.upstreamFiles ?? inventory.totals.sourceFiles,
      mapped: reports.sourceParity.report?.totals?.mappedFiles ?? 0,
      declarationOnly: reports.sourceParity.report?.totals?.declarationOnlyFiles ?? 0,
    },
    sizeScenarios: (reports.size.report?.scenarios ?? []).map((entry) => ({
      id: entry.id,
      candidateBrotli11: entry.candidate?.sizes?.brotli11 ?? 0,
      upstreamBrotli11: entry.upstream?.sizes?.brotli11 ?? 0,
      deltaBrotli11: entry.comparison?.deltaBytes?.brotli11 ?? 0,
      passed: entry.passed === true,
    })),
    performanceWorkloads: (reports.performance.report?.workloads ?? []).map((entry) => ({
      id: entry.id,
      category: entry.category,
      ratio: entry.statistics?.ratio?.pointEstimate ?? 0,
      upper95: entry.statistics?.ratio?.confidenceInterval?.upper95 ?? 0,
      margin: entry.statistics?.ratio?.nonInferiorityMarginRatio ?? 0,
      passed: entry.passed === true,
    })),
    runtimeOnlyModules: [
      ["reactivity", "reactivity.project.generated.js", "reactivity.esm-bundler.js"],
      ["runtime-core", "runtime-core.project.generated.js", "runtime-core.esm-bundler.js"],
      ["runtime-dom", "runtime-dom.project.generated.js", "runtime-dom.esm-bundler.js"],
      ["shared", "shared.project.generated.js", "shared.esm-bundler.js"],
    ].map(([name, candidateSuffix, upstreamSuffix]) => {
      const scenario = (reports.size.report?.scenarios ?? []).find(({ id }) => id === "runtime-only-client");
      const candidate = scenario?.moduleAudit?.candidate?.graph?.modules?.find(({ id }) => id.endsWith(candidateSuffix));
      const upstream = scenario?.moduleAudit?.upstream?.graph?.modules?.find(({ id }) => id.endsWith(upstreamSuffix));
      return { name, candidateBytes: candidate?.renderedBytes ?? 0, upstreamBytes: upstream?.renderedBytes ?? 0 };
    }),
    blockers: result.failures,
  };
  mkdirSync(webRoot, { recursive: true });
  writeFileSync(resolve(webRoot, "evidence.json"), `${JSON.stringify(evidence, null, 2)}\n`);
  writeFileSync(resolve(webRoot, "index.html"), pageHtml(scope, inventory, evidence));
  writeFileSync(resolve(webRoot, "styles.css"), styles);
  console.log(
    `Built ${relative(projectRoot, webRoot)} from machine-readable evidence (${evidence.complete ? "complete" : `${evidence.blockers.length} open checks`}).`,
  );
  return evidence;
}

function isMain() {
  return process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}

if (isMain()) buildPages();
