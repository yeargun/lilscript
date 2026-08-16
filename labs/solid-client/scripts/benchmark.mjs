import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { chromium } from "../../../benchmarks/browser/playwright-runtime.mjs";
import { entryBundle, root } from "./project.mjs";
import {
  balancedRandomOrders,
  median,
  pairedDifference,
  pairedRatio,
  sampleSummary,
} from "./statistics.mjs";

const samples = Number(process.env.LILSCRIPT_SOLID_PERF_SAMPLES ?? 32);
const lsxSamples = Number(process.env.LILSCRIPT_SOLID_LSX_PERF_SAMPLES ?? 32);
const updates = Number(process.env.LILSCRIPT_SOLID_UPDATES ?? 4000);
const warmups = Number(process.env.LILSCRIPT_SOLID_WARMUPS ?? 500);
const lsxUpdates = Number(process.env.LILSCRIPT_SOLID_LSX_UPDATES ?? 4000);
const lsxWarmups = Number(process.env.LILSCRIPT_SOLID_LSX_WARMUPS ?? 500);
const bootstrapIterations = Number(
  process.env.LILSCRIPT_SOLID_BOOTSTRAP_ITERATIONS ?? 10_000,
);
const maxRatio = Number(process.env.LILSCRIPT_SOLID_MAX_RATIO ?? 1.03);
const maxFirstInteractionDeltaMs = Number(
  process.env.LILSCRIPT_SOLID_FIRST_INTERACTION_DELTA_MS ?? 0.25,
);
const maxColdCpuDeltaMs = Number(
  process.env.LILSCRIPT_SOLID_COLD_CPU_DELTA_MS ?? 1,
);
const maxColdWallDeltaMs = Number(
  process.env.LILSCRIPT_SOLID_COLD_WALL_DELTA_MS ?? 0.25,
);
const jsHeapAbsoluteMarginBytes = Number(
  process.env.LILSCRIPT_SOLID_JS_HEAP_MARGIN_BYTES ?? 128 * 1024,
);
const managedHeapAbsoluteMarginBytes = Number(
  process.env.LILSCRIPT_SOLID_MANAGED_HEAP_MARGIN_BYTES ?? 256 * 1024,
);
const rssNoiseBytes = Number(
  process.env.LILSCRIPT_SOLID_RSS_NOISE_BYTES ?? 4 * 1024 * 1024,
);
const embedderNoiseBytes = Number(
  process.env.LILSCRIPT_SOLID_EMBEDDER_NOISE_BYTES ?? 128 * 1024,
);
const seed = process.env.LILSCRIPT_SOLID_BENCHMARK_SEED ?? "solidlil-2026-08";
const allowSmallSamples =
  process.env.LILSCRIPT_SOLID_ALLOW_SMALL_SAMPLES === "1";

if (!allowSmallSamples) {
  assert.ok(samples >= 21, "browser app benchmark requires 21 blocks");
  assert.ok(lsxSamples >= 21, "browser LSX benchmark requires 21 blocks");
}
assert.ok(bootstrapIterations >= 1000, "at least 1,000 bootstrap iterations");

const generated = resolve(root, "artifacts", "generated");
const sizeReportPath = resolve(root, "artifacts", "size-report.json");
const sizeReportBytes = readFileSync(sizeReportPath);
const sizeReport = JSON.parse(sizeReportBytes);
assert.equal(sizeReport.schemaVersion, 2, "current size report required");
const lifecycleEvidence = JSON.parse(
  readFileSync(resolve(root, "artifacts/lifecycle-parity.json"), "utf8"),
);
assert.equal(lifecycleEvidence.behaviorEquivalent, true);
assert.equal(lifecycleEvidence.stableHighWater, true);
assert.equal(lifecycleEvidence.allSlotsReleased, true);

const variants = {
  solidVite: entryBundle("solid"),
  lilscriptVite: entryBundle("lilscript"),
  solidClosure: resolve(generated, "solid-closure-advanced.js"),
  lilscriptClosure: resolve(generated, "lilscript-closure-advanced.js"),
};
const lsxVariants = {
  solidLsx: entryBundle("lsx-solid"),
  solidlilLsx: entryBundle("lsx-lilscript"),
};
const sources = Object.fromEntries(
  [...Object.entries(variants), ...Object.entries(lsxVariants)].map(
    ([name, path]) => [name, readFileSync(path, "utf8")],
  ),
);

const memoryFields = [
  "jsHeapUsed",
  "embedderHeapUsed",
  "managedHeapUsed",
  "backingStorage",
  "processRss",
  "documents",
  "nodes",
  "jsEventListeners",
];

const markup = {
  app: '<!doctype html><html><body><main id="app"></main></body></html>',
  lsx: '<!doctype html><html><head></head><body><main id="app"></main><aside id="portal-target"></aside><svg id="svg-portal-target"></svg><aside id="shadow-portal-target"></aside></body></html>',
};

