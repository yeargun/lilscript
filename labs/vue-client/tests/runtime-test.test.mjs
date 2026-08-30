import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { after, before, describe, test } from "node:test";
import { createServer } from "vite";
import { entries as vueSourceEntries } from "../upstream/vue/scripts/aliases.js";

const root = resolve(import.meta.dirname, "..");
const upstream = resolve(root, "upstream/vue");
const evidence = JSON.parse(
  readFileSync(resolve(root, "compatibility/runtime-test.json"), "utf8"),
);
let candidate;
let core;
let internal;
let oracle;
let server;

before(async () => {
  const build = spawnSync(process.execPath, [resolve(root, "scripts/build-runtime-test.mjs")], {
    cwd: root,
    encoding: "utf8",
    env: process.env,
  });
  assert.equal(build.status, 0, `${build.stdout}${build.stderr}`);

  candidate = await import(`../packages/vuelil/runtime-test.js?test=${Date.now()}`);
  core = await import("../packages/vuelil/runtime-core.js");
  internal = await import("../packages/vuelil/runtime-core.internal.js");

  server = await createServer({
    root: upstream,
    configFile: false,
    logLevel: "silent",
    server: { middlewareMode: true },
    resolve: { alias: vueSourceEntries },
    define: {
      __DEV__: true,
      __TEST__: true,
      __VERSION__: JSON.stringify("3.5.42"),
      __BROWSER__: false,
      __GLOBAL__: false,
      __ESM_BUNDLER__: true,
      __ESM_BROWSER__: false,
      __CJS__: true,
      __SSR__: false,
      __FEATURE_OPTIONS_API__: true,
      __FEATURE_SUSPENSE__: true,
      __FEATURE_PROD_DEVTOOLS__: false,
      __FEATURE_PROD_HYDRATION_MISMATCH_DETAILS__: false,
      __COMPAT__: false,
    },
  });
  oracle = await server.ssrLoadModule("/packages/runtime-test/src/index.ts");
  const upstreamPackage = JSON.parse(readFileSync(resolve(upstream, "package.json"), "utf8"));
  assert.equal(upstreamPackage.version, "3.5.42");
});

after(async () => {
  await server?.close();
});

function reflectFunctions(api, names) {
  return Object.fromEntries(names.map(name => [name, [api[name].name, api[name].length]]));
}

function summarizeValue(value) {
  if (typeof value === "function") return `[function ${value.name}/${value.length}]`;
  if (Array.isArray(value)) return value.map(summarizeValue);
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.keys(value).map(key => [key, summarizeValue(value[key])]));
  }
  return value;
}

function summarizeOps(ops) {
  return ops.map(op => ({
    keys: Object.keys(op),
    type: op.type,
    nodeType: op.nodeType,
    tag: op.tag,
    text: op.text,
    targetId: op.targetNode?.id,
    targetType: op.targetNode?.type,
    parentId: op.parentNode?.id,
    refId: op.refNode === null ? null : op.refNode?.id,
    propKey: op.propKey,
    propPrevValue: summarizeValue(op.propPrevValue),
    propNextValue: summarizeValue(op.propNextValue),
  }));
}

function nodeOperationScenario(api) {
  const root = api.nodeOps.createElement("root");
  api.resetOps();
  const element = api.nodeOps.createElement("button");
  const text = api.nodeOps.createText("before");
  const comment = api.nodeOps.createComment("tail");
  api.nodeOps.insert(element, root, null);
  api.nodeOps.insert(comment, root, null);
  api.nodeOps.insert(comment, root, element);
  api.nodeOps.insert(text, element, null);
  api.nodeOps.setText(text, "after");
  api.nodeOps.remove(text);
  api.nodeOps.setScopeId(element, "data-v-owner");
  api.nodeOps.setElementText(element, "label");
  const operations = summarizeOps(api.dumpOps());
  return {
    root: {
      type: root.type,
      children: root.children.map(child => child.id),
    },
    element: {
      type: element.type,
      tag: element.tag,
      parentId: element.parentNode.id,
      props: element.props,
      children: element.children.map(child => ({
        id: child.id,
        type: child.type,
        text: child.text,
        parentIdentity: child.parentNode === element,
      })),
      rawIdentity: api.reactive(element) === element,
    },
    comment: {
      type: comment.type,
      parentId: comment.parentNode.id,
    },
    operations,
    drained: api.dumpOps(),
  };
}

