const RESOLVING = Symbol("z.visit/resolving")

export function visit(z, schema, fnOrHandlers) {
  const fn =
    typeof fnOrHandlers === "function"
      ? fnOrHandlers
      : (node) => {
          const h = fnOrHandlers[node._zod.def.type]
          return h ? h(node) : node
        }

  const cache = new Map()

  function clone(inst, def) {
    const Ctor = inst._zod?.constr ?? inst.constructor
    return new Ctor(def)
  }

  function run(s) {
    const cached = cache.get(s)
    if (cached === RESOLVING) {
      return new z.ZodLazy({
        type: "lazy",
        getter: () => cache.get(s),
      })
    }
    if (cached !== undefined) return cached
    cache.set(s, RESOLVING)
    const mapped = fn(mapInner(s))
    cache.set(s, mapped)
    return mapped
  }

  function mapInner(s) {
    const def = s._zod.def
    const kind = def.type
    if (kind === "object") {
      const oldShape = def.shape ?? {}
      let changed = false
      const newShape = {}
      for (const k of Object.keys(oldShape)) {
        const mapped = run(oldShape[k])
        if (mapped !== oldShape[k]) changed = true
        newShape[k] = mapped
      }
      let newCatchall = def.catchall
      if (def.catchall) {
        newCatchall = run(def.catchall)
        if (newCatchall !== def.catchall) changed = true
      }
      return changed ? clone(s, { ...def, shape: newShape, catchall: newCatchall }) : s
    }
    if (kind === "array") {
      const mapped = run(def.element)
      return mapped === def.element ? s : clone(s, { ...def, element: mapped })
    }
    if (kind === "tuple") {
      const oldItems = def.items ?? []
      let changed = false
      const newItems = []
      for (const item of oldItems) {
        const mapped = run(item)
        if (mapped !== item) changed = true
        newItems.push(mapped)
      }
      let newRest = def.rest
      if (def.rest) {
        newRest = run(def.rest)
        if (newRest !== def.rest) changed = true
      }
      return changed ? clone(s, { ...def, items: newItems, rest: newRest }) : s
    }
    if (kind === "record" || kind === "map") {
      const newKey = run(def.keyType)
      const newVal = run(def.valueType)
      return newKey === def.keyType && newVal === def.valueType ? s : clone(s, { ...def, keyType: newKey, valueType: newVal })
    }
    if (kind === "set") {
      const newVal = run(def.valueType)
      return newVal === def.valueType ? s : clone(s, { ...def, valueType: newVal })
    }
    if (kind === "union") {
      const oldOptions = def.options ?? []
      let changed = false
      const newOptions = []
      for (const opt of oldOptions) {
        const mapped = run(opt)
        if (mapped !== opt) changed = true
        newOptions.push(mapped)
      }
      return changed ? clone(s, { ...def, options: newOptions }) : s
    }
    if (kind === "intersection") {
      const newLeft = run(def.left)
      const newRight = run(def.right)
      return newLeft === def.left && newRight === def.right ? s : clone(s, { ...def, left: newLeft, right: newRight })
    }
    if (
      kind === "optional" ||
      kind === "nullable" ||
      kind === "default" ||
      kind === "prefault" ||
      kind === "catch" ||
      kind === "readonly" ||
      kind === "nonoptional" ||
      kind === "promise" ||
      kind === "success"
    ) {
      const newInner = run(def.innerType)
      return newInner === def.innerType ? s : clone(s, { ...def, innerType: newInner })
    }
    if (kind === "pipe") {
      const newIn = run(def.in)
      const newOut = run(def.out)
      return newIn === def.in && newOut === def.out ? s : clone(s, { ...def, in: newIn, out: newOut })
    }
    if (kind === "function") {
      const newInput = run(def.input)
      const newOutput = run(def.output)
      return newInput === def.input && newOutput === def.output
        ? s
        : clone(s, { ...def, input: newInput, output: newOutput })
    }
    if (kind === "lazy") {
      const original = def.getter
      const rest = { ...def }
      delete rest._cachedInner
      return clone(s, { ...rest, getter: () => run(original()) })
    }
    return s
  }

  return run(schema)
}

export function installVisit(z) {
  z.core = z.core ?? {}
  z.core.visit = (schema, fnOrHandlers) => visit(z, schema, fnOrHandlers)
  z.visit = z.core.visit
  z.deepPartial = (schema) =>
    visit(z, schema, {
      object: (s) => s.partial(),
      union: (s) => {
        const def = s._zod.def
        return def.discriminator === undefined ? s : z.union(def.options)
      },
    })
}