function metricMap(result) {
  return Object.fromEntries(
    result.metrics.map(({ name, value }) => [name, value]),
  );
}

function durationDelta(after, before, name) {
  return Math.max(0, ((after[name] ?? 0) - (before[name] ?? 0)) * 1000);
}

function processRss(processInfo) {
  const ids = processInfo.map(({ id }) => String(id));
  if (ids.length === 0) return 0;
  const result = spawnSync("ps", ["-o", "rss=", "-p", ids.join(",")], {
    encoding: "utf8",
  });
  if (result.status !== 0) {
    throw new Error(`Unable to read Chromium RSS: ${result.stderr}`);
  }
  return (
    result.stdout
      .trim()
      .split(/\s+/u)
      .filter(Boolean)
      .reduce((total, value) => total + Number(value), 0) * 1024
  );
}

async function browserMemory(pageClient, browserClient) {
  for (let index = 0; index < 4; index += 1) {
    await pageClient.send("HeapProfiler.collectGarbage");
  }
  // Oilpan finalizers and GC bookkeeping can enqueue renderer tasks after the
  // CDP collection command resolves. Let two animation frames drain those
  // tasks before taking the snapshot or starting the next timed operation.
  await pageClient.send("Runtime.evaluate", {
    awaitPromise: true,
    expression:
      "new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))",
    returnByValue: true,
  });
  const [heap, dom, processes] = await Promise.all([
    pageClient.send("Runtime.getHeapUsage"),
    pageClient.send("Memory.getDOMCounters"),
    browserClient.send("SystemInfo.getProcessInfo"),
  ]);
  return {
    jsHeapUsed: heap.usedSize,
    embedderHeapUsed: heap.embedderHeapUsedSize,
    managedHeapUsed: heap.usedSize + heap.embedderHeapUsedSize,
    backingStorage: heap.backingStorageSize,
    processRss: processRss(processes.processInfo),
    documents: dom.documents,
    nodes: dom.nodes,
    jsEventListeners: dom.jsEventListeners,
  };
}

function retainedMemoryDelta(current, baseline) {
  return Object.fromEntries(
    memoryFields.map((field) => [field, current[field] - baseline[field]]),
  );
}

async function measuredBrowserTask(page, client, operation) {
  const before = metricMap(await client.send("Performance.getMetrics"));
  const wallMs = await operation();
  const after = metricMap(await client.send("Performance.getMetrics"));
  return {
    wallMs,
    cpuMs: durationDelta(after, before, "TaskDuration"),
    processCpuMs: durationDelta(after, before, "ProcessTime"),
    compileCpuMs: durationDelta(after, before, "V8CompileDuration"),
  };
}

