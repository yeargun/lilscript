import {
  batch as lilBatch,
  cancelCallback as lilCancelCallback,
  createComputed as lilCreateComputed,
  createDeferred as lilCreateDeferred,
  createEffect as lilCreateEffect,
  createMemoWithPrevious as lilCreateMemoWithPrevious,
  createReaction as lilCreateReaction,
  createRenderEffect as lilCreateRenderEffect,
  createRoot as lilCreateRoot,
  createRootWithOwner as lilCreateRootWithOwner,
  createSelector as lilCreateSelector,
  createSignal as lilCreateSignal,
  createUniqueId as lilCreateUniqueId,
  enableScheduling as lilEnableScheduling,
  getListener as lilGetListener,
  getOwner as lilGetOwner,
  indexArray as lilIndexArray,
  indexArrayWithFallback as lilIndexArrayWithFallback,
  mapArray as lilMapArray,
  mapArrayWithFallback as lilMapArrayWithFallback,
  onCleanup as lilOnCleanup,
  onMount as lilOnMount,
  ownerParent as lilOwnerParent,
  requestCallback as lilRequestCallback,
  runWithOwner as lilRunWithOwner,
  selectorSelected,
  signalGet,
  signalSet,
  signalUpdate,
  untrack as lilUntrack,
} from "./reactive.generated.js";

export const equalFn = (previous, next) => previous === next;
export const $PROXY = Symbol("solid-proxy");
export const $TRACK = Symbol("solid-track");
export const $DEVCOMP = Symbol("solid-dev-component");
const IS_DEV =
  typeof import.meta.env !== "undefined" && import.meta.env.DEV === true;
// Distribution builds may prove that no exported entry can initiate
// hydration. Keeping this as a build-time constant lets Vite/Oxc erase the
// corresponding branches without changing the complete browser entry.
const CLIENT_ONLY =
  import.meta.env?.SOLIDLIL_CLIENT_ONLY === true ||
  import.meta.env?.SOLIDLIL_CLIENT_ONLY === "true";
const devOwnerInitialized = Symbol("solidlil-dev-owner");
const devSignalNode = Symbol("solidlil-dev-signal");
const devHooks = {
  afterCreateOwner: null,
  afterRegisterGraph: null,
  afterUpdate: null,
};
let devRootDepth = 0;
let devWriteDepth = 0;
let devBatchDepth = 0;
let devUpdatePending = false;

function ensureDevOwner(owner) {
  if (!IS_DEV || !owner) return owner;
  owner.owned ||= [];
  owner.sourceMap ||= [];
  return owner;
}

function activateDevOwner(owner, metadata) {
  if (!IS_DEV || !owner) return owner;
  ensureDevOwner(owner);
  if (metadata) Object.assign(owner, metadata);
  if (!owner[devOwnerInitialized]) {
    Object.defineProperty(owner, devOwnerInitialized, { value: true });
    devHooks.afterCreateOwner?.(owner);
  }
  return owner;
}

function reserveDevOwner(metadata) {
  if (!IS_DEV) return null;
  const parent = ensureDevOwner(lilGetOwner());
  if (!parent) return null;
  const index = parent.owned.length;
  parent.owned.push({ ...metadata });
  return { index, metadata, parent };
}

function bindDevOwner(reservation, owner) {
  if (!IS_DEV) return owner;
  activateDevOwner(owner, reservation?.metadata);
  if (reservation) reservation.parent.owned[reservation.index] = owner;
  return owner;
}

function withDevComputation(compute, options) {
  if (!IS_DEV) return compute;
  const reservation = reserveDevOwner({ name: options?.name });
  let initialized = false;
  return (...args) => {
    if (!initialized) {
      bindDevOwner(reservation, lilGetOwner());
      initialized = true;
    }
    return compute(...args);
  };
}

function flushDevUpdate() {
  if (
    !IS_DEV ||
    !devUpdatePending ||
    devRootDepth ||
    devWriteDepth ||
    devBatchDepth
  )
    return;
  devUpdatePending = false;
  devHooks.afterUpdate?.();
}

function requestDevUpdate() {
  if (!IS_DEV) return;
  devUpdatePending = true;
  flushDevUpdate();
}

function registerDevGraph(value) {
  if (!IS_DEV) return value;
  const owner = ensureDevOwner(lilGetOwner());
  if (owner) owner.sourceMap.push(value);
  devHooks.afterRegisterGraph?.(value);
  return value;
}

function devWriteSignal(node, value) {
  const signal = node?.[devSignalNode] ?? node?.[signalHandle] ?? node;
  return signalSet(signal, value);
}

export const DEV = IS_DEV
  ? {
      hooks: devHooks,
      registerGraph: registerDevGraph,
      writeSignal: devWriteSignal,
    }
  : undefined;
