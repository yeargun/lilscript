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
import { resolve } from "node:path";
import { compilerPath, projectRoot, repositoryRoot } from "../tooling/compiler-path.mjs";

const runtimeDomSource = resolve(projectRoot, "src/runtime-dom");
const compatSource = resolve(projectRoot, "src/vue-compat");
const sharedSource = resolve(projectRoot, "src/shared");
const packageDirectory = resolve(projectRoot, "packages/vuelil");
const testDirectory = resolve(projectRoot, "tests");
const temporary = mkdtempSync(resolve(tmpdir(), "vuelil-vue-compat-"));
const packageRuntimeDom = resolve(packageDirectory, "runtime-dom.compat.js");
const testRuntimeDom = resolve(testDirectory, "vue-compat-runtime-dom.candidate.mjs");
const standardOutput = resolve(packageDirectory, "vue-compat.js");
const runtimeOutput = resolve(packageDirectory, "vue-compat.runtime.js");
const testOutput = resolve(testDirectory, "vue-compat-upstream.candidate.mjs");
const runtimeDomEvidence = JSON.parse(
  readFileSync(resolve(projectRoot, "compatibility/runtime-dom.json"), "utf8"),
);
const runtimeDomExports = runtimeDomEvidence.runtimeExports;

const internalRuntimeDomExports = new Set(["mathmlNS", "patchEvent", "vtcKey", "xlinkNS"]);
const functionLengths = new Map([
  ["addEventListener", 3],
  ["initDirectivesForSSR", 0],
  ["initVModelForSSR", 0],
  ["initVShowForSSR", 0],
  ["patchAttr", 4],
  ["patchClass", 3],
  ["patchDOMProp", 4],
  ["patchEvent", 4],
  ["patchProp", 6],
  ["patchStyle", 3],
  ["removeEventListener", 3],
  ["unsafeToTrustedHTML", 1],
  ["withKeys", 2],
  ["withModifiers", 2],
]);
const objectMethods = {
  nodeOps: {
    insert: 3,
    remove: 1,
    createElement: 4,
    createText: 1,
    createComment: 1,
    setText: 2,
    setElementText: 2,
    parentNode: 1,
    nextSibling: 1,
    querySelector: 1,
    setScopeId: 2,
    insertStaticContent: 6,
  },
  vShow: { beforeMount: 3, mounted: 3, updated: 3, beforeUnmount: 2 },
  vModelText: { created: 3, mounted: 2, beforeUpdate: 3 },
  vModelCheckbox: { created: 3, mounted: ["setChecked", 3], beforeUpdate: 3 },
  vModelRadio: { created: 3, beforeUpdate: 3 },
  vModelSelect: { created: 3, mounted: 2, beforeUpdate: 3, updated: 2 },
  vModelDynamic: { created: 3, mounted: 3, beforeUpdate: 4, updated: 4 },
};

function filesIn(root, suffix) {
  const files = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const path = resolve(root, entry.name);
    if (entry.isDirectory()) files.push(...filesIn(path, suffix));
    else if (entry.name.endsWith(suffix)) files.push(path);
  }
  return files;
}