async function runBrowserObservation({
  block,
  browser,
  browserClient,
  kind,
  name,
  source,
  updates: measuredUpdates,
  warmups: untimedUpdates,
}) {
  const context = await browser.newContext({
    locale: "en-US",
    serviceWorkers: "block",
    viewport: { height: 720, width: 1280 },
  });
  const page = await context.newPage();
  const pageErrors = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  try {
    await page.setContent(markup[kind], { waitUntil: "load" });
    if (kind === "lsx") {
      await page.evaluate(() => {
        globalThis.registerLsxBoundaryDiagnostics = (
          boundaryCleanups,
          initialBoundaryCleanups,
          suspenseContentCleanups,
          suspenseFallbackCleanups,
        ) => {
          globalThis.__lsxBoundaryDiagnostics = () => ({
            boundaryCleanups: boundaryCleanups(),
            initialBoundaryCleanups: initialBoundaryCleanups(),
            suspenseContentCleanups: suspenseContentCleanups(),
            suspenseFallbackCleanups: suspenseFallbackCleanups(),
          });
        };
      });
    }
    await page.evaluate(
      ({ artifactName, block, source }) => {
        globalThis.__solidBenchmarkSource = `${source}\n//# sourceURL=${artifactName}-cold-${block}.js`;
      },
      { artifactName: name, block, source },
    );
    const pageClient = await context.newCDPSession(page);
    await pageClient.send("Performance.enable", { timeDomain: "threadTicks" });
    await pageClient.send("HeapProfiler.enable");

    const baseline = await browserMemory(pageClient, browserClient);
    const coldMount = await measuredBrowserTask(page, pageClient, () =>
      page.evaluate(() => {
        const started = performance.now();
        (0, eval)(globalThis.__solidBenchmarkSource);
        return performance.now() - started;
      }),
    );
    const cold = await browserMemory(pageClient, browserClient);
    const firstInteraction = await measuredBrowserTask(page, pageClient, () =>
      page.evaluate(() => {
        const button = document.querySelector('[data-action="increment"]');
        if (!button) throw new Error("missing increment button");
        const started = performance.now();
        button.click();
        return performance.now() - started;
      }),
    );

    await page.evaluate(
      ({ kind, warmups }) => {
        const button = document.querySelector('[data-action="increment"]');
        for (let index = 0; index < warmups; index += 1) button.click();
        if (kind === "app") {
          const reset = document.querySelector('[data-action="reset"]');
          if (!reset) throw new Error("missing reset button");
          reset.click();
        }
      },
      { kind, warmups: untimedUpdates },
    );
    const warm = await measuredBrowserTask(page, pageClient, () =>
      page.evaluate((count) => {
        const button = document.querySelector('[data-action="increment"]');
        const started = performance.now();
        for (let index = 0; index < count; index += 1) button.click();
        return performance.now() - started;
      }, measuredUpdates),
    );

    const checksum = await page.evaluate(
      (kind) =>
        kind === "app"
          ? Number(document.querySelector('[data-value="count"]')?.textContent)
          : Number(document.querySelector("#lsx-root")?.dataset.count),
      kind,
    );
    const expectedChecksum =
      kind === "app" ? measuredUpdates : 1 + untimedUpdates + measuredUpdates;
    assert.equal(checksum, expectedChecksum, `${name}: final browser count`);
    if (kind === "app") {
      assert.equal(
        await page.locator('[data-value="doubled"]').textContent(),
        String(measuredUpdates * 2),
        `${name}: final browser memo`,
      );
    }
    const live = await browserMemory(pageClient, browserClient);

    const teardown = await page.evaluate(
      ({ checksum, kind }) => {
        const button = document.querySelector('[data-action="increment"]');
        if (kind === "app") {
          const count = document.querySelector('[data-value="count"]');
          if (typeof globalThis.__disposeSolidBenchmark !== "function") {
            throw new Error("missing app disposer");
          }
          globalThis.__disposeSolidBenchmark();
          globalThis.__disposeSolidBenchmark();
          const empty = document.querySelector("#app").childNodes.length === 0;
          button.click();
          const result = {
            cleanup: null,
            countStopped: count.textContent === String(checksum),
            empty,
            slots: null,
          };
          delete globalThis.__disposeSolidBenchmark;
          return result;
        }
        const root = document.querySelector("#lsx-root");
        if (typeof globalThis.__disposeLsx !== "function") {
          throw new Error("missing LSX disposer");
        }
        globalThis.__disposeLsx();
        globalThis.__disposeLsx();
        const empty = [
          "#app",
          "#portal-target",
          "#svg-portal-target",
          "#shadow-portal-target",
        ].every(
          (selector) =>
            document.querySelector(selector).childNodes.length === 0,
        );
        button.click();
        const result = {
          cleanup: globalThis.__lsxBoundaryDiagnostics?.() ?? null,
          countStopped: root.dataset.count === String(checksum),
          empty,
          slots: globalThis.__lsxDiagnostics?.() ?? null,
        };
        delete globalThis.__disposeLsx;
        delete globalThis.__lsxBoundaryDiagnostics;
        delete globalThis.__lsxDiagnostics;
        return result;
      },
      { checksum, kind },
    );
    assert.equal(teardown.empty, true, `${name}: browser DOM unmounted`);
    assert.equal(
      teardown.countStopped,
      true,
      `${name}: stale browser event stopped`,
    );
    if (kind === "lsx") {
      assert.deepEqual(teardown.cleanup, {
        boundaryCleanups: 1,
        initialBoundaryCleanups: 1,
        suspenseContentCleanups: 1,
        suspenseFallbackCleanups: 1,
      });
      if (teardown.slots) {
        assert.equal(teardown.slots.freeOwners, teardown.slots.owners);
        assert.equal(teardown.slots.freeEffects, teardown.slots.effects);
        assert.equal(teardown.slots.pendingEffects, 0);
      }
    }
    const disposed = await browserMemory(pageClient, browserClient);
    assert.deepEqual(pageErrors, [], `${name}: uncaught browser errors`);
    return {
      block,
      checksum,
      coldMount,
      firstInteraction,
      kind,
      memory: {
        absolute: { baseline, cold, disposed, live },
        retained: {
          cold: retainedMemoryDelta(cold, baseline),
          disposed: retainedMemoryDelta(disposed, baseline),
          live: retainedMemoryDelta(live, baseline),
        },
      },
      name,
      teardown,
      updates: measuredUpdates,
      warm,
      warmups: untimedUpdates,
    };
  } finally {
    await context.close();
  }
}