export const sharedConfig = {
  context: undefined,
  registry: undefined,
  effects: undefined,
  done: false,
  getContextId() {
    return hydrationContextId(this.context.count);
  },
  getNextContextId() {
    return hydrationContextId(this.context.count++);
  },
};

function hydrationContextId(count) {
  const number = String(count);
  const prefixLength = number.length - 1;
  return `${sharedConfig.context.id}${
    prefixLength ? String.fromCharCode(96 + prefixLength) : ""
  }${number}`;
}

const signalHandle = Symbol("solidlil-signal");
let hydrationEnabled = false;
let callbackSchedulingEnabled = false;
let transitionScheduler;
let activeTransition;
let committingTransition;
let transitionState;
// Transition tuple: done, memos, staged writes, promises, committed, finished,
// setPending, resolve, suspense-driven.
let externalSourceConfig;
let suspenseContext;
let suspenseListContext;
let boundaryResetters;
const errorHandlers = new WeakMap();

function ensureCallbackScheduling() {
  if (callbackSchedulingEnabled) return;
  callbackSchedulingEnabled = true;
  lilEnableScheduling(queueMicrotask);
}

function equalsOption(options) {
  if (options?.equals === false) return () => false;
  return options?.equals ?? equalFn;
}

function accessorFor(signal, transitionCompute) {
  const accessor = () => {
    if (activeTransition?.[2].has(signal))
      return activeTransition[2].get(signal);
    if (activeTransition && transitionCompute) {
      if (activeTransition[1].has(signal))
        return activeTransition[1].get(signal);
      const value = transitionCompute();
      activeTransition[1].set(signal, value);
      return value;
    }
    return signalGet(signal);
  };
  accessor[signalHandle] = signal;
  return accessor;
}

function writeSignalValue(signal, next) {
  if (activeTransition) {
    const previous = activeTransition[2].has(signal)
      ? activeTransition[2].get(signal)
      : signalGet(signal);
    const value = typeof next === "function" ? next(previous) : next;
    activeTransition[2].set(signal, value);
    activeTransition[1].clear();
    return value;
  }
  return typeof next === "function"
    ? signalUpdate(signal, next)
    : signalSet(signal, next);
}

function writeSignal(signal, next) {
  devWriteDepth += 1;
  try {
    return writeSignalValue(signal, next);
  } finally {
    devWriteDepth -= 1;
    requestDevUpdate();
  }
}

function setFrameworkSignal(accessor, value) {
  return signalSet(accessor[signalHandle], value, true);
}

function updateFrameworkSignal(accessor, update) {
  return signalUpdate(accessor[signalHandle], update, true);
}

function castError(error) {
  if (error instanceof Error) return error;
  return Error(typeof error === "string" ? error : "Unknown error", {
    cause: error,
  });
}

function decorateOwner(owner) {
  if (owner && owner.owner === undefined)
    owner.owner = decorateOwner(lilOwnerParent(owner));
  return owner;
}

function handleError(error, owner = lilGetOwner()) {
  const normalized = castError(error);
  let current = owner;
  while (current) {
    const handlers = errorHandlers.get(current);
    if (handlers?.length) {
      for (const handler of handlers) {
        try {
          handler(normalized);
        } catch (nextError) {
          return handleError(nextError, lilOwnerParent(current));
        }
      }
      return undefined;
    }
    current = lilOwnerParent(current);
  }
  throw normalized;
}

function guardComputation(compute) {
  const tracked = withExternalSource(compute);
  return (...args) => {
    try {
      return tracked(...args);
    } catch (error) {
      return handleError(error);
    }
  };
}

function withExternalSource(compute) {
  if (!externalSourceConfig) return compute;
  const version = lilCreateSignal(0, () => false);
  const trigger = () => signalUpdate(version, (value) => value + 1);
  const source = externalSourceConfig.factory(compute, trigger);
  lilOnCleanup(() => source.dispose());
  return (...args) => {
    signalGet(version);
    return source.track(...args);
  };
}

function signalFor(accessor, equals = equalFn) {
  let signal = accessor[signalHandle];
  if (signal === undefined) {
    const tracked = createMemo(accessor, undefined, { equals });
    signal = tracked[signalHandle];
  }
  return signal;
}

function resolveChildren(value) {
  while (typeof value === "function" && !value.length) value = value();
  if (!Array.isArray(value)) return value;
  const resolved = [];
  for (const child of value) {
    const next = resolveChildren(child);
    if (Array.isArray(next)) resolved.push(...next);
    else resolved.push(next);
  }
  return resolved;
}

export function enableHydration() {
  if (!CLIENT_ONLY) hydrationEnabled = true;
}

export function enableScheduling(scheduler = requestCallback) {
  transitionScheduler = scheduler;
}

