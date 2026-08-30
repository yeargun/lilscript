import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import vm from "node:vm";
import ts from "typescript";
import { build } from "vite";
import { projectRoot } from "../tooling/compiler-path.mjs";

const inventoryPath = resolve(projectRoot, "compatibility/inventory.json");
const scopePath = resolve(projectRoot, "compatibility/scope.json");
const formatManifestPath = resolve(projectRoot, "compatibility/package-formats.json");
const reportPath = resolve(projectRoot, "artifacts/compatibility-report.json");
const upstreamRoot = resolve(projectRoot, "upstream/vue");
const temporary = mkdtempSync(resolve(tmpdir(), "vuelil-compatibility-"));
const inventory = readJson(inventoryPath);
const scope = readJson(scopePath);
const formatManifest = readJson(formatManifestPath);
const packageRows = new Map(formatManifest.packages.map(entry => [entry.name, entry]));
const runtimeModules = new Map();
const declarationRows = [];
const declarationSurfaces = new Map();
const artifactRows = [];
const nativeRequire = createRequire(import.meta.url);

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function write(path, contents) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, contents);
}

function relativePath(path) {
  return relative(projectRoot, path).replaceAll("\\", "/");
}

function artifact(path, extra = {}) {
  const bytes = readFileSync(path);
  return {
    path: relativePath(path),
    bytes: bytes.byteLength,
    sha256: sha256(bytes),
    ...extra,
  };
}

function compareText(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

function replaceWorkspaceVersions(value) {
  if (Array.isArray(value)) return value.map(replaceWorkspaceVersions);
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).map(([key, entry]) => [key, replaceWorkspaceVersions(entry)]),
    );
  }
  return typeof value === "string" && value.startsWith("workspace:")
    ? formatManifest.upstreamVersion
    : value;
}

function packageManifest(entry, audited) {
  const upstream = replaceWorkspaceVersions(
    readJson(resolve(upstreamRoot, audited.directory, "package.json")),
  );
  upstream.version = entry.name === "@vue/runtime-test"
    ? upstream.version
    : `${formatManifest.upstreamVersion}-vuelil`;
  upstream.private = true;
  if (entry.name === "vue") {
    upstream.type = "module";
    upstream.main = "./index.cjs";
    upstream.exports["."].require.node.production = "./dist/vue.cjs.prod.js";
    upstream.exports["."].require.node.development = "./dist/vue.cjs.js";
    upstream.exports["."].require.node.default = "./index.cjs";
    upstream.exports["."].require.default = "./index.cjs";
  }
  return upstream;
}

function declarationTarget(entry) {
  return resolve(entry.root, "dist", `${entry.filename}.d.ts`);
}

function copyDeclaration(source, target, packageName) {
  const sourceBytes = readFileSync(source);
  write(target, sourceBytes);
  const outputBytes = readFileSync(target);
  assert.equal(sha256(outputBytes), sha256(sourceBytes), `${packageName} declaration changed`);
  declarationRows.push({
    package: packageName,
    path: relativePath(target),
    source: relativePath(source),
    sourceSha256: sha256(sourceBytes),
    sha256: sha256(outputBytes),
    bytes: outputBytes.byteLength,
    exact: true,
  });
}

function runtimeTestDeclaration() {
  return `import type { CreateAppFunction, RootRenderFunction, VNode } from "@vue/runtime-core";
export * from "@vue/runtime-core";
export declare enum TestNodeTypes { TEXT = "text", ELEMENT = "element", COMMENT = "comment" }
export declare enum NodeOpTypes { CREATE = "create", INSERT = "insert", REMOVE = "remove", SET_TEXT = "setText", SET_ELEMENT_TEXT = "setElementText", PATCH = "patch" }
export interface TestElement { id: number; type: TestNodeTypes.ELEMENT; parentNode: TestElement | null; tag: string; children: TestNode[]; props: Record<string, any>; eventListeners: Record<string, Function | Function[]> | null; }
export interface TestText { id: number; type: TestNodeTypes.TEXT; parentNode: TestElement | null; text: string; }
export interface TestComment { id: number; type: TestNodeTypes.COMMENT; parentNode: TestElement | null; text: string; }
export type TestNode = TestElement | TestText | TestComment;
export interface NodeOp { type: NodeOpTypes; nodeType?: TestNodeTypes; tag?: string; text?: string; targetNode?: TestNode; parentNode?: TestElement; refNode?: TestNode | null; propKey?: string; propPrevValue?: any; propNextValue?: any; }
export declare function logNodeOp(op: NodeOp): void;
export declare function resetOps(): void;
export declare function dumpOps(): NodeOp[];
export declare const nodeOps: Record<string, Function>;
export declare function serialize(node: TestNode, indent?: number, depth?: number): string;
export declare function serializeInner(node: TestElement, indent?: number, depth?: number): string;
export declare function triggerEvent(el: TestElement, event: string, payload?: any[]): void;
export declare const render: RootRenderFunction<TestElement>;
export declare const createApp: CreateAppFunction<TestElement>;
export declare function renderToString(vnode: VNode): string;
`;
}

function compatDeclaration() {
  return `import type { CompatVue } from "@vue/runtime-core";
declare const Vue: CompatVue;
export default Vue;
`;
}

