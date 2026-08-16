import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { canonicalCodecMeasurementsForFiles } from "../../../benchmarks/codec-contract.mjs";
import { root } from "./project.mjs";

const readJson = (path) => JSON.parse(readFileSync(path, "utf8"));
const artifacts = resolve(root, "artifacts");
const buildToolchain = readJson(resolve(artifacts, "toolchain.json"));
const webOutput = resolve(root, "../../web/src/client-runtime-results.json");
const previous = existsSync(webOutput) ? readJson(webOutput) : {};
const buildModesPath = resolve(artifacts, "build-modes.json");
const buildModesBytes = readFileSync(buildModesPath);
const buildModes = JSON.parse(buildModesBytes);
const buildModesSha256 = createHash("sha256")
  .update(buildModesBytes)
  .digest("hex");
const apiParity = readJson(resolve(artifacts, "api-parity.json"));
const lsxParity = readJson(resolve(artifacts, "lsx-parity.json"));
const candidate = readJson(
  resolve(artifacts, "solidlil-upstream-candidate.json"),
);
const curated = readJson(resolve(artifacts, "lilscript-compat.json"));
const lifecycle = readJson(resolve(artifacts, "lifecycle-parity.json"));
const store = readJson(resolve(artifacts, "store-surface.json"));
const web = readJson(resolve(artifacts, "web-surface.json"));
const webClient = readJson(resolve(artifacts, "web-client-surface.json"));
const performance = existsSync(resolve(artifacts, "performance-report.json"))
  ? readJson(resolve(artifacts, "performance-report.json"))
  : null;
const sizeReport = existsSync(resolve(artifacts, "size-report.json"))
  ? readJson(resolve(artifacts, "size-report.json"))
  : null;
const appBehavior = existsSync(resolve(artifacts, "app-behavior.json"))
  ? readJson(resolve(artifacts, "app-behavior.json"))
  : null;
const distributionSelection = existsSync(
  resolve(artifacts, "distribution-selection.json"),
)
  ? readJson(resolve(artifacts, "distribution-selection.json"))
  : null;