export function enableExternalSource(factory, externalUntrack = (fn) => fn()) {
  if (externalSourceConfig) {
    const previous = externalSourceConfig;
    externalSourceConfig = {
      factory(compute, trigger) {
        const outer = previous.factory(compute, trigger);
        const inner = factory((value) => outer.track(value), trigger);
        return {
          track: (value) => inner.track(value),
          dispose() {
            inner.dispose();
            outer.dispose();
          },
        };
      },
      untrack: (fn) => previous.untrack(() => externalUntrack(fn)),
    };
  } else {
    externalSourceConfig = { factory, untrack: externalUntrack };
  }
}

export function createSignal(value, options) {
  const signal = lilCreateSignal(value, equalsOption(options));
  const accessor = accessorFor(signal);
  if (IS_DEV && !options?.internal) {
    const node = {
      name: options?.name,
      get value() {
        return lilUntrack(() => signalGet(signal));
      },
    };
    Object.defineProperty(node, devSignalNode, { value: signal });
    registerDevGraph(node);
  }
  return [
    accessor,
    (next) =>
      IS_DEV ? writeSignal(signal, next) : writeSignalValue(signal, next),
  ];
}

export function createMemo(compute, initialValue, options) {
  const guarded = guardComputation(
    IS_DEV ? withDevComputation(compute, options) : compute,
  );
  const equals = equalsOption(options);
  let initialized = false;
  const signal = lilCreateMemoWithPrevious(
    initialValue,
    guarded,
    (previous, next) => {
      if (!initialized) {
        initialized = true;
        return false;
      }
      return equals(previous, next);
    },
  );
  return accessorFor(signal, () => guarded(signalGet(signal)));
}

export function createEffect(compute, initialValue, options) {
  let previous = initialValue;
  const guarded = guardComputation(
    IS_DEV ? withDevComputation(compute, options) : compute,
  );
  lilCreateEffect(() => {
    previous = guarded(previous);
  });
}

export function createComputed(compute, initialValue, options) {
  let previous = initialValue;
  const guarded = guardComputation(
    IS_DEV ? withDevComputation(compute, options) : compute,
  );
  lilCreateComputed(() => {
    previous = guarded(previous);
  });
}

export function createRenderEffect(compute, initialValue, options) {
  let previous = initialValue;
  const guarded = guardComputation(
    IS_DEV ? withDevComputation(compute, options) : compute,
  );
  lilCreateRenderEffect(() => {
    previous = guarded(previous);
  });
}

export function createRoot(compute, owner) {
  const unowned = !compute.length;
  const guarded = (dispose) => {
    if (IS_DEV) activateDevOwner(getOwner());
    try {
      return unowned
        ? IS_DEV
          ? compute(() => {
              throw Error(
                "Dispose method must be an explicit argument to createRoot function",
              );
            })
          : compute()
        : compute(() => untrack(dispose));
    } catch (error) {
      return handleError(error);
    }
  };
  const execute = () => {
    if (unowned) return lilCreateRootWithOwner(guarded, null);
    return owner === undefined
      ? lilCreateRoot(guarded)
      : lilCreateRootWithOwner(guarded, owner);
  };
  if (!IS_DEV) return execute();
  devRootDepth += 1;
  try {
    return execute();
  } finally {
    devRootDepth -= 1;
    requestDevUpdate();
  }
}

export function getOwner() {
  return decorateOwner(lilGetOwner());
}

export function getListener() {
  return decorateOwner(lilGetListener());
}

export function runWithOwner(owner, compute) {
  return lilRunWithOwner(owner, () => {
    try {
      return compute();
    } catch (error) {
      return handleError(error);
    }
  });
}

export function createSelector(source, equals = equalFn) {
  const selector = lilCreateSelector(signalFor(source, equals), equals);
  return (key) => selectorSelected(selector, key);
}

export function createReaction(onInvalidate, _options) {
  return lilCreateReaction(onInvalidate);
}

export function on(dependencies, compute, options) {
  const list = Array.isArray(dependencies) ? dependencies : null;
  let previousInput;
  let deferred = options && options.defer;
  return (previousValue) => {
    const input = list
      ? list.map((dependency) => dependency())
      : dependencies();
    if (deferred) {
      deferred = false;
      return previousValue;
    }
    const next = untrack(() => compute(input, previousInput, previousValue));
    previousInput = input;
    return next;
  };
}

export function onMount(callback) {
  const owner = lilGetOwner();
  lilOnMount(() => {
    try {
      callback();
    } catch (error) {
      handleError(error, owner);
    }
  });
}

export function onError(handler) {
  const owner = lilGetOwner();
  if (!owner) return;
  const handlers = errorHandlers.get(owner);
  if (handlers) handlers.push(handler);
  else errorHandlers.set(owner, [handler]);
}

export function catchError(compute, handler) {
  const parent = lilGetOwner();
  return lilCreateRootWithOwner(
    (_dispose) => {
      const owner = lilGetOwner();
      errorHandlers.set(owner, [handler]);
      try {
        return compute();
      } catch (error) {
        return handleError(error, owner);
      }
    },
    parent,
    true,
  );
}

