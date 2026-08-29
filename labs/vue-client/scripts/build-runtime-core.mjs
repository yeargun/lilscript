import { spawnSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { compilerPath, projectRoot, repositoryRoot } from "../tooling/compiler-path.mjs";

const source = resolve(projectRoot, "src/runtime-core/index.lil");
const host = resolve(projectRoot, "src/runtime-core/host.js");
const output = resolve(projectRoot, "packages/vuelil/runtime-core.js");
const upstreamCandidate = resolve(projectRoot, "tests/runtime-core-upstream.candidate.mjs");
const temporary = mkdtempSync(resolve(tmpdir(), "vuelil-runtime-core-"));
const compiled = resolve(temporary, "index.js");

const dependencyExports = {
  reactivity: [
    "EffectScope", "ReactiveEffect", "TrackOpTypes", "TriggerOpTypes", "customRef",
    "effect", "effectScope", "getCurrentScope", "getCurrentWatcher", "isProxy",
    "isReactive", "isReadonly", "isRef", "isShallow", "markRaw", "onScopeDispose",
    "onWatcherCleanup", "proxyRefs", "reactive", "readonly", "ref", "shallowReactive",
    "shallowReadonly", "shallowRef", "stop", "toRaw", "toRef", "toRefs", "toValue",
    "triggerRef", "unref",
  ],
  shared: [
    "camelize", "capitalize", "normalizeClass", "normalizeProps", "normalizeStyle",
    "toDisplayString", "toHandlerKey",
  ],
};

const publicCompiledExports = new Set([
  "Comment", "Fragment", "Static", "Text", "cloneVNode", "computed", "createBlock",
  "createCommentVNode", "createElementBlock", "createElementVNode", "createStaticVNode",
  "createTextVNode", "createVNode", "createRenderer", "defineComponent", "getCurrentInstance",
  "guardReactiveProps", "hasInjectionContext", "inject", "isMemoSame", "isVNode", "mergeProps",
  "nextTick", "onBeforeMount", "onBeforeUnmount", "onBeforeUpdate", "onErrorCaptured",
  "onMounted", "onRenderTracked", "onRenderTriggered", "onServerPrefetch", "onUnmounted",
  "onUpdated", "openBlock", "popScopeId", "provide", "pushScopeId", "queuePostFlushCb",
  "renderHyperscript", "renderList", "resolveComponent", "resolveDirective",
  "resolveDynamicComponent", "setBlockTracking", "toHandlers", "transformVNodeArgs",
  "version", "watch", "watchEffect", "watchPostEffect", "watchSyncEffect", "withCtx",
  "withDirectives", "withMemo", "withScopeId",
]);

const anonymousFunctions = new Set([
  "onBeforeMount", "onBeforeUnmount", "onBeforeUpdate", "onMounted", "onRenderTracked",
  "onRenderTriggered", "onServerPrefetch", "onUnmounted", "onUpdated",
]);

const functionLengths = new Map([
  ["createBaseVNode", 1], ["createCommentVNode", 0], ["createElementVNode", 1],
  ["createTextVNode", 0], ["createVNode", 0], ["onErrorCaptured", 1], ["withCtx", 1],
  ["resolveComponent", 2],
]);

function parseExports(exports) {
  return exports.split(",").map((entry) => {
    const [local, publicName = local] = entry.trim().split(/\s+as\s+/);
    return { local, publicName };
  });
}

function withNamesAndExports(module, keepPublicOnly) {
  return module.replace(/export\{([^}]*)\}\s*$/, (_statement, exports) => {
    const entries = parseExports(exports);
    const definitions = [];
    const namedLocals = new Set();
    for (const { local, publicName } of entries) {
      if (namedLocals.has(local)) continue;
      let name = publicName;
      if (publicName === "renderHyperscript") name = "h";
      if (publicName === "createVNode") name = "createVNodeWithArgsTransform";
      if (publicName === "warn") name = "warn$1";
      if (anonymousFunctions.has(publicName)) name = "";
      if (/^[A-Z]/.test(publicName) || [
        "version", "currentBlock", "isBlockTreeEnabled", "currentRenderingInstance",
        "currentScopeId", "currentInstance", "currentApp", "isInSSRComponentSetup",
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
    const kept = entries.filter(({ publicName }) =>
      !keepPublicOnly || publicCompiledExports.has(publicName),
    );
    const rendered = kept.map(({ local, publicName }) => {
      const exported = publicName === "renderHyperscript" ? "h" : publicName;
      return local === exported ? local : `${local} as ${exported}`;
    });
    return `${definitions.join("")}export{${rendered.join(",")}}`;
  });
}

function dependencyStatements(prefix) {
  return Object.entries(dependencyExports).map(([module, names]) => {
    const imports = names.map(name => `${name} as runtimeCoreDependency_${name}`).join(",");
    const definitions = module === "shared"
      ? names.map(name => {
          const functionName = ["camelize", "capitalize", "toHandlerKey"].includes(name) ? "" : name;
          return `Object.defineProperty(runtimeCoreDependency_${name},"name",{configurable:true,value:${JSON.stringify(functionName)}});`;
        }).join("")
      : "";
    const exports = names.map(name => `runtimeCoreDependency_${name} as ${name}`).join(",");
    return `import{${imports}}from${JSON.stringify(`${prefix}${module}.js`)};${definitions}export{${exports}};`;
  }).join("");
}

try {
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
  let compiledModule = readFileSync(compiled, "utf8")
    .replaceAll('"../../packages/vuelil/reactivity.js"', '"./reactivity.js"')
    .replaceAll('"../../packages/vuelil/shared.js"', '"./shared.js"')
    .replace(/import\{[^;]*\}from["']\.\/host\.js["'];?/, "");
  if (/from["']\.\/host\.js["']/.test(compiledModule)) {
    throw new Error("failed to inline the runtime-core host adapter");
  }

  const banner = "// Generated from src/runtime-core/index.lil and its primitive ECMAScript host adapter.\n";
  const publicModule = withNamesAndExports(compiledModule, true);
  writeFileSync(
    output,
    `${banner}${hostModule}\n${publicModule}\n${dependencyStatements("./")}\n`,
  );

  const testModule = withNamesAndExports(compiledModule, false)
    .replaceAll('"./reactivity.js"', '"../packages/vuelil/reactivity.js"')
    .replaceAll('"./shared.js"', '"../packages/vuelil/shared.js"');
  writeFileSync(
    upstreamCandidate,
    `${banner}${hostModule}\n${testModule}\n${dependencyStatements("../packages/vuelil/")}\n`,
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
