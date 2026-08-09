import { readFile, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const webRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = resolve(webRoot, "..");
const dataRoot = join(webRoot, "src");
const readJson = async (path) => JSON.parse(await readFile(path, "utf8"));
const githubRoot = "https://github.com/yeargun/lilscript/blob/main/";
const maxSourceBytes = 20_000;

const [compiler, libraries, popular, paired, scenarios, scenarioPackage] = await Promise.all([
  readJson(join(dataRoot, "benchmark-results.json")),
  readJson(join(dataRoot, "library-results.json")),
  readJson(join(dataRoot, "popular-library-results.json")),
  readJson(join(dataRoot, "paired-results.json")),
  readJson(join(dataRoot, "scenario-results.json")),
  readJson(join(repoRoot, "benchmarks/scenarios/package.json")),
]);

function lane(id, defaults = {}) {
  const known = {
    reference: ["JavaScript", "unmangled", "off"],
    esbuild: ["esbuild", "identifier", "off"],
    closure: ["Closure Compiler", "property", "closed-world"],
    hand: ["Hand oracle", "specialized", "manual"],
    lilscript: ["LilScript", "property", "closed-world"],
    "lilscript-specialized": ["LilScript", "specialized", "closed-world"],
    vite: ["Vite 8", "identifier", "off"],
    terser: ["Terser", "identifier", "off"],
    rawJs: ["JavaScript", "unmangled", "off"],
    lilscriptVite: ["LilScript + Vite 8", "identifier", "off"],
  };
  const [tool = defaults.tool ?? "Other", mode = defaults.mode ?? "unknown", properties = defaults.propertyMangling ?? "unknown"] = known[id] ?? [];
  return { tool, mode, propertyMangling: properties, ...defaults };
}

function metricArtifact(artifact, defaults = {}) {
  if (!artifact || !Number.isFinite(artifact.raw)) return null;
  return {
    id: artifact.id ?? defaults.id,
    label: artifact.label ?? defaults.label,
    raw: artifact.raw,
    gzip: artifact.gzip,
    brotli: artifact.brotli,
    medianMs: artifact.medianMs ?? null,
    output: artifact.output ?? null,
    ...lane(artifact.id ?? defaults.id, defaults),
  };
}

async function source(path, label, language) {
  const absolute = join(repoRoot, path);
  if (!existsSync(absolute)) return null;
  const contents = await readFile(absolute, "utf8");
  const code = Buffer.byteLength(contents) <= maxSourceBytes
    ? contents
    : `${contents.slice(0, maxSourceBytes)}\n\n/* Source preview truncated; open the complete file below. */\n`;
  return { path, label, language, code, url: `${githubRoot}${path}` };
}

async function sources(entries) {
  return (await Promise.all(entries.map((entry) => source(...entry)))).filter(Boolean);
}

const projects = [];
for (const result of scenarios.results) {
  const packageVersions = result.packages.map((name) => ({ name, version: scenarioPackage.dependencies[name] }));
  projects.push({
    key: `scenario:${result.id}`,
    id: result.id,
    title: result.title,
    category: result.category,
    status: "verified",
    summary: result.summary,
    fairness: scenarios.fairness,
    expected: result.expected,
    packages: packageVersions,
    verification: result.verification,
    artifacts: result.artifacts.map((artifact) => metricArtifact(artifact, artifact)),
    sources: await sources([
      [result.source.javascript, "npm JavaScript application", "javascript"],
      [result.source.lilscript, "LilScript application", "lilscript"],
    ]),
  });
}

for (const result of compiler.results) {
  const base = `benchmarks/apps/cases/${result.name}`;
  projects.push({
    key: `compiler:${result.name}`,
    id: result.name,
    title: result.title,
    category: "compiler-app",
    status: "verified",
    summary: "Equivalent readable JavaScript and LilScript application logic; the hand-written artifact is shown only as an optimization oracle.",
    fairness: "Closure receives the readable JavaScript reference. LilScript receives the same algorithm and abstraction scope. Every artifact must match the fixed stdout contract before size and load/execution time are measured.",
    expected: result.expected,
    packages: [],
    artifacts: result.artifacts.map((artifact) => metricArtifact(artifact)),
    sources: await sources([
      [`${base}/js/main.js`, "Readable JavaScript", "javascript"],
      [`${base}/closure/main.js`, "Closure input", "javascript"],
      [`${base}/lil/main.lil`, "LilScript", "lilscript"],
      [`${base}/hand.js`, "Hand optimization oracle", "javascript"],
    ]),
  });
}

for (const result of libraries.diagnostics) {
  const base = `benchmarks/libraries/apps/${result.id}`;
  projects.push({
    key: `library:${result.id}`,
    id: result.id,
    title: result.title,
    category: "complete-library",
    status: result.eligible ? "eligible" : "blocked",
    summary: result.scope,
    fairness: libraries.eligibilityRule,
    expected: result.expected,
    packages: result.packages,
    blockers: result.blockers,
    artifacts: result.surfaceArtifacts.map((artifact) => metricArtifact(artifact)),
    sources: await sources([
      [`${base}/js/library.js`, "Installed npm public surface", "javascript"],
      [`${base}/js/main.js`, "Checked npm application", "javascript"],
      [`${base}/lil/main.lil`, "Checked LilScript application", "lilscript"],
    ]),
  });
}

for (const result of popular.results) {
  const artifactSpecs = [
    ["rawJs", "Unminified npm bundle", result.rawJs],
    ["terser", "npm + Terser", result.terser],
    ["closure", `npm + Closure ${result.closureLevel ?? "ADVANCED"}`, result.closure],
    ["vite", "npm + Vite 8", result.vite],
    ["lilscript", "LilScript direct", result.lilscript],
    ["lilscriptVite", "LilScript + Vite 8", result.lilscriptVite],
  ];
  const base = `benchmarks/popular/apps/${result.id}`;
  projects.push({
    key: `popular:${result.id}`,
    id: result.id,
    title: result.project ?? result.title ?? result.id,
    category: "popular-library",
    status: result.eligible ? "eligible" : result.exactSurface ? "blocked" : "partial",
    summary: result.compatibilityNotes,
    fairness: popular.eligibilityRule,
    expected: result.expected ?? null,
    packages: result.packages ?? [],
    blockers: result.blockers ?? [],
    artifacts: artifactSpecs.map(([id, label, value]) => metricArtifact(value, { id, label })).filter(Boolean),
    sources: await sources([
      [`${base}/js/main.js`, "Installed npm application", "javascript"],
      [`${base}/lil/main.lil`, "LilScript application", "lilscript"],
      [`benchmarks/popular/ports/${result.id}/index.lil`, "LilScript port entrypoint", "lilscript"],
      [`benchmarks/popular/ports/${result.id}/entry.lil`, "LilScript port entrypoint", "lilscript"],
    ]),
  });
}

for (const result of paired.results) {
  projects.push({
    key: `paired:${result.id}`,
    id: result.id,
    title: result.id.split("-").map((part) => part[0].toUpperCase() + part.slice(1)).join(" "),
    category: "generated-pair",
    status: "verified",
    summary: "Closure JavaScript and LilScript are generated from one neutral typed workload schema.",
    fairness: "Both sources come from the same schema and must match the contract in JavaScript, C, and native execution before the three independent size gates.",
    expected: result.contract,
    packages: [],
    artifacts: [
      metricArtifact({ id: "lilscript", label: "LilScript", ...result.lilscript }),
      metricArtifact({ id: "closure", label: "Closure ADVANCED", ...result.closure }),
    ],
    sources: await sources([[paired.source, "Neutral workload schema", "json"]]),
  });
}

const artifactCount = projects.reduce((total, project) => total + project.artifacts.length, 0);
const catalog = {
  metadata: {
    generatedAt: new Date().toISOString(),
    projectCount: projects.length,
    artifactCount,
    sourcePreviewLimit: maxSourceBytes,
    versions: {
      vite: scenarios.metadata.vite,
      closure: scenarios.metadata.closure,
      node: scenarios.metadata.node,
    },
  },
  definitions: {
    raw: "UTF-8 JavaScript bytes before transport compression.",
    gzip: "Each JavaScript file compressed independently with gzip level 9.",
    brotli: "Each JavaScript file compressed independently with Brotli quality 11.",
    identifier: "Local/top-level bindings may be renamed; ordinary public properties stay stable.",
    property: "Property renaming is valid only under the row's stated private or closed-world boundary.",
  },
  projects,
};

await writeFile(join(dataRoot, "benchmark-catalog.json"), `${JSON.stringify(catalog, null, 2)}\n`);
console.log(`Generated ${projects.length} projects and ${artifactCount} artifact rows.`);
