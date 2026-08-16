import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { build } from "vite";
import {
  canonicalCodecProvenance,
  canonicalCodecSizes,
} from "../../../benchmarks/codec-contract.mjs";
import {
  bundleSolidLilCandidate,
  selectSolidLilDistribution,
} from "./distribution-selection.mjs";
import { root } from "./project.mjs";

const generated = resolve(root, "artifacts/generated");
const buildModesPath = resolve(root, "artifacts/build-modes.json");
const buildModesBytes = readFileSync(buildModesPath);
const buildModes = JSON.parse(buildModesBytes);
mkdirSync(generated, { recursive: true });

async function bundle(entry, output) {
  const result = await build({
    configFile: false,
    root,
    logLevel: "error",
    resolve: {
      conditions: ["browser", "module", "import", "default"],
    },
    build: {
      target: "es2022",
      minify: "oxc",
      write: false,
      lib: { entry, formats: ["es"], fileName: "bundle" },
      rolldownOptions: { output: { codeSplitting: false } },
    },
  });
  const outputs = Array.isArray(result)
    ? result.flatMap((item) => item.output)
    : result.output;
  const chunks = outputs.filter((item) => item.type === "chunk");
  assert.equal(chunks.length, 1, `${entry} should emit one JavaScript chunk`);
  const code = `${chunks[0].code.trim()}\n`;
  writeFileSync(output, code);
  return code;
}

async function importFresh(path) {
  return import(`${pathToFileURL(path).href}?v=${Date.now()}-${Math.random()}`);
}

function size(code) {
  const measured = canonicalCodecSizes(
    code,
    "SolidLil Store surface verification",
  );
  return {
    brotli11: measured.brotli,
    gzip9: measured.gzip,
    raw: measured.raw,
  };
}

function behaviorDigest(harness) {
  const covered = new Set();
  const store = new Proxy(harness.store, {
    get(target, property, receiver) {
      if (typeof property === "string") covered.add(property);
      return Reflect.get(target, property, receiver);
    },
  });
  const snapshots = [];
  const [state, setState] = store.createStore({
    flags: { first: true, second: false },
    meta: { removable: "yes", stable: 1 },
    todos: [
      { id: 1, text: "one", done: false },
      { id: 2, text: "two", done: false },
    ],
    user: { name: "Ada", role: "author" },
  });
  let dispose = () => {};
  harness.createRoot((rootDispose) => {
    dispose = rootDispose;
    harness.createEffect(() => {
      snapshots.push({
        hasExtra: "extra" in state.meta,
        keys: Object.keys(state.meta).sort().join(","),
        name: state.user.name,
        summary: state.todos
          .map((todo) => `${todo.id}:${todo.text}:${todo.done}`)
          .join("|"),
      });
    });
  });

  const immutableName = state.user.name;
  state.user.name = "ignored";
  const immutableWriteIgnored = state.user.name === immutableName;
  setState("user", "name", "Grace");
  setState("todos", 0, "done", true);
  setState(
    "todos",
    (todo) => todo.id === 2,
    "text",
    (text) => text.toUpperCase(),
  );
  setState("todos", { from: 0, to: 1, by: 1 }, "visited", true);
  setState("flags", ["first", "second"], (value) => !value);
  setState("meta", { extra: 2, stable: 3 });
  setState("meta", "removable", undefined);

  const preservedTodo = state.todos[1];
  setState(
    "todos",
    store.reconcile([
      { id: 2, text: "TWO reconciled", done: true },
      { id: 3, text: "three", done: false },
    ]),
  );
  const reconcilePreservedIdentity = state.todos[0] === preservedTodo;

  setState(
    store.produce((draft) => {
      draft.user.role = "maintainer";
      draft.todos.push({ id: 4, text: "four", done: false });
    }),
  );

  const [array, setArray] = store.createStore([1, 2, 3]);
  setArray((current) => [...current, 4]);
  setArray([9, 8]);

  const mutableSnapshots = [];
  const mutable = store.createMutable({ count: 1, nested: { value: "a" } });
  let disposeMutable = () => {};
  harness.createRoot((rootDispose) => {
    disposeMutable = rootDispose;
    harness.createEffect(() =>
      mutableSnapshots.push(`${mutable.count}:${mutable.nested.value}`),
    );
  });
  mutable.count = 2;
  store.modifyMutable(
    mutable,
    store.produce((draft) => {
      draft.count += 3;
      draft.nested.value = "b";
    }),
  );

  const rawState = state[store.$RAW];
  const unwrapped = store.unwrap(state);
  const rawIdentity = rawState === unwrapped;
  const dev = store.DEV;

  const beforeDispose = snapshots.length;
  dispose();
  setState("user", "name", "after dispose");
  const parentStoppedAfterDispose = snapshots.length === beforeDispose;
  const mutableBeforeDispose = mutableSnapshots.length;
  disposeMutable();
  mutable.count = 99;
  const mutableStoppedAfterDispose =
    mutableSnapshots.length === mutableBeforeDispose;

  assert.deepEqual(
    [...covered].sort(),
    Object.keys(harness.store).sort(),
    "every Solid Store export needs executable coverage",
  );

  return {
    array: [...array],
    dev: dev === undefined ? "undefined" : String(dev),
    immutableWriteIgnored,
    mutable: store.unwrap(mutable),
    mutableSnapshots,
    mutableStoppedAfterDispose,
    parentStoppedAfterDispose,
    rawIdentity,
    reconcilePreservedIdentity,
    snapshots,
    state: store.unwrap(state),
  };
}

