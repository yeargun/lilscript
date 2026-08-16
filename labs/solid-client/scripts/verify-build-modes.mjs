import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { build } from "vite";
import {
  canonicalCodecProvenance,
  canonicalCodecSizes,
} from "../../../benchmarks/codec-contract.mjs";
import { compilerPath } from "../tooling/compiler-path.mjs";
import { selectSolidLilDistribution } from "./distribution-selection.mjs";
import { root } from "./project.mjs";

const generated = resolve(root, "artifacts/generated");
const packageRoot = resolve(root, "packages/solidlil");
const source = resolve(root, "apps/lilscript/src/reactive.lil");
const openConfig = resolve(root, "config/open-world.toml");
const closedConfig = resolve(root, "config/closed-world.toml");
const selectedCompiler = compilerPath();
const publicNames = [
  "$DEVCOMP",
  "$PROXY",
  "$TRACK",
  "DEV",
  "ErrorBoundary",
  "For",
  "Index",
  "Match",
  "Show",
  "Suspense",
  "SuspenseList",
  "Switch",
  "batch",
  "cancelCallback",
  "catchError",
  "children",
  "createComponent",
  "createComputed",
  "createContext",
  "createDeferred",
  "createEffect",
  "createMemo",
  "createReaction",
  "createRenderEffect",
  "createResource",
  "createRoot",
  "createSelector",
  "createSignal",
  "createUniqueId",
  "enableExternalSource",
  "enableHydration",
  "enableScheduling",
  "equalFn",
  "from",
  "getListener",
  "getOwner",
  "indexArray",
  "lazy",
  "mapArray",
  "mergeProps",
  "observable",
  "on",
  "onCleanup",
  "onError",
  "onMount",
  "requestCallback",
  "resetErrorBoundaries",
  "runWithOwner",
  "sharedConfig",
  "splitProps",
  "startTransition",
  "untrack",
  "useContext",
  "useTransition",
].sort();

mkdirSync(generated, { recursive: true });
mkdirSync(packageRoot, { recursive: true });

