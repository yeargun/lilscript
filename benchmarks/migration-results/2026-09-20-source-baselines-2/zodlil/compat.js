import { createRequire } from "node:module"

const require = createRequire(import.meta.url)

export function applyCompat(z) {
  const util = {
    isPlainObject(o) {
      if (o === null || typeof o !== "object") return false
      const ctor = o.constructor
      if (ctor === undefined) return true
      if (typeof ctor !== "function") return true
      const prot = ctor.prototype
      if (prot === null || typeof prot !== "object") return false
      return Object.prototype.hasOwnProperty.call(prot, "isPrototypeOf")
    },
    shallowClone(o) {
      if (util.isPlainObject(o)) return { ...o }
      if (Array.isArray(o)) return [...o]
      if (o instanceof Map) return new Map(o)
      if (o instanceof Set) return new Set(o)
      return o
    },
    floatSafeRemainder(val, step) {
      const valDec = (val.toString().split(".")[1] || "").length
      const stepDec = (step.toString().split(".")[1] || "").length
      const mul = 10 ** Math.max(valDec, stepDec)
      return (Math.round(val * mul) % Math.round(step * mul)) / mul
    },
    jsonStringifyReplacer(_, value) {
      return typeof value === "bigint" ? value.toString() : value
    },
    nullish(input) {
      return input === null || input === undefined
    },
    prefixIssues(path, issues) {
      return issues.map((iss) => ({ ...iss, path: [...path, ...(iss.path ?? [])] }))
    },
    issue(arg, input, inst) {
      if (typeof arg === "string") {
        return { code: "custom", message: arg, input, inst, path: [] }
      }
      return arg
    },
    cleanEnum(obj) {
      return Object.entries(obj)
        .filter(([k]) => Number.isNaN(Number.parseInt(k, 10)))
        .map((el) => el[1])
    },
    getEnumValues(entries) {
      const numericValues = Object.values(entries).filter((v) => typeof v === "number")
      return Object.entries(entries)
        .filter(([k]) => numericValues.indexOf(+k) === -1)
        .map(([, v]) => v)
    },
    joinValues(array, separator = "|") {
      return array.map((val) => (typeof val === "string" ? `"${val}"` : String(val))).join(separator)
    },
    cached(getter) {
      return {
        get value() {
          const value = getter()
          Object.defineProperty(this, "value", { value })
          return value
        },
      }
    },
    assertNever() {
      throw new Error("Unexpected value in exhaustive check")
    },
    assert() {},
    assertIs() {},
    assertEqual(val) {
      return val
    },
    toZod() {
      return (schema) => schema
    },
  }

  z.core = z.core ?? {}
  z.core.util = Object.assign(z.core.util ?? {}, util)
  z.core.$ZodError = z.ZodError
  z.core.config = z.config
  z.core.globalConfig = z.core.globalConfig ?? {}
  z.core.globalRegistry = z.globalRegistry
  z.core.registry = z.registry
  z.core.parse = z.parse
  z.core.safeParse = z.safeParse
  z.core.clone = (inst) => inst.clone()
  z.core.$constructor = (name, init) => {
    const ctor = function (def) {
      const inst = Object.create(ctor.prototype)
      init(inst, def ?? {})
      return inst
    }
    Object.defineProperty(ctor, "name", { value: name })
    return ctor
  }

  z.float32 = z.float32 ?? ((params) => z.number(params))
  z.float64 = z.float64 ?? ((params) => z.number(params))
  z.int32 = z.int32 ?? ((params) => z.number(params).int())
  z.uint32 = z.uint32 ?? ((params) => z.number(params).int().nonnegative())
  z.int64 = z.int64 ?? ((params) => z.bigint(params))
  z.uint64 = z.uint64 ?? ((params) => z.bigint(params))
  z.describe = z.describe ?? ((schema, desc) => schema.describe(desc))
  z.meta = z.meta ?? ((schema, value) => schema.meta(value))
  z.mac = z.mac ?? ((params) => z.string(params).mac(params))
  z.e164 = z.e164 ?? ((params) => z.string(params).e164(params))
  z.cidrv4 = z.cidrv4 ?? ((params) => z.string(params).cidrv4(params))
  z.cidrv6 = z.cidrv6 ?? ((params) => z.string(params).cidrv6(params))
  z.base64 = z.base64 ?? ((params) => z.string(params).base64(params))
  z.base64url = z.base64url ?? ((params) => z.string(params).base64url(params))
  z.xid = z.xid ?? ((params) => z.string(params).xid(params))
  z.ksuid = z.ksuid ?? ((params) => z.string(params).ksuid(params))
  z.cuid = z.cuid ?? ((params) => z.string(params).cuid(params))
  z.cuid2 = z.cuid2 ?? ((params) => z.string(params).cuid2(params))
  z.emoji = z.emoji ?? ((params) => z.string(params).emoji(params))
  z.uuidv4 = z.uuidv4 ?? ((params) => z.string(params).uuidv4(params))
  z.uuidv6 = z.uuidv6 ?? ((params) => z.string(params).uuidv6(params))
  z.uuidv7 = z.uuidv7 ?? ((params) => z.string(params).uuidv7(params))
  z.stringFormat = z.stringFormat ?? ((format, fnOrRegex, params) => {
    const regex = fnOrRegex instanceof RegExp ? fnOrRegex : undefined
    const fn = typeof fnOrRegex === "function" ? fnOrRegex : undefined
    if (regex) return z.string(params).regex(regex)
    if (fn) return z.string(params).refine(fn)
    return z.string(params)
  })
  z.creditCard = z.creditCard ?? ((params) => {
    return z
      .string(params)
      .regex(/^\d(?:[ -]?\d){11,18}$/, params)
      .refine((value) => {
        const digits = String(value).replace(/\D/g, "")
        let sum = 0
        let alt = false
        for (let i = digits.length - 1; i >= 0; i--) {
          let n = digits.charCodeAt(i) - 48
          if (alt) {
            n *= 2
            if (n > 9) n -= 9
          }
          sum += n
          alt = !alt
        }
        return sum % 10 === 0
      }, params)
  })
  z.templateLiteral = z.templateLiteral ?? ((parts, params) => {
    const source = parts
      .map((part) => {
        if (typeof part === "string") return part.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")
        if (typeof part === "number" || typeof part === "bigint" || typeof part === "boolean") return String(part)
        const type = part?.def?.type ?? part?._zod?.def?.type
        if (type === "string") return ".*"
        if (type === "number") return "-?\\d+(?:\\.\\d+)?"
        if (type === "bigint") return "-?\\d+"
        if (type === "boolean") return "true|false"
        if (type === "null") return "null"
        if (type === "undefined") return "undefined"
        if (type === "literal") {
          const values = part.def?.values ?? part._zod?.def?.values ?? []
          return values.map((v) => String(v).replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|")
        }
        if (type === "enum") {
          const values = part.def?.values ?? part.options ?? []
          return values.map((v) => String(v).replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|")
        }
        return ".*"
      })
      .join("")
    return z.string(params).regex(new RegExp(`^${source}$`))
  })
  z.deepPartial = z.deepPartial ?? ((schema) => deepPartial(z, schema))
  z.fromJSONSchema = (json, params) => fromJson(z, json, params)
  z.safeEncode = z.safeEncode ?? ((schema, data, ctx) => schema.safeParse(data, { ...(ctx ?? {}), direction: "backward" }))
  z.encode = z.encode ?? ((schema, data, ctx) => schema.encode(data, ctx))
  z.decode = z.decode ?? ((schema, data, ctx) => schema.decode(data, ctx))
  z.safeDecode = z.safeDecode ?? ((schema, data, ctx) => schema.safeParse(data, ctx))
  z.encodeAsync = z.encodeAsync ?? ((schema, data, ctx) => schema.encodeAsync(data, ctx))
  z.decodeAsync = z.decodeAsync ?? ((schema, data, ctx) => schema.decodeAsync(data, ctx))
  z.safeEncodeAsync = z.safeEncodeAsync ?? ((schema, data, ctx) => schema.safeParse(data, { ...(ctx ?? {}), direction: "backward" }).then ? schema.safeParseAsync(data, { ...(ctx ?? {}), direction: "backward" }) : Promise.resolve(schema.safeParse(data, { ...(ctx ?? {}), direction: "backward" })))
  z.safeDecodeAsync = z.safeDecodeAsync ?? ((schema, data, ctx) => schema.safeParseAsync(data, ctx))
  z.setErrorMap = (fn) => z.config({ customError: fn })
  z.compile = z.compile ?? ((schema) => schema)
  z.core.clone = (inst, def, params) => {
    const Ctor = inst._zod?.constr ?? inst.constructor
    const cl = new Ctor(def ?? inst._zod?.def ?? inst.def)
    if (params?.parent) cl._zod.parent = inst
    return cl
  }

  if (z.ZodTuple?.prototype && !z.ZodTuple.prototype.rest) {
    Object.defineProperty(z.ZodTuple.prototype, "rest", {
      configurable: true,
      value(rest) {
        const items = this.def?.items ?? this._zod?.def?.items
        return z.tuple(items ?? [], rest)
      },
    })
  }

  try {
    const locales = require("zod/v4/locales")
    z.locales = { ...z.locales, ...(locales.default ?? locales) }
  } catch {}

  return z
}

function walkJson(schema, params, seen) {
  if (!schema) return {}
  if (seen.has(schema)) return { $ref: seen.get(schema) }
  const def = schema.def ?? schema._zod?.def ?? {}
  const type = def.type ?? schema.type
  const json = innerWalk(schema, def, type, params, seen)
  if (params?.io !== "input") {
    json.$schema ??= "https://json-schema.org/draft/2020-12/schema"
  }
  return json
}

function innerWalk(schema, def, type, params, seen) {
  if (type === "string") return { type: "string" }
  if (type === "number") return { type: "number" }
  if (type === "boolean") return { type: "boolean" }
  if (type === "bigint") return { type: "integer" }
  if (type === "null") return { type: "null" }
  if (type === "undefined" || type === "void") return { not: {} }
  if (type === "any" || type === "unknown") return {}
  if (type === "never") return { not: {} }
  if (type === "date") return { type: "string", format: "date-time" }
  if (type === "array") return { type: "array", items: stripSchema(walkJson(def.element ?? schema.element, params, seen)) }
  if (type === "object") {
    const properties = {}
    const required = []
    const shape = def.shape ?? schema.shape ?? {}
    for (const key of Object.keys(shape)) {
      const field = shape[key]
      const fieldType = field?.def?.type ?? field?.type
      const inner = fieldType === "optional" ? field.def?.innerType ?? field.unwrap?.() : field
      properties[key] = stripSchema(walkJson(inner ?? field, params, seen))
      if (fieldType !== "optional") required.push(key)
    }
    const json = { type: "object", properties, additionalProperties: false }
    if (required.length) json.required = required
    return json
  }
  if (type === "union") return { anyOf: (def.options ?? schema.options ?? []).map((opt) => stripSchema(walkJson(opt, params, seen))) }
  if (type === "intersection") return { allOf: [stripSchema(walkJson(def.left, params, seen)), stripSchema(walkJson(def.right, params, seen))] }
  if (type === "literal") {
    const values = def.values ?? []
    return values.length === 1 ? { const: values[0] } : { enum: values }
  }
  if (type === "enum") {
    const values = def.values ?? schema.options ?? []
    return { enum: [...values] }
  }
  if (type === "optional" || type === "nullable" || type === "default" || type === "prefault" || type === "catch") {
    return stripSchema(walkJson(def.innerType, params, seen))
  }
  if (type === "pipe") return stripSchema(walkJson(def.out ?? def.in, params, seen))
  if (type === "lazy") {
    try {
      return stripSchema(walkJson(def.getter?.() ?? schema.unwrap?.(), params, seen))
    } catch {
      return {}
    }
  }
  if (type === "record") return { type: "object", additionalProperties: stripSchema(walkJson(def.valueType, params, seen)) }
  if (type === "tuple") return { type: "array", prefixItems: (def.items ?? []).map((item) => stripSchema(walkJson(item, params, seen))) }
  return {}
}

function stripSchema(json) {
  if (!json || typeof json !== "object") return json
  const next = { ...json }
  delete next.$schema
  return next
}

function deepPartial(z, schema, seen = new Map()) {
  if (seen.has(schema)) return seen.get(schema)
  const type = schema?.def?.type ?? schema?.type
  if (type === "object") {
    const next = schema.partial()
    seen.set(schema, next)
    return next
  }
  if (type === "array") return z.array(deepPartial(z, schema.element ?? schema.def?.element, seen))
  if (type === "optional") return z.optional(deepPartial(z, schema.unwrap(), seen))
  if (type === "nullable") return z.nullable(deepPartial(z, schema.unwrap(), seen))
  if (type === "union") return z.union((schema.options ?? schema.def?.options ?? []).map((opt) => deepPartial(z, opt, seen)))
  if (type === "lazy") return z.lazy(() => deepPartial(z, schema.unwrap(), seen))
  return schema
}

function fromJson(z, json) {
  if (!json || typeof json !== "object") return z.unknown()
  if (json.const !== undefined) return z.literal(json.const)
  if (json.enum) return z.enum(json.enum.map(String))
  if (json.anyOf) return z.union(json.anyOf.map((item) => fromJson(z, item)))
  if (json.allOf) return json.allOf.map((item) => fromJson(z, item)).reduce((a, b) => a.and(b))
  if (json.type === "string") {
    let s = z.string()
    if (json.format === "credit_card") s = s.check(z.creditCard())
    else if (json.format && typeof z[json.format] === "function") s = s.check(z[json.format]())
    if (typeof json.minLength === "number") s = s.min(json.minLength)
    if (typeof json.maxLength === "number") s = s.max(json.maxLength)
    if (json.pattern) s = s.regex(new RegExp(json.pattern))
    return s
  }
  if (json.type === "number" || json.type === "integer") return z.number()
  if (json.type === "boolean") return z.boolean()
  if (json.type === "null") return z.null()
  if (json.type === "array") return z.array(fromJson(z, json.items ?? {}))
  if (json.type === "object") {
    const shape = {}
    for (const [key, value] of Object.entries(json.properties ?? {})) {
      const field = fromJson(z, value)
      shape[key] = json.required?.includes(key) ? field : field.optional()
    }
    return z.object(shape)
  }
  return z.unknown()
}
