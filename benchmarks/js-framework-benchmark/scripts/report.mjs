import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { benchmarkRoot, metadataPath } from "./paths.mjs";
import { geometricMean, hashString, summarize } from "./measurement-utils.mjs";

const artifactsRoot = resolve(benchmarkRoot, "artifacts");
const checkpointArg = process.argv.includes("--checkpoint")
  ? process.argv[process.argv.indexOf("--checkpoint") + 1]
  : "checkpoint.json";
const checkpointPath = resolve(artifactsRoot, checkpointArg);
const resultsPath = resolve(artifactsRoot, "results.json");
const reportPath = resolve(benchmarkRoot, "report.html");
const templatePath = resolve(benchmarkRoot, "report-template.html");
const state = JSON.parse(readFileSync(checkpointPath, "utf8"));
const metadata = JSON.parse(readFileSync(metadataPath, "utf8"));

const CPU_WORKLOADS = {
  "01_run1k": "Create 1,000 rows",
  "02_replace1k": "Replace 1,000 rows",
  "03_update10th1k_x16": "Update every 10th row ×16",
  "04_select1k": "Select a row",
  "05_swap1k": "Swap two rows",
  "06_remove-one-1k": "Remove one row",
  "07_create10k": "Create 10,000 rows",
  "08_create1k-after1k_x2": "Append 1,000 rows ×2",
  "09_clear1k_x8": "Clear 1,000 rows ×8",
};
const MEMORY_WORKLOADS = {
  "21_ready-memory": "Ready memory",
  "22_run-memory": "Memory with 1,000 rows",
  "25_run-clear-memory": "Memory after five create/clear cycles",
};
const COLD_WORKLOADS = { "cold-page-load": "Cold page load" };
const frameworkKeys = Object.keys(state.sizes);

function displayName(name) {
  const names = {
    ripple: "Ripple",
    inferno: "Inferno",
    solid: "Solid",
    "vue-jsx-vapor": "Vue JSX Vapor",
    "solid-store": "Solid Store",
    rezact: "Rezact",
    "lit-html": "Lit HTML",
    lit: "Lit",
    vue: "Vue",
    "react-hooks": "React Hooks",
    "react-compiler-hooks": "React Compiler Hooks",
    "react-zustand": "React + Zustand",
    solidlil: "SolidLil",
    "solid-v2": "Solid 2.0",
    "solidlil-v2": "solidlil",
  };
  return names[name] ?? name;
}

function frameworkKey(name) {
  const matches = frameworkKeys.filter((candidate) =>
    candidate.startsWith(`${name}-v`),
  );
  return (
    matches.find((candidate) =>
      !metadata.frameworks.some(({ path }) => {
        const other = path.slice(path.lastIndexOf("/") + 1);
        return (
          other !== name &&
          other.startsWith(`${name}-`) &&
          candidate.startsWith(`${other}-v`)
        );
      }),
    ) ?? null
  );
}

function sampleValues(phase, key, workload, metric) {
  return state.samples
    .filter(
      (sample) =>
        sample.phase === phase &&
        sample.framework === key &&
        sample.workload === workload,
    )
    .sort((left, right) => left.block - right.block)
    .map((sample) => ({
      block: sample.block,
      value: metric ? sample.value[metric] : sample.value,
    }))
    .filter((sample) => Number.isFinite(sample.value));
}

function aggregate(phase, key, workload, metric) {
  const samples = sampleValues(phase, key, workload, metric);
  const seed = metadata.measurement.seed ^ hashString(`${phase}:${key}:${workload}:${metric ?? "value"}`);
  return samples.length === 0
    ? null
    : {
        ...summarize(
          samples.map((sample) => sample.value),
          seed,
        ),
        blocks: Object.fromEntries(samples.map((sample) => [sample.block, sample.value])),
      };
}