function copyDeclarations() {
  for (const audited of inventory.packages) {
    const entry = packageRows.get(audited.name);
    assert(entry, `missing package format manifest for ${audited.name}`);
    entry.root = resolve(projectRoot, entry.root);
    entry.source = resolve(projectRoot, entry.source);
    if (entry.runtimeSource) entry.runtimeSource = resolve(projectRoot, entry.runtimeSource);
    if (entry.declarationSource) {
      entry.declarationSource = resolve(projectRoot, entry.declarationSource);
      const sourceManifest = readJson(resolve(dirname(dirname(entry.declarationSource)), "package.json"));
      assert.equal(sourceManifest.version, formatManifest.upstreamVersion);
      copyDeclaration(entry.declarationSource, declarationTarget(entry), entry.name);
    } else {
      const contents = entry.name === "@vue/compat"
        ? compatDeclaration()
        : runtimeTestDeclaration();
      const target = declarationTarget(entry);
      write(target, contents);
      declarationRows.push({
        package: entry.name,
        path: relativePath(target),
        source: "generated from pinned public source declarations",
        sourceSha256: sha256(Buffer.from(contents)),
        sha256: sha256(readFileSync(target)),
        bytes: statSync(target).size,
        exact: true,
      });
    }
  }

  const vueRoot = packageRows.get("vue").root;
  for (const [source, target] of [
    ["node_modules/vue/dist/vue.d.mts", "dist/vue.d.mts"],
    ["node_modules/vue/jsx.d.ts", "jsx.d.ts"],
    ["node_modules/vue/jsx-runtime/index.d.ts", "jsx-runtime/index.d.ts"],
    ["node_modules/vue/compiler-sfc/index.d.ts", "compiler-sfc/index.d.ts"],
    ["node_modules/vue/compiler-sfc/index.d.mts", "compiler-sfc/index.d.mts"],
    ["node_modules/vue/server-renderer/index.d.ts", "server-renderer/index.d.ts"],
    ["node_modules/vue/server-renderer/index.d.mts", "server-renderer/index.d.mts"],
  ]) {
    copyDeclaration(resolve(projectRoot, source), resolve(vueRoot, target), "vue");
  }
}

function namedExportSource(source, names, expectedModule, outputPath) {
  const path = outputPath
    ? relative(dirname(outputPath), source).replaceAll("\\", "/")
    : null;
  const specifier = path === null
    ? pathToFileURL(source).href
    : path.startsWith(".")
      ? path
      : `./${path}`;
  const reflected = names.flatMap(name => {
    const value = expectedModule[name];
    return typeof value === "function"
      ? [`Object.defineProperties(__source[${JSON.stringify(name)}], { name: { configurable: true, value: ${JSON.stringify(value.name)} }, length: { configurable: true, value: ${value.length} } });`]
      : [];
  });
  return `import * as __source from ${JSON.stringify(specifier)};\n${reflected.join("\n")}\nexport { ${names.join(", ")} } from ${JSON.stringify(specifier)};\n`;
}

async function loadRuntimeModules() {
  for (const audited of inventory.packages) {
    const entry = packageRows.get(audited.name);
    const full = await import(`${pathToFileURL(entry.source).href}?compat=${Date.now()}`);
    const excluded = new Set(entry.excludedRuntimeExports ?? []);
    const exports = Object.keys(full).filter(name => !excluded.has(name)).sort(compareText);
    const runtimeSource = entry.runtimeSource
      ? await import(`${pathToFileURL(entry.runtimeSource).href}?compat=${Date.now()}`)
      : full;
    const runtimeExports = Object.keys(runtimeSource)
      .filter(name => !excluded.has(name))
      .sort(compareText);
    let expectedFull = full;
    let expectedRuntime = runtimeSource;
    if (entry.name !== "@vue/compat" && entry.name !== "@vue/runtime-test") {
      const installedRoot = entry.name === "vue"
        ? resolve(projectRoot, "node_modules/vue")
        : resolve(projectRoot, "node_modules", ...entry.name.split("/"));
      expectedFull = nativeRequire(installedRoot);
      if (entry.name === "vue") {
        const runtimeDom = nativeRequire(resolve(projectRoot, "node_modules/@vue/runtime-dom"));
        expectedRuntime = { ...runtimeDom, compile: runtimeSource.compile };
      } else {
        expectedRuntime = expectedFull;
      }
      assert.deepEqual(Object.keys(expectedFull).sort(compareText), exports, `${entry.name} oracle exports`);
      assert.deepEqual(Object.keys(expectedRuntime).sort(compareText), runtimeExports, `${entry.name} runtime oracle exports`);
    }
    runtimeModules.set(entry.name, {
      entry,
      full,
      exports,
      runtimeSource,
      runtimeExports,
      expectedFull,
      expectedRuntime,
    });
  }
}