const paths = {
  solid: {
    api: resolve(root, "api/solid-store.js"),
    apiOutput: resolve(generated, "solid-store.js"),
    harness: resolve(root, "api/solid-store-harness.js"),
    harnessOutput: resolve(generated, "solid-store-harness.js"),
  },
  solidlil: {
    api: resolve(root, "api/solidlil-store.js"),
    apiOutput: resolve(generated, "solidlil-store.js"),
    harness: resolve(root, "api/solidlil-store-harness.js"),
    harnessOutput: resolve(generated, "solidlil-store-harness.js"),
  },
};
const code = {};
const modules = {};
const harnesses = {};
for (const name of Object.keys(paths)) {
  if (name === "solidlil") {
    const selected = await selectSolidLilDistribution({
      entry: paths[name].api,
      output: paths[name].apiOutput,
      target: "store-open",
    });
    code[name] = selected.code;
    paths[name].distributionSelection = selected.selection;
    await bundleSolidLilCandidate({
      candidateId: selected.selection.winner,
      entry: paths[name].harness,
      output: paths[name].harnessOutput,
    });
  } else {
    code[name] = await bundle(paths[name].api, paths[name].apiOutput);
    await bundle(paths[name].harness, paths[name].harnessOutput);
  }
  modules[name] = await importFresh(paths[name].apiOutput);
  harnesses[name] = await importFresh(paths[name].harnessOutput);
}
assert.deepEqual(
  Object.keys(modules.solidlil).sort(),
  Object.keys(modules.solid).sort(),
  "Solid Store exact exports",
);
const behavior = {
  solid: behaviorDigest(harnesses.solid),
  solidlil: behaviorDigest(harnesses.solidlil),
};
assert.deepEqual(behavior.solidlil, behavior.solid, "Solid Store behavior");

const sizes = {
  solid: size(code.solid),
  solidlil: size(code.solidlil),
};
const ratio = Object.fromEntries(
  Object.keys(sizes.solid).map((metric) => [
    metric,
    sizes.solidlil[metric] / sizes.solid[metric],
  ]),
);
const report = {
  schemaVersion: 2,
  generatedAt: new Date().toISOString(),
  baseline: "solid-js@1.9.13 store browser bundle",
  exports: Object.keys(modules.solid).sort(),
  exportCount: Object.keys(modules.solid).length,
  exactExports: true,
  behaviorEquivalent: true,
  distributionSelection: paths.solidlil.distributionSelection,
  codecs: canonicalCodecProvenance("SolidLil Store surface verification"),
  compiler: buildModes.toolchain.compiler,
  sourceBuildModesSha256: createHash("sha256")
    .update(buildModesBytes)
    .digest("hex"),
  sizes,
  ratio,
  brotliSuperior: sizes.solidlil.brotli11 < sizes.solid.brotli11,
};
assert.deepEqual(report.codecs, buildModes.toolchain.codecs);
writeFileSync(
  resolve(root, "artifacts/store-surface.json"),
  `${JSON.stringify(report, null, 2)}\n`,
);
writeFileSync(
  resolve(root, "artifacts/store-surface.md"),
  `# SolidLil Store verified surface\n\nAll ${report.exportCount} browser exports are exact and behavior-equivalent for immutable stores, mutable stores, path updates, keyed reconciliation, producer updates, nested tracking, and disposal.\n\n| Metric | Solid | SolidLil | Ratio |\n| --- | ---: | ---: | ---: |\n| Brotli-11 | ${sizes.solid.brotli11} B | ${sizes.solidlil.brotli11} B | ${ratio.brotli11.toFixed(3)} |\n| Gzip-9 | ${sizes.solid.gzip9} B | ${sizes.solidlil.gzip9} B | ${ratio.gzip9.toFixed(3)} |\n| Raw | ${sizes.solid.raw} B | ${sizes.solidlil.raw} B | ${ratio.raw.toFixed(3)} |\n`,
);
console.log(
  `SolidLil Store: ${report.exportCount} exports verified; Brotli-11 ${sizes.solidlil.brotli11} B vs Solid ${sizes.solid.brotli11} B (${ratio.brotli11.toFixed(3)}x).`,
);
