import { performance } from "node:perf_hooks";
import { artifactUrl, checksum, emitResult, nodeEnvironment } from "./common.mjs";

const reactivity = await import(artifactUrl());

function exercise(rowCount, cycles) {
  const rows = reactivity.reactive(
    Array.from({ length: rowCount }, (_, id) => ({
      id,
      value: (id * 17) % 997,
      active: id % 3 !== 0,
    })),
  );
  let observed = 0;
  let effectRuns = 0;
  const total = reactivity.computed(() =>
    rows.reduce((sum, row) => sum + (row.active ? row.value : 0), 0),
  );
  const runner = reactivity.effect(() => {
    effectRuns += 1;
    observed = total.value;
  });
  for (let cycle = 0; cycle < cycles; cycle += 1) {
    for (let index = 0; index < rows.length; index += 1) {
      const row = rows[index];
      row.value = (row.value * 33 + cycle + index) % 10_007;
      if ((cycle + index) % 11 === 0) row.active = !row.active;
    }
  }
  reactivity.stop(runner);
  return {
    observed,
    total: total.value,
    effectRuns,
    rows: rows.map(({ id, value, active }) => [id, value, active]),
  };
}

exercise(24, 4);
const rowCount = 128;
const cycles = 72;
const started = performance.now();
const result = exercise(rowCount, cycles);
const durationMs = performance.now() - started;

emitResult({
  durationMs,
  operations: rowCount * cycles,
  checksum: checksum(JSON.stringify(result)),
  environment: nodeEnvironment(),
});