async function rendererScenario(api) {
  const calls = [];
  const count = api.ref(0);
  const root = api.nodeOps.createElement("root");
  const App = () => api.h("article", {
    id: `item-${count.value}`,
    empty: "",
    absent: null,
    onClick: [
      (amount, label) => calls.push(["first", amount, label]),
      amount => calls.push(["second", amount]),
    ],
    "on:update:modelValue": amount => calls.push(["model", amount]),
  }, [
    api.h("span", String(count.value)),
    api.createCommentVNode("note"),
  ]);

  api.resetOps();
  api.render(api.h(App), root);
  const initial = {
    compact: api.serialize(root),
    indented: api.serialize(root, 2),
    operations: summarizeOps(api.dumpOps()),
  };
  const element = root.children[0];
  api.triggerEvent(element, "click", [2, "payload"]);
  api.triggerEvent(element, "update:modelValue", [3]);
  count.value = 1;
  await api.nextTick();
  const updated = {
    compact: api.serialize(root),
    operations: summarizeOps(api.dumpOps()),
    calls,
  };
  const oneOff = api.renderToString(api.h("p", { title: "one-off" }, "value"));
  api.resetOps();
  api.render(null, root);
  const unmounted = {
    children: root.children.length,
    operations: summarizeOps(api.dumpOps()),
  };
  return { initial, updated, oneOff, unmounted };
}

describe("Vue 3.5.42 runtime-test package", () => {
  test("matches the complete export surface, reflection, and runtime-core identities", () => {
    const expected = Object.keys(oracle).sort();
    assert.equal(expected.length, 158);
    assert.deepEqual(Object.keys(candidate).sort(), expected);
    assert.equal(evidence.aggregateExportCount, 158);
    assert.deepEqual(evidence.aggregateExports, expected);

    const functions = [
      "createApp", "dumpOps", "logNodeOp", "render", "renderToString",
      "resetOps", "serialize", "serializeInner", "triggerEvent",
    ];
    assert.deepEqual(reflectFunctions(candidate, functions), reflectFunctions(oracle, functions));
    assert.deepEqual(
      reflectFunctions(candidate.nodeOps, Object.keys(oracle.nodeOps)),
      reflectFunctions(oracle.nodeOps, Object.keys(oracle.nodeOps)),
    );

    for (const name of Object.keys(core)) {
      assert.equal(candidate[name], core[name], `${name} public identity`);
      assert.equal(candidate[name], internal[name], `${name} internal identity`);
    }
    assert.deepEqual(candidate.NodeOpTypes, oracle.NodeOpTypes);
    assert.deepEqual(candidate.TestNodeTypes, oracle.TestNodeTypes);

    const artifact = readFileSync(resolve(root, "packages/vuelil/runtime-test.js"), "utf8");
    assert.match(artifact, /from["']\.\/runtime-core\.internal\.js["']/);
    assert.doesNotMatch(artifact, /(?:from|import\()\s*["'](?:vue|@vue\/)/);
  });

  test("matches upstream node operations exactly", () => {
    const actual = nodeOperationScenario(candidate);
    const expected = nodeOperationScenario(oracle);
    assert.deepEqual(actual, expected);
    assert.deepEqual(actual.operations.map(op => op.type), [
      "create", "create", "create", "insert", "insert", "insert", "insert",
      "setText", "remove", "setElementText",
    ]);
  });

  test("matches serialization, event triggering, and renderer updates", async () => {
    const actual = await rendererScenario(candidate);
    const expected = await rendererScenario(oracle);
    assert.deepEqual(actual, expected);
    assert.equal(
      actual.initial.compact,
      '<root><article id="item-0" empty><span>0</span><!--note--></article></root>',
    );
    assert.equal(actual.updated.compact, '<root><article id="item-1" empty><span>1</span><!--note--></article></root>');
    assert.deepEqual(actual.updated.calls, [
      ["first", 2, "payload"],
      ["second", 2],
      ["model", 3],
    ]);
    assert.equal(actual.oneOff, '<p title="one-off">value</p>');
    assert.equal(actual.unmounted.children, 0);
  });
});
