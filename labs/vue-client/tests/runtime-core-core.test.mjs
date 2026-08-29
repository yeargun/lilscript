import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { resolve } from "node:path";
import { before, describe, test } from "node:test";

const root = resolve(import.meta.dirname, "..");
const require = createRequire(import.meta.url);
let candidate;
let internal;
let oracle;

before(async () => {
  const build = spawnSync(process.execPath, [resolve(root, "scripts/build-runtime-core.mjs")], {
    cwd: root,
    encoding: "utf8",
    env: process.env,
  });
  assert.equal(build.status, 0, `${build.stdout}${build.stderr}`);
  const stamp = Date.now();
  candidate = await import(`../packages/vuelil/runtime-core.js?test=${stamp}`);
  internal = await import(`./runtime-core-upstream.candidate.mjs?test=${stamp}`);
  oracle = await import("@vue/runtime-core");
  assert.equal(require("@vue/runtime-core/package.json").version, "3.5.42");
});

function serializable(value) {
  if (typeof value === "symbol") return String(value);
  if (typeof value === "function") return `[function ${value.name}/${value.length}]`;
  if (Array.isArray(value)) return value.map(serializable);
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.keys(value).map(key => [key, serializable(value[key])]));
  }
  return value;
}

function summarizeVNode(vnode) {
  return {
    keys: Object.keys(vnode),
    type: typeof vnode.type === "symbol" ? String(vnode.type) : vnode.type,
    props: serializable(vnode.props),
    key: vnode.key,
    children: Array.isArray(vnode.children)
      ? vnode.children.map(child => child?.__v_isVNode ? summarizeVNode(child) : child)
      : vnode.children,
    shapeFlag: vnode.shapeFlag,
    patchFlag: vnode.patchFlag,
    dynamicProps: vnode.dynamicProps,
    dynamicChildren: vnode.dynamicChildren?.map(child => child.type) ?? vnode.dynamicChildren,
    staticCount: vnode.staticCount,
  };
}

