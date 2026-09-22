import {
  createContext,
  createIntMemo,
  createIntSignal,
  createJsSignal,
  createRenderEffect,
  createRoot,
  createSignal as createEqualsSignal,
  createStore as createLilStore,
  createUniqueId,
  createUserEffect,
  beginAction,
  endAction,
  flush,
  getOwner as getOwnerId,
  inAction,
  isPending as signalIsPending,
  latest as signalLatest,
  onCleanup,
  onSettled as lilOnSettled,
  provideContext,
  rememberOptimistic,
  rememberRevert,
  runWithOwner as runWithOwnerId,
  signalCommit,
  signalGet,
  signalMarkPending,
  signalPeek,
  signalPending,
  signalSet,
  signalUpdate,
  storeGet,
  storeId,
  storeReplace,
  storeSet,
  untrack,
  useContext,
} from "./core.js"

export {
  createContext,
  createRenderEffect,
  createRoot,
  createUniqueId,
  createUserEffect,
  flush,
  onCleanup,
  provideContext,
  storeGet,
  storeSet,
  untrack,
  useContext,
}

function equalsOf(options) {
  if (!options) return Object.is
  if (options.equals === false) return () => false
  if (typeof options.equals === "function") return options.equals
  return Object.is
}

function isThenable(value) {
  return value != null && typeof value.then === "function"
}

function accessorOf(signal) {
  const read = () => signalGet(signal)
  read._signal = signal
  return read
}

export function onSettled(callback) {
  lilOnSettled(() => {
    const cleanup = callback()
    if (typeof cleanup === "function") onCleanup(cleanup)
  })
}

export function getOwner() {
  return getOwnerId()
}

export function runWithOwner(owner, callback) {
  return runWithOwnerId(owner, callback)
}

export function isPending(source) {
  if (source && source._signal) return signalPending(source._signal)
  return signalIsPending(source)
}

export function latest(source) {
  if (source && source._signal) return signalPeek(source._signal)
  return signalLatest(source)
}

export function createSignal(initial, options) {
  const signal = typeof initial === "number" && Number.isInteger(initial) && !options
    ? createIntSignal(initial)
    : createEqualsSignal(initial, equalsOf(options))
  const write = (value) => {
    if (typeof value === "function") return signalUpdate(signal, value)
    return signalSet(signal, value)
  }
  return [accessorOf(signal), write]
}

export function createMemo(compute, options) {
  const equals = equalsOf(options)
  const signal = createEqualsSignal(undefined, equals)
  let epoch = 0
  const run = () => {
    const result = compute()
    if (isThenable(result)) {
      const token = ++epoch
      signalMarkPending(signal, true)
      result.then(
        (value) => {
          if (token !== epoch) return
          signalMarkPending(signal, false)
          signalCommit(signal, value)
        },
        (error) => {
          if (token !== epoch) return
          signalMarkPending(signal, false)
          throw error
        },
      )
      return
    }
    epoch += 1
    if (untrack(() => signalPending(signal))) signalMarkPending(signal, false)
    signalCommit(signal, result)
  }
  const start = () => {
    createRenderEffect(run)
  }
  if (options?.lazy) {
    const read = () => {
      if (!read._started) {
        read._started = true
        start()
      }
      return signalGet(signal)
    }
    read._signal = signal
    read.refresh = run
    return read
  }
  start()
  const read = accessorOf(signal)
  read.refresh = run
  return read
}

export function createEffect(compute, apply, options) {
  const effectOptions = typeof apply === "object" && apply != null ? apply : options
  const applyFn = typeof apply === "function"
    ? apply
    : typeof apply?.effect === "function"
      ? apply.effect
      : undefined
  const errorFn = effectOptions?.error
  let skip = !!effectOptions?.defer
  const run = () => {
    try {
      if (!applyFn) {
        compute()
        return
      }
      const value = compute()
      if (skip) {
        skip = false
        return
      }
      const cleanup = untrack(() => applyFn(value))
      if (typeof cleanup === "function") onCleanup(cleanup)
    } catch (error) {
      if (typeof errorFn === "function") errorFn(error)
      else throw error
    }
  }
  if (applyFn) createUserEffect(run)
  else createUserEffect(run)
}

export function createMemoInt(compute) {
  const signal = createIntMemo(compute)
  return accessorOf(signal)
}

export function jsSignal(initial) {
  const signal = createJsSignal(initial)
  return [accessorOf(signal), (value) => signalSet(signal, value)]
}

export function createStore(initial) {
  const store = createLilStore(initial)
  const read = () => storeGet(store)
  const write = (updater) => {
    if (typeof updater === "function") {
      const current = storeGet(store)
      const result = updater(current)
      if (result !== undefined && result !== current) storeReplace(store, result)
      else storeReplace(store, current)
      return
    }
    storeReplace(store, updater)
  }
  return [read, write]
}

export function createOptimistic(initial, options) {
  const signal = typeof initial === "number" && Number.isInteger(initial) && !options
    ? createIntSignal(initial)
    : createEqualsSignal(initial, equalsOf(options))
  const write = (value) => {
    rememberOptimistic(signal)
    if (typeof value === "function") return signalUpdate(signal, value)
    return signalSet(signal, value)
  }
  return [accessorOf(signal), write]
}

export function createOptimisticStore(initial) {
  const store = createLilStore(initial)
  const read = () => storeGet(store)
  const write = (updater) => {
    if (inAction()) {
      const snapshot = structuredClone(storeGet(store))
      rememberRevert(storeId(store), () => storeReplace(store, snapshot))
    }
    if (typeof updater === "function") {
      const current = storeGet(store)
      const result = updater(current)
      if (result !== undefined && result !== current) storeReplace(store, result)
      else storeReplace(store, current)
      return
    }
    storeReplace(store, updater)
  }
  return [read, write]
}

export function action(genFn) {
  return (...args) => {
    beginAction()
    const run = async () => {
      try {
        const iter = genFn(...args)
        if (iter && typeof iter.next === "function") {
          let step = await iter.next()
          while (!step.done) {
            flush()
            const yielded = await step.value
            step = await iter.next(yielded)
          }
          endAction(true)
          flush()
          return step.value
        }
        const result = await iter
        endAction(true)
        flush()
        return result
      } catch (error) {
        endAction(false)
        flush()
        throw error
      }
    }
    return run()
  }
}

export function children(fn) {
  const resolved = createMemo(fn)
  const read = () => resolved()
  read.toArray = () => {
    const value = read()
    if (value == null) return []
    return Array.isArray(value) ? value.flat(Infinity).filter((node) => node != null) : [value]
  }
  return read
}

export function lazy(loader) {
  let component
  let failure
  let started = false
  const status = createJsSignal(0)
  const start = () => {
    if (started) return
    started = true
    signalMarkPending(status, true)
    loader().then(
      (mod) => {
        component = mod.default
        signalMarkPending(status, false)
        signalCommit(status, 1)
      },
      (error) => {
        failure = error
        signalMarkPending(status, false)
        signalCommit(status, 2)
      },
    )
  }
  const Comp = (props) => {
    start()
    const pending = signalPending(status)
    if (failure) throw failure
    if (pending || !component) throw "NotReady"
    return component(props)
  }
  Comp.preload = () => {
    start()
    return Promise.resolve()
  }
  return Comp
}