function compile(source, output) {
  const result = spawnSync(
    compilerPath(),
    [
      source,
      "--target", "js-module",
      "--mode", "development",
      "--config", resolve(repositoryRoot, "tests/config/no-optimization-no-peephole.toml"),
      "--jobs", "1",
      "--codec-jobs", "1",
      "-o", output,
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

function descriptor(target, name, length) {
  return `Object.defineProperties(${target},{name:{configurable:true,value:${JSON.stringify(name)}},length:{configurable:true,value:${length}}});`;
}

function hostAliasDefinitions(module) {
  const definitions = new Map();
  for (const match of module.matchAll(
    /import\{([^}]*)\}from["'](?:\.\.\/)*(?:\.\/)?host\.js["'];?/gu,
  )) {
    for (const entry of match[1].split(",")) {
      const [imported, local = imported] = entry.trim().split(/\s+as\s+/u);
      if (local !== imported) definitions.set(local, imported);
    }
  }
  return [...definitions]
    .map(([local, imported]) => `const ${local}=${imported};`)
    .join("");
}

function prepareRuntimeDom(module, coreSpecifier, sharedSpecifier) {
  const aliases = hostAliasDefinitions(module);
  const rewritten = module
    .replace(
      /["'](?:\.\.\/){2,3}packages\/vuelil\/runtime-core\.js["']/gu,
      JSON.stringify(coreSpecifier),
    )
    .replace(
      /["'](?:\.\.\/){2,3}packages\/vuelil\/shared\.js["']/gu,
      JSON.stringify(sharedSpecifier),
    )
    .replace(/import\{[^;]*\}from["'](?:\.\.\/)*(?:\.\/)?host\.js["'];?/gu, "")
    .replace(/export\{([^}]*)\}\s*$/u, (_statement, bindings) => {
      const entries = parseExports(bindings);
      const definitions = [];
      for (const { local, publicName } of entries) {
        if (functionLengths.has(publicName)) {
          definitions.push(descriptor(local, publicName, functionLengths.get(publicName)));
        }
        for (const [method, reflection] of Object.entries(objectMethods[publicName] ?? {})) {
          const [name, length] = Array.isArray(reflection) ? reflection : [method, reflection];
          definitions.push(descriptor(`${local}.${method}`, name, length));
        }
      }
      const kept = entries.filter(({ publicName }) => !internalRuntimeDomExports.has(publicName));
      const exports = kept.map(({ local, publicName }) =>
        local === publicName ? local : `${local} as ${publicName}`
      );
      return `${definitions.join("")}export{${exports.join(",")}};export*from${JSON.stringify(coreSpecifier)};`;
    });
  if (/(?:\.\.\/){2,3}packages\/vuelil\/(?:runtime-core|shared)\.js/u.test(rewritten)) {
    throw new Error("failed to rewrite a compat runtime-dom dependency");
  }
  if (/from["'](?:\.\.\/)*(?:\.\/)?host\.js["']/u.test(rewritten)) {
    throw new Error("failed to inline the compat runtime-dom host adapter");
  }
  return `${aliases}${rewritten}`;
}

function prepareRuntimeDomGraph(root, runtimeCore) {
  const graph = resolve(root, "src/runtime-dom");
  const dependencies = resolve(root, "packages/vuelil");
  mkdirSync(resolve(root, "src"), { recursive: true });
  mkdirSync(dependencies, { recursive: true });
  cpSync(runtimeDomSource, graph, { recursive: true });
  copyFileSync(runtimeCore, resolve(dependencies, "runtime-core.js"));
  copyFileSync(resolve(packageDirectory, "shared.js"), resolve(dependencies, "shared.js"));
  for (const path of filesIn(graph, ".lil")) {
    const source = readFileSync(path, "utf8").replace(
      /^(bool [A-Z][A-Z0-9_]*_COMPAT) = false;$/gmu,
      "$1 = true;",
    );
    writeFileSync(path, source);
  }
  return resolve(graph, "index.lil");
}

function prepareCompatGraph(root, test) {
  const graph = resolve(root, "src/vue-compat");
  const dependencies = resolve(root, "packages/vuelil");
  mkdirSync(resolve(root, "src"), { recursive: true });
  mkdirSync(dependencies, { recursive: true });
  cpSync(compatSource, graph, { recursive: true });
  cpSync(sharedSource, resolve(root, "src/shared"), { recursive: true });
  copyFileSync(
    test ? testRuntimeDom : packageRuntimeDom,
    resolve(dependencies, "runtime-dom.js"),
  );
  copyFileSync(
    test
      ? resolve(testDirectory, "runtime-core-upstream.candidate.mjs")
      : resolve(packageDirectory, "runtime-core.compat.js"),
    resolve(dependencies, "runtime-core.js"),
  );
  copyFileSync(
    test
      ? resolve(testDirectory, "compiler-dom-upstream.candidate.mjs")
      : resolve(packageDirectory, "compiler-dom.js"),
    resolve(dependencies, "compiler-dom.js"),
  );
  copyFileSync(resolve(packageDirectory, "shared.js"), resolve(dependencies, "shared.js"));
  for (const path of filesIn(graph, ".lil")) {
    let source = readFileSync(path, "utf8")
      .replace(/^bool DEV_BROWSER = true;$/mu, "bool DEV_BROWSER = false;")
      .replace(/^bool DEV_ESM_BUNDLER = false;$/mu, "bool DEV_ESM_BUNDLER = true;");
    if (test) source = source.replace(/^bool INDEX_TEST = false;$/mu, "bool INDEX_TEST = true;");
    writeFileSync(path, source);
  }
  return graph;
}

function prepareCompatModule(module, dependencies) {
  const rewritten = module
    .replace(
      /["'](?:\.\.\/){2,3}packages\/vuelil\/(runtime-dom|runtime-core|compiler-dom|shared)\.js["']/gu,
      (_specifier, name) => JSON.stringify(dependencies[name]),
    );
  if (/(?:\.\.\/){2,3}packages\/vuelil\/(?:runtime-dom|runtime-core|compiler-dom|shared)\.js/u.test(rewritten)) {
    throw new Error("failed to rewrite a vue-compat dependency");
  }
  return rewritten;
}

function assertIndependent(module, label) {
  assert.doesNotMatch(
    module,
    /(?:from|import\()\s*["'](?:vue|@vue\/|.*upstream\/vue)/u,
    `${label} imports an upstream Vue implementation`,
  );
}

try {
  const packageDomRoot = resolve(temporary, "runtime-dom-package");
  const testDomRoot = resolve(temporary, "runtime-dom-test");
  const packageDomCompiled = resolve(temporary, "runtime-dom-package.js");
  const testDomCompiled = resolve(temporary, "runtime-dom-test.js");
  compile(
    prepareRuntimeDomGraph(
      packageDomRoot,
      resolve(packageDirectory, "runtime-core.compat.js"),
    ),
    packageDomCompiled,
  );
  compile(
    prepareRuntimeDomGraph(
      testDomRoot,
      resolve(testDirectory, "runtime-core-upstream.candidate.mjs"),
    ),
    testDomCompiled,
  );

  const host = readFileSync(resolve(runtimeDomSource, "host.js"), "utf8").replaceAll(
    "export function ",
    "function ",
  );
  const runtimeBanner = "// Generated compat-enabled VueLil runtime-dom graph.\n";
  const packageDomModule = `${runtimeBanner}${host}\n${prepareRuntimeDom(
    readFileSync(packageDomCompiled, "utf8"),
    "./runtime-core.compat.js",
    "./shared.js",
  )}\n`;
  const testDomModule = `${runtimeBanner}${host}\n${prepareRuntimeDom(
    readFileSync(testDomCompiled, "utf8"),
    "./runtime-core-upstream.candidate.mjs",
    "../packages/vuelil/shared.js",
  )}\n`;
  assertIndependent(packageDomModule, "compat runtime-dom package");
  assertIndependent(testDomModule, "compat runtime-dom test candidate");
  writeFileSync(packageRuntimeDom, packageDomModule);
  writeFileSync(testRuntimeDom, testDomModule);

  const packageGraph = prepareCompatGraph(resolve(temporary, "vue-compat-package"), false);
  const testGraph = prepareCompatGraph(resolve(temporary, "vue-compat-test"), true);
  const standardCompiled = resolve(temporary, "vue-compat.js");
  const runtimeCompiled = resolve(temporary, "vue-compat.runtime.js");
  const testCompiled = resolve(temporary, "vue-compat.test.js");
  compile(resolve(packageGraph, "esm-index.lil"), standardCompiled);
  compile(resolve(packageGraph, "esm-runtime.lil"), runtimeCompiled);
  compile(resolve(testGraph, "esm-index.lil"), testCompiled);

  const packageDependencies = {
    "runtime-dom": "./runtime-dom.compat.js",
    "runtime-core": "./runtime-core.compat.js",
    "compiler-dom": "./compiler-dom.js",
    shared: "./shared.js",
  };
  const testDependencies = {
    "runtime-dom": "./vue-compat-runtime-dom.candidate.mjs",
    "runtime-core": "./runtime-core-upstream.candidate.mjs",
    "compiler-dom": "./compiler-dom-upstream.candidate.mjs",
    shared: "../packages/vuelil/shared.js",
  };
  const banner = "// Generated from all six src/vue-compat owners.\n";
  const standardModule = `${banner}${prepareCompatModule(
    readFileSync(standardCompiled, "utf8"),
    packageDependencies,
  )}\n`;
  const runtimeModule = `${banner}${prepareCompatModule(
    readFileSync(runtimeCompiled, "utf8"),
    packageDependencies,
  )}\n`;
  const testModule = `${banner}${prepareCompatModule(
    readFileSync(testCompiled, "utf8"),
    testDependencies,
  )}\n`;
  assertIndependent(standardModule, "standard vue-compat package");
  assertIndependent(runtimeModule, "runtime-only vue-compat package");
  assertIndependent(testModule, "vue-compat test candidate");
  writeFileSync(standardOutput, standardModule);
  writeFileSync(runtimeOutput, runtimeModule);
  writeFileSync(testOutput, testModule);

  const stamp = Date.now();
  const [standard, runtimeOnly, runtimeDom] = await Promise.all([
    import(`${standardOutput}?build=${stamp}`),
    import(`${runtimeOutput}?build=${stamp}`),
    import(packageRuntimeDom),
  ]);
  const expectedExports = [...new Set([...runtimeDomExports, "configureCompat", "default"])].sort();
  assert.equal(runtimeDomExports.length, 170, "pinned runtime-dom export count");
  assert.deepEqual(Object.keys(runtimeDom).sort(), runtimeDomExports);
  assert.deepEqual(Object.keys(standard).sort(), expectedExports);
  assert.deepEqual(Object.keys(runtimeOnly).sort(), expectedExports);
  for (const name of runtimeDomExports) {
    assert.equal(standard[name], runtimeDom[name], `${name} standard singleton identity`);
    assert.equal(runtimeOnly[name], runtimeDom[name], `${name} runtime-only singleton identity`);
  }
  assert.equal(standard.configureCompat, standard.default.configureCompat);
  assert.equal(runtimeOnly.configureCompat, runtimeOnly.default.configureCompat);
  assert.notEqual(standard.default.compile, runtimeOnly.default.compile);

  console.log(JSON.stringify({
    standardOutput,
    runtimeOutput,
    packageRuntimeDom,
    testOutput,
    exports: expectedExports.length,
  }));
} finally {
  rmSync(temporary, { force: true, recursive: true });
}