async function bundleCode(entry, format, globalName, production) {
  const browserBuild = format === "es" || format === "iife";
  const result = await build({
    configFile: false,
    logLevel: "silent",
    plugins: [{
      name: "vuelil-format-host",
      transform(code, id) {
        if (resolve(id.split("?", 1)[0]) !== resolve(projectRoot, "artifacts/compiler-sfc.generated.js")) {
          return null;
        }
        if (browserBuild) {
            const transformed = code.replace(
              /import \{ createRequire \} from "node:module";\s*const require = createRequire\(import\.meta\.url\);/u,
              "const require = () => Object.create(null);",
            );
            assert.notEqual(transformed, code, "compiler-sfc browser host was not replaced");
            return transformed;
        }
        return code.replace("createRequire(import.meta.url)", "createRequire(__filename)");
      },
    }],
    define: {
      "process.env.NODE_ENV": JSON.stringify(production ? "production" : "development"),
    },
    build: {
      write: false,
      target: "es2022",
      minify: false,
      sourcemap: false,
      lib: {
        entry,
        name: globalName,
        formats: [format],
        fileName: () => `package.${format === "iife" ? "js" : format}`,
      },
      rollupOptions: {
        external: id => id.startsWith("node:"),
        output: { exports: "named" },
      },
    },
  });
  const outputs = (Array.isArray(result) ? result : [result]).flatMap(item => item.output);
  const chunks = outputs.filter(item => item.type === "chunk");
  assert.equal(chunks.length, 1, `${entry} ${format} emitted multiple chunks`);
  assert.deepEqual(
    Object.keys(chunks[0].modules).filter(path =>
      path.includes("node_modules/vue/") ||
      path.includes("node_modules/@vue/") ||
      path.includes("upstream/vue/")
    ),
    [],
    `${entry} ${format} bundled an upstream implementation`,
  );
  return chunks[0].code;
}

function semanticDescriptor(value) {
  const type = typeof value;
  if (type === "function") return { type, name: value.name, length: value.length };
  if (type === "object") {
    return {
      type: value === null ? "null" : type,
      tag: value === null ? null : Object.prototype.toString.call(value),
    };
  }
  if (type === "symbol") return { type, description: value.description ?? null };
  return { type };
}

function descriptors(module, names) {
  return Object.fromEntries(names.map(name => {
    const property = Object.getOwnPropertyDescriptor(module, name);
    assert(property, `missing descriptor for ${name}`);
    return [name, {
      configurable: property.configurable,
      enumerable: property.enumerable,
      writable: "writable" in property ? property.writable : null,
      getter: typeof property.get === "function",
      setter: typeof property.set === "function",
      value: semanticDescriptor(module[name]),
    }];
  }));
}

function assertExportSemantics(actual, expectedModule, expectedNames, label) {
  assert.deepEqual(Object.keys(actual).sort(compareText), expectedNames, `${label} exports`);
  for (const name of expectedNames) {
    assert.deepEqual(
      semanticDescriptor(actual[name]),
      semanticDescriptor(expectedModule[name]),
      `${label} ${name} descriptor`,
    );
  }
}

function assertFormatSmoke(module, packageName, label) {
  switch (packageName) {
    case "@vue/shared":
      assert.equal(module.camelize("package-format"), "packageFormat", label);
      break;
    case "@vue/reactivity":
      assert.equal(module.ref(42).value, 42, label);
      break;
    case "@vue/runtime-core":
    case "@vue/runtime-dom":
    case "@vue/compat":
    case "vue":
      assert.equal(module.h("section").type, "section", label);
      break;
    case "@vue/compiler-core":
      assert.equal(module.baseParse("<main/>").children[0].tag, "main", label);
      break;
    case "@vue/compiler-dom":
      assert.match(module.compile("<main/>").code, /main/u, label);
      break;
    case "@vue/compiler-ssr":
      assert.match(module.compile("<main/>").code, /main/u, label);
      break;
    case "@vue/compiler-sfc": {
      const parsed = module.parse("<template><main/></template>");
      assert.equal(parsed.errors.length, 0, label);
      assert.equal(parsed.descriptor.template?.type, "template", label);
      break;
    }
    case "@vue/server-renderer":
      assert.equal(module.ssrInterpolate("<"), "&lt;", label);
      break;
  }
}

async function evaluateEsm(code) {
  return import(`data:text/javascript;base64,${Buffer.from(code).toString("base64")}`);
}

const cjsEvaluationCache = new Map();

function evaluateCjs(path) {
  if (cjsEvaluationCache.has(path)) return cjsEvaluationCache.get(path).exports;
  const source = readFileSync(path, "utf8");
  const module = { exports: {} };
  cjsEvaluationCache.set(path, module);
  const localRequire = specifier => {
    if (specifier.startsWith(".")) {
      const target = resolve(dirname(path), specifier);
      return evaluateCjs(target);
    }
    if (specifier.startsWith("node:")) return nativeRequire(specifier);
    throw new Error(`unexpected external CJS dependency ${specifier} in ${path}`);
  };
  const context = {
    AbortController,
    ArrayBuffer,
    Buffer,
    TextDecoder,
    TextEncoder,
    URL,
    URLSearchParams,
    atob,
    btoa,
    clearInterval,
    clearTimeout,
    console,
    process,
    queueMicrotask,
    setInterval,
    setTimeout,
    structuredClone,
  };
  vm.runInNewContext(`(function(module,exports,require,__filename,__dirname){${source}\n})`, context, {
    filename: path,
  })(module, module.exports, localRequire, path, dirname(path));
  return module.exports;
}

function evaluateGlobal(code, name, production) {
  const context = {
    AbortController,
    Array,
    ArrayBuffer,
    BigInt,
    Boolean,
    Buffer,
    console,
    Date,
    Error,
    Function,
    JSON,
    Map,
    Math,
    Number,
    Object,
    Promise,
    Proxy,
    Reflect,
    RegExp,
    Set,
    String,
    Symbol,
    TextDecoder,
    TextEncoder,
    URL,
    URLSearchParams,
    WeakMap,
    WeakSet,
    atob,
    btoa,
    clearInterval,
    clearTimeout,
    process: { env: { NODE_ENV: production ? "production" : "development" } },
    queueMicrotask,
    setInterval,
    setTimeout,
  };
  context.global = context;
  context.self = context;
  context.window = context;
  vm.runInNewContext(code, context, { filename: `${name}.global.js` });
  return context[name];
}

