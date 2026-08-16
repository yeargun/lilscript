import { createRequire } from "node:module";
import { performance } from "node:perf_hooks";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { resolveJqueryLilscriptArtifact } from "./jquery-benchmark-artifact.mjs";

const root = dirname(fileURLToPath(import.meta.url));
const [implementation, mode, workload] = process.argv.slice(2);
if (!global.gc) throw new Error("run with --expose-gc");
if (implementation !== "npm" && implementation !== "lilscript") {
  throw new Error(`unknown implementation ${implementation}`);
}
const artifact = implementation === "lilscript"
  ? resolveJqueryLilscriptArtifact({
      defaultArtifactPath: join(
        root,
        "build/jquery-config-audit/lean-balanced.terser.js",
      ),
    })
  : null;

const { JSDOM } = await import("jsdom");
const dom = new JSDOM(`<!doctype html><html><body><main id="fixture"></main></body></html>`, {
  pretendToBeVisual: true,
});
globalThis.window = dom.window;
globalThis.document = dom.window.document;

let $;
if (implementation === "npm") {
  const jquery = createRequire(import.meta.url)("jquery");
  // The CommonJS entrypoint exports an initialized jQuery when `window` was
  // installed before require, and a window factory otherwise.
  $ = jquery.fn?.jquery ? jquery : jquery(dom.window);
} else {
  $ = (await import(
    `${pathToFileURL(artifact.path).href}?jquery-benchmark=${process.pid}`
  )).jQuery;
}

const fixture = document.getElementById("fixture");
fixture.innerHTML = Array.from({ length: 96 }, (_, index) =>
  `<section class="group g-${index & 7}"><button class="item ${index & 1 ? "odd" : "even"}" data-i="${index}">${index}</button></section>`,
).join("");

function coreWork(count, retain) {
  const retained = retain ? [] : null;
  let checksum = 0;
  for (let iteration = 0; iteration < count; iteration += 1) {
    const merged = $.extend(true, {}, {
      index: iteration,
      nested: { active: (iteration & 1) === 0, values: [iteration, iteration + 1] },
    });
    const selected = $(fixture).find(`.g-${iteration & 7} .item`).filter(iteration & 1 ? ".odd" : ".even");
    checksum += merged.nested.values[1] + selected.length;
    if (retained) retained.push(merged, selected);
  }
  return { checksum, retained };
}

function eventsWork(count, retain) {
  const retained = retain ? [] : null;
  const button = fixture.querySelector("button");
  const target = $(button);
  let checksum = 0;
  const handler = (event, value) => {
    checksum += value + (event.type === "bench" ? 1 : 0);
  };
  for (let iteration = 0; iteration < count; iteration += 1) {
    target.on("bench.ns", handler);
    target.trigger("bench", [iteration & 7]);
    target.off("bench.ns", handler);
    if (retained) {
      const copy = $({});
      copy.on("bench", handler);
      retained.push(copy);
    }
  }
  return { checksum, retained: retained ?? target };
}

function deferredWork(count, retain) {
  const retained = retain ? [] : null;
  let checksum = 0;
  for (let iteration = 0; iteration < count; iteration += 1) {
    const deferred = $.Deferred();
    deferred.done((value) => {
      checksum += value;
    });
    deferred.resolve(iteration & 7);
    if (retained) retained.push(deferred);
  }
  return { checksum, retained };
}

function collectionWork(count, retain) {
  const retained = retain ? [] : null;
  let checksum = 0;
  for (let iteration = 0; iteration < count; iteration += 1) {
    const collection = $({ index: iteration });
    checksum += collection.length;
    if (retained) retained.push(collection);
  }
  return { checksum, retained };
}

function eventStateWork(count, retain) {
  const retained = retain ? [] : null;
  let checksum = 0;
  const handler = () => {
    checksum += 1;
  };
  for (let iteration = 0; iteration < count; iteration += 1) {
    const collection = $({ index: iteration });
    collection.on("bench", handler);
    checksum += collection.length;
    if (retained) retained.push(collection);
  }
  return { checksum, retained };
}

const work = workload === "core"
  ? coreWork
  : workload === "events"
    ? eventsWork
    : workload === "deferred"
      ? deferredWork
      : workload === "collection"
        ? collectionWork
        : workload === "event-state"
          ? eventStateWork
      : null;
if (!work) throw new Error(`unknown workload ${workload}`);

const performanceCounts = { core: 2_000, events: 8_000, deferred: 12_000, collection: 20_000, "event-state": 8_000 };
const memoryCounts = { core: 800, events: 1_200, deferred: 2_000, collection: 4_000, "event-state": 1_200 };

if (mode === "performance") {
  work(Math.floor(performanceCounts[workload] / 10), false);
  global.gc();
  const started = performance.now();
  const result = work(performanceCounts[workload], false);
  console.log(JSON.stringify({ milliseconds: performance.now() - started, checksum: result.checksum }));
} else if (mode === "memory") {
  // Measure retained workload state, not one-time compilation, inline-cache,
  // selector-cache, or lazy runtime initialization. The performance lane
  // already warms the exact workload for the same reason; keep the memory
  // baseline equally quiescent before taking its first heap snapshot.
  work(Math.max(1, Math.floor(memoryCounts[workload] / 10)), false);
  global.gc();
  const before = process.memoryUsage();
  const result = work(memoryCounts[workload], true);
  globalThis.__retainedJqueryBenchmark = result.retained;
  global.gc();
  const after = process.memoryUsage();
  console.log(JSON.stringify({
    bytes: after.heapUsed - before.heapUsed + after.arrayBuffers - before.arrayBuffers,
    checksum: result.checksum,
  }));
} else {
  throw new Error(`unknown mode ${mode}`);
}
