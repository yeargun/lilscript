import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { canonicalCodecProvenance } from "../../benchmarks/codec-contract.mjs";

const webRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = resolve(webRoot, "..");
const dataRoot = join(webRoot, "src");
const readJson = async (path) => JSON.parse(await readFile(path, "utf8"));
const githubRoot = "https://github.com/yeargun/lilscript/blob/main/";
const maxSourceBytes = 20_000;
const evidencePaths = {
  compiler: join(dataRoot, "benchmark-results.json"),
  libraries: join(dataRoot, "library-results.json"),
  popular: join(dataRoot, "popular-library-results.json"),
  paired: join(dataRoot, "paired-results.json"),
  scenarios: join(dataRoot, "scenario-results.json"),
  clientRuntime: join(dataRoot, "client-runtime-results.json"),
  motionLab: join(dataRoot, "motion-lab-results.json"),
};

const [
  compiler,
  libraries,
  popular,
  paired,
  scenarios,
  scenarioPackage,
  clientRuntime,
  motionLab,
] = await Promise.all([
  readJson(evidencePaths.compiler),
  readJson(evidencePaths.libraries),
  readJson(evidencePaths.popular),
  readJson(evidencePaths.paired),
  readJson(evidencePaths.scenarios),
  readJson(join(repoRoot, "benchmarks/scenarios/package.json")),
  readJson(evidencePaths.clientRuntime),
  readJson(evidencePaths.motionLab),
]);

const evidence = [
  {
    id: "compiler",
    report: compiler,
    schemaVersion: 2,
    codecs: compiler.metadata?.codecs,
    compiler: compiler.metadata?.compiler,
    objectiveContract: compiler.metadata?.objectiveContract,
  },
  {
    id: "libraries",
    report: libraries,
    schemaVersion: 2,
    codecs: libraries.metadata?.codecs,
    compiler: libraries.metadata?.compiler,
    objectiveContract: libraries.metadata?.objectiveContract,
  },
  {
    id: "popular",
    report: popular,
    schemaVersion: 2,
    codecs: popular.metadata?.codecs,
    compiler: popular.metadata?.compiler,
    objectiveContract: popular.metadata?.objectiveContract,
  },
  {
    id: "paired",
    report: paired,
    schemaVersion: 2,
    codecs: paired.codecs,
    compiler: paired.compiler,
    objectiveContract: paired.objectiveContract,
  },
  {
    id: "scenarios",
    report: scenarios,
    schemaVersion: 2,
    codecs: scenarios.metadata?.codecs,
    compiler: scenarios.metadata?.compiler,
    objectiveContract: scenarios.metadata?.objectiveContract,
  },
  {
    id: "clientRuntime",
    report: clientRuntime,
    schemaVersion: 6,
    codecs: clientRuntime.codecs,
    compiler: clientRuntime.compiler,
    objectiveContract: clientRuntime.objectiveContract,
  },
  {
    id: "motionLab",
    report: motionLab,
    schemaVersion: 1,
    codecs: motionLab.metadata?.codecs,
    compiler: motionLab.metadata?.compiler,
    objectiveContract: motionLab.metadata?.objectiveContract,
  },
];

function requireCanonicalCodecs(codecs, label) {
  if (
    codecs?.implementation !== "lilscript-codec" ||
    codecs?.schemaVersion !== 1 ||
    codecs?.gzip9?.encoder !== "upstream-stock-zlib-c" ||
    codecs?.gzip9?.libraryVersion !== "1.3.1" ||
    codecs?.brotli11?.encoder !== "official-google-brotli-c" ||
    codecs?.brotli11?.libraryVersion !== "1.1.0" ||
    !/^[a-f0-9]{64}$/u.test(codecs?.scorer?.sha256 ?? "")
  ) {
    throw new Error(`${label} is not canonical lilscript-codec evidence`);
  }
}