function run(program, args) {
  const result = spawnSync(program, args, {
    cwd: root,
    encoding: "utf8",
    env: process.env,
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(
      `${program} ${args.join(" ")}\n${result.stdout ?? ""}${result.stderr ?? ""}`,
    );
  }
  return result.stdout.trim();
}

function compile(output, config) {
  run(selectedCompiler, [
    source,
    "--target",
    "js-module",
    "--config",
    config,
    "-o",
    output,
  ]);
}

async function bundle(entry, output) {
  const result = await build({
    configFile: false,
    root,
    logLevel: "error",
    resolve: {
      conditions: ["browser", "module", "import", "default"],
    },
    build: {
      target: "es2022",
      minify: "oxc",
      write: false,
      lib: { entry, formats: ["es"], fileName: "bundle" },
      rolldownOptions: { output: { codeSplitting: false } },
    },
  });
  const outputs = Array.isArray(result)
    ? result.flatMap((item) => item.output)
    : result.output;
  const chunks = outputs.filter((item) => item.type === "chunk");
  assert.equal(chunks.length, 1, `${entry} should emit one JavaScript chunk`);
  const code = `${chunks[0].code.trim()}\n`;
  writeFileSync(output, code);
  return code;
}

function size(code) {
  const measured = canonicalCodecSizes(
    code,
    "SolidLil build-mode verification",
  );
  return {
    raw: measured.raw,
    gzip9: measured.gzip,
    brotli11: measured.brotli,
  };
}

function comparison(baseline, candidate) {
  const ratio = Object.fromEntries(
    Object.keys(baseline).map((metric) => [
      metric,
      candidate[metric] / baseline[metric],
    ]),
  );
  return {
    solid: baseline,
    solidlil: candidate,
    ratio,
    brotliSuperior: candidate.brotli11 < baseline.brotli11,
    compressedSuperior:
      candidate.brotli11 < baseline.brotli11 &&
      candidate.gzip9 < baseline.gzip9,
    rawSuperior: candidate.raw < baseline.raw,
    // Brotli-11 is the release gate because deployed JavaScript transfer size
    // is the primary product metric. Raw and gzip remain visible diagnostics.
    superior: candidate.brotli11 < baseline.brotli11,
  };
}

async function importFresh(path) {
  const url = pathToFileURL(path);
  url.searchParams.set("build", `${Date.now()}-${Math.random()}`);
  return import(url.href);
}

function assertSurface(module, label) {
  assert.deepEqual(Object.keys(module).sort(), publicNames, `${label} exports`);
}

function instrumentSurface(module) {
  const covered = new Set();
  return {
    covered,
    api: new Proxy(module, {
      get(target, property, receiver) {
        if (typeof property === "string") covered.add(property);
        return Reflect.get(target, property, receiver);
      },
    }),
  };
}

function behaviorDigest(api) {
  let dispose = () => {};
  let detachedDispose = () => {};
  let updateOutside = () => {};
  const effects = [];
  const renders = [];
  const cleanups = [];
  const computed = [];
  const explicit = [];
  const invalidations = [];
  const context = api.createContext("default");
  const contextDefault = api.useContext(context);
  const value = api.createRoot((rootDispose) => {
    dispose = rootDispose;
    const firstOwner = api.getOwner();
    const secondOwner = api.getOwner();
    api.onCleanup(() => cleanups.push("first"));
    api.onCleanup(() => cleanups.push("second"));
    const [count, setCount] = api.createSignal(1);
    updateOutside = setCount;
    const doubled = api.createMemo(() => count() * 2);
    api.createComputed(() => computed.push(count()));
    api.createRenderEffect(() => renders.push(`${count()}:${doubled()}`));
    api.createEffect(() => effects.push(`${count()}:${doubled()}`));
    api.createEffect(
      api.on(
        count,
        (next, previous, accumulator) => {
          explicit.push(`${previous ?? "none"}>${next}:${accumulator ?? 0}`);
          return (accumulator ?? 0) + next;
        },
        { defer: true },
      ),
      0,
    );
    api.onMount(() => {
      cleanups.push("mount");
      api.onCleanup(() => cleanups.push("mount-cleanup"));
    });
    const track = api.createReaction(() => invalidations.push("invalidated"));
    track(count);
    const selected = api.createSelector(count);
    const before = api.untrack(() => count());
    api.batch(() => {
      setCount(2);
      setCount((previous) => previous + 3);
    });
    track(count);
    setCount(6);
    const provided = context.Provider({
      value: "provided",
      get children() {
        return api.useContext(context);
      },
    });
    const detached = api.createRoot((childDispose) => {
      detachedDispose = childDispose;
      api.onCleanup(() => cleanups.push("detached"));
      return 17;
    }, null);
    return {
      before,
      count: count(),
      doubled: doubled(),
      selected6: selected(6),
      selected5: selected(5),
      ownerIdentity: firstOwner === secondOwner,
      listenerOutside: api.getListener() === null,
      context: provided(),
      detached,
      component: api.createComponent((props) => props.value + 1, {
        value: 8,
      }),
      uniqueIds: [api.createUniqueId(), api.createUniqueId()],
    };
  });
  updateOutside(7);
  dispose();
  const afterParentDispose = [...cleanups];
  detachedDispose();
  return {
    value,
    effects,
    renders,
    computed,
    explicit,
    invalidations,
    contextDefault,
    afterParentDispose,
    cleanups,
  };
}

function utilityDigest(api) {
  const [dynamic, setDynamic] = api.createSignal(1);
  const merged = api.mergeProps(
    { first: "base", live: undefined },
    {
      first: "override",
      get live() {
        return dynamic();
      },
      rest: 7,
    },
  );
  const split = api.splitProps(merged, ["first"], ["live"]);
  let setList = () => {};
  let setWhen = () => {};
  let setMode = () => {};
  let pushFrom = () => {};
  let unsubscribe = () => {};
  let dispose = () => {};
  let producerCleanups = 0;
  let rowCleanups = 0;
  const observed = [];
  const controls = api.createRoot((rootDispose) => {
    dispose = rootDispose;
    const [list, updateList] = api.createSignal([1, 2]);
    const [when, updateWhen] = api.createSignal(0);
    const [mode, updateMode] = api.createSignal(0);
    setList = updateList;
    setWhen = updateWhen;
    setMode = updateMode;
    const nested = api.children(() => [1, [() => 2], null]);
    const mapped = api.mapArray(list, (item, index) => {
      api.onCleanup(() => {
        rowCleanups += 1;
      });
      return () => `${item}:${index()}`;
    });
    const indexed = api.indexArray(list, (item, index) => {
      api.onCleanup(() => {
        rowCleanups += 1;
      });
      return () => `${item()}:${index}`;
    });
    const forValue = api.For({
      get each() {
        return list();
      },
      children: (item, index) => () => `${item}:${index()}`,
    });
    const indexValue = api.Index({
      get each() {
        return list();
      },
      children: (item, index) => () => `${item()}:${index}`,
    });
    const shown = api.Show({
      get when() {
        return when();
      },
      children: (value) => () => value(),
      fallback: "hidden",
    });
    const switched = api.Switch({
      get children() {
        return [
          api.Match({
            get when() {
              return mode() === 1;
            },
            children: "one",
          }),
          api.Match({
            get when() {
              return mode() === 2;
            },
            children: (value) => `two:${value()}`,
          }),
        ];
      },
      fallback: "none",
    });
    const fromValue = api.from((setter) => {
      pushFrom = setter;
      return () => {
        producerCleanups += 1;
      };
    }, 0);
    unsubscribe = api.observable(dynamic).subscribe((value) => {
      observed.push(value);
    }).unsubscribe;
    return {
      nested,
      mapped,
      indexed,
      forValue,
      indexValue,
      shown,
      switched,
      fromValue,
    };
  });

  const rows = (accessor) => accessor().map((row) => row());
  const showValue = () => {
    const value = controls.shown();
    return typeof value === "function" ? value() : value;
  };
  const initial = {
    children: controls.nested(),
    childArray: controls.nested.toArray(),
    mapped: rows(controls.mapped),
    indexed: rows(controls.indexed),
    forValue: rows(controls.forValue),
    indexValue: rows(controls.indexValue),
    show: showValue(),
    switch: controls.switched(),
    from: controls.fromValue(),
  };
  setDynamic(4);
  setList([2, 1, 3]);
  setWhen(5);
  setMode(2);
  pushFrom(9);
  const updated = {
    dynamic: merged.live,
    mapped: rows(controls.mapped),
    indexed: rows(controls.indexed),
    forValue: rows(controls.forValue),
    indexValue: rows(controls.indexValue),
    show: showValue(),
    switch: controls.switched(),
    from: controls.fromValue(),
  };
  unsubscribe();
  dispose();
  setDynamic(8);
  return {
    markers: [api.$DEVCOMP, api.$PROXY, api.$TRACK].map((value) => [
      typeof value,
      value.description,
    ]),
    dev: api.DEV,
    equal: [api.equalFn(1, 1), api.equalFn(1, 2), api.equalFn(NaN, NaN)],
    sharedConfig: Object.keys(api.sharedConfig)
      .sort()
      .map((key) => [key, typeof api.sharedConfig[key]]),
    merged: [merged.first, updated.dynamic, Object.keys(merged).sort()],
    split: [
      split.map((value) => Object.keys(value).sort()),
      split[0].first,
      split[1].live,
      split[2].rest,
    ],
    initial,
    updated,
    observed,
    producerCleanups,
    rowCleanups,
  };
}

async function schedulerDigest(api) {
  const [source, setSource] = api.createSignal(1);
  let dispose = () => {};
  const deferred = api.createRoot((rootDispose) => {
    dispose = rootDispose;
    return api.createDeferred(source);
  });
  const initial = deferred();
  setSource(2);
  const immediate = deferred();
  const deadline = Date.now() + 1000;
  while (deferred() !== 2 && Date.now() < deadline) {
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 1));
  }
  const settled = deferred();

  const callbacks = [];
  const cancelled = api.requestCallback(() => callbacks.push("cancelled"));
  api.cancelCallback(cancelled);
  api.requestCallback((didTimeout) => callbacks.push(`live:${didTimeout}`), {
    timeout: 100,
  });
  const callbackDeadline = Date.now() + 1000;
  while (callbacks.length === 0 && Date.now() < callbackDeadline) {
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 1));
  }
  dispose();
  return { initial, immediate, settled, callbacks };
}

