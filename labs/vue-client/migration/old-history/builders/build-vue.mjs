import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { build } from "vite";
import { compilerPath, projectRoot, repositoryRoot } from "../tooling/compiler-path.mjs";

const sourceDirectory = resolve(projectRoot, "src/vue");
const packageDirectory = resolve(projectRoot, "packages/vuelil");
const standardOutput = resolve(packageDirectory, "vue.js");
const runtimeOutput = resolve(packageDirectory, "vue.runtime.js");
const productionDirectory = resolve(packageDirectory, "production");
const productionStandardOutput = resolve(productionDirectory, "vue.js");
const productionRuntimeOutput = resolve(productionDirectory, "vue.runtime.js");
const globalTestOutput = resolve(packageDirectory, "vue.global.test.js");
const testOutput = resolve(projectRoot, "tests/vue-upstream.candidate.mjs");
const temporary = mkdtempSync(resolve(tmpdir(), "vuelil-vue-"));

function compile(input, output, production = false) {
  const args = [input, "--target", "js-module"];
  if (production) {
    args.push(
      "--mode", "production",
      "--config", resolve(projectRoot, "config/open-world.toml"),
      "--jobs", "1",
      "--codec-jobs", "1",
    );
  } else {
    args.push(
      "--mode", "development",
      "--config", resolve(repositoryRoot, "tests/config/no-optimization-no-peephole.toml"),
      "--jobs", "1",
      "--codec-jobs", "1",
    );
  }
  args.push("-o", output);
  const result = spawnSync(
    compilerPath(),
    args,
    { cwd: projectRoot, encoding: "utf8", env: process.env },
  );
  if (result.status !== 0) throw new Error(`${result.stdout ?? ""}${result.stderr ?? ""}`);
}

function prepareGraph(name, { browser, development = true, esmBundler, global }) {
  const root = resolve(temporary, name);
  const graph = resolve(root, "src/vue");
  const dependencies = resolve(root, "packages/vuelil");
  mkdirSync(resolve(root, "src"), { recursive: true });
  mkdirSync(dependencies, { recursive: true });
  cpSync(sourceDirectory, graph, { recursive: true });
  for (const dependency of ["compiler-dom.js", "runtime-dom.js", "shared.js"]) {
    const sourceRoot = development ? packageDirectory : productionDirectory;
    copyFileSync(resolve(sourceRoot, dependency), resolve(dependencies, dependency));
  }

  for (const name of ["index.lil", "runtime.lil"]) {
    const path = resolve(graph, name);
    const source = readFileSync(path, "utf8")
      .replace("extern bool __DEV__;", `bool __DEV__ = ${development};`)
      .replace("extern bool __GLOBAL__;", `bool __GLOBAL__ = ${global};`)
      .replace("extern bool __ESM_BUNDLER__;", `bool __ESM_BUNDLER__ = ${esmBundler};`)
      .replace("extern bool __ESM_BROWSER__;", "bool __ESM_BROWSER__ = false;");
    writeFileSync(path, source);
  }

  const devPath = resolve(graph, "dev.lil");
  writeFileSync(
    devPath,
    readFileSync(devPath, "utf8")
      .replace("bool VUE_DEV_BROWSER = true;", `bool VUE_DEV_BROWSER = ${browser};`)
      .replace("bool VUE_DEV_ESM_BUNDLER = true;", `bool VUE_DEV_ESM_BUNDLER = ${esmBundler};`),
  );
  return graph;
}