for (const item of evidence) {
  if (item.report.schemaVersion !== item.schemaVersion) {
    throw new Error(
      `${item.id} evidence schema ${JSON.stringify(item.report.schemaVersion)} is stale; expected ${item.schemaVersion}`,
    );
  }
  if (!item.objectiveContract) {
    throw new Error(`${item.id} evidence lacks an objective/artifact contract`);
  }
  requireCanonicalCodecs(item.codecs, item.id);
  if (!/^[a-f0-9]{64}$/u.test(item.compiler?.sha256 ?? "")) {
    throw new Error(
      `${item.id} evidence lacks an exact compiler binary digest`,
    );
  }
}
const canonicalCodecs = evidence[0].codecs;
const compilerDigest = evidence[0].compiler.sha256;
for (const item of evidence.slice(1)) {
  if (JSON.stringify(item.codecs) !== JSON.stringify(canonicalCodecs)) {
    throw new Error(
      `${item.id} was measured by a different scorer binary; regenerate all publication inputs from one quiescent build`,
    );
  }
  if (item.compiler.sha256 !== compilerDigest) {
    throw new Error(
      `${item.id} was compiled by a different LilScript binary; regenerate all publication inputs from one quiescent build`,
    );
  }
}
const currentCodecs = canonicalCodecProvenance("web benchmark catalog");
if (JSON.stringify(currentCodecs) !== JSON.stringify(canonicalCodecs)) {
  throw new Error(
    "publication inputs do not match the current canonical scorer binary",
  );
}
const currentCompilerPath = join(
  repoRoot,
  `target/release/lilscript${process.platform === "win32" ? ".exe" : ""}`,
);
if (!existsSync(currentCompilerPath)) {
  throw new Error(`current compiler is missing: ${currentCompilerPath}`);
}
const currentCompilerDigest = createHash("sha256")
  .update(await readFile(currentCompilerPath))
  .digest("hex");
if (currentCompilerDigest !== compilerDigest) {
  throw new Error(
    "publication inputs do not match the current LilScript compiler binary",
  );
}

const inputEvidence = Object.fromEntries(
  await Promise.all(
    evidence.map(async ({ id, report }) => {
      const bytes = await readFile(evidencePaths[id]);
      return [
        id,
        {
          path: evidencePaths[id].slice(repoRoot.length + 1),
          sha256: createHash("sha256").update(bytes).digest("hex"),
          schemaVersion: report.schemaVersion,
          compiler: evidence.find((item) => item.id === id).compiler,
          objectiveContract: evidence.find((item) => item.id === id)
            .objectiveContract,
        },
      ];
    }),
  ),
);

function comparisonStats(rows) {
  const deltas = rows
    .map(([candidate, baseline]) => (candidate / baseline - 1) * 100)
    .sort((left, right) => left - right);
  const middle = Math.floor(deltas.length / 2);
  const candidateTotal = rows.reduce((sum, [candidate]) => sum + candidate, 0);
  const baselineTotal = rows.reduce((sum, [, baseline]) => sum + baseline, 0);
  return {
    count: rows.length,
    mean: deltas.reduce((sum, delta) => sum + delta, 0) / deltas.length,
    median:
      deltas.length % 2 === 0
        ? (deltas[middle - 1] + deltas[middle]) / 2
        : deltas[middle],
    wins: deltas.filter((delta) => delta < 0).length,
    ties: deltas.filter((delta) => delta === 0).length,
    losses: deltas.filter((delta) => delta > 0).length,
    candidateTotal,
    baselineTotal,
    aggregate: (candidateTotal / baselineTotal - 1) * 100,
  };
}

function artifact(result, id, collection = "artifacts") {
  const match = result[collection].find((candidate) => candidate.id === id);
  if (!match) throw new Error(`Missing ${id} from ${result.id ?? result.name}`);
  return match;
}

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
  const [
    tool = defaults.tool ?? "Other",
    mode = defaults.mode ?? "unknown",
    properties = defaults.propertyMangling ?? "unknown",
  ] = known[id] ?? [];
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
  const code =
    Buffer.byteLength(contents) <= maxSourceBytes
      ? contents
      : `${contents.slice(0, maxSourceBytes)}\n\n/* Source preview truncated; open the complete file below. */\n`;
  return { path, label, language, code, url: `${githubRoot}${path}` };
}