async function runRandomizedBrowserBlocks({
  blockCount,
  kind,
  labels,
  name,
  seedSuffix,
  updates: measuredUpdates,
  warmups: untimedUpdates,
}) {
  const orders = balancedRandomOrders(
    labels,
    blockCount,
    `${seed}:${seedSuffix}`,
  );
  const measurements = Object.fromEntries(labels.map((label) => [label, []]));
  const restartEvery = labels.length * 2;
  let browser = null;
  let browserClient = null;
  let browserVersion = null;
  try {
    for (const [block, order] of orders.entries()) {
      if (block % restartEvery === 0) {
        if (browser) await browser.close();
        browser = await chromium.launch({
          headless: true,
          args: [
            "--disable-background-networking",
            "--disable-component-update",
            "--disable-default-apps",
            "--enable-precise-memory-info",
            "--no-first-run",
          ],
        });
        browserClient = await browser.newBrowserCDPSession();
        browserVersion ??= browser.version();
        assert.equal(browser.version(), browserVersion);
      }
      for (const label of order) {
        measurements[label].push(
          await runBrowserObservation({
            block,
            browser,
            browserClient,
            kind,
            name: label,
            source: sources[label],
            updates: measuredUpdates,
            warmups: untimedUpdates,
          }),
        );
      }
      if ((block + 1) % 5 === 0 || block + 1 === blockCount) {
        process.stderr.write(
          `[playwright] ${name}: ${block + 1}/${blockCount} randomized blocks\n`,
        );
      }
    }
  } finally {
    if (browser) await browser.close();
  }
  return { browserVersion, measurements, orders, restartEvery };
}

const appBenchmark = await runRandomizedBrowserBlocks({
  blockCount: samples,
  kind: "app",
  labels: Object.keys(variants),
  name: "app CPU/RAM",
  seedSuffix: "playwright-app",
  updates,
  warmups,
});
const lsxBenchmark = await runRandomizedBrowserBlocks({
  blockCount: lsxSamples,
  kind: "lsx",
  labels: Object.keys(lsxVariants),
  name: "LSX CPU/RAM",
  seedSuffix: "playwright-lsx",
  updates: lsxUpdates,
  warmups: lsxWarmups,
});
assert.equal(appBenchmark.browserVersion, lsxBenchmark.browserVersion);

const performanceMetrics = {
  coldCpu: {
    label: "cold parse/eval/mount main-thread CPU",
    select: (sample) => sample.coldMount.cpuMs,
  },
  coldWall: {
    label: "cold parse/eval/mount wall time",
    select: (sample) => sample.coldMount.wallMs,
  },
  firstCpu: {
    label: "first interaction main-thread CPU",
    select: (sample) => sample.firstInteraction.cpuMs,
  },
  firstWall: {
    label: "first interaction wall time",
    select: (sample) => sample.firstInteraction.wallMs,
  },
  warmCpu: {
    label: "warm steady-state main-thread CPU",
    select: (sample) => sample.warm.cpuMs,
  },
  warmWall: {
    label: "warm steady-state wall time",
    select: (sample) => sample.warm.wallMs,
  },
};

function comparePerformance(baseline, candidate, label) {
  return {
    metrics: Object.fromEntries(
      Object.entries(performanceMetrics).map(([id, metric]) => {
        const baselineValues = baseline.map(metric.select);
        const candidateValues = candidate.map(metric.select);
        const absoluteMargin = id.startsWith("first")
          ? maxFirstInteractionDeltaMs
          : id === "coldCpu"
            ? maxColdCpuDeltaMs
            : id === "coldWall"
              ? maxColdWallDeltaMs
              : 0;
        assert.ok(
          baselineValues.every((value) => value > 0),
          `${label}/${id}: positive browser baseline`,
        );
        return [
          id,
          {
            label: metric.label,
            unit: "ms",
            baseline: sampleSummary(baselineValues),
            candidate: sampleSummary(candidateValues),
            comparison: pairedRatio(baselineValues, candidateValues, {
              bootstrapIterations,
              nonInferiorityMargin: maxRatio,
              seed: `${seed}:${label}:${id}:ratio`,
            }),
            absoluteComparison: pairedDifference(
              baselineValues,
              candidateValues,
              {
                bootstrapIterations,
                nonInferiorityMargin: absoluteMargin,
                seed: `${seed}:${label}:${id}:difference`,
              },
            ),
          },
        ];
      }),
    ),
  };
}

function performanceEligible(comparison) {
  const ratioMetrics = ["warmCpu", "warmWall"];
  const firstMetrics = ["firstCpu", "firstWall"];
  return (
    ratioMetrics.every(
      (metric) => comparison.metrics[metric].comparison.nonInferior,
    ) &&
    comparison.metrics.coldWall.absoluteComparison.nonInferior &&
    firstMetrics.every(
      (metric) => comparison.metrics[metric].absoluteComparison.nonInferior,
    )
  );
}

function performanceMetricEligible(id, metric) {
  if (id === "coldCpu") return null;
  return id === "coldWall" || id.startsWith("first")
    ? metric.absoluteComparison.nonInferior
    : metric.comparison.nonInferior;
}

