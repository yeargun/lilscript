import assert from "node:assert/strict";
import test from "node:test";
import * as reference from "@vue/compiler-sfc";
import * as candidate from "../packages/vuelil/compiler-sfc.js";
import * as internals from "./compiler-sfc-upstream.candidate.mjs";

const reflectedFunctions = new Map([
  ["compileScript", 2],
  ["compileStyle", 1],
  ["compileStyleAsync", 1],
  ["compileTemplate", 1],
  ["extractRuntimeEmits", 1],
  ["extractRuntimeProps", 1],
  ["inferRuntimeType", 2],
  ["invalidateTypeCache", 1],
  ["parse", 1],
  ["registerTS", 1],
  ["resolveTypeElements", 4],
  ["rewriteDefault", 2],
  ["rewriteDefaultAST", 3],
  ["shouldTransformRef", 0],
]);

test("compiler-sfc facade preserves implemented public reflection", () => {
  assert.deepEqual(Object.keys(candidate).sort(), [
    "MagicString",
    "babelParse",
    "compileScript",
    "compileStyle",
    "compileStyleAsync",
    "compileTemplate",
    "errorMessages",
    "extractIdentifiers",
    "extractRuntimeEmits",
    "extractRuntimeProps",
    "generateCodeFrame",
    "inferRuntimeType",
    "invalidateTypeCache",
    "isInDestructureAssignment",
    "isStaticProperty",
    "parse",
    "parseCache",
    "registerTS",
    "resolveTypeElements",
    "rewriteDefault",
    "rewriteDefaultAST",
    "shouldTransformRef",
    "version",
    "walk",
    "walkIdentifiers",
  ]);
  for (const [name, length] of reflectedFunctions) {
    assert.equal(candidate[name].name, name);
    assert.equal(candidate[name].length, length);
  }
  assert.equal(candidate.MagicString.name, reference.MagicString.name);
  assert.equal(candidate.MagicString.length, reference.MagicString.length);
  assert.equal(candidate.babelParse.name, reference.babelParse.name);
  assert.equal(candidate.babelParse.length, reference.babelParse.length);
  assert.equal(candidate.walk.name, reference.walk.name);
  assert.equal(candidate.walk.length, reference.walk.length);
  assert.equal(candidate.version, reference.version);
  assert.deepEqual(candidate.errorMessages, reference.errorMessages);
  assert.equal(candidate.shouldTransformRef(), false);
});

test("rewriteDefault is byte-identical to Vue 3.5.42", () => {
  const fixtures = [
    "export const value = 1",
    "export default { name: 'Demo' }",
    "const App = {}\nexport { App as default, App as named }",
    "export { default, foo } from './component.js'",
    "@sealed\nexport default class Component {}",
  ];
  for (const source of fixtures) {
    const plugins = source.startsWith("@") ? ["decorators-legacy"] : undefined;
    assert.equal(
      candidate.rewriteDefault(source, "__default__", plugins),
      reference.rewriteDefault(source, "__default__", plugins),
    );
  }
});

test("parse descriptors, source maps, and cache identity match Vue 3.5.42", () => {
  const source = `<template lang="pug">\n  div {{ value }}\n</template>\n` +
    `<script setup lang="ts">const value = 1</script>\n` +
    `<style scoped module>div { color: v-bind(value) }</style>\n` +
    `<docs lang="md"># title</docs>`;
  const options = { filename: "src/Fixture.vue", sourceRoot: "/workspace" };
  const actual = candidate.parse(source, options);
  const expected = reference.parse(source, options);
  const serializable = result => ({
    descriptor: {
      ...result.descriptor,
      shouldForceReload: undefined,
    },
    errors: result.errors.map(error => ({ message: error.message, loc: error.loc })),
  });
  assert.deepEqual(serializable(actual), serializable(expected));
  assert.equal(candidate.parse(source, options), actual);
});

test("compileStyle sync and async outputs match Vue 3.5.42", async () => {
  const fixtures = [
    {
      source: ".foo, :deep(.bar) { color: v-bind(theme.color); }",
      filename: "fixture.css",
      id: "data-v-test",
      scoped: true,
    },
    {
      source: "$color: red; .foo { color: $color; }",
      filename: "fixture.scss",
      id: "data-v-test",
      preprocessLang: "scss",
    },
  ];
  for (const options of fixtures) {
    const actual = candidate.compileStyle(options);
    const expected = reference.compileStyle(options);
    assert.equal(actual.code, expected.code);
    if (!options.ssr) assert.deepEqual(actual.map, expected.map);
    assert.deepEqual([...actual.dependencies], [...expected.dependencies]);
    assert.deepEqual(
      actual.errors.map(error => error.message),
      expected.errors.map(error => error.message),
    );
  }

  const moduleOptions = {
    source: ".foo-bar { color: red }",
    filename: "fixture.module.css",
    id: "data-v-test",
    modules: true,
    modulesOptions: { generateScopedName: "[name]__[local]" },
  };
  const actual = await candidate.compileStyleAsync(moduleOptions);
  const expected = await reference.compileStyleAsync(moduleOptions);
  assert.equal(actual.code, expected.code);
  assert.deepEqual(actual.modules, expected.modules);
});

