import { spawnSync } from "node:child_process";
import {
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

const sourceDirectory = resolve(projectRoot, "src/runtime-core");
const sharedDirectory = resolve(projectRoot, "src/shared");
const host = resolve(projectRoot, "src/runtime-core/host.js");
const output = resolve(projectRoot, "packages/vuelil/runtime-core.js");
const implementationOutput = resolve(projectRoot, "packages/vuelil/runtime-core.internal.js");
const upstreamCandidate = resolve(projectRoot, "tests/runtime-core-upstream.candidate.mjs");
const temporary = mkdtempSync(resolve(tmpdir(), "vuelil-runtime-core-"));
const compiled = resolve(temporary, "index.js");
const temporarySourceRoot = resolve(temporary, "src");
const temporaryRuntimeCore = resolve(temporarySourceRoot, "runtime-core");
const source = resolve(temporaryRuntimeCore, "index.lil");

const anonymousFunctions = new Set([
  "onBeforeMount", "onBeforeUnmount", "onBeforeUpdate", "onMounted", "onRenderTracked",
  "onRenderTriggered", "onServerPrefetch", "onUnmounted", "onUpdated",
]);

const functionLengths = new Map([
  ["createBaseVNode", 1], ["createCommentVNode", 0], ["createElementVNode", 1],
  ["createTextVNode", 0], ["createVNode", 0], ["hydrateOnIdle", 0],
  ["hydrateOnInteraction", 0], ["onErrorCaptured", 1], ["renderSlot", 6],
  ["resolveComponent", 2], ["toHandlers", 2], ["useModel", 2], ["withCtx", 1],
]);
const internalTestExports = new Set([
  "SchedulerJobFlags", "createAppAPI", "flushPostFlushCbs", "flushPreFlushCbs",
  "isBlockTreeEnabled", "normalizeVNode", "queueJob", "setCurrentInstance",
  "setCurrentRenderingInstance",
]);

function parseExports(exports) {
  return exports.split(",").map((entry) => {
    const [local, publicName = local] = entry.trim().split(/\s+as\s+/);
    return { local, publicName };
  });
}

function renderedExportName(publicName) {
  return publicName === "renderHyperscript" ? "h" : publicName;
}

function exportedNames(module, includeInternal) {
  const match = module.match(/export\{([^}]*)\}\s*$/);
  if (!match) throw new Error("compiled runtime-core module has no export clause");
  return parseExports(match[1])
    .filter(({ publicName }) => includeInternal || !internalTestExports.has(publicName))
    .map(({ publicName }) => renderedExportName(publicName));
}

function withNamesAndExports(module, includeInternal) {
  return module.replace(/export\{([^}]*)\}\s*$/, (_statement, exports) => {
    const entries = parseExports(exports);
    const definitions = [];
    const namedLocals = new Set();
    for (const { local, publicName } of entries) {
      if (namedLocals.has(local)) continue;
      let name = publicName;
      if (publicName === "renderHyperscript") name = "h";
      if (publicName === "createElementVNode") name = "createBaseVNode";
      if (publicName === "createVNode") name = "createVNodeWithArgsTransform";
      if (publicName === "setDevtoolsHook") name = "setDevtoolsHook$1";
      if (publicName === "warn") name = "warn$1";
      if (["camelize", "capitalize", "toHandlerKey"].includes(publicName)) name = "";
      if (anonymousFunctions.has(publicName)) name = "";
      if (/^[A-Z]/.test(publicName) || [
        "version", "currentBlock", "isBlockTreeEnabled", "currentRenderingInstance",
        "currentScopeId", "currentInstance", "currentApp", "isInSSRComponentSetup",
        "compatUtils", "devtools", "resolveFilter", "ssrContextKey", "ssrUtils",
      ].includes(publicName)) continue;
      definitions.push(
        `Object.defineProperty(${local},"name",{configurable:true,value:${JSON.stringify(name)}});`,
      );
      if (functionLengths.has(publicName)) {
        definitions.push(
          `Object.defineProperty(${local},"length",{configurable:true,value:${functionLengths.get(publicName)}});`,
        );
      }
      namedLocals.add(local);
    }
    const rendered = entries.filter(({ publicName }) =>
      includeInternal || !internalTestExports.has(publicName),
    ).map(({ local, publicName }) => {
      const exported = renderedExportName(publicName);
      return local === exported ? local : `${local} as ${exported}`;
    });
    return `${definitions.join("")}export{${rendered.join(",")}}`;
  });
}

