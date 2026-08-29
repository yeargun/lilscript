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
import { fileURLToPath } from "node:url";
import { build } from "vite";
import { minify } from "rolldown/utils";
import { codecPath, compilerPath, projectRoot } from "../tooling/compiler-path.mjs";
import {
  reactivityExportOwners,
  renderClosedReactivityBuildEntry,
} from "../src/reactivity/build-entry.mjs";

const source = resolve(projectRoot, "src/reactivity/index.lil");
const sourceDirectory = resolve(projectRoot, "src/reactivity");
const sharedDirectory = resolve(projectRoot, "src/shared");
const host = resolve(projectRoot, "src/reactivity/host.js");
const developmentOutput = resolve(projectRoot, "packages/vuelil/reactivity.js");
const productionOutput = resolve(projectRoot, "artifacts/reactivity.esm-browser.prod.js");
const reportOutput = resolve(projectRoot, "artifacts/reactivity-size-report.json");
const upstream = resolve(
  projectRoot,
  "upstream/vue/packages/reactivity/dist/reactivity.esm-browser.prod.js",
);
const upstreamCandidate = resolve(projectRoot, "tests/reactivity-upstream.candidate.js");
const productionConfig = resolve(projectRoot, "config/reactivity-production.toml");

const publicFunctionNames = new Set([
  "computed", "customRef", "effect", "effectScope", "enableTracking",
  "getCurrentScope", "getCurrentWatcher", "isProxy", "isReactive",
  "isReadonly", "isRef", "isShallow", "markRaw", "onEffectCleanup",
  "onScopeDispose", "onWatcherCleanup", "pauseTracking", "proxyRefs",
  "reactive", "reactiveReadArray", "readonly", "ref", "resetTracking",
  "shallowReactive", "shallowReadArray", "shallowReadonly", "shallowRef",
  "stop", "toRaw", "toReactive", "toReadonly", "toRef", "toRefs",
  "toValue", "track", "traverse", "trigger", "triggerRef", "unref", "watch",
]);
const runtimeExports = [
  "ARRAY_ITERATE_KEY", "EffectFlags", "EffectScope", "ITERATE_KEY",
  "MAP_KEY_ITERATE_KEY", "ReactiveEffect", "ReactiveFlags", "TrackOpTypes",
  "TriggerOpTypes", "WatchErrorCodes", "computed", "customRef", "effect",
  "effectScope", "enableTracking", "getCurrentScope", "getCurrentWatcher",
  "isProxy", "isReactive", "isReadonly", "isRef", "isShallow", "markRaw",
  "onEffectCleanup", "onScopeDispose", "onWatcherCleanup", "pauseTracking",
  "proxyRefs", "reactive", "reactiveReadArray", "readonly", "ref",
  "resetTracking", "shallowReactive", "shallowReadArray", "shallowReadonly",
  "shallowRef", "stop", "toRaw", "toReactive", "toReadonly", "toRef",
  "toRefs", "toValue", "track", "traverse", "trigger", "triggerRef", "unref",
  "watch",
];
const productionMinifyOptions = {
  module: true,
  compress: { target: "es2022" },
  mangle: true,
  mangleProps: {
    include: /^(?:allowRecurse|boundCleanup|callHook|computedOrders|computeds|dirtyValue|effectOrders|effectValue|evaluating|evaluationDepth|facade|forceTrigger|getter|hadSources|handle|hasCallback|mapOwner|multi|notifyVersion|onCount|output|pendingWhilePaused|publicEffects|publicMap|publicScopes|rawValue|selfInvalidated|sources|tracking|versions)$/,
  },
  codegen: { removeWhitespace: true, legalComments: "none" },
};

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: projectRoot,
    encoding: "utf8",
    env: process.env,
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(`${result.stdout ?? ""}${result.stderr ?? ""}`);
  }
}

function bytes(path) {
  return readFileSync(path).byteLength;
}

function normalizePublicFunctionNames(code, compact = false) {
  return code.replace(/export\{([^}]*)\}\s*$/, (statement, exports) => {
    const functions = exports
      .split(",")
      .map(entry => {
        const [local, publicName = local] = entry.trim().split(/\s+as\s+/);
        return { local, publicName };
      })
      .filter(({ publicName }) => publicFunctionNames.has(publicName));
    const definitions = compact
      ? `for(const[f,n]of[${functions.map(({ local, publicName }) =>
          `[${local},${JSON.stringify(publicName)}]`).join(",")}])Object.defineProperty(f,"name",{configurable:true,value:n});`
      : functions
        .map(({ local, publicName }) =>
          `Object.defineProperty(${local},"name",{configurable:true,value:${JSON.stringify(publicName)}});`,
        )
        .join("");
    return `${definitions}${statement}`;
  });
}

function renderExportEntry(exportNames, specifier = "./index") {
  const names = [...new Set(exportNames)].sort();
  return `import { ${names.join(", ")} } from ${JSON.stringify(specifier)};\n` +
    `export { ${names.join(", ")} };\n`;
}