function unwrapReactive(value) {
  let depth = 0;
  while (typeof value === "function" && depth++ < 40) value = value();
  if (Array.isArray(value)) return value.map(unwrapReactive);
  return value;
}

async function microtasks(count = 2) {
  for (let index = 0; index < count; index += 1)
    await new Promise((resolvePromise) => queueMicrotask(resolvePromise));
}

async function advancedDigest(api) {
  api.sharedConfig.context = { id: "root", count: 0 };
  api.enableHydration();
  let hydrationInside;
  const hydrationValue = api.createComponent(() => {
    hydrationInside = { ...api.sharedConfig.context };
    return "hydrated component";
  });
  const hydrationAfter = { ...api.sharedConfig.context };
  api.sharedConfig.context = undefined;

  let owner;
  api.createRoot(() => {
    owner = api.getOwner();
  });
  const ownerRestored = api.runWithOwner(owner, () => api.getOwner() === owner);

  const caught = [];
  const caughtReturn = api.catchError(
    () => {
      throw "caught";
    },
    (error) => caught.push([error.name, error.message]),
  );
  const observed = [];
  api.createRoot(() => {
    api.onError((error) => observed.push([error.name, error.message]));
    api.createComputed(() => {
      throw new TypeError("computed");
    });
  });
  let resetBoundary;
  let boundaryThrows = true;
  let disposeBoundary = () => {};
  const boundary = api.createRoot((dispose) => {
    disposeBoundary = dispose;
    return api.ErrorBoundary({
      get children() {
        if (boundaryThrows) throw new RangeError("boundary");
        return "healthy";
      },
      fallback(error, reset) {
        resetBoundary = reset;
        return `fallback:${error.name}:${error.message}`;
      },
    });
  });
  const boundaryInitial = unwrapReactive(boundary);
  boundaryThrows = false;
  resetBoundary();
  const boundaryReset = unwrapReactive(boundary);
  api.resetErrorBoundaries();
  const boundaryGlobalReset = unwrapReactive(boundary);
  disposeBoundary();

  const requests = [];
  const resourceChanges = [];
  const resourceErrors = [];
  let setResourceKey = () => {};
  let disposeResource = () => {};
  let resource;
  let resourceActions;
  api.createRoot((dispose) => {
    disposeResource = dispose;
    api.onError((error) => resourceErrors.push([error.name, error.message]));
    const [key, setKey] = api.createSignal(1);
    setResourceKey = setKey;
    [resource, resourceActions] = api.createResource(
      key,
      (nextKey, info) =>
        new Promise((resolvePromise, rejectPromise) =>
          requests.push({
            key: nextKey,
            info: { value: info.value, refetching: info.refetching },
            reject: rejectPromise,
            resolve: resolvePromise,
          }),
        ),
      { initialValue: "seed" },
    );
    api.createEffect(() =>
      resourceChanges.push(
        `${resource.state}:${resource.loading}:${resource()}`,
      ),
    );
  });
  const resourceInitial = {
    loading: resource.loading,
    state: resource.state,
    value: resource(),
  };
  requests[0].resolve("one");
  await microtasks();
  const resourceReady = {
    latest: resource.latest,
    loading: resource.loading,
    state: resource.state,
    value: resource(),
  };
  setResourceKey(2);
  const resourceRefreshing = {
    latest: resource.latest,
    loading: resource.loading,
    state: resource.state,
    value: resource(),
  };
  requests[1].reject("bad");
  await microtasks();
  let resourceThrown;
  try {
    resource();
  } catch (error) {
    resourceThrown = [error.name, error.message];
  }
  resourceActions.mutate("manual");
  const refetch = resourceActions.refetch("manual-refetch");
  const resourceRequestInfo = requests.map((request) => [
    request.key,
    request.info,
  ]);
  requests.at(-1).resolve("two");
  await refetch;
  await microtasks();
  const resourceFinal = { state: resource.state, value: resource() };
  disposeResource();

  let resolveDisposedResource;
  let disposePendingResource = () => {};
  let disposedResource;
  const disposedResourceEffects = [];
  const disposedResourcePromise = new Promise((resolvePromise) => {
    resolveDisposedResource = resolvePromise;
  });
  api.createRoot((dispose) => {
    disposePendingResource = dispose;
    [disposedResource] = api.createResource(() => disposedResourcePromise);
    api.createEffect(() => disposedResourceEffects.push(disposedResource()));
  });
  const disposedResourceBefore = {
    loading: disposedResource.loading,
    state: disposedResource.state,
    value: disposedResource(),
  };
  disposePendingResource();
  resolveDisposedResource("late");
  await microtasks();
  const disposedResourceAfter = {
    effects: disposedResourceEffects,
    loading: disposedResource.loading,
    state: disposedResource.state,
    value: disposedResource(),
  };

  let resolveLazy;
  let lazyLoads = 0;
  const Lazy = api.lazy(() => {
    lazyLoads += 1;
    return new Promise((resolvePromise) => {
      resolveLazy = resolvePromise;
    });
  });
  const firstPreload = Lazy.preload();
  const secondPreload = Lazy.preload();
  let disposeLazy = () => {};
  const lazyView = api.createRoot((dispose) => {
    disposeLazy = dispose;
    return Lazy({ value: 7 });
  });
  const lazyInitial = unwrapReactive(lazyView);
  resolveLazy({ default: (props) => `loaded:${props.value}` });
  await firstPreload;
  await microtasks();
  const lazyFinal = unwrapReactive(lazyView);
  disposeLazy();

  let resolveFirst;
  let resolveSecond;
  let disposeSuspense = () => {};
  const firstPromise = new Promise((resolvePromise) => {
    resolveFirst = resolvePromise;
  });
  const secondPromise = new Promise((resolvePromise) => {
    resolveSecond = resolvePromise;
  });
  const suspenseView = api.createRoot((dispose) => {
    disposeSuspense = dispose;
    const [first] = api.createResource(() => firstPromise);
    const [second] = api.createResource(() => secondPromise);
    return api.SuspenseList({
      revealOrder: "forwards",
      tail: "collapsed",
      get children() {
        return [
          api.Suspense({
            fallback: "loading:first",
            get children() {
              return `first:${first()}`;
            },
          }),
          api.Suspense({
            fallback: "loading:second",
            get children() {
              return `second:${second()}`;
            },
          }),
        ];
      },
    });
  });
  const suspenseInitial = unwrapReactive(suspenseView);
  resolveFirst("A");
  await microtasks();
  const suspenseMiddle = unwrapReactive(suspenseView);
  resolveSecond("B");
  await microtasks();
  const suspenseFinal = unwrapReactive(suspenseView);
  disposeSuspense();

  const transitionQueue = [];
  api.enableScheduling((callback) => transitionQueue.push(callback));
  const directTransition = api.startTransition;
  const [transitionPending, transition] = api.useTransition();
  const transitionIdentity = directTransition === transition;
  const [transitionSource, setTransitionSource] = api.createSignal(1);
  const transitionMemo = api.createMemo(() => transitionSource() * 2);
  const transitionLog = [];
  api.createEffect(() => transitionLog.push(`effect:${transitionMemo()}`));
  const transitionDone = transition(() => {
    transitionLog.push(`before:${transitionSource()}:${transitionMemo()}`);
    setTransitionSource(2);
    transitionLog.push(`after:${transitionSource()}:${transitionMemo()}`);
  });
  const transitionSync = {
    memo: transitionMemo(),
    pending: transitionPending(),
    queued: transitionQueue.length,
    source: transitionSource(),
  };
  await microtasks(1);
  const transitionWaiting = {
    memo: transitionMemo(),
    pending: transitionPending(),
    queued: transitionQueue.length,
    source: transitionSource(),
  };
  while (transitionQueue.length) transitionQueue.shift()();
  await transitionDone;
  const transitionFinal = {
    log: transitionLog,
    memo: transitionMemo(),
    pending: transitionPending(),
    source: transitionSource(),
  };

  const externalTriggers = [];
  const externalValues = [];
  let externalFactories = 0;
  let externalTracks = 0;
  let externalDisposals = 0;
  let externalUntracks = 0;
  let externalValue = 1;
  api.enableExternalSource(
    (compute, trigger) => {
      externalFactories += 1;
      externalTriggers.push(trigger);
      return {
        track(previous) {
          externalTracks += 1;
          return compute(previous);
        },
        dispose() {
          externalDisposals += 1;
        },
      };
    },
    (compute) => {
      externalUntracks += 1;
      return compute();
    },
  );
  let disposeExternal = () => {};
  let setExternalSource = () => {};
  let externalMemo;
  api.createRoot((dispose) => {
    disposeExternal = dispose;
    const [source, setSource] = api.createSignal(2);
    setExternalSource = setSource;
    externalMemo = api.createMemo(() => source() * externalValue);
    api.createEffect(() => externalValues.push(externalMemo()));
    api.untrack(source);
  });
  externalValue = 3;
  externalTriggers[0]();
  setExternalSource(4);
  disposeExternal();

  return {
    boundary: {
      caught,
      caughtReturn: String(caughtReturn),
      globalReset: boundaryGlobalReset,
      initial: boundaryInitial,
      observed,
      reset: boundaryReset,
    },
    external: {
      disposals: externalDisposals,
      factories: externalFactories,
      tracks: externalTracks,
      untracks: externalUntracks,
      values: externalValues,
    },
    hydration: {
      after: hydrationAfter,
      inside: hydrationInside,
      value: hydrationValue,
    },
    lazy: {
      final: lazyFinal,
      initial: lazyInitial,
      loads: lazyLoads,
      preloadIdentity: firstPreload === secondPreload,
    },
    ownerRestored,
    resource: {
      changes: resourceChanges,
      errors: resourceErrors,
      disposedAfter: disposedResourceAfter,
      disposedBefore: disposedResourceBefore,
      final: resourceFinal,
      initial: resourceInitial,
      ready: resourceReady,
      refreshing: resourceRefreshing,
      requests: resourceRequestInfo,
      thrown: resourceThrown,
    },
    suspense: {
      final: suspenseFinal,
      initial: suspenseInitial,
      middle: suspenseMiddle,
    },
    transition: {
      final: transitionFinal,
      identity: transitionIdentity,
      sync: transitionSync,
      waiting: transitionWaiting,
    },
  };
}

