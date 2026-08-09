import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const labRoot = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(labRoot, "../..");
const manifest = JSON.parse(
  readFileSync(join(labRoot, "compatibility/libraries.json"), "utf8"),
);
const packageJson = JSON.parse(readFileSync(join(labRoot, "package.json"), "utf8"));
const measured = JSON.parse(readFileSync(join(labRoot, "build/results.json"), "utf8"));
const performance = JSON.parse(
  readFileSync(join(labRoot, "build/performance-memory.json"), "utf8"),
);

const materialRegressionLimit = 1.05;
const targets = new Map(manifest.targets.map((target) => [target.id, target]));
const commit = execFileSync("git", ["rev-parse", "--short", "HEAD"], {
  cwd: repoRoot,
  encoding: "utf8",
}).trim();
const dirty = execFileSync("git", ["status", "--porcelain"], {
  cwd: repoRoot,
  encoding: "utf8",
}).trim().length > 0;

const results = measured.map((measuredRow) => {
  let row = measuredRow;
  let solidPerformance = null;
  let externalCompatibilityNotes = null;
  const rowSource = row.source
    ? (isAbsolute(row.source) ? row.source : join(repoRoot, row.source))
    : null;
  if (row.id === "solid-js" && rowSource && existsSync(rowSource)) {
    const sizeReport = JSON.parse(readFileSync(rowSource, "utf8"));
    const normalize = (size) => ({
      raw: size.raw,
      gzip: size.gzip9 ?? size.gzip,
      brotli: size.brotli11 ?? size.brotli,
    });
    row = {
      ...row,
      vite: normalize(sizeReport.sizes["solid-todolist"]),
      lilscriptVite: normalize(sizeReport.sizes["solidlil-todolist"]),
      comparisons: sizeReport.comparisons?.todolistLilx ?? row.comparisons,
    };
    const performancePath = join(dirname(rowSource), "performance-report.json");
    if (existsSync(performancePath)) {
      const report = JSON.parse(readFileSync(performancePath, "utf8"));
      solidPerformance = {
        performance: {
          npmMs: report.medians.solid.total,
          lilscriptMs: report.medians.solidlilLsx.total,
          ratio: report.ratios.lsx,
        },
        retainedMemory: {
          npmBytes: report.retainedMemory?.solid ?? null,
          lilscriptBytes: report.retainedMemory?.solidlilLsx ?? null,
          ratio: report.memoryRatios?.lsx ?? null,
        },
      };
      const solid = sizeReport.sizes["solid-todolist"];
      const lsx = sizeReport.sizes["solidlil-todolist"];
      const babel = sizeReport.sizes["solidlil-babel-todolist"];
      const solidCore = sizeReport.sizes["solid-core-min"];
      const lilCore = sizeReport.sizes["solidlil-core-min"];
      externalCompatibilityNotes =
        `Measured in lilscript-solid-lab; this is not a complete Solid replacement. ` +
        `Fresh Vite/oxc app JS: Solid ${solid.raw} raw / ${solid.brotli11} brotli, ` +
        `solidlil LSX ${lsx.raw} / ${lsx.brotli11}, and identical-JSX reactive-swap ` +
        `${babel.raw} / ${babel.brotli11}. Isolated ratios: LSX time ` +
        `${report.ratios.lsx.toFixed(3)} and retained heap ${report.memoryRatios.lsx.toFixed(3)}; ` +
        `identical-JSX time ${report.ratios.babel.toFixed(3)} and retained heap ` +
        `${report.memoryRatios.babel.toFixed(3)}. Used-core bundle: Solid ` +
        `${solidCore.raw} / ${solidCore.brotli11}, solidlil ${lilCore.raw} / ${lilCore.brotli11}; ` +
        `all three measured surfaces pass their strict gates.`;
    }
  }
  const target = targets.get(row.id);
  if (!target) throw new Error(`missing compatibility target for ${row.id}`);
  const runtime = performance.results[row.id] ?? solidPerformance;
  const performanceGate = runtime
    && runtime.performance.ratio != null
    && runtime.retainedMemory.ratio != null
    ? runtime.performance.ratio <= materialRegressionLimit &&
      runtime.retainedMemory.ratio <= materialRegressionLimit
    : null;
  const exactSurface =
    target.status.startsWith("exact-");
  const sizeGate = row.vite && row.lilscriptVite
    ? row.lilscriptVite.raw <= Math.min(row.vite.raw, row.closure.raw) &&
      row.lilscriptVite[row.costModel ?? "brotli"] <=
        Math.min(
          row.vite[row.costModel ?? "brotli"],
          row.closure[row.costModel ?? "brotli"],
        )
    : null;
  return {
    ...row,
    status: target.status,
    packages: target.packages.map((name, index) => ({
      name,
      version: target.versions[index] ?? target.versions[0],
    })),
    entrypoint: target.entrypoint ?? null,
    publicRuntimeApi: target.publicRuntimeApi ?? [],
    compatibilityNotes:
      externalCompatibilityNotes ?? target.notes ?? target.reason ?? "",
    performance: runtime,
    exactSurface,
    performanceGate,
    sizeGate,
    eligible: exactSurface && performanceGate === true && sizeGate === true,
  };
});

