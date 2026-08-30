import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

const root = resolve(import.meta.dirname, "..");

test("standalone Vue package preserves the VueLil singleton graph", async () => {
  const [vue, runtimeOnly, runtimeDom, reactivity] = await Promise.all([
    import(`../packages/vuelil/vue.js?test=${Date.now()}`),
    import(`../packages/vuelil/vue.runtime.js?test=${Date.now()}`),
    import("../packages/vuelil/runtime-dom.js"),
    import("../packages/vuelil/reactivity.js"),
  ]);
  assert.deepEqual(
    Object.keys(vue).sort(),
    [...new Set([...Object.keys(runtimeDom), "compile"])].sort(),
  );
  assert.deepEqual(Object.keys(runtimeOnly).sort(), Object.keys(vue).sort());
  for (const name of Object.keys(runtimeDom)) {
    assert.equal(vue[name], runtimeDom[name]);
    assert.equal(runtimeOnly[name], runtimeDom[name]);
  }
  for (const name of Object.keys(reactivity).filter(
    name => name in vue && name !== "computed" && name !== "watch",
  )) {
    assert.equal(vue[name], reactivity[name]);
  }
  assert.equal(vue.compile("<div>ok</div>")._rc, true);
  assert.equal(runtimeOnly.compile.name, "compile");
});

test("candidate package exposes build-facing ESM entrypoints", async () => {
  const [rootEntry, full, runtime, compilerSfc, serverRenderer, jsx] = await Promise.all([
    import("../packages/vuelil/index.js"),
    import("../packages/vuelil/dist/vue.esm-bundler.js"),
    import("../packages/vuelil/dist/vue.runtime.esm-bundler.js"),
    import("../packages/vuelil/compiler-sfc/index.js"),
    import("../packages/vuelil/server-renderer/index.js"),
    import("../packages/vuelil/jsx-runtime/index.js"),
  ]);
  assert.equal(rootEntry.compile, runtime.compile);
  assert.equal(full.compile.name, "compileToFunction");
  assert.equal(runtime.compile.name, "compile");
  assert.equal(typeof compilerSfc.parse, "function");
  assert.equal(typeof serverRenderer.renderToString, "function");
  assert.equal(jsx.Fragment, runtime.Fragment);
  assert.equal(jsx.jsx, jsx.jsxs);
  assert.equal(jsx.jsx, jsx.jsxDEV);

  const manifest = JSON.parse(
    readFileSync(resolve(root, "packages/vuelil/package.json"), "utf8"),
  );
  assert.equal(manifest.types, "dist/vue.d.ts");
  assert.equal(manifest.exports["."].import.types, "./dist/vue.d.mts");
  assert.equal(
    manifest.exports["."].import.default,
    "./dist/vue.runtime.esm-bundler.js",
  );
  assert.equal(manifest.exports["./jsx"], "./jsx.d.ts");
});

test("Vue package artifacts contain no upstream implementation imports", () => {
  for (const path of [
    "packages/vuelil/vue.js",
    "packages/vuelil/vue.runtime.js",
    "tests/vue-upstream.candidate.mjs",
  ]) {
    assert.doesNotMatch(
      readFileSync(resolve(root, path), "utf8"),
      /(?:from|import\()\s*["'](?:vue|@vue\/|.*upstream\/vue)/u,
      path,
    );
  }
});