function format(value) {
  return new Intl.NumberFormat("en-US").format(value);
}

function reportHtml(report) {
  const sizes = report.openWorld.size;
  const delta = (metric) => {
    const value = (sizes.ratio[metric] - 1) * 100;
    return `${value > 0 ? "+" : ""}${value.toFixed(1)}%`;
  };
  return `<!doctype html>
<html lang="en"><head>
  <meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="description" content="Solid and SolidLil open-world and closed-world build verification.">
  <title>SolidLil build-mode verification</title>
  <style>
    :root{color-scheme:light;--paper:#f4f1e9;--ink:#17221f;--muted:#58655f;--line:#cfd7cf;--green:#08785b;--panel:#fffdf7}*{box-sizing:border-box}body{margin:0;background:linear-gradient(135deg,#e2f1e9,transparent 38rem),var(--paper);color:var(--ink);font:16px/1.55 Inter,system-ui,sans-serif}main{width:min(1040px,calc(100% - 32px));margin:auto;padding:64px 0 88px}h1{max-width:13ch;margin:.15em 0;font-size:clamp(2.5rem,7vw,5.2rem);line-height:.96;letter-spacing:-.055em}.eyebrow{color:var(--green);font:700 .76rem/1 ui-monospace,monospace;letter-spacing:.13em;text-transform:uppercase}.lead{max-width:68ch;color:var(--muted)}.grid{display:grid;grid-template-columns:1fr 1fr;gap:16px;margin:32px 0}.card{padding:24px;border:1px solid var(--line);border-radius:18px;background:var(--panel)}.card strong{display:block;color:var(--green);font-size:1.55rem}.table{overflow:auto;border:1px solid var(--line);border-radius:18px;background:var(--panel)}table{width:100%;min-width:720px;border-collapse:collapse;font-variant-numeric:tabular-nums}th,td{padding:13px 16px;border-bottom:1px solid var(--line);text-align:right}th:first-child{text-align:left}thead th{color:var(--muted);font-size:.75rem;text-transform:uppercase}.win{color:var(--green);font-weight:750}.note{max-width:72ch;margin-top:28px;padding-left:16px;border-left:3px solid var(--green);color:var(--muted)}code{font-family:ui-monospace,monospace}@media(max-width:680px){main{padding-top:40px}.grid{grid-template-columns:1fr}}
  </style>
</head><body><main>
  <p class="eyebrow">Solid 1.9.13 · reusable ABI evidence</p>
  <h1>The public surface stays public.</h1>
  <p class="lead">The open-world module preserves and executes ${publicNames.length} Solid-compatible primitives. The closed-world diagnostic retains the same binding count while renaming every public symbol; the application lane links those calls away entirely.</p>
  <section class="grid" aria-label="Results"><article class="card"><span>Open-world API</span><strong>pass</strong><p>Exact exports and reactive behavior match the official client runtime.</p></article><article class="card"><span>Closed-world mangling</span><strong>pass</strong><p>${report.closedWorld.emittedExports.length} of ${report.closedWorld.sourceExports.length} runtime exports renamed.</p></article></section>
  <h2>Reusable core bundle</h2><p class="lead">One Vite 8/Oxc ESM chunk per implementation; lower is better. Brotli-11 is the primary release gate; gzip and raw bytes remain visible diagnostics.</p>
  <div class="table"><table><thead><tr><th>Artifact</th><th>Brotli-11 · primary</th><th>Gzip-9</th><th>Raw</th></tr></thead><tbody><tr><th>Official Solid core</th><td>${format(sizes.solid.brotli11)}</td><td>${format(sizes.solid.gzip9)}</td><td>${format(sizes.solid.raw)}</td></tr><tr><th>SolidLil core</th><td>${format(sizes.solidlil.brotli11)}</td><td>${format(sizes.solidlil.gzip9)}</td><td>${format(sizes.solidlil.raw)}</td></tr><tr><th>Delta vs Solid</th><td>${delta("brotli11")}</td><td>${delta("gzip9")}</td><td>${delta("raw")}</td></tr></tbody></table></div>
  <p class="note"><strong>Scope:</strong> this is the exact ${publicNames.length}-export Solid browser-core surface. JSX/LSX, DOM, Store, application, performance, and retained-memory evidence are reported as separate gates.</p>
</main></body></html>\n`;
}