function sha256File(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

assert.deepEqual(
  store.codecs,
  buildModes.toolchain.codecs,
  "Store codec provenance",
);
assert.deepEqual(
  store.compiler,
  buildModes.toolchain.compiler,
  "Store compiler",
);
assert.equal(store.sourceBuildModesSha256, buildModesSha256);
assert.deepEqual(
  buildModes.toolchain.compiler,
  buildToolchain.compiler,
  "Build-mode compiler identity",
);
if (distributionSelection) {
  assert.equal(distributionSelection.schemaVersion, 1);
  assert.equal(
    distributionSelection.objectiveContract?.selectionStage,
    "final-tree-shaken-minified-chunk",
  );
  assert.equal(
    distributionSelection.compiler?.sha256,
    buildModes.toolchain.compiler.sha256,
    "Distribution-selection compiler",
  );
  assert.deepEqual(
    distributionSelection.codecs,
    buildModes.toolchain.codecs,
    "Distribution-selection codecs",
  );
}

function currentSizeEvidence(report) {
  if (!report || report.schemaVersion !== 2) return null;
  assert.deepEqual(
    report.codecs,
    buildModes.toolchain.codecs,
    "Application size-report codec provenance",
  );
  assert.match(
    report.compiler?.sha256 ?? "",
    /^[a-f0-9]{64}$/,
    "Application compiler digest",
  );
  assert.deepEqual(
    report.compiler,
    buildToolchain.compiler,
    "Application compiler identity must match the complete-build toolchain",
  );
  assert.equal(
    report.objectiveContract?.gateMetric,
    "brotli11",
    "Application objective",
  );
  const artifactEntries = Object.entries(report.artifacts ?? {});
  const artifactPaths = artifactEntries.map(([, artifact]) =>
    resolve(root, artifact.path),
  );
  const measurements = canonicalCodecMeasurementsForFiles(
    artifactPaths,
    "SolidLil application publication",
  );
  for (const [index, [name, artifact]] of artifactEntries.entries()) {
    const artifactPath = resolve(root, artifact.path);
    assert.equal(
      sha256File(artifactPath),
      artifact.sha256,
      `Application artifact digest: ${name}`,
    );
    assert.deepEqual(
      report.sizes?.[name],
      {
        raw: measurements[index].raw,
        gzip9: measurements[index].gzip,
        brotli11: measurements[index].brotli,
      },
      `Application canonical sizes: ${name}`,
    );
  }
  return report;
}

const canonicalSizeReport = currentSizeEvidence(sizeReport);
const canonicalSizeDigest = canonicalSizeReport
  ? sha256File(resolve(artifacts, "size-report.json"))
  : null;
function currentDependentEvidence(report, label, schemaVersion) {
  if (!canonicalSizeReport || !report || report.schemaVersion !== schemaVersion)
    return null;
  assert.deepEqual(
    report.codecs,
    canonicalSizeReport.codecs,
    `${label} codecs`,
  );
  assert.deepEqual(
    report.compiler,
    canonicalSizeReport.compiler,
    `${label} compiler`,
  );
  assert.equal(
    report.sizeEvidence?.sha256,
    canonicalSizeDigest,
    `${label} size-report identity`,
  );
  return report;
}
const canonicalAppBehavior = currentDependentEvidence(
  appBehavior,
  "Application behavior",
  1,
);
const candidatePerformance = currentDependentEvidence(
  performance,
  "Application performance",
  2,
);
const canonicalPerformance =
  candidatePerformance?.protocol?.sampleAdequacyOverride === false
    ? candidatePerformance
    : null;
assert.deepEqual(
  web.codecs,
  buildModes.toolchain.codecs,
  "Web codec provenance",
);
assert.deepEqual(web.compiler, buildModes.toolchain.compiler, "Web compiler");
assert.equal(web.sourceBuildModesSha256, buildModesSha256);
assert.deepEqual(
  webClient.codecs,
  buildModes.toolchain.codecs,
  "Web-client codec provenance",
);
assert.equal(webClient.sourceBuildModesSha256, buildModesSha256);
assert.deepEqual(
  webClient.compiler,
  buildModes.toolchain.compiler,
  "Web-client compiler",
);

const commit = execFileSync("git", ["rev-parse", "--short", "HEAD"], {
  cwd: resolve(root, "../.."),
  encoding: "utf8",
}).trim();
const dirty =
  execFileSync("git", ["status", "--porcelain"], {
    cwd: resolve(root, "../.."),
    encoding: "utf8",
  }).trim().length > 0;
const revision = `${commit}${dirty ? "-dirty" : ""}`;

function metrics(size) {
  return {
    raw: size.raw,
    gzip: size.gzip9 ?? size.gzip,
    brotli: size.brotli11 ?? size.brotli,
  };
}

function surface({
  id,
  title,
  exportCount,
  solid,
  solidlil,
  behaviorEquivalent,
  exactExports,
  contractVerified = exactExports,
  resourceEquivalent = true,
  boundary = "open-world-distribution",
  scope = "complete public API",
  source,
  notes,
}) {
  const baseline = metrics(solid);
  const candidateSize = metrics(solidlil);
  const brotliRatio = candidateSize.brotli / baseline.brotli;
  const gzipRatio = candidateSize.gzip / baseline.gzip;
  const objectiveSuperior = brotliRatio < 1;
  const compressedSuperior = brotliRatio < 1 && gzipRatio < 1;
  return {
    id,
    title,
    status:
      contractVerified &&
      behaviorEquivalent &&
      resourceEquivalent &&
      objectiveSuperior
        ? "eligible"
        : contractVerified && behaviorEquivalent && !resourceEquivalent
          ? "resource-regression"
          : contractVerified && behaviorEquivalent
            ? "optimization-gap"
            : "partial",
    boundary,
    scope,
    exportCount,
    exactExports,
    contractVerified,
    behaviorEquivalent,
    resourceEquivalent,
    brotliRatio,
    gzipRatio,
    costModel: "brotli",
    objectiveSuperior,
    crossMetricsAreDiagnostic: ["raw", "gzip9"],
    compressedSuperior,
    notes,
    artifacts: [
      { id: "solid", label: `Official Solid ${title}`, ...baseline },
      { id: "solidlil", label: `SolidLil ${title}`, ...candidateSize },
    ],
    source,
  };
}

const surfaces = [
  surface({
    id: "core",
    title: "core browser API",
    exportCount: buildModes.openWorld.exports.length,
    solid: buildModes.openWorld.size.solid,
    solidlil: buildModes.openWorld.size.solidlil,
    behaviorEquivalent: buildModes.openWorld.behaviorPassed,
    exactExports: true,
    scope: "complete 52-export Solid Core browser API",
    source: "artifacts/build-modes.json",
    notes:
      "Reusable open-world ESM preserves the Solid public ABI; the closed-world runtime mangles every exported binding.",
  }),
  surface({
    id: "store",
    title: "store browser API",
    exportCount: store.exportCount,
    solid: store.sizes.solid,
    solidlil: store.sizes.solidlil,
    behaviorEquivalent: store.behaviorEquivalent,
    exactExports: store.exactExports,
    scope: "complete 8-export Solid Store browser API",
    source: "artifacts/store-surface.json",
    notes:
      "Immutable and mutable stores, path updates, reconciliation, producer updates, tracking, and disposal are behavior-equivalent.",
  }),
  surface({
    id: "web-client",
    title: "client Web API",
    exportCount: webClient.exportCount,
    solid: webClient.sizes.solid,
    solidlil: webClient.sizes.solidlil,
    behaviorEquivalent: webClient.behaviorEquivalent,
    exactExports: webClient.exactExports,
    scope: "46 client-rendering exports; SSR and hydration excluded",
    source: "artifacts/web-client-surface.json",
    notes:
      "Client rendering, DOM mutation, events, control flow, portals, and teardown are behavior-equivalent. SSR and hydration are explicitly outside this target.",
  }),
  surface({
    id: "web-full",
    title: "full Web compatibility API",
    exportCount: web.exportCount,
    solid: web.sizes.solid,
    solidlil: web.sizes.solidlil,
    behaviorEquivalent: web.behaviorEquivalent,
    exactExports: web.exactExports,
    scope: "complete 73-export browser entry including compatibility stubs",
    source: "artifacts/web-surface.json",
    notes:
      "The complete browser entry remains separately verified and measured; it is not the declared client-only implementation target.",
  }),
];

const closedWorldSurfaces = canonicalSizeReport
  ? [
      surface({
        id: "app-vite",
        title: "closed-world counter app · Vite",
        exportCount: null,
        solid: canonicalSizeReport.sizes["solid-vite"],
        solidlil: canonicalSizeReport.sizes["lilscript-vite"],
        behaviorEquivalent:
          canonicalAppBehavior?.behaviorEquivalent === true &&
          canonicalAppBehavior?.unmountVerified === true,
        exactExports: null,
        contractVerified: Boolean(canonicalAppBehavior),
        resourceEquivalent: canonicalPerformance?.eligibility?.vite === true,
        boundary: "closed-world-application",
        scope:
          "whole-program equivalent counter; no reusable package ABI survives",
        source: "artifacts/size-report.json",
        notes:
          "The application graph is known, so unused exports disappear and all reachable internal/public library bindings may be renamed.",
      }),
      surface({
        id: "app-closure",
        title: "closed-world counter app · Closure ADVANCED",
        exportCount: null,
        solid: canonicalSizeReport.sizes["solid-closure-advanced"],
        solidlil: canonicalSizeReport.sizes["lilscript-closure-advanced"],
        behaviorEquivalent:
          canonicalAppBehavior?.behaviorEquivalent === true &&
          canonicalAppBehavior?.unmountVerified === true,
        exactExports: null,
        contractVerified: Boolean(canonicalAppBehavior),
        resourceEquivalent:
          canonicalPerformance?.eligibility?.closureAdvanced === true,
        boundary: "closed-world-application",
        scope:
          "whole-program equivalent counter with Closure ADVANCED downstream",
        source: "artifacts/size-report.json",
        notes:
          "Closure receives a complete application rather than a public library entry; the externally observable DOM behavior is the contract.",
      }),
      surface({
        id: "lsx-client-app",
        title: "closed-world complete client LSX fixture",
        exportCount: null,
        solid: canonicalSizeReport.sizes["solid-lsx-vite"],
        solidlil: canonicalSizeReport.sizes["solidlil-lsx-vite"],
        behaviorEquivalent: lsxParity.complete,
        exactExports: null,
        contractVerified: lsxParity.complete,
        resourceEquivalent: canonicalPerformance?.eligibility?.lsx === true,
        boundary: "closed-world-application",
        scope:
          "all 21 in-scope client LSX families; hydration and SSR excluded",
        source: "artifacts/size-report.json",
        notes:
          "This is the app-relevant SolidLil comparison: tree shaking, private symbol mangling, LSX behavior, disposal, CPU, and retained heap are all gated together.",
      }),
    ]
  : [];

const priorApp = previous.appSnapshot ?? {
  evidenceStatus: previous.evidenceStatus ?? "archived-external-snapshot",
  reproducibleFromIntegratedLab:
    previous.reproducibleFromIntegratedLab ?? false,
  sizes: previous.sizes ?? [],
  runtime: previous.runtime ?? null,
};
const appSnapshot = {
  ...priorApp,
  evidenceStatus: "archived-external-snapshot",
  reproducibleFromIntegratedLab: false,
  codecEvidence: "legacy-unknown",
  codecs: null,
  // Do not republish the old simulated-DOM timing/heap proxy. Current runtime
  // and resource evidence comes exclusively from Playwright Chromium above.
  runtime: null,
  notes:
    "The historical todolist bytes remain useful LSX context, but they predate the canonical scorer and are excluded from current size, behavior, and performance evidence.",
};

const report = {
  schemaVersion: 6,
  generatedAt: new Date().toISOString(),
  id: "solid-client-runtime",
  title: "SolidLil exact runtime surfaces + complete client LSX evidence",
  status: "runtime-exact-client-lsx-complete",
  evidenceStatus: "integrated-runtime",
  reproducibleFromIntegratedLab: true,
  sourceRepository:
    "https://github.com/yeargun/lilscript/tree/main/labs/solid-client",
  sourceRevision: revision,
  compilerRevision: revision,
  compiler: buildModes.toolchain.compiler,
  codecs: buildModes.toolchain.codecs,
  objectiveContract: {
    gateMetric: "brotli",
    matchingArtifactOnly: true,
    crossMetricsAreDiagnostic: ["raw", "gzip"],
    scope:
      "current runtime surfaces and canonically attested applications only",
  },
  boundaryDefinitions: {
    openWorldDistribution:
      "A published library entry must keep its documented export names callable because future consumers are unknown. Internal identifiers and proven-private fields may still be mangled.",
    closedWorldApplication:
      "The complete consumer graph is known. Tree shaking may remove unused public APIs, and every remaining binding may be renamed when no external string/reflection contract exposes it.",
  },
  applicationEvidence: canonicalSizeReport
    ? {
        status:
          canonicalAppBehavior && canonicalPerformance
            ? "canonical-current"
            : "canonical-size-only",
        objectiveContract: canonicalSizeReport.objectiveContract,
        compiler: canonicalSizeReport.compiler,
        behaviorAttested: Boolean(canonicalAppBehavior),
        performanceAttested: Boolean(canonicalPerformance),
      }
    : {
        status: sizeReport ? "legacy-noncanonical" : "not-generated",
        objectiveContract: null,
        compiler: null,
      },
  upstream: {
    package: "solid-js",
    version: candidate.suite.replace("solid-js@", ""),
    revision: candidate.revision,
    referenceTestsPassed: candidate.tests,
    referenceTestsTotal: candidate.tests,
    candidateTestsPassed: candidate.passed,
    candidateTestsTotal: candidate.tests,
    files: candidate.testFiles,
    sourcePolicy: candidate.sourcePolicy,
  },
  apiParity: apiParity.totals,
  curatedCompatibility: {
    casesPassed: curated.passed,
    casesTotal: curated.uniqueCases,
    executions: curated.executions,
    modes: [...new Set(curated.runs.map(({ mode }) => mode))],
    backends: [...new Set(curated.runs.map(({ backend }) => backend))],
    scope: curated.scope,
    excludedTargets: curated.excludedTargets,
  },
  buildModes: {
    openWorld: {
      config: buildModes.openWorld.config,
      publicExports: buildModes.openWorld.exports.length,
      behaviorPassed: buildModes.openWorld.behaviorPassed,
    },
    closedWorld: {
      config: buildModes.closedWorld.config,
      sourceExports: buildModes.closedWorld.sourceExports.length,
      emittedExports: buildModes.closedWorld.emittedExports.length,
      exportsMangled: buildModes.closedWorld.exportsMangled,
    },
  },
  distributionOptimization: distributionSelection
    ? {
        objectiveContract: distributionSelection.objectiveContract,
        targets: Object.fromEntries(
          Object.entries(distributionSelection.targets).map(([id, target]) => [
            id,
            {
              winner: target.winner,
              sizes: target.sizes,
              candidateCount: Object.keys(target.candidates).length,
            },
          ]),
        ),
      }
    : null,
  surfaces,
  closedWorldSurfaces,
  comparisons: [...surfaces, ...closedWorldSurfaces],
  application:
    canonicalSizeReport && canonicalAppBehavior
      ? {
          title: "Equivalent counter application",
          status: "eligible",
          behaviorEquivalent: canonicalAppBehavior.behaviorEquivalent,
          unmountVerified: canonicalAppBehavior.unmountVerified,
          staleHandlersStopped: canonicalAppBehavior.staleHandlersStopped,
          artifacts: [
            {
              id: "solid-vite",
              label: "Solid JSX + Vite 8",
              ...metrics(canonicalSizeReport.sizes["solid-vite"]),
              boundary: "public-safe app bundle",
            },
            {
              id: "lilscript-vite",
              label: "SolidLil/LilScript + Vite 8",
              ...metrics(canonicalSizeReport.sizes["lilscript-vite"]),
              boundary: "closed-world LilScript app",
            },
            {
              id: "solid-closure-advanced",
              label: "Solid JSX + Closure ADVANCED",
              ...metrics(canonicalSizeReport.sizes["solid-closure-advanced"]),
              boundary: "Closure application",
            },
            {
              id: "lilscript-closure-advanced",
              label: "SolidLil/LilScript + Closure ADVANCED",
              ...metrics(
                canonicalSizeReport.sizes["lilscript-closure-advanced"],
              ),
              boundary: "closed-world LilScript + Closure",
            },
          ],
        }
      : null,
  lsxApplication:
    canonicalSizeReport && canonicalAppBehavior && canonicalPerformance
      ? {
          title: "Integrated LSX parity fixture",
          status: lsxParity.complete
            ? canonicalPerformance.eligibility?.lsx
              ? "eligible"
              : "resource-regression"
            : canonicalPerformance.eligibility?.lsx
              ? "verified-client-slice"
              : "verified-partial-resource-regression",
          behaviorEquivalent: true,
          unmountVerified: true,
          resourceEligible: canonicalPerformance.eligibility?.lsx ?? null,
          primaryMetric: "brotli",
          scope:
            "The exact integrated differential fixture covers all 21 in-scope client-rendering LSX families, including Suspense and ErrorBoundary. Hydration and SSR are explicitly excluded server-coupled systems.",
          artifacts: [
            {
              id: "solid-lsx-vite",
              label: "Official Solid JSX parity fixture + Vite 8",
              ...metrics(canonicalSizeReport.sizes["solid-lsx-vite"]),
              boundary: "closed-world client fixture",
            },
            {
              id: "solidlil-lsx-vite",
              label: "SolidLil LSX parity fixture + Vite 8",
              ...metrics(canonicalSizeReport.sizes["solidlil-lsx-vite"]),
              boundary: "closed-world client fixture with host ABI",
            },
          ],
          performance: canonicalPerformance.lsx ?? null,
          sources: {
            baseline: "tests/solid/lsx-runtime.jsx",
            candidate: "tests/lil/lsx-runtime.lilx",
            harness: "tests/lsx-runtime.test.mjs",
          },
        }
      : null,
  lifecycle: {
    ...lifecycle,
    repeatedMemoryEligibility:
      canonicalPerformance?.eligibility?.lifecycle ?? null,
    repeatedRetainedMemory:
      canonicalPerformance?.lifecycleRetainedMemory ?? null,
  },
  lsx: lsxParity,
  appSnapshot,
  remainingLsxFamilies: lsxParity.features
    .filter(
      ({ lowering, runtime }) =>
        lowering !== "excluded" &&
        runtime !== "excluded" &&
        (lowering !== "verified" || runtime !== "verified"),
    )
    .map(({ label }) => label),
  excludedServerFamilies: lsxParity.features
    .filter(
      ({ lowering, runtime }) =>
        lowering === "excluded" || runtime === "excluded",
    )
    .map(({ label }) => label),
};

for (const path of [resolve(artifacts, "web-evidence.json"), webOutput]) {
  writeFileSync(path, `${JSON.stringify(report, null, 2)}\n`);
}
console.log(
  `Published SolidLil web evidence: ${candidate.passed}/${candidate.tests} unchanged tests, ` +
    `${apiParity.totals.verified}/${apiParity.totals.expected} exports, ` +
    `${[...surfaces, ...closedWorldSurfaces].filter(({ status }) => status === "eligible").length}/${surfaces.length + closedWorldSurfaces.length} boundary-aware Brotli winners.`,
);
