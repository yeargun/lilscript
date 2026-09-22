import {
  createContext as createLilContext,
  createJsSignal,
  createRenderEffect as createLilRenderEffect,
  createRoot,
  createUniqueId,
  createUserEffect,
  enablePendingThrows,
  flush,
  getOwner as getOwnerId,
  onCleanup,
  signalCommit,
  signalGet,
  signalMarkPending,
  signalPeek,
  storeVersion,
  untrack,
  useContext as useLilContext,
  createSelector,
  selectorMatch,
} from "./core.js"
import {
  action,
  children,
  createEffect,
  createMemo,
  createOptimistic,
  createOptimisticStore,
  createSignal,
  createStore as createTupleStore,
  getOwner,
  isPending,
  latest,
  lazy,
  onSettled,
  runWithOwner,
} from "./compat.js"

export {
  action,
  children,
  createEffect,
  createMemo,
  createOptimistic,
  createOptimisticStore,
  createRoot,
  createSelector,
  createSignal,
  selectorMatch,
  createUniqueId,
  flush,
  getOwner,
  isPending,
  latest,
  lazy,
  onCleanup,
  onSettled,
  runWithOwner,
  untrack,
}

export const $DEVCOMP = Symbol("dev-component")
export const $PROXY = Symbol("solid-proxy")
export const $REFRESH = Symbol("solid-refresh")
export const $TRACK = Symbol("solid-track")
export const DEV = undefined
export const NoHydrateContext = createLilContext(false)

export class NotReadyError extends Error {
  constructor(message = "NotReady") {
    super(message)
    this.name = "NotReadyError"
  }
}

export const sharedConfig = {
  hydrating: false,
  done: true,
  resources: {},
  events: null,
  registry: new Map(),
  completed: null,
  getNextContextId() {
    nextChildId += 1
    return String(nextChildId)
  },
  isHydrationInProgress() {
    return sharedConfig.hydrating && !sharedConfig.done
  },
  onHydrationEnd(callback) {
    if (!sharedConfig.hydrating || sharedConfig.done) {
      queueMicrotask(callback)
      return
    }
    hydrationEndCallbacks.push(callback)
  },
}

const hydrationEndCallbacks = []
let nextChildId = 0
let loadingEnforced = false
const externalSources = []
const revealStack = []

function isThenable(value) {
  return value != null && typeof value.then === "function"
}

function unwrap(value) {
  if (typeof value === "function") return value()
  if (value && typeof value.read === "function") return value.read()
  return value
}

function clonePlain(value, seen = new WeakMap()) {
  if (value == null || typeof value !== "object") return value
  if (seen.has(value)) return seen.get(value)
  if (Array.isArray(value)) {
    const copy = []
    seen.set(value, copy)
    for (const item of value) copy.push(clonePlain(item, seen))
    return copy
  }
  const copy = {}
  seen.set(value, copy)
  for (const key of Object.keys(value)) copy[key] = clonePlain(value[key], seen)
  return copy
}

function keyOf(options) {
  if (options == null) return (item) => item?.id
  if (options === null) return null
  if (typeof options === "function") return options
  if (typeof options === "string") return (item) => item?.[options]
  if (options.key === null) return null
  if (typeof options.key === "function") return options.key
  if (typeof options.key === "string") return (item) => item?.[options.key]
  return (item) => item?.id
}

function mergeReconcile(target, source, keyFn) {
  if (source == null || typeof source !== "object") return source
  if (target == null || typeof target !== "object") return clonePlain(source)
  if (Array.isArray(source)) {
    const next = []
    const used = new Set()
    for (let index = 0; index < source.length; index += 1) {
      const incoming = source[index]
      let previous
      if (keyFn && incoming != null && typeof incoming === "object") {
        const key = keyFn(incoming)
        previous = target.find((item, itemIndex) => {
          if (used.has(itemIndex) || item == null || typeof item !== "object") return false
          return keyFn(item) === key
        })
        if (previous != null) used.add(target.indexOf(previous))
      } else if (index < target.length) {
        previous = target[index]
        used.add(index)
      }
      next.push(mergeReconcile(previous, incoming, keyFn))
    }
    return next
  }
  const next = { ...target }
  for (const key of Object.keys(source)) {
    next[key] = mergeReconcile(target[key], source[key], keyFn)
  }
  for (const key of Object.keys(next)) {
    if (!(key in source)) delete next[key]
  }
  return next
}

export function isEqual(left, right) {
  return Object.is(left, right)
}

export function isWrappable(value) {
  return value != null && typeof value === "object"
}

export function isDisposed(owner = getOwnerId()) {
  return owner < 0
}

export function getObserver() {
  return getOwnerId()
}

