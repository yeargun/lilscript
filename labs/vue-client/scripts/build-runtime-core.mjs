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
import { runtimeCoreTestFlags } from "../tests/runtime-core-test-flags.mjs";

const sourceDirectory = resolve(projectRoot, "src/runtime-core");
const sharedDirectory = resolve(projectRoot, "src/shared");
const host = resolve(projectRoot, "src/runtime-core/host.js");
const output = resolve(projectRoot, "packages/vuelil/runtime-core.js");
const implementationOutput = resolve(projectRoot, "packages/vuelil/runtime-core.internal.js");
const compatOutput = resolve(projectRoot, "packages/vuelil/runtime-core.compat.js");
const compatImplementationOutput = resolve(
  projectRoot,
  "packages/vuelil/runtime-core.compat.internal.js",
);
const upstreamCandidate = resolve(projectRoot, "tests/runtime-core-upstream.candidate.mjs");
const browserCandidate = resolve(projectRoot, "tests/vue-runtime-core-browser.candidate.mjs");
const temporary = mkdtempSync(resolve(tmpdir(), "vuelil-runtime-core-"));
const productionRoot = resolve(temporary, "production");
const compatRoot = resolve(temporary, "compat");
const testRoot = resolve(temporary, "test");
const browserRoot = resolve(temporary, "browser");
const projectRootBuild = resolve(temporary, "project");
const productionCompiled = resolve(temporary, "production.js");
const compatCompiled = resolve(temporary, "compat.js");
const testCompiled = resolve(temporary, "test.js");
const browserCompiled = resolve(temporary, "browser.js");
const projectCompiled = resolve(temporary, "project.js");
const productionPackageCompiled = resolve(temporary, "production-package.js");
const projectVariant = process.env.VUELIL_PROJECT_VARIANT;
const projectOutput = resolve(
  projectRoot,
  process.env.VUELIL_PROJECT_VARIANT
    ? `packages/vuelil/production/${process.env.VUELIL_PROJECT_VARIANT}/runtime-core.js`
    : "packages/vuelil/runtime-core.project.generated.js",
);
const productionDirectory = resolve(projectRoot, "packages/vuelil/production");
const productionOutput = resolve(productionDirectory, "runtime-core.js");
const projectReactivityOutput = resolve(
  projectRoot,
  process.env.VUELIL_PROJECT_VARIANT
    ? `packages/vuelil/production/${process.env.VUELIL_PROJECT_VARIANT}/reactivity.js`
    : "packages/vuelil/reactivity.project.generated.js",
);
const projectSharedOutput = resolve(
  projectRoot,
  process.env.VUELIL_PROJECT_VARIANT
    ? `packages/vuelil/production/${process.env.VUELIL_PROJECT_VARIANT}/shared.js`
    : "packages/vuelil/shared.project.generated.js",
);
const projectCoreImports = Object.freeze({
  "./apiComputed": ["computed"],
  "./apiDefineComponent": ["defineComponent"],
  "./apiInject": ["inject", "provide"],
  "./component": ["registerRuntimeCompiler"],
  "./componentRenderContext": ["popScopeId", "pushScopeId"],
  "./components/BaseTransition": ["BaseTransition", "BaseTransitionPropsValidators"],
  "./errorHandling": ["ErrorCodes", "callWithAsyncErrorHandling"],
  "./h": ["h"],
  "./helpers/renderList": ["renderList"],
  "./helpers/resolveAssets": ["resolveComponent"],
  "./renderer": ["createRenderer"],
  "./scheduler": ["nextTick"],
  "./vnode": [
    "Fragment",
    "createBlock",
    "createElementBlock",
    "createElementVNode",
    "createTextVNode",
    "createVNode",
    "openBlock",
  ],
  "./warning": ["warn"],
  "./reactivity-bridge": ["reactive", "ref"],
  "../shared/index": [
    "camelize",
    "capitalize",
    "extend",
    "hyphenate",
    "includeBooleanAttr",
    "isArray",
    "isFunction",
    "isModelListener",
    "isObject",
    "isOn",
    "isSpecialBooleanAttr",
    "isString",
    "isSymbol",
    "normalizeClass",
    "toNumber",
  ],
  "../shared/toDisplayString": ["toDisplayString"],
});
const requestedProjectExports = process.env.VUELIL_PROJECT_EXPORTS
  ? new Set(process.env.VUELIL_PROJECT_EXPORTS.split(",").filter(Boolean))
  : null;

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
  "SchedulerJobFlags", "compatH", "createAppAPI", "createComponentInstance",
  "currentInstance", "deprecationData", "flushPostFlushCbs", "flushPreFlushCbs",
  "formatComponentName", "isBlockTreeEnabled", "isEmitListener", "normalizeVNode",
  "queueJob", "resetSuspenseId", "setCurrentInstance", "singletonApp", "SuspenseImpl",
  "setCurrentRenderingInstance", "toggleDeprecationWarning",
]);
const sharedSingletonExports = new Set([
  "camelize", "capitalize", "normalizeClass", "normalizeProps", "normalizeStyle",
  "toDisplayString", "toHandlerKey",
]);

