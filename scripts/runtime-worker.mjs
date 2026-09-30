// One isolated sample for runtime-cost.py. Workloads export run(namespace, N)
// and optionally install() for the common host. Only artifact loading and
// workload execution are timed, separately; verification is outside the timer.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const request = JSON.parse(readFileSync(process.argv[2], "utf8"));
if (typeof global.gc !== "function") throw new Error("runtime worker needs --expose-gc");
const workload = await import(pathToFileURL(request.workload).href);
if (typeof workload.run !== "function") throw new Error("workload must export run(namespace, iterations)");
if (workload.install) await workload.install();
const cpuMicros = (before) => {
  const used = process.cpuUsage(before);
  return used.user + used.system;
};
const elapsed = (before) => Number(process.hrtime.bigint() - before);
const startupCpu = process.cpuUsage();
const startupClock = process.hrtime.bigint();
const entryURL = pathToFileURL(request.entry);
if (request.entry_query) entryURL.searchParams.set("lilscript_control", request.entry_query);
const api = await import(entryURL.href);
const startup_wall_ns = elapsed(startupClock);
const startup_cpu_us = cpuMicros(startupCpu);

function verify(result) {
  assert.ok(result && typeof result === "object", "workload must return an object");
  assert.ok(Object.hasOwn(result, "oracle"), "workload result must include oracle");
  assert.deepStrictEqual(result.oracle, request.expected, "workload oracle differs");
  assert.deepStrictEqual(result.counters, request.counters, "declared workload counters differ");
}

const warmup = [];
for (let index = 0; index < request.warmup_runs; index++) {
  const beforeCPU = process.cpuUsage();
  const beforeClock = process.hrtime.bigint();
  const result = await workload.run(api, request.iterations);
  warmup.push({wall_ns: elapsed(beforeClock), cpu_us: cpuMicros(beforeCPU)});
  verify(result);
}
global.gc();
const beforeMemory = process.memoryUsage();
const workCpu = process.cpuUsage();
const workClock = process.hrtime.bigint();
const result = await workload.run(api, request.iterations);
const steady_wall_ns = elapsed(workClock);
const steady_cpu_us = cpuMicros(workCpu);
verify(result);
// Keep the complete observable result, including an optional retained object,
// alive across GC. These are retained deltas, not total allocations or peaks.
globalThis.__lilscript_runtime_result = result;
global.gc();
const afterMemory = process.memoryUsage();
const retained = Object.hasOwn(result, "retained");
console.log(JSON.stringify({
  schema: 1,
  engine: { version: process.version, versions: process.versions, execArgv: process.execArgv },
  oracle: result.oracle,
  counters: result.counters,
  warmup,
  metrics: {
    startup_wall_ns, startup_cpu_us, steady_wall_ns, steady_cpu_us,
    rss_bytes: afterMemory.rss,
    retained_heap_bytes: retained ? afterMemory.heapUsed - beforeMemory.heapUsed : null,
    retained_array_buffer_bytes: retained ? afterMemory.arrayBuffers - beforeMemory.arrayBuffers : null,
  },
}));