export function getNextChildId() {
  nextChildId += 1
  return String(nextChildId)
}

export function createOwner() {
  let owner = -1
  createRoot(() => {
    owner = getOwnerId()
  })
  return owner
}

const REQUIRED_CONTEXT = Symbol("solid-context")

export function createContext(defaultValue, options) {
  if (arguments.length === 0) return createLilContext(REQUIRED_CONTEXT)
  return createLilContext(defaultValue)
}

export function useContext(context) {
  const value = useLilContext(context)
  if (value === REQUIRED_CONTEXT) throw new Error("ContextNotFoundError")
  return value
}

export function createStore(initial, seed, options) {
  if (typeof initial === "function") {
    const projected = createProjection(initial, seed ?? {}, options)
    return [projected, (updater) => {
      const current = unwrap(projected)
      if (typeof updater === "function") updater(current)
      else Object.assign(current, updater)
    }]
  }
  return createTupleStore(initial)
}

export function createProjection(fn, seed, options) {
  const extract = keyOf(options)
  const [store, setStore] = createTupleStore(clonePlain(unwrap(seed) ?? {}))
  const run = () => {
    const draft = untrack(() => clonePlain(store()))
    const result = fn(draft)
    const commit = (value) => {
      const next = value === undefined ? draft : value
      untrack(() => setStore(mergeReconcile(store(), next, extract)))
    }
    if (result && typeof result[Symbol.asyncIterator] === "function") {
      ;(async () => {
        for await (const value of result) commit(value)
      })()
      return
    }
    if (isThenable(result)) {
      result.then(commit)
      return
    }
    commit(result)
  }
  createRenderEffect(run)
  const read = () => store()
  read.refresh = run
  return new Proxy(read, {
    apply(_target, _this, args) {
      return read(...args)
    },
    get(_target, key) {
      if (key === "refresh") return run
      if (key === $TRACK || key === $PROXY) return read
      const value = read()
      return value == null ? undefined : value[key]
    },
  })
}

export function reconcile(value, key = "id") {
  const extract = keyOf(key)
  return (state) => {
    const merged = mergeReconcile(state, value, extract)
    if (Array.isArray(state) && Array.isArray(merged)) {
      state.length = 0
      state.push(...merged)
      return
    }
    if (state && typeof state === "object" && merged && typeof merged === "object") {
      for (const name of Object.keys(state)) {
        if (!(name in merged)) delete state[name]
      }
      Object.assign(state, merged)
    }
  }
}

export function snapshot(item) {
  return clonePlain(unwrap(item))
}

export function deep(store) {
  if (store && typeof storeVersion === "function") {
    try { storeVersion(store) } catch {}
  }
  if (typeof store === "function") store()
  return clonePlain(unwrap(store))
}

export function merge(...sources) {
  return new Proxy({}, {
    get(_target, key) {
      if (key === $PROXY || key === $TRACK) return true
      for (let index = sources.length - 1; index >= 0; index -= 1) {
        const source = typeof sources[index] === "function" ? sources[index]() : sources[index]
        if (source && key in source) return source[key]
      }
    },
    has(_target, key) {
      return sources.some((source) => {
        const value = typeof source === "function" ? source() : source
        return value && key in value
      })
    },
    ownKeys() {
      const keys = new Set()
      for (const source of sources) {
        const value = typeof source === "function" ? source() : source
        if (value) Object.keys(value).forEach((key) => keys.add(key))
      }
      return [...keys]
    },
    getOwnPropertyDescriptor(_target, key) {
      if (this.has(_target, key)) return { configurable: true, enumerable: true, value: this.get(_target, key) }
    },
  })
}

export function omit(props, ...keys) {
  const hidden = new Set(keys)
  return new Proxy(props, {
    get(target, key) {
      if (hidden.has(key)) return undefined
      return target[key]
    },
    has(target, key) {
      return !hidden.has(key) && key in target
    },
    ownKeys(target) {
      return Reflect.ownKeys(target).filter((key) => !hidden.has(key))
    },
  })
}

export function storePath(...parts) {
  const setter = parts.pop()
  return (state) => {
    let cursor = state
    for (let index = 0; index < parts.length - 1; index += 1) {
      cursor = applyPart(cursor, parts[index])
    }
    const last = parts[parts.length - 1]
    if (setter === storePath.DELETE) {
      if (typeof last === "string" || typeof last === "number") delete cursor[last]
      return
    }
    const next = typeof setter === "function" ? setter(cursor[last]) : setter
    if (typeof last === "function" && Array.isArray(cursor)) {
      cursor.forEach((item, index) => {
        if (last(item, index)) Object.assign(item, typeof setter === "function" ? setter(item) : setter)
      })
      return
    }
    cursor[last] = next
  }
}
storePath.DELETE = Symbol("storePath.DELETE")