test("CSS variable extraction and code generation preserve exact forms", () => {
  const descriptor = candidate.parse(
    `<script setup>const color = 'red'</script>` +
      `<style>div { color: v-bind(color); width: v-bind('theme.width') }</style>`,
    { filename: "Fixture.vue", sourceMap: false },
  ).descriptor;
  assert.deepEqual(descriptor.cssVars, ["color", "theme.width"]);
  assert.equal(
    internals.genCssVarsFromList(descriptor.cssVars, "abc", false),
    `{
  "abc-color": (color),
  "abc-theme\\.width": (theme.width)
}`,
  );
  assert.equal(
    internals.genCssVarsCode(
      ["color", "theme.width"],
      { __isScriptSetup: true, color: "setup-const" },
      "abc",
      false,
    ),
    `_useCssVars(_ctx => ({
  "abc-color": (color),
  "abc-theme\\.width": (_ctx.theme.width)
}))`,
  );
});

test("cache policy uses bounded LRU semantics in the Node facade", () => {
  const cache = internals.createCache(2);
  cache.set("a", 1);
  cache.set("b", 2);
  cache.get("a");
  cache.set("c", 3);
  assert.equal(cache.has("a"), true);
  assert.equal(cache.has("b"), false);
  assert.equal(cache.has("c"), true);
});

test("defineExpose processor preserves macro recognition and duplicate errors", () => {
  const expression = candidate.babelParse("defineExpose({ foo: 1 })").program.body[0]
    .expression;
  const ordinaryCall = candidate.babelParse("expose({ foo: 1 })").program.body[0]
    .expression;
  const ctx = {
    hasDefineExposeCall: false,
    error(message) {
      throw new Error(message);
    },
  };

  assert.equal(internals.DEFINE_EXPOSE, "defineExpose");
  assert.equal(internals.processDefineExpose(ctx, ordinaryCall), false);
  assert.equal(ctx.hasDefineExposeCall, false);
  assert.equal(internals.processDefineExpose(ctx, expression), true);
  assert.equal(ctx.hasDefineExposeCall, true);
  assert.throws(
    () => internals.processDefineExpose(ctx, expression),
    { message: "duplicate defineExpose() call" },
  );
});

test("defineSlots processor preserves validation and assigned-call rewriting", () => {
  const declaration = candidate.babelParse(
    "const slots = defineSlots<{ default: { msg: string } }>()",
    { sourceType: "module", plugins: ["typescript"] },
  ).program.body[0].declarations[0];
  const overwrites = [];
  const helpers = [];
  const ctx = {
    hasDefineSlotsCall: false,
    startOffset: 17,
    s: {
      overwrite(...args) {
        overwrites.push(args);
      },
    },
    helper(name) {
      helpers.push(name);
      return `_${name}`;
    },
    error(message) {
      throw new Error(message);
    },
  };

  assert.equal(internals.DEFINE_SLOTS, "defineSlots");
  const ordinaryCall = candidate.babelParse("slots()").program.body[0]
    .expression;
  assert.equal(internals.processDefineSlots(ctx, ordinaryCall), false);
  assert.equal(ctx.hasDefineSlotsCall, false);
  assert.equal(
    internals.processDefineSlots(ctx, declaration.init, declaration.id),
    true,
  );
  assert.equal(ctx.hasDefineSlotsCall, true);
  assert.deepEqual(helpers, ["useSlots"]);
  assert.deepEqual(overwrites, [[
    17 + declaration.init.start,
    17 + declaration.init.end,
    "_useSlots()",
  ]]);

  const unassigned = candidate.babelParse("defineSlots()").program.body[0]
    .expression;
  const unassignedCtx = {
    hasDefineSlotsCall: false,
    error(message) {
      throw new Error(message);
    },
  };
  assert.equal(internals.processDefineSlots(unassignedCtx, unassigned), true);
  assert.equal(unassignedCtx.hasDefineSlotsCall, true);

  const withArgument = candidate.babelParse("defineSlots({})").program.body[0]
    .expression;
  assert.throws(
    () => internals.processDefineSlots({ ...unassignedCtx, hasDefineSlotsCall: false }, withArgument),
    { message: "defineSlots() cannot accept arguments" },
  );
  assert.throws(
    () => internals.processDefineSlots(ctx, declaration.init, declaration.id),
    { message: "duplicate defineSlots() call" },
  );
});

test("compileTemplate output and source maps match Vue 3.5.42", () => {
  const fixtures = [
    {
      source: '<div><img src="./logo.png">{{ msg }}</div>',
      filename: "Fixture.vue",
      id: "data-v-test",
      scoped: true,
    },
    {
      source: "div\n  span {{ msg }}",
      filename: "Fixture.vue",
      id: "data-v-test",
      preprocessLang: "pug",
    },
    {
      source: "<div>{{ msg }}</div>",
      filename: "Fixture.vue",
      id: "data-v-test",
      ssr: true,
      ssrCssVars: ["theme.color"],
    },
  ];
  for (const options of fixtures) {
    const actual = candidate.compileTemplate(options);
    const expected = reference.compileTemplate(options);
    assert.equal(actual.code, expected.code);
    assert.deepEqual(actual.map, expected.map);
    assert.deepEqual(actual.tips, expected.tips);
    assert.deepEqual(
      actual.errors.map(error => error.message ?? error),
      expected.errors.map(error => error.message ?? error),
    );
  }
});
