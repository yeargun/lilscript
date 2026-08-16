import assert from "node:assert/strict";
import { pathToFileURL } from "node:url";

if (typeof globalThis.gc !== "function") {
  throw new Error("lifecycle memory benchmark requires --expose-gc");
}

const [name, bundlePath, cyclesText, diagnosticsPath] = process.argv.slice(2);
const cycles = Number(cyclesText);
const url = pathToFileURL(bundlePath);
url.searchParams.set("sample", `${process.pid}-${Date.now()}`);
const api = await import(url.href);
const diagnostics = diagnosticsPath
  ? await import(pathToFileURL(diagnosticsPath).href)
  : null;

function collect() {
  for (let index = 0; index < 4; index += 1) globalThis.gc();
  return process.memoryUsage().heapUsed;
}

function slotSnapshot() {
  return diagnostics
    ? {
        owners: diagnostics.ownerSlots(),
        effects: diagnostics.effectSlots(),
        freeOwners: diagnostics.freeOwnerSlots(),
        freeEffects: diagnostics.freeEffectSlots(),
        pendingEffects: diagnostics.pendingEffects(),
      }
    : null;
}

function assertReleased(slots, label) {
  if (!slots) return;
  assert.equal(slots.owners, slots.freeOwners, `${name}: ${label} owners`);
  assert.equal(slots.effects, slots.freeEffects, `${name}: ${label} effects`);
  assert.equal(slots.pendingEffects, 0, `${name}: ${label} pending effects`);
}

function churnRoots(count) {
  let cleanupCount = 0;
  let effectRuns = 0;
  let checksum = 0;
  for (let index = 0; index < count; index += 1) {
    let update = () => {};
    const dispose = api.createRoot((rootDispose) => {
      const payload = Array.from({ length: 64 }, (_, item) => index + item);
      const [value, setValue] = api.createSignal(index);
      update = setValue;
      const doubled = api.createMemo(() => value() * 2);
      api.createEffect(() => {
        checksum = (checksum + doubled()) | 0;
        effectRuns += 1;
      });
      api.onCleanup(() => {
        checksum = (checksum + payload[0]) | 0;
        cleanupCount += 1;
      });
      return rootDispose;
    });
    update(index + 1);
    dispose();
  }
  return { cleanupCount, effectRuns, checksum };
}

function verifyStaleDisposer() {
  let staleCleanups = 0;
  const staleDispose = api.createRoot((dispose) => {
    api.onCleanup(() => {
      staleCleanups += 1;
    });
    return dispose;
  });
  staleDispose();
  staleDispose();

  let freshCleanups = 0;
  let freshRuns = 0;
  let setFresh = () => {};
  const freshDispose = api.createRoot((dispose) => {
    const [fresh, writeFresh] = api.createSignal(0);
    setFresh = writeFresh;
    api.createEffect(() => {
      fresh();
      freshRuns += 1;
    });
    api.onCleanup(() => {
      freshCleanups += 1;
    });
    return dispose;
  });

  // Solid's disposer is permanently tied to its original owner. Calling it
  // after an internal slot is reused must not dispose the new root.
  staleDispose();
  setFresh(1);
  assert.equal(staleCleanups, 1, `${name}: stale disposer is idempotent`);
  assert.equal(
    freshCleanups,
    0,
    `${name}: stale disposer preserves fresh root`,
  );
  assert.equal(freshRuns, 2, `${name}: fresh root remains reactive`);
  freshDispose();
  freshDispose();
  assert.equal(freshCleanups, 1, `${name}: fresh disposer is idempotent`);
  return { staleCleanups, freshCleanups, freshRuns };
}