function applyPart(cursor, part) {
  if (cursor == null) return cursor
  if (typeof part === "string" || typeof part === "number") return cursor[part]
  if (Array.isArray(part)) return part.map((key) => cursor[key])
  return cursor
}

export function createDeepProxy(value) {
  if (!isWrappable(value)) return value
  return new Proxy(value, {
    get(target, key) {
      if (key === $PROXY) return true
      const next = target[key]
      return isWrappable(next) ? createDeepProxy(next) : next
    },
  })
}

export function flatten(value, options = {}) {
  if (typeof value === "function" && !options.doNotUnwrap) value = value()
  if (Array.isArray(value)) {
    return value.flatMap((item) => flatten(item, options)).filter((item) => {
      if (!options.skipNonRendered) return true
      return item != null && item !== true && item !== false && item !== ""
    })
  }
  if (options.skipNonRendered && (value == null || value === true || value === false || value === "")) {
    return []
  }
  return value
}

export function mapArray(list, map, options = {}) {
  const keyed = options.keyed
  let prevItems = []
  let prevMapped = []
  let prevKeys = []
  return createMemo(() => {
    const items = typeof list === "function" ? list() : list
    if (!items || items.length === 0) {
      prevItems = []
      prevMapped = []
      prevKeys = []
      return options.fallback ? [options.fallback()] : []
    }
    const next = []
    const nextKeys = []
    const used = new Set()
    for (let index = 0; index < items.length; index += 1) {
      const item = items[index]
      const key = keyed === false
        ? index
        : typeof keyed === "function"
          ? keyed(item)
          : item
      nextKeys.push(key)
      let found = -1
      for (let cursor = 0; cursor < prevKeys.length; cursor += 1) {
        if (!used.has(cursor) && Object.is(prevKeys[cursor], key)) {
          found = cursor
          break
        }
      }
      if (found >= 0) {
        used.add(found)
        next.push(prevMapped[found])
      } else {
        const indexOf = () => index
        if (keyed === false) next.push(map(() => item, index))
        else if (typeof keyed === "function") next.push(map(() => item, indexOf))
        else next.push(map(item, indexOf))
      }
    }
    prevItems = items
    prevMapped = next
    prevKeys = nextKeys
    return next
  })
}

export function repeat(count, map, options = {}) {
  return createMemo(() => {
    const total = typeof count === "function" ? count() : count
    const from = options.from ? options.from() ?? 0 : 0
    if (!total) return options.fallback ? [options.fallback()] : []
    return Array.from({ length: total }, (_item, index) => map(from + index))
  })
}

export function createTrackedEffect(compute) {
  createUserEffect(() => {
    const cleanup = compute()
    if (typeof cleanup === "function") onCleanup(cleanup)
  })
}

export function createReaction(effectFn, options) {
  return (tracking) => {
    createEffect(
      () => {
        tracking()
        return true
      },
      () => {
        if (typeof effectFn === "function") effectFn()
        else effectFn.effect()
      },
      options,
    )
  }
}

export function createRenderEffect(compute, apply) {
  if (typeof apply === "function") {
    createLilRenderEffect(() => apply(compute()))
    return
  }
  createLilRenderEffect(compute)
}

export function refresh(target) {
  if (target && typeof target.refresh === "function") {
    target.refresh()
    return
  }
  if (typeof target === "function" && target.refresh) {
    target.refresh()
  }
}

export function resolve(fn) {
  return new Promise((ok, fail) => {
    const attempt = () => {
      enablePendingThrows(true)
      try {
        const value = fn()
        if (isThenable(value)) {
          value.then(ok, fail)
          return
        }
        ok(value)
      } catch (error) {
        if (error === "NotReady" || error instanceof NotReadyError) {
          onSettled(() => attempt())
          return
        }
        fail(error)
      } finally {
        enablePendingThrows(false)
      }
    }
    attempt()
  })
}

export function affects(target) {
  if (target && target._signal) signalMarkPending(target._signal, true)
  else if (typeof target === "function" && target._signal) signalMarkPending(target._signal, true)
}

export function enableExternalSource(factory) {
  externalSources.push(factory)
}

export function enforceLoadingBoundary(enabled = true) {
  loadingEnforced = enabled
}

export function enableHydration() {
  sharedConfig.hydrating = true
  sharedConfig.done = false
}

export function finishHydration() {
  sharedConfig.hydrating = false
  sharedConfig.done = true
  while (hydrationEndCallbacks.length) hydrationEndCallbacks.shift()()
}