const compatBuildFlags = Object.freeze({
  ...runtimeCoreTestFlags,
  __TEST__: false,
  __VERSION__: "3.5.42",
});
const browserBuildFlags = Object.freeze({
  ...runtimeCoreTestFlags,
  __TEST__: false,
  __VERSION__: "3.5.42",
  __BROWSER__: true,
  __GLOBAL__: true,
  __ESM_BUNDLER__: false,
  __CJS__: false,
  __SSR__: false,
  __COMPAT__: false,
});
const projectBuildFlags = Object.freeze({
  ...runtimeCoreTestFlags,
  __DEV__: false,
  __TEST__: false,
  __VERSION__: "3.5.42",
  __BROWSER__: true,
  __GLOBAL__: false,
  __ESM_BUNDLER__: true,
  __CJS__: false,
  __SSR__: true,
  __COMPAT__: false,
  __FEATURE_OPTIONS_API__: false,
  __FEATURE_SUSPENSE__: true,
  __FEATURE_PROD_DEVTOOLS__: false,
  __FEATURE_PROD_HYDRATION_MISMATCH_DETAILS__: false,
});

const ownerFlagSuffixes = [
  ["PROD_HYDRATION_MISMATCH_DETAILS", "__FEATURE_PROD_HYDRATION_MISMATCH_DETAILS__"],
  ["FEATURE_PROD_DEVTOOLS", "__FEATURE_PROD_DEVTOOLS__"],
  ["PROD_DEVTOOLS", "__FEATURE_PROD_DEVTOOLS__"],
  ["FEATURE_OPTIONS_API", "__FEATURE_OPTIONS_API__"],
  ["FEATURE_SUSPENSE", "__FEATURE_SUSPENSE__"],
  ["ESM_BUNDLER", "__ESM_BUNDLER__"],
  ["ESM_BROWSER", "__ESM_BROWSER__"],
  ["BROWSER", "__BROWSER__"],
  ["GLOBAL", "__GLOBAL__"],
  ["COMPAT", "__COMPAT__"],
  ["TEST", "__TEST__"],
  ["SSR", "__SSR__"],
  ["DEV", "__DEV__"],
];

function flagForOwnerConstant(name) {
  if (name === "SUSPENSE_FEATURE") return "__FEATURE_SUSPENSE__";
  for (const [suffix, flag] of ownerFlagSuffixes) {
    if (name === suffix || name.endsWith(`_${suffix}`)) return flag;
  }
  throw new Error(`unmapped runtime-core build constant: ${name}`);
}

function selectFlags(module, flags) {
  return module.replace(
    /^bool ([A-Z][A-Z0-9_]*) = (?:true|false);$/gm,
    (_statement, name) => `bool ${name} = ${flags[flagForOwnerConstant(name)]};`,
  );
}

function selectTestEntryFlags(module) {
  if (!module.includes('export string version = "3.5.42";')) {
    throw new Error("failed to select runtime-core test version");
  }
  let selected = module.replace(
    'export string version = "3.5.42";',
    `export string version = ${JSON.stringify(runtimeCoreTestFlags.__VERSION__)};`,
  );
  const productionCompatExports = [
    "export JsValue resolveFilter = null;",
    "export JsValue compatUtils = null;",
    "export JsValue DeprecationTypes = null;",
  ].join("\n");
  const testCompatExports = [
    "export JsValue resolveFilter = compatResolveFilter;",
    "export JsValue compatUtils = runtimeCompatUtils;",
    "export JsValue DeprecationTypes = runtimeDeprecationTypes;",
  ].join("\n");
  if (!selected.includes(productionCompatExports)) {
    throw new Error("failed to select runtime-core test compat exports");
  }
  selected = selected.replace(productionCompatExports, testCompatExports);
  return selected;
}

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