function memoryComparison(baseline, candidate, label) {
  const phases = {};
  for (const phase of ["cold", "live", "disposed"]) {
    phases[phase] = {};
    for (const field of memoryFields) {
      const baselineValues = baseline.map(
        (sample) => sample.memory.retained[phase][field],
      );
      const candidateValues = candidate.map(
        (sample) => sample.memory.retained[phase][field],
      );
      const forceDifference = [
        "embedderHeapUsed",
        "processRss",
        "documents",
        "nodes",
        "jsEventListeners",
      ].includes(field);
      const ratioEligible =
        !forceDifference && baselineValues.every((value) => value > 0);
      const differenceMargin =
        field === "processRss"
          ? rssNoiseBytes
          : field === "jsHeapUsed"
            ? jsHeapAbsoluteMarginBytes
            : field === "managedHeapUsed"
              ? managedHeapAbsoluteMarginBytes
              : field === "embedderHeapUsed"
                ? embedderNoiseBytes
                : 0;
      const absoluteComparison = pairedDifference(
        baselineValues,
        candidateValues,
        {
          bootstrapIterations,
          nonInferiorityMargin: differenceMargin,
          seed: `${seed}:${label}:${phase}:${field}:difference`,
        },
      );
      phases[phase][field] = {
        baseline: sampleSummary(baselineValues),
        candidate: sampleSummary(candidateValues),
        comparisonType: ratioEligible ? "ratio" : "difference",
        comparison: ratioEligible
          ? pairedRatio(baselineValues, candidateValues, {
              bootstrapIterations,
              nonInferiorityMargin: maxRatio,
              seed: `${seed}:${label}:${phase}:${field}:ratio`,
            })
          : absoluteComparison,
        absoluteComparison,
      };
    }
  }
  return { phases };
}

function memoryMetricEligible(metric, field) {
  return (
    metric.comparison.nonInferior ||
    (["jsHeapUsed", "managedHeapUsed"].includes(field) &&
      metric.absoluteComparison.nonInferior)
  );
}

function memoryEligible(comparison) {
  return ["cold", "live", "disposed"].every((phase) => {
    const heap = comparison.phases[phase].jsHeapUsed;
    const managedHeap = comparison.phases[phase].managedHeapUsed;
    const rss = comparison.phases[phase].processRss;
    assert.equal(heap.comparisonType, "ratio", `${phase} browser heap ratio`);
    return (
      memoryMetricEligible(heap, "jsHeapUsed") &&
      memoryMetricEligible(managedHeap, "managedHeapUsed") &&
      memoryMetricEligible(rss, "processRss")
    );
  });
}

const statistics = {
  vite: comparePerformance(
    appBenchmark.measurements.solidVite,
    appBenchmark.measurements.lilscriptVite,
    "vite-browser-performance",
  ),
  closureAdvanced: comparePerformance(
    appBenchmark.measurements.solidClosure,
    appBenchmark.measurements.lilscriptClosure,
    "closure-browser-performance",
  ),
  lsx: comparePerformance(
    lsxBenchmark.measurements.solidLsx,
    lsxBenchmark.measurements.solidlilLsx,
    "lsx-browser-performance",
  ),
};
const memoryStatistics = {
  vite: memoryComparison(
    appBenchmark.measurements.solidVite,
    appBenchmark.measurements.lilscriptVite,
    "vite-browser-memory",
  ),
  closureAdvanced: memoryComparison(
    appBenchmark.measurements.solidClosure,
    appBenchmark.measurements.lilscriptClosure,
    "closure-browser-memory",
  ),
  lsx: memoryComparison(
    lsxBenchmark.measurements.solidLsx,
    lsxBenchmark.measurements.solidlilLsx,
    "lsx-browser-memory",
  ),
};

function legacyTimes(measurements) {
  return Object.fromEntries(
    Object.entries(measurements).map(([name, entries]) => [
      name,
      median(entries.map((sample) => sample.warm.wallMs)),
    ]),
  );
}

function legacyMemory(measurements, phase) {
  return Object.fromEntries(
    Object.entries(measurements).map(([name, entries]) => [
      name,
      median(entries.map((sample) => sample.memory.retained[phase].jsHeapUsed)),
    ]),
  );
}

const medians = legacyTimes(appBenchmark.measurements);
const retainedMemory = legacyMemory(appBenchmark.measurements, "live");
const disposedMemory = legacyMemory(appBenchmark.measurements, "disposed");
const lsxMedians = legacyTimes(lsxBenchmark.measurements);
const lsxRetainedMemory = legacyMemory(lsxBenchmark.measurements, "live");
const lsxDisposedMemory = legacyMemory(lsxBenchmark.measurements, "disposed");
const ratios = {
  closureAdvanced:
    statistics.closureAdvanced.metrics.warmWall.comparison.pointEstimate,
  vite: statistics.vite.metrics.warmWall.comparison.pointEstimate,
};
const memoryRatios = {
  closureAdvanced:
    memoryStatistics.closureAdvanced.phases.live.jsHeapUsed.comparison
      .pointEstimate,
  vite: memoryStatistics.vite.phases.live.jsHeapUsed.comparison.pointEstimate,
};
const disposedMemoryRatios = {
  closureAdvanced:
    memoryStatistics.closureAdvanced.phases.disposed.jsHeapUsed.comparison
      .pointEstimate,
  vite: memoryStatistics.vite.phases.disposed.jsHeapUsed.comparison
    .pointEstimate,
};
const lsxRatio = statistics.lsx.metrics.warmWall.comparison.pointEstimate;
const lsxMemoryRatio =
  memoryStatistics.lsx.phases.live.jsHeapUsed.comparison.pointEstimate;