export function createDeferred(source, options) {
  ensureCallbackScheduling();
  const signal = lilCreateDeferred(
    signalFor(source, equalsOption(options)),
    equalsOption(options),
    options?.timeoutMs ?? 1073741823,
  );
  return accessorFor(signal);
}

export function createResource(
  sourceOrFetcher,
  fetcherOrOptions,
  maybeOptions,
) {
  const hasSource = typeof fetcherOrOptions === "function";
  const source = hasSource ? sourceOrFetcher : true;
  const fetcher = hasSource ? fetcherOrOptions : sourceOrFetcher;
  const options = (hasSource ? maybeOptions : fetcherOrOptions) ?? {};
  const dynamic = typeof source === "function" ? createMemo(source) : null;
  const storage = options.storage ?? createSignal;
  const [value, setValue] = storage(options.initialValue);
  const [error, setError] = createSignal(undefined);
  const [track, trigger] = createSignal(
    undefined,
    IS_DEV ? { equals: false, internal: true } : { equals: false },
  );
  const [state, setState] = createSignal(
    "initialValue" in options ? "ready" : "unresolved",
  );
  const contexts = new Set();
  let resolved = "initialValue" in options;
  let promise = null;
  let requestId = 0;
  let scheduled = false;
  let owner = lilGetOwner();

  function complete(id, nextValue, nextError, key) {
    if (id !== requestId) return nextValue;
    promise = null;
    if (key !== undefined) resolved = true;
    batch(() => {
      if (nextError === undefined) setValue(() => nextValue);
      setState(
        nextError !== undefined ? "errored" : resolved ? "ready" : "unresolved",
      );
      setError(nextError);
    });
    for (const context of contexts) context.decrement();
    contexts.clear();
    return nextValue;
  }

  function load(refetching = true) {
    if (refetching !== false && scheduled) return promise;
    scheduled = false;
    const key = dynamic ? dynamic() : source;
    const id = ++requestId;
    if (key == null || key === false) {
      promise = null;
      return complete(id, untrack(value), undefined, undefined);
    }
    let next;
    try {
      next = untrack(() =>
        fetcher(key, {
          value: value(),
          refetching,
        }),
      );
    } catch (fetchError) {
      return complete(id, undefined, castError(fetchError), key);
    }
    if (!next || typeof next.then !== "function") {
      return complete(id, next, undefined, key);
    }
    promise = next;
    if ("v" in next && "s" in next) {
      return next.s === 1
        ? complete(id, next.v, undefined, key)
        : complete(id, undefined, castError(next.v), key);
    }
    scheduled = true;
    queueMicrotask(() => {
      scheduled = false;
    });
    batch(() => {
      setState(resolved ? "refreshing" : "pending");
      setError(undefined);
      trigger();
    });
    const settled = next.then(
      (nextValue) => {
        if (options.onHydrated && !resolved)
          queueMicrotask(() => options.onHydrated(key, { value: nextValue }));
        return complete(id, nextValue, undefined, key);
      },
      (nextError) => complete(id, undefined, castError(nextError), key),
    );
    promise = settled;
    const loadingTransition = activeTransition ?? committingTransition;
    if (loadingTransition) {
      loadingTransition[3].add(next);
      const finish = () => {
        loadingTransition[3].delete(next);
        finishTransition(loadingTransition);
      };
      next.then(finish, finish);
    }
    return settled;
  }

  function read() {
    const current = value();
    const currentError = error();
    if (currentError !== undefined && !promise) throw currentError;
    if (lilGetListener() && suspenseContext) {
      const suspense = useContext(suspenseContext);
      if (suspense) {
        createComputed(() => {
          track();
          if (promise && !contexts.has(suspense)) {
            suspense.increment();
            contexts.add(suspense);
          }
        });
      }
    }
    return current;
  }

  Object.defineProperties(read, {
    state: { get: () => state() },
    error: { get: () => error() },
    loading: {
      get() {
        const current = state();
        return current === "pending" || current === "refreshing";
      },
    },
    latest: {
      get() {
        if (!resolved) return read();
        const currentError = error();
        if (currentError !== undefined && !promise) throw currentError;
        return value();
      },
    },
  });

  if (dynamic) {
    createComputed(() => {
      owner = lilGetOwner();
      load(false);
    });
  } else load(false);
  return [
    read,
    {
      refetch: (info) => runWithOwner(owner, () => load(info)),
      mutate: setValue,
    },
  ];
}

function provideContext(context, value, read) {
  const parent = lilGetOwner();
  return lilCreateRootWithOwner(
    (_dispose) => {
      const owner = decorateOwner(lilGetOwner());
      owner.context = { [context.id]: value };
      return read();
    },
    parent,
    true,
  );
}