const openRuntime = resolve(packageRoot, "reactive.generated.js");
const closedRuntime = resolve(generated, "solidlil-reactive-closed.js");
compile(openRuntime, openConfig);
compile(closedRuntime, closedConfig);

const solidBundle = resolve(generated, "solid-core-open.js");
const solidlilBundle = resolve(generated, "solidlil-core-open.js");
const solidCode = await bundle(resolve(root, "api/solid-core.js"), solidBundle);
const { code: solidlilCode, selection: distributionSelection } =
  await selectSolidLilDistribution({
    entry: resolve(root, "api/solidlil-core.js"),
    output: solidlilBundle,
    target: "core-open",
  });
const [solid, solidlil, openRaw, closedRaw] = await Promise.all([
  importFresh(solidBundle),
  importFresh(solidlilBundle),
  importFresh(openRuntime),
  importFresh(closedRuntime),
]);
assertSurface(solid, "Solid");
assertSurface(solidlil, "SolidLil");
for (const name of publicNames) {
  assert.equal(
    typeof solidlil[name],
    typeof solid[name],
    `SolidLil.${name} export type`,
  );
}
const solidInstrumented = instrumentSurface(solid);
const solidlilInstrumented = instrumentSurface(solidlil);
const digest = behaviorDigest(solidInstrumented.api);
assert.deepEqual(
  behaviorDigest(solidlilInstrumented.api),
  digest,
  "SolidLil reactive behavior",
);
const utilities = utilityDigest(solidInstrumented.api);
assert.deepEqual(
  utilityDigest(solidlilInstrumented.api),
  utilities,
  "SolidLil utility/control-flow behavior",
);
const scheduler = await schedulerDigest(solidInstrumented.api);
assert.deepEqual(
  await schedulerDigest(solidlilInstrumented.api),
  scheduler,
  "SolidLil deferred/callback scheduler behavior",
);
const advanced = await advancedDigest(solidInstrumented.api);
assert.deepEqual(
  await advancedDigest(solidlilInstrumented.api),
  advanced,
  "SolidLil resource/error/transition/suspense/external-source behavior",
);
assert.deepEqual(
  [...solidInstrumented.covered].sort(),
  publicNames,
  "official Solid executable export coverage",
);
assert.deepEqual(
  [...solidlilInstrumented.covered].sort(),
  publicNames,
  "SolidLil executable export coverage",
);

