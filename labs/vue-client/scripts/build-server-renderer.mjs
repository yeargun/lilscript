import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, relative, resolve } from "node:path";
import { compilerPath, projectRoot, repositoryRoot } from "../tooling/compiler-path.mjs";

const sourceDirectory = resolve(projectRoot, "src/server-renderer");
const output = resolve(projectRoot, "packages/vuelil/server-renderer.js");
const testOutput = resolve(projectRoot, "tests/server-renderer-upstream.candidate.mjs");
const temporary = mkdtempSync(resolve(tmpdir(), "vuelil-server-renderer-"));
const graph = resolve(temporary, "src/server-renderer");
const source = resolve(graph, "index.lil");
const compiled = resolve(temporary, "server-renderer.js");

const expectedExports = [
  "pipeToNodeWritable",
  "pipeToWebWritable",
  "renderToNodeStream",
  "renderToSimpleStream",
  "renderToStream",
  "renderToString",
  "renderToWebStream",
  "ssrGetDirectiveProps",
  "ssrGetDynamicModelProps",
  "ssrIncludeBooleanAttr",
  "ssrInterpolate",
  "ssrLooseContain",
  "ssrLooseEqual",
  "ssrRenderAttr",
  "ssrRenderAttrs",
  "ssrRenderClass",
  "ssrRenderComponent",
  "ssrRenderDynamicAttr",
  "ssrRenderDynamicModel",
  "ssrRenderList",
  "ssrRenderSlot",
  "ssrRenderSlotInner",
  "ssrRenderStyle",
  "ssrRenderSuspense",
  "ssrRenderTeleport",
  "ssrRenderVNode",
].sort();

const functionReflection = new Map([
  ["pipeToNodeWritable", ["pipeToNodeWritable", 1]],
  ["pipeToWebWritable", ["pipeToWebWritable", 1]],
  ["renderToNodeStream", ["renderToNodeStream", 1]],
  ["renderToSimpleStream", ["renderToSimpleStream", 3]],
  ["renderToStream", ["renderToStream", 1]],
  ["renderToString", ["renderToString", 1]],
  ["renderToWebStream", ["renderToWebStream", 1]],
  ["ssrGetDirectiveProps", ["ssrGetDirectiveProps", 4]],
  ["ssrGetDynamicModelProps", ["ssrGetDynamicModelProps", 0]],
  ["ssrIncludeBooleanAttr", ["includeBooleanAttr", 1]],
  ["ssrInterpolate", ["ssrInterpolate", 1]],
  ["ssrLooseContain", ["ssrLooseContain", 2]],
  ["ssrLooseEqual", ["looseEqual", 2]],
  ["ssrRenderAttr", ["ssrRenderAttr", 2]],
  ["ssrRenderAttrs", ["ssrRenderAttrs", 2]],
  ["ssrRenderClass", ["ssrRenderClass", 1]],
  ["ssrRenderComponent", ["ssrRenderComponent", 1]],
  ["ssrRenderDynamicAttr", ["ssrRenderDynamicAttr", 3]],
  ["ssrRenderDynamicModel", ["ssrRenderDynamicModel", 3]],
  ["ssrRenderList", ["ssrRenderList", 2]],
  ["ssrRenderSlot", ["ssrRenderSlot", 7]],
  ["ssrRenderSlotInner", ["ssrRenderSlotInner", 8]],
  ["ssrRenderStyle", ["ssrRenderStyle", 1]],
  ["ssrRenderSuspense", ["ssrRenderSuspense", 2]],
  ["ssrRenderTeleport", ["ssrRenderTeleport", 5]],
  ["ssrRenderVNode", ["renderVNode", 4]],
]);

function lilFiles(root) {
  const files = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const path = resolve(root, entry.name);
    if (entry.isDirectory()) files.push(...lilFiles(path));
    else if (entry.name.endsWith(".lil")) files.push(path);
  }
  return files;
}

function specifier(from, target) {
  const path = relative(dirname(from), target)
    .replaceAll("\\", "/")
    .replace(/\.lil$/u, "");
  return path.startsWith(".") ? path : `./${path}`;
}

function prepareGraph() {
  mkdirSync(resolve(temporary, "src"), { recursive: true });
  cpSync(sourceDirectory, graph, { recursive: true });

  const dependencies = resolve(temporary, "packages/vuelil");
  mkdirSync(dependencies, { recursive: true });
  for (const name of ["compiler-ssr.js", "runtime-dom.js", "shared.js"]) {
    copyFileSync(resolve(projectRoot, "packages/vuelil", name), resolve(dependencies, name));
  }

  const globals = new Map();
  const files = lilFiles(graph);
  for (const path of files) {
    const module = readFileSync(path, "utf8");
    const imported = new Set();
    for (const match of module.matchAll(
      /import\s+extern\s*\{([\s\S]*?)\}\s*from\s*"[^"]+"\s*;/gu,
    )) {
      for (const binding of match[1].split(",")) {
        const parts = binding.trim().split(/\s+as\s+/u);
        if (parts[0]) imported.add(parts.at(-1));
      }
    }
    for (const match of module.matchAll(
      /^extern\s+((?:JsValue|bool|int|string|void)\s+([A-Za-z_$][\w$]*)\s*(?:\([^;]*\))?);$/gmu,
    )) {
      if (imported.has(match[2])) continue;
      const previous = globals.get(match[2]);
      if (previous && previous !== match[1]) {
        throw new Error(`incompatible declarations for server-renderer global ${match[2]}`);
      }
      globals.set(match[2], match[1]);
    }
  }

  const buildGlobals = resolve(graph, "build-globals.lil");
  writeFileSync(buildGlobals, `${[...globals].map(([name, declaration]) => {
    if (name === "__CJS__" || name === "__DEV__") {
      return `export JsValue ${name} = true;`;
    }
    return `export extern ${declaration};`;
  }).join("\n")}\n`);

  for (const path of files) {
    const importPath = JSON.stringify(specifier(path, buildGlobals));
    const module = readFileSync(path, "utf8").replace(
      /^extern\s+(?:JsValue|bool|int|string|void)\s+([A-Za-z_$][\w$]*)\s*(?:\([^;]*\))?;$/gmu,
      (declaration, name) => globals.has(name)
        ? `import { ${name} } from ${importPath};`
        : declaration,
    );
    writeFileSync(path, module);
  }
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
    const [local, publicName = local] = binding.trim().split(/\s+as\s+/u);
    return { local, publicName };
  });
}