function hostAliasDefinitions(module) {
  const definitions = new Map();
  for (const match of module.matchAll(/import\{([^}]*)\}from["']\.\/host\.js["'];?/g)) {
    for (const entry of match[1].split(",")) {
      const [imported, local = imported] = entry.trim().split(/\s+as\s+/);
      if (local !== imported) definitions.set(local, imported);
    }
  }
  return [...definitions].map(([local, imported]) => `const ${local}=${imported};`).join("");
}

function prepareSourceGraph() {
  cpSync(sourceDirectory, temporaryRuntimeCore, { recursive: true });
  cpSync(sharedDirectory, resolve(temporarySourceRoot, "shared"), { recursive: true });
  const temporaryDependencies = resolve(temporary, "packages/vuelil");
  mkdirSync(temporaryDependencies, { recursive: true });
  cpSync(resolve(projectRoot, "packages/vuelil/reactivity.js"), resolve(temporaryDependencies, "reactivity.js"));
  cpSync(resolve(projectRoot, "packages/vuelil/shared.js"), resolve(temporaryDependencies, "shared.js"));

  const sharedGlobals = new Set([
    "Array", "JSON", "Math", "Number", "Object", "String", "Uint8Array",
    "console", "global", "globalThis", "parseFloat", "parseInt", "process",
    "self", "window",
  ]);
  const runtimeGlobals = [
    "Boolean", "Date", "Element", "Error", "Function", "HTMLElement",
    "IntersectionObserver", "Map", "MathMLElement", "Promise", "Proxy",
    "Reflect", "RegExp", "SVGElement", "Set", "Symbol", "WeakMap", "WeakSet",
    "__COMPAT__", "__GLOBAL__", "clearTimeout", "document", "isNaN",
    "matchMedia", "setTimeout",
  ];
  const runtimeGlobalSet = new Set(runtimeGlobals);
  const buildGlobals = resolve(temporaryRuntimeCore, "build-globals.lil");
  const reactivityBridge = resolve(temporaryRuntimeCore, "reactivity-bridge.lil");
  writeFileSync(reactivityBridge, `
import extern {
  EffectFlags, EffectScope, ReactiveEffect, ReactiveFlags, TrackOpTypes,
  TriggerOpTypes, WatchErrorCodes, computed, customRef, effect, effectScope,
  getCurrentScope, getCurrentWatcher, isProxy, isReactive, isReadonly, isRef,
  isShallow, markRaw, onScopeDispose, onWatcherCleanup, pauseTracking, proxyRefs,
  reactive, readonly, ref, resetTracking, shallowReactive, shallowReadArray,
  shallowReadonly, shallowRef, stop, toRaw, toReactive, toReadonly, toRef,
  toRefs, toValue, track, traverse, trigger, triggerRef, unref, watch
} from "../../packages/vuelil/reactivity.js";
extern JsValue EffectFlags;
extern JsValue EffectScope;
extern JsValue ReactiveEffect;
extern JsValue ReactiveFlags;
extern JsValue TrackOpTypes;
extern JsValue TriggerOpTypes;
extern JsValue WatchErrorCodes;
extern JsValue computed(JsValue getterOrOptions, JsValue debugOptions = JS.undefined());
extern JsValue customRef(JsValue factory);
extern JsValue effect(JsValue fn, JsValue options = JS.undefined());
extern JsValue effectScope(JsValue detached);
extern JsValue getCurrentScope();
extern JsValue getCurrentWatcher();
extern bool isProxy(JsValue value);
extern bool isReactive(JsValue value);
extern bool isReadonly(JsValue value);
extern bool isRef(JsValue value);
extern bool isShallow(JsValue value);
extern JsValue markRaw(JsValue value);
extern void onScopeDispose(JsValue fn, bool failSilently = false);
extern void onWatcherCleanup(JsValue cleanupFn, bool failSilently = false, JsValue owner = JS.undefined());
extern void pauseTracking();
extern JsValue proxyRefs(JsValue objectWithRefs);
extern JsValue reactive(JsValue target);
extern JsValue readonly(JsValue target);
extern JsValue ref(JsValue value);
extern void resetTracking();
extern JsValue shallowReactive(JsValue target);
extern JsValue shallowReadArray(JsValue array);
extern JsValue shallowReadonly(JsValue target);
extern JsValue shallowRef(JsValue value);
extern void stop(JsValue runner);
extern JsValue toRaw(JsValue observed);
extern JsValue toReactive(JsValue value);
extern JsValue toReadonly(JsValue value);
extern JsValue toRef(JsValue source, JsValue key, JsValue defaultValue);
extern JsValue toRefs(JsValue object);
extern JsValue toValue(JsValue source);
extern void track(JsValue target, JsValue operation, JsValue key);
extern JsValue traverse(JsValue value, float depth = 0, JsValue seen = JS.undefined());
extern void trigger(JsValue target, JsValue operation, JsValue key, JsValue newValue, JsValue oldValue, JsValue oldTarget);
extern void triggerRef(JsValue refValue);
extern JsValue unref(JsValue value);
extern JsValue watch;
export bool DEV = true;
export bool TEST = true;
export {
  EffectFlags, EffectScope, ReactiveEffect, ReactiveFlags, TrackOpTypes,
  TriggerOpTypes, WatchErrorCodes, computed, customRef, effect, effectScope,
  getCurrentScope, getCurrentWatcher, isProxy, isReactive, isReadonly, isRef,
  isShallow, markRaw, onScopeDispose, onWatcherCleanup, pauseTracking, proxyRefs,
  reactive, readonly, ref, resetTracking, shallowReactive, shallowReadArray,
  shallowReadonly, shallowRef, stop, toRaw, toReactive, toReadonly, toRef,
  toRefs, toValue, track, traverse, trigger, triggerRef, unref, watch
};
`);
  const sourceFiles = [];
  const collectLilFiles = directory => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const path = resolve(directory, entry.name);
      if (entry.isDirectory()) collectLilFiles(path);
      else if (entry.name.endsWith(".lil")) sourceFiles.push(path);
    }
  };
  collectLilFiles(temporaryRuntimeCore);

  const specifier = (from, target) => {
    const path = relative(dirname(from), target)
      .replaceAll("\\", "/")
      .replace(/\.lil$/, "");
    return path.startsWith(".") ? path : `./${path}`;
  };
  for (const path of sourceFiles) {
    const original = readFileSync(path, "utf8");
    const externalized = original.replace(
      /import\s*\{([^}]*)\}\s*from\s*["'](?:\.\.\/)+reactivity\/[^"']+["'];/g,
      (_statement, imports) =>
        `import {${imports}} from ${JSON.stringify(specifier(path, reactivityBridge))};`,
    );
    const aliased = externalized.replace(
      /^extern JsValue ([A-Za-z_$][A-Za-z0-9_$]*);$/gm,
      (declaration, name) => {
        let owner;
        if (name === "arguments") owner = resolve(temporarySourceRoot, "shared/general.lil");
        else if (sharedGlobals.has(name)) owner = resolve(temporarySourceRoot, "shared/makeMap.lil");
        else if (runtimeGlobalSet.has(name)) owner = buildGlobals;
        else return declaration;
        return `import { ${name} } from ${JSON.stringify(specifier(path, owner))};`;
      },
    );
    if (aliased !== original) writeFileSync(path, aliased);
  }
  writeFileSync(
    buildGlobals,
    `${runtimeGlobals.map(name => name === "__COMPAT__"
      ? "export JsValue __COMPAT__ = false;"
      : `export extern JsValue ${name};`).join("\n")}\n`,
  );

  const barrel = readFileSync(source, "utf8");
  writeFileSync(source, [
    'import { createAppAPI as runtimeTestCreateAppAPI } from "./apiCreateApp";',
    'import { setCurrentInstance as runtimeTestSetCurrentInstance } from "./component";',
    'import { setCurrentRenderingInstance as runtimeTestSetCurrentRenderingInstance } from "./componentRenderContext";',
    'import { isBlockTreeEnabled as runtimeTestIsBlockTreeEnabled } from "./vnode";',
    'import { SchedulerJobFlags as runtimeTestSchedulerJobFlags, flushPostFlushCbs as runtimeTestFlushPostFlushCbs, flushPreFlushCbs as runtimeTestFlushPreFlushCbs, queueJob as runtimeTestQueueJob } from "./scheduler";',
    barrel,
    "export {",
    "  runtimeTestCreateAppAPI as createAppAPI,",
    "  runtimeTestFlushPostFlushCbs as flushPostFlushCbs,",
    "  runtimeTestFlushPreFlushCbs as flushPreFlushCbs,",
    "  runtimeTestIsBlockTreeEnabled as isBlockTreeEnabled,",
    "  normalizeVNode,",
    "  runtimeTestQueueJob as queueJob,",
    "  runtimeTestSchedulerJobFlags as SchedulerJobFlags,",
    "  runtimeTestSetCurrentInstance as setCurrentInstance,",
    "  runtimeTestSetCurrentRenderingInstance as setCurrentRenderingInstance",
    "};",
    "",
  ].join("\n"));
}