async function sources(entries) {
  return (await Promise.all(entries.map((entry) => source(...entry)))).filter(
    Boolean,
  );
}

const projects = [];
for (const result of scenarios.results) {
  const packageVersions = result.packages.map((name) => ({
    name,
    version: scenarioPackage.dependencies[name],
  }));
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
    artifacts: result.artifacts.map((artifact) =>
      metricArtifact(artifact, artifact),
    ),
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
    summary:
      "Equivalent readable JavaScript and LilScript application logic; the hand-written artifact is shown only as an optimization oracle.",
    fairness:
      "Closure receives the readable JavaScript reference. LilScript receives the same algorithm and abstraction scope. Every artifact must match the fixed stdout contract before size and load/execution time are measured.",
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
    artifacts: result.surfaceArtifacts.map((artifact) =>
      metricArtifact(artifact),
    ),
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
    [
      "closure",
      `npm + Closure ${result.closureLevel ?? "ADVANCED"}`,
      result.closure,
    ],
    ["vite", "npm + Vite 8", result.vite],
    ["lilscript", "LilScript direct", result.lilscript],
    ["lilscriptVite", "LilScript + Vite 8", result.lilscriptVite],
  ];
  const base = `benchmarks/popular/apps/${result.id}`;
  const demos =
    result.id === "motion"
      ? [
          {
            label: "Motion lab gallery",
            url: "/libraries.html#motion-lab-examples",
          },
          ...motionLab.examples.flatMap((example) => [
            { label: `${example.title} · npm`, url: example.npmUrl },
            { label: `${example.title} · LilScript`, url: example.lilUrl },
          ]),
        ]
      : [];
  projects.push({
    key: `popular:${result.id}`,
    id: result.id,
    title: result.project ?? result.title ?? result.id,
    category: "popular-library",
    status: result.eligible
      ? "eligible"
      : result.exactSurface
        ? "blocked"
        : "partial",
    summary: result.compatibilityNotes,
    fairness: popular.eligibilityRule,
    expected: result.expected ?? null,
    packages: result.packages ?? [],
    blockers: result.blockers ?? [],
    demos,
    artifacts: artifactSpecs
      .map(([id, label, value]) => metricArtifact(value, { id, label }))
      .filter(Boolean),
    sources: await sources([
      [`${base}/js/main.js`, "Installed npm application", "javascript"],
      [`${base}/lil/main.lil`, "LilScript application", "lilscript"],
      [
        `benchmarks/popular/ports/${result.id}/index.lil`,
        "LilScript port entrypoint",
        "lilscript",
      ],
      [
        `benchmarks/popular/ports/${result.id}/entry.lil`,
        "LilScript port entrypoint",
        "lilscript",
      ],
    ]),
  });
}