function rewriteDependencies(module, dependencies) {
  const rewritten = module.replace(
    /["'](?:\.\.\/){2,3}packages\/vuelil\/(compiler-dom|runtime-dom|shared)\.js["']/gu,
    (_specifier, name) => JSON.stringify(dependencies[name]),
  );
  if (/(?:\.\.\/){2,3}packages\/vuelil\//u.test(rewritten)) {
    throw new Error("failed to rewrite a vue package dependency");
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

function asciiJavaScript(module) {
  return module.replace(/[^\x00-\x7f]/gu, character => {
    const point = character.codePointAt(0);
    if (point <= 0xffff) return `\\u${point.toString(16).padStart(4, "0")}`;
    const offset = point - 0x10000;
    const high = 0xd800 + (offset >> 10);
    const low = 0xdc00 + (offset & 0x3ff);
    return `\\u${high.toString(16)}\\u${low.toString(16)}`;
  });
}

async function bundleGlobal(entry) {
  const result = await build({
    configFile: false,
    logLevel: "silent",
    build: {
      write: false,
      target: "es2022",
      minify: "oxc",
      sourcemap: false,
      lib: {
        entry,
        name: "Vue",
        formats: ["iife"],
        fileName: () => "vue.global.test.js",
      },
    },
  });
  const outputs = (Array.isArray(result) ? result : [result]).flatMap(entry => entry.output);
  const chunks = outputs.filter(entry => entry.type === "chunk");
  assert.equal(chunks.length, 1, "Vue test global must contain one JavaScript chunk");
  const upstreamModules = Object.keys(chunks[0].modules).filter(id =>
    id.includes("node_modules/vue/") ||
    id.includes("node_modules/@vue/") ||
    id.includes("upstream/vue/")
  );
  assert.deepEqual(upstreamModules, [], "Vue test global contains upstream implementation code");
  const module = asciiJavaScript(chunks[0].code);
  assert.doesNotMatch(module, /[^\x00-\x7f]/u, "Vue test global is not ASCII-safe");
  writeFileSync(globalTestOutput, module);
}

try {
  const graph = prepareGraph("module", {
    browser: false,
    esmBundler: true,
    global: false,
  });
  const globalGraph = prepareGraph("global", {
    browser: true,
    esmBundler: false,
    global: true,
  });
  const productionGraph = prepareGraph("production", {
    browser: true,
    development: false,
    esmBundler: true,
    global: false,
  });
  const standardCompiled = resolve(temporary, "vue.js");
  const runtimeCompiled = resolve(temporary, "vue.runtime.js");
  const globalCompiled = resolve(temporary, "vue.global.js");
  const productionStandardCompiled = resolve(temporary, "vue.production.js");
  const productionRuntimeCompiled = resolve(temporary, "vue.runtime.production.js");
  compile(resolve(graph, "index.lil"), standardCompiled);
  compile(resolve(graph, "runtime.lil"), runtimeCompiled);
  compile(resolve(globalGraph, "index.lil"), globalCompiled);
  compile(resolve(productionGraph, "index.lil"), productionStandardCompiled, true);
  compile(resolve(productionGraph, "runtime.lil"), productionRuntimeCompiled, true);

  const packageDependencies = {
    "compiler-dom": "./compiler-dom.js",
    "runtime-dom": "./runtime-dom.js",
    shared: "./shared.js",
  };
  const testDependencies = {
    "compiler-dom": "./compiler-dom-upstream.candidate.mjs",
    "runtime-dom": "./runtime-dom-upstream.candidate.mjs",
    shared: "../packages/vuelil/shared.js",
  };
  const productionDependencies = {
    "compiler-dom": "./compiler-dom.js",
    "runtime-dom": "./runtime-dom.js",
    shared: "./shared.js",
  };
  const banner = "// Generated from all three src/vue owners.\n";
  const standardModule = `${banner}${rewriteDependencies(
    readFileSync(standardCompiled, "utf8"),
    packageDependencies,
  )}\n`;
  const runtimeModule = `${banner}${rewriteDependencies(
    readFileSync(runtimeCompiled, "utf8"),
    packageDependencies,
  )}\n`;
  const testModule = `${banner}${rewriteDependencies(
    readFileSync(standardCompiled, "utf8"),
    testDependencies,
  )}\n`;
  const productionStandardModule = `${banner}${rewriteDependencies(
    readFileSync(productionStandardCompiled, "utf8"),
    productionDependencies,
  )}\n`;
  const productionRuntimeModule = `${banner}${rewriteDependencies(
    readFileSync(productionRuntimeCompiled, "utf8"),
    productionDependencies,
  )}\n`;
  assertIndependent(standardModule, "standard Vue package");
  assertIndependent(runtimeModule, "runtime-only Vue package");
  assertIndependent(testModule, "Vue test candidate");
  assertIndependent(productionStandardModule, "production standard Vue package");
  assertIndependent(productionRuntimeModule, "production runtime-only Vue package");

  mkdirSync(packageDirectory, { recursive: true });
  writeFileSync(standardOutput, standardModule);
  writeFileSync(runtimeOutput, runtimeModule);
  writeFileSync(testOutput, testModule);
  mkdirSync(productionDirectory, { recursive: true });
  writeFileSync(productionStandardOutput, productionStandardModule);
  writeFileSync(productionRuntimeOutput, productionRuntimeModule);
  const globalEntry = resolve(temporary, "vue.global.entry.js");
  writeFileSync(globalEntry, rewriteDependencies(readFileSync(globalCompiled, "utf8"), {
    "compiler-dom": resolve(packageDirectory, "compiler-dom.js"),
    "runtime-dom": resolve(projectRoot, "tests/vue-runtime-dom-browser.candidate.mjs"),
    shared: resolve(packageDirectory, "shared.js"),
  }));
  await bundleGlobal(globalEntry);

  const stamp = Date.now();
  const [standard, runtimeOnly, runtimeDom] = await Promise.all([
    import(`${standardOutput}?build=${stamp}`),
    import(`${runtimeOutput}?build=${stamp}`),
    import("../packages/vuelil/runtime-dom.js"),
  ]);
  const expectedExports = [...new Set([...Object.keys(runtimeDom), "compile"])].sort();
  assert.equal(Object.keys(runtimeDom).length, 170, "pinned runtime-dom export count");
  assert.deepEqual(Object.keys(standard).sort(), expectedExports);
  assert.deepEqual(Object.keys(runtimeOnly).sort(), expectedExports);
  for (const name of Object.keys(runtimeDom)) {
    assert.equal(standard[name], runtimeDom[name], `${name} standard singleton identity`);
    assert.equal(runtimeOnly[name], runtimeDom[name], `${name} runtime-only singleton identity`);
  }
  assert.equal(standard.compile.name, "compileToFunction");
  assert.equal(standard.compile.length, 1);
  assert.equal(runtimeOnly.compile.name, "compile");
  assert.equal(runtimeOnly.compile.length, 0);
  const render = standard.compile("<div>{{ value }}</div>");
  assert.equal(typeof render, "function");
  assert.equal(render._rc, true);
  const warnings = [];
  const consoleWarn = console.warn;
  console.warn = (...args) => warnings.push(args.join(" "));
  try {
    assert.equal(runtimeOnly.compile("<div/>"), undefined);
  } finally {
    console.warn = consoleWarn;
  }
  assert.ok(warnings.some(message => message.includes(
    'Runtime compilation is not supported in this build of Vue.',
  )));

  console.log(JSON.stringify({
    standardOutput,
    runtimeOutput,
    productionStandardOutput,
    productionRuntimeOutput,
    globalTestOutput,
    testOutput,
    exports: expectedExports.length,
  }));
} finally {
  rmSync(temporary, { force: true, recursive: true });
}
