import assert from "node:assert/strict";
import test from "node:test";
import * as candidate from "../packages/vuelil/compiler-ssr.js";
import * as candidateDOM from "../packages/vuelil/compiler-dom.js";
import * as oracle from "@vue/compiler-ssr";
import * as oracleDOM from "@vue/compiler-dom";

test("compiler-ssr facade preserves the public surface and reflection", () => {
  const oracleExports = Object.keys(oracle)
    .filter(name => !["__esModule", "default", "module.exports"].includes(name))
    .sort();
  assert.deepStrictEqual(Object.keys(candidate).sort(), oracleExports);
  assert.deepStrictEqual(
    [candidate.compile.name, candidate.compile.length],
    [oracle.compile.name, oracle.compile.length],
  );
});

test("compiler-ssr preserves runtime helper symbols and registrations", async () => {
  const internal = await import(
    `./compiler-ssr-upstream.candidate.mjs?helpers=${Date.now()}`
  );
  const helperNames = [
    "SSR_GET_DIRECTIVE_PROPS",
    "SSR_GET_DYNAMIC_MODEL_PROPS",
    "SSR_INCLUDE_BOOLEAN_ATTR",
    "SSR_INTERPOLATE",
    "SSR_LOOSE_CONTAIN",
    "SSR_LOOSE_EQUAL",
    "SSR_RENDER_ATTR",
    "SSR_RENDER_ATTRS",
    "SSR_RENDER_CLASS",
    "SSR_RENDER_COMPONENT",
    "SSR_RENDER_DYNAMIC_ATTR",
    "SSR_RENDER_DYNAMIC_MODEL",
    "SSR_RENDER_LIST",
    "SSR_RENDER_SLOT",
    "SSR_RENDER_SLOT_INNER",
    "SSR_RENDER_STYLE",
    "SSR_RENDER_SUSPENSE",
    "SSR_RENDER_TELEPORT",
    "SSR_RENDER_VNODE",
  ];
  assert.equal(new Set(helperNames.map(name => internal[name])).size, helperNames.length);
  for (const name of helperNames) {
    assert.equal(typeof internal[name], "symbol", name);
    assert.equal(
      candidateDOM.helperNameMap[internal[name]],
      internal.ssrHelpers[internal[name]],
      name,
    );
  }
});

test("compiler-ssr differential output covers all transform families", () => {
  const vectors = [
    "<div/>",
    "<div>{{ value }}</div>",
    '<input type="checkbox" v-model="model" value="x">',
    '<div v-show="visible"/>',
    '<div v-if="ok"/><span v-else/>',
    '<div v-for="(item, key) in list">{{ item }}</div>',
    '<slot name="main"><span/></slot>',
    '<Teleport to="#target"><div/></Teleport>',
    '<Suspense><template #default><Async/></template></Suspense>',
    '<Transition appear><div/></Transition>',
    '<TransitionGroup tag="ul"><li v-for="x in xs" :key="x"/></TransitionGroup>',
    '<Widget v-bind="props"><template #default="p">{{ p.value }}</template></Widget>',
  ];
  for (const source of vectors) {
    assert.equal(candidate.compile(source).code, oracle.compile(source).code, source);
  }
});

test("compiler-ssr compile options match upstream", () => {
  const vectors = [
    ["<div/>", { mode: "module", scopeId: "data-v-test" }],
    ["<div/>", { mode: "function", scopeId: "data-v-test" }],
    ["<div/>", { ssrCssVars: "{ color: color }" }],
    ["<slot/>", { mode: "module", scopeId: "data-v-test", slotted: true }],
  ];
  for (const [source, options] of vectors) {
    const actual = candidate.compile(source, options);
    const expected = oracle.compile(source, options);
    assert.equal(actual.code, expected.code, source);
    assert.deepStrictEqual(
      [...actual.ast.ssrHelpers].map(value => candidateDOM.helperNameMap[value]),
      [...expected.ast.ssrHelpers].map(value => oracleDOM.helperNameMap[value]),
      source,
    );
  }
});