const frameworks = metadata.frameworks.flatMap(({ path, version }) => {
  const name = path.slice(path.lastIndexOf("/") + 1);
  const key = frameworkKey(name);
  if (!key || !state.sizes[key]) return [];
  const size = state.sizes[key];
  return [{
    id: name,
    key,
    name: displayName(name),
    version,
    solidlil: name === "solidlil" || name === "solidlil-v2",
    size: {
      jsBrotli: size.bundle.brotliBytes,
      pageBrotli: size.pageBrotliBytes,
      jsGzip: size.bundle.gzipBytes,
      jsRaw: size.bundle.rawBytes,
      pageRaw: size.pageRawBytes,
      files: size.bundle.files,
    },
    cpu: Object.fromEntries(
      Object.keys(CPU_WORKLOADS).map((workload) => [
        workload,
        {
          total: aggregate("cpu", key, workload, "total"),
          script: aggregate("cpu", key, workload, "script"),
          paint: aggregate("cpu", key, workload, "paint"),
        },
      ]),
    ),
    memory: Object.fromEntries(
      Object.keys(MEMORY_WORKLOADS).map((workload) => [
        workload,
        aggregate("memory", key, workload),
      ]),
    ),
    cold: {
      "cold-page-load": {
        domContentLoaded: aggregate("cold", key, "cold-page-load", "domContentLoadedMs"),
        load: aggregate("cold", key, "cold-page-load", "loadMs"),
        firstPaint: aggregate("cold", key, "cold-page-load", "firstPaintMs"),
      },
    },
  }];
});

function normalizedScores(section, workloads, metric) {
  const bestByWorkload = Object.fromEntries(
    Object.keys(workloads).map((workload) => {
      const values = frameworks
        .map((framework) =>
          metric
            ? framework[section][workload]?.[metric]?.median
            : framework[section][workload]?.median,
        )
        .filter(Number.isFinite);
      return [workload, values.length > 0 ? Math.min(...values) : null];
    }),
  );
  return Object.fromEntries(
    frameworks.map((framework) => {
      const ratios = Object.keys(workloads)
        .map((workload) => {
          const value = metric
            ? framework[section][workload]?.[metric]?.median
            : framework[section][workload]?.median;
          const best = bestByWorkload[workload];
          return Number.isFinite(value) && Number.isFinite(best) ? value / best : null;
        })
        .filter(Number.isFinite);
      return [framework.id, ratios.length === Object.keys(workloads).length ? geometricMean(ratios) : null];
    }),
  );
}

const expected = {
  cpu: metadata.measurement.blocks * Object.keys(CPU_WORKLOADS).length * frameworks.length,
  memory: metadata.measurement.blocks * Object.keys(MEMORY_WORKLOADS).length * frameworks.length,
  cold: metadata.measurement.blocks * frameworks.length,
  size: frameworks.length,
};
const observed = {
  cpu: state.samples.filter((sample) => sample.phase === "cpu").length,
  memory: state.samples.filter((sample) => sample.phase === "memory").length,
  cold: state.samples.filter((sample) => sample.phase === "cold").length,
  size: Object.keys(state.sizes).length,
};

const results = {
  schemaVersion: 1,
  generatedAt: new Date().toISOString(),
  provenance: state.provenance,
  configuration: state.configuration,
  upstream: {
    repository: metadata.repository,
    commit: metadata.commit,
  },
  status: Object.fromEntries(
    Object.keys(expected).map((phase) => [
      phase,
      {
        observed: observed[phase],
        expected: expected[phase],
        complete: observed[phase] === expected[phase],
      },
    ]),
  ),
  workloads: {
    cpu: CPU_WORKLOADS,
    memory: MEMORY_WORKLOADS,
    cold: COLD_WORKLOADS,
  },
  normalizedScores: {
    cpu: normalizedScores("cpu", CPU_WORKLOADS, "total"),
    memory: normalizedScores("memory", MEMORY_WORKLOADS),
  },
  frameworks,
};

writeFileSync(resultsPath, `${JSON.stringify(results, null, 2)}\n`);
const serialized = JSON.stringify(results).replaceAll("<", "\\u003c");
const template = readFileSync(templatePath, "utf8");
if (!template.includes("__REPORT_DATA__")) {
  throw new Error("Report template is missing __REPORT_DATA__");
}
writeFileSync(reportPath, template.replace("__REPORT_DATA__", serialized));
console.log(`Wrote ${reportPath}`);
console.log(`Wrote ${resultsPath}`);