const solidRuntimeSourceMap = {
  core: [
    [
      "labs/solid-client/apps/lilscript/src/reactive.lil",
      "LilScript reactive runtime",
      "lilscript",
    ],
    [
      "labs/solid-client/packages/solidlil/index.js",
      "Solid-compatible public facade",
      "javascript",
    ],
  ],
  store: [
    [
      "labs/solid-client/packages/solidlil/store.js",
      "SolidLil Store implementation",
      "javascript",
    ],
  ],
  "web-client": [
    [
      "labs/solid-client/api/solidlil-web-client.js",
      "Client-only public export ledger",
      "javascript",
    ],
    [
      "labs/solid-client/packages/solidlil/web.js",
      "SolidLil Web implementation",
      "javascript",
    ],
  ],
  "web-full": [
    [
      "labs/solid-client/packages/solidlil/web.js",
      "SolidLil Web implementation",
      "javascript",
    ],
  ],
};
for (const surface of clientRuntime.surfaces ?? []) {
  projects.push({
    key: `framework:solidlil-${surface.id}`,
    id: `solidlil-${surface.id}`,
    title: `SolidLil ${surface.title}`,
    category: "framework-runtime",
    status:
      surface.status === "eligible"
        ? "eligible"
        : surface.status === "optimization-gap"
          ? "blocked"
          : "partial",
    summary: surface.notes,
    fairness: `The official Solid and SolidLil entries expose the same public names for this declared surface, run the same pinned browser-target behavior gates, and are bundled independently as reusable open-world ESM. Scope: ${surface.scope}. The declared artifact targets Brotli-11; raw and gzip-9 are diagnostics, not extra gates on that Brotli-selected artifact.`,
    expected: `${surface.exportCount} exact exports; ${clientRuntime.upstream.candidateTestsPassed}/${clientRuntime.upstream.candidateTestsTotal} unchanged upstream tests across core/web/store`,
    packages: [
      {
        name: clientRuntime.upstream.package,
        version: clientRuntime.upstream.version,
      },
    ],
    blockers:
      surface.status === "optimization-gap"
        ? [
            `The Brotli-objective gate is open: ratio ${surface.brotliRatio.toFixed(3)}×. Gzip ratio ${surface.gzipRatio.toFixed(3)}× is diagnostic for the Brotli-selected artifact. Behavior remains exact while optimization continues.`,
          ]
        : [],
    demos: [{ label: "SolidLil evidence page", url: "/solidlil.html" }],
    verification: {
      exactExports: surface.exactExports,
      behaviorEquivalent: surface.behaviorEquivalent,
      objectiveSuperior: surface.objectiveSuperior ?? surface.brotliRatio < 1,
      compressedSuperior: surface.compressedSuperior,
      boundary: surface.boundary,
      candidateUpstream: `${clientRuntime.upstream.candidateTestsPassed}/${clientRuntime.upstream.candidateTestsTotal}`,
    },
    artifacts: surface.artifacts.map((value) =>
      metricArtifact(value, {
        tool: "Vite 8 / Oxc",
        mode: "open-world reusable API",
        propertyMangling: "public API preserved",
      }),
    ),
    sources: await sources(solidRuntimeSourceMap[surface.id] ?? []),
  });
}

if (clientRuntime.application) {
  projects.push({
    key: "framework:solidlil-application",
    id: "solidlil-application",
    title: "Solid / SolidLil equivalent counter app",
    category: "framework-runtime",
    status: clientRuntime.application.status,
    summary:
      "The same counter, derived values, batch update, reset, effects, idempotent unmount, and stale-handler contract through Solid JSX and a LilScript-owned DOM application.",
    fairness:
      "This is an application rewrite, not a reusable-runtime API comparison. Compare the two Vite rows together or the two Closure rows together; the LilScript lane may mangle its complete closed world.",
    expected:
      "count, doubled value, parity, batching, reset, document effect, idempotent unmount, and stopped stale handlers",
    packages: [
      {
        name: clientRuntime.upstream.package,
        version: clientRuntime.upstream.version,
      },
    ],
    artifacts: clientRuntime.application.artifacts.map((value) =>
      metricArtifact(value, {
        tool: value.id.includes("closure")
          ? "Closure Compiler"
          : "Vite 8 / Oxc",
        mode: value.boundary,
        propertyMangling: value.id.startsWith("lilscript")
          ? "closed-world"
          : "tool default",
      }),
    ),
    sources: await sources([
      [
        "labs/solid-client/apps/solid/src/main.jsx",
        "Official Solid JSX application",
        "jsx",
      ],
      [
        "labs/solid-client/apps/lilscript/src/main.lil",
        "LilScript application",
        "lilscript",
      ],
    ]),
  });
}