export function createContext(defaultValue, options) {
  const context = {
    id: Symbol("context"),
    defaultValue,
    Provider(props) {
      const reservation = reserveDevOwner({ name: options?.name });
      return provideContext(context, props.value, () => {
        if (IS_DEV) bindDevOwner(reservation, lilGetOwner());
        return children(() => props.children);
      });
    },
  };
  return context;
}

export function useContext(context) {
  let owner = decorateOwner(lilGetOwner());
  while (owner) {
    const value = owner.context?.[context.id];
    if (value !== undefined) return value;
    owner = owner.owner;
  }
  return context.defaultValue;
}

export function children(source) {
  const memo = createMemo(source);
  const resolved = createMemo(() => resolveChildren(memo()));
  resolved.toArray = () => {
    const value = resolved();
    return Array.isArray(value) ? value : value == null ? [] : [value];
  };
  return resolved;
}

export function createComponent(component, props) {
  if (!IS_DEV) {
    if (!CLIENT_ONLY && hydrationEnabled && sharedConfig.context) {
      const parent = sharedConfig.context;
      sharedConfig.context = {
        ...parent,
        id: sharedConfig.getNextContextId(),
        count: 0,
      };
      const result = untrack(() => component(props || {}));
      sharedConfig.context = parent;
      return result;
    }
    return untrack(() => component(props || {}));
  }
  const invoke = () => {
    if (CLIENT_ONLY || !hydrationEnabled || !sharedConfig.context)
      return untrack(() => component(props || {}));
    const parent = sharedConfig.context;
    sharedConfig.context = {
      ...parent,
      id: sharedConfig.getNextContextId(),
      count: 0,
    };
    try {
      return untrack(() => component(props || {}));
    } finally {
      sharedConfig.context = parent;
    }
  };
  const parent = getOwner();
  const reservation = reserveDevOwner({
    component,
    name: component.name,
    props,
  });
  return lilCreateRootWithOwner(
    (_dispose) => {
      bindDevOwner(reservation, getOwner());
      return invoke();
    },
    parent,
    true,
  );
}

const supportsProxy = typeof Proxy === "function";
const trueFunction = () => true;
const propTraps = {
  get(target, property, receiver) {
    if (property === $PROXY) return receiver;
    return target.get(property);
  },
  has(target, property) {
    if (property === $PROXY) return true;
    return target.has(property);
  },
  set: trueFunction,
  deleteProperty: trueFunction,
  getOwnPropertyDescriptor(target, property) {
    return {
      configurable: true,
      enumerable: true,
      get() {
        return target.get(property);
      },
      set: trueFunction,
      deleteProperty: trueFunction,
    };
  },
  ownKeys(target) {
    return target.keys();
  },
};

function resolvePropSource(source) {
  return !(source = typeof source === "function" ? source() : source)
    ? {}
    : source;
}

function resolvePropSources() {
  for (let index = 0; index < this.length; index += 1) {
    const value = this[index]();
    if (value !== undefined) return value;
  }
  return undefined;
}

export function mergeProps(...sources) {
  let proxy = false;
  for (let index = 0; index < sources.length; index += 1) {
    const source = sources[index];
    proxy ||= !!source && $PROXY in source;
    if (typeof source === "function") {
      proxy = true;
      sources[index] = createMemo(source);
    }
  }
  if (supportsProxy && proxy) {
    return new Proxy(
      {
        get(property) {
          for (let index = sources.length - 1; index >= 0; index -= 1) {
            const value = resolvePropSource(sources[index])[property];
            if (value !== undefined) return value;
          }
          return undefined;
        },
        has(property) {
          for (let index = sources.length - 1; index >= 0; index -= 1) {
            if (property in resolvePropSource(sources[index])) return true;
          }
          return false;
        },
        keys() {
          const keys = [];
          for (const source of sources)
            keys.push(...Object.keys(resolvePropSource(source)));
          return [...new Set(keys)];
        },
      },
      propTraps,
    );
  }
  const sourceMap = {};
  const defined = Object.create(null);
  for (let index = sources.length - 1; index >= 0; index -= 1) {
    const source = sources[index];
    if (!source) continue;
    const sourceKeys = Object.getOwnPropertyNames(source);
    for (let keyIndex = sourceKeys.length - 1; keyIndex >= 0; keyIndex -= 1) {
      const key = sourceKeys[keyIndex];
      if (key === "__proto__" || key === "constructor") continue;
      const descriptor = Object.getOwnPropertyDescriptor(source, key);
      if (!defined[key]) {
        defined[key] = descriptor.get
          ? {
              enumerable: true,
              configurable: true,
              get: resolvePropSources.bind(
                (sourceMap[key] = [descriptor.get.bind(source)]),
              ),
            }
          : descriptor.value !== undefined
            ? descriptor
            : undefined;
      } else if (sourceMap[key]) {
        if (descriptor.get) sourceMap[key].push(descriptor.get.bind(source));
        else if (descriptor.value !== undefined)
          sourceMap[key].push(() => descriptor.value);
      }
    }
  }
  const target = {};
  for (const key of Object.keys(defined).reverse()) {
    const descriptor = defined[key];
    if (descriptor?.get) Object.defineProperty(target, key, descriptor);
    else target[key] = descriptor?.value;
  }
  return target;
}