function churnCollections(count) {
  let createdRows = 0;
  let cleanedRows = 0;
  let effectRuns = 0;
  let checksum = 0;

  for (let cycle = 0; cycle < count; cycle += 1) {
    const makeRow = (id) => ({
      id,
      payload: Array.from({ length: 16 }, (_, index) => cycle + id + index),
    });
    const first = makeRow(1);
    const second = makeRow(2);
    const third = makeRow(3);
    const fourth = makeRow(4);
    const collection = api.createRoot((rootDispose) => {
      const [rows, setRows] = api.createSignal([first, second, third]);
      const mapped = api.mapArray(rows, (row, index) => {
        createdRows += 1;
        api.onCleanup(() => {
          cleanedRows += 1;
        });
        const value = api.createMemo(() => row.payload[0] + index());
        api.createEffect(() => {
          checksum = (checksum + value()) | 0;
          effectRuns += 1;
        });
        return value;
      });
      const indexed = api.indexArray(rows, (row, index) => {
        createdRows += 1;
        api.onCleanup(() => {
          cleanedRows += 1;
        });
        const value = api.createMemo(() => row().payload[0] + index);
        api.createEffect(() => {
          checksum = (checksum + value()) | 0;
          effectRuns += 1;
        });
        return value;
      });
      return { dispose: rootDispose, indexed, mapped, setRows };
    });
    const read = (accessors) => {
      for (const accessor of accessors())
        checksum = (checksum + accessor()) | 0;
    };
    read(collection.mapped);
    read(collection.indexed);
    collection.setRows([third, first, fourth]);
    read(collection.mapped);
    read(collection.indexed);
    collection.setRows([fourth]);
    read(collection.mapped);
    read(collection.indexed);
    collection.setRows([]);
    read(collection.mapped);
    read(collection.indexed);
    collection.dispose();
    collection.dispose();
  }

  assert.equal(
    cleanedRows,
    createdRows,
    `${name}: every mapped and indexed row cleaned up`,
  );
  return { createdRows, cleanedRows, effectRuns, checksum };
}

async function churnDisposedResources(count) {
  const pending = [];
  let cleanupCount = 0;
  let effectRuns = 0;
  for (let cycle = 0; cycle < count; cycle += 1) {
    let resolveResource;
    const promise = new Promise((resolve) => {
      resolveResource = resolve;
    });
    const dispose = api.createRoot((rootDispose) => {
      const [resource] = api.createResource(() => promise);
      api.createEffect(() => {
        resource();
        effectRuns += 1;
      });
      api.onCleanup(() => {
        cleanupCount += 1;
      });
      return rootDispose;
    });
    dispose();
    dispose();
    pending.push({ promise, resolveResource, effectRuns });
  }
  const runsBeforeResolution = effectRuns;
  for (let index = 0; index < pending.length; index += 1) {
    pending[index].resolveResource(index);
  }
  await Promise.all(pending.map(({ promise }) => promise));
  await Promise.resolve();
  await Promise.resolve();
  assert.equal(
    cleanupCount,
    count,
    `${name}: disposed resource roots clean up`,
  );
  assert.equal(
    effectRuns,
    runsBeforeResolution,
    `${name}: late resource results cannot revive disposed effects`,
  );
  return { cleanupCount, effectRuns, runsBeforeResolution };
}

const collectionCycles = Math.max(100, Math.min(2500, Math.floor(cycles / 10)));
const resourceCycles = Math.max(32, Math.min(256, Math.floor(cycles / 50)));
const warmRootCycles = Math.min(cycles, Math.max(1000, Math.floor(cycles / 5)));
const warmCollectionCycles = Math.min(
  collectionCycles,
  Math.max(100, Math.floor(collectionCycles / 5)),
);

// Reach both the runtime pool high-water mark and V8's hot-code tier before the
// retained-heap baseline. Otherwise one fresh process may charge TurboFan/JIT
// metadata to the measured workload while another happens to tier up during
// the short warmup, producing a false bimodal "retention" signal.
churnRoots(warmRootCycles);
churnCollections(warmCollectionCycles);
await churnDisposedResources(resourceCycles);
verifyStaleDisposer();
const baseline = collect();
const warmSlots = slotSnapshot();
assertReleased(warmSlots, "warmup");
const roots = churnRoots(cycles);
const collections = churnCollections(collectionCycles);
const resources = await churnDisposedResources(resourceCycles);
const staleDisposer = verifyStaleDisposer();
const after = collect();
const slots = slotSnapshot();
assertReleased(slots, "completed workload");
if (slots) {
  assert.equal(
    slots.owners,
    warmSlots.owners,
    `${name}: stable owner high-water`,
  );
  assert.equal(
    slots.effects,
    warmSlots.effects,
    `${name}: stable effect high-water`,
  );
}

assert.equal(roots.cleanupCount, cycles, `${name}: every root cleaned up`);
assert.equal(roots.effectRuns, cycles * 2, `${name}: effect lifecycle`);

process.stdout.write(
  `${JSON.stringify({
    bytes: Math.max(0, after - baseline),
    bytesPerCycle: Math.max(0, after - baseline) / cycles,
    cycles,
    collectionCycles,
    resourceCycles,
    warmRootCycles,
    warmCollectionCycles,
    warmSlots,
    slots,
    roots,
    collections,
    resources,
    staleDisposer,
  })}\n`,
);