if (clientRuntime.lsxApplication) {
  projects.push({
    key: "framework:solidlil-lsx",
    id: "solidlil-lsx",
    title: "Solid / SolidLil integrated LSX parity fixture",
    category: "framework-runtime",
    status: clientRuntime.lsxApplication.status,
    summary:
      "The current monorepo differential fixture exercises the verified client-only LSX surface and includes SolidLil's production DOM host ABI in the candidate bundle.",
    fairness: clientRuntime.lsxApplication.scope,
    expected:
      "Normalized DOM/state parity, keyed identity, branch churn, idempotent unmount, stopped stale handlers, and zero occupied owner/effect slots after disposal",
    packages: [
      {
        name: clientRuntime.upstream.package,
        version: clientRuntime.upstream.version,
      },
    ],
    blockers: clientRuntime.remainingLsxFamilies,
    exclusions: clientRuntime.excludedServerFamilies,
    demos: [{ label: "SolidLil evidence page", url: "/solidlil.html" }],
    verification: {
      behaviorEquivalent: clientRuntime.lsxApplication.behaviorEquivalent,
      unmountVerified: clientRuntime.lsxApplication.unmountVerified,
      resourceEligible: clientRuntime.lsxApplication.resourceEligible,
      timeRatio: clientRuntime.lsxApplication.performance?.ratio ?? null,
      liveMemoryRatio:
        clientRuntime.lsxApplication.performance?.memoryRatio ?? null,
      disposedMemoryRatio:
        clientRuntime.lsxApplication.performance?.disposedMemoryRatio ?? null,
      integrated: true,
    },
    artifacts: clientRuntime.lsxApplication.artifacts.map((value) =>
      metricArtifact(value, {
        tool: "Vite 8 / Oxc",
        mode: value.boundary,
        propertyMangling: value.id.startsWith("solidlil")
          ? "closed-world"
          : "tool default",
      }),
    ),
    sources: await sources([
      [
        "labs/solid-client/tests/solid/lsx-runtime.jsx",
        "Official Solid JSX parity fixture",
        "jsx",
      ],
      [
        "labs/solid-client/tests/lil/lsx-runtime.lilx",
        "SolidLil LSX parity fixture",
        "lilscript",
      ],
      [
        "labs/solid-client/tests/lsx-runtime.test.mjs",
        "Differential behavior and teardown harness",
        "javascript",
      ],
    ]),
  });
}

for (const result of paired.results) {
  projects.push({
    key: `paired:${result.id}`,
    id: result.id,
    title: result.id
      .split("-")
      .map((part) => part[0].toUpperCase() + part.slice(1))
      .join(" "),
    category: "generated-pair",
    status: "verified",
    summary:
      "Closure JavaScript and LilScript are generated from one neutral typed workload schema.",
    fairness:
      "Both sources come from the same schema and must match the contract in JavaScript, C, and native execution before the Brotli-objective size gate. Raw and gzip are diagnostics for that artifact.",
    expected: result.contract,
    packages: [],
    artifacts: [
      metricArtifact({
        id: "lilscript",
        label: "LilScript",
        ...result.lilscript,
      }),
      metricArtifact({
        id: "closure",
        label: "Closure ADVANCED",
        ...result.closure,
      }),
    ],
    sources: await sources([
      [paired.source, "Neutral workload schema", "json"],
    ]),
  });
}

const artifactCount = projects.reduce(
  (total, project) => total + project.artifacts.length,
  0,
);
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
    evidence: {
      codecs: canonicalCodecs,
      compilerSha256: compilerDigest,
      inputs: inputEvidence,
      objectiveContracts: Object.fromEntries(
        evidence.map(({ id, objectiveContract }) => [id, objectiveContract]),
      ),
    },
  },
  definitions: {
    raw: "UTF-8 JavaScript bytes before transport compression.",
    gzip: "Each JavaScript file compressed independently with gzip level 9.",
    brotli:
      "Each JavaScript file compressed independently with Brotli quality 11.",
    identifier:
      "Local/top-level bindings may be renamed; ordinary public properties stay stable.",
    property:
      "Property renaming is valid only under the row's stated private or closed-world boundary.",
  },
  projects,
};