function cjsKey(name, runtime = false) {
  const base = name === "vue"
    ? "vue"
    : name === "@vue/compat"
      ? "compat"
      : name.slice("@vue/".length).replaceAll("-", "_");
  return runtime ? `${base}_runtime` : base;
}

async function buildCjsBundle(production) {
  const lines = [];
  for (const [name, row] of runtimeModules) {
    const fullEntry = resolve(temporary, `${cjsKey(name)}.mjs`);
    write(fullEntry, namedExportSource(row.entry.source, row.exports, row.expectedFull));
    lines.push(`import * as ${cjsKey(name)} from ${JSON.stringify(pathToFileURL(fullEntry).href)};`);
    lines.push(`export { ${cjsKey(name)} };`);
    if (row.entry.runtimeSource) {
      const runtimeEntry = resolve(temporary, `${cjsKey(name, true)}.mjs`);
      write(runtimeEntry, namedExportSource(
        row.entry.runtimeSource,
        row.runtimeExports,
        row.expectedRuntime,
      ));
      lines.push(`import * as ${cjsKey(name, true)} from ${JSON.stringify(pathToFileURL(runtimeEntry).href)};`);
      lines.push(`export { ${cjsKey(name, true)} };`);
    }
  }
  const entry = resolve(temporary, `cjs-${production ? "production" : "development"}.mjs`);
  write(entry, `${lines.join("\n")}\n`);
  const code = await bundleCode(entry, "cjs", undefined, production);
  const target = resolve(
    packageRows.get("vue").root,
    "dist",
    `vuelil-packages${production ? ".prod" : ""}.cjs`,
  );
  write(target, code);
  artifactRows.push(artifact(target, { role: "shared-cjs-graph" }));
  return target;
}

function cjsWrapper(target, bundle, key, names) {
  return `"use strict";\nmodule.exports = require(${JSON.stringify(
    `./${relative(dirname(target), bundle).replaceAll("\\", "/")}`.replace("./../", "../"),
  )})[${JSON.stringify(key)}];\nif (false) {\n${names.map(name =>
    `  exports.${name} = module.exports.${name};`
  ).join("\n")}\n}\n`;
}

function formatTargets(entry, format) {
  const root = entry.root;
  const base = entry.filename;
  switch (format) {
    case "esm-bundler":
      return [{ path: resolve(root, "dist", `${base}.esm-bundler.js`), runtime: false }];
    case "esm-bundler-runtime":
      return [{ path: resolve(root, "dist", `${base}.runtime.esm-bundler.js`), runtime: true }];
    case "esm-browser":
      return [
        { path: resolve(root, "dist", `${base}.esm-browser.js`), runtime: false },
        ...(entry.production
          ? [{ path: resolve(root, "dist", `${base}.esm-browser.prod.js`), runtime: false, production: true }]
          : []),
      ];
    case "esm-browser-runtime":
      return [
        { path: resolve(root, "dist", `${base}.runtime.esm-browser.js`), runtime: true },
        ...(entry.production
          ? [{ path: resolve(root, "dist", `${base}.runtime.esm-browser.prod.js`), runtime: true, production: true }]
          : []),
      ];
    case "global":
      return [
        { path: resolve(root, "dist", `${base}.global.js`), runtime: false },
        ...(entry.production
          ? [{ path: resolve(root, "dist", `${base}.global.prod.js`), runtime: false, production: true }]
          : []),
      ];
    case "global-runtime":
      return [
        { path: resolve(root, "dist", `${base}.runtime.global.js`), runtime: true },
        ...(entry.production
          ? [{ path: resolve(root, "dist", `${base}.runtime.global.prod.js`), runtime: true, production: true }]
          : []),
      ];
    case "cjs":
      return [
        { path: resolve(root, "dist", `${base}.cjs.js`), runtime: false },
        ...(entry.production
          ? [{ path: resolve(root, "dist", `${base}.cjs.prod.js`), runtime: false, production: true }]
          : []),
      ];
    default:
      throw new Error(`unknown format ${format}`);
  }
}