export function splitProps(props, ...groups) {
  const length = groups.length;
  if (supportsProxy && $PROXY in props) {
    const blocked = length > 1 ? groups.flat() : groups[0];
    const result = groups.map(
      (group) =>
        new Proxy(
          {
            get: (property) =>
              group.includes(property) ? props[property] : undefined,
            has: (property) => group.includes(property) && property in props,
            keys: () => group.filter((property) => property in props),
          },
          propTraps,
        ),
    );
    result.push(
      new Proxy(
        {
          get: (property) =>
            blocked.includes(property) ? undefined : props[property],
          has: (property) =>
            blocked.includes(property) ? false : property in props,
          keys: () =>
            Object.keys(props).filter((key) => !blocked.includes(key)),
        },
        propTraps,
      ),
    );
    return result;
  }
  const objects = Array.from({ length: length + 1 }, () => ({}));
  for (const property of Object.getOwnPropertyNames(props)) {
    let groupIndex = length;
    for (let index = 0; index < groups.length; index += 1) {
      if (groups[index].includes(property)) {
        groupIndex = index;
        break;
      }
    }
    const descriptor = Object.getOwnPropertyDescriptor(props, property);
    const defaultDescriptor =
      !descriptor.get &&
      !descriptor.set &&
      descriptor.enumerable &&
      descriptor.writable &&
      descriptor.configurable;
    if (defaultDescriptor) objects[groupIndex][property] = descriptor.value;
    else Object.defineProperty(objects[groupIndex], property, descriptor);
  }
  return objects;
}

export function mapArray(list, mapFunction, options = {}) {
  const signal = signalFor(
    () => {
      const items = list() || [];
      items[$TRACK];
      return items;
    },
    () => false,
  );
  const wrapped = (item, indexSignal) =>
    mapFunction(item, accessorFor(indexSignal));
  return options.fallback
    ? lilMapArrayWithFallback(signal, wrapped, options.fallback)
    : lilMapArray(signal, wrapped);
}

export function indexArray(list, mapFunction, options = {}) {
  const signal = signalFor(
    () => {
      const items = list() || [];
      items[$TRACK];
      return items;
    },
    () => false,
  );
  const wrapped = (itemSignal, index) =>
    mapFunction(accessorFor(itemSignal), index);
  return options.fallback
    ? lilIndexArrayWithFallback(signal, wrapped, options.fallback)
    : lilIndexArray(signal, wrapped);
}

export function For(props) {
  const fallback = "fallback" in props && { fallback: () => props.fallback };
  return createMemo(mapArray(() => props.each, props.children, fallback));
}

export function Index(props) {
  const fallback = "fallback" in props && { fallback: () => props.fallback };
  return createMemo(indexArray(() => props.each, props.children, fallback));
}

export function lazy(loader) {
  let component;
  let promise;
  const load = () =>
    promise ||
    (promise = Promise.resolve(loader()).then((module) => {
      component = () => module.default;
      return module.default;
    }));
  const wrapped = (props) => {
    if (!component) {
      const [resource] = createResource(load);
      component = resource;
    }
    return createMemo(() => {
      const resolved = component();
      return resolved ? untrack(() => resolved(props)) : "";
    });
  };
  wrapped.preload = load;
  return wrapped;
}

export function Show(props) {
  const conditionValue = createMemo(() => props.when);
  const condition = props.keyed
    ? conditionValue
    : createMemo(conditionValue, undefined, {
        equals: (previous, next) => !previous === !next,
      });
  return createMemo(() => {
    const value = condition();
    if (!value) return props.fallback;
    const child = props.children;
    if (typeof child !== "function" || !child.length) return child;
    return untrack(() =>
      child(
        props.keyed
          ? value
          : () => {
              if (!untrack(condition)) throw Error("Stale read from <Show>.");
              return conditionValue();
            },
      ),
    );
  });
}

export function Match(props) {
  return props;
}