function reflectExports(module) {
  return module.replace(/export\{([^}]*)\}\s*$/u, (statement, bindings) => {
    const entries = parseExports(bindings);
    const actual = entries.map(({ publicName }) => publicName).sort();
    assert.deepEqual(actual, expectedExports, "compiled server-renderer export surface");
    const definitions = entries.flatMap(({ local, publicName }) => {
      const reflection = functionReflection.get(publicName);
      if (!reflection) return [];
      const [name, length] = reflection;
      return `Object.defineProperties(${local},{name:{configurable:true,value:${JSON.stringify(name)}},length:{configurable:true,value:${length}}});`;
    });
    return `${definitions.join("")}${statement}`;
  });
}

function hostAliasDefinitions(module) {
  const definitions = [];
  for (const match of module.matchAll(
    /import\{([^}]*)\}from["'](?:\.\.\/)*(?:\.\/)?host\.js["'];?/gu,
  )) {
    for (const entry of match[1].split(",")) {
      const [imported, local = imported] = entry.trim().split(/\s+as\s+/u);
      if (local !== imported) definitions.push(`const ${local}=${imported};`);
    }
  }
  return definitions.join("");
}

function assemble(module, dependencyPrefix) {
  const hostAliases = hostAliasDefinitions(module);
  const rewritten = module
    .replace(
      /["'](?:\.\.\/){2,3}packages\/vuelil\/(compiler-ssr|runtime-dom|shared)\.js["']/gu,
      (_match, name) => JSON.stringify(`${dependencyPrefix}${name}.js`),
    )
    .replace(/import\{[^;]*\}from["'](?:\.\.\/)*(?:\.\/)?host\.js["'];?/gu, "");
  if (/(?:\.\.\/){2,3}packages\/vuelil\//u.test(rewritten)) {
    throw new Error("failed to rewrite a server-renderer dependency");
  }
  if (/from["'](?:\.\.\/)*(?:\.\/)?host\.js["']/u.test(rewritten)) {
    throw new Error("failed to inline the server-renderer host adapter");
  }
  return `${hostAliases}${reflectExports(rewritten)}`;
}

try {
  prepareGraph();
  compile();

  const compiledModule = readFileSync(compiled, "utf8");
  const hostModule = readFileSync(resolve(sourceDirectory, "host.js"), "utf8")
    .replaceAll("export function ", "function ");
  const banner = "// Generated from all 15 src/server-renderer/*.lil owners and the stream host adapter.\n";

  mkdirSync(resolve(projectRoot, "packages/vuelil"), { recursive: true });
  const packageModule = `${banner}${hostModule}\n${assemble(compiledModule, "./")}\n`;
  const candidateModule = `${banner}${hostModule}\n${assemble(compiledModule, "../packages/vuelil/")}\n`;
  writeFileSync(output, packageModule);
  writeFileSync(testOutput, candidateModule);

  const stamp = Date.now();
  const [serverRenderer, runtimeDom, runtimeCore, runtimeTest, shared] = await Promise.all([
    import(`../packages/vuelil/server-renderer.js?build=${stamp}`),
    import("../packages/vuelil/runtime-dom.js"),
    import("../packages/vuelil/runtime-core.js"),
    import("../packages/vuelil/runtime-test.js"),
    import("../packages/vuelil/shared.js"),
  ]);
  assert.deepEqual(Object.keys(serverRenderer).sort(), expectedExports);
  assert.equal(serverRenderer.ssrIncludeBooleanAttr, shared.includeBooleanAttr);
  assert.equal(serverRenderer.ssrLooseEqual, shared.looseEqual);
  for (const name of Object.keys(runtimeCore)) {
    assert.equal(runtimeDom[name], runtimeCore[name], `${name} runtime-dom singleton identity`);
    assert.equal(runtimeTest[name], runtimeCore[name], `${name} runtime-test singleton identity`);
  }
  assert.doesNotMatch(packageModule, /(?:from|import\()\s*["'](?:vue|@vue\/)/u);

  const stream = serverRenderer.renderToNodeStream(runtimeDom.h("p", "stream"));
  let rendered = "";
  for await (const chunk of stream) rendered += chunk;
  assert.equal(rendered, "<p>stream</p>", "Node stream host primitive");

  console.log(JSON.stringify({
    output,
    testOutput,
    exports: Object.keys(serverRenderer).sort(),
    bytes: readFileSync(output).byteLength,
  }));
} finally {
  rmSync(temporary, { force: true, recursive: true });
}
