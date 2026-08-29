import assert from "node:assert/strict";
import { before, describe, test } from "node:test";
import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const require = createRequire(import.meta.url);
let candidate;
let production;
let oracle;

before(async () => {
  const build = spawnSync(process.execPath, [resolve(root, "scripts/build-reactivity.mjs")], {
    cwd: root,
    encoding: "utf8",
    env: process.env,
  });
  assert.equal(build.status, 0, `${build.stdout}${build.stderr}`);
  candidate = await import(`../packages/vuelil/reactivity.js?test=${Date.now()}`);
  production = await import(`../artifacts/reactivity.esm-browser.prod.js?test=${Date.now()}`);
  oracle = await import(process.env.VUE_REACTIVITY_ORACLE ?? "@vue/reactivity");
  assert.equal(
    require("@vue/reactivity/package.json").version,
    "3.5.42",
    "differential oracle must stay pinned",
  );
});

function exercise(api) {
  const events = [];
  const raw = { count: 0, nested: { value: 1 }, item: api.ref(2) };
  const state = api.reactive(raw);
  const doubled = api.computed(previous => {
    events.push(["computed", previous]);
    return state.count * 2;
  });
  const runner = api.effect(() => {
    events.push(["effect", state.count, state.nested.value, doubled.value, state.item]);
    return state.count;
  });
  state.count = 1;
  state.nested.value = 3;
  state.item = 5;
  api.stop(runner);
  state.count = 2;
  const manual = runner();
  state.count = 3;
  return {
    events,
    manual,
    rawItem: raw.item.value,
    identities: [
      api.reactive(raw) === state,
      api.reactive(state) === state,
      api.toRaw(state) === raw,
      api.isReactive(state),
      api.isReadonly(state),
      api.isProxy(state),
      api.isReactive(state.nested),
      api.isRef(state.item),
    ],
  };
}

function assertDevelopmentAndProduction(scenario) {
  const expected = scenario(oracle);
  assert.deepEqual(scenario(candidate), expected);
  assert.deepEqual(scenario(production), expected);
}