function withNamesAndExports(module, includeInternal, excludedExports = new Set()) {
  return module.replace(/export\{([^}]*)\}\s*$/, (_statement, exports) => {
    const entries = parseExports(exports).filter(
      ({ publicName }) => !excludedExports.has(renderedExportName(publicName)),
    );
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
        "compatUtils", "deprecationData", "devtools", "resolveFilter", "singletonApp",
        "ssrContextKey", "ssrUtils",
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

function prepareSourceGraph(root, testFlags = null) {
  const temporarySourceRoot = resolve(root, "src");
  const temporaryRuntimeCore = resolve(temporarySourceRoot, "runtime-core");
  const source = resolve(temporaryRuntimeCore, "index.lil");
  mkdirSync(temporarySourceRoot, { recursive: true });
  cpSync(sourceDirectory, temporaryRuntimeCore, { recursive: true });
  cpSync(sharedDirectory, resolve(temporarySourceRoot, "shared"), { recursive: true });
  if (testFlags === projectBuildFlags) {
    const suspensePath = resolve(temporaryRuntimeCore, "components/Suspense.lil");
    const suspenseSource = readFileSync(suspensePath, "utf8");
    const nestedQueue = `  if (suspense.truthy() && suspense["pendingBranch"].truthy()) {
    if (suspenseArray(effect)) {
      for (int index = 0;index < suspenseLength(effect);index++) {
        JS.invoke(suspense["effects"], "push", effect[index]);
      }
    } else {
      JS.invoke(suspense["effects"], "push", effect);
    }
  } else {
    queuePostFlushCb(effect);
  }`;
    const flattenedQueue = `  bool pending = suspense.truthy() && suspense["pendingBranch"].truthy();
  if (pending && suspenseArray(effect)) {
    for (int index = 0;index < suspenseLength(effect);index++) {
      JS.invoke(suspense["effects"], "push", effect[index]);
    }
  }
  if (pending && !suspenseArray(effect)) {
    JS.invoke(suspense["effects"], "push", effect);
  }
  if (!pending) queuePostFlushCb(effect);`;
    if (!suspenseSource.includes(nestedQueue)) {
      throw new Error("failed to select optimizer-safe production Suspense shape");
    }
    writeFileSync(suspensePath, suspenseSource.replace(nestedQueue, flattenedQueue));
  }
  const temporaryDependencies = resolve(root, "packages/vuelil");
  mkdirSync(temporaryDependencies, { recursive: true });
  cpSync(resolve(projectRoot, "packages/vuelil/reactivity.js"), resolve(temporaryDependencies, "reactivity.js"));
  cpSync(resolve(projectRoot, "packages/vuelil/shared.js"), resolve(temporaryDependencies, "shared.js"));

  const sharedGeneral = resolve(temporarySourceRoot, "shared/general.lil");
  const general = readFileSync(sharedGeneral, "utf8");
  const localNoop = "export void NOOP() {}";
  if (!general.includes(localNoop)) {
    throw new Error("failed to externalize runtime-core NOOP");
  }
  writeFileSync(sharedGeneral, general.replace(
    localNoop,
    [
      'import extern { NOOP as runtimeSharedNOOP } from "../../packages/vuelil/shared.js";',
      "extern JsValue runtimeSharedNOOP;",
      "export JsValue NOOP = runtimeSharedNOOP;",
    ].join("\n"),
  ));

  const sharedGlobals = new Set([
    "Array", "JSON", "Math", "Number", "Object", "String", "Uint8Array",
    "console", "global", "globalThis", "parseFloat", "parseInt", "process",
    "self", "window",
  ]);
  const runtimeGlobals = [
    "Boolean", "Date", "Element", "Error", "Function", "HTMLElement",
    "IntersectionObserver", "Map", "MathMLElement", "Promise", "Proxy",
    "Reflect", "RegExp", "SVGElement", "Set", "Symbol", "WeakMap", "WeakSet",
    "__COMPAT__", "__DEV__", "__FEATURE_OPTIONS_API__", "__FEATURE_PROD_DEVTOOLS__",
    "__FEATURE_PROD_HYDRATION_MISMATCH_DETAILS__", "__GLOBAL__", "clearTimeout", "document", "isNaN",
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
export bool DEV = ${testFlags?.__DEV__ ?? true};
export bool TEST = ${testFlags?.__TEST__ ?? true};
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
    let selected = testFlags
      ? selectFlags(original, testFlags)
      : original.replace(
          /^bool (COMPONENT_SSR|KEEP_ALIVE_SSR) = false;$/gmu,
          "bool $1 = true;",
        );
    if (testFlags?.__COMPAT__ && path === source) selected = selectTestEntryFlags(selected);
    const externalized = selected.replace(
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
  const globalFlags = testFlags ?? { __COMPAT__: false, __DEV__: true };
  writeFileSync(
    buildGlobals,
    `${runtimeGlobals.map(name => testFlags?.__TEST__ && name === "__DEV__"
      ? "export extern JsValue __DEV__;"
      : Object.hasOwn(globalFlags, name)
      ? `export JsValue ${name} = ${JSON.stringify(globalFlags[name])};`
      : `export extern JsValue ${name};`).join("\n")}\n`,
  );

  const barrel = readFileSync(source, "utf8");
  writeFileSync(source, [
    'import { createAppAPI as runtimeTestCreateAppAPI } from "./apiCreateApp";',
    'import { createComponentInstance as runtimeTestCreateComponentInstance, currentInstance as runtimeTestCurrentInstance, formatComponentName as runtimeTestFormatComponentName, setCurrentInstance as runtimeTestSetCurrentInstance } from "./component";',
    'import { isEmitListener as runtimeTestIsEmitListener } from "./componentEmits";',
    'import { resetSuspenseId as runtimeTestResetSuspenseId, SuspenseImpl as runtimeTestSuspenseImpl } from "./components/Suspense";',
    'import { setCurrentRenderingInstance as runtimeTestSetCurrentRenderingInstance } from "./componentRenderContext";',
    'import { deprecationData as runtimeTestDeprecationData, toggleDeprecationWarning as runtimeTestToggleDeprecationWarning } from "./compat/compatConfig";',
    'import { singletonApp as runtimeTestSingletonApp } from "./compat/global";',
    'import { compatH as runtimeTestCompatH } from "./compat/renderFn";',
    'import { isBlockTreeEnabled as runtimeTestIsBlockTreeEnabled } from "./vnode";',
    'import { SchedulerJobFlags as runtimeTestSchedulerJobFlags, flushPostFlushCbs as runtimeTestFlushPostFlushCbs, flushPreFlushCbs as runtimeTestFlushPreFlushCbs, queueJob as runtimeTestQueueJob } from "./scheduler";',
    barrel,
    "export {",
    "  runtimeTestCompatH as compatH,",
    "  runtimeTestCreateAppAPI as createAppAPI,",
    "  runtimeTestCreateComponentInstance as createComponentInstance,",
    "  runtimeTestCurrentInstance as currentInstance,",
    "  runtimeTestDeprecationData as deprecationData,",
    "  runtimeTestFlushPostFlushCbs as flushPostFlushCbs,",
    "  runtimeTestFlushPreFlushCbs as flushPreFlushCbs,",
    "  runtimeTestFormatComponentName as formatComponentName,",
    "  runtimeTestIsBlockTreeEnabled as isBlockTreeEnabled,",
    "  runtimeTestIsEmitListener as isEmitListener,",
    "  normalizeVNode,",
    "  runtimeTestQueueJob as queueJob,",
    "  runtimeTestResetSuspenseId as resetSuspenseId,",
    "  runtimeTestSchedulerJobFlags as SchedulerJobFlags,",
    "  runtimeTestSetCurrentInstance as setCurrentInstance,",
    "  runtimeTestSetCurrentRenderingInstance as setCurrentRenderingInstance,",
    "  runtimeTestSingletonApp as singletonApp,",
    "  runtimeTestSuspenseImpl as SuspenseImpl,",
    "  runtimeTestToggleDeprecationWarning as toggleDeprecationWarning",
    "};",
    "",
  ].join("\n"));
  return source;
}

function compile(source, compiled, production = false) {
  const args = [source, "--target", "js-module"];
  if (production) {
    args.push(
      "--mode",
      "production",
      "--config",
      resolve(projectRoot, "config/open-world.toml"),
      "--jobs",
      "1",
      "--codec-jobs",
      "1",
    );
  } else {
    args.push(
      "--config",
      resolve(repositoryRoot, "tests/config/no-optimization-no-peephole.toml"),
      "--jobs",
      "1",
      "--codec-jobs",
      "1",
    );
  }
  args.push("-o", compiled);
  const result = spawnSync(
    compilerPath(),
    args,
    { cwd: projectRoot, encoding: "utf8", env: process.env },
  );
  if (result.status !== 0) throw new Error(`${result.stdout ?? ""}${result.stderr ?? ""}`);
}

function prepareCompiledModule(compiled, dependencies) {
  const compiledSource = readFileSync(compiled, "utf8");
  const hostAliases = hostAliasDefinitions(compiledSource);
  const module = compiledSource
    .replace(
      /["'](?:\.\.\/){2,3}packages\/vuelil\/reactivity\.js["']/g,
      JSON.stringify(dependencies.reactivity),
    )
    .replace(
      /["'](?:\.\.\/){2,3}packages\/vuelil\/shared\.js["']/g,
      JSON.stringify(dependencies.shared),
    )
    .replace(/import\{[^;]*\}from["']\.\/host\.js["'];?/g, "");
  if (/from["']\.\/host\.js["']/.test(module)) {
    throw new Error("failed to inline the runtime-core host adapter");
  }
  if (/(?:\.\.\/){2,3}packages\/vuelil\/(?:reactivity|shared)\.js/.test(module)) {
    throw new Error("failed to rewrite a runtime-core dependency");
  }
  return { hostAliases, module };
}

function renderProjectEntry() {
  const required = requestedProjectExports
    ? new Set([
        ...requestedProjectExports,
        "BaseTransition",
        "BaseTransitionPropsValidators",
        "DeprecationTypes",
        "ErrorCodes",
        "callWithAsyncErrorHandling",
        "compatUtils",
        "createRenderer",
        "h",
        "warn",
      ])
    : null;
  const selectedImports = Object.entries(projectCoreImports)
    .map(([specifier, names]) => [
      specifier,
      required ? names.filter(name => required.has(name)) : names,
    ])
    .filter(([_specifier, names]) => names.length > 0);
  const imports = selectedImports.map(
    ([specifier, names]) => `import { ${names.join(", ")} } from ${JSON.stringify(specifier)};`,
  );
  const names = selectedImports.flatMap(([_specifier, bindings]) => bindings);
  return [
    ...imports,
    "export JsValue compatUtils = null;",
    "export JsValue DeprecationTypes = null;",
    `export { ${names.join(", ")} };`,
    "",
  ].join("\n");
}

try {
  const productionSource = projectVariant ? null : prepareSourceGraph(productionRoot);
  const compatSource = projectVariant ? null : prepareSourceGraph(compatRoot, compatBuildFlags);
  const testSource = projectVariant ? null : prepareSourceGraph(testRoot, runtimeCoreTestFlags);
  const browserSource = projectVariant ? null : prepareSourceGraph(browserRoot, browserBuildFlags);
  const projectSource = prepareSourceGraph(projectRootBuild, projectBuildFlags);
  const projectEntry = resolve(dirname(projectSource), "project-entry.lil");
  writeFileSync(projectEntry, renderProjectEntry());
  if (!projectVariant) {
    compile(productionSource, productionCompiled);
    compile(compatSource, compatCompiled);
    compile(testSource, testCompiled);
    compile(browserSource, browserCompiled);
  }
  compile(projectEntry, projectCompiled, true);
  if (!projectVariant) {
    const productionPackageExports = exportedNames(
      readFileSync(productionCompiled, "utf8"),
      false,
    );
    const productionPackageEntry = resolve(
      dirname(projectSource),
      "production-package-entry.lil",
    );
    writeFileSync(
      productionPackageEntry,
      `import { ${productionPackageExports.join(", ")} } from "./index";\n` +
        `export { ${productionPackageExports.join(", ")} };\n`,
    );
    compile(productionPackageEntry, productionPackageCompiled, true);
  }

  mkdirSync(resolve(projectRoot, "packages/vuelil"), { recursive: true });
  const hostModule = readFileSync(host, "utf8").replaceAll("export function ", "function ");
  const project = prepareCompiledModule(projectCompiled, {
    reactivity: projectVariant ? "./reactivity.js" : "./reactivity.project.generated.js",
    shared: projectVariant ? "./shared.js" : "./shared.project.generated.js",
  });

  const banner = "// Generated from the mirrored src/runtime-core/*.lil graph and its ECMAScript host adapter.\n";
  mkdirSync(dirname(projectOutput), { recursive: true });
  writeFileSync(
    projectReactivityOutput,
    readFileSync(resolve(projectRoot, "artifacts/reactivity.esm-browser.prod.js")),
  );
  writeFileSync(
    projectSharedOutput,
    readFileSync(resolve(projectRoot, "artifacts/shared-runtime.generated.js")),
  );
  writeFileSync(
    projectOutput,
    `${banner}${hostModule}\n${project.hostAliases}\n${withNamesAndExports(
      project.module,
      false,
    )}\n`,
  );
  if (!projectVariant) {
    const production = prepareCompiledModule(productionCompiled, {
      reactivity: "./reactivity.js",
      shared: "./shared.js",
    });
    const test = prepareCompiledModule(testCompiled, {
      reactivity: "../packages/vuelil/reactivity.js",
      shared: "../packages/vuelil/shared.js",
    });
    const compat = prepareCompiledModule(compatCompiled, {
      reactivity: "./reactivity.js",
      shared: "./shared.js",
    });
    const browser = prepareCompiledModule(browserCompiled, {
      reactivity: "../packages/vuelil/reactivity.js",
      shared: "../packages/vuelil/shared.js",
    });
    const productionPackage = prepareCompiledModule(productionPackageCompiled, {
      reactivity: "./reactivity.js",
      shared: "./shared.js",
    });
    const implementationModule = withNamesAndExports(
      production.module,
      true,
      sharedSingletonExports,
    );
    writeFileSync(
      implementationOutput,
      `${banner}${hostModule}\n${production.hostAliases}\n${implementationModule}\n` +
        `export { ${[...sharedSingletonExports].join(", ")} } from "./shared.js";\n`,
    );
    const publicExports = exportedNames(production.module, false);
    writeFileSync(
      output,
      `${banner}export { ${publicExports.join(", ")} } from "./runtime-core.internal.js";\n`,
    );
    const compatImplementationModule = withNamesAndExports(
      compat.module,
      true,
      sharedSingletonExports,
    );
    writeFileSync(
      compatImplementationOutput,
      `${banner}${hostModule}\n${compat.hostAliases}\n${compatImplementationModule}\n` +
        `export { ${[...sharedSingletonExports].join(", ")} } from "./shared.js";\n`,
    );
    const compatPublicExports = exportedNames(compat.module, false);
    writeFileSync(
      compatOutput,
      `${banner}export { ${compatPublicExports.join(", ")} } from "./runtime-core.compat.internal.js";\n`,
    );
    writeFileSync(
      upstreamCandidate,
      `${banner}${hostModule}\n${test.hostAliases}\n${withNamesAndExports(test.module, true)}\n`,
    );
    writeFileSync(
      browserCandidate,
      `${banner}${hostModule}\n${browser.hostAliases}\n${withNamesAndExports(browser.module, true)}\n`,
    );
    mkdirSync(productionDirectory, { recursive: true });
    writeFileSync(
      resolve(productionDirectory, "reactivity.js"),
      readFileSync(resolve(projectRoot, "artifacts/reactivity.esm-browser.prod.js")),
    );
    writeFileSync(
      resolve(productionDirectory, "shared.js"),
      readFileSync(resolve(projectRoot, "artifacts/shared-runtime.generated.js")),
    );
    writeFileSync(
      productionOutput,
      `${banner}${hostModule}\n${productionPackage.hostAliases}\n${withNamesAndExports(
        productionPackage.module,
        false,
        sharedSingletonExports,
      )}\nexport { ${[...sharedSingletonExports].join(", ")} } from "./shared.js";\n`,
    );

    const runtime = await import(`../packages/vuelil/runtime-core.js?build=${Date.now()}`);
    const shared = await import("../packages/vuelil/shared.js");
    for (const name of sharedSingletonExports) {
      if (runtime[name] !== shared[name]) {
        throw new Error(`runtime-core did not preserve ${name} shared singleton identity`);
      }
    }
    console.log(JSON.stringify({
      output,
      compatOutput,
      upstreamCandidate,
      browserCandidate,
      projectOutput,
      productionOutput,
      exports: Object.keys(runtime).sort(),
      bytes: readFileSync(output).byteLength,
    }));
  } else {
    console.log(JSON.stringify({ projectOutput, exports: [...requestedProjectExports] }));
  }
} finally {
  rmSync(temporary, { force: true, recursive: true });
}
