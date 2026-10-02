import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { compilerPath, projectRoot, repositoryRoot } from "../tooling/compiler-path.mjs";

const sourceDirectory = resolve(projectRoot, "src/runtime-test");
const output = resolve(projectRoot, "packages/vuelil/runtime-test.js");
const temporary = mkdtempSync(resolve(tmpdir(), "vuelil-runtime-test-"));
const graph = resolve(temporary, "src/runtime-test");
const source = resolve(graph, "index.lil");
const compiled = resolve(temporary, "runtime-test.js");

const ownFunctionLengths = new Map([
  ["createApp", 1],
  ["dumpOps", 0],
  ["logNodeOp", 1],
  ["render", 3],
  ["renderToString", 1],
  ["resetOps", 0],
  ["serialize", 1],
  ["serializeInner", 1],
  ["triggerEvent", 2],
]);

const nodeOpMethods = new Map([
  ["insert", 3],
  ["remove", 1],
  ["createElement", 1],
  ["createText", 1],
  ["createComment", 1],
  ["setText", 2],
  ["setElementText", 2],
  ["parentNode", 1],
  ["nextSibling", 1],
  ["querySelector", 0],
  ["setScopeId", 2],
]);

const ownExports = [
  "NodeOpTypes",
  "TestNodeTypes",
  "createApp",
  "dumpOps",
  "logNodeOp",
  "nodeOps",
  "render",
  "renderToString",
  "resetOps",
  "serialize",
  "serializeInner",
  "triggerEvent",
];

function replaceRequired(module, pattern, replacement, description) {
  const replaced = module.replace(pattern, replacement);
  if (replaced === module) throw new Error(`failed to externalize ${description}`);
  return replaced;
}

function importedNames(bindings) {
  return bindings.split(",").map(binding => {
    const [imported, local = imported] = binding.trim().split(/\s+as\s+/);
    return { imported, local };
  });
}

function prepareGraph() {
  mkdirSync(resolve(temporary, "src"), { recursive: true });
  cpSync(sourceDirectory, graph, { recursive: true });

  const dependencies = resolve(temporary, "packages/vuelil");
  mkdirSync(dependencies, { recursive: true });
  for (const name of ["reactivity.js", "runtime-core.internal.js", "shared.js"]) {
    cpSync(resolve(projectRoot, "packages/vuelil", name), resolve(dependencies, name));
  }

  let entry = readFileSync(source, "utf8");
  entry = replaceRequired(
    entry,
    'import { extend } from "../shared/general";',
    [
      'import extern { extend as runtimeTestExtend } from "../../packages/vuelil/shared.js";',
      "extern JsValue runtimeTestExtend;",
    ].join("\n"),
    "runtime-test shared dependency",
  ).replace("JS.call(extend,", "JS.call(runtimeTestExtend,");
  entry = replaceRequired(
    entry,
    /import \{([\s\S]*?)\} from "\.\.\/runtime-core\/index";/,
    (_statement, bindings) => {
      const declarations = importedNames(bindings)
        .map(({ local }) => `extern JsValue ${local};`)
        .join("\n");
      return `import extern {${bindings}} from "../../packages/vuelil/runtime-core.internal.js";\n${declarations}`;
    },
    "runtime-core dependency",
  );
  writeFileSync(source, entry);

  const globals = resolve(graph, "build-globals.lil");
  writeFileSync(globals, [
    "export extern JsValue JSON;",
    "export extern JsValue Object;",
    "export extern JsValue arguments;",
    "export extern JsValue console;",
    "",
  ].join("\n"));

  const nodeOpsPath = resolve(graph, "nodeOps.lil");
  let nodeOps = readFileSync(nodeOpsPath, "utf8");
  nodeOps = replaceRequired(
    nodeOps,
    'import { markRaw } from "../reactivity/reactive";',
    [
      'import extern { markRaw as runtimeTestMarkRaw } from "../../packages/vuelil/reactivity.js";',
      "extern JsValue runtimeTestMarkRaw;",
    ].join("\n"),
    "runtime-test reactivity dependency",
  );
  nodeOps = replaceRequired(
    nodeOps,
    'import { arguments } from "../shared/general";\nimport { console } from "../shared/makeMap";',
    'import { arguments, console } from "./build-globals";',
    "node operation globals",
  ).replaceAll("markRaw(node);", "runtimeTestMarkRaw(node);");
  writeFileSync(nodeOpsPath, nodeOps);

  const patchPropPath = resolve(graph, "patchProp.lil");
  let patchProp = readFileSync(patchPropPath, "utf8");
  patchProp = replaceRequired(
    patchProp,
    'import { isOn } from "../shared/general";',
    [
      'import extern { isOn as runtimeTestPatchIsOn } from "../../packages/vuelil/shared.js";',
      "extern JsValue runtimeTestPatchIsOn;",
    ].join("\n"),
    "patchProp shared dependency",
  ).replace("if (isOn(key))", "if (JS.call(runtimeTestPatchIsOn, JS.undefined(), key).truthy())");
  writeFileSync(patchPropPath, patchProp);

  const serializePath = resolve(graph, "serialize.lil");
  let serialize = readFileSync(serializePath, "utf8");
  serialize = replaceRequired(
    serialize,
    'import { isOn } from "../shared/general";',
    [
      'import extern { isOn as runtimeTestSerializeIsOn } from "../../packages/vuelil/shared.js";',
      "extern JsValue runtimeTestSerializeIsOn;",
    ].join("\n"),
    "serialization shared dependency",
  );
  serialize = replaceRequired(
    serialize,
    'import { JSON, Object } from "../shared/makeMap";',
    'import { JSON, Object } from "./build-globals";',
    "serialization globals",
  ).replace("if (isOn(key) ||", "if (JS.call(runtimeTestSerializeIsOn, JS.undefined(), key).truthy() ||");
  writeFileSync(serializePath, serialize);

  const triggerPath = resolve(graph, "triggerEvent.lil");
  let trigger = readFileSync(triggerPath, "utf8");
  trigger = replaceRequired(
    trigger,
    'import { arguments, isArray } from "../shared/general";',
    [
      'import extern { isArray as runtimeTestIsArray } from "../../packages/vuelil/shared.js";',
      'import { arguments } from "./build-globals";',
      "extern JsValue runtimeTestIsArray;",
    ].join("\n"),
    "event shared dependency",
  ).replace(
    "JS.call(isArray, JS.undefined(), listener)",
    "JS.call(runtimeTestIsArray, JS.undefined(), listener)",
  );
  writeFileSync(triggerPath, trigger);
}