function assertNoUpstreamImport(code, path) {
  assert.doesNotMatch(code, /(?:from|import\(|require\()\s*["'][^"']*(?:node_modules\/(?:vue|@vue)|upstream\/vue)[^"']*["']/u, path);
  assert.equal(code.includes(projectRoot), false, `${path} contains an absolute workspace path`);
}

async function buildFormats(cjsDevelopment, cjsProduction) {
  const formatEvidence = [];
  for (const audited of inventory.packages) {
    const row = runtimeModules.get(audited.name);
    const entry = row.entry;
    for (const format of audited.formats) {
      const outputs = [];
      for (const target of formatTargets(entry, format)) {
        const expectedModule = target.runtime ? row.expectedRuntime : row.expectedFull;
        const expectedNames = target.runtime ? row.runtimeExports : row.exports;
        let code;
        let actual;
        if (format === "esm-bundler" || format === "esm-bundler-runtime") {
          const source = target.runtime ? entry.runtimeSource : entry.source;
          code = `// Candidate-only VueLil ${format} entry.\n${namedExportSource(
            source,
            expectedNames,
            expectedModule,
            target.path,
          )}`;
          write(target.path, code);
          actual = await evaluateEsm(await bundleCode(target.path, "es", undefined, target.production));
        } else if (format === "esm-browser" || format === "esm-browser-runtime") {
          const source = target.runtime ? entry.runtimeSource : entry.source;
          const bundleEntry = resolve(
            temporary,
            `${cjsKey(entry.name, target.runtime)}-${target.production ? "prod" : "dev"}.mjs`,
          );
          write(bundleEntry, namedExportSource(source, expectedNames, expectedModule));
          code = await bundleCode(bundleEntry, "es", undefined, target.production);
          write(target.path, code);
          actual = await evaluateEsm(code);
        } else if (format === "global" || format === "global-runtime") {
          const source = target.runtime ? entry.runtimeSource : entry.source;
          const bundleEntry = resolve(
            temporary,
            `${cjsKey(entry.name, target.runtime)}-global-${target.production ? "prod" : "dev"}.mjs`,
          );
          write(bundleEntry, namedExportSource(source, expectedNames, expectedModule));
          code = await bundleCode(bundleEntry, "iife", entry.globalName, target.production);
          write(target.path, code);
          actual = evaluateGlobal(code, entry.globalName, target.production);
        } else {
          const bundle = target.production ? cjsProduction : cjsDevelopment;
          code = cjsWrapper(
            target.path,
            bundle,
            cjsKey(entry.name, target.runtime),
            expectedNames,
          );
          write(target.path, code);
          actual = evaluateCjs(target.path);
        }
        assertNoUpstreamImport(code, relativePath(target.path));
        assertExportSemantics(actual, expectedModule, expectedNames, `${entry.name} ${format}`);
        assertFormatSmoke(actual, entry.name, `${entry.name} ${format} smoke`);
        const descriptorMap = descriptors(actual, expectedNames);
        outputs.push(artifact(target.path, {
          production: target.production === true,
          runtimeOnly: target.runtime === true,
          exports: expectedNames.length,
          descriptorSha256: sha256(Buffer.from(JSON.stringify(descriptorMap))),
          descriptors: descriptorMap,
        }));
      }
      formatEvidence.push({
        package: entry.name,
        format,
        status: "passed",
        artifacts: outputs,
      });
    }
  }
  return formatEvidence;
}

function writeCjsEntrypoint(path, bundle, key, names) {
  const contents = cjsWrapper(path, bundle, key, names);
  write(path, contents);
  artifactRows.push(artifact(path, { role: "package-entrypoint" }));
}

function writeEsmEntrypoint(path, source, names, expectedModule) {
  const contents = namedExportSource(source, names, expectedModule, path);
  write(path, contents);
  artifactRows.push(artifact(path, { role: "package-entrypoint" }));
}

function writePackageEntrypoints(cjsDevelopment) {
  for (const audited of inventory.packages) {
    const row = runtimeModules.get(audited.name);
    const entry = row.entry;
    write(resolve(entry.root, "package.json"), `${JSON.stringify(packageManifest(entry, audited), null, 2)}\n`);
    if (entry.name === "vue") continue;
    writeCjsEntrypoint(
      resolve(entry.root, "index.js"),
      cjsDevelopment,
      cjsKey(entry.name),
      row.exports,
    );
    if (entry.name === "@vue/runtime-test") {
      writeEsmEntrypoint(
        resolve(entry.root, "dist/runtime-test.esm-bundler.js"),
        entry.source,
        row.exports,
        row.expectedFull,
      );
    }
  }

  const vue = runtimeModules.get("vue");
  const root = vue.entry.root;
  writeEsmEntrypoint(
    resolve(root, "index.mjs"),
    vue.entry.runtimeSource,
    vue.runtimeExports,
    vue.expectedRuntime,
  );
  writeCjsEntrypoint(
    resolve(root, "index.cjs"),
    cjsDevelopment,
    cjsKey("vue"),
    vue.exports,
  );
  // A package boundary without an explicit type lets Node 24 syntax-detect the
  // upstream-compatible mix of ESM and CJS .js distribution files.
  write(resolve(root, "dist/package.json"), "{}\n");

  for (const [directory, packageName, source, exports, key] of [
    ["compiler-sfc", "@vue/compiler-sfc", packageRows.get("@vue/compiler-sfc").source, runtimeModules.get("@vue/compiler-sfc").exports, cjsKey("@vue/compiler-sfc")],
    ["server-renderer", "@vue/server-renderer", packageRows.get("@vue/server-renderer").source, runtimeModules.get("@vue/server-renderer").exports, cjsKey("@vue/server-renderer")],
  ]) {
    const directoryPath = resolve(root, directory);
    write(resolve(directoryPath, "package.json"), `${JSON.stringify({ main: "index.js", module: "index.mjs" }, null, 2)}\n`);
    writeCjsEntrypoint(resolve(directoryPath, "index.js"), cjsDevelopment, key, exports);
    writeEsmEntrypoint(
      resolve(directoryPath, "index.mjs"),
      source,
      exports,
      runtimeModules.get(packageName).expectedFull,
    );
    if (packageName === "@vue/compiler-sfc") {
      writeCjsEntrypoint(
        resolve(directoryPath, "index.browser.js"),
        cjsDevelopment,
        key,
        exports,
      );
      writeEsmEntrypoint(
        resolve(directoryPath, "index.browser.mjs"),
        source,
        exports,
        runtimeModules.get(packageName).expectedFull,
      );
    }
  }

  const jsxDirectory = resolve(root, "jsx-runtime");
  const jsxEsm = `import { Fragment, h } from "../vue.runtime.js";
function jsx(type, props, key) {
  const { children, ...rest } = props ?? {};
  if (arguments.length > 2) rest.key = key;
  return h(type, rest, children);
}
export { Fragment, jsx, jsx as jsxDEV, jsx as jsxs };
`;
  const jsxCjs = `"use strict";
const { Fragment, h } = require("../dist/vuelil-packages.cjs").vue_runtime;
function jsx(type, props, key) {
  const { children, ...rest } = props || {};
  if (arguments.length > 2) rest.key = key;
  return h(type, rest, children);
}
exports.Fragment = Fragment;
exports.jsx = jsx;
exports.jsxDEV = jsx;
exports.jsxs = jsx;
`;
  write(resolve(jsxDirectory, "package.json"), `${JSON.stringify({ main: "index.js", module: "index.mjs", types: "index.d.ts" }, null, 2)}\n`);
  write(resolve(jsxDirectory, "index.js"), jsxCjs);
  write(resolve(jsxDirectory, "index.mjs"), jsxEsm);
  artifactRows.push(artifact(resolve(jsxDirectory, "index.js"), { role: "package-entrypoint" }));
  artifactRows.push(artifact(resolve(jsxDirectory, "index.mjs"), { role: "package-entrypoint" }));
}

function concreteTargets(value, targets = []) {
  if (typeof value === "string") {
    if (!value.includes("*")) targets.push(value);
  } else if (value && typeof value === "object") {
    for (const child of Object.values(value)) concreteTargets(child, targets);
  }
  return targets;
}

function verifyPackageEntrypoints() {
  const evidence = [];
  for (const audited of inventory.packages) {
    const entry = packageRows.get(audited.name);
    const manifestPath = resolve(entry.root, "package.json");
    const manifest = readJson(manifestPath);
    const actualEntrypoints = manifest.exports && typeof manifest.exports === "object"
      ? Object.keys(manifest.exports).sort(compareText)
      : manifest.main || manifest.module || manifest.types
        ? ["."]
        : [];
    assert.deepEqual(actualEntrypoints, audited.packageEntrypoints, `${entry.name} entrypoints`);
    const targets = [manifest.main, manifest.module, manifest.types, manifest.unpkg, manifest.jsdelivr]
      .filter(value => typeof value === "string");
    concreteTargets(manifest.exports, targets);
    const files = [...new Set(targets)].map(target => {
      const path = resolve(entry.root, target);
      assert(existsSync(path), `${entry.name} entrypoint target ${target} is missing`);
      return artifact(path);
    });
    evidence.push({
      name: entry.name,
      status: "passed",
      entrypoints: "passed",
      publicExports: "passed",
      publicExportNames: audited.publicExports,
      declarationExportNames: declarationSurfaces.get(entry.name),
      packageEntrypoints: audited.packageEntrypoints,
      manifest: artifact(manifestPath),
      targets: files,
      runtimeExportNames: runtimeModules.get(entry.name).exports,
      runtimeDescriptors: descriptors(
        runtimeModules.get(entry.name).expectedFull,
        runtimeModules.get(entry.name).exports,
      ),
    });
  }
  return evidence;
}

function verifyNodePackageResolution() {
  const nodeModules = resolve(temporary, "resolution/node_modules");
  mkdirSync(resolve(nodeModules, "@vue"), { recursive: true });
  for (const [name, row] of runtimeModules) {
    const link = name === "vue"
      ? resolve(nodeModules, "vue")
      : resolve(nodeModules, ...name.split("/"));
    symlinkSync(row.entry.root, link, "dir");
  }

  const expectations = Object.fromEntries(
    [...runtimeModules].map(([name, row]) => [name, row.exports]),
  );
  const cjsTest = resolve(temporary, "resolution/require.cjs");
  write(cjsTest, `const assert = require("node:assert/strict");
const expectations = ${JSON.stringify(expectations)};
for (const [name, expected] of Object.entries(expectations)) {
  assert.deepEqual(Object.keys(require(name)).sort(), expected, name);
}
assert.equal(typeof require("vue/compiler-sfc").parse, "function");
assert.equal(typeof require("vue/server-renderer").renderToString, "function");
assert.deepEqual(Object.keys(require("vue/jsx-runtime")).sort(), ["Fragment", "jsx", "jsxDEV", "jsxs"]);
assert.equal(require("vue/package.json").name, "vue");
`);
  const esmTest = resolve(temporary, "resolution/import.mjs");
  write(esmTest, `import assert from "node:assert/strict";
const expectations = ${JSON.stringify(expectations)};
for (const [name, expected] of Object.entries(expectations)) {
  const loaded = await import(name);
  for (const exported of expected) assert.ok(exported in loaded, \`${"${name}"} missing ${"${exported}"}\`);
}
assert.equal(typeof (await import("vue/compiler-sfc")).parse, "function");
assert.equal(typeof (await import("vue/server-renderer")).renderToString, "function");
assert.equal(typeof (await import("vue/jsx-runtime")).jsx, "function");
assert.equal(typeof (await import("vue/jsx-dev-runtime")).jsxDEV, "function");
`);
  for (const [path, mode] of [[cjsTest, "require"], [esmTest, "import"]]) {
    const result = spawnSync(process.execPath, [path], {
      cwd: dirname(path),
      encoding: "utf8",
      maxBuffer: 32 * 1024 * 1024,
    });
    if (result.status !== 0) {
      throw new Error(`Node ${mode} package resolution failed:\n${result.stdout ?? ""}${result.stderr ?? ""}`);
    }
  }
  return { import: "passed", require: "passed" };
}

function typeScriptPaths() {
  return Object.fromEntries([
    ["vue", resolve(packageRows.get("vue").root, "dist/vue.d.mts")],
    ["vue/jsx", resolve(packageRows.get("vue").root, "jsx.d.ts")],
    ...inventory.packages
      .filter(entry => entry.name.startsWith("@vue/"))
      .map(entry => [entry.name, declarationTarget(packageRows.get(entry.name))]),
  ].map(([name, path]) => [name, [path]]));
}

function verifyDeclarationSurfaces() {
  const program = ts.createProgram({
    rootNames: inventory.packages.map(entry =>
      declarationTarget(packageRows.get(entry.name))
    ),
    options: {
      baseUrl: projectRoot,
      lib: ["lib.esnext.d.ts", "lib.dom.d.ts"],
      module: ts.ModuleKind.ESNext,
      moduleResolution: ts.ModuleResolutionKind.Bundler,
      paths: typeScriptPaths(),
      skipLibCheck: true,
      strict: true,
      target: ts.ScriptTarget.ES2022,
    },
  });
  const checker = program.getTypeChecker();
  for (const audited of inventory.packages) {
    const path = declarationTarget(packageRows.get(audited.name));
    const source = program.getSourceFile(path);
    assert(source, `${audited.name} declaration was not loaded`);
    const symbol = checker.getSymbolAtLocation(source);
    assert(symbol, `${audited.name} declaration has no module symbol`);
    const names = checker.getExportsOfModule(symbol)
      .map(entry => entry.getName())
      .filter(name => name !== "__esModule")
      .sort(compareText);
    declarationSurfaces.set(audited.name, names);
  }
}

function runTsc(config, label, traceResolution = false) {
  const tsc = existsSync(resolve(projectRoot, "node_modules/.bin/tsc"))
    ? resolve(projectRoot, "node_modules/.bin/tsc")
    : resolve(upstreamRoot, "node_modules/.bin/tsc");
  assert(existsSync(tsc), "TypeScript is not installed; run npm install");
  const result = spawnSync(tsc, ["-p", config, "--pretty", "false", ...(traceResolution ? ["--traceResolution"] : [])], {
    cwd: projectRoot,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(`${label} failed:\n${result.stdout ?? ""}${result.stderr ?? ""}`);
  }
  return result.stdout ?? "";
}

function verifyDeclarationTests() {
  const testPaths = inventory.declarationTestFiles.map(path => resolve(upstreamRoot, path));
  const before = new Map(testPaths.map(path => [path, sha256(readFileSync(path))]));
  const builtConfig = resolve(temporary, "tsconfig.dts-built.json");
  const builtOutput = resolve(temporary, "dts-built");
  const builtSource = resolve(upstreamRoot, "packages-private/dts-built-test/src/index.ts");
  write(builtConfig, `${JSON.stringify({
    compilerOptions: {
      baseUrl: projectRoot,
      declaration: true,
      emitDeclarationOnly: true,
      ignoreDeprecations: "6.0",
      jsx: "preserve",
      lib: ["esnext", "dom"],
      module: "esnext",
      moduleResolution: "bundler",
      outDir: builtOutput,
      paths: typeScriptPaths(),
      rootDir: dirname(builtSource),
      strict: true,
    },
    files: [builtSource],
  }, null, 2)}\n`);
  runTsc(builtConfig, "declaration fixture build");
  const builtDeclaration = resolve(builtOutput, "index.d.ts");
  assert(existsSync(builtDeclaration), "declaration fixture did not emit index.d.ts");

  const testConfig = resolve(temporary, "tsconfig.declaration-tests.json");
  write(testConfig, `${JSON.stringify({
    compilerOptions: {
      baseUrl: projectRoot,
      ignoreDeprecations: "6.0",
      jsx: "preserve",
      lib: ["esnext", "dom"],
      module: "esnext",
      moduleResolution: "bundler",
      noEmit: true,
      paths: {
        ...typeScriptPaths(),
        "dts-built-test/src/index": [builtDeclaration],
      },
      strict: true,
    },
    files: [
      ...testPaths,
      resolve(upstreamRoot, "packages-private/dts-test/utils.d.ts"),
    ],
  }, null, 2)}\n`);
  const trace = runTsc(testConfig, "unchanged Vue declaration tests", true);
  for (const match of trace.matchAll(/Module name '((?:vue(?:\/jsx)?)|(?:@vue\/[^']+))' was successfully resolved to '([^']+)'/gu)) {
    assert(
      resolve(match[2]).startsWith(resolve(projectRoot, "packages/vuelil")),
      `${match[1]} resolved outside candidate declarations: ${match[2]}`,
    );
  }
  const rows = testPaths.map(path => {
    const current = sha256(readFileSync(path));
    assert.equal(current, before.get(path), `${relativePath(path)} changed during declaration tests`);
    const git = spawnSync("git", ["show", `HEAD:${relative(upstreamRoot, path).replaceAll("\\", "/")}`], {
      cwd: upstreamRoot,
      encoding: null,
      maxBuffer: 16 * 1024 * 1024,
    });
    assert.equal(git.status, 0, `cannot read pinned ${relativePath(path)}`);
    assert.equal(current, sha256(git.stdout), `${relativePath(path)} differs from pinned upstream`);
    return {
      path: relative(upstreamRoot, path).replaceAll("\\", "/"),
      status: "passed",
      sha256: current,
      unchanged: true,
    };
  });
  assert.equal(rows.length, 20);
  return rows;
}

function verifyExistingRuntimeEvidence() {
  assert.equal(scope.gates.runtimeTestFilesPassed, inventory.totals.upstreamTestFiles);
  return inventory.packages.flatMap(audited => {
    const evidenceName = audited.directory.split("/").at(-1);
    const evidencePath = resolve(projectRoot, `compatibility/${evidenceName}.json`);
    const evidence = readJson(evidencePath);
    assert.equal(evidence.upstreamTests?.status, "passed", `${audited.name} runtime tests`);
    assert.equal(evidence.upstreamTests?.files, audited.testFiles.length, `${audited.name} runtime file count`);
    return audited.testFiles.map(path => {
      const source = readFileSync(resolve(upstreamRoot, path));
      const git = spawnSync("git", ["show", `HEAD:${path}`], {
        cwd: upstreamRoot,
        encoding: null,
        maxBuffer: 16 * 1024 * 1024,
      });
      assert.equal(git.status, 0, `cannot read pinned ${path}`);
      assert.equal(sha256(source), sha256(git.stdout), `${path} differs from pinned upstream`);
      return {
        path,
        status: "passed",
        sha256: sha256(source),
        unchanged: true,
        evidence: relativePath(evidencePath),
        evidenceSha256: sha256(readFileSync(evidencePath)),
      };
    });
  });
}

async function main() {
  assert.equal(formatManifest.schemaVersion, 1);
  assert.equal(formatManifest.upstreamVersion, inventory.upstream.version);
  assert.equal(packageRows.size, inventory.packages.length);
  copyDeclarations();
  verifyDeclarationSurfaces();
  await loadRuntimeModules();
  const cjsDevelopment = await buildCjsBundle(false);
  const cjsProduction = await buildCjsBundle(true);
  writePackageEntrypoints(cjsDevelopment);
  const formats = await buildFormats(cjsDevelopment, cjsProduction);
  const packages = verifyPackageEntrypoints();
  const packageResolution = verifyNodePackageResolution();
  const declarationTests = verifyDeclarationTests();
  const runtimeTests = verifyExistingRuntimeEvidence();

  const scanned = [...artifactRows, ...formats.flatMap(row => row.artifacts)]
    .filter((entry, index, all) => all.findIndex(other => other.path === entry.path) === index)
    .sort((left, right) => compareText(left.path, right.path));
  for (const entry of scanned.filter(({ path }) => /\.[cm]?js$/u.test(path))) {
    assertNoUpstreamImport(readFileSync(resolve(projectRoot, entry.path), "utf8"), entry.path);
  }

  const report = {
    schemaVersion: 1,
    upstream: {
      version: inventory.upstream.version,
      revision: inventory.upstream.revision,
      tree: inventory.upstream.tree,
    },
    inventory: { sha256: sha256(readFileSync(inventoryPath)) },
    noUpstreamImplementationImports: true,
    packageResolution,
    totals: {
      packages: packages.length,
      packageEntrypoints: packages.reduce((total, entry) => total + entry.packageEntrypoints.length, 0),
      publicExports: packages.reduce((total, entry) => total + entry.publicExportNames.length, 0),
      declarationExports: packages.reduce(
        (total, entry) => total + entry.declarationExportNames.length,
        0,
      ),
      runtimeExports: packages.reduce((total, entry) => total + entry.runtimeExportNames.length, 0),
      runtimeDescriptors: packages.reduce(
        (total, entry) => total + Object.keys(entry.runtimeDescriptors).length,
        0,
      ),
      formatRows: formats.length,
      formatArtifacts: formats.reduce((total, entry) => total + entry.artifacts.length, 0),
      formatDescriptors: formats.reduce(
        (total, entry) => total + entry.artifacts.reduce(
          (subtotal, output) => subtotal + Object.keys(output.descriptors).length,
          0,
        ),
        0,
      ),
      declarations: declarationRows.length,
      artifactEvidence: scanned.length,
      declarationTestFiles: declarationTests.length,
      runtimeTestFiles: runtimeTests.length,
      tests: runtimeTests.length + declarationTests.length,
      passed: runtimeTests.length + declarationTests.length,
      failed: 0,
    },
    declarations: declarationRows.sort((left, right) => compareText(left.path, right.path)),
    tests: [...runtimeTests, ...declarationTests].sort((left, right) => compareText(left.path, right.path)),
    packages,
    formats,
    artifacts: scanned,
  };
  write(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  console.log(JSON.stringify({ report: reportPath, ...report.totals }));
}

try {
  await main();
} catch (error) {
  const stack = error instanceof Error ? error.stack ?? error.message : String(error);
  console.error(stack.replace(/data:text\/javascript;base64,[^\s)]+/gu, "<generated module>"));
  process.exitCode = 1;
} finally {
  rmSync(temporary, { force: true, recursive: true });
}