export function Switch(props) {
  const matches = children(() => props.children);
  const switchFunction = createMemo(() => {
    const childrenValue = matches();
    const candidates = Array.isArray(childrenValue)
      ? childrenValue
      : [childrenValue];
    let select = () => undefined;
    for (let index = 0; index < candidates.length; index += 1) {
      const candidate = candidates[index];
      const previousSelect = select;
      const conditionValue = createMemo(() =>
        previousSelect() ? undefined : candidate.when,
      );
      const condition = candidate.keyed
        ? conditionValue
        : createMemo(conditionValue, undefined, {
            equals: (previous, next) => !previous === !next,
          });
      select = () =>
        previousSelect() ||
        (condition() ? [index, conditionValue, candidate] : undefined);
    }
    return select;
  });
  return createMemo(() => {
    const selected = switchFunction()();
    if (!selected) return props.fallback;
    const [index, conditionValue, candidate] = selected;
    const child = candidate.children;
    if (typeof child !== "function" || !child.length) return child;
    return untrack(() =>
      child(
        candidate.keyed
          ? conditionValue()
          : () => {
              const currentIndex = untrack(switchFunction)()?.[0];
              if (currentIndex !== index) {
                // A nested DOM computation can already be queued when its
                // parent switches branches. Let that owned computation finish
                // its synchronous turn; the parent immediately disposes it.
                // Reads made after unmount have no listener and must throw.
                if (!lilGetListener()) throw Error("Stale read from <Match>.");
              }
              return conditionValue();
            },
      ),
    );
  });
}

export function resetErrorBoundaries() {
  if (!boundaryResetters) return;
  for (const reset of [...boundaryResetters]) reset();
}

export function ErrorBoundary(props) {
  const [error, setError] = createSignal(undefined);
  boundaryResetters ||= new Set();
  boundaryResetters.add(setError);
  onCleanup(() => boundaryResetters.delete(setError));
  const fallbackFor = (current) => {
    const fallback = props.fallback;
    return typeof fallback === "function" && fallback.length
      ? untrack(() => fallback(current, () => setError(undefined)))
      : fallback;
  };
  return createMemo(() => {
    const current = error();
    if (current !== undefined) return fallbackFor(current);
    let caught;
    const result = catchError(
      () => props.children,
      (nextError) => {
        caught = nextError;
        setError(nextError);
      },
    );
    return caught === undefined ? result : fallbackFor(caught);
  });
}

function getSuspenseContext() {
  return suspenseContext || (suspenseContext = createContext());
}

function getSuspenseListContext() {
  return suspenseListContext || (suspenseListContext = createContext());
}

const suspenseListEquals = (previous, next) =>
  previous.showContent === next.showContent &&
  previous.showFallback === next.showFallback;

export function SuspenseList(props) {
  const [wrapper] = createSignal(() => ({ inFallback: false }));
  const parentList = useContext(getSuspenseListContext());
  const registry = [];
  const [registryVersion] = createSignal(0);
  let building = true;
  const show = parentList
    ? parentList.register(createMemo(() => wrapper()().inFallback))
    : null;
  const resolved = createMemo(
    (previous) => {
      const reveal = props.revealOrder;
      const tail = props.tail;
      const parent = show ? show() : { showContent: true, showFallback: true };
      registryVersion();
      const entries = registry;
      const reverse = reveal === "backwards";
      if (reveal === "together") {
        const all = entries.every((inFallback) => !inFallback());
        const result = entries.map(() => ({
          showContent: all && parent.showContent,
          showFallback: parent.showFallback,
        }));
        result.inFallback = !all;
        return result;
      }
      let stopped = false;
      let inFallback = previous.inFallback;
      const result = [];
      for (let index = 0; index < entries.length; index += 1) {
        const currentIndex = reverse ? entries.length - index - 1 : index;
        const fallback = entries[currentIndex]();
        if (!stopped && !fallback) {
          result[currentIndex] = {
            showContent: parent.showContent,
            showFallback: parent.showFallback,
          };
        } else {
          const firstStopped = !stopped;
          if (firstStopped) inFallback = true;
          result[currentIndex] = {
            showContent: firstStopped,
            showFallback:
              (!tail || (firstStopped && tail === "collapsed")) &&
              parent.showFallback,
          };
          stopped = true;
        }
      }
      if (!stopped) inFallback = false;
      result.inFallback = inFallback;
      return result;
    },
    { inFallback: false },
  );
  setFrameworkSignal(wrapper, resolved);
  const listContext = getSuspenseListContext();
  return provideContext(
    listContext,
    {
      register(inFallback) {
        const index = registry.length;
        registry.push(inFallback);
        if (!building)
          updateFrameworkSignal(registryVersion, (version) => version + 1);
        return createMemo(
          () =>
            resolved()[index] ?? {
              showContent: true,
              showFallback: true,
            },
          undefined,
          { equals: suspenseListEquals },
        );
      },
    },
    () => {
      const result = props.children;
      building = false;
      updateFrameworkSignal(registryVersion, (version) => version + 1);
      return result;
    },
  );
}