const sourceExports = Object.keys(openRaw).sort();
const emittedExports = Object.keys(closedRaw).sort();
assert.equal(
  emittedExports.length,
  sourceExports.length,
  "closed export count",
);
assert.equal(
  emittedExports.filter((name) => sourceExports.includes(name)).length,
  0,
  "closed-world build must rename every runtime export",
);

const report = {
  schemaVersion: 2,
  generatedAt: new Date().toISOString(),
  toolchain: {
    node: process.version,
    solid: "1.9.13",
    vite: "8.2.0",
    compiler: {
      source: process.env.LILSCRIPT_COMPILER
        ? "LILSCRIPT_COMPILER"
        : "repository-release",
      path: relative(root, selectedCompiler) || selectedCompiler,
      version: run(selectedCompiler, ["--version"]),
      sha256: createHash("sha256")
        .update(readFileSync(selectedCompiler))
        .digest("hex"),
    },
    codecs: canonicalCodecProvenance("SolidLil build-mode verification"),
  },
  openWorld: {
    config: relative(root, openConfig),
    exports: publicNames,
    behaviorDigest: digest,
    utilityDigest: utilities,
    schedulerDigest: scheduler,
    advancedDigest: advanced,
    behaviorPassed: true,
    distributionSelection,
    size: comparison(size(solidCode), size(solidlilCode)),
  },
  closedWorld: {
    config: relative(root, closedConfig),
    sourceExports,
    emittedExports,
    exportsMangled: true,
  },
};
assert.equal(
  report.openWorld.size.superior,
  true,
  `SolidLil open-world size regression: ${JSON.stringify(report.openWorld.size)}`,
);

writeFileSync(
  resolve(root, "artifacts/build-modes.json"),
  `${JSON.stringify(report, null, 2)}\n`,
);
writeFileSync(resolve(root, "artifacts/build-modes.html"), reportHtml(report));
console.log(
  `SolidLil open-world Brotli ${report.openWorld.size.solidlil.brotli11}/${report.openWorld.size.solid.brotli11}; public ABI and closed-world mangling passed.`,
);
// Solid's cooperative scheduler installs a MessageChannel in Node. This is a
// finite verification command, so do not let that referenced port keep the
// process alive after every assertion and artifact write has completed.
process.exit(0);