function assemble(compiledPath, hostSource, nameMode = "expanded") {
  const compiledModule = readFileSync(compiledPath, "utf8").replace(
    /^import\s*\{[\s\S]*?\}\s*from\s*["']\.\/host\.js["'];?\s*/,
    "",
  );
  if (/from\s*["']\.\/host\.js["']/.test(compiledModule)) {
    throw new Error("failed to inline the foreign host adapter import");
  }
  const assembled = `${hostSource}\n${compiledModule}`;
  if (nameMode === "none") return assembled;
  return normalizePublicFunctionNames(assembled, nameMode === "compact");
}

function compile(input, output, mode, config, target = "js-module") {
  const args = [input, "--target", target, "--mode", mode];
  if (config) args.push("--config", config);
  args.push("--jobs", "1", "--codec-jobs", "1");
  args.push("-o", output);
  run(compilerPath(), args);
}

function prepareSourceGraph(temporary, production) {
  const sourceRoot = resolve(temporary, "src");
  const graph = resolve(sourceRoot, "reactivity");
  mkdirSync(sourceRoot, { recursive: true });
  cpSync(sourceDirectory, graph, { recursive: true });
  cpSync(sharedDirectory, resolve(sourceRoot, "shared"), { recursive: true });
  if (production) {
    const constants = resolve(graph, "constants.lil");
    const original = readFileSync(constants, "utf8");
    const selected = original
      .replace("export bool DEV = true;", "export bool DEV = false;")
      .replace("export bool TEST = true;", "export bool TEST = false;");
    if (selected === original) throw new Error("failed to select production constants");
    writeFileSync(constants, selected);
  }
  writeFileSync(resolve(graph, "production.lil"), renderExportEntry(runtimeExports));
  writeFileSync(resolve(graph, "test.lil"), [
    `import { ${runtimeExports.join(", ")} } from "./index";`,
    'import { getDepFromReactive, targetMap } from "./dep";',
    'import { endBatch, startBatch } from "./effect";',
    `export { ${[...runtimeExports, "endBatch", "getDepFromReactive", "startBatch", "targetMap"].join(", ")} };`,
    "",
  ].join("\n"));
  return graph;
}

export async function buildSelectedReactivity(exportNames, output) {
  const temporary = mkdtempSync(resolve(tmpdir(), "vuelil-reactivity-selected-"));
  const selectedCompiled = resolve(temporary, "selected.js");
  try {
    const names = [...new Set(exportNames)].sort();
    const publicExports = new Set(runtimeExports);
    const unsupported = names.filter((name) => !publicExports.has(name));
    if (unsupported.length > 0) {
      throw new Error(`unknown @vue/reactivity exports: ${unsupported.join(", ")}`);
    }
    const graph = prepareSourceGraph(temporary, true);
    const selectedSource = resolve(graph, "selected.lil");
    const owners = reactivityExportOwners(readFileSync(resolve(graph, "index.lil"), "utf8"));
    const hostModule = readFileSync(host, "utf8").replaceAll(
      "export function ",
      "function ",
    );
    writeFileSync(
      selectedSource,
      renderClosedReactivityBuildEntry(exportNames, owners),
    );
    compile(selectedSource, selectedCompiled, "production", productionConfig, "js");
    const bridge = [
      "let __vuelilSelectedExports;",
      "function hostInstallSelected(value){__vuelilSelectedExports=value}",
    ].join("\n");
    const bindings = names
      .map((name) => `const ${name}=__vuelilSelectedExports[${JSON.stringify(name)}];`)
      .join("\n");
    const assembled = `${bridge}\n${assemble(selectedCompiled, hostModule, "none")}\n${bindings}\nexport{${names.join(",")}};`;
    const minified = await minify(
      "reactivity.selected.js",
      assembled,
      productionMinifyOptions,
    );
    if (minified.errors.length > 0) {
      throw new Error(minified.errors.map(error => error.message).join("\n"));
    }
    const code = minified.code;
    mkdirSync(resolve(output, ".."), { recursive: true });
    writeFileSync(output, code);
    return {
      code,
      exportNames: names,
      sourcePath: source,
      sourceKind: "complete-reactivity",
    };
  } finally {
    rmSync(temporary, { force: true, recursive: true });
  }
}

export async function buildReactivity() {
  const temporary = mkdtempSync(resolve(tmpdir(), "vuelil-reactivity-"));
  const developmentGraph = resolve(temporary, "development");
  const productionGraph = resolve(temporary, "production");
  const developmentInternalCompiled = resolve(temporary, "development-internal.js");
  const developmentCompiled = resolve(temporary, "development.js");
  const productionCompiled = resolve(temporary, "production.js");
  const productionEntry = resolve(temporary, "production-entry.mjs");
  try {
  const hostText = readFileSync(host, "utf8");
  const hostModule = hostText.replaceAll("export function ", "function ");
  const banner = "// Generated from the mirrored src/reactivity/*.lil graph and its ECMAScript host adapter.\n";

  const developmentSource = prepareSourceGraph(developmentGraph, false);
  compile(resolve(developmentSource, "test.lil"), developmentInternalCompiled, "development");
  writeFileSync(upstreamCandidate, `${banner}${assemble(developmentInternalCompiled, hostModule)}`);
  compile(resolve(developmentSource, "production.lil"), developmentCompiled, "development");
  const developmentModule = `${banner}${assemble(developmentCompiled, hostModule)}`;
  mkdirSync(resolve(projectRoot, "packages/vuelil"), { recursive: true });
  writeFileSync(developmentOutput, developmentModule);

  const productionSource = prepareSourceGraph(productionGraph, true);
  compile(resolve(productionSource, "production.lil"), productionCompiled, "production", productionConfig);
  writeFileSync(productionEntry, assemble(productionCompiled, hostModule, "compact"));

  const bundled = await build({
    configFile: false,
    logLevel: "silent",
    build: {
      write: false,
      target: "es2022",
      minify: "oxc",
      lib: { entry: productionEntry, formats: ["es"], fileName: "reactivity" },
      rolldownOptions: { output: { codeSplitting: false } },
    },
  });
  const chunks = (Array.isArray(bundled) ? bundled.flatMap(item => item.output) : bundled.output)
    .filter(item => item.type === "chunk");
  if (chunks.length !== 1) throw new Error(`expected one production chunk, received ${chunks.length}`);
  mkdirSync(resolve(projectRoot, "artifacts"), { recursive: true });
  const minified = await minify(
    "reactivity.esm-browser.prod.js",
    chunks[0].code,
    productionMinifyOptions,
  );
  if (minified.errors.length > 0) {
    throw new Error(minified.errors.map(error => error.message).join("\n"));
  }
  for (const developmentOnly of [
    "[Vue warn]",
    "Invalid watch source:",
    "onTrack",
    "onTrigger",
  ]) {
    if (minified.code.includes(developmentOnly)) {
      throw new Error(`production artifact retained development diagnostic: ${developmentOnly}`);
    }
  }
  writeFileSync(productionOutput, minified.code);

  const measurement = spawnSync(
    codecPath(),
    ["--json", developmentOutput, productionOutput, upstream, productionCompiled, host],
    { cwd: projectRoot, encoding: "utf8", env: process.env },
  );
  if (measurement.status !== 0) {
    throw new Error(`${measurement.stdout ?? ""}${measurement.stderr ?? ""}`);
  }
  const measured = JSON.parse(measurement.stdout);
  const [development, production, upstreamResult, compiledKernel, hostAdapter] =
    measured.artifacts;
  const metrics = ({ raw, gzip9, brotli11 }) => ({ raw, gzip9, brotli11 });
  const report = {
    schemaVersion: 1,
    generatedBy: "scripts/build-reactivity.mjs",
    baseline: {
      path: "packages/vuelil/reactivity.js",
      raw: 52380,
      brotli11: 11889,
    },
    codecs: measured.codecs,
    artifacts: {
      development: {
        path: "packages/vuelil/reactivity.js",
        ...metrics(development),
      },
      production: {
        path: "artifacts/reactivity.esm-browser.prod.js",
        ...metrics(production),
      },
      upstream: {
        path: "upstream/vue/packages/reactivity/dist/reactivity.esm-browser.prod.js",
        ...metrics(upstreamResult),
      },
      compiledKernelBeforeBundling: metrics(compiledKernel),
      hostAdapterSource: metrics(hostAdapter),
    },
    comparison: {
      brotli11Delta: production.brotli11 - upstreamResult.brotli11,
      beatsUpstreamBrotli11: production.brotli11 < upstreamResult.brotli11,
      requiredAdditionalBrotli11Reduction: Math.max(
        0,
        production.brotli11 - upstreamResult.brotli11 + 1,
      ),
      reductionFromDevelopment: {
        raw: development.raw - production.raw,
        gzip9: development.gzip9 - production.gzip9,
        brotli11: development.brotli11 - production.brotli11,
      },
      reductionFromRequestedBaseline: {
        raw: 52380 - production.raw,
        brotli11: 11889 - production.brotli11,
      },
    },
    blocker: production.brotli11 < upstreamResult.brotli11
      ? null
      : "The source-parity port still pays for generic JsValue property access and the primitive host ABI; maximum Brotli search plus whole-module OXC minification does not yet match Vue's hand-authored production build.",
  };
  writeFileSync(reportOutput, `${JSON.stringify(report, null, 2)}\n`);

  console.log(JSON.stringify({
    developmentOutput,
    productionOutput,
    sourceBytes: bytes(source),
    hostBytes: bytes(host),
    development,
    production,
    upstream: upstreamResult,
    compiledKernel,
    hostAdapter,
  }));
  } finally {
    rmSync(temporary, { force: true, recursive: true });
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  await buildReactivity();
}