function compile() {
  const result = spawnSync(
    compilerPath(),
    [
      source,
      "--target", "js-module",
      "--mode", "development",
      "--config", resolve(repositoryRoot, "tests/config/no-optimization-no-peephole.toml"),
      "--jobs", "1",
      "--codec-jobs", "1",
      "-o", compiled,
    ],
    { cwd: projectRoot, encoding: "utf8", env: process.env },
  );
  if (result.status !== 0) throw new Error(`${result.stdout ?? ""}${result.stderr ?? ""}`);
}

function parseExports(bindings) {
  return bindings.split(",").map(binding => {
    const [local, publicName = local] = binding.trim().split(/\s+as\s+/);
    return { local, publicName };
  });
}

function descriptor(target, name, length) {
  return `Object.defineProperties(${target},{name:{configurable:true,value:${JSON.stringify(name)}},length:{configurable:true,value:${length}}});`;
}

function reflectRuntimeTest(module) {
  return module.replace(/export\{([^}]*)\}\s*$/, (statement, bindings) => {
    const entries = parseExports(bindings);
    const definitions = [];
    for (const { local, publicName } of entries) {
      if (ownFunctionLengths.has(publicName)) {
        definitions.push(descriptor(local, publicName, ownFunctionLengths.get(publicName)));
      }
      if (publicName === "nodeOps") {
        for (const [name, length] of nodeOpMethods) {
          definitions.push(descriptor(`${local}.${name}`, name, length));
        }
      }
    }
    return `${definitions.join("")}${statement}`;
  });
}

try {
  prepareGraph();
  compile();

  let module = readFileSync(compiled, "utf8")
    .replaceAll('"../../packages/vuelil/runtime-core.internal.js"', '"./runtime-core.internal.js"')
    .replaceAll('"../../packages/vuelil/reactivity.js"', '"./reactivity.js"')
    .replaceAll('"../../packages/vuelil/shared.js"', '"./shared.js"');
  if (/(?:\.\.\/){2}packages\/vuelil\//.test(module)) {
    throw new Error("failed to rewrite a runtime-test dependency");
  }
  module = reflectRuntimeTest(module);

  const banner = "// Generated from the five mirrored src/runtime-test/*.lil owners.\n";
  mkdirSync(resolve(projectRoot, "packages/vuelil"), { recursive: true });
  writeFileSync(output, `${banner}${module}\n`);

  const stamp = Date.now();
  const [runtimeTest, runtimeCore, runtimeCoreInternal] = await Promise.all([
    import(`../packages/vuelil/runtime-test.js?build=${stamp}`),
    import(`../packages/vuelil/runtime-core.js?build=${stamp}`),
    import("../packages/vuelil/runtime-core.internal.js"),
  ]);
  const expected = [...new Set([...Object.keys(runtimeCore), ...ownExports])].sort();
  assert.deepEqual(Object.keys(runtimeTest).sort(), expected, "runtime-test export surface");
  for (const name of Object.keys(runtimeCore)) {
    assert.equal(runtimeTest[name], runtimeCore[name], `${name} public runtime-core identity`);
    assert.equal(runtimeTest[name], runtimeCoreInternal[name], `${name} runtime-core identity`);
  }
  assert.doesNotMatch(module, /(?:from|import\()\s*["'](?:vue|@vue\/)/);

  console.log(JSON.stringify({
    output,
    exports: Object.keys(runtimeTest).sort(),
    bytes: readFileSync(output).byteLength,
  }));
} finally {
  rmSync(temporary, { force: true, recursive: true });
}
