function restack(err, callee) {
  try {
    Error.captureStackTrace(err, callee)
  } catch {}
  const lines = String(err.stack ?? "").split("\n")
  const header = []
  const frames = []
  for (const line of lines) {
    if (line.trim().startsWith("at ")) {
      if (line.includes("processTicksAndRejections")) continue
      if (line.includes("internal/process")) continue
      if (line.includes("async-api")) continue
      frames.push(line)
    } else {
      header.push(line)
    }
  }
  if (!header.length) header.push(`${err.name}: ${err.message}`)
  if (frames.length) err.stack = [...header, ...frames].join("\n")
}

function throwing2(runSafe) {
  let fn
  fn = function (data, params) {
    const result = runSafe.call(this, data, params)
    if (result?.success) return result.data
    if (result?.error) {
      restack(result.error, fn)
      throw result.error
    }
    return result
  }
  return fn
}

function throwing3(runSafe) {
  let fn
  fn = function (schema, data, ctx) {
    const result = runSafe(schema, data, ctx)
    if (result?.success) return result.data
    if (result?.error) {
      restack(result.error, fn)
      throw result.error
    }
    return result
  }
  return fn
}

function asyncThrowing2(runSafe) {
  let fn
  fn = async function (data, params) {
    const result = await runSafe.call(this, data, params)
    if (result?.success) return result.data
    if (result?.error) {
      restack(result.error, fn)
      throw result.error
    }
    return result
  }
  return fn
}

function asyncThrowing3(runSafe) {
  let fn
  fn = async function (schema, data, ctx) {
    const result = await runSafe(schema, data, ctx)
    if (result?.success) return result.data
    if (result?.error) {
      restack(result.error, fn)
      throw result.error
    }
    return result
  }
  return fn
}

function installOwn(proto, key, make) {
  Object.defineProperty(proto, key, {
    configurable: true,
    enumerable: true,
    get() {
      const self = this
      const fn = make(self)
      Object.defineProperty(self, key, {
        configurable: true,
        writable: true,
        enumerable: true,
        value: fn,
      })
      return fn
    },
    set(value) {
      Object.defineProperty(this, key, { configurable: true, writable: true, enumerable: true, value })
    },
  })
}

export function installAsyncApi(target) {
  target.parse = throwing3((schema, data, ctx) => target.safeParse(schema, data, ctx))
  target.encode = throwing3((schema, data, ctx) =>
    target.safeParse(schema, data, { ...(ctx ?? {}), direction: "backward" }),
  )
  target.decode = throwing3((schema, data, ctx) =>
    target.safeParse(schema, data, { ...(ctx ?? {}), direction: "forward" }),
  )
  target.parseAsync = asyncThrowing3((schema, data, ctx) => target.safeParseAsync(schema, data, ctx))
  target.encodeAsync = asyncThrowing3((schema, data, ctx) =>
    target.safeParseAsync(schema, data, { ...(ctx ?? {}), direction: "backward" }),
  )
  target.decodeAsync = asyncThrowing3((schema, data, ctx) =>
    target.safeParseAsync(schema, data, { ...(ctx ?? {}), direction: "forward" }),
  )
  const proto = target.ZodType?.prototype
  if (!proto) return
  installOwn(proto, "parse", (self) => throwing2((data, params) => self.safeParse(data, params)))
  installOwn(proto, "encode", (self) =>
    throwing2((data, params) => self.safeParse(data, { ...(params ?? {}), direction: "backward" })),
  )
  installOwn(proto, "decode", (self) =>
    throwing2((data, params) => self.safeParse(data, { ...(params ?? {}), direction: "forward" })),
  )
  installOwn(proto, "parseAsync", (self) => asyncThrowing2((data, params) => self.safeParseAsync(data, params)))
  installOwn(proto, "encodeAsync", (self) =>
    asyncThrowing2((data, params) => self.safeParseAsync(data, { ...(params ?? {}), direction: "backward" })),
  )
  installOwn(proto, "decodeAsync", (self) =>
    asyncThrowing2((data, params) => self.safeParseAsync(data, { ...(params ?? {}), direction: "forward" })),
  )
}