const lsxDisposedMemoryRatio =
  memoryStatistics.lsx.phases.disposed.jsHeapUsed.comparison.pointEstimate;

const allTeardowns = [
  ...Object.values(appBenchmark.measurements).flat(),
  ...Object.values(lsxBenchmark.measurements).flat(),
].every(({ teardown }) => teardown.empty && teardown.countStopped);
const eligibility = {
  vite:
    performanceEligible(statistics.vite) &&
    memoryEligible(memoryStatistics.vite),
  closureAdvanced:
    performanceEligible(statistics.closureAdvanced) &&
    memoryEligible(memoryStatistics.closureAdvanced),
  lifecycle: lifecycleEvidence.allSlotsReleased && allTeardowns,
  lsx:
    performanceEligible(statistics.lsx) && memoryEligible(memoryStatistics.lsx),
};

const report = {
  schemaVersion: 2,
  generatedAt: new Date().toISOString(),
  browser: {
    engine: "Chromium",
    version: appBenchmark.browserVersion,
    automation: "Playwright 1.62.1",
    headless: true,
  },
  codecs: sizeReport.codecs,
  compiler: sizeReport.compiler,
  sizeEvidence: {
    path: "artifacts/size-report.json",
    sha256: createHash("sha256").update(sizeReportBytes).digest("hex"),
  },
  protocol: {
    design: "randomized complete block, paired by block",
    seed,
    processIsolation:
      "Fresh Playwright incognito context and page per artifact; Chromium restarts after every balanced 2n block cycle.",
    coldDefinition:
      "Unique-source parse/eval/mount in a fresh Chromium context with code cache defeated by a block-specific source URL.",
    warmDefinition: `${warmups} app or ${lsxWarmups} LSX untimed browser interactions precede a long measured loop.`,
    cpuClock:
      "Chromium CDP Performance TaskDuration with threadTicks (main-thread CPU), paired with in-page performance.now wall time.",
    ramDefinition:
      "Chromium Runtime.getHeapUsage, Oilpan/embedder heap, DOM/listener counters, and summed Chromium-process RSS after four CDP forced GCs.",
    confidence:
      "Paired 95% percentile bootstrap confidence intervals and paired sign-flip permutation p-values.",
    bootstrapIterations,
    outlierPolicy: "No observations removed or winsorized.",
    nonInferiorityMarginRatio: maxRatio,
    diagnosticColdCpuReferenceMarginMs: maxColdCpuDeltaMs,
    coldWallAbsoluteMarginMs: maxColdWallDeltaMs,
    firstInteractionAbsoluteMarginMs: maxFirstInteractionDeltaMs,
    jsHeapAbsoluteMarginBytes,
    managedHeapAbsoluteMarginBytes,
    rssAbsoluteNoiseMarginBytes: rssNoiseBytes,
    embedderAbsoluteNoiseMarginBytes: embedderNoiseBytes,
    primaryPerformanceMetrics: [
      "coldMount.wallMs",
      "firstInteraction.cpuMs",
      "firstInteraction.wallMs",
      "warm.cpuMs",
      "warm.wallMs",
    ],
    diagnosticCpuMetrics: ["coldMount.cpuMs"],
    primaryRamMetrics: [
      "cold.jsHeapUsed",
      "live.jsHeapUsed",
      "disposed.jsHeapUsed",
      "cold.managedHeapUsed",
      "live.managedHeapUsed",
      "disposed.managedHeapUsed",
      "cold.processRss",
      "live.processRss",
      "disposed.processRss",
    ],
    diagnosticRamMetrics: [
      "cold.embedderHeapUsed",
      "live.embedderHeapUsed",
      "disposed.embedderHeapUsed",
      "cold.backingStorage",
      "live.backingStorage",
      "disposed.backingStorage",
      "documents",
      "nodes",
      "jsEventListeners",
    ],
    sampleAdequacyOverride: allowSmallSamples,
  },
  environment: `Playwright 1.62.1, Chromium ${appBenchmark.browserVersion}; ${updates} app and ${lsxUpdates} LSX measured browser interactions`,
  samples,
  memorySamples: samples,
  lifecycleSamples: 0,
  lifecycleCycles: lifecycleEvidence.cycles,
  lifecycleNoiseBytes: null,
  maxRatio,
  statistics,
  memoryStatistics,
  lifecycleStatistics: {
    method:
      "Chromium post-unmount heap/RSS plus deterministic runtime slot accounting",
    allBrowserTeardownsVerified: allTeardowns,
    allSlotsReleased: lifecycleEvidence.allSlotsReleased,
    stableHighWater: lifecycleEvidence.stableHighWater,
  },
  orders: {
    app: appBenchmark.orders,
    lsx: lsxBenchmark.orders,
  },
  browserRestartEveryBlocks: {
    app: appBenchmark.restartEvery,
    lsx: lsxBenchmark.restartEvery,
  },
  medians,
  ratios,
  retainedMemory,
  memoryRatios,
  disposedMemory,
  disposedMemoryRatios,
  lifecycleRetainedMemory: null,
  lifecycleMeasurements: null,
  lifecycleSlots: lifecycleEvidence.slots,
  lifecycleWarmSlots: lifecycleEvidence.warmSlots,
  lifecycleWorkloads: lifecycleEvidence.workloads,
  lifecycleLimit: null,
  eligibility,
  lsx: {
    samples: lsxSamples,
    memorySamples: lsxSamples,
    updates: lsxUpdates,
    warmups: lsxWarmups,
    medians: lsxMedians,
    ratio: lsxRatio,
    retainedMemory: lsxRetainedMemory,
    memoryRatio: lsxMemoryRatio,
    disposedMemory: lsxDisposedMemory,
    disposedMemoryRatio: lsxDisposedMemoryRatio,
    statistics: statistics.lsx,
    memoryStatistics: memoryStatistics.lsx,
    measurements: Object.fromEntries(
      Object.entries(lsxBenchmark.measurements).map(([name, entries]) => [
        name,
        entries.map((sample) => sample.warm.wallMs),
      ]),
    ),
    memoryMeasurements: Object.fromEntries(
      Object.entries(lsxBenchmark.measurements).map(([name, entries]) => [
        name,
        entries.map((sample) => sample.memory.retained.live.jsHeapUsed),
      ]),
    ),
    disposedMemoryMeasurements: Object.fromEntries(
      Object.entries(lsxBenchmark.measurements).map(([name, entries]) => [
        name,
        entries.map((sample) => sample.memory.retained.disposed.jsHeapUsed),
      ]),
    ),
  },
  measurements: Object.fromEntries(
    Object.entries(appBenchmark.measurements).map(([name, entries]) => [
      name,
      entries.map((sample) => sample.warm.wallMs),
    ]),
  ),
  cpuMeasurements: Object.fromEntries(
    Object.entries(appBenchmark.measurements).map(([name, entries]) => [
      name,
      entries.map((sample) => sample.warm.cpuMs),
    ]),
  ),
  memoryMeasurements: Object.fromEntries(
    Object.entries(appBenchmark.measurements).map(([name, entries]) => [
      name,
      entries.map((sample) => sample.memory.retained.live.jsHeapUsed),
    ]),
  ),
  rawSamples: {
    app: appBenchmark.measurements,
    lsx: lsxBenchmark.measurements,
  },
};