describe("Vue 3.5.42 runtime-core tranche", () => {
  test("exports only implemented public bindings with matching reflection", () => {
    const compatibility = JSON.parse(
      readFileSync(resolve(root, "compatibility/runtime-core.json"), "utf8"),
    );
    const expected = [
      "Comment", "EffectScope", "Fragment", "ReactiveEffect", "Static", "Text",
      "TrackOpTypes", "TriggerOpTypes", "camelize", "capitalize", "cloneVNode",
      "computed", "createBlock", "createCommentVNode", "createElementBlock",
      "createElementVNode", "createRenderer", "createStaticVNode", "createTextVNode", "createVNode",
      "customRef", "defineComponent", "effect", "effectScope", "getCurrentInstance",
      "getCurrentScope", "getCurrentWatcher", "guardReactiveProps", "h",
      "hasInjectionContext", "inject", "isMemoSame", "isProxy", "isReactive",
      "isReadonly", "isRef", "isShallow", "isVNode", "markRaw", "mergeProps",
      "nextTick", "normalizeClass", "normalizeProps", "normalizeStyle", "onBeforeMount",
      "onBeforeUnmount", "onBeforeUpdate", "onErrorCaptured", "onMounted",
      "onRenderTracked", "onRenderTriggered", "onScopeDispose", "onServerPrefetch",
      "onUnmounted", "onUpdated", "onWatcherCleanup", "openBlock", "popScopeId",
      "provide", "proxyRefs", "pushScopeId", "queuePostFlushCb", "reactive", "readonly",
      "ref", "renderList", "resolveComponent", "resolveDirective", "resolveDynamicComponent",
      "setBlockTracking", "shallowReactive", "shallowReadonly",
      "shallowRef", "stop", "toDisplayString", "toHandlerKey", "toHandlers", "toRaw",
      "toRef", "toRefs", "toValue", "transformVNodeArgs", "triggerRef", "unref",
      "version", "watch", "watchEffect", "watchPostEffect", "watchSyncEffect", "withCtx",
      "withDirectives", "withMemo", "withScopeId",
    ];
    assert.deepEqual(Object.keys(candidate).sort(), expected.sort());
    assert.deepEqual(compatibility.runtimeExports, Object.keys(candidate).sort());
    assert.deepEqual(
      compatibility.unsupportedRuntimeExports,
      Object.keys(oracle).filter(name =>
        name !== "default" && name !== "__esModule" && name !== "module.exports" && !(name in candidate),
      ).sort(),
    );
    for (const name of expected) {
      assert.ok(name in oracle, `${name} exists in the oracle`);
      const actualDescriptor = Object.getOwnPropertyDescriptor(candidate, name);
      const oracleDescriptor = Object.getOwnPropertyDescriptor(oracle, name);
      assert.deepEqual(
        {
          enumerable: actualDescriptor.enumerable,
          configurable: actualDescriptor.configurable,
          writable: actualDescriptor.writable,
        },
        {
          enumerable: oracleDescriptor.enumerable,
          configurable: oracleDescriptor.configurable,
          writable: oracleDescriptor.writable,
        },
        `${name} export descriptor`,
      );
      if (typeof candidate[name] === "function") {
        assert.equal(candidate[name].length, oracle[name].length, `${name}.length`);
        assert.equal(candidate[name].name, oracle[name].name, `${name}.name`);
      }
    }
    assert.equal(candidate.Fragment, oracle.Fragment);
    assert.equal(candidate.Text, oracle.Text);
    assert.equal(candidate.Comment, oracle.Comment);
    assert.equal(candidate.Static, oracle.Static);
    const artifact = readFileSync(resolve(root, "packages/vuelil/runtime-core.js"), "utf8");
    assert.doesNotMatch(artifact, /(?:from|import\()\s*["'](?:vue|@vue\/)/);
  });

  test("matches VNode creation, cloning, normalization, props, and h", () => {
    function scenario(api) {
      const click = () => {};
      const vnode = api.createVNode("div", {
        id: "root",
        class: ["one", { two: true }],
        style: [{ color: "red" }, { width: "2px" }],
        onClick: click,
      }, "hello", 1, ["id"]);
      const clone = api.cloneVNode(vnode, {
        key: 0,
        class: { three: true },
        style: "height: 3px",
        onClick: () => {},
      });
      const child = api.h("span", null, "child");
      return {
        vnode: summarizeVNode(vnode),
        clone: summarizeVNode(clone),
        h: summarizeVNode(api.h("main", { class: ["a", "b"] }, child)),
        merged: serializable(api.mergeProps(
          { class: { a: true }, style: "width: 1px", onClick: click },
          { class: ["b"], style: { color: "blue" }, onClick: click, title: "ok" },
        )),
      };
    }
    assert.deepEqual(scenario(candidate), scenario(oracle));

    const normalized = [null, false, ["x"], 2].map(internal.normalizeVNode);
    assert.deepEqual(normalized.map(vnode => String(vnode.type)), [
      "Symbol(v-cmt)", "Symbol(v-cmt)", "Symbol(v-fgt)", "Symbol(v-txt)",
    ]);
    assert.equal(normalized[3].children, "2");
  });

  test("matches block collection and memo behavior", () => {
    function blockScenario(api) {
      const hoist = api.createVNode("i");
      let dynamic;
      const rootVNode = (
        api.openBlock(),
        api.createBlock("div", null, [
          hoist,
          (dynamic = api.createVNode("b", null, "x", 1)),
        ])
      );
      const cache = [];
      let renders = 0;
      const first = api.withMemo([Number.NaN, 1], () => {
        renders++;
        return api.createVNode("p");
      }, cache, 0);
      const second = api.withMemo([Number.NaN, 1], () => {
        renders++;
        return api.createVNode("p");
      }, cache, 0);
      return {
        dynamicTypes: rootVNode.dynamicChildren.map(child => child.type),
        trackedIdentity: rootVNode.dynamicChildren[0] === dynamic,
        memoIdentity: first === second,
        memo: first.memo,
        cacheIndex: first.cacheIndex,
        renders,
      };
    }
    assert.deepEqual(blockScenario(candidate), blockScenario(oracle));
  });

  test("matches renderList and toHandlers across supported inputs", () => {
    function scenario(api) {
      const cache = [];
      const array = api.renderList(["a", "b"], (value, index, unused, old) => [
        value, index, unused, old,
      ], cache, 0);
      const object = api.renderList({ a: 1, b: 2 }, (value, key, index) => [
        value, key, index,
      ]);
      const iterable = api.renderList(new Set([3, 4]), (value, index) => [value, index]);
      const range = api.renderList(3, (value, index) => [value, index]);
      const handlers = api.toHandlers({ click: 1, camelCase: 2 }, true);
      return { array, object, iterable, range, handlers, cache };
    }
    assert.deepEqual(scenario(candidate), scenario(oracle));

    for (const make of [api => api.reactive([{ n: 1 }]), api => api.shallowReactive([{ n: 1 }])]) {
      assert.deepEqual(
        candidate.renderList(make(candidate), candidate.isReactive),
        oracle.renderList(make(oracle), oracle.isReactive),
      );
    }
  });

  test("matches nextTick callback values and receiver binding", async () => {
    async function scenario(api) {
      const receiver = { value: 7 };
      const order = [];
      const result = api.nextTick.call(receiver, function () {
        order.push("tick");
        return this.value;
      });
      order.push("sync");
      return { orderBefore: order.slice(), value: await result, order };
    }
    assert.deepEqual(await scenario(candidate), await scenario(oracle));
  });

  test("matches renderer keyed updates and reactive component effects", async () => {
    async function scenario(api) {
      const operations = [];
      const node = (type, value = "") => ({ type, value, parent: null, children: [], props: {} });
      const renderer = api.createRenderer({
        createElement: tag => node("element", tag),
        createText: text => node("text", text),
        createComment: text => node("comment", text),
        insert(child, parent, anchor) {
          if (child.parent) child.parent.children.splice(child.parent.children.indexOf(child), 1);
          const index = anchor ? parent.children.indexOf(anchor) : -1;
          if (index < 0) parent.children.push(child);
          else parent.children.splice(index, 0, child);
          child.parent = parent;
          operations.push(["insert", child.type, child.value]);
        },
        remove(child) {
          if (child.parent) child.parent.children.splice(child.parent.children.indexOf(child), 1);
          child.parent = null;
          operations.push(["remove", child.type, child.value]);
        },
        setText(child, text) { child.value = text; },
        setElementText(child, text) {
          child.children = text ? [Object.assign(node("text", text), { parent: child })] : [];
        },
        parentNode: child => child.parent,
        nextSibling(child) {
          if (!child.parent) return null;
          return child.parent.children[child.parent.children.indexOf(child) + 1] ?? null;
        },
        patchProp(child, key, _previous, value) { child.props[key] = value; },
      });
      const root = node("root");
      renderer.render(api.h("div", [
        api.h("i", { key: 1 }, "one"),
        api.h("i", { key: 2 }, "two"),
      ]), root);
      renderer.render(api.h("div", [
        api.h("i", { key: 2 }, "two"),
        api.h("b", { key: 3 }, "three"),
        api.h("i", { key: 1 }, "one"),
      ]), root);
      const count = api.ref(0);
      renderer.render(api.h({ setup: () => () => api.h("p", String(count.value)) }), root);
      count.value++;
      await api.nextTick();
      function serialize(current) {
        if (current.type === "text") return current.value;
        return `<${current.value}>${current.children.map(serialize).join("")}</${current.value}>`;
      }
      return {
        html: root.children.map(serialize).join(""),
        operationKinds: operations.map(operation => operation[0]),
      };
    }
    assert.deepEqual(await scenario(candidate), await scenario(oracle));
  });

  test("matches app registration, plugins, config, and injection context", () => {
    const renderer = oracle.createRenderer({
      insert() {}, remove() {}, createElement: () => ({}), createText: () => ({}),
      createComment: () => ({}), setText() {}, setElementText() {}, parentNode() {},
      nextSibling() {}, patchProp() {}, setScopeId() {},
      insertStaticContent: () => [{}, {}],
    });
    function scenario(api, createApp) {
      const calls = [];
      const app = createApp({ name: "Root" });
      const component = {};
      const directive = {};
      function plugin(target, value) {
        calls.push([target === app, value]);
        target.provide("plugin", value);
      }
      const chain = app.use(plugin, 2).mixin({ name: "mix" })
        .component("Widget", component).directive("focus", directive)
        .provide("plain", 1);
      return {
        chain: chain === app,
        calls,
        component: app.component("Widget") === component,
        directive: app.directive("focus") === directive,
        injection: app.runWithContext(() => [api.inject("plugin"), api.inject("plain")]),
        context: {
          configKeys: Object.keys(app.config),
          mixins: app._context.mixins.length,
          nullProvides: Object.getPrototypeOf(app._context.provides) === null,
          cacheKinds: [app._context.optionsCache, app._context.propsCache, app._context.emitsCache]
            .map(value => value instanceof WeakMap),
        },
        methods: ["use", "mixin", "component", "directive", "mount", "onUnmount", "unmount", "provide", "runWithContext"]
          .map(name => [name, app[name].name, app[name].length]),
        configAccessor: (() => {
          const descriptor = Object.getOwnPropertyDescriptor(app, "config");
          return [descriptor.enumerable, descriptor.configurable, descriptor.get.name, descriptor.set.name];
        })(),
      };
    }
    assert.deepEqual(
      scenario(internal, internal.createAppAPI(() => {})),
      scenario(oracle, renderer.createApp),
    );
  });

  test("registers lifecycle hooks with current-instance and tracking isolation", () => {
    const scope = candidate.effectScope(true);
    const target = {
      scope,
      parent: null,
      provides: Object.create(null),
      vnode: { appContext: { provides: Object.assign(Object.create(null), { inherited: 1 }) } },
      proxy: { marker: "proxy" },
      isMounted: false,
      m: null,
    };
    const reset = internal.setCurrentInstance(target);
    const key = Symbol("key");
    internal.provide(key, 2);
    assert.equal(internal.inject("inherited"), 1);
    assert.equal(internal.inject(key, "missing"), "missing");
    assert.equal(internal.inject("factory", function () { return this.marker; }, true), "proxy");
    assert.equal(internal.hasInjectionContext(), true);

    const source = candidate.ref(0);
    let hookRuns = 0;
    let sawCurrent = false;
    internal.onMounted(() => {
      hookRuns++;
      source.value;
      sawCurrent = internal.getCurrentInstance() === target;
    });
    reset();
    assert.equal(internal.hasInjectionContext(), false);
    let effectRuns = 0;
    const runner = candidate.effect(() => {
      effectRuns++;
      target.m[0]();
    });
    source.value++;
    candidate.stop(runner);
    assert.deepEqual({ hookRuns, effectRuns, sawCurrent }, { hookRuns: 1, effectRuns: 1, sawCurrent: true });
  });
});