describe("Vue 3.5.42 reactivity kernel milestone", () => {
  test("ships only the implemented surface with matching function arities", () => {
    assert.deepEqual(Object.keys(candidate).sort(), [
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
    ]);
    assert.deepEqual(Object.keys(production).sort(), Object.keys(candidate).sort());
    for (const name of Object.keys(candidate)) {
      assert.equal(candidate[name].length, oracle[name].length, `${name}.length`);
      assert.equal(production[name].length, oracle[name].length, `production ${name}.length`);
      if (typeof candidate[name] === "function") {
        assert.equal(candidate[name].name, oracle[name].name, `${name}.name`);
        assert.equal(production[name].name, oracle[name].name, `production ${name}.name`);
      }
    }
    const artifact = readFileSync(resolve(root, "packages/vuelil/reactivity.js"), "utf8");
    assert.doesNotMatch(artifact, /(?:from|import\()\s*["'](?:vue|@vue\/)/);
  });

  test("matches refs, dynamic effects, computed caching, stop, and identities", () => {
    assertDevelopmentAndProduction(exercise);
  });

  test("matches ref and shallowRef conversion and force triggering", () => {
    function scenario(api) {
      const raw = { value: 1 };
      const deep = api.ref(raw);
      const shallow = api.shallowRef(raw);
      let deepRuns = 0;
      let shallowRuns = 0;
      api.effect(() => {
        deepRuns++;
        deep.value.value;
      });
      api.effect(() => {
        shallowRuns++;
        shallow.value.value;
      });
      deep.value.value++;
      shallow.value.value++;
      api.triggerRef(shallow);
      deep.value = api.reactive(raw);
      return {
        deepRuns,
        shallowRuns,
        deepReactive: api.isReactive(deep.value),
        shallowReactive: api.isReactive(shallow.value),
        shallowFlag: api.isShallow(shallow),
        idempotent: api.ref(deep) === deep,
        unref: api.unref(deep).value,
        nanStable: (() => {
          const value = api.ref(Number.NaN);
          let runs = 0;
          api.effect(() => { runs++; value.value; });
          value.value = Number.NaN;
          return runs;
        })(),
        signedZero: (() => {
          const value = api.ref(-0);
          let runs = 0;
          api.effect(() => { runs++; value.value; });
          value.value = 0;
          return runs;
        })(),
      };
    }
    assertDevelopmentAndProduction(scenario);
  });

  test("matches deep and shallow mutable/readonly proxy behavior", () => {
    function scenario(api) {
      const raw = { nested: { n: 1 }, value: api.ref(2) };
      const deep = api.reactive(raw);
      const shallow = api.shallowReactive({ nested: { n: 1 }, value: api.ref(2) });
      const ro = api.readonly(deep);
      const shallowRo = api.shallowReadonly({ nested: { n: 1 }, value: api.ref(2) });
      const oldWarn = console.warn;
      console.warn = () => {};
      try {
        ro.nested.n = 9;
        shallowRo.nested.n = 4;
        shallowRo.value = 7;
      } finally {
        console.warn = oldWarn;
      }
      return {
        deepRef: deep.value,
        shallowRefPreserved: api.isRef(shallow.value),
        nested: [api.isReactive(deep.nested), api.isReactive(shallow.nested)],
        readonly: [api.isReadonly(ro), api.isReactive(ro), api.isProxy(ro)],
        shallowReadonly: [api.isReadonly(shallowRo), api.isShallow(shallowRo)],
        mutations: [raw.nested.n, shallowRo.nested.n, api.isRef(shallowRo.value)],
        raw: api.toRaw(ro) === raw,
      };
    }
    assertDevelopmentAndProduction(scenario);
  });

  test("matches array index, length, iteration, search, and mutation tracking", () => {
    function scenario(api) {
      const rawItem = {};
      const list = api.reactive([rawItem]);
      const snapshots = [];
      api.effect(() => snapshots.push(`${list.length}:${list.join("|")}:${Object.keys(list).length}`));
      list.push("a");
      list[3] = "b";
      list.length = 1;
      const proxiedItem = list[0];
      return {
        snapshots,
        includesRaw: list.includes(rawItem),
        includesProxy: list.includes(proxiedItem),
        indexRaw: list.indexOf(rawItem),
        nestedReactive: api.isReactive(proxiedItem),
      };
    }
    assertDevelopmentAndProduction(scenario);
  });

  test("matches scheduler, onStop, manual runner, and branch cleanup", () => {
    function scenario(api) {
      const state = api.reactive({ enabled: true, left: 1, right: 2 });
      const events = [];
      let scheduled = 0;
      const runner = api.effect(
        () => events.push(state.enabled ? state.left : state.right),
        { scheduler: () => scheduled++, onStop: () => events.push("stopped") },
      );
      state.left++;
      const beforeManual = events.slice();
      runner();
      state.enabled = false;
      runner();
      state.left++;
      state.right++;
      api.stop(runner);
      api.stop(runner);
      return { beforeManual, events, scheduled };
    }
    assertDevelopmentAndProduction(scenario);
  });

  test("matches inherited raw identity, hasOwnProperty, nested effects, and effect runners", () => {
    function scenario(api) {
      const state = api.reactive({ outer: 1, inner: 2, present: true });
      const inherited = Object.create(state);
      const runs = [];
      const child = api.effect(() => runs.push(["child", state.inner]));
      api.effect(() => {
        runs.push(["parent", state.outer, state.hasOwnProperty("present")]);
        child();
      });
      const rerun = api.effect(child);
      state.inner++;
      delete state.present;
      state.outer++;
      api.stop(rerun);
      return {
        runs,
        inheritedRaw: api.toRaw(inherited) === inherited,
        proxyRaw: api.toRaw(new Proxy(state, {})) === api.toRaw(state),
      };
    }
    assertDevelopmentAndProduction(scenario);
  });

  test("matches array mutators used from effects without self-recursion", () => {
    function scenario(api) {
      const pushes = api.reactive([]);
      let pushA = 0;
      let pushB = 0;
      api.effect(() => { pushA++; pushes.push(1); });
      api.effect(() => { pushB++; pushes.push(2); });

      const removals = api.reactive([1, 2, 3, 4]);
      let popA = 0;
      let popB = 0;
      api.effect(() => { popA++; removals.pop(); });
      api.effect(() => { popB++; removals.shift(); });
      return { pushes: [...pushes], removals: [...removals], runs: [pushA, pushB, popA, popB] };
    }
    assertDevelopmentAndProduction(scenario);
  });

  test("matches runner effect controls used by Vue consumers", () => {
    function scenario(api) {
      const source = api.ref(0);
      const values = [];
      const runner = api.effect(() => values.push(source.value));
      runner.effect.pause();
      source.value = 1;
      source.value = 2;
      const paused = values.slice();
      runner.effect.resume();
      const flagsBeforeStop = runner.effect.flags;
      runner.effect.stop();
      source.value = 3;
      runner.effect.run();
      return { values, paused, flagsBeforeStop, flagsAfterStop: runner.effect.flags };
    }
    assertDevelopmentAndProduction(scenario);
  });

  test("matches lazy, chained, unchanged, and writable computed refs", () => {
    function scenario(api) {
      const source = api.ref(1);
      let firstCalls = 0;
      let secondCalls = 0;
      const parity = api.computed(previous => {
        firstCalls++;
        return source.value % 2;
      });
      const chained = api.computed(() => {
        secondCalls++;
        return parity.value + 10;
      });
      const observed = [];
      api.effect(() => observed.push(chained.value));
      source.value = 3;
      source.value = 4;
      const writable = api.computed({
        get: () => source.value + 1,
        set: value => { source.value = value - 1; },
      });
      writable.value = 10;
      return {
        observed,
        calls: [firstCalls, secondCalls],
        writable: [writable.value, source.value],
        flags: [api.isRef(chained), api.isReadonly(chained), api.isReadonly(writable)],
      };
    }
    assertDevelopmentAndProduction(scenario);
  });

  test("markRaw and non-extensible common targets retain identity", () => {
    for (const api of [candidate, production, oracle]) {
      const marked = api.markRaw({ value: 1 });
      const frozen = Object.freeze({ value: 1 });
      assert.equal(api.reactive(marked), marked);
      assert.equal(api.readonly(frozen), frozen);
      assert.equal(api.isProxy(marked), false);
    }
  });
});