function ratioCell(metric) {
  const value = metric.comparison;
  return `${value.pointEstimate.toFixed(3)} [${value.confidenceInterval.lower95.toFixed(3)}, ${value.confidenceInterval.upper95.toFixed(3)}]`;
}

function performanceRows() {
  const rows = [];
  for (const [label, comparison] of [
    ["Vite app", statistics.vite],
    ["Closure app", statistics.closureAdvanced],
    ["LSX fixture", statistics.lsx],
  ]) {
    for (const id of [
      "coldWall",
      "coldCpu",
      "firstWall",
      "firstCpu",
      "warmWall",
      "warmCpu",
    ]) {
      const metric = comparison.metrics[id];
      const gate = performanceMetricEligible(id, metric);
      rows.push(
        `| ${label} | ${metric.label} | ${metric.baseline.median.toFixed(3)} | ${metric.candidate.median.toFixed(3)} | ${ratioCell(metric)} | ${metric.absoluteComparison.confidenceInterval.upper95.toFixed(3)} ms | ${gate === null ? "diagnostic" : gate ? "pass" : "fail"} |`,
      );
    }
  }
  return rows.join("\n");
}

function memoryRows(field) {
  const rows = [];
  for (const [label, comparison] of [
    ["Vite app", memoryStatistics.vite],
    ["Closure app", memoryStatistics.closureAdvanced],
    ["LSX fixture", memoryStatistics.lsx],
  ]) {
    for (const phase of ["cold", "live", "disposed"]) {
      const metric = comparison.phases[phase][field];
      const comparisonText =
        metric.comparisonType === "ratio"
          ? `${ratioCell(metric)}; Δ upper ${Math.round(metric.absoluteComparison.confidenceInterval.upper95).toLocaleString("en-US")} B`
          : `${Math.round(metric.comparison.pointEstimate).toLocaleString("en-US")} B [upper ${Math.round(metric.comparison.confidenceInterval.upper95).toLocaleString("en-US")} B]`;
      rows.push(
        `| ${label} | ${phase} | ${Math.round(metric.baseline.median).toLocaleString("en-US")} | ${Math.round(metric.candidate.median).toLocaleString("en-US")} | ${comparisonText} | ${memoryMetricEligible(metric, field) ? "pass" : "fail"} |`,
      );
    }
  }
  return rows.join("\n");
}