const report = {
  metadata: {
    generatedAt: new Date().toISOString(),
    compilerRevision: `${commit}${dirty ? "-dirty" : ""}`,
    node: process.version,
    vite: packageJson.devDependencies.vite,
    esbuild: packageJson.devDependencies.esbuild,
    terser: packageJson.devDependencies.terser,
    closure: packageJson.devDependencies["google-closure-compiler"],
    performanceRounds: performance.rounds,
    materialRegressionLimit,
  },
  eligibilityRule: manifest.eligibilityRule,
  results,
};

writeFileSync(
  join(repoRoot, "web/src/popular-library-results.json"),
  `${JSON.stringify(report, null, 2)}\n`,
);

const solidResult = measured.find((row) => row.id === "solid-js");
const solidSource = solidResult?.source
  ? (isAbsolute(solidResult.source) ? solidResult.source : join(repoRoot, solidResult.source))
  : null;
if (solidSource && existsSync(solidSource)) {
  const performancePath = join(dirname(solidSource), "performance-report.json");
  const clientRuntimePath = join(repoRoot, "web/src/client-runtime-results.json");
  if (existsSync(performancePath) && existsSync(clientRuntimePath)) {
    const sizeReport = JSON.parse(readFileSync(solidSource, "utf8"));
    const performanceReport = JSON.parse(readFileSync(performancePath, "utf8"));
    const clientRuntime = JSON.parse(readFileSync(clientRuntimePath, "utf8"));
    const sizeSources = {
      "solid-todolist": "solid-todolist",
      "solidlil-lsx": "solidlil-todolist",
      "solidlil-babel": "solidlil-babel-todolist",
      "solid-core": "solid-core-min",
      "solidlil-core": "solidlil-core-min",
    };
    const solidRoot = dirname(solidSource);
    const solidRevision = execFileSync("git", ["rev-parse", "--short", "HEAD"], {
      cwd: solidRoot,
      encoding: "utf8",
    }).trim();
    const solidDirty = execFileSync("git", ["status", "--porcelain"], {
      cwd: solidRoot,
      encoding: "utf8",
    }).trim().length > 0;
    clientRuntime.sourceRevision = `${solidRevision}${solidDirty ? "-dirty" : ""}`;
    clientRuntime.compilerRevision = report.metadata.compilerRevision;
    clientRuntime.sizes = clientRuntime.sizes.map((size) => {
      const measuredSize = sizeReport.sizes[sizeSources[size.id]];
      return measuredSize
        ? {
            ...size,
            raw: measuredSize.raw,
            gzip: measuredSize.gzip9,
            brotli: measuredSize.brotli11,
          }
        : size;
    });
    clientRuntime.runtime = {
      environment: performanceReport.environment,
      samples: performanceReport.samples,
      memorySamples: performanceReport.memorySamples,
      solidMedianMs: performanceReport.medians.solid.total,
      lsxMedianMs: performanceReport.medians.solidlilLsx.total,
      babelMedianMs: performanceReport.medians.solidlilBabel.total,
      lsxTimeRatio: performanceReport.ratios.lsx,
      babelTimeRatio: performanceReport.ratios.babel,
      solidRetainedBytes: performanceReport.retainedMemory.solid,
      lsxRetainedBytes: performanceReport.retainedMemory.solidlilLsx,
      babelRetainedBytes: performanceReport.retainedMemory.solidlilBabel,
      lsxMemoryRatio: performanceReport.memoryRatios.lsx,
      babelMemoryRatio: performanceReport.memoryRatios.babel,
    };
    writeFileSync(clientRuntimePath, `${JSON.stringify(clientRuntime, null, 2)}\n`);
  }
}

console.log(
  `Published ${results.length} popular-library rows (${results.filter((row) => row.eligible).length} eligible) to web/src/popular-library-results.json`,
);