try {
  prepareSourceGraph();
  const result = spawnSync(
    compilerPath(),
    [
      source,
      "--target", "js-module",
      "--config", resolve(repositoryRoot, "tests/config/no-optimization-no-peephole.toml"),
      "-o", compiled,
    ],
    { cwd: projectRoot, encoding: "utf8", env: process.env },
  );
  if (result.status !== 0) throw new Error(`${result.stdout ?? ""}${result.stderr ?? ""}`);

  mkdirSync(resolve(projectRoot, "packages/vuelil"), { recursive: true });
  const hostModule = readFileSync(host, "utf8").replaceAll("export function ", "function ");
  const compiledSource = readFileSync(compiled, "utf8");
  const hostAliases = hostAliasDefinitions(compiledSource);
  let compiledModule = compiledSource
    .replaceAll('"../../packages/vuelil/reactivity.js"', '"./reactivity.js"')
    .replaceAll('"../../packages/vuelil/shared.js"', '"./shared.js"')
    .replaceAll('"../../../packages/vuelil/reactivity.js"', '"./reactivity.js"')
    .replaceAll('"../../../packages/vuelil/shared.js"', '"./shared.js"')
    .replace(/import\{[^;]*\}from["']\.\/host\.js["'];?/g, "");
  if (/from["']\.\/host\.js["']/.test(compiledModule)) {
    throw new Error("failed to inline the runtime-core host adapter");
  }

  const banner = "// Generated from the mirrored src/runtime-core/*.lil graph and its ECMAScript host adapter.\n";
  const implementationModule = withNamesAndExports(compiledModule, true);
  writeFileSync(
    implementationOutput,
    `${banner}${hostModule}\n${hostAliases}\n${implementationModule}\n`,
  );

  const publicExports = exportedNames(compiledModule, false);
  writeFileSync(
    output,
    `${banner}export { ${publicExports.join(", ")} } from "./runtime-core.internal.js";\n`,
  );

  writeFileSync(
    upstreamCandidate,
    `${banner}export * from "../packages/vuelil/runtime-core.internal.js";\n`,
  );

  const runtime = await import(`../packages/vuelil/runtime-core.js?build=${Date.now()}`);
  console.log(JSON.stringify({
    output,
    exports: Object.keys(runtime).sort(),
    bytes: readFileSync(output).byteLength,
  }));
} finally {
  rmSync(temporary, { force: true, recursive: true });
}
