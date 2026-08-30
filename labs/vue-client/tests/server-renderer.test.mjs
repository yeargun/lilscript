import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { before, describe, test } from "node:test";
import * as oracle from "@vue/server-renderer";
import * as oracleVue from "vue";

const root = resolve(import.meta.dirname, "..");
let candidate;
let runtimeCore;
let runtimeDom;
let runtimeTest;

before(async () => {
  const build = spawnSync(process.execPath, [resolve(root, "scripts/build-server-renderer.mjs")], {
    cwd: root,
    encoding: "utf8",
    env: process.env,
  });
  assert.equal(build.status, 0, `${build.stdout}${build.stderr}`);
  [candidate, runtimeCore, runtimeDom, runtimeTest] = await Promise.all([
    import(`../packages/vuelil/server-renderer.js?test=${Date.now()}`),
    import("../packages/vuelil/runtime-core.js"),
    import("../packages/vuelil/runtime-dom.js"),
    import("../packages/vuelil/runtime-test.js"),
  ]);
});

function publicOracle() {
  return Object.fromEntries(Object.entries(oracle).filter(([name]) =>
    !["__esModule", "default", "module.exports"].includes(name)));
}

function collectNodeStream(stream) {
  return new Promise((resolveResult, reject) => {
    let result = "";
    stream.on("data", chunk => { result += chunk; });
    stream.on("error", reject);
    stream.on("end", () => resolveResult(result));
  });
}

async function collectWebStream(stream) {
  const reader = stream.getReader();
  const decoder = new TextDecoder();
  let result = "";
  while (true) {
    const { done, value } = await reader.read();
    if (done) return result;
    result += decoder.decode(value, { stream: true });
  }
}

describe("Vue 3.5.42 server-renderer package", () => {
  test("preserves exactly 26 runtime exports and function reflection", () => {
    const expected = publicOracle();
    assert.equal(Object.keys(expected).length, 26);
    assert.deepEqual(Object.keys(candidate).sort(), Object.keys(expected).sort());
    for (const name of Object.keys(expected)) {
      if (typeof expected[name] === "function") {
        assert.deepEqual(
          [candidate[name].name, candidate[name].length],
          [expected[name].name, expected[name].length],
          name,
        );
      }
    }
    const artifact = readFileSync(resolve(root, "packages/vuelil/server-renderer.js"), "utf8");
    assert.doesNotMatch(artifact, /(?:from|import\()\s*["'](?:vue|@vue\/)/u);
  });

  test("shares one runtime-core identity through runtime-dom and runtime-test", () => {
    for (const name of Object.keys(runtimeCore)) {
      assert.equal(runtimeDom[name], runtimeCore[name], `${name} runtime-dom identity`);
      assert.equal(runtimeTest[name], runtimeCore[name], `${name} runtime-test identity`);
    }
  });

  test("matches upstream SSR helper behavior", () => {
    const attributes = [
      { id: "app", hidden: true, title: '<unsafe>"', class: ["a", { b: true }] },
      { style: [{ color: "red" }, { "--size": 2 }], value: "text" },
      { className: 7, onClick: () => {}, ".value": "ignored", "^data-id": "x" },
    ];
    for (const props of attributes) {
      assert.equal(candidate.ssrRenderAttrs(props, "div"), oracle.ssrRenderAttrs(props, "div"));
    }
    for (const value of [null, "<tag>", ["a", { b: 1 }], { nested: true }]) {
      assert.equal(candidate.ssrInterpolate(value), oracle.ssrInterpolate(value));
    }
    for (const [type, model, value] of [
      ["text", "hello", null],
      ["checkbox", [1, 2], "2"],
      ["radio", 1, "1"],
    ]) {
      assert.equal(
        candidate.ssrRenderDynamicModel(type, model, value),
        oracle.ssrRenderDynamicModel(type, model, value),
      );
    }
  });

  test("matches string, Node stream, and web stream rendering", async () => {
    const CandidateApp = { render: () => runtimeDom.h("main", { class: ["ssr", "ok"] }, "hello") };
    const OracleApp = { render: () => oracleVue.h("main", { class: ["ssr", "ok"] }, "hello") };
    const expected = await oracle.renderToString(oracleVue.createSSRApp(OracleApp));
    assert.equal(await candidate.renderToString(runtimeDom.createSSRApp(CandidateApp)), expected);
    assert.equal(
      await collectNodeStream(candidate.renderToNodeStream(runtimeDom.createSSRApp(CandidateApp))),
      expected,
    );
    assert.equal(
      await collectWebStream(candidate.renderToWebStream(runtimeDom.createSSRApp(CandidateApp))),
      expected,
    );
  });
});