const markdown = `# SolidLil Playwright CPU and RAM validation

${report.environment}. Lower is better. Every observation executes in actual Chromium through Playwright, is paired by randomized block, and is retained. Ratios are SolidLil / official Solid geometric means with 95% paired bootstrap confidence intervals.

## Browser CPU and wall time

| Boundary | Metric | Solid median ms | SolidLil median ms | Ratio [95% CI] | Absolute upper 95% delta | Gate |
| --- | --- | ---: | ---: | ---: | ---: | --- |
${performanceRows()}

Warm CPU/wall time gate the upper ratio bound at ${maxRatio.toFixed(2)}×. Cold wall latency uses a ${maxColdWallDeltaMs.toFixed(2)} ms absolute upper bound. Sub-2 ms cold CDP CPU stays diagnostic because unrelated renderer tasks can dominate it even when direct wall latency is stable. First-interaction latency uses a ${maxFirstInteractionDeltaMs.toFixed(2)} ms absolute upper bound so timer quantization cannot turn a tiny absolute difference into a misleading large ratio.

## Chromium JavaScript heap

Four forced Chromium collections precede baseline, cold, live, and disposed snapshots.
JavaScript heap passes when either its 95% ratio upper bound is at most ${maxRatio.toFixed(2)}× or its paired absolute upper difference is at most ${jsHeapAbsoluteMarginBytes.toLocaleString("en-US")} B. This avoids treating a few kilobytes over a small retained baseline as a material regression; the combined managed heap and total-process RSS remain independent gates, while the JS/Oilpan split stays visible.

| Boundary | Phase | Solid median B | SolidLil median B | Comparison [95% CI] | Gate |
| --- | --- | ---: | ---: | ---: | --- |
${memoryRows("jsHeapUsed")}

## Chromium managed heap

This combines JavaScript and Oilpan/embedder heap, allowing an internal bookkeeping trade between those heaps while keeping both components in the report. It passes at ${maxRatio.toFixed(2)}× or a paired absolute upper difference of ${managedHeapAbsoluteMarginBytes.toLocaleString("en-US")} B.

| Boundary | Phase | Solid median B | SolidLil median B | Ratio [95% CI] | Gate |
| --- | --- | ---: | ---: | ---: | --- |
${memoryRows("managedHeapUsed")}

## Chromium process RSS

RSS sums every Chromium process reported by CDP for the isolated run. Because allocator/page granularity creates zeros and jumps, this is a paired absolute-difference gate with a ${rssNoiseBytes.toLocaleString("en-US")} B upper allowance.

| Boundary | Phase | Solid median retained B | SolidLil median retained B | Difference [upper 95%] | Gate |
| --- | --- | ---: | ---: | ---: | --- |
${memoryRows("processRss")}

## Ownership and unmount

Every Playwright sample performs idempotent unmount, checks empty application and portal roots, and proves stale controls stop. The deterministic ownership workload separately returns all ${lifecycleEvidence.slots.owners} owner and ${lifecycleEvidence.slots.effects} effect slots with zero pending effects.

## Eligibility

- Vite application: **${eligibility.vite ? "pass" : "fail"}**
- Closure ADVANCED application: **${eligibility.closureAdvanced ? "pass" : "fail"}**
- Integrated LSX fixture: **${eligibility.lsx ? "pass" : "fail"}**
- Browser teardown plus lifecycle slots: **${eligibility.lifecycle ? "pass" : "fail"}**
`;

writeFileSync(
  resolve(root, "artifacts", "performance-report.json"),
  `${JSON.stringify(report, null, 2)}\n`,
);
writeFileSync(resolve(root, "artifacts", "performance-report.md"), markdown);

console.log(
  `Playwright Chromium validation: Vite ${ratios.vite.toFixed(3)}x, Closure ${ratios.closureAdvanced.toFixed(3)}x, LSX ${lsxRatio.toFixed(3)}x warm wall; eligibility ${JSON.stringify(eligibility)}.`,
);
if (Object.values(eligibility).includes(false)) {
  throw new Error(
    `SolidLil exceeds a preregistered Playwright CPU/RAM non-inferiority bound`,
  );
}