export function ssrHandleError() {}
export function ssrScope(fn) {
  return fn
}
export function runInServerComponentScope(fn) {
  return fn()
}
export function inServerComponentScope() {
  return false
}
export function creationStamp() {
  return 0
}
export function getProjectionTrace() {
  return undefined
}
export function materializeContainerTrace() {
  return createTupleStore({})[0]
}

export function createErrorBoundary(fn, fallback) {
  const [error, setError] = createSignal()
  const [failed, setFailed] = createSignal(false)
  const reset = () => {
    setError()
    setFailed(false)
    flush()
  }
  return () => {
    if (failed()) return fallback(() => error(), reset)
    try {
      return fn()
    } catch (fault) {
      setError(fault)
      setFailed(true)
      flush()
      return fallback(() => error(), reset)
    }
  }
}

function currentReveal() {
  return revealStack[revealStack.length - 1]
}

export function createRevealOrder(fn, options = {}) {
  const orderOf = typeof options.order === "function" ? options.order : () => options.order ?? "sequential"
  const collapsedOf = typeof options.collapsed === "function" ? options.collapsed : () => !!options.collapsed
  const version = createJsSignal(0)
  const controller = {
    slots: [],
    orderOf,
    collapsedOf,
    version,
    bump() {
      signalCommit(version, signalPeek(version) + 1)
    },
  }
  revealStack.push(controller)
  try {
    return fn()
  } finally {
    revealStack.pop()
  }
}

export function registerRevealSlot() {
  const controller = currentReveal()
  if (!controller) {
    return {
      allowed() { return true },
      setReady() {},
    }
  }
  const slot = { ready: false }
  controller.slots.push(slot)
  return {
    allowed() {
      signalGet(controller.version)
      const order = controller.orderOf() ?? "sequential"
      if (order === "natural") return true
      if (order === "together") return controller.slots.every((item) => item.ready)
      const index = controller.slots.indexOf(slot)
      return controller.slots.slice(0, index).every((item) => item.ready)
    },
    setReady(value) {
      if (slot.ready === value) return
      slot.ready = value
      controller.bump()
    },
  }
}

export function createLoadingBoundary(fn, fallback, options) {
  const slot = registerRevealSlot()
  return () => {
    if (!slot.allowed()) return fallback()
    enablePendingThrows(true)
    try {
      const value = fn()
      slot.setReady(true)
      return value
    } catch (error) {
      slot.setReady(false)
      return fallback()
    } finally {
      enablePendingThrows(false)
    }
  }
}

export function For(props) {
  return mapArray(
    () => props.each,
    props.children,
    { keyed: props.keyed, fallback: props.fallback == null ? undefined : () => props.fallback },
  )
}

export function Repeat(props) {
  return repeat(
    () => props.count,
    typeof props.children === "function" ? props.children : () => props.children,
    { from: props.from == null ? undefined : () => props.from, fallback: props.fallback == null ? undefined : () => props.fallback },
  )
}

export function Show(props) {
  return () => {
    const when = typeof props.when === "function" ? props.when() : props.when
    if (when) {
      if (typeof props.children === "function") {
        return props.keyed ? props.children(when) : props.children(() => when)
      }
      return props.children
    }
    return typeof props.fallback === "function" ? props.fallback() : props.fallback
  }
}

export function Switch(props) {
  return () => {
    const matches = flatten(props.children, { skipNonRendered: true })
    const list = Array.isArray(matches) ? matches : [matches]
    for (const match of list) {
      const when = typeof match?.when === "function" ? match.when() : match?.when
      if (when) {
        if (typeof match.children === "function") {
          return match.keyed ? match.children(when) : match.children(() => when)
        }
        return match.children
      }
    }
    return typeof props.fallback === "function" ? props.fallback() : props.fallback
  }
}

export function Match(props) {
  return props
}

export function Errored(props) {
  return createErrorBoundary(
    () => (typeof props.children === "function" ? props.children() : props.children),
    (error, reset) => (typeof props.fallback === "function" ? props.fallback(error, reset) : props.fallback),
  )
}

export function Loading(props) {
  return createLoadingBoundary(
    () => (typeof props.children === "function" ? props.children() : props.children),
    () => (typeof props.fallback === "function" ? props.fallback() : props.fallback),
    { on: props.on },
  )
}

export function Reveal(props) {
  return createRevealOrder(
    () => (typeof props.children === "function" ? props.children() : props.children),
    { order: () => props.order ?? "sequential", collapsed: () => !!props.collapsed },
  )
}

export function Hydration(props) {
  return typeof props.children === "function" ? props.children() : props.children
}

export function NoHydration(props) {
  if (sharedConfig.hydrating) return undefined
  return typeof props.children === "function" ? props.children() : props.children
}

export function createComponent(Comp, props) {
  return untrack(() => Comp(props))
}