export function Suspense(props) {
  const [inFallback] = createSignal(false);
  let pending = 0;
  const store = {
    effects: [],
    inFallback,
    resolved: false,
    increment() {
      if (++pending === 1) setFrameworkSignal(inFallback, true);
    },
    decrement() {
      if (--pending === 0) setFrameworkSignal(inFallback, false);
    },
  };
  const list = useContext(getSuspenseListContext());
  const show = list?.register(store.inFallback);
  const owner = lilGetOwner();
  let dispose;
  let fallbackValue;
  onCleanup(() => dispose?.());
  const context = getSuspenseContext();
  return provideContext(context, store, () => {
    const rendered = createMemo(() => props.children);
    return createMemo((previous) => {
      const visibility = show
        ? show()
        : { showContent: true, showFallback: true };
      if (!store.inFallback() && visibility.showContent) {
        store.resolved = true;
        dispose?.();
        dispose = undefined;
        return rendered();
      }
      if (!visibility.showFallback) return undefined;
      if (dispose) return fallbackValue;
      return createRoot((fallbackDispose) => {
        dispose = fallbackDispose;
        return (fallbackValue = props.fallback);
      }, owner);
    });
  });
}

function transitionPair() {
  return transitionState || (transitionState = lilCreateSignal(false, equalFn));
}

function finishTransition(transition) {
  if (transition[5] || !transition[4] || transition[3].size) return;
  transition[5] = true;
  transition[6](false);
  transition[7]();
}

export function startTransition(compute) {
  if (activeTransition) {
    compute();
    return activeTransition[0];
  }
  let resolveDone;
  let rejectDone;
  const done = new Promise((resolve, reject) => {
    resolveDone = resolve;
    rejectDone = reject;
  });
  Promise.resolve().then(() => {
    if (!transitionScheduler && !suspenseContext) {
      try {
        batch(compute);
        resolveDone();
      } catch (error) {
        rejectDone(error);
      }
      return;
    }
    const suspenseDriven = !transitionScheduler && !!suspenseContext;
    const pendingSignal = transitionPair();
    const setPending = (value) => signalSet(pendingSignal, value, true);
    const transition = [
      done,
      new Map(),
      new Map(),
      new Set(),
      false,
      false,
      setPending,
      resolveDone,
      suspenseDriven,
    ];
    activeTransition = transition;
    try {
      batch(compute);
    } catch (error) {
      activeTransition = undefined;
      rejectDone(error);
      return;
    }
    activeTransition = undefined;
    setPending(true);
    try {
      const scheduler = transitionScheduler ?? ((callback) => callback());
      scheduler(() => {
        try {
          committingTransition = transition;
          batch(() => {
            for (const [signal, value] of transition[2])
              signalSet(signal, value);
          });
          committingTransition = undefined;
          transition[4] = true;
          if (transition[8] && transition[3].size === 0) {
            queueMicrotask(() =>
              queueMicrotask(() => finishTransition(transition)),
            );
          } else finishTransition(transition);
        } catch (error) {
          committingTransition = undefined;
          transition[5] = true;
          setPending(false);
          rejectDone(error);
        }
      });
    } catch (error) {
      transition[5] = true;
      setPending(false);
      rejectDone(error);
    }
  });
  return done;
}

export function useTransition() {
  return [accessorFor(transitionPair()), startTransition];
}

export function observable(input) {
  const observableSymbol = Symbol.observable || "@@observable";
  return {
    subscribe(observer) {
      if (!(observer instanceof Object))
        throw TypeError("Expected the observer to be an object.");
      const handler =
        typeof observer === "function"
          ? observer
          : observer.next?.bind(observer);
      if (!handler) return { unsubscribe() {} };
      const dispose = createRoot((rootDispose) => {
        createEffect(() => {
          const value = input();
          untrack(() => handler(value));
        });
        return rootDispose;
      });
      if (lilGetOwner()) onCleanup(dispose);
      return { unsubscribe: dispose };
    },
    [observableSymbol]() {
      return this;
    },
  };
}

export function from(producer, initialValue = undefined) {
  const [value, setValue] = createSignal(initialValue, { equals: false });
  if (producer && typeof producer.subscribe === "function") {
    const subscription = producer.subscribe((next) => setValue(() => next));
    onCleanup(() => {
      if (typeof subscription === "function") subscription();
      else subscription?.unsubscribe?.();
    });
  } else {
    onCleanup(producer((next) => setValue(() => next)));
  }
  return value;
}

export const batch = IS_DEV
  ? (callback) => {
      devBatchDepth += 1;
      try {
        return lilBatch(callback);
      } finally {
        devBatchDepth -= 1;
        flushDevUpdate();
      }
    }
  : lilBatch;

export function untrack(callback) {
  return externalSourceConfig
    ? lilUntrack(() => externalSourceConfig.untrack(callback))
    : lilUntrack(callback);
}

export const onCleanup = lilOnCleanup;

export const createUniqueId = lilCreateUniqueId;

export function requestCallback(callback, options) {
  ensureCallbackScheduling();
  return lilRequestCallback(
    callback,
    typeof options === "number" ? options : (options?.timeout ?? 1073741823),
  );
}

export const cancelCallback = lilCancelCallback;