const marketplace = {
  candidate: { raw: 7957, gzip: 3132, brotli: 2665 },
  baseline: { raw: 8801, gzip: 3132, brotli: 2672 },
  source: "project1-marketplace/docs/BUNDLE_COMPARISON.md",
  evidenceStatus: "historical-noncanonical",
  includedInOverall: false,
};
const solidAppArtifacts = clientRuntime.appSnapshot.sizes;
const solid = artifact(
  { id: "client-runtime", artifacts: solidAppArtifacts },
  "solid-todolist",
);
const solidlil = artifact(
  { id: "client-runtime", artifacts: solidAppArtifacts },
  "solidlil-lsx",
);
const pairedRows = paired.results.map((result) => [
  result.lilscript.brotli,
  result.closure.brotli,
]);
const compilerRows = compiler.results.map((result) => [
  artifact(result, "lilscript").brotli,
  artifact(result, "closure").brotli,
]);
const completePortRows = libraries.results.map((result) => [
  artifact(result, "lilscript", "surfaceArtifacts").brotli,
  Math.min(
    ...result.surfaceArtifacts
      .filter((candidate) => candidate.id !== "lilscript")
      .map((candidate) => candidate.brotli),
  ),
]);
const exactEntrypoints = popular.results
  .filter((result) => result.eligible)
  .map((result) => ({
    id: result.id,
    candidate: result.lilscriptVite.brotli,
    baseline: result.vite.brotli,
  }));
const exactRows = exactEntrypoints.map((result) => [
  result.candidate,
  result.baseline,
]);
const frameworkComparisons =
  clientRuntime.comparisons ?? clientRuntime.surfaces;
const frameworkRuntimeRows = frameworkComparisons.map((surface) => {
  const baseline = artifact(surface, "solid");
  const candidate = artifact(surface, "solidlil");
  return [candidate.brotli, baseline.brotli];
});
const uiRows = [[marketplace.candidate.brotli, marketplace.baseline.brotli]];
const comparisonSummary = {
  metadata: {
    generatedAt: new Date().toISOString(),
    codec: "brotli",
    formula:
      "case-normalized percentage difference; negative values mean smaller",
    selection: "behavior-verified publishable rows only",
    codecs: canonicalCodecs,
    compilerSha256: compilerDigest,
    inputs: inputEvidence,
  },
  overall: comparisonStats([
    ...pairedRows,
    ...compilerRows,
    ...completePortRows,
    ...exactRows,
    ...frameworkRuntimeRows,
  ]),
  smallScripts: {
    paired: comparisonStats(pairedRows),
    compilerWorkloads: comparisonStats(compilerRows),
  },
  packages: {
    completePorts: comparisonStats(completePortRows),
    exactEntrypoints: comparisonStats(exactRows),
    exactRows: exactEntrypoints,
  },
  frameworkRuntime: {
    exactSurfaces: comparisonStats(frameworkRuntimeRows),
    rows: frameworkComparisons.map((surface) => ({
      id: surface.id,
      boundary: surface.boundary,
      candidate: artifact(surface, "solidlil").brotli,
      baseline: artifact(surface, "solid").brotli,
      status: surface.status,
    })),
  },
  apps: {
    marketplace,
    solidLsx: clientRuntime.lsxApplication
      ? {
          candidate: artifact(
            clientRuntime.lsxApplication,
            "solidlil-lsx-vite",
          ),
          baseline: artifact(clientRuntime.lsxApplication, "solid-lsx-vite"),
          status: clientRuntime.lsxApplication.status,
        }
      : null,
    solid: {
      candidate: solidlil,
      baseline: solid,
      timeRatio: clientRuntime.appSnapshot.runtime?.lsxTimeRatio ?? null,
      memoryRatio: clientRuntime.appSnapshot.runtime?.lsxMemoryRatio ?? null,
      status: "historical-noncanonical",
      includedInOverall: false,
      codecs: null,
    },
  },
};

await writeFile(
  join(dataRoot, "benchmark-catalog.json"),
  `${JSON.stringify(catalog, null, 2)}\n`,
);
await writeFile(
  join(dataRoot, "comparison-summary.json"),
  `${JSON.stringify(comparisonSummary, null, 2)}\n`,
);
console.log(
  `Generated ${projects.length} projects, ${artifactCount} artifact rows, and the compact comparison summary.`,
);
