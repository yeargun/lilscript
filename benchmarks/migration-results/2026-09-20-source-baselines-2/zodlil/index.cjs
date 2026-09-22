var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __export = (target, all) => {
  for (var name in all)
    __defProp(target, name, { get: all[name], enumerable: true });
};
var __copyProps = (to, from, except, desc) => {
  if (from && typeof from === "object" || typeof from === "function") {
    for (let key of __getOwnPropNames(from))
      if (!__hasOwnProp.call(to, key) && key !== except)
        __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
  }
  return to;
};
var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);

// dist/index.js
var index_exports = {};
__export(index_exports, {
  $ZodError: () => $ZodError,
  NEVER: () => NEVER,
  TimePrecision: () => TimePrecision,
  ZodAny: () => ZodAny,
  ZodArray: () => ZodArray,
  ZodBigInt: () => ZodBigInt,
  ZodBoolean: () => ZodBoolean,
  ZodCatch: () => ZodCatch,
  ZodCodec: () => ZodCodec,
  ZodCustom: () => ZodCustom,
  ZodDate: () => ZodDate,
  ZodDefault: () => ZodDefault,
  ZodDiscriminatedUnion: () => ZodDiscriminatedUnion,
  ZodEnum: () => ZodEnum,
  ZodError: () => ZodError,
  ZodExactOptional: () => ZodExactOptional,
  ZodFile: () => ZodFile,
  ZodFunction: () => ZodFunction,
  ZodISODate: () => ZodISODate,
  ZodISODateTime: () => ZodISODateTime,
  ZodISODuration: () => ZodISODuration,
  ZodISOTime: () => ZodISOTime,
  ZodIntersection: () => ZodIntersection,
  ZodIssueCode: () => ZodIssueCode,
  ZodLazy: () => ZodLazy,
  ZodLiteral: () => ZodLiteral,
  ZodMap: () => ZodMap,
  ZodNaN: () => ZodNaN,
  ZodNever: () => ZodNever,
  ZodNonOptional: () => ZodNonOptional,
  ZodNull: () => ZodNull,
  ZodNullable: () => ZodNullable,
  ZodNumber: () => ZodNumber,
  ZodObject: () => ZodObject,
  ZodOptional: () => ZodOptional,
  ZodPipe: () => ZodPipe,
  ZodPrefault: () => ZodPrefault,
  ZodPreprocess: () => ZodPreprocess,
  ZodPromise: () => ZodPromise,
  ZodReadonly: () => ZodReadonly,
  ZodRealError: () => ZodRealError,
  ZodRecord: () => ZodRecord,
  ZodSet: () => ZodSet,
  ZodString: () => ZodString,
  ZodSuccess: () => ZodSuccess,
  ZodSymbol: () => ZodSymbol,
  ZodTemplateLiteral: () => ZodTemplateLiteral,
  ZodTransform: () => ZodTransform,
  ZodTuple: () => ZodTuple,
  ZodType: () => ZodType,
  ZodUndefined: () => ZodUndefined,
  ZodUnion: () => ZodUnion,
  ZodUnknown: () => ZodUnknown,
  ZodVoid: () => ZodVoid,
  ZodXor: () => ZodXor,
  _default: () => _default,
  any: () => any,
  array: () => array,
  base64: () => base642,
  base64url: () => base64url2,
  bigint: () => bigint2,
  boolean: () => boolean2,
  catch: () => $catch,
  check: () => check,
  cidrv4: () => cidrv42,
  cidrv6: () => cidrv62,
  codec: () => codec,
  coerce: () => coerce,
  compile: () => compile,
  config: () => config,
  core: () => core,
  creditCard: () => creditCard2,
  cuid: () => cuid3,
  cuid2: () => cuid22,
  custom: () => custom,
  date: () => date2,
  decode: () => decode,
  decodeAsync: () => decodeAsync,
  deepPartial: () => deepPartial2,
  default: () => index_default,
  describe: () => describe,
  discriminatedUnion: () => discriminatedUnion,
  e164: () => e1642,
  email: () => email2,
  emoji: () => emoji2,
  encode: () => encode,
  encodeAsync: () => encodeAsync,
  enum: () => $enum,
  exactOptional: () => exactOptional,
  file: () => file,
  flattenError: () => flattenError,
  float32: () => float32,
  float64: () => float64,
  formatError: () => formatError,
  fromJSONSchema: () => fromJSONSchema,
  function: () => $function,
  getDiscriminatedOption: () => getDiscriminatedOption,
  globalRegistry: () => globalRegistry2,
  guid: () => guid2,
  hash: () => hash,
  hex: () => hex2,
  hostname: () => hostname2,
  httpUrl: () => httpUrl,
  instanceof: () => $instanceof,
  int: () => int,
  int32: () => int32,
  int64: () => int64,
  interface: () => $interface,
  intersection: () => intersection,
  invertCodec: () => invertCodec,
  ipv4: () => ipv42,
  ipv6: () => ipv62,
  iso: () => iso,
  json: () => json,
  jwt: () => jwt,
  keyof: () => keyof,
  ksuid: () => ksuid2,
  lazy: () => lazy,
  literal: () => literal,
  locales: () => locales,
  looseObject: () => looseObject,
  looseRecord: () => looseRecord,
  mac: () => mac2,
  map: () => map,
  maxLength: () => maxLength,
  meta: () => meta,
  minLength: () => minLength,
  nan: () => nan,
  nanoid: () => nanoid2,
  nativeEnum: () => nativeEnum,
  never: () => never,
  nonoptional: () => nonoptional,
  null: () => $null,
  nullable: () => nullable,
  number: () => number2,
  object: () => object,
  optional: () => optional,
  parse: () => parse,
  parseAsync: () => parseAsync,
  partialRecord: () => partialRecord,
  pipe: () => pipe,
  prefault: () => prefault,
  preprocess: () => preprocess,
  prettifyError: () => prettifyError,
  promise: () => promise,
  properties: () => properties,
  property: () => property,
  readonly: () => readonly,
  record: () => record,
  refine: () => refine,
  regexes: () => regexes_exports,
  registry: () => registry,
  safeDecode: () => safeDecode,
  safeDecodeAsync: () => safeDecodeAsync,
  safeEncode: () => safeEncode,
  safeEncodeAsync: () => safeEncodeAsync,
  safeParse: () => safeParse,
  safeParseAsync: () => safeParseAsync,
  set: () => $set,
  setErrorMap: () => setErrorMap,
  slugify: () => slugify,
  strictObject: () => strictObject,
  string: () => string2,
  stringFormat: () => stringFormat,
  stringbool: () => stringbool,
  success: () => success,
  superRefine: () => superRefine,
  symbol: () => symbol,
  templateLiteral: () => templateLiteral,
  toJSONSchema: () => toJSONSchema,
  transform: () => transform,
  treeifyError: () => treeifyError,
  trim: () => trim,
  tuple: () => tuple,
  uint32: () => uint32,
  uint64: () => uint64,
  ulid: () => ulid2,
  undefined: () => $undefined,
  union: () => union,
  unknown: () => unknown,
  url: () => url,
  util: () => util,
  uuid: () => uuid2,
  uuidv4: () => uuidv4,
  uuidv6: () => uuidv6,
  uuidv7: () => uuidv7,
  visit: () => visit2,
  void: () => $void,
  with: () => $with,
  xid: () => xid2,
  xor: () => xor,
  z: () => of
});
module.exports = __toCommonJS(index_exports);

// dist/zod.core.js
var qf = "Encountered Promise during synchronous parse. Use .parseAsync() instead.";
var rf = 'Invalid discriminated union option at index "';
var sf = "return async function(data,params){return await impl(data,params)}";
var tf = ".exactPartial() cannot be used on object schemas containing refinements";
var uf = ".partial() cannot be used on object schemas containing refinements";
var vf = "[.*+?^${}()|[\\]\\\\]";
var wf = "Invalid input";
var xf = "){5}[0-9A-F]{2}$|^(?:[0-9a-f]{2}";
var yf = '"';
var zf = "Invalid input: expected ";
var Af = "";
var Bf = " items";
var Cf = " not found in enum";
var Df = "){5}[0-9a-f]{2}$";
var Ef = "^(?:[0-9A-F]{2}";
var X = (a) => a != null && "object" == typeof a;
var ja = () => /* @__PURE__ */ new Set();
var oa = (a) => ea.keys(a);
var ha = (a, b) => ea.assign(a, b);
var tb = (a, b) => {
  if (!(b == null || !X(b))) for (var c, h2 = Reflect.ownKeys(b), r2 = h2.length, f = 0; f < r2; f++) c = ea.getOwnPropertyDescriptor(b, h2[f]), c !== void 0 && c != null && ea.defineProperty(a, h2[f], c);
};
var ba = (a, b, c) => {
  ea.defineProperty(a, b, c);
};
var ub = (a, b) => {
  var c = Error.captureStackTrace;
  "function" == typeof c && c(a, b);
};
var Oa = (a) => true === Number.isFinite(a);
var la = (a) => new Error(a);
var $ = (a) => X(a) && "function" == typeof a.then;
var Nb = (a) => ({ enumerable: false, writable: true, configurable: true, value: a });
var Ia = (a) => ({ enumerable: false, writable: false, configurable: true, value: a });
var Aa = (a, b) => new globalThis.Function(a, b);
var pa = (a, b) => b == null ? false : true === Vd.call(a.prototype, b);
var Ae;
var da;
var Re;
(function() {
  let b = (a2, b2, c2) => {
    if ("number_format" == a2) return _d(b2, c2);
    if ("min_length" == a2) return ae(b2, c2);
    if ("max_length" == a2) return be(b2, c2);
    if ("greater_than" == a2) return ce(b2, c2);
    if ("less_than" == a2) return de(b2, c2);
    if ("string_format" == a2) return b2.pattern === void 0 || "string" != typeof b2.format ? void 0 : "email" != b2.format + "" ? void 0 : $d(b2, c2);
  }, a = (a2) => {
    if (a2 !== void 0 && a2._zod !== void 0) {
      var c2 = a2._zod;
      if ("function" != typeof c2.check) {
        var f2 = c2.def;
        "string" == typeof f2.check && (a2 = b(f2.check + "", f2, a2), a2 === void 0 || (c2.check = a2));
      }
    }
  }, c = (a2, b2, c2) => {
    let f2 = {}, h3 = (0, function(h4, r2) {
      var _2, e2, n2, m2, i, o2, s2, g2, y, A2, l2, t2, d, q, W2, j2, Y2, Z2, N2, k2 = h4.value;
      if (!X(k2) || Array.isArray(k2)) return h4.issues.push({ expected: "object", code: "invalid_type", input: k2, inst: c2 }), h4;
      n2 = this.def, e2 = this["~lil"], e2 === void 0 && (e2 = Gc(c2), this["~lil"] = e2), e2 = true === e2, n2.catchall !== void 0 && n2.catchall != null && (e2 = true), r2 !== void 0 && r2 != null && (m2 = r2.async, true === m2 && (e2 = true), m2 = r2.jitless, true === m2 && (e2 = true), "string" == typeof r2.direction && "backward" == r2.direction + "" && (e2 = true)), i = globalThis.__zod_globalConfig, i !== void 0 && i != null && true === i.jitless && (e2 = true);
      if (e2) return m2 = this.run, m2 == f2.fn ? b2.call(this, h4, r2) : a2.call(this, h4, r2);
      m2 = n2.shape, m2 == null && (m2 = {}), s2 = this["~skeys"], s2 === void 0 && (s2 = oa(m2), this["~skeys"] = s2), o2 = {}, t2 = s2.length;
      if (2 == t2) {
        d = s2[0] + "";
        if ("__proto__" != d) {
          e2 = m2[d];
          if (e2 !== void 0 && e2 != null) {
            g2 = e2._zod;
            if (g2 !== void 0 && g2 != null && "function" == typeof g2.run) {
              n2 = true === d in k2, e2 = void 0, n2 && (e2 = k2[d]), e2 = g2.run({ value: e2, issues: [] }, r2);
              if ($(e2)) throw new Error(qf);
              "string" == typeof g2.optin ? (i = g2.optin + "", i = "optional" == i ? 1 : "defaulted" == i ? 2 : 0) : i = 0;
              l2 = "string" == typeof g2.optout && "optional" == g2.optout + "" ? 1 : 0, A2 = e2.issues, y = _2 = A2.length, n2 && 0 == y ? (o2[d] = e2.value === void 0 ? void 0 : e2.value, g2 = true) : g2 = false;
              if (!g2) {
                g2 = !n2 && 1 == l2 && 1 == i;
                if (!g2 && y > 0) if (i && 1 == l2 && !n2) g2 = true;
                else for (W2 = 0; W2 < y; W2++) {
                  A2 = e2.issues[W2], Y2 = [d], l2 = A2.path;
                  if (Array.isArray(l2)) for (Z2 = l2.length, q = 0; q < Z2; q++) t2 = l2[q], Y2.push(t2);
                  A2.path = Y2, l2 = h4.issues, l2.push(A2);
                }
                !g2 && !n2 && 0 == i && (0 == y && (i = [d], g2 = { expected: "nonoptional", code: "invalid_type", input: void 0, path: i }, h4.issues.push(g2)), g2 = true);
                g2 || (e2.value === void 0 ? n2 && (o2[d] = void 0) : o2[d] = e2.value);
              }
            }
          }
        }
        d = s2[1] + "";
        if ("__proto__" != d) {
          g2 = m2[d];
          if (g2 !== void 0 && g2 != null) {
            s2 = g2._zod;
            if (s2 !== void 0 && s2 != null && "function" == typeof s2.run) {
              g2 = true === d in k2, m2 = void 0, g2 && (m2 = k2[d]), m2 = s2.run({ value: m2, issues: [] }, r2);
              if ($(m2)) throw new Error(qf);
              "string" == typeof s2.optin ? (r2 = s2.optin + "", r2 = "optional" == r2 ? 1 : "defaulted" == r2 ? 2 : 0) : r2 = 0;
              n2 = "string" == typeof s2.optout && "optional" == s2.optout + "" ? 1 : 0, e2 = m2.issues, k2 = _2 = e2.length, g2 && 0 == k2 ? (o2[d] = m2.value === void 0 ? void 0 : m2.value, s2 = true) : s2 = false;
              if (!s2) {
                s2 = !g2 && 1 == n2 && 1 == r2;
                if (!s2 && k2 > 0) if (r2 && 1 == n2 && !g2) s2 = true;
                else for (i = 0; i < k2; i++) {
                  e2 = m2.issues[i], A2 = [d], n2 = e2.path;
                  if (Array.isArray(n2)) for (W2 = n2.length, y = 0; y < W2; y++) l2 = n2[y], A2.push(l2);
                  e2.path = A2, n2 = h4.issues, n2.push(e2);
                }
                !s2 && !g2 && 0 == r2 && (0 == k2 && (r2 = [d], s2 = { expected: "nonoptional", code: "invalid_type", input: void 0, path: r2 }, h4.issues.push(s2)), s2 = true);
                s2 || (m2.value === void 0 ? g2 && (o2[d] = void 0) : o2[d] = m2.value);
              }
            }
          }
        }
      } else if (4 == t2) {
        g2 = s2[0] + "", e2 = m2[g2];
        if (e2 !== void 0 && e2 != null) {
          d = e2._zod;
          if (d !== void 0 && d != null && "function" == typeof d.run) {
            n2 = true === g2 in k2, e2 = void 0, n2 && (e2 = k2[g2]), e2 = d.run({ value: e2, issues: [] }, r2);
            if ($(e2)) throw new Error(qf);
            "string" == typeof d.optin ? (i = d.optin + "", i = "optional" == i ? 1 : "defaulted" == i ? 2 : 0) : i = 0;
            l2 = "string" == typeof d.optout && "optional" == d.optout + "" ? 1 : 0, A2 = e2.issues, y = A2.length, n2 && 0 == y ? (o2[g2] = e2.value === void 0 ? void 0 : e2.value, d = true) : d = false;
            if (!d) {
              d = !n2 && 1 == l2 && 1 == i;
              if (!d && y > 0) if (i && 1 == l2 && !n2) d = true;
              else for (W2 = 0; W2 < y; W2++) {
                A2 = e2.issues[W2], Y2 = [g2], l2 = A2.path;
                if (Array.isArray(l2)) for (Z2 = l2.length, q = 0; q < Z2; q++) t2 = l2[q], Y2.push(t2);
                A2.path = Y2, l2 = h4.issues, l2.push(A2);
              }
              !d && !n2 && 0 == i && (0 == y && (i = [g2], d = { expected: "nonoptional", code: "invalid_type", input: void 0, path: i }, h4.issues.push(d)), d = true);
              d || (e2.value === void 0 ? n2 && (o2[g2] = void 0) : o2[g2] = e2.value);
            }
          }
        }
        g2 = s2[1] + "";
        e2 = m2[g2];
        if (e2 !== void 0 && e2 != null) {
          d = e2._zod;
          if (d !== void 0 && d != null && "function" == typeof d.run) {
            n2 = true === g2 in k2, e2 = void 0, n2 && (e2 = k2[g2]), e2 = d.run({ value: e2, issues: [] }, r2);
            if ($(e2)) throw new Error(qf);
            "string" == typeof d.optin ? (i = d.optin + "", i = "optional" == i ? 1 : "defaulted" == i ? 2 : 0) : i = 0;
            l2 = "string" == typeof d.optout && "optional" == d.optout + "" ? 1 : 0, A2 = e2.issues, y = A2.length, n2 && 0 == y ? (o2[g2] = e2.value === void 0 ? void 0 : e2.value, d = true) : d = false;
            if (!d) {
              d = !n2 && 1 == l2 && 1 == i;
              if (!d && y > 0) if (i && 1 == l2 && !n2) d = true;
              else for (W2 = 0; W2 < y; W2++) {
                A2 = e2.issues[W2], Y2 = [g2], l2 = A2.path;
                if (Array.isArray(l2)) for (Z2 = l2.length, q = 0; q < Z2; q++) t2 = l2[q], Y2.push(t2);
                A2.path = Y2, l2 = h4.issues, l2.push(A2);
              }
              !d && !n2 && 0 == i && (0 == y && (i = [g2], d = { expected: "nonoptional", code: "invalid_type", input: void 0, path: i }, h4.issues.push(d)), d = true);
              d || (e2.value === void 0 ? n2 && (o2[g2] = void 0) : o2[g2] = e2.value);
            }
          }
        }
        g2 = s2[2] + "";
        e2 = m2[g2];
        if (e2 !== void 0 && e2 != null) {
          d = e2._zod;
          if (d !== void 0 && d != null && "function" == typeof d.run) {
            n2 = true === g2 in k2, e2 = void 0, n2 && (e2 = k2[g2]), e2 = d.run({ value: e2, issues: [] }, r2);
            if ($(e2)) throw new Error(qf);
            "string" == typeof d.optin ? (i = d.optin + "", i = "optional" == i ? 1 : "defaulted" == i ? 2 : 0) : i = 0;
            l2 = "string" == typeof d.optout && "optional" == d.optout + "" ? 1 : 0, A2 = e2.issues, y = A2.length, n2 && 0 == y ? (o2[g2] = e2.value === void 0 ? void 0 : e2.value, d = true) : d = false;
            if (!d) {
              d = !n2 && 1 == l2 && 1 == i;
              if (!d && y > 0) if (i && 1 == l2 && !n2) d = true;
              else for (W2 = 0; W2 < y; W2++) {
                A2 = e2.issues[W2], Y2 = [g2], l2 = A2.path;
                if (Array.isArray(l2)) for (Z2 = l2.length, q = 0; q < Z2; q++) t2 = l2[q], Y2.push(t2);
                A2.path = Y2, l2 = h4.issues, l2.push(A2);
              }
              !d && !n2 && 0 == i && (0 == y && (i = [g2], d = { expected: "nonoptional", code: "invalid_type", input: void 0, path: i }, h4.issues.push(d)), d = true);
              d || (e2.value === void 0 ? n2 && (o2[g2] = void 0) : o2[g2] = e2.value);
            }
          }
        }
        d = s2[3] + "";
        g2 = m2[d];
        if (g2 !== void 0 && g2 != null) {
          s2 = g2._zod;
          if (s2 !== void 0 && s2 != null && "function" == typeof s2.run) {
            g2 = true === d in k2, m2 = void 0, g2 && (m2 = k2[d]), m2 = s2.run({ value: m2, issues: [] }, r2);
            if ($(m2)) throw new Error(qf);
            "string" == typeof s2.optin ? (r2 = s2.optin + "", r2 = "optional" == r2 ? 1 : "defaulted" == r2 ? 2 : 0) : r2 = 0;
            n2 = "string" == typeof s2.optout && "optional" == s2.optout + "" ? 1 : 0, e2 = m2.issues, k2 = e2.length, g2 && 0 == k2 ? (o2[d] = m2.value === void 0 ? void 0 : m2.value, s2 = true) : s2 = false;
            if (!s2) {
              s2 = !g2 && 1 == n2 && 1 == r2;
              if (!s2 && k2 > 0) if (r2 && 1 == n2 && !g2) s2 = true;
              else for (i = 0; i < k2; i++) {
                e2 = m2.issues[i], A2 = [d], n2 = e2.path;
                if (Array.isArray(n2)) for (W2 = n2.length, y = 0; y < W2; y++) l2 = n2[y], A2.push(l2);
                e2.path = A2, n2 = h4.issues, n2.push(e2);
              }
              !s2 && !g2 && 0 == r2 && (0 == k2 && (r2 = [d], s2 = { expected: "nonoptional", code: "invalid_type", input: void 0, path: r2 }, h4.issues.push(s2)), s2 = true);
              s2 || (m2.value === void 0 ? g2 && (o2[d] = void 0) : o2[d] = m2.value);
            }
          }
        }
      } else {
        i = 0;
        while (i < t2) {
          d = s2[i] + "";
          if ("__proto__" == d) i++;
          else {
            e2 = m2[d];
            if (e2 !== void 0 && e2 != null) {
              g2 = e2._zod;
              if (g2 !== void 0 && g2 != null && "function" == typeof g2.run) {
                n2 = true === d in k2, e2 = void 0, n2 && (e2 = k2[d]), e2 = g2.run({ value: e2, issues: [] }, r2);
                if ($(e2)) throw new Error(qf);
                "string" == typeof g2.optin ? (y = g2.optin + "", y = "optional" == y ? 1 : "defaulted" == y ? 2 : 0) : y = 0;
                W2 = "string" == typeof g2.optout && "optional" == g2.optout + "" ? 1 : 0, q = e2.issues;
                var p = q.length;
                n2 && 0 == p ? (o2[d] = e2.value === void 0 ? void 0 : e2.value, g2 = true) : g2 = false;
                if (!g2) {
                  g2 = !n2 && 1 == W2 && 1 == y;
                  if (!g2 && p > 0) if (y && 1 == W2 && !n2) g2 = true;
                  else for (q = 0; q < p; q++) {
                    l2 = e2.issues[q], Z2 = [d], W2 = l2.path;
                    if (Array.isArray(W2)) for (N2 = W2.length, Y2 = 0; Y2 < N2; Y2++) j2 = W2[Y2], Z2.push(j2);
                    l2.path = Z2, Y2 = h4.issues, Y2.push(l2);
                  }
                  !g2 && !n2 && 0 == y && (0 == p && (y = [d], g2 = { expected: "nonoptional", code: "invalid_type", input: void 0, path: y }, h4.issues.push(g2)), g2 = true);
                  g2 || (e2.value === void 0 ? n2 && (o2[d] = void 0) : o2[d] = e2.value);
                }
              }
            }
            i++;
          }
        }
      }
      h4.value = o2;
      return h4;
    });
    f2.fn = h3;
    return h3;
  }, f = (a2, b2, c2, f2) => {
    var h3 = b2.length;
    return 0 == h3 ? a2 : !((a3) => {
      for (var b3, h4, c3, r2 = a3.length, f3 = 0; f3 < r2; f3++) {
        b3 = a3[f3], b3._zod === void 0 ? c3 = true : (h4 = b3._zod, c3 = "function" != typeof h4.check);
        if (c3) return false;
      }
      return true;
    })(b2) ? c2 : function(r2, e2) {
      var g2 = this;
      if (e2 !== void 0 && e2 != null) {
        var d;
        if ("string" == typeof e2.direction && "backward" == e2.direction + "") return c2.call(g2, r2, e2);
        d = e2.skipChecks;
        if (true === d) return a2.call(g2, r2, e2);
      }
      d = {};
      d.go = function(a3, r3) {
        r3 |= 0;
        while (r3 < h3) {
          var n3, s2 = b2[r3];
          if (s2 !== void 0 && s2 != null) {
            var W2 = s2._zod;
            if (W2 == null) return c2.call(g2, a3, e2);
            var k2, q, o2, l2, i, m2, y, X2, Y2, A2 = W2.def, t2 = A2 !== void 0 && A2 != null && "function" == typeof A2.when;
            n3 = a3.aborted, n3 = true === n3;
            if (!n3) {
              k2 = a3.issues, Y2 = k2.length, l2 = 0;
              while (l2 < Y2) q = k2[l2], o2 = q.continue, t2 ? false === o2 && (n3 = true) : true === o2 || (n3 = true), l2++;
            }
            t2 ? (n3 = a3.issues.length > 0 && n3, !n3 && !A2.when(a3) && (n3 = true)) : n3 = a3.issues.length > 0 && n3;
            if (!n3) {
              n3 = W2.check;
              if ("function" != typeof n3) return c2.call(g2, a3, e2);
              n3 = n3.call(s2, a3);
              if ($(n3)) {
                if (e2 !== void 0 && e2 != null && false === e2.async) throw new Error(qf);
                i = r3 + 1 | 0;
                return Promise.resolve(n3).then(/* @__PURE__ */ ((a4, b3, c3) => function(f3) {
                  return a4.go(b3, +(0 + c3));
                })(d, a3, i));
              }
            }
          }
          r3++;
        }
        for (m2 = a3.issues, X2 = m2.length, i = 0; i < X2; i++) y = m2[i], y.schema === void 0 && (y = m2[i], y.schema = f2);
        return a3;
      };
      var n2 = a2.call(g2, r2, e2);
      return $(n2) ? n2.then(function(a3) {
        return d.go(a3, 0);
      }) : d.go(n2, 0);
    };
  }, h2 = (a2, b2, c2) => {
    let f2 = {}, h3 = (0, function(h4, r2) {
      var i = h4.value;
      if (!Array.isArray(i)) return h4.issues.push({ expected: "array", code: "invalid_type", input: i, inst: c2 }), h4;
      var d = this["~lil"];
      d === void 0 && (d = Gc(c2), this["~lil"] = d), d = true === d;
      if (r2 !== void 0 && r2 != null) {
        var e2 = r2.async;
        true === e2 && (d = true), e2 = r2.jitless, true === e2 && (d = true), "string" == typeof r2.direction && "backward" == r2.direction + "" && (d = true);
      }
      if (d) return e2 = this.run, e2 == f2.fn ? b2.call(this, h4, r2) : a2.call(this, h4, r2);
      var g2 = this.def;
      e2 = g2.element;
      if (e2 == null || e2._zod === void 0 || "function" != typeof e2._zod.run) return a2.call(this, h4, r2);
      var k2 = e2._zod, o2 = i.length, m2 = new Array(o2), q = h4.issues, y = [];
      g2 = 0;
      while (g2 < o2) {
        var n2 = k2.run;
        e2 = i[g2];
        var s2 = k2.run({ value: e2, issues: [] }, r2);
        if ($(s2)) {
          if (r2 !== void 0 && r2 != null && false === r2.async) throw new Error(qf);
          y.push(s2.then(/* @__PURE__ */ ((a3, b3, c3) => function(f3) {
            var r3, e3 = f3.issues;
            if (e3 !== void 0 && e3.length > 0) {
              var n3 = e3.length;
              for (r3 = 0; r3 < n3; r3++) {
                var g3 = [+(0 + c3)], d2 = e3[r3], h5 = d2.path;
                if (Array.isArray(h5)) {
                  var i2 = h5.length;
                  for (d2 = 0; d2 < i2; d2++) {
                    var s3 = h5[d2];
                    g3.push(s3);
                  }
                }
                h5 = e3[r3];
                h5.path = g3, h5 = e3[r3], b3.push(h5);
              }
            }
            a3[c3] = f3.value;
          })(m2, q, g2)));
        } else {
          n2 = s2.issues;
          if (n2 !== void 0 && n2.length > 0) {
            var X2 = n2.length;
            for (e2 = 0; e2 < X2; e2++) {
              var l2 = [+(0 + g2)];
              d = n2[e2];
              var A2 = d.path;
              if (Array.isArray(A2)) {
                var Y2 = A2.length;
                for (d = 0; d < Y2; d++) {
                  var W2 = A2[d];
                  l2.push(W2);
                }
              }
              W2 = n2[e2];
              W2.path = l2, l2 = n2[e2], q.push(l2);
            }
          }
          m2[g2] = s2.value;
        }
        g2++;
      }
      h4.value = m2;
      return y.length > 0 ? Promise.all(y).then(function(a3) {
        return h4;
      }) : h4;
    });
    f2.fn = h3;
    return h3;
  };
  Ae = function(b2) {
    var f2 = [], h3 = b2._zod, c2 = h3.def;
    "string" == typeof c2.check && f2.push(b2);
    b2 = c2.checks;
    if (b2 !== void 0 && Array.isArray(b2)) {
      var r2 = b2.length;
      for (c2 = 0; c2 < r2; c2++) a(b2[c2]), h3 = b2[c2], f2.push(h3);
    }
    return f2;
  }, da = function(b2, c2) {
    c2.check = b2, c2.when === void 0 && ("min_length" == b2 || "max_length" == b2 || "length_equals" == b2 ? c2.when = function(a2) {
      return ((a3) => {
        var b3 = a3.value;
        return b3 == null ? false : b3.length !== void 0;
      })(a2) ? a2 : void 0;
    } : ("min_size" == b2 || "max_size" == b2 || "size_equals" == b2) && (c2.when = function(a2) {
      return ((a3) => {
        var b3 = a3.value;
        return b3 == null ? false : b3.size !== void 0;
      })(a2) ? a2 : void 0;
    }));
    var f2 = "min_length" == b2 ? "$ZodCheckMinLength" : "max_length" == b2 ? "$ZodCheckMaxLength" : "length_equals" == b2 ? "$ZodCheckLengthEquals" : "greater_than" == b2 ? "$ZodCheckGreaterThan" : "less_than" == b2 ? "$ZodCheckLessThan" : "multiple_of" == b2 ? "$ZodCheckMultipleOf" : "min_size" == b2 ? "$ZodCheckMinSize" : "max_size" == b2 ? "$ZodCheckMaxSize" : "size_equals" == b2 ? "$ZodCheckSizeEquals" : "includes" == b2 ? "$ZodCheckIncludes" : "starts_with" == b2 ? "$ZodCheckStartsWith" : "ends_with" == b2 ? "$ZodCheckEndsWith" : "string_format" == b2 ? "$ZodCheckStringFormat" : "overwrite" == b2 ? "$ZodCheckOverwrite" : "mime_type" == b2 ? "$ZodCheckMimeType" : "number_format" == b2 ? "$ZodCheckNumberFormat" : "lowercase" == b2 ? "$ZodCheckStringFormat" : "uppercase" == b2 ? "$ZodCheckStringFormat" : "$ZodCheck";
    b2 = ja(), b2.add("$ZodCheck"), b2.add(f2);
    var h3 = { def: c2, onattach: [], traits: b2 };
    b2 = { def: c2 }, c2 = function() {
      return b2;
    }, ba(c2, "name", Ia(f2)), b2.constructor = c2, ba(b2, "_zod", Nb(h3)), a(b2);
    return b2;
  }, Re = function(b2, r2, e2, d, g2) {
    a(b2);
    var n2 = r2.kind;
    if (0 == n2) r2 = Xd(b2);
    else if (1 == n2) r2 = Yd(b2);
    else if (2 == n2) r2 = Zd(b2);
    else if (15 == n2) r2 = c(d, g2, b2);
    else if (16 == n2) r2 = h2(d, g2, b2);
    else {
      return;
    }
    e2["~pf"] = d;
    e2["~rf"] = g2, e2.parse = 15 == n2 || 16 == n2 ? d : r2, e2.run = f(r2, Ae(b2), g2, b2);
  };
})();
function Ce(a, b) {
  var c = b.type + "";
  a.id = dc, dc++, a.kind = "string" == c ? 0 : "number" == c ? 1 : "int" == c ? 1 : "boolean" == c ? 2 : "bigint" == c ? 3 : "symbol" == c ? 4 : "date" == c ? 5 : "nan" == c ? 6 : "undefined" == c ? 7 : "null" == c ? 8 : "any" == c ? 9 : "unknown" == c ? 10 : "never" == c ? 11 : "void" == c ? 12 : "literal" == c ? 13 : "enum" == c ? 14 : "object" == c ? 15 : "array" == c ? 16 : "tuple" == c ? 17 : "record" == c ? 18 : "map" == c ? 19 : "set" == c ? 20 : "union" == c ? 21 : "intersection" == c ? 22 : "optional" == c ? 23 : "nullable" == c ? 24 : "default" == c ? 25 : "prefault" == c ? 26 : "catch" == c ? 27 : "nonoptional" == c ? 28 : "lazy" == c ? 29 : "promise" == c ? 30 : "transform" == c ? 31 : "pipe" == c ? 32 : "readonly" == c ? 33 : "custom" == c ? 34 : "file" == c ? 35 : "success" == c ? 37 : "function" == c ? 39 : "template_literal" == c ? 40 : 34, "union" == c && "string" != typeof b.discriminator && false === b.inclusive && (a.kind = 38), a.handle = void 0, a.def = b, a.ctor = void 0, a.typeName = c, "int" == c && (a.typeName = "number"), a.trait = "ZodType", a.values = void 0, a.optin = 0, a.optout = 0, a.hasChecks = false, Ja.push(a), ((a2) => {
    var b2 = a2.typeName, c2 = a2.def;
    if ("undefined" == b2) b2 = ja(), b2.add(void 0), a2.values = b2;
    else if ("null" == b2) b2 = ja(), b2.add(null), a2.values = b2;
    else if ("literal" == b2) {
      var f = ja();
      b2 = c2.values;
      if (Array.isArray(b2)) {
        var h2 = b2.length;
        for (c2 = 0; c2 < h2; c2++) {
          var r2 = b2[c2];
          f.add(r2);
        }
      }
      a2.values = f;
    } else if ("enum" == b2) {
      for (f = ja(), h2 = Gd(c2.entries), r2 = h2.length, b2 = 0; b2 < r2; b2++) c2 = h2[b2], f.add(c2);
      a2.values = f;
    } else if ("optional" == b2) {
      if (b2 = xa(a2)) {
        if (b2.values !== void 0) {
          f = ja(), r2 = Array.from(b2.values);
          var e2 = r2.length;
          for (h2 = 0; h2 < e2; h2++) {
            var d = r2[h2];
            f.add(d);
          }
          c2.exact || f.add(void 0);
          a2.values = f;
        }
        a2.optin = 2 == b2.optin ? 2 : 1;
      } else {
        a2.optin = 1;
      }
      a2.optout = 1;
    } else if ("nullable" == b2) {
      if (b2 = xa(a2)) {
        if (b2.values !== void 0) {
          for (c2 = ja(), h2 = Array.from(b2.values), r2 = h2.length, f = 0; f < r2; f++) e2 = h2[f], c2.add(e2);
          c2.add(null), a2.values = c2;
        }
        a2.optin = b2.optin;
        a2.optout = b2.optout;
      }
    } else if ("default" == b2 || "prefault" == b2 || "catch" == b2) c2 = xa(a2), "catch" == b2 ? (a2.optin = 1, !c2 || (b2 = c2, a2.values = b2.values, 2 == b2.optin && (a2.optin = 2))) : (a2.optin = 2, !c2 || (a2.values = c2.values));
    else if ("readonly" == b2) b2 = xa(a2), b2 && (a2.values = b2.values, a2.optin = b2.optin, a2.optout = b2.optout);
    else if ("nonoptional" == b2) b2 = xa(a2), b2 && (a2.values = b2.values);
    else if ("transform" == b2) a2.optin = 1;
    else if ("pipe" == b2) b2 = ya(a2, "in"), b2 && (a2.values = b2.values, a2.optin = b2.optin, a2.optout = b2.optout);
    else if ("union" == b2) {
      e2 = c2.options;
      if (Array.isArray(e2)) {
        var g2 = e2.length;
        b2 = g2 > 0;
        var n2 = ja();
        f = false, h2 = false, r2 = false, d = 0;
        while (d < g2) {
          if (c2 = sa(e2[d])) {
            2 == c2.optin && (f = true), 0 != c2.optin && (h2 = true), 1 == c2.optout && (r2 = true);
            if (c2.values === void 0) b2 = false;
            else {
              var i = Array.from(c2.values), s2 = i.length;
              for (c2 = 0; c2 < s2; c2++) {
                var l2 = i[c2];
                n2.add(l2);
              }
            }
          } else {
            b2 = false;
          }
          d++;
        }
        b2 && (a2.values = n2);
        f ? a2.optin = 2 : h2 && (a2.optin = 1), r2 && (a2.optout = 1);
      }
    }
    a2.hasChecks = false;
    "string" == typeof a2.def.check ? a2.hasChecks = true : (b2 = a2.def.checks, Array.isArray(b2) && b2.length > 0 && (a2.hasChecks = true));
  })(a);
}
var Ob = (a, b, c, f) => {
  f.issues.length > 0 && _a(f.issues, a.issues, c), b[c] = f.value;
};
var Wc = (a) => {
  var c = a.handle._zod, b = c.optin;
  return "string" == typeof b ? "defaulted" == b + "" ? 2 : "optional" == b + "" ? 1 : 0 : a.optin;
};
var Xc = (a) => {
  var c = a.handle._zod, b = c.optout;
  return "string" == typeof b && "optional" == b + "" ? 1 : a.optout;
};
var Yc = (a, b, c, f, h2, r2, e2) => {
  if (f && 0 == e2.issues.length) {
    b[c] = e2.value === void 0 ? void 0 : e2.value;
    return;
  }
  var d = 1 == r2, g2 = 0 != h2;
  r2 = !f && d && 1 == h2, !r2 && e2.issues.length > 0 && (g2 && d && !f ? r2 = true : _a(e2.issues, a.issues, c)), !r2 && !f && 0 == h2 && (h2 = e2.issues, 0 == h2.length && (h2 = Ba("nonoptional", void 0), wb(h2, c), r2 = a.issues, r2.push(h2)), r2 = true);
  if (!r2) e2.value === void 0 ? f && (b[c] = void 0) : b[c] = e2.value;
};
var Pb = (a) => a.replace(new RegExp(vf, "g"), "\\$&") + "";
var Qb = (a) => {
  var b = a.length, c = a.startsWith("^") ? 1 : 0;
  !a.endsWith("$") || (b = b - 1 | 0);
  return a.slice(c, b) + "";
};
var ia = (a, b, c) => {
  var f = a.handle._zod, h2 = ea.getPrototypeOf(f);
  if (true === b in h2 && ec !== f) {
    ec = void 0;
    return;
  }
  ec = f;
  ba(h2, b, { configurable: true, get: function() {
    ba(this, b, ge);
    var f2 = $a;
    $a = false;
    var a2;
    try {
      a2 = c(this);
      if ($a) Reflect.deleteProperty(this, b);
      else {
        ba(this, b, { configurable: true, writable: true, value: a2 });
      }
      f2 && ($a = true);
      return a2;
    } catch (a3) {
      Reflect.deleteProperty(this, b), f2 && ($a = true);
      throw a3;
    }
  }, set: function(a2) {
    ba(this, b, { configurable: true, writable: true, value: a2 });
  } });
};
var Zc = (a) => 1 == a.optin ? "optional" : 2 == a.optin ? "defaulted" : void 0;
var _c = (a) => 1 == a.optout ? "optional" : void 0;
var vb = (a) => {
  var b = a.kind;
  return 15 == b ? true : 16 == b ? true : 17 == b ? true : 18 == b ? true : 19 == b ? true : 20 == b ? true : false;
};
var Rb = (a, b, c) => {
  if (b == null || !X(b)) return false;
  if (X(b._zod)) return (a = sa(b)) ? $c(a, c) : false;
  if (!Array.isArray(b)) return false;
  for (var h2 = b.length, f = 0; f < h2; f++) if (Rb(a, b[f], c)) return true;
  return false;
};
var $c = (a, b) => {
  var c, G2 = Hc.get(a.handle);
  if (G2 !== void 0 && G2 != null) return true === G2;
  if (b.has(a.handle)) return true;
  b.add(a.handle);
  if (29 == a.kind) G2 = a.handle._zod, G2 = Rb(a, G2.innerType, b);
  else {
    var r2 = a.def;
    c = r2.shape;
    if (X(c) && c != null) for (var h2 = oa(c), e2 = h2.length, G2 = false, f = 0; f < e2; f++) Rb(a, c[h2[f]], b) && (G2 = true);
    else {
      G2 = false;
    }
    for (f = oa(r2), h2 = f.length, c = 0; c < h2; c++) "shape" != f[c] + "" && Rb(a, r2[f[c]], b) && (G2 = true);
  }
  b.delete(a.handle);
  Hc.set(a.handle, G2);
  return G2;
};
var ad = (a) => {
  for (var b, f = [], h2 = a.length, c = 0; c < h2; c++) {
    b = ha({}, a[c]);
    if (Array.isArray(b.path)) {
      var r2 = b.path.slice(0);
      b.path = r2;
    }
    f.push(b);
  }
  return f;
};
var fb = (a, b, c) => {
  var h2 = a.handle._zod, f = h2.memoizer;
  if (f === void 0 || f.handoff === void 0 || f.handoff == null) return c;
  h2 = f.handoff, f.handoff = void 0, a = { value: c, issues: null };
  var r2 = b.value;
  h2.set(r2, a), h2 = f.open, h2.push(a);
  return c;
};
var bd = (a, b) => {
  var f = a.handle._zod, c = f.memoizer;
  if (c != null) c.handoff = void 0, a = c.open, f = "number" == typeof c.openDepth ? c.openDepth | 0 : 0, Array.isArray(a) && a.length > f && (a = a.pop(), c = b.issues, a.issues = c.length > 0 ? ad(b.issues) : []);
};
var cd = (a, b, c) => a ? { code: "too_big", maximum: 0 + b, inclusive: true, input: c, origin: "array" } : { code: "too_small", minimum: 0 + b, inclusive: true, input: c, origin: "array" };
var dd = (a, b, c, f, h2) => {
  var e2 = a.value, n2 = c.length, i = b.length;
  for (c = 0; c < i; c++) {
    var r2 = f[c], d = c < n2, g2 = sa(b[c]), s2 = g2 ? g2.optin : 0;
    if (!d && c >= h2 && 1 == s2) {
      e2.length = c;
      break;
    }
    if (r2 !== void 0 && r2 != null && r2.issues.length > 0) {
      if (!d && c >= h2) {
        e2.length = c;
        break;
      }
      d = r2.issues;
      g2 = a.issues, _a(d, g2, +(0 + c));
    }
    r2 !== void 0 && r2 != null && (e2[c] = r2.value);
  }
  a = e2.length - 1 | 0;
  while (a >= n2) {
    c = sa(b[a]);
    if (c && 1 == c.optout && e2[a] === void 0) e2.length = a, a--;
    else break;
  }
};
var gb = (a, b) => {
  a.value = b.value, ud(b.issues, a.issues);
};
var ed = (a, b, c, f) => {
  var h2, e2, d, i, g2, n2 = c.length, r2 = 0;
  while (r2 < n2) {
    h2 = c[r2];
    if (h2 !== void 0 && h2 != null && 0 == h2.issues.length) {
      gb(b, h2);
      return;
    }
    r2++;
  }
  for (e2 = [], r2 = 0; r2 < n2; r2++) h2 = c[r2], h2 !== void 0 && h2 != null && !Xb(h2) && e2.push(h2);
  if (1 == e2.length) {
    gb(b, e2[0]);
    return;
  }
  for (d = [], r2 = 0; r2 < n2; r2++) {
    h2 = c[r2], g2 = [];
    if (h2 !== void 0 && h2 != null) for (e2 = h2.issues, i = e2.length, h2 = 0; h2 < i; h2++) g2.push(Wa(e2[h2], a, f));
    d.push(g2);
  }
  b.issues.push({ code: "invalid_union", errors: d, path: [] });
};
var ca = (a) => {
  let b = a._zod;
  return Ja[+b.id];
};
var sa = (a) => {
  if (a == null || !X(a)) return null;
  var b = a._zod;
  if (b == null || b.id === void 0) return null;
  a = b.id | 0;
  return a < 0 || a >= Ja.length ? null : Ja[a];
};
var fd;
var gd;
(function() {
  let b = (a2) => {
    if (a2 === void 0) return "undefined";
    if (a2 === null) return "null";
    var b2 = typeof a2;
    if ("number" == b2) return true === Number.isNaN(a2) ? "nan" : !Oa(a2) ? a2 + "" : "number";
    if ("object" == b2) {
      if (a2 == null) return "null";
      if (Array.isArray(a2)) return "array";
      b2 = ea.getPrototypeOf(a2);
      return b2 !== ea.prototype && "function" == typeof a2.constructor && (b2 = a2.constructor.name + "", b2.length > 0) ? b2 : "object";
    }
    return b2;
  }, a = (a2) => "bigint" == typeof a2 ? a2 + "n" : "string" == typeof a2 ? yf + a2 + yf : a2 === void 0 ? "undefined" : a2 == null ? "null" : a2 + "";
  fd = function(b2, c) {
    for (var r2 = b2.length, f = Af, h2 = 0; h2 < r2; h2++) h2 > 0 && (f = f + c), f += a(b2[h2]);
    return f;
  }, gd = function(c) {
    var f = c.code + "";
    if ("invalid_type" == f) {
      var r2 = c.expected + "";
      "nan" == r2 && (r2 = "NaN");
      f = b(c.input), "nan" == f && (f = "NaN"), "number" == f && "number" == typeof c.input && !Oa(c.input) && (f = c.input + "");
      return zf + r2 + ", received " + f;
    }
    if ("invalid_value" == f) {
      f = c.values;
      return f == null || !Array.isArray(f) ? wf : 1 == f.length ? zf + a(f[0]) : "Invalid option: expected one of " + fd(f, "|");
    }
    if ("too_big" == f) {
      c.exact ? f = "exactly " : (f = c.inclusive, f = false !== f ? "<=" : "<");
      var h2 = c.origin !== void 0 ? c.origin + "" : "value";
      if ("string" == h2) return "Too big: expected string to have " + f + c.maximum + " characters";
      if ("array" == h2) return "Too big: expected array to have " + f + c.maximum + Bf;
      if ("set" == h2) return "Too big: expected set to have " + f + c.maximum + Bf;
      if ("map" == h2) return "Too big: expected map to have " + f + c.maximum + " entries";
      if ("file" == h2) return "Too big: expected file to have " + f + c.maximum + " bytes";
      f = "Too big: expected " + h2 + " to be " + f;
      return f + c.maximum;
    }
    if ("too_small" == f) {
      c.exact ? f = "exactly " : (f = c.inclusive, f = false !== f ? ">=" : ">"), h2 = c.origin !== void 0 ? c.origin + "" : "value";
      if ("string" == h2) return "Too small: expected string to have " + f + c.minimum + " characters";
      if ("array" == h2) return "Too small: expected array to have " + f + c.minimum + Bf;
      if ("set" == h2) return "Too small: expected set to have " + f + c.minimum + Bf;
      if ("map" == h2) return "Too small: expected map to have " + f + c.minimum + " entries";
      if ("file" == h2) return "Too small: expected file to have " + f + c.minimum + " bytes";
      f = "Too small: expected " + h2 + " to be " + f;
      return f + c.minimum;
    }
    if ("invalid_format" == f) {
      f = c.format + "";
      return "starts_with" == f ? 'Invalid string: must start with "' + c.prefix + yf : "ends_with" == f ? 'Invalid string: must end with "' + c.suffix + yf : "includes" == f ? 'Invalid string: must include "' + c.includes + yf : "regex" == f ? "Invalid string: must match pattern " + c.pattern : "Invalid " + ("regex" == f ? "input" : "email" == f ? "email address" : "url" == f ? "URL" : "emoji" == f ? "emoji" : "uuid" == f ? "UUID" : "uuidv4" == f ? "UUIDv4" : "uuidv6" == f ? "UUIDv6" : "uuidv7" == f ? "UUIDv7" : "nanoid" == f ? "nanoid" : "guid" == f ? "GUID" : "cuid" == f ? "cuid" : "cuid2" == f ? "cuid2" : "ulid" == f ? "ULID" : "xid" == f ? "XID" : "ksuid" == f ? "KSUID" : "datetime" == f ? "ISO datetime" : "date" == f ? "ISO date" : "time" == f ? "ISO time" : "duration" == f ? "ISO duration" : "ipv4" == f ? "IPv4 address" : "ipv6" == f ? "IPv6 address" : "mac" == f ? "MAC address" : "cidrv4" == f ? "IPv4 range" : "cidrv6" == f ? "IPv6 range" : "base64" == f ? "base64-encoded string" : "base64url" == f ? "base64url-encoded string" : "json_string" == f ? "JSON string" : "e164" == f ? "E.164 number" : "credit_card" == f ? "credit card number" : "jwt" == f ? "JWT" : "template_literal" == f ? "input" : f);
    }
    if ("not_multiple_of" == f) return "Invalid number: must be a multiple of " + c.divisor;
    if ("unrecognized_keys" == f) return f = c.keys, c = f.length > 1 ? "s" : Af, "Unrecognized key" + c + ": " + fd(f, ", ");
    if ("invalid_key" == f) return "Invalid key in " + c.origin;
    if ("invalid_union" == f) {
      Array.isArray(c.options) ? (h2 = c.options, f = h2.length > 0) : f = false;
      if (f) {
        for (h2 = c.options, r2 = h2.length, c = Af, f = 0; f < r2; f++) f > 0 && (c = c + " | "), c = c + "'" + h2[f] + "'";
        return "Invalid discriminator value. Expected " + c;
      }
      f = c.inclusive;
      return false === f ? "Invalid input: more than one option matched" : wf;
    }
    return "invalid_element" == f ? "Invalid value in " + c.origin : "custom" == f ? "string" == typeof c.message ? c.message + "" : wf : wf;
  };
})();
var Wa = /* @__PURE__ */ (function() {
  let a = (a2) => {
    if (a2 != null) {
      if ("string" == typeof a2) return a2;
      if (X(a2) && "string" == typeof a2.message) return a2.message;
    }
  }, b = (a2) => {
    if (!(a2 == null || !X(a2))) {
      var b2 = a2._zod;
      if (b2 != null) return a2 = b2.def, a2.error;
    }
  }, c = (c2, f2) => {
    var h2;
    if ("string" == typeof c2.message && (c2.message + "").length > 0) return c2.message + "";
    h2 = b(c2.inst);
    if ("function" == typeof h2) {
      var r2 = a(h2(c2));
      if (r2 !== void 0) return r2 + "";
    }
    if ("string" == typeof h2) return h2 + "";
    h2 = c2.schema;
    if (h2 !== c2.inst) {
      h2 = b(c2.schema);
      if ("function" == typeof h2 && (r2 = a(h2(c2)), r2 !== void 0)) return r2 + "";
      if ("string" == typeof h2) return h2 + "";
    }
    return f2 !== void 0 && "function" == typeof f2.error && (h2 = a(f2.error(c2)), h2 !== void 0) ? h2 + "" : "function" == typeof Cb && (f2 = a(Cb(c2)), f2 !== void 0) ? f2 + "" : "function" == typeof ab && (f2 = a(ab(c2)), f2 !== void 0) ? f2 + "" : gd(c2);
  }, f = (a2) => {
    var b2 = a2.inst;
    if (b2 !== void 0 && b2 != null && !!X(b2)) {
      var f2 = b2._zod;
      if (f2 !== void 0 && f2 != null) {
        var c2 = f2.traits;
        if (c2 !== void 0 && c2 != null && !!c2.has("$ZodType")) c2.has("$ZodCheck") ? a2.schema === void 0 && (a2.schema = b2) : a2.schema = b2;
      }
    }
  };
  return function(a2, b2, h2) {
    a2.inst === void 0 && b2 && (a2.inst = b2.handle), f(a2);
    var g2 = c(a2, h2);
    b2 = {};
    for (var r2, d = oa(a2), n2 = d.length, e2 = 0; e2 < n2; e2++) r2 = d[e2] + "", "inst" != r2 && "schema" != r2 && "continue" != r2 && "input" != r2 && (b2[r2] = a2[r2]);
    (b2.path === void 0 || b2.path == null) && (b2.path = []), b2.message = g2, h2 !== void 0 && h2 != null && h2.reportInput && (b2.input = a2.input);
    return b2;
  };
})();
var wb = (a, b) => {
  var c = a.path;
  c == null && (c = [], a.path = c), c.unshift(b);
};
var Ba = (a, b) => {
  var c = { expected: a, code: "invalid_type", input: b };
  "number" == a && "number" == typeof b && (true === Number.isNaN(b) ? c.received = "NaN" : Oa(b) || (c.received = b + ""));
  "date" == a && pa(Date, b) && true === Number.isNaN(b.getTime()) && (c.received = "Invalid Date");
  return c;
};
var hb;
var od;
(function() {
  let a = (a2, b2, c2, f2, h3, r3) => {
    var e2 = a2 ? "too_big" : "too_small";
    pa(Date, c2) && (c2 = c2.getTime()), b2 = { origin: b2, code: e2 }, a2 ? b2.maximum = c2 : b2.minimum = c2, b2.inclusive = f2, b2.input = r3, h3 && (b2.exact = true), b2.continue = true;
    return b2;
  }, b = (a2, b2) => {
    var c2 = globalThis.Math;
    a2 /= b2, c2 = +c2.round(a2), b2 = a2 < 0 ? 0 - a2 : a2, b2 > 1 || (b2 = 1);
    var f2 = +Number.EPSILON * b2;
    b2 = a2 - c2, b2 < 0 && (b2 = 0 - b2);
    return b2 < f2 ? 0 : a2 - c2;
  }, c = (a2) => {
    if (!/^\d(?:[ -]?\d){11,18}$/.test(a2)) return false;
    for (var c2, f2 = a2.length, h3 = Af, b2 = 0; b2 < f2; b2++) c2 = a2.slice(b2, b2 + 1), c2 >= "0" && c2 <= "9" && (h3 = h3 + c2);
    c2 = h3.length - 1, b2 = 0, f2 = false;
    while (c2 >= 0) a2 = +Number(h3.slice(c2, c2 + 1 | 0)) | 0, f2 && (a2 = a2 * 2 | 0, a2 > 9 && (a2 = a2 - 9 | 0)), b2 = b2 + a2 | 0, f2 = !f2, c2--;
    while (b2 >= 10) b2 -= 10;
    return 0 == b2;
  }, f = (a2, b2) => {
    var f2 = a2.split(".");
    if (3 != f2.length) return false;
    a2 = f2[0];
    if (a2 == null || 0 == (a2 + "").length) return false;
    try {
      f2 = globalThis.JSON;
      var h3 = globalThis.atob, r3 = h3(a2), c2 = f2.parse(r3);
      return !X(c2) || c2 == null ? false : Ga.call(c2, "typ") && "JWT" != c2.typ + "" ? false : c2.alg === void 0 ? false : b2 !== void 0 && b2 != null && "string" == typeof b2 && c2.alg + "" != b2 + "" ? false : true;
    } catch {
      return false;
    }
  }, h2 = (a2) => {
    if (!/^[A-Za-z0-9_-]*$/.test(a2)) return false;
    a2 = a2.replace(/-/g, "+").replace(/_/g, "/") + "";
    for (var b2 = 0, c2 = 0; c2 < a2.length; c2++) b2++, 4 == b2 && (b2 = 0);
    1 == b2 && (a2 = a2 + "==="), 2 == b2 && (a2 = a2 + "=="), 3 == b2 && (a2 = a2 + "=");
    return md(a2);
  }, r2 = (a2) => {
    a2 = a2.split("/");
    if (2 != a2.length) return false;
    var b2 = a2[1] + "";
    if (0 == b2.length) return false;
    var c2 = Number(b2);
    if (c2 + "" != b2) return false;
    b2 = +c2;
    return b2 < 0 ? false : b2 > 128 ? false : nd(a2[0] + "");
  };
  hb = function(b2, c2, f2, h3, r3, e2, d) {
    b2.issues.push(a(c2, f2, h3, r3, e2, d));
  }, od = function(e2, d, g2) {
    var n2;
    if (X(e2) && X(e2._zod) && "function" == typeof e2._zod.check) {
      var i = e2._zod;
      n2 = i.check.call(e2, d);
      if ($(n2)) {
        if (g2 !== void 0 && false === g2.async) throw new Error(qf);
        d.$pending = n2;
      }
      return;
    }
    i = vc(e2);
    var s2 = i.check + "";
    n2 = d.value;
    var l2, y;
    if ("min_length" == s2 || "max_length" == s2 || "length_equals" == s2) {
      g2 = "string" == typeof n2;
      if (!g2 && !Array.isArray(n2)) return;
      e2 = n2.length, g2 && (e2 = Array.from(n2).length), g2 = Array.isArray(n2) ? "array" : "string", "min_length" == s2 && e2 < +i.minimum && (l2 = d.issues, l2.push(a(false, g2, +i.minimum, true, false, n2))), "max_length" == s2 && e2 > +i.maximum && (l2 = d.issues, l2.push(a(true, g2, +i.maximum, true, false, n2))), "length_equals" == s2 && e2 != i.length && (e2 < i.length ? (e2 = d.issues, e2.push(a(false, g2, i.length, true, true, n2))) : (e2 = d.issues, e2.push(a(true, g2, i.length, true, true, n2))));
      return;
    }
    if ("min_size" == s2 || "max_size" == s2 || "size_equals" == s2) {
      if (Array.isArray(n2)) e2 = n2.length;
      else if (X(n2) && n2.size !== void 0) e2 = +n2.size;
      else {
        return;
      }
      g2 = Array.isArray(n2) ? "array" : "set";
      pa(Map, n2) && (g2 = "map"), l2 = globalThis.File, l2 !== void 0 && pa(l2, n2) && (g2 = "file"), "min_size" == s2 && e2 < +i.minimum && hb(d, false, g2, i.minimum, true, false, n2), "max_size" == s2 && e2 > +i.maximum && hb(d, true, g2, i.maximum, true, false, n2), "size_equals" == s2 && e2 != +i.size && (e2 < +i.size ? (e2 = i.size, hb(d, false, g2, e2, true, true, n2)) : hb(d, true, g2, i.size, true, true, n2));
      return;
    }
    if ("greater_than" == s2 || "less_than" == s2) {
      if ("number" != typeof n2 && !pa(Date, n2) && "bigint" != typeof n2) return;
      e2 = i.value, g2 = i.inclusive, l2 = false !== g2, g2 = pa(Date, n2) ? "date" : "number", "bigint" == typeof n2 && (g2 = "bigint"), i.origin === void 0 || (g2 = i.origin + "");
      if ("greater_than" == s2) {
        i = n2 > e2, l2 && (i = n2 >= e2);
        if (i) return;
        hb(d, false, g2, e2, l2, false, n2);
      } else {
        i = n2 < e2, l2 && (i = n2 <= e2);
        if (i) return;
        hb(d, true, g2, e2, l2, false, n2);
      }
      return;
    }
    if ("multiple_of" == s2) {
      if ("bigint" == typeof n2) {
        e2 = n2 % i.value, e2 === BigInt(0) || (e2 = d.issues, g2 = i.value, e2.push({ code: "not_multiple_of", divisor: g2, path: [] }));
        return;
      }
      if ("number" != typeof n2) return;
      e2 = +n2, 0 != b(e2, +i.value) && (e2 = d.issues, g2 = i.value, e2.push({ origin: "number", code: "not_multiple_of", divisor: g2, input: n2 }));
      return;
    }
    if ("number_format" == s2) {
      e2 = i.format + "", "safeint" == e2 && true !== Number.isSafeInteger(n2) && (+n2 > 0 ? d.issues.push(a(true, "number", 9007199254740991, true, false, n2)) : d.issues.push(a(false, "number", -9007199254740991, true, false, n2))), ("int32" == e2 || "safeint" == e2 || "int" == e2) && (true === Number.isInteger(n2) || d.issues.push(Ba("int", n2))), "finite" == e2 && !Oa(n2) && d.issues.push(Ba("number", n2));
      return;
    }
    if ("includes" == s2) {
      if ("string" != typeof n2) return;
      g2 = n2 + "", n2 = i.includes + "", e2 = g2.includes(n2), "number" == typeof i.position && (e2 = g2.slice(i.position | 0).includes(n2));
      if (e2) return;
      ta(d, "includes", { includes: i.includes });
      return;
    }
    if ("starts_with" == s2) {
      if ("string" != typeof n2) return;
      e2 = n2 + "";
      if (e2.startsWith(i.prefix + "")) return;
      ta(d, "starts_with", { prefix: i.prefix });
      return;
    }
    if ("ends_with" == s2) {
      if ("string" != typeof n2) return;
      e2 = n2 + "";
      if (e2.endsWith(i.suffix + "")) return;
      ta(d, "ends_with", { suffix: i.suffix });
      return;
    }
    if ("string_format" == s2 || "lowercase" == s2 || "uppercase" == s2) {
      if ("string" != typeof n2) return;
      e2 = n2 + "";
      var m2 = s2;
      "string_format" == s2 && (m2 = i.format + "");
      if ("lowercase" == m2 && e2 != e2.toLowerCase()) {
        ta(d, "lowercase", void 0);
        return;
      }
      if ("uppercase" == m2 && e2 != e2.toUpperCase()) {
        ta(d, "uppercase", void 0);
        return;
      }
      if ("url" == m2) {
        ((a2, b2, c2) => {
          var r3 = c2.trim();
          if (!a2.normalize && a2.protocol !== void 0 && a2.protocol != null && !/^https?:\/\//i.test(r3)) {
            ta(b2, "url", void 0);
            return;
          }
          var h3;
          try {
            h3 = new URL(r3);
          } catch {
            ta(b2, "url", void 0);
            return;
          }
          if (a2.hostname !== void 0 && a2.hostname != null) {
            c2 = a2.hostname, c2.lastIndex = 0;
            var f2 = h3.hostname;
            c2.test(f2) || ta(b2, "url", { note: "Invalid hostname", pattern: c2.source });
          }
          a2.protocol !== void 0 && a2.protocol != null && (f2 = a2.protocol, c2 = h3.protocol + "", c2.endsWith(":") && (c2 = c2.slice(0, c2.length - 1)), f2.lastIndex = 0, f2.test(c2) || ta(b2, "url", { note: "Invalid protocol", pattern: f2.source }));
          b2.value = r3.replace(/[\t\n\r]/g, Af) + "", a2.normalize && (b2.value = h3.href);
        })(i, d, n2);
        return;
      }
      if ("credit_card" == m2) {
        if (c(e2)) return;
        ta(d, "credit_card", void 0);
        return;
      }
      if ("jwt" == m2) {
        if (f(e2, i.alg)) return;
        n2 = { code: "invalid_format", format: "jwt", input: n2 }, i.alg !== void 0 && i.alg != null && (n2.algorithm = i.alg), e2 = d.issues, e2.push(n2);
        return;
      }
      if ("base64" == m2) {
        if (md(e2)) return;
        ta(d, "base64", void 0);
        return;
      }
      if ("base64url" == m2) {
        if (h2(e2)) return;
        ta(d, "base64url", void 0);
        return;
      }
      if ("ipv6" == m2) {
        if (nd(e2)) return;
        ta(d, "ipv6", void 0);
        return;
      }
      if ("cidrv6" == m2) {
        r2(e2) || ta(d, "cidrv6", void 0);
        return;
      }
      if ("function" == typeof i.fn) {
        e2 = i.fn(n2);
        if ($(e2)) {
          d.$pending = e2.then(function(a2) {
            !a2 && d.issues.push({ code: "invalid_format", format: m2, input: n2, continue: !i.abort });
            return d;
          });
          return;
        }
        if (e2) return;
        e2 = d.issues, g2 = m2, e2.push({ code: "invalid_format", format: g2, input: n2, continue: !i.abort });
        return;
      }
      if (i.pattern !== void 0) {
        g2 = i.pattern, g2.lastIndex = 0, g2 = i.pattern;
        if (g2.test(e2)) return;
        g2 = i.pattern, e2 = { pattern: g2.toString() }, ta(d, m2, e2);
      }
      return;
    }
    if ("mime_type" == s2) {
      e2 = i.mime;
      if (!Array.isArray(e2)) return;
      for (s2 = n2.type + "", i = e2.length, g2 = 0; g2 < i; g2++) if (e2[g2] + "" == s2) return;
      g2 = n2.type, d.issues.push({ code: "invalid_value", values: e2, input: g2 });
      return;
    }
    if ("property" == s2) {
      if (e2 = sa(i.schema)) {
        e2 = va(e2, n2[i.property], g2);
        if ($(e2)) {
          g2 = i.property, d.$pending = e2.then(function(a2) {
            for (var c2 = a2.issues, f2 = c2.length, b2 = 0; b2 < f2; b2++) wb(c2[b2], g2), d.issues.push(c2[b2]);
            return d;
          });
          return;
        }
        for (g2 = e2.issues, n2 = g2.length, e2 = 0; e2 < n2; e2++) s2 = g2[e2], wb(s2, i.property), d.issues.push(g2[e2]);
      }
      return;
    }
    if ("overwrite" == s2) {
      "function" == typeof i.transform && (d.value = i.transform(n2));
      return;
    }
    if ("custom" == s2) {
      s2 = void 0, X(e2._zod) && (l2 = e2._zod, s2 = l2.bag);
      if (X(s2) && s2.Class !== void 0) {
        if (pa(s2.Class, n2)) return;
        e2 = d.issues, g2 = s2.Class, e2.push(Ba(g2.name + "", n2));
        return;
      }
      if ("function" != typeof i.fn) return;
      s2 = d.issues, l2 = { value: n2, issues: s2, addIssue: function(a2) {
        "string" == typeof a2 ? a2 = { message: a2, code: "custom", input: n2, inst: e2 } : (!a2.fatal || (a2.continue = false), a2.code === void 0 && (a2.code = "custom"), true === "input" in a2 || (a2.input = n2), a2.inst === void 0 && (a2.inst = e2), a2.continue === void 0 && (a2.continue = !i.abort));
        s2.push(a2);
      } }, y = s2.length, l2 = i.fn(n2, l2);
      if ($(l2)) {
        if (g2 !== void 0 && false === g2.async) throw new Error(qf);
        d.$pending = l2.then(function(a2) {
          ld(a2, d, n2, e2, i, y);
          return d;
        });
        return;
      }
      ld(l2, d, n2, e2, i, y);
      return;
    }
  };
})();
var rc = (a) => "bigint" == typeof a ? a + "" : a;
var sc = (a, b) => {
  b == null && (b = []);
  var c = ja();
  c.add("$ZodError"), c.add("ZodError"), a.name = "ZodError", ba(a, "issues", Nb(b)), ba(a, "_zod", Nb({ def: b, traits: c })), ba(a, "message", { enumerable: true, configurable: true, get: function() {
    var a2 = this._zod;
    if (a2.message !== void 0) return a2.message;
    var b2 = a2.def;
    a2.message = JSON.stringify(b2, function(a3, b3) {
      return rc(b3);
    }, 2);
    return a2.message;
  }, set: function(a2) {
    let b2 = this._zod;
    b2.message = a2;
  } });
  return a;
};
var Le;
var af;
var bf;
var cf;
var df;
var ef;
var Fc;
var ff;
var gf;
var hf;
var jf;
var kf;
(function() {
  let a = (a2, b2, c2, f2) => {
    ba(a2, b2, { configurable: true, enumerable: false, get: function() {
      let a3 = c2(this);
      ba(this, b2, { configurable: true, writable: true, enumerable: f2, value: a3 });
      return a3;
    }, set: function(a3) {
      ba(this, b2, { configurable: true, writable: true, enumerable: true, value: a3 });
    } });
  }, f = (a2, b2) => {
    var f2 = a2.prototype, c2 = function(a3, b3) {
      ba(f2, a3, { configurable: true, enumerable: true, get: function() {
        let a4 = this._zod;
        return a4.def[b3];
      } });
    };
    "ZodArray" == b2 && c2("element", "element");
    ("ZodRecord" == b2 || "ZodMap" == b2) && (c2("keyType", "keyType"), c2("valueType", "valueType")), "ZodSet" == b2 && c2("valueType", "valueType"), ("ZodUnion" == b2 || "ZodDiscriminatedUnion" == b2 || "ZodXor" == b2) && (c2("options", "options"), c2("discriminator", "discriminator")), ("ZodPipe" == b2 || "ZodCodec" == b2 || "ZodPreprocess" == b2) && (c2("in", "in"), c2("out", "out")), "ZodEnum" == b2 && (ba(f2, "enum", { configurable: true, enumerable: true, get: function() {
      var b3, c3 = this._zod, f3 = c3.def, a3 = f3.entries;
      if (X(a3) && !Array.isArray(a3)) return a3;
      c3 = {};
      if (Array.isArray(a3)) for (f3 = a3.length, b3 = 0; b3 < f3; b3++) c3[a3[b3]] = a3[b3];
      return c3;
    } }), ba(f2, "options", { configurable: true, enumerable: true, get: function() {
      let a3 = this._zod, b3 = a3.def;
      return Gd(b3.entries);
    } })), "ZodLiteral" == b2 && (ba(f2, "values", { configurable: true, enumerable: true, get: function() {
      let a3 = this._zod;
      return a3.values;
    } }), ba(f2, "value", { configurable: true, enumerable: true, get: function() {
      var b3 = this._zod, c3 = b3.def, a3 = c3.values;
      Array.isArray(a3) || (a3 = Array, b3 = this._zod, a3 = a3.from(b3.values));
      if (a3.length > 1) throw new Error("This schema contains multiple valid literal values. Use `.values` instead.");
      return a3[0];
    } }));
  }, c = (a2, b2, c2, f2) => {
    ha(c2, Sa(f2));
    return fa(a2, da(b2, c2));
  }, r2 = (a2, b2, c2) => {
    c2 = aa(0 == a2 ? "string" : 1 == a2 ? "number" : 2 == a2 ? "boolean" : 3 == a2 ? "bigint" : 4 == a2 ? "symbol" : 5 == a2 ? "date" : 6 == a2 ? "nan" : 7 == a2 ? "undefined" : 8 == a2 ? "null" : 9 == a2 ? "any" : 10 == a2 ? "unknown" : 11 == a2 ? "never" : 12 == a2 ? "void" : 13 == a2 ? "literal" : 14 == a2 ? "enum" : 15 == a2 ? "object" : 16 == a2 ? "array" : 17 == a2 ? "tuple" : 18 == a2 ? "record" : 19 == a2 ? "map" : 20 == a2 ? "set" : 21 == a2 ? "union" : 38 == a2 ? "union" : 22 == a2 ? "intersection" : 23 == a2 ? "optional" : 24 == a2 ? "nullable" : 25 == a2 ? "default" : 26 == a2 ? "prefault" : 27 == a2 ? "catch" : 28 == a2 ? "nonoptional" : 29 == a2 ? "lazy" : 30 == a2 ? "promise" : 31 == a2 ? "transform" : 32 == a2 ? "pipe" : 33 == a2 ? "readonly" : 34 == a2 ? "custom" : 35 == a2 ? "file" : 36 == a2 ? "custom" : 37 == a2 ? "success" : 39 == a2 ? "function" : 40 == a2 ? "template_literal" : "custom", c2), c2.innerType = b2;
    return new (0 == a2 ? Z : 1 == a2 ? ga : 2 == a2 ? cb : 3 == a2 ? na : 4 == a2 ? gc : 5 == a2 ? Da : 6 == a2 ? hc : 7 == a2 ? ic : 8 == a2 ? Db : 9 == a2 ? jc : 10 == a2 ? La : 11 == a2 ? lb : 12 == a2 ? kc : 13 == a2 ? Eb : 14 == a2 ? Ma : 15 == a2 ? ka : 16 == a2 ? za : 17 == a2 ? lc : 18 == a2 ? Fb : 19 == a2 ? mb : 20 == a2 ? nb : 21 == a2 ? Na : 38 == a2 ? Gb : 22 == a2 ? Hb : 23 == a2 ? db : 24 == a2 ? nc : 25 == a2 ? Ib : 26 == a2 ? Jb : 27 == a2 ? Kb : 28 == a2 ? oc : 29 == a2 ? ob : 30 == a2 ? Lb : 31 == a2 ? pb : 32 == a2 ? Ha : 33 == a2 ? Sc : 34 == a2 ? Ua : 35 == a2 ? rb : 37 == a2 ? pc : 39 == a2 ? Va : 40 == a2 ? qc : Y)(c2);
  }, d = (a2) => {
    var b2 = ca(a2).def.shape;
    return b2 == null ? {} : b2;
  }, e2 = (a2) => {
    var b2 = ca(a2).def.checks;
    return Array.isArray(b2) && b2.length > 0;
  }, g2 = (a2, b2) => {
    let c2 = {};
    Ca(c2, { configurable: true, enumerable: true, get: function() {
      var c3 = {}, f2 = d(a2);
      tb(c3, f2), b2 !== void 0 && X(b2) && b2 != null && tb(c3, b2), Ca(this, { value: c3, writable: true, enumerable: true, configurable: true });
      return c3;
    } });
    return c2;
  }, i = (a2, b2, c2, f2) => {
    if (c2 && e2(a2)) {
      if (f2) throw la(tf);
      throw la(uf);
    }
    var d2 = {}, h3 = ea.defineProperties(d2, ea.getOwnPropertyDescriptors(ca(a2).def));
    Ca(h3, { configurable: true, enumerable: true, get: function() {
      var e3, n2, s3, h4, d3, i2, g3 = ca(a2).def.shape;
      g3 == null && (g3 = {}), b2 !== void 0 && b2 != null && ((a3, b3) => {
        var c3 = Reflect.ownKeys(b3), f3 = c3.length;
        b3 = 0;
        while (b3 < f3) if ("string" != typeof c3[b3]) b3++;
        else {
          var h5 = c3[b3];
          if (!Ga.call(a3, h5)) throw la('Unrecognized key: "' + c3[b3] + yf);
          b3++;
        }
      })(g3, b2), i2 = Reflect.ownKeys(g3), n2 = {}, s3 = i2.length, d3 = 0;
      while (d3 < s3) {
        e3 = i2[d3];
        if ("string" != typeof e3) d3++;
        else h4 = g3[e3], (b2 == null || b2[e3]) && (h4 = c2 ? f2 ? new eb({ type: "optional", innerType: h4, exact: true }) : r2(23, h4, void 0) : r2(28, h4, void 0)), ba(n2, e3, { value: h4, writable: true, enumerable: true, configurable: true }), d3++;
      }
      Ca(this, { value: n2, writable: true, enumerable: true, configurable: true });
      return n2;
    } }), c2 && (h3.checks = []);
    return new ka(h3);
  }, b = (a2, b2, c2, f2) => {
    b2 = { format: b2, pattern: c2 }, c2 = ((a3) => {
      if ("string" == typeof a3) return a3;
      if (X(a3) && a3 != null) {
        if ("string" == typeof a3.error) return a3.error;
        if ("string" == typeof a3.message) return a3.message;
      }
      return Af;
    })(f2) + "", c2.length > 0 && (b2.error = c2);
    return fa(a2, da("string_format", b2));
  }, s2 = () => {
    let a2 = db.prototype;
    h2(a2, function(a3) {
      return function() {
        return ca(a3).def.innerType;
      };
    }), a2 = nc.prototype, h2(a2, function(a3) {
      return function() {
        return ca(a3).def.innerType;
      };
    }), a2 = eb.prototype, h2(a2, function(a3) {
      return function() {
        return ca(a3).def.innerType;
      };
    }), a2 = oc.prototype, h2(a2, function(a3) {
      return function() {
        return ca(a3).def.innerType;
      };
    }), a2 = za.prototype, h2(a2, function(a3) {
      return function() {
        return ca(a3).def.element;
      };
    }), a2 = Ib.prototype, h2(a2, function(a3) {
      return function() {
        return ca(a3).def.innerType;
      };
    }), a2 = Kb.prototype, h2(a2, function(a3) {
      return function() {
        return ca(a3).def.innerType;
      };
    }), a2 = Jb.prototype, h2(a2, function(a3) {
      return function() {
        return ca(a3).def.innerType;
      };
    }), a2 = ob.prototype, h2(a2, function(a3) {
      return function() {
        return ca(a3).def.getter();
      };
    }), a2 = Lb.prototype, h2(a2, function(a3) {
      return function() {
        return ca(a3).def.innerType;
      };
    });
  }, h2 = (a2, b2) => {
    ba(a2, "unwrap", { configurable: true, enumerable: false, get: function() {
      let a3 = b2(this);
      ba(this, "unwrap", { configurable: true, writable: true, enumerable: true, value: a3 });
      return a3;
    }, set: function(a3) {
      ba(this, "unwrap", { configurable: true, writable: true, enumerable: true, value: a3 });
    } });
  };
  Le = function() {
    let b2 = ua.prototype, c2 = Error.prototype;
    ea.setPrototypeOf(b2, c2), b2 = ua, ba(b2, "name", Ia("ZodError")), b2 = ua, ba(b2, "init", Ia(function(a2, b3) {
      return sc(a2, b3);
    })), b2 = Ka.prototype, c2 = ua.prototype, ea.setPrototypeOf(b2, c2), ba(Ka, "name", Ia("ZodError")), b2 = Ka, ba(b2, "init", ua.init), b2 = ua, c2 = Symbol.hasInstance, ba(b2, c2, Ia(function(a2) {
      if (a2 == null || !X(a2)) return false;
      var b3 = a2._zod;
      return b3 == null ? false : !!b3.traits.has("ZodError");
    })), b2 = Ka, c2 = Symbol.hasInstance, ba(b2, c2, Ia(function(a2) {
      if (pa(Error, a2)) return true;
      if (a2 == null || !X(a2)) return false;
      var b3 = a2._zod;
      return b3 == null ? false : !!b3.traits.has("ZodError");
    })), b2 = ua.prototype, ba(b2, "toString", { configurable: true, enumerable: false, get: function() {
      var a2 = this;
      let b3 = function() {
        return a2.message;
      };
      ba(a2, "toString", { value: b3, configurable: true, writable: true });
      return b3;
    }, set: function(a2) {
      ba(this, "toString", { value: a2, configurable: true, writable: true });
    } }), b2 = ua.prototype, a(b2, "format", function(a2) {
      return function(b3) {
        let c3 = { _errors: [] };
        Sb(a2.issues, [], c3, b3);
        return c3;
      };
    }, false), b2 = ua.prototype, a(b2, "flatten", function(a2) {
      return function(b3) {
        return hd(a2, b3);
      };
    }, false), b2 = ua.prototype, a(b2, "addIssue", function(a2) {
      return function(b3) {
        let c3 = a2.issues;
        c3.push(b3), c3 = a2._zod, b3 = a2.issues, c3.message = JSON.stringify(b3, function(a3, b4) {
          return rc(b4);
        }, 2);
      };
    }, false), b2 = ua.prototype, a(b2, "addIssues", function(a2) {
      return function(b3) {
        for (var f2 = b3.length, c3 = 0; c3 < f2; c3++) a2.issues.push(b3[c3]);
        c3 = a2._zod, b3 = a2.issues, c3.message = JSON.stringify(b3, function(a3, b4) {
          return rc(b4);
        }, 2);
      };
    }, false), b2 = ua.prototype, ba(b2, "isEmpty", { enumerable: false, configurable: true, get: function() {
      let a2 = this.issues;
      return 0 == a2.length;
    } });
  }, af = function() {
    let b2 = Y.prototype, c2 = true;
    ba(b2, "_def", { configurable: c2, get: function() {
      let a2 = this._zod;
      return a2.def;
    } }), b2 = Y.prototype, a(b2, "parse", function(a2) {
      var b3;
      let c3 = a2._zod;
      b3 = ee(c3, function(c4, f2) {
        return Fd(a2, c4, f2, b3);
      });
      return b3;
    }, c2), b2 = Y.prototype, a(b2, "safeParse", function(a2) {
      return fe(a2._zod, function(b3, c3) {
        Xa();
        return jb(Ab(b3, ca(a2), c3));
      });
    }, c2), b2 = Y.prototype, a(b2, "parseAsync", function(a2) {
      var b3;
      b3 = new globalThis.Function("impl", sf)(function(c3, f2) {
        return Ra(a2, c3, f2, b3);
      });
      return b3;
    }, c2), b2 = Y.prototype, a(b2, "safeParseAsync", function(a2) {
      return function(b3, c3) {
        return ac(a2, b3, c3);
      };
    }, c2), b2 = Y.prototype, a(b2, "spa", function(a2) {
      return a2.safeParseAsync;
    }, c2), b2 = Y.prototype, a(b2, "encode", function(a2) {
      var b3 = function(c3, f2) {
        var h3 = f2;
        h3 = h3 == null ? {} : ha({}, h3), h3.direction = "backward";
        return Qa(a2, c3, h3, b3);
      };
      return b3;
    }, c2), b2 = Y.prototype, a(b2, "decode", function(a2) {
      var b3 = function(c3, f2) {
        var h3 = f2;
        h3 = h3 == null ? {} : ha({}, h3), h3.direction = "forward";
        return Qa(a2, c3, h3, b3);
      };
      return b3;
    }, c2), b2 = Y.prototype, a(b2, "encodeAsync", function(a2) {
      var b3;
      b3 = new globalThis.Function("impl", sf)(function(c3, f2) {
        var h3 = f2 == null ? {} : ha({}, f2);
        h3.direction = "backward";
        return Ra(a2, c3, h3, b3);
      });
      return b3;
    }, c2), b2 = Y.prototype, a(b2, "decodeAsync", function(a2) {
      var b3;
      b3 = new globalThis.Function("impl", sf)(function(c3, f2) {
        var h3 = f2 == null ? {} : ha({}, f2);
        h3.direction = "forward";
        return Ra(a2, c3, h3, b3);
      });
      return b3;
    }, c2), b2 = Y.prototype, a(b2, "optional", function(a2) {
      return function() {
        return r2(23, a2, void 0);
      };
    }, c2), b2 = Y.prototype, a(b2, "exactOptional", function(a2) {
      return function() {
        return new eb({ type: "optional", innerType: a2, exact: true });
      };
    }, c2), b2 = Y.prototype, a(b2, "nullable", function(a2) {
      return function() {
        return r2(24, a2, void 0);
      };
    }, c2), b2 = Y.prototype, a(b2, "nullish", function(a2) {
      return function() {
        return r2(23, r2(24, a2, void 0), void 0);
      };
    }, c2), b2 = Y.prototype, a(b2, "array", function(a2) {
      return function() {
        let b3 = aa("array", void 0);
        b3.element = a2;
        return new za(b3);
      };
    }, c2), b2 = Y.prototype, a(b2, "or", function(a2) {
      return function(b3) {
        let f2 = [a2, b3], c3 = aa("union", void 0);
        c3.options = f2;
        return new Na(c3);
      };
    }, c2), b2 = Y.prototype, a(b2, "and", function(a2) {
      return function(b3) {
        return new Hb({ type: "intersection", left: a2, right: b3 });
      };
    }, c2), b2 = Y.prototype, a(b2, "optional", function(a2) {
      return function() {
        return r2(23, a2, void 0);
      };
    }, c2), b2 = Y.prototype, a(b2, "default", function(a2) {
      return function(b3) {
        let c3 = { type: "default", innerType: a2 };
        ba(c3, "defaultValue", { configurable: true, enumerable: true, get: function() {
          return "function" == typeof b3 ? b3() : b3;
        } });
        return new Ib(c3);
      };
    }, c2), b2 = Y.prototype, a(b2, "prefault", function(a2) {
      return function(b3) {
        let c3 = { type: "prefault", innerType: a2 };
        ba(c3, "defaultValue", { configurable: true, enumerable: true, get: function() {
          return "function" == typeof b3 ? b3() : b3;
        } });
        return new Jb(c3);
      };
    }, c2), b2 = Y.prototype, a(b2, "catch", function(a2) {
      return function(b3) {
        var c3 = "function" != typeof b3 ? function() {
          return b3;
        } : b3;
        return new Kb({ type: "catch", innerType: a2, catchValue: c3 });
      };
    }, c2), b2 = Y.prototype, a(b2, "removeDefault", function(a2) {
      return function() {
        return ca(a2).def.innerType;
      };
    }, c2), b2 = Y.prototype, a(b2, "removeCatch", function(a2) {
      return function() {
        return ca(a2).def.innerType;
      };
    }, c2), b2 = Y.prototype, a(b2, "nonoptional", function(a2) {
      return function(b3) {
        return r2(28, a2, b3);
      };
    }, c2), b2 = Y.prototype, a(b2, "transform", function(a2) {
      return function(b3) {
        let c3 = new pb({ type: "transform", transform: b3 });
        return new Ha({ type: "pipe", in: a2, out: c3 });
      };
    }, c2), b2 = Y.prototype, a(b2, "pipe", function(a2) {
      return function(b3) {
        return new Ha({ type: "pipe", in: a2, out: b3 });
      };
    }, c2), b2 = Y.prototype, a(b2, "readonly", function(a2) {
      return function() {
        return r2(33, a2, void 0);
      };
    }, c2), b2 = Y.prototype, a(b2, "brand", function(a2) {
      return function() {
        return a2;
      };
    }, c2), b2 = Y.prototype, a(b2, "describe", function(a2) {
      return function(b3) {
        let c3 = wa(a2, void 0);
        Ya().add.call(Ya(), c3);
        let f2 = Ya();
        f2.add(c3, { description: b3 });
        return c3;
      };
    }, c2), b2 = Y.prototype, a(b2, "meta", function(a2) {
      return function(b3) {
        var c3 = Ya();
        if (b3 === void 0) return c3.get(a2);
        var f2 = wa(a2, void 0);
        c3.add(f2, b3);
        return f2;
      };
    }, c2), b2 = Y.prototype, a(b2, "refine", function(a2) {
      return function(b3, c3) {
        let f2 = c3;
        f2 = aa("custom", f2), f2.fn = b3, f2.check = "custom";
        return fa(a2, new Ua(f2));
      };
    }, c2), b2 = Y.prototype, a(b2, "superRefine", function(a2) {
      return function(b3, c3) {
        return fa(a2, Ld(b3, c3));
      };
    }, c2), b2 = Y.prototype, a(b2, "overwrite", function(a2) {
      return function(b3) {
        return fa(a2, da("overwrite", { transform: b3 }));
      };
    }, c2), b2 = Y.prototype, a(b2, "check", function(a2) {
      return function() {
        for (var b3, h3 = arguments.length, c3 = a2, f2 = 0; f2 < h3; f2++) b3 = arguments[f2], "function" == typeof b3 ? c3 = fa(c3, Ub(b3, void 0)) : X(b3) && b3._zod !== void 0 && (c3 = fa(c3, b3));
        return c3;
      };
    }, c2), b2 = Y.prototype, a(b2, "with", function(a2) {
      return a2.check;
    }, c2), b2 = Y.prototype, a(b2, "clone", function(a2) {
      return function(b3) {
        return wa(a2, b3);
      };
    }, c2), b2 = Y.prototype, a(b2, "description", function(a2) {
      var b3 = Ya().get.call(Ya(), a2);
      return b3 == null ? void 0 : b3.description;
    }, c2), b2 = Y.prototype, a(b2, "isOptional", function(a2) {
      return function() {
        return $b(a2, void 0, void 0).success;
      };
    }, c2), b2 = Y.prototype, a(b2, "isNullable", function(a2) {
      return function() {
        return $b(a2, null, void 0).success;
      };
    }, c2), b2 = Y.prototype, a(b2, "apply", function(a2) {
      return function(b3) {
        var f2 = [a2];
        for (var h3, r3 = arguments.length, c3 = 1; c3 < r3; c3++) h3 = arguments[c3], f2.push(h3);
        return b3.apply(void 0, f2);
      };
    }, c2), b2 = Y.prototype, a(b2, "register", function(a2) {
      return function(b3, c3) {
        b3.add(a2, c3);
        return a2;
      };
    }, c2), b2 = Y.prototype, a(b2, "~standard", function(a2) {
      return { version: 1, vendor: "zod", validate: function(b3) {
        try {
          var c3 = $b(a2, b3, void 0);
          return c3.success ? { value: c3.data } : { issues: c3.error.issues };
        } catch {
          return ac(a2, b3, void 0).then(function(a3) {
            return a3.success ? { value: a3.data } : { issues: a3.error.issues };
          });
        }
      }, jsonSchema: { input: function(b3) {
        a2.constructor;
        if (X(a2._zod) && "function" == typeof a2.toJSONSchema) {
          var c3 = b3;
          c3 = c3 == null ? {} : ha({}, c3), c3.io = "input";
          return a2.toJSONSchema(c3);
        }
        return { type: ca(a2).typeName };
      }, output: function(b3) {
        if ("function" == typeof a2.toJSONSchema) {
          var c3 = b3;
          c3 = c3 == null ? {} : ha({}, c3), c3.io = "output";
          return a2.toJSONSchema(c3);
        }
        return { type: ca(a2).typeName };
      } } };
    }, false), b2 = Y.prototype, a(b2, "toJSONSchema", function(a2) {
      return function() {
        return { type: ca(a2).typeName };
      };
    }, c2), b2 = Z.prototype, a(b2, "format", function(a2) {
      var c3 = a2._zod, b3 = c3.bag;
      return b3.format === void 0 ? null : b3.format;
    }, c2), b2 = Z.prototype, a(b2, "minLength", function(a2) {
      var c3 = a2._zod, b3 = c3.bag;
      return b3.minimum === void 0 ? null : b3.minimum;
    }, c2), b2 = Z.prototype, a(b2, "maxLength", function(a2) {
      var c3 = a2._zod, b3 = c3.bag;
      return b3.maximum === void 0 ? null : b3.maximum;
    }, c2);
  }, bf = function() {
    let f2 = Z.prototype, h3 = true;
    a(f2, "min", function(a2) {
      return function(b2, f3) {
        return c(a2, "min_length", { minimum: b2 }, f3);
      };
    }, h3), f2 = Z.prototype, a(f2, "max", function(a2) {
      return function(b2, f3) {
        return c(a2, "max_length", { maximum: b2 }, f3);
      };
    }, h3), f2 = Z.prototype, a(f2, "length", function(a2) {
      return function(b2, f3) {
        return c(a2, "length_equals", { length: b2 }, f3);
      };
    }, h3), f2 = Z.prototype, a(f2, "nonempty", function(a2) {
      return function(b2) {
        return c(a2, "min_length", { minimum: 1 }, b2);
      };
    }, h3), f2 = Z.prototype, a(f2, "includes", function(a2) {
      return function(b2, f3) {
        var h4 = { includes: b2 };
        X(f3) && "number" == typeof f3.position && (h4.position = f3.position);
        return c(a2, "includes", h4, f3);
      };
    }, h3), f2 = Z.prototype, a(f2, "startsWith", function(a2) {
      return function(b2, f3) {
        return c(a2, "starts_with", { prefix: b2 }, f3);
      };
    }, h3), f2 = Z.prototype, a(f2, "endsWith", function(a2) {
      return function(b2, f3) {
        return c(a2, "ends_with", { suffix: b2 }, f3);
      };
    }, h3), f2 = Z.prototype, a(f2, "regex", function(a2) {
      return function(b2, f3) {
        return c(a2, "string_format", { format: "regex", pattern: b2 }, f3);
      };
    }, h3), f2 = Z.prototype, a(f2, "email", function(a2) {
      return function(c2) {
        return b(a2, "email", Jc, c2);
      };
    }, h3), f2 = Z.prototype, a(f2, "url", function(a2) {
      return function(b2) {
        var f3 = { format: "url" };
        X(b2) && b2.hostname !== void 0 && (f3.hostname = b2.hostname), X(b2) && b2.protocol !== void 0 && (f3.protocol = b2.protocol), X(b2) && b2.normalize !== void 0 && (f3.normalize = b2.normalize);
        return c(a2, "string_format", f3, b2);
      };
    }, h3), f2 = Z.prototype, a(f2, "uuid", function(a2) {
      return function(c2) {
        return b(a2, "uuid", kb, c2);
      };
    }, h3), f2 = Z.prototype, a(f2, "uuidv4", function(a2) {
      return function(c2) {
        return b(a2, "uuid", kb, c2);
      };
    }, h3), f2 = Z.prototype, a(f2, "uuidv6", function(a2) {
      return function(c2) {
        return b(a2, "uuid", kb, c2);
      };
    }, h3), f2 = Z.prototype, a(f2, "uuidv7", function(a2) {
      return function(c2) {
        return b(a2, "uuid", kb, c2);
      };
    }, h3), f2 = Z.prototype, a(f2, "guid", function(a2) {
      return function(c2) {
        return b(a2, "guid", Kc, c2);
      };
    }, h3), f2 = Z.prototype, a(f2, "cuid", function(a2) {
      return function(c2) {
        return b(a2, "cuid", /^[cC][0-9a-z]{6,}$/, c2);
      };
    }, h3), f2 = Z.prototype, a(f2, "cuid2", function(a2) {
      return function(c2) {
        return b(a2, "cuid2", /^[0-9a-z]+$/, c2);
      };
    }, h3), f2 = Z.prototype, a(f2, "ulid", function(a2) {
      return function(c2) {
        return b(a2, "ulid", Lc, c2);
      };
    }, h3), f2 = Z.prototype, a(f2, "nanoid", function(a2) {
      return function(c2) {
        return b(a2, "nanoid", Mc, c2);
      };
    }, h3), f2 = Z.prototype, a(f2, "base64", function(a2) {
      return function(c2) {
        let f3 = new globalThis.RegExp("^$|^(?:[0-9a-zA-Z+/]{4})*(?:(?:[0-9a-zA-Z+/]{2}==)|(?:[0-9a-zA-Z+/]{3}=))?$");
        return b(a2, "base64", f3, c2);
      };
    }, h3), f2 = Z.prototype, a(f2, "base64url", function(a2) {
      return function(c2) {
        return b(a2, "base64url", /^[A-Za-z0-9_-]*$/, c2);
      };
    }, h3), f2 = Z.prototype, a(f2, "ipv4", function(a2) {
      return function(c2) {
        return b(a2, "ipv4", Nc, c2);
      };
    }, h3), f2 = Z.prototype, a(f2, "ipv6", function(a2) {
      return function(c2) {
        return b(a2, "ipv6", Oc, c2);
      };
    }, h3), f2 = Z.prototype, a(f2, "cidrv4", function(a2) {
      return function(c2) {
        return b(a2, "cidrv4", /^((25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]|[0-9])\.){3}(25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]|[0-9])\/([0-9]|[1-2][0-9]|3[0-2])$/, c2);
      };
    }, h3), f2 = Z.prototype, a(f2, "cidrv6", function(a2) {
      return function(c2) {
        return b(a2, "cidrv6", /^(([0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}|([0-9a-fA-F]{1,4}:){1,7}:|([0-9a-fA-F]{1,4}:){1,6}:[0-9a-fA-F]{1,4}|([0-9a-fA-F]{1,4}:){1,5}(:[0-9a-fA-F]{1,4}){1,2}|([0-9a-fA-F]{1,4}:){1,4}(:[0-9a-fA-F]{1,4}){1,3}|([0-9a-fA-F]{1,4}:){1,3}(:[0-9a-fA-F]{1,4}){1,4}|([0-9a-fA-F]{1,4}:){1,2}(:[0-9a-fA-F]{1,4}){1,5}|[0-9a-fA-F]{1,4}:((:[0-9a-fA-F]{1,4}){1,6})|:((:[0-9a-fA-F]{1,4}){1,7}|:))\/(12[0-8]|1[01][0-9]|[1-9]?[0-9])$/, c2);
      };
    }, h3), f2 = Z.prototype, a(f2, "jwt", function(a2) {
      return function(b2) {
        var f3 = { format: "jwt" };
        X(b2) && "string" == typeof b2.alg && (f3.alg = b2.alg);
        return c(a2, "string_format", f3, b2);
      };
    }, h3), f2 = Z.prototype, a(f2, "emoji", function(a2) {
      return function(c2) {
        return b(a2, "emoji", ne, c2);
      };
    }, h3), f2 = Z.prototype, a(f2, "e164", function(a2) {
      return function(c2) {
        return b(a2, "e164", /^\+[1-9]\d{6,14}$/, c2);
      };
    }, h3), f2 = Z.prototype, a(f2, "mac", function(a2) {
      return function(c2) {
        let f3;
        f3 = X(c2) && "string" == typeof c2.delimiter ? c2.delimiter + "" : ":", f3 = ma(Ef + f3 + xf + f3 + Df);
        return b(a2, "mac", f3, c2);
      };
    }, h3), f2 = Z.prototype, a(f2, "creditCard", function(a2) {
      return function(c2) {
        return b(a2, "credit_card", Qc, c2);
      };
    }, h3), f2 = Z.prototype, a(f2, "xid", function(a2) {
      return function(c2) {
        return b(a2, "xid", /^[0-9a-vA-V]{20}$/, c2);
      };
    }, h3), f2 = Z.prototype, a(f2, "ksuid", function(a2) {
      return function(c2) {
        return b(a2, "ksuid", /^[A-Za-z0-9]{27}$/, c2);
      };
    }, h3), f2 = Z.prototype, a(f2, "datetime", function(a2) {
      return function(c2) {
        return b(a2, "datetime", Jd(c2), c2);
      };
    }, h3), f2 = Z.prototype, a(f2, "date", function(a2) {
      return function(c2) {
        let f3 = ma("^(?:(?:\\d\\d[2468][048]|\\d\\d[13579][26]|\\d\\d0[48]|[02468][048]00|[13579][26]00)-02-29|\\d{4}-(?:(?:0[13578]|1[02])-(?:0[1-9]|[12]\\d|3[01])|(?:0[469]|11)-(?:0[1-9]|[12]\\d|30)|(?:02)-(?:0[1-9]|1\\d|2[0-8])))$");
        return b(a2, "date", f3, c2);
      };
    }, h3), f2 = Z.prototype, a(f2, "time", function(a2) {
      return function(c2) {
        return b(a2, "time", Id(c2), c2);
      };
    }, h3), f2 = Z.prototype, a(f2, "duration", function(a2) {
      return function(c2) {
        return b(a2, "duration", Pc, c2);
      };
    }, h3), f2 = Z.prototype, a(f2, "trim", function(a2) {
      return function() {
        return fa(a2, da("overwrite", { transform: function(a3) {
          return a3.trim();
        } }));
      };
    }, h3), f2 = Z.prototype, a(f2, "toLowerCase", function(a2) {
      return function() {
        return fa(a2, da("overwrite", { transform: function(a3) {
          return a3.toLowerCase();
        } }));
      };
    }, h3), f2 = Z.prototype, a(f2, "toUpperCase", function(a2) {
      return function() {
        return fa(a2, da("overwrite", { transform: function(a3) {
          return a3.toUpperCase();
        } }));
      };
    }, h3), f2 = Z.prototype, a(f2, "lowercase", function(a2) {
      return function() {
        return fa(a2, da("lowercase", {}));
      };
    }, h3), f2 = Z.prototype, a(f2, "uppercase", function(a2) {
      return function() {
        return fa(a2, da("uppercase", {}));
      };
    }, h3), f2 = Z.prototype, a(f2, "normalize", function(a2) {
      return function(b2) {
        var c2 = "NFC";
        "string" == typeof b2 && (c2 = b2);
        return fa(a2, da("overwrite", { transform: function(a3) {
          return a3.normalize(c2);
        } }));
      };
    }, h3), f2 = Z.prototype, a(f2, "slugify", function(a2) {
      return function() {
        return fa(a2, da("overwrite", { transform: function(a3) {
          return a3.toLowerCase().trim().replace(/[^\w\s-]/g, Af).replace(/[\s_-]+/g, "-").replace(/^-+|-+$/g, Af);
        } }));
      };
    }, h3);
  }, cf = function() {
    let b2 = ga.prototype, f2 = true;
    a(b2, "min", function(a2) {
      return function(b3, f3) {
        let h3 = b3;
        h3 = { value: h3, inclusive: true, origin: "number" };
        return c(a2, "greater_than", h3, f3);
      };
    }, f2), b2 = ga.prototype, a(b2, "max", function(a2) {
      return function(b3, f3) {
        let h3 = b3;
        h3 = { value: h3, inclusive: true, origin: "number" };
        return c(a2, "less_than", h3, f3);
      };
    }, f2), b2 = ga.prototype, a(b2, "gt", function(a2) {
      return function(b3, f3) {
        let h3 = b3;
        h3 = { value: h3, inclusive: false, origin: "number" };
        return c(a2, "greater_than", h3, f3);
      };
    }, f2), b2 = ga.prototype, a(b2, "gte", function(a2) {
      return a2.min;
    }, f2), b2 = ga.prototype, a(b2, "lt", function(a2) {
      return function(b3, f3) {
        let h3 = b3;
        h3 = { value: h3, inclusive: false, origin: "number" };
        return c(a2, "less_than", h3, f3);
      };
    }, f2), b2 = ga.prototype, a(b2, "lte", function(a2) {
      return a2.max;
    }, f2), b2 = ga.prototype, a(b2, "int", function(a2) {
      return function() {
        return fa(a2, da("number_format", { format: "int" }));
      };
    }, f2), b2 = ga.prototype, a(b2, "positive", function(a2) {
      return function() {
        return fa(a2, da("greater_than", { value: 0, inclusive: false, origin: "number" }));
      };
    }, f2), b2 = ga.prototype, a(b2, "negative", function(a2) {
      return function() {
        return fa(a2, da("less_than", { value: 0, inclusive: false, origin: "number" }));
      };
    }, f2), b2 = ga.prototype, a(b2, "nonnegative", function(a2) {
      return function() {
        return fa(a2, da("greater_than", { value: 0, inclusive: true, origin: "number" }));
      };
    }, f2), b2 = ga.prototype, a(b2, "nonpositive", function(a2) {
      return function() {
        return fa(a2, da("less_than", { value: 0, inclusive: true, origin: "number" }));
      };
    }, f2), b2 = ga.prototype, a(b2, "multipleOf", function(a2) {
      return function(b3, f3) {
        return c(a2, "multiple_of", { value: b3 }, f3);
      };
    }, f2), b2 = ga.prototype, a(b2, "step", function(a2) {
      return a2.multipleOf;
    }, f2), b2 = ga.prototype, a(b2, "finite", function(a2) {
      return function() {
        return a2;
      };
    }, f2), b2 = ga.prototype, a(b2, "safe", function(a2) {
      return function() {
        return fa(a2, da("number_format", { format: "safeint" }));
      };
    }, f2), b2 = ga.prototype, a(b2, "format", function(a2) {
      var c2 = a2._zod, b3 = c2.bag;
      return b3.format === void 0 ? null : b3.format;
    }, f2), b2 = ga.prototype, a(b2, "minValue", function(a2) {
      var h3 = a2._zod, f3 = h3.bag, r3 = globalThis.Math, b3 = Number.NEGATIVE_INFINITY, c2 = f3.minimum;
      c2 === void 0 && (c2 = b3), a2 = f3.exclusiveMinimum, a2 === void 0 || (b3 = a2);
      return r3.max(c2, b3);
    }, f2), b2 = ga.prototype, a(b2, "maxValue", function(a2) {
      var h3 = a2._zod, f3 = h3.bag, r3 = globalThis.Math, b3 = Number.POSITIVE_INFINITY, c2 = f3.maximum;
      c2 === void 0 && (c2 = b3), a2 = f3.exclusiveMaximum, a2 === void 0 || (b3 = a2);
      return r3.min(c2, b3);
    }, f2), b2 = ga.prototype, a(b2, "isInt", function(a2 = 0.5) {
      var b3 = a2._zod, c2 = b3.bag;
      b3 = "string" == typeof c2.format ? c2.format + "" : Af, b3 = b3.includes("int"), b3 || (a2 = c2.multipleOf, true === Number.isSafeInteger(a2) && (b3 = true));
      return b3;
    }, f2), b2 = ga.prototype, a(b2, "isFinite", function(a2) {
      return true;
    }, f2);
  }, df = function() {
    let b2 = na.prototype, f2 = true;
    a(b2, "min", function(a2) {
      return function(b3) {
        return fa(a2, da("greater_than", { value: b3, inclusive: true, origin: "bigint" }));
      };
    }, f2), b2 = na.prototype, a(b2, "max", function(a2) {
      return function(b3) {
        return fa(a2, da("less_than", { value: b3, inclusive: true, origin: "bigint" }));
      };
    }, f2), b2 = na.prototype, a(b2, "gt", function(a2) {
      return function(b3) {
        return fa(a2, da("greater_than", { value: b3, inclusive: false, origin: "bigint" }));
      };
    }, f2), b2 = na.prototype, a(b2, "gte", function(a2) {
      return a2.min;
    }, f2), b2 = na.prototype, a(b2, "lt", function(a2) {
      return function(b3) {
        return fa(a2, da("less_than", { value: b3, inclusive: false, origin: "bigint" }));
      };
    }, f2), b2 = na.prototype, a(b2, "lte", function(a2) {
      return a2.max;
    }, f2), b2 = na.prototype, a(b2, "positive", function(a2) {
      return function() {
        return fa(a2, da("greater_than", { value: BigInt(0), inclusive: false, origin: "bigint" }));
      };
    }, f2), b2 = na.prototype, a(b2, "negative", function(a2) {
      return function() {
        return fa(a2, da("less_than", { value: BigInt(0), inclusive: false, origin: "bigint" }));
      };
    }, f2), b2 = na.prototype, a(b2, "nonnegative", function(a2) {
      return function() {
        return fa(a2, da("greater_than", { value: BigInt(0), inclusive: true, origin: "bigint" }));
      };
    }, f2), b2 = na.prototype, a(b2, "nonpositive", function(a2) {
      return function() {
        return fa(a2, da("less_than", { value: BigInt(0), inclusive: true, origin: "bigint" }));
      };
    }, f2), b2 = na.prototype, a(b2, "multipleOf", function(a2) {
      return function(b3, f3) {
        return c(a2, "multiple_of", { value: b3 }, f3);
      };
    }, f2), b2 = na.prototype, a(b2, "format", function(a2) {
      var c2 = a2._zod, b3 = c2.bag;
      return b3.format === void 0 ? null : b3.format;
    }, f2), b2 = na.prototype, a(b2, "minValue", function(a2) {
      var c2 = a2._zod, b3 = c2.bag;
      return b3.minimum === void 0 ? null : b3.minimum;
    }, f2), b2 = na.prototype, a(b2, "maxValue", function(a2) {
      var c2 = a2._zod, b3 = c2.bag;
      return b3.maximum === void 0 ? null : b3.maximum;
    }, f2);
  }, ef = function() {
    let b2 = Da.prototype;
    a(b2, "min", function(a2) {
      return function(b3) {
        return fa(a2, da("greater_than", { value: b3, inclusive: true, origin: "date" }));
      };
    }, true), b2 = Da.prototype, a(b2, "max", function(a2) {
      return function(b3) {
        return fa(a2, da("less_than", { value: b3, inclusive: true, origin: "date" }));
      };
    }, true), b2 = Da.prototype, a(b2, "minValue", function(a2) {
      var h3 = a2._zod, f2 = h3.bag, r3 = globalThis.Math, b3 = Number.NEGATIVE_INFINITY, c2 = f2.minimum;
      c2 === void 0 && (c2 = b3), a2 = f2.exclusiveMinimum, a2 === void 0 || (b3 = a2);
      return r3.max(c2, b3);
    }, true), b2 = Da.prototype, a(b2, "maxValue", function(a2) {
      var h3 = a2._zod, f2 = h3.bag, r3 = globalThis.Math, b3 = Number.POSITIVE_INFINITY, c2 = f2.maximum;
      c2 === void 0 && (c2 = b3), a2 = f2.exclusiveMaximum, a2 === void 0 || (b3 = a2);
      return r3.min(c2, b3);
    }, true), b2 = Da.prototype, a(b2, "minDate", function(a2) {
      var b3 = a2.minValue;
      return b3 == null || !Oa(b3) ? null : new Date(b3);
    }, true), b2 = Da.prototype, a(b2, "maxDate", function(a2) {
      var b3 = a2.maxValue;
      return b3 == null || !Oa(b3) ? null : new Date(b3);
    }, true);
  }, Fc = function(b2) {
    a(b2, "min", function(a2) {
      return function(b3, f2) {
        return c(a2, "min_size", { minimum: b3 }, f2);
      };
    }, true), a(b2, "max", function(a2) {
      return function(b3, f2) {
        return c(a2, "max_size", { maximum: b3 }, f2);
      };
    }, true), a(b2, "size", function(a2) {
      return function(b3) {
        return fa(a2, da("size_equals", { size: b3 }));
      };
    }, true), a(b2, "nonempty", function(a2) {
      return function() {
        return fa(a2, da("min_size", { minimum: 1 }));
      };
    }, true);
  }, ff = function() {
    let b2 = Va.prototype;
    a(b2, "implement", function(a2) {
      return function(b3) {
        if ("function" != typeof b3) throw la("implement() must be called with a function");
        var c2 = ca(a2), f2 = c2.def.input;
        return Bc(b3, f2, c2.def.output, false);
      };
    }, true), b2 = Va.prototype, a(b2, "implementAsync", function(a2) {
      return function(b3) {
        if ("function" != typeof b3) throw la("implementAsync() must be called with a function");
        var c2 = ca(a2), f2 = c2.def.input;
        return Bc(b3, f2, c2.def.output, true);
      };
    }, true), b2 = Va.prototype, a(b2, "input", function(a2) {
      return function(b3, c2) {
        var f2 = b3;
        Array.isArray(f2) && (f2 = Ec(f2, c2, void 0));
        return wa(a2, { input: f2 });
      };
    }, true), b2 = Va.prototype, a(b2, "output", function(a2) {
      return function(b3) {
        return wa(a2, { output: b3 });
      };
    }, true);
  }, gf = function() {
    let b2 = za.prototype;
    a(b2, "min", function(a2) {
      return function(b3, f2) {
        return c(a2, "min_length", { minimum: b3 }, f2);
      };
    }, true), b2 = za.prototype, a(b2, "max", function(a2) {
      return function(b3, f2) {
        return c(a2, "max_length", { maximum: b3 }, f2);
      };
    }, true), b2 = za.prototype, a(b2, "length", function(a2) {
      return function(b3, f2) {
        return c(a2, "length_equals", { length: b3 }, f2);
      };
    }, true), b2 = za.prototype, a(b2, "nonempty", function(a2) {
      return function(b3) {
        return c(a2, "min_length", { minimum: 1 }, b3);
      };
    }, true);
  }, hf = function() {
    let b2 = ka.prototype, c2 = true;
    Ca(b2, { configurable: c2, enumerable: false, get: function() {
      let a2 = this._zod, b3 = a2.def;
      return b3.shape;
    } }), b2 = ka.prototype, a(b2, "strict", function(a2) {
      return function() {
        return wa(a2, { catchall: new lb({ type: "never" }) });
      };
    }, c2), b2 = ka.prototype, a(b2, "passthrough", function(a2) {
      return function() {
        return wa(a2, { catchall: new La({ type: "unknown" }) });
      };
    }, c2), b2 = ka.prototype, a(b2, "strip", function(a2) {
      return function() {
        let c3 = {}, b3 = ha(c3, ca(a2).def);
        b3.catchall = void 0;
        return new ka(b3);
      };
    }, c2), b2 = ka.prototype, a(b2, "loose", function(a2) {
      return a2.passthrough;
    }, c2), b2 = ka.prototype, a(b2, "catchall", function(a2) {
      return function(b3) {
        return wa(a2, { catchall: b3 });
      };
    }, c2), b2 = ka.prototype, a(b2, "extend", function(a2) {
      return function(b3) {
        if (e2(a2)) for (var f2, r3 = d(a2), h3 = oa(b3), n2 = h3.length, c3 = 0; c3 < n2; c3++) {
          f2 = h3[c3];
          if (ea.getOwnPropertyDescriptor(r3, f2) !== void 0) throw la("Cannot overwrite keys on object schemas containing refinements. Use `.safeExtend()` instead.");
        }
        return wa(a2, g2(a2, b3));
      };
    }, c2), b2 = ka.prototype, a(b2, "safeExtend", function(a2) {
      return function(b3) {
        return wa(a2, g2(a2, b3));
      };
    }, c2), b2 = ka.prototype, a(b2, "merge", function(a2) {
      return function(b3) {
        if (e2(a2)) throw la(".merge() cannot be used on object schemas containing refinements. Use .safeExtend() instead.");
        var c3 = g2(a2, ca(b3).def.shape);
        ba(c3, "catchall", { configurable: true, enumerable: true, get: function() {
          return ca(b3).def.catchall;
        } });
        var f2 = ca(b3).def.checks;
        c3.checks = Array.isArray(f2) ? f2 : [];
        return wa(a2, c3);
      };
    }, c2), b2 = ka.prototype, a(b2, "pick", function(a2) {
      return function(b3) {
        if (e2(a2)) throw la(".pick() cannot be used on object schemas containing refinements");
        var r3 = d(a2);
        n(r3, b3);
        for (var c3, f2 = {}, g3 = Reflect.ownKeys(b3), i2 = g3.length, h3 = 0; h3 < i2; h3++) c3 = g3[h3], "string" == typeof c3 && b3[c3] && ba(f2, c3, { value: r3[c3], writable: true, enumerable: true, configurable: true });
        c3 = { checks: [] }, Ca(c3, { configurable: true, enumerable: true, get: function() {
          Ca(this, { value: f2, writable: true, enumerable: true, configurable: true });
          return f2;
        } });
        return wa(a2, c3);
      };
    }, c2), b2 = ka.prototype, a(b2, "omit", function(a2) {
      return function(b3) {
        if (e2(a2)) throw la(".omit() cannot be used on object schemas containing refinements");
        var c3 = d(a2);
        n(c3, b3);
        var f2 = {};
        tb(f2, c3);
        var h3 = Reflect.ownKeys(b3), g3 = h3.length;
        for (c3 = 0; c3 < g3; c3++) {
          var r3;
          "string" == typeof h3[c3] && b3[h3[c3]] && (r3 = h3[c3], Reflect.deleteProperty(f2, r3));
        }
        c3 = { checks: [] };
        Ca(c3, { configurable: true, enumerable: true, get: function() {
          Ca(this, { value: f2, writable: true, enumerable: true, configurable: true });
          return f2;
        } });
        return wa(a2, c3);
      };
    }, c2), b2 = ka.prototype, a(b2, "partial", function(a2) {
      return function(b3) {
        if (e2(a2)) throw la(uf);
        return i(a2, b3, true, false);
      };
    }, c2), b2 = ka.prototype, a(b2, "exactPartial", function(a2) {
      return function(b3) {
        if (e2(a2)) throw la(tf);
        return i(a2, b3, true, true);
      };
    }, c2), b2 = ka.prototype, a(b2, "required", function(a2) {
      return function(b3) {
        return i(a2, b3, false, false);
      };
    }, c2), b2 = ka.prototype, a(b2, "keyof", function(a2) {
      return function() {
        return Kd(oa(d(a2)), void 0);
      };
    }, c2);
  }, jf = function() {
    let b2 = Ma.prototype;
    a(b2, "extract", function(a2) {
      return function(b3, c2) {
        for (var h3, r3 = ca(a2), e3 = r3.def.entries, d2 = {}, g3 = b3.length, f2 = 0; f2 < g3; f2++) {
          h3 = b3[f2];
          if (!Ga.call(e3, h3)) throw la("Key " + b3[f2] + Cf);
          h3 = b3[f2], ba(d2, h3, { value: e3[b3[f2]], writable: true, enumerable: true, configurable: true });
        }
        f2 = ea.defineProperties({}, ea.getOwnPropertyDescriptors(r3.def));
        ha(f2, Sa(c2)), f2.entries = d2, f2.checks = [];
        return new Ma(f2);
      };
    }, true), b2 = Ma.prototype, a(b2, "exclude", function(a2) {
      return function(b3, c2) {
        var h3 = ca(a2), f2 = h3.def.entries, e3 = ea.defineProperties({}, ea.getOwnPropertyDescriptors(f2)), d2 = b3.length;
        for (f2 = 0; f2 < d2; f2++) {
          var r3 = h3.def.entries, g3 = b3[f2];
          if (!Ga.call(r3, g3)) throw la("Key " + b3[f2] + Cf);
          r3 = b3[f2], Reflect.deleteProperty(e3, r3);
        }
        f2 = ea.defineProperties({}, ea.getOwnPropertyDescriptors(h3.def));
        ha(f2, Sa(c2)), f2.entries = e3, f2.checks = [];
        return new Ma(f2);
      };
    }, true);
  }, kf = function() {
    Xa(), Le(), af(), bf(), cf(), df(), ef(), Fc(mb.prototype), Fc(nb.prototype), Fc(rb.prototype), a(rb.prototype, "mime", function(a2) {
      return function(b2, f2) {
        var h3 = b2;
        if (!Array.isArray(h3)) {
          var r3 = [h3];
          h3 = r3;
        }
        h3 = { mime: h3 };
        return c(a2, "mime_type", h3, f2);
      };
    }, true), gf(), hf(), jf(), s2(), ff(), f(za, "ZodArray"), f(Fb, "ZodRecord"), f(mb, "ZodMap"), f(nb, "ZodSet"), f(Na, "ZodUnion"), f(mc, "ZodDiscriminatedUnion"), f(Gb, "ZodXor"), f(Ha, "ZodPipe"), f(qb, "ZodCodec"), f(Mb, "ZodPreprocess"), f(Ma, "ZodEnum"), f(Eb, "ZodLiteral");
  };
})();
var hd;
var Sb;
var tc;
(function() {
  let a = (a2, b2) => "function" == typeof a2 ? a2(b2) : b2.message, b = (a2, b2, c) => {
    !Ga.call(a2, b2) && ba(a2, b2, { value: c(), writable: true, enumerable: true, configurable: true });
    return a2[b2];
  };
  hd = function(c, f) {
    for (var h2, r2, d = {}, g2 = [], n2 = c.issues, i = n2.length, e2 = 0; e2 < i; e2++) h2 = n2[e2], r2 = a(f, h2), c = h2.path, Array.isArray(c) && c.length > 0 ? (h2 = c[0], b(d, h2, function() {
      return [];
    }).push(r2)) : g2.push(r2);
    return { formErrors: g2, fieldErrors: d };
  }, Sb = function(c, f, h2, r2) {
    var d, g2, i, n2, e2, s2, l2, m2, A2 = c.length, y = 0;
    while (y < A2) {
      d = c[y], e2 = d.code + "";
      if ("invalid_union" == e2 && Array.isArray(d.errors) && d.errors.length > 0) for (g2 = d.errors, n2 = g2.length, e2 = 0; e2 < n2; e2++) s2 = f.concat(d.path), Sb(g2[e2], s2, h2, r2);
      else if ("invalid_key" == e2 || "invalid_element" == e2) e2 = f.concat(d.path), Sb(d.issues, e2, h2, r2);
      else {
        g2 = f.concat(d.path);
        if (!Array.isArray(g2) || 0 == g2.length) h2._errors.push(a(r2, d));
        else for (s2 = g2.length, i = h2, e2 = 0; e2 < s2; e2++) l2 = g2[e2], m2 = e2 == (s2 - 1 | 0), "_errors" === l2 ? (m2 && i._errors.push(a(r2, d)), n2 = i) : (n2 = b(i, l2, function() {
          return { _errors: [] };
        }), m2 && n2._errors.push(a(r2, d))), i = n2;
      }
      y++;
    }
  }, tc = function(c, f, h2, r2) {
    var d, g2, n2, i, e2, y, A2, s2, l2, W2 = c.length, m2 = 0;
    while (m2 < W2) {
      d = c[m2], e2 = d.code + "";
      if ("invalid_union" == e2 && Array.isArray(d.errors) && d.errors.length > 0) for (n2 = d.errors, g2 = n2.length, e2 = 0; e2 < g2; e2++) i = n2[e2], tc(i, f.concat(d.path), h2, r2);
      else if ("invalid_key" == e2 || "invalid_element" == e2) e2 = d.issues, tc(e2, f.concat(d.path), h2, r2);
      else {
        l2 = f.concat(d.path);
        if (!Array.isArray(l2) || 0 == l2.length) h2.errors.push(a(r2, d));
        else for (i = l2.length, e2 = h2, n2 = 0; n2 < i; n2++) g2 = l2[n2], y = n2 == (i - 1 | 0), "string" == typeof g2 ? ((e2.properties === void 0 || e2.properties == null) && (e2.properties = {}), A2 = e2.properties, e2 = g2 = b(A2, g2, function() {
          return { errors: [] };
        })) : ((e2.items === void 0 || e2.items == null) && (e2.items = []), s2 = e2.items, (s2[g2] === void 0 || s2[g2] == null) && (s2[g2] = { errors: [] }), e2 = g2 = s2[g2]), y && e2.errors.push(a(r2, d));
      }
      m2++;
    }
  };
})();
var id = (a) => {
  var c = [];
  if (!Array.isArray(a)) return Af;
  for (var b, h2, r2 = a.length, f = 0; f < r2; f++) b = a[f], X(b) && b != null && b.key !== void 0 && (b = h2 = b.key), "number" == typeof b ? c.push("[" + b + "]") : "symbol" == typeof b ? c.push("[" + JSON.stringify(String(b), void 0, void 0) + "]") : (b = b + "", /[^\w$]/.test(b) ? c.push("[" + JSON.stringify(b, void 0, void 0) + "]") : (c.length > 0 && c.push("."), c.push(b)));
  return c.join(Af);
};
var Tb = () => {
  var a = globalThis.__zod_globalConfig;
  a == null && (a = {}, globalThis.__zod_globalConfig = a);
  return a;
};
var jd = () => function(a) {
  return gd(a);
};
var Xa = () => {
  var a = Tb();
  a.localeError === void 0 ? (ab = jd(), a.localeError = ab) : ab = a.localeError, "function" == typeof a.customError && (Cb = a.customError);
};
var kd = (a, b) => ({ _map: a, _idmap: b, add: function(c, f) {
  a.set(c, f);
  if (X(f) && f != null && f.id !== void 0) {
    var h2 = f.id;
    b.set(h2, c);
  }
  return c;
}, clear: function() {
  return this._map = /* @__PURE__ */ new WeakMap(), this._idmap = /* @__PURE__ */ new Map(), a = this._map, b = this._idmap, this;
}, remove: function(c) {
  var h2, f = a.get(c);
  X(f) && f != null && f.id !== void 0 && (h2 = b, h2.delete(f.id)), a.delete(c);
}, get: function(b2) {
  var c, h2 = a.get(b2), r2 = void 0;
  X(b2) && X(b2._zod) && (c = b2._zod, r2 = c.parent);
  if (r2 !== void 0 && r2 != null) {
    c = this.get(r2);
    var f = {};
    X(c) && c != null && ha(f, c), Reflect.deleteProperty(f, "id"), X(h2) && h2 != null && ha(f, h2);
    return 0 == oa(f).length ? void 0 : f;
  }
  return h2;
}, has: function(b2) {
  return !!a.has(b2);
} });
var Ya = () => {
  var a;
  if (globalThis.__zod_globalRegistry !== void 0 && globalThis.__zod_globalRegistry != null) return bb = globalThis.__zod_globalRegistry, bb;
  bb === void 0 && (a = /* @__PURE__ */ new WeakMap(), bb = kd(a, /* @__PURE__ */ new Map()), globalThis.__zod_globalRegistry = bb);
  return bb;
};
var Pa = (a) => a == null ? false : "backward" == a.direction + "";
var uc = (a) => {
  var b = a.def.defaultValue;
  "function" == typeof b && (b = b());
  return ((a2) => {
    if (Array.isArray(a2)) return a2.slice(0);
    if (pa(Map, a2)) return new Map(a2);
    if (pa(Set, a2)) return new Set(a2);
    if (X(a2) && !Array.isArray(a2) && !pa(Date, a2)) {
      var b2 = ea.getPrototypeOf(a2);
      if (b2 == null || b2 === ea.prototype) return ha({}, a2);
    }
    return a2;
  })(b);
};
var Ub = (a, b) => {
  var c = { check: "custom" };
  X(b) && b != null && ha(c, b), b = da("custom", c), c = b._zod, c.check = a;
  return b;
};
var wa = (a, b) => {
  var f = ca(a), c = f.def;
  b !== void 0 && X(b) && (c = {}, tb(c, f.def), tb(c, b)), c = new f.ctor(c), b === void 0 && (b = c._zod, b.parent = a);
  return c;
};
var fa = (a, b) => {
  var c = ca(a).def.checks;
  c = c === void 0 || !Array.isArray(c) ? [] : c.slice(0), c.push(b), b = wa(a, { checks: c }), c = b._zod, c.parent = a;
  return b;
};
var ya = (a, b) => sa(a.def[b]);
var vc = (a) => X(a) && a._zod !== void 0 && X(a._zod) && a._zod.def !== void 0 ? a._zod.def : X(a) && a.def !== void 0 ? a.def : a;
var wc = (a) => {
  var b, f, c = 0;
  while (29 == a.kind && c < 64) {
    b = void 0, X(a.handle) && X(a.handle._zod) && (f = a.handle._zod, b = f.innerType);
    if (b == null) {
      b = a.def.getter;
      if ("function" != typeof b) break;
      b = b();
    }
    b = sa(b);
    b ? (a = b, c++) : c = 64;
  }
  return a;
};
var ta = (a, b, c) => {
  var f = {};
  "url" != b && "jwt" != b && "ipv6" != b && "cidrv6" != b && "template_literal" != b && (f.origin = "string");
  f.code = "invalid_format", f.format = b, c !== void 0 && X(c) && ha(f, c), f.continue === void 0 && (f.continue = true), b = a.issues, b.push(f);
};
var Vb = (a, b, c) => {
  for (var f = a.issues, h2 = f.length; b < h2; b++) a = f[b], a.inst === void 0 && (a = f[b], a.inst = c);
};
var ld = (a, b, c, f, h2, r2) => {
  if (b.issues.length == r2 && !a) a = [], Array.isArray(h2.path) && (a = h2.path.slice(0)), a = { code: "custom", input: c, inst: f, path: a, continue: !h2.abort }, h2.params === void 0 || (a.params = h2.params), c = b.issues, c.push(a);
};
var md = (a) => {
  if (0 == a.length) return true;
  if (/\s/.test(a)) return false;
  for (var b = 0, c = 0; c < a.length; c++) b++, 4 == b && (b = 0);
  if (b) return false;
  try {
    globalThis.atob(a);
    return true;
  } catch {
    return false;
  }
};
var nd = (a) => {
  if (!/^[0-9a-fA-F:.]+$/.test(a)) return false;
  try {
    new URL("http://[" + a + "]");
    return true;
  } catch {
    return false;
  }
};
var Wb = (a) => {
  var b = a.$pending;
  if ($(b)) return Reflect.deleteProperty(a, "$pending"), b;
};
var Xb = (a) => {
  if (a.aborted) return true;
  var b = a.issues, c = b.length;
  a = 0;
  while (a < c) {
    var f = b[a], h2 = f.continue;
    if (true !== h2) return true;
    a++;
  }
  return false;
};
var pd = (a, b, c) => {
  for (var f = a.issues, h2 = f.length; b < h2; b++) a = f[b], a.schema === void 0 && (a = f[b], a.schema = c);
};
var xc;
var Yb;
(function() {
  let a = (a2) => {
    if (a2.aborted) return true;
    var b2 = a2.issues, c = b2.length;
    a2 = 0;
    while (a2 < c) {
      var f = b2[a2], h2 = f.continue;
      if (false === h2) return true;
      a2++;
    }
    return false;
  }, b = (a2, b2, c) => {
    var h2 = !c.abort, f = a2.issues, r2 = f.length;
    while (b2 < r2) a2 = f[b2], a2.continue = h2, b2++;
  };
  xc = function(b2, c, f, h2) {
    var e2, n2, r2, i, d, g2 = b2.def.checks;
    if (g2 === void 0 || !Array.isArray(g2)) return c;
    n2 = g2.length;
    while (h2 < n2) {
      e2 = g2[h2], r2 = vc(e2);
      if ("function" == typeof r2.when) {
        if (a(c)) {
          h2++;
          continue;
        }
        if (!r2.when(c)) {
          h2++;
          continue;
        }
      } else if (Xb(c)) {
        h2++;
        continue;
      }
      i = c.issues;
      r2 = i.length, od(e2, c, f), h2++, d = Wb(c);
      if ($(d)) return d.then(/* @__PURE__ */ ((a2, b3, c2, f2, h3, r3) => function(e3) {
        let d2 = a2.handle;
        Vb(b3, h3, f2), pd(b3, h3, d2);
        return xc(a2, b3, c2, r3);
      })(b2, c, f, e2, r2, h2));
      d = b2.handle, Vb(c, r2, e2), pd(c, r2, d);
    }
    return c;
  }, Yb = function(a2, c, f) {
    if (c.memo) return c;
    Vb(c, 0, a2.handle);
    if ("string" == typeof a2.def.check) {
      m = c.issues.length, od(a2.handle, c, f), Vb(c, m, a2.handle), b(c, m, a2.def);
      var h2 = Wb(c);
      if ($(h2)) return h2.then(function(b2) {
        return xc(a2, c, f, 0);
      });
      if (a2.def.abort && (h2 = c.issues, h2.length > m)) return c;
    }
    return xc(a2, c, f, 0);
  };
})();
var yc = (a) => {
  var b = a.$waits;
  if (b === void 0 || !Array.isArray(b)) return a;
  Reflect.deleteProperty(a, "$waits");
  return 0 == b.length ? a : Promise.all(b).then(function(b2) {
    return a;
  });
};
var xb = (a, b, c) => {
  if ($(b)) {
    var f = a.$waits;
    (f === void 0 || !Array.isArray(f)) && (f = [], a.$waits = f), f.push(b.then(c));
  } else c(b);
};
var Zb = (a, b, c) => {
  var f = Wb(b);
  if ($(f)) return f.then(function(f2) {
    return Zb(a, b, c);
  });
  f = yc(b);
  if ($(f)) return f.then(function(b2) {
    vb(a) && bd(a, b2);
    return !a.hasChecks ? b2 : Yb(a, b2, c);
  });
  vb(a) && bd(a, b);
  return !a.hasChecks ? b : Yb(a, b, c);
};
var yb;
var qa;
(function() {
  let f = (a2) => "number" != typeof a2 ? false : true === Number.isNaN(a2), a = (a2, b2) => {
    $(b2) && (a2.$pending = b2);
  }, b = (b2, c2, d) => {
    var n2 = wc(b2), g2 = c2.value;
    b2 = n2.kind;
    var i;
    if (!(9 == b2 || 10 == b2)) {
      if (0 == b2) {
        if (n2.def.coerce) try {
          c2.value = String(g2), g2 = c2.value;
        } catch {
        }
        if ("string" == typeof g2) return;
        ra(c2, "string", g2);
        return;
      }
      if (1 == b2) {
        if (n2.def.coerce) try {
          c2.value = Number(g2), g2 = c2.value;
        } catch {
        }
        if ("number" != typeof g2 ? false : true === Number.isNaN(g2) ? false : !Oa(g2) ? false : true) return;
        ra(c2, "number", g2);
        return;
      }
      if (2 == b2) {
        n2.def.coerce && (c2.value = Boolean(g2)), g2 = c2.value;
        if ("boolean" == typeof g2) return;
        ra(c2, "boolean", g2);
        return;
      }
      if (3 == b2) {
        if (n2.def.coerce) {
          try {
            c2.value = BigInt(g2);
          } catch {
            ra(c2, "bigint", g2);
            return;
          }
          g2 = c2.value;
        }
        if ("bigint" == typeof g2) return;
        ra(c2, "bigint", g2);
        return;
      }
      if (4 == b2) {
        if ("symbol" == typeof g2) return;
        ra(c2, "symbol", g2);
        return;
      }
      if (5 == b2) {
        if (n2.def.coerce) try {
          c2.value = new Date(g2), g2 = c2.value;
        } catch {
        }
        if (!pa(Date, g2) ? false : true === Number.isNaN(g2.getTime()) ? false : true) return;
        ra(c2, "date", g2);
        return;
      }
      if (6 == b2) {
        if (f(g2)) return;
        ra(c2, "nan", g2);
        return;
      }
      if (7 == b2) {
        if (g2 === void 0) return;
        ra(c2, "undefined", g2);
        return;
      }
      if (12 == b2) {
        if (g2 === void 0) return;
        ra(c2, "void", g2);
        return;
      }
      if (8 == b2) {
        if (g2 === void 0 ? false : g2 == null) return;
        ra(c2, "null", g2);
        return;
      }
      if (11 == b2) {
        ra(c2, "never", g2);
        return;
      }
      if (13 == b2) {
        if (n2.values !== void 0 && n2.values.has(g2)) return;
        d = Array.from(n2.values), b2 = g2, c2.issues.push({ code: "invalid_value", values: d, input: b2 });
        return;
      }
      if (14 == b2) {
        if (n2.values !== void 0 && n2.values.has(g2)) return;
        d = Array.from(n2.values), b2 = g2, c2.issues.push({ code: "invalid_value", values: d, input: b2 });
        return;
      }
      if (23 == b2) {
        b2 = xa(n2);
        if (n2.def.exact) {
          !b2 || a(c2, qa(b2, c2, d));
          return;
        }
        if (g2 === void 0) {
          !b2 || 2 == b2.optin && (b2 = qa(b2, c2, d), $(b2) ? c2.$pending = b2.then(function(a2) {
            rd(a2);
            return a2;
          }) : rd(c2));
          return;
        }
        !b2 || a(c2, qa(b2, c2, d));
        return;
      }
      if (24 == b2) {
        if (g2 === null) {
          c2.value = g2;
          return;
        }
        b2 = xa(n2);
        !b2 || a(c2, qa(b2, c2, d));
        return;
      }
      if (25 == b2 || 26 == b2) {
        i = xa(n2);
        if (Pa(d)) {
          !i || a(c2, qa(i, c2, d));
          return;
        }
        if (g2 === void 0) {
          c2.value = uc(n2), 26 == b2 && i && a(c2, qa(i, c2, d));
          return;
        }
        if (i) {
          d = qa(i, c2, d);
          if ($(d)) {
            var s2 = 0;
            25 == b2 && (s2 = 1), c2.$pending = d.then(function(a2) {
              1 == s2 && a2.value === void 0 && (a2.value = uc(n2));
              return a2;
            });
            return;
          }
        }
        25 == b2 && c2.value === void 0 && (c2.value = uc(n2));
        return;
      }
      if (27 == b2) {
        b2 = xa(n2);
        if (Pa(d)) {
          !b2 || a(c2, qa(b2, c2, d));
          return;
        }
        if (b2) {
          b2 = qa(b2, c2, d);
          if ($(b2)) {
            c2.$pending = b2.then(function(a2) {
              sd(a2, n2, d);
              return a2;
            });
            return;
          }
          sd(c2, n2, d);
        }
        return;
      }
      if (28 == b2) {
        if (b2 = xa(n2)) {
          b2 = qa(b2, c2, d);
          if ($(b2)) {
            c2.$pending = b2.then(function(a2) {
              td(a2);
              return a2;
            });
            return;
          }
        }
        td(c2);
        return;
      }
      if (33 == b2) {
        if (!Pa(d)) {
          if (b2 = xa(n2)) {
            b2 = qa(b2, c2, d);
            if ($(b2)) c2.$pending = b2.then(function(a2) {
              if (!a2.memo) {
                var b3 = ea.freeze(a2.value);
                a2.value = b3;
              }
              return a2;
            });
            else if (!c2.memo) {
              var l2 = ea.freeze(c2.value);
              c2.value = l2;
            }
          }
          return;
        }
        b2 = xa(n2);
        !b2 || a(c2, qa(b2, c2, d));
        return;
      }
      if (16 == b2) Oe(n2, c2, d);
      else if (17 == b2) h2(n2, c2, d);
      else if (15 == b2) Pe(n2, c2, d);
      else if (18 == b2) Qe(n2, c2, d);
      else if (21 == b2 || 38 == b2) r2(n2, c2, d);
      else if (22 == b2) ((a2, b3, c3) => {
        var h3 = ya(a2, "left"), f2 = ya(a2, "right");
        if (!(!h3 || !f2)) {
          a2 = va(h3, b3.value, c3), f2 = va(f2, b3.value, c3);
          if ($(a2) || $(f2)) {
            c3 = [Promise.resolve(a2), Promise.resolve(f2)], b3.$pending = Promise.all(c3).then(function(a3) {
              let c4 = a3[0];
              zd(b3, c4, a3[1]);
              return b3;
            });
            return;
          }
          zd(b3, a2, f2);
        }
      })(n2, c2, d);
      else if (19 == b2) e2(n2, c2, d);
      else if (20 == b2) ((a2, b3, c3) => {
        var h3, r3, e3, d2, f2 = b3.value;
        if (!X(f2) || f2.add === void 0) {
          b3.issues.push(Ba("set", f2));
          return;
        }
        h3 = fb(a2, b3, ja());
        d2 = f2.values(), f2 = d2.next();
        while (!f2.done) r3 = f2.value, e3 = ya(a2, "valueType"), e3 ? xb(b3, va(e3, r3, c3), function(a3) {
          a3.issues.length > 0 && ud(a3.issues, b3.issues);
          var c4 = a3.value;
          h3.add(c4);
        }) : h3.add(r3), f2 = d2.next();
        b3.value = h3;
      })(n2, c2, d);
      else if (39 == b2) ((a2, b3) => {
        var c3 = b3.value;
        if ("function" != typeof c3) {
          ra(b3, "function", c3);
          return;
        }
        var r3 = a2.def.input, f2 = a2.def.output, h3 = sa(f2);
        a2 = h3 && 30 == h3.kind, b3.value = Bc(c3, r3, f2, a2);
      })(n2, c2);
      else if (40 == b2) {
        if ("string" != typeof g2) {
          ra(c2, "string", g2);
          return;
        }
        d = n2.handle._zod;
        b2 = d.pattern, b2 !== void 0 && b2 != null && (b2.lastIndex = 0, b2.test(g2) || ta(c2, "template_literal", { pattern: b2.source }));
      } else 34 == b2 || (37 == b2 ? (b2 = xa(n2), !b2 || (n2 = va(b2, g2, d).issues, c2.value = 0 == n2.length)) : 35 == b2 && (b2 = globalThis.File, b2 === void 0 ? ra(c2, "file", g2) : pa(b2, g2) || ra(c2, "file", g2)));
    }
  }, h2 = (a2, b2, c2) => {
    var h3 = b2.value;
    if (!Array.isArray(h3)) {
      b2.issues.push(Ba("tuple", h3));
      return;
    }
    var e3 = a2.def.items;
    Array.isArray(e3) || (e3 = []);
    var n2 = h3.length, r3 = e3.length, f2 = vd(e3, true), m2 = vd(e3, false), s2 = ya(a2, "rest");
    if (!s2) {
      if (n2 < f2) {
        b2.issues.push(cd(false, f2, h3));
        return;
      }
      n2 > r3 && b2.issues.push(cd(true, r3, h3));
    }
    var l2 = fb(a2, b2, []);
    b2.value = l2;
    var g2 = [], i = [];
    for (f2 = 0; f2 < r3; f2++) g2.push(void 0);
    for (f2 = 0; f2 < r3; f2++) {
      var y = { i: 0 + f2 }, d = va(sa(e3[f2]), h3[f2], c2);
      $(d) ? i.push(d.then(/* @__PURE__ */ ((a3, b3) => function(c3) {
        a3[b3.i] = c3;
      })(g2, y))) : g2[f2] = d;
    }
    if (n2 > r3 && s2) for (; f2 < n2; f2++) d = { i: 0 + f2 }, r3 = va(s2, h3[f2], c2), $(r3) ? i.push(r3.then(/* @__PURE__ */ ((a3, b3, c3, f3) => function(a4) {
      Ob(b3, c3, f3.i, a4);
    })(a2, b2, l2, d))) : Ob(b2, l2, d.i, r3);
    if (i.length > 0) {
      b2.$pending = Promise.all(i).then(function(a3) {
        dd(b2, e3, h3, g2, m2);
        return b2;
      });
      return;
    }
    dd(b2, e3, h3, g2, m2);
  }, r2 = (a2, b2, c2) => {
    var r3 = b2.value, n2 = a2.def.options;
    Array.isArray(n2) || (n2 = []);
    var h3 = "string" == typeof a2.def.discriminator ? a2.def.discriminator + "" : Af, s2 = n2.length;
    if (h3.length > 0) {
      if (!X(r3) || Array.isArray(r3)) {
        b2.issues.push(Ba("object", r3));
        return;
      }
      var e3 = yd(a2), f2 = r3[h3];
      f2 = e3.get(f2);
      var d;
      if (f2 !== void 0 && f2 != null) {
        c2 = Za(ca(f2), r3, c2);
        if ($(c2)) {
          b2.$pending = c2.then(function(a3) {
            gb(b2, a3);
            return b2;
          });
          return;
        }
        gb(b2, c2);
        return;
      }
      if (!(!!a2.def.unionFallback || Pa(c2))) {
        a2 = { code: "invalid_union", errors: [], note: "No matching discriminator", discriminator: h3, options: Array.from(e3.keys()), path: [] }, wb(a2, h3), c2 = b2.issues, c2.push(a2);
        return;
      }
    }
    h3 = [];
    d = [];
    var g2 = [];
    for (f2 = 0; f2 < s2; f2++) h3.push(void 0);
    f2 = 0;
    while (f2 < s2) {
      e3 = va(sa(n2[f2]), r3, c2);
      if ($(e3)) {
        var i = { i: 0 + f2 };
        g2.push(e3.then(/* @__PURE__ */ ((a3, b3) => function(c3) {
          a3[b3.i] = c3;
        })(h3, i)));
      } else {
        h3[f2] = e3, i = e3.issues;
        if (0 == i.length) {
          if (38 != a2.kind) {
            gb(b2, e3);
            return;
          }
          d.push(f2);
        }
      }
      f2++;
    }
    if (g2.length > 0) {
      b2.$pending = Promise.all(g2).then(function(f3) {
        if (38 == a2.kind) {
          for (var e4 = [], r4 = 0; r4 < s2; r4++) f3 = h3[r4], f3 !== void 0 && f3 != null && 0 == f3.issues.length && e4.push(r4);
          if (1 == e4.length) return gb(b2, h3[e4[0]]), b2;
        }
        ed(a2, b2, h3, c2);
        return b2;
      });
      return;
    }
    if (38 == a2.kind) {
      if (1 == d.length) {
        b2.value = h3[d[0]].value;
        return;
      }
      if (0 == d.length) {
        for (d = [], r3 = 0; r3 < s2; r3++) {
          for (f2 = sa(n2[r3]), f2 && (a2 = f2), f2 = h3[r3], e3 = f2.issues, g2 = [], i = e3.length, f2 = 0; f2 < i; f2++) g2.push(Wa(e3[f2], a2, c2));
          d.push(g2);
        }
        b2.issues.push({ code: "invalid_union", errors: d, path: [] });
        return;
      }
      b2.issues.push({ code: "invalid_union", errors: [], inclusive: false, matches: d, path: [] });
      return;
    }
    ed(a2, b2, h3, c2);
  }, e2 = (a2, b2, c2) => {
    var h3 = b2.value;
    if (!pa(Map, h3)) {
      b2.issues.push(Ba("map", h3));
      return;
    }
    var d = fb(a2, b2, /* @__PURE__ */ new Map()), n2 = [], i = h3.entries(), f2 = i.next();
    while (!f2.done) {
      var s2 = f2.value, g2 = s2[0], l2 = s2[1], r3 = { value: g2, issues: [] }, e3 = { value: l2, issues: [] }, m2 = ya(a2, "keyType");
      m2 && (r3 = va(m2, g2, c2)), f2 = ya(a2, "valueType"), f2 && (e3 = va(f2, l2, c2)), $(r3) || $(e3) ? (f2 = [Promise.resolve(r3), Promise.resolve(e3)], n2.push(Promise.all(f2).then(/* @__PURE__ */ ((a3, b3, c3, f3, h4) => function(r4) {
        let e4 = r4[0];
        Cd(e4, r4[1], a3, h4, c3, b3);
        let d2 = r4[0], g3 = d2.value, n3 = r4[1].value;
        f3.set(g3, n3);
      })(b2, c2, h3, d, g2)))) : (Cd(r3, e3, b2, g2, h3, c2), d.set(r3.value, e3.value)), f2 = i.next();
    }
    b2.value = d;
    n2.length > 0 && (b2.$pending = Promise.all(n2).then(function(a3) {
      b2.value = d;
      return b2;
    }));
  }, c = (a2, b2, c2) => {
    var f2 = ya(a2, "in"), r3 = ya(a2, "out"), e3 = a2.def.transform, h3 = a2.def.reverseTransform;
    if (31 == a2.kind) {
      if (Pa(c2)) throw new Ic("ZodTransform");
      if (c2 !== void 0 && X(c2)) {
        f2 = c2["~memo"];
        if (X(f2) && f2.backEdges !== void 0 && f2.backEdges != null && f2.backEdges.has(b2.value)) {
          a2 = new Error("Cannot parse a reference cycle that closes through a transform"), a2.name = "ZodCyclicError";
          throw a2;
        }
      }
      return "function" == typeof e3 ? _b(a2, b2, c2, e3) : b2;
    }
    if (Pa(c2)) {
      if (r3) {
        r3 = qa(r3, b2, c2);
        if ($(r3)) return r3.then(function(b3) {
          if (Ac(b3)) return b3.aborted = true, b3;
          var r4 = "function" == typeof h3 ? _b(a2, b3, c2, h3) : b3;
          return $(r4) ? r4.then(function(a3) {
            return ib(a3, c2, f2);
          }) : ib(b3, c2, f2);
        });
        if (Ac(b2)) return b2.aborted = true, b2;
      }
      return "function" == typeof h3 && (h3 = _b(a2, b2, c2, h3), $(h3)) ? h3.then(function(a3) {
        return ib(a3, c2, f2);
      }) : ib(b2, c2, f2);
    }
    return f2 && (f2 = qa(f2, b2, c2), $(f2)) ? f2.then(function(b3) {
      return Dd(a2, b3, c2);
    }) : Dd(a2, b2, c2);
  };
  yb = function(a2, f2, h3) {
    if (32 == a2.kind || 31 == a2.kind) return c(a2, f2, h3);
    b(a2, f2, h3), a2 = Wb(f2);
    return $(a2) ? a2.then(function(a3) {
      return yc(f2);
    }) : yc(f2);
  }, qa = function(a2, f2, h3) {
    a2 = wc(a2);
    if (vb(a2)) {
      var r3 = ((a3, b2, c2) => {
        var f3 = a3.handle._zod, h4 = f3.memoizer;
        if (h4 != null) {
          if (h4.recursive === void 0) {
            h4.recursive = $c(a3, ja());
            if (!h4.recursive) return;
          } else {
            if (!h4.recursive) return;
          }
          var e4 = b2.value;
          if (!(e4 == null || "object" != typeof e4) && !(c2 == null || !X(c2))) {
            f3 = c2["~memo"];
            var r4;
            f3 == null && (f3 = /* @__PURE__ */ new Map(), f3 = { buckets: f3, backEdges: void 0 }, c2["~memo"] = f3), h4.ctx === c2 ? r4 = h4.bucket : (r4 = f3.buckets.get(a3.handle), r4 == null && (r4 = /* @__PURE__ */ new Map(), f3.buckets.set(a3.handle, r4)), h4.ctx = c2, h4.bucket = r4), a3 = r4.get(e4);
            if (a3 !== void 0 && a3 != null) return b2.value = a3.value, a3.issues != null ? (c2 = a3.issues, c2.length > 0 && ((a4, b3) => {
              var c3 = ad(a4), f4 = c3.length;
              for (a4 = 0; a4 < f4; a4++) b3.issues.push(c3[a4]);
            })(a3.issues, b2)) : (b2.memo = true, (f3.backEdges === void 0 || f3.backEdges == null) && (f3.backEdges = ja()), f3.backEdges.add(a3.value)), b2;
            h4.handoff = r4, a3 = h4.open, h4.openDepth = a3.length;
          }
        }
      })(a2, f2, h3);
      if (r3 !== void 0) return Yb(a2, r3, h3);
    }
    if (30 == a2.kind) {
      if (h3 === void 0 || false === h3.async) throw new Error(qf);
      r3 = f2.value;
      return Promise.resolve(r3).then(function(b2) {
        f2.value = b2;
        var c2 = xa(a2);
        return c2 ? qa(c2, f2, h3) : f2;
      });
    }
    if (h3 !== void 0 && X(h3) && h3.skipChecks) return yb(a2, f2, h3);
    if (Pa(h3) && a2.hasChecks) {
      r3 = ha({}, h3), r3.skipChecks = true;
      var e3 = f2.value;
      r3 = yb(a2, { value: e3, issues: [] }, r3);
      if ($(r3)) {
        if (h3 !== void 0 && false === h3.async) throw new Error(qf);
        return r3.then(function(b2) {
          return qd(a2, b2, f2, h3);
        });
      }
      return qd(a2, r3, f2, h3);
    }
    if (32 == a2.kind || 31 == a2.kind) return r3 = c(a2, f2, h3), $(r3) ? r3.then(function(b2) {
      return Zb(a2, b2, h3);
    }) : Zb(a2, f2, h3);
    b(a2, f2, h3);
    return Zb(a2, f2, h3);
  };
})();
var qd = (a, b, c, f) => {
  if (Xb(b)) return b.aborted = true, b;
  b = Yb(a, c, f);
  if ($(b)) {
    if (f !== void 0 && false === f.async) throw new Error(qf);
    return b.then(function(b2) {
      return yb(a, b2, f);
    });
  }
  return yb(a, b, f);
};
var Za = (a, b, c) => a.handle._zod.run({ value: b, issues: [] }, c);
var va = (a, b, c) => !!a ? Za(a, b, c) : { value: b, issues: [] };
var ra = (a, b, c) => {
  a.issues.push(Ba(b, c));
};
var rd = (a) => {
  var b = a.issues;
  if (0 != b.length) b = a.issues, b.length = 0, a.value = void 0, a.aborted = false;
};
var sd = (a, b, c) => {
  var f = a.issues;
  if (0 != f.length) {
    f = b.def.catchValue, f === void 0 && (f = b.def.defaultValue);
    if ("function" == typeof f) {
      for (var r2 = [], e2 = a.issues, d = e2.length, h2 = 0; h2 < d; h2++) r2.push(Wa(e2[h2], b, c));
      b = a.value, c = a.issues, f = f({ value: b, issues: c, error: { issues: r2 }, input: a.value });
    }
    a.value = f;
    a.issues.length = 0, a.aborted = false;
  }
};
var td = (a) => {
  0 == a.issues.length && a.value === void 0 && a.issues.push({ code: "invalid_type", expected: "nonoptional", input: a.value });
};
var ud = (a, b) => {
  for (var f, h2 = a.length, c = 0; c < h2; c++) f = a[c], b.push(f);
};
var _a = (a, b, c) => {
  for (var h2, r2 = a.length, f = 0; f < r2; f++) wb(a[f], c), h2 = a[f], b.push(h2);
};
var Oe;
var Pe;
(function() {
  let b = (a2) => {
    var b2 = a2.kind;
    return 32 == b2 ? true : 31 == b2 ? true : 30 == b2 ? true : 29 == b2 ? true : 25 == b2 ? true : 26 == b2 ? true : 27 == b2 ? true : false;
  }, a = (a2, c, f, h2) => {
    if (vb(a2)) return Za(a2, c, f);
    if (Pa(f) && a2.hasChecks) return Za(a2, c, f);
    if (b(a2)) return Za(a2, c, f);
    h2.value = c;
    var r2 = h2.issues;
    r2.length = 0, h2.aborted === void 0 || (h2.aborted = false), h2.memo === void 0 || (h2.memo = false), r2 = a2.handle._zod, h2 = r2.run(h2, f);
    return $(h2) ? Za(a2, c, f) : h2;
  };
  Oe = function(b2, c, f) {
    var e2 = c.value;
    if (!Array.isArray(e2)) {
      c.issues.push(Ba("array", e2));
      return;
    }
    for (var h2, g2, n2 = ya(b2, "element"), d = fb(b2, c, []), i = { value: void 0, issues: [] }, s2 = e2.length, r2 = 0; r2 < s2; r2++) n2 ? (h2 = a(n2, e2[r2], f, i), $(h2) ? (g2 = +(0 + r2), xb(c, h2, /* @__PURE__ */ ((a2, b3, c2, f2) => function(a3) {
      Ob(b3, c2, f2, a3);
    })(b2, c, d, g2))) : Ob(c, d, +(0 + r2), h2)) : (h2 = e2[r2], d.push(h2));
    c.value = d;
  }, Pe = function(b2, c, f) {
    var h2, l2, g2, d, r2, s2, e2, m2, y, n2, W2, k2, o2, A2, i = c.value;
    if (!X(i) || Array.isArray(i)) {
      c.issues.push(Ba("object", i));
      return;
    }
    l2 = b2.def.shape;
    l2 == null && (l2 = {}), g2 = b2.handle._zod, h2 = g2["~keys"], d = g2["~fids"];
    if (!Array.isArray(h2) || !Array.isArray(d)) {
      for (h2 = oa(l2), d = [], s2 = h2.length, r2 = 0; r2 < s2; r2++) e2 = sa(Reflect.get(l2, h2[r2] + "")), e2 ? d.push(+(0 + e2.id)) : d.push(-1);
      g2["~keys"] = h2, g2["~fids"] = d;
    }
    g2 = fb(b2, c, {});
    m2 = { value: void 0, issues: [] }, y = h2.length, e2 = 0;
    while (e2 < y) {
      s2 = h2[e2] + "", r2 = +d[e2] | 0;
      if ("__proto__" == s2 || r2 < 0) {
        e2++;
        continue;
      }
      r2 = Ja[r2];
      A2 = true === s2 in i, n2 = void 0, A2 && (n2 = i[s2]), n2 = a(r2, n2, f, m2), $(n2) ? (W2 = Wc(r2), k2 = Xc(r2), xb(c, n2, /* @__PURE__ */ ((a2, b3, c2, f2, h3, r3, e3) => function(a3) {
        Yc(b3, c2, f2, h3, r3, e3, a3);
      })(b2, c, g2, s2, A2, W2, k2))) : Yc(c, g2, s2, A2, Wc(r2), Xc(r2), n2), e2++;
    }
    h2 = b2.def.catchall;
    if (h2 !== void 0 && h2 != null) {
      for (d = sa(h2), r2 = [], m2 = oa(i), y = m2.length, n2 = 0; n2 < y; n2++) h2 = m2[n2] + "", Ga.call(l2, h2) || ("__proto__" == h2 ? !d || 11 == d.kind && r2.push(h2) : d ? (e2 = d, 11 == e2.kind ? r2.push(h2) : (e2 = Za(e2, i[h2], f), o2 = e2.issues, o2.length > 0 ? _a(e2.issues, c.issues, h2) : g2[h2] = e2.value)) : g2[h2] = i[h2]);
      r2.length > 0 && c.issues.push({ code: "unrecognized_keys", keys: r2, input: i, path: [], continue: true });
    }
    c.value = g2;
  };
})();
var vd = (a, b) => {
  var f = a.length - 1 | 0;
  while (f >= 0) {
    var c = sa(a[f]);
    c = c && (b ? 0 != c.optin : 1 == c.optout);
    if (!c) return f + 1 | 0;
    f--;
  }
  return 0;
};
var wd = (a, b, c, f) => {
  f.issues.length > 0 && _a(f.issues, a.issues, c), ba(b, c, { value: f.value, writable: true, enumerable: true, configurable: true });
};
var Qe;
var zc;
(function() {
  let a = (a2) => {
    if (!X(a2) || Array.isArray(a2)) return false;
    a2 = ea.getPrototypeOf(a2);
    return a2 == null ? true : a2 === ea.prototype;
  }, b = (a2) => {
    if ("string" != typeof a2) return false;
    var b2 = Number(a2);
    return "number" != typeof b2 || !Oa(b2) ? false : b2 + "" == a2 + "";
  };
  Qe = function(c, f, h2) {
    var i = f.value;
    if (!a(i)) {
      ra(f, "record", i);
      return;
    }
    var A2 = fb(c, f, {});
    f.value = A2;
    var d, r2, s2, k2, l2, g2, e2, m2, n2, o2, W2, y, q = "string" == typeof c.def.mode && "loose" == c.def.mode + "";
    r2 = !!c.def.partial, s2 = ya(c, "keyType"), k2 = ya(c, "valueType"), d = void 0, s2 && (c = s2, c.values === void 0 || (d = Array.from(c.values)));
    if (d !== void 0 && Array.isArray(d) && !r2) {
      l2 = ja(), g2 = d.length, c = 0;
      while (c < g2) {
        r2 = d[c], e2 = typeof r2;
        if ("string" != e2 && "number" != e2 && "symbol" != e2) {
          c++;
          continue;
        }
        "number" == typeof r2 && (r2 = r2 + "");
        l2.add(r2);
        if ("string" == typeof r2 && "__proto__" == r2 + "") {
          c++;
          continue;
        }
        r2 = va(s2, d[c], h2);
        if ($(r2)) {
          c++;
          continue;
        }
        e2 = r2.issues;
        if (e2.length > 0) {
          for (y = [], e2 = r2.issues, m2 = e2.length, e2 = 0; e2 < m2; e2++) y.push(Wa(r2.issues[e2], s2, h2));
          r2 = [], e2 = d[c], r2.push(e2), e2 = f.issues, m2 = d[c], e2.push({ code: "invalid_key", origin: "record", issues: y, input: m2, path: r2 }), c++;
          continue;
        }
        n2 = r2.value;
        if ("string" == typeof n2 && "__proto__" == n2 + "") {
          c++;
          continue;
        }
        r2 = d[c];
        xb(f, va(k2, Reflect.get(i, r2), h2), /* @__PURE__ */ ((a2, b2, c2) => function(f2) {
          wd(a2, b2, c2, f2);
        })(f, A2, n2)), c++;
      }
      for (d = [], r2 = oa(i), s2 = r2.length, h2 = 0; h2 < s2; h2++) c = r2[h2], l2.has(c) || (q ? "__proto__" != c + "" && (A2[c] = i[c]) : d.push(c));
      d.length > 0 && f.issues.push({ code: "unrecognized_keys", keys: d, input: i, continue: true });
      return;
    }
    y = [];
    m2 = Reflect.ownKeys(i), o2 = m2.length, n2 = 0;
    while (n2 < o2) {
      c = m2[n2];
      if ("string" == typeof c && "__proto__" == c + "") {
        n2++;
        continue;
      }
      r2 = ea.prototype;
      if (!r2.propertyIsEnumerable.call(i, c)) {
        n2++;
        continue;
      }
      r2 = va(s2, c, h2);
      !$(r2) ? (g2 = r2.issues, e2 = g2.length > 0) : e2 = false, e2 && b(c) && (e2 = va(s2, Number(c), h2), !$(e2) ? (W2 = e2.issues, g2 = 0 == W2.length) : g2 = false, g2 && (r2 = e2)), $(r2) ? e2 = true : (g2 = r2.issues, e2 = g2.length > 0);
      if (e2) {
        if (q) A2[c] = i[c];
        else if (d !== void 0) y.push(c);
        else {
          g2 = [];
          if (!$(r2)) for (e2 = r2.issues, W2 = e2.length, e2 = 0; e2 < W2; e2++) g2.push(Wa(r2.issues[e2], s2, h2));
          r2 = [c], f.issues.push({ code: "invalid_key", origin: "record", issues: g2, input: c, path: r2 });
        }
        n2++;
        continue;
      }
      l2 = r2.value;
      if ("string" == typeof l2 && "__proto__" == l2 + "") {
        n2++;
        continue;
      }
      xb(f, va(k2, i[c], h2), /* @__PURE__ */ ((a2, b2, c2) => function(f2) {
        wd(a2, b2, c2, f2);
      })(f, A2, l2));
      n2++;
    }
    y.length > 0 && f.issues.push({ code: "unrecognized_keys", keys: y, input: i, continue: true });
  }, zc = function(b2, c) {
    if (true === Wd(b2, c)) return { valid: true, data: b2 };
    var f;
    pa(Date, b2) && pa(Date, c) ? (f = b2.getTime(), f = f === c.getTime()) : f = false;
    if (f) return { valid: true, data: b2 };
    if (a(b2) && a(c)) {
      for (var g2, s2, n2, i, r2, e2 = {}, d = [b2, c], h2 = 0; h2 < 2; h2++) for (g2 = d[h2], i = Reflect.ownKeys(g2), s2 = i.length, r2 = 0; r2 < s2; r2++) f = i[r2], n2 = "string" == typeof f && "__proto__" == f + "", n2 || ba(e2, f, { value: g2[f], writable: true, enumerable: true, configurable: true });
      g2 = oa(b2), n2 = oa(c), i = g2.length, r2 = 0;
      while (r2 < i) {
        f = g2[r2] + "";
        if ("__proto__" != f) {
          for (s2 = n2.length, h2 = false, d = 0; d < s2; d++) n2[d] + "" == f && (h2 = true);
          if (h2) {
            h2 = zc(b2[f], c[f]);
            if (!h2.valid) {
              e2 = [f], b2 = h2.mergeErrorPath;
              if (Array.isArray(b2)) for (f = b2.length, c = 0; c < f; c++) r2 = b2[c], e2.push(r2);
              return { valid: false, mergeErrorPath: e2 };
            }
            e2[f] = h2.data;
          }
        }
        r2++;
      }
      return { valid: true, data: e2 };
    }
    if (Array.isArray(b2) && Array.isArray(c)) {
      f = b2.length;
      if (f != c.length) return { valid: false, mergeErrorPath: [] };
      for (h2 = [], r2 = b2.length, f = 0; f < r2; f++) {
        e2 = zc(b2[f], c[f]);
        if (!e2.valid) {
          h2 = [f], b2 = e2.mergeErrorPath;
          if (Array.isArray(b2)) for (f = b2.length, c = 0; c < f; c++) r2 = b2[c], h2.push(r2);
          return { valid: false, mergeErrorPath: h2 };
        }
        d = e2.data;
        h2.push(d);
      }
      return { valid: true, data: h2 };
    }
    return { valid: false, mergeErrorPath: [] };
  };
})();
var xd = (a, b) => {
  for (var f = [], h2 = a.length, c = 0; c < h2; c++) f.push(Wa(a[c], null, b));
  return f;
};
var zb = (a) => {
  if (!(a == null || !X(a) || a._zod === void 0)) {
    var n2 = a._zod, c = n2.bag;
    if (c.propValues !== void 0) return a = n2.bag, a.propValues;
    a = ca(a);
    if (15 == a.kind) {
      c = {};
      var h2 = a.def.shape;
      if (X(h2)) {
        var e2, f, i, s2, b, g2, d = oa(h2), l2 = d.length, r2 = 0;
        while (r2 < l2) {
          e2 = d[r2], a = sa(h2[e2]);
          if (a && a.values !== void 0) {
            for (f = ja(), g2 = Array.from(a.values), i = g2.length, b = 0; b < i; b++) s2 = g2[b], f.add(s2);
            0 != a.optin && f.add(void 0), ba(c, e2, { value: f, writable: true, enumerable: true, configurable: true });
          }
          r2++;
        }
      }
      a = n2.bag;
      a.propValues = c;
      return c;
    }
    if (32 == a.kind) return zb(a.def.in);
    if (29 == a.kind) return zb(wc(a).handle);
    if (21 == a.kind || 38 == a.kind) {
      c = {}, r2 = a.def.options;
      if (Array.isArray(r2)) {
        i = r2.length, f = 0;
        while (f < i) {
          a = zb(r2[f]);
          if (a == null || 0 == oa(a).length) throw la(rf + f + yf);
          for (e2 = oa(a), s2 = e2.length, d = 0; d < s2; d++) {
            h2 = e2[d], Ga.call(c, h2) || ba(c, h2, { value: ja(), writable: true, enumerable: true, configurable: true }), b = a[h2];
            if (b !== void 0 && b != null) for (l2 = Array.from(b), g2 = l2.length, b = 0; b < g2; b++) c[h2].add(l2[b]);
          }
          f++;
        }
      }
      n2.bag.propValues = c;
      return c;
    }
  }
};
var yd = (a) => {
  var b = a.handle._zod, h2 = b.bag;
  if (h2.optionsMap !== void 0) return h2.optionsMap;
  var f = /* @__PURE__ */ new Map(), e2 = a.def.discriminator + "", r2 = a.def.options, g2 = r2.length;
  a = 0;
  while (a < g2) {
    var c = zb(r2[a]);
    b = void 0;
    var d;
    X(c) && Ga.call(c, e2) && (b = c[e2]);
    if (b === void 0) throw la(rf + a + yf);
    if (b == null) throw la(rf + a + yf);
    if (0 == b.size) throw la(rf + a + yf);
    for (c = Array.from(b), d = c.length, b = 0; b < d; b++) {
      var n2 = c[b];
      if (f.has(n2)) throw la('Duplicate discriminator value "' + c[b] + yf);
      f.set(c[b], r2[a]);
    }
    a++;
  }
  h2.optionsMap = f;
  return f;
};
var zd = (a, b, c) => {
  for (var h2, r2, i, n2, e2 = {}, d = {}, p = b.issues, g2 = p.length, p = void 0, f = 0; f < g2; f++) !Ad(b.issues[f], "l", e2, d) ? (h2 = a.issues, r2 = b.issues[f], h2.push(r2)) : p === void 0 && "unrecognized_keys" == b.issues[f].code + "" && (p = b.issues[f]);
  for (f = c.issues, g2 = f.length, f = 0; f < g2; f++) !Ad(c.issues[f], "r", e2, d) ? (h2 = a.issues, r2 = c.issues[f], h2.push(r2)) : p === void 0 && "unrecognized_keys" == c.issues[f].code + "" && (p = c.issues[f]);
  for (f = [], g2 = oa(e2), i = g2.length, d = 0; d < i; d++) h2 = g2[d] + "", r2 = e2[h2], r2.l ? (n2 = e2[h2], r2 = !!n2.r) : r2 = false, r2 && f.push(h2);
  if (f.length > 0 && p !== void 0) {
    for (h2 = [], g2 = p.keys, r2 = f.length, e2 = 0; e2 < r2; e2++) for (i = g2.length, d = 0; d < i; d++) n2 = g2[d] + "", n2 == f[e2] + "" && (n2 = f[e2], h2.push(n2));
    h2.length > 0 && (p = ha({}, p), p.keys = h2, f = a.issues, f.push(p));
  }
  p = zc(b.value, c.value);
  if (p.valid) a.value = p.data;
  else if (!Xb(a)) {
    a = p.mergeErrorPath;
    throw la("Unmergable intersection. Error path: " + JSON.stringify(a, void 0, void 0));
  }
};
var Ad = (a, b, c, f) => {
  var r2, e2, h2 = a.path;
  if ("unrecognized_keys" == a.code + "" && (h2 == null || 0 == h2.length)) {
    for (h2 = a.keys, e2 = h2.length, f = 0; f < e2; f++) a = h2[f] + "", (c[a] === void 0 || c[a] == null) && (c[a] = {}), c[a][b] = true;
    return true;
  }
  return "invalid_key" == a.code + "" && "record" == a.origin + "" && h2 !== void 0 && 1 == h2.length ? (r2 = h2[0] + "", f[r2] === void 0 && (f[r2] = a), (c[r2] === void 0 || c[r2] == null) && (c[r2] = {}), a = c[r2], a[b] = true, true) : false;
};
var Bd = (a) => {
  a = typeof a;
  return "string" == a ? true : "number" == a ? true : "symbol" == a ? true : false;
};
var Cd = (a, b, c, f, h2, r2) => {
  var e2 = a.issues;
  e2.length > 0 && (Bd(f) ? _a(a.issues, c.issues, f) : (e2 = c.issues, e2.push({ code: "invalid_key", origin: "map", issues: xd(a.issues, r2), input: h2, path: [] }))), a = b.issues, a.length > 0 && (Bd(f) ? _a(b.issues, c.issues, f) : (a = c.issues, a.push({ code: "invalid_element", origin: "map", key: f, issues: xd(b.issues, r2), input: h2, path: [] })));
};
var _b = (a, b, c, f) => {
  if ("function" != typeof f) return b;
  var h2 = function(c2) {
    if ("string" == typeof c2) {
      var f2 = b.value;
      c2 = { message: c2, code: "custom", input: f2, inst: a.handle };
    } else {
      c2.fatal && (c2.continue = false), c2.code === void 0 && (c2.code = "custom"), true === "input" in c2 || (c2.input = b.value), c2.inst === void 0 && (c2.inst = a.handle);
    }
    b.issues.push(c2);
  }, r2 = b.value;
  h2 = { addIssue: h2, value: r2, issues: b.issues }, f = f(b.value, h2);
  if ($(f)) {
    if (c !== void 0 && false === c.async) throw new Error(qf);
    return f.then(function(a2) {
      b.value = a2;
      return b;
    });
  }
  b.value = f;
  return b;
};
var Ac = (a) => {
  var b = a.issues, c = b.length;
  for (a = 0; a < c; a++) {
    var f = b[a];
    if ("unrecognized_keys" != f.code + "") return true;
  }
  return false;
};
var ib = (a, b, c) => !!c ? qa(c, a, b) : a;
var Dd = (a, b, c) => {
  if (Ac(b)) return b.aborted = true, b;
  var f = a.def.transform, h2 = ya(a, "out");
  return "function" == typeof f && (f = _b(a, b, c, f), $(f)) ? f.then(function(a2) {
    return ib(a2, c, h2);
  }) : ib(b, c, h2);
};
var Bc = (a, b, c, f) => f ? ((a2, b2, c2, f2, h2) => {
  let r2 = new globalThis.Function("parseAsync,applyFn,input,output,implFn", "return async function(){var a=Array.from(arguments);var p=input?await parseAsync(input,a):a;var r=await applyFn(implFn,this,p);return output?await parseAsync(output,r):r}");
  return r2.apply(void 0, [a2, b2, c2, f2, h2]);
})(function(a2, b2) {
  return Ra(a2, b2, void 0, void 0);
}, function(a2, b2, c2) {
  return a2.apply(b2, c2);
}, b, c, a) : function() {
  return Qa(c, a.apply(this, Qa(b, Array.from(arguments), void 0, void 0)), void 0, void 0);
};
var Ab = (a, b, c) => {
  var h2 = [], f = a.issues, r2 = f.length;
  for (f = 0; f < r2; f++) h2.push(Wa(a.issues[f], b, c));
  b = a.value;
  return { value: b, issues: h2 };
};
var jb = (a) => {
  var b = a.issues;
  return b.length > 0 ? (b = a.issues, { success: false, error: new Ka(b) }) : { success: true, data: a.value };
};
var Cc = (a, b, c) => a._zod.run({ value: b, issues: [] }, c);
var Ed = (a) => {
  if (a == null) return { async: false };
  a = ha({}, a), a.async = false;
  return a;
};
var $b = (a, b, c) => {
  c = Ed(c), b = Cc(a, b, c);
  if ($(b)) throw new Error(qf);
  if (0 == b.issues.length) return { success: true, data: b.value };
  Xa();
  return jb(Ab(b, ca(a), c));
};
var Fd = (a, b, c, f) => {
  Xa();
  var r2 = ca(a), e2 = Ab(b, r2, c), h2 = jb(e2);
  f == null && (f = Fd), ub(h2.error, f);
  throw h2.error;
};
var Qa = (a, b, c, f) => {
  c = Ed(c), b = Cc(a, b, c);
  if ($(b)) throw new Error(qf);
  var h2 = b.issues;
  if (0 == h2.length) return b.value;
  Xa();
  var r2 = ca(a), e2 = Ab(b, r2, c);
  h2 = jb(e2), f == null && (f = Qa), ub(h2.error, f);
  throw h2.error;
};
var ac = /* @__PURE__ */ (function() {
  let a = (a2, b2, c) => {
    if (0 == b2.issues.length) return b2;
    Xa();
    return Ab(b2, a2, c);
  }, b = (b2, c, f) => {
    var h2 = ca(b2);
    b2 = Cc(b2, c, f);
    return $(b2) ? b2.then(function(b3) {
      return a(h2, b3, f);
    }) : a(h2, b2, f);
  };
  return function(a2, c, f) {
    f == null ? f = { async: true } : (f = ha({}, f), f.async = true), a2 = b(a2, c, f);
    return $(a2) ? a2.then(function(a3) {
      return jb(a3);
    }) : Promise.resolve(jb(a2));
  };
})();
var Ra = (a, b, c, f) => ac(a, b, c).then(function(a2) {
  if (a2.success) return a2.data;
  var b2 = f == null ? Ra : f;
  ub(a2.error, b2);
  throw a2.error;
});
var Gd = (a) => {
  var h2 = [];
  if (Array.isArray(a)) {
    for (var b, e2, g2, r2, d, c = a.length, f = 0; f < c; f++) b = a[f], h2.push(b);
    return h2;
  }
  if (!X(a) || a == null) return h2;
  for (r2 = [], f = oa(a), e2 = f.length, c = 0; c < e2; c++) b = a[f[c]], "number" == typeof b && r2.push(b);
  for (c = 0; c < e2; c++) {
    d = Number(f[c] + "");
    if ("number" == typeof d) for (g2 = r2.length, b = 0; ; ) {
      if (b >= g2) {
        b = false;
        break;
      }
      if (r2[b] === d) {
        b = true;
        break;
      }
      b++;
    }
    else {
      b = false;
    }
    b || (b = a[f[c]], h2.push(b));
  }
  return h2;
};
var _ = /* @__PURE__ */ (function() {
  let c = (a2) => {
    var c2 = ea.getOwnPropertyDescriptor(a2, "shape");
    if (!(c2 !== void 0 && c2 != null && "function" == typeof c2.get)) {
      var b2 = a2.shape;
      b2 === void 0 && (b2 = {}), b2 == null && (b2 = {}), fc.set(a2, b2), Ca(a2, { configurable: true, get: function() {
        for (var r2, e2, d, c3 = {}, h3 = Reflect.ownKeys(b2), g2 = h3.length, f2 = 0; f2 < g2; f2++) r2 = h3[f2], e2 = b2, d = h3[f2], ba(c3, r2, { value: Reflect.get(e2, d), writable: true, enumerable: true, configurable: true });
        Ca(a2, { value: c3, writable: true, enumerable: true, configurable: true }), fc.set(a2, c3);
        return c3;
      } });
    }
  }, f = (a2, b2) => {
    var c2 = ja();
    c2.add("ZodType"), c2.add("$ZodType"), c2.add(a2), a2.length > 0 && c2.add("$" + a2), a2 = b2.__parent;
    for (var f2 = 0; a2 !== void 0 && a2 != null && f2 < 8; f2++) b2 = a2.name + "", b2.length > 0 && (c2.add(b2), c2.add("$" + b2)), a2 = b2 = a2.__parent;
    return c2;
  }, a = (a2, b2) => {
    var c2, h3, f2 = b2.check + "";
    ("min_length" == f2 || "min_size" == f2) && (c2 = a2.minimum, (c2 === void 0 || b2.minimum > c2) && (a2.minimum = b2.minimum)), ("max_length" == f2 || "max_size" == f2) && (c2 = a2.maximum, (c2 === void 0 || b2.maximum < c2) && (a2.maximum = b2.maximum)), "length_equals" == f2 && (a2.minimum = b2.length, a2.maximum = b2.length, a2.length = b2.length), "size_equals" == f2 && (a2.minimum = b2.size, a2.maximum = b2.size, a2.size = b2.size), "greater_than" == f2 && (c2 = b2.inclusive, false === c2 ? (c2 = a2.exclusiveMinimum, (c2 === void 0 || b2.value > c2) && (a2.exclusiveMinimum = b2.value)) : (c2 = a2.minimum, (c2 === void 0 || b2.value > c2) && (a2.minimum = b2.value))), "less_than" == f2 && (c2 = b2.inclusive, false === c2 ? (c2 = a2.exclusiveMaximum, (c2 === void 0 || b2.value < c2) && (a2.exclusiveMaximum = b2.value)) : (c2 = a2.maximum, (c2 === void 0 || b2.value < c2) && (a2.maximum = b2.value))), "multiple_of" == f2 && a2.multipleOf === void 0 && (a2.multipleOf = b2.value), "number_format" == f2 && (a2.format = b2.format, c2 = b2.format + "", ("safeint" == c2 || "int" == c2) && (a2.minimum = Number.MIN_SAFE_INTEGER, a2.maximum = Number.MAX_SAFE_INTEGER), "int32" == c2 && (a2.minimum = -2147483648, a2.maximum = 2147483647), "uint32" == c2 && (a2.minimum = 0, a2.maximum = 4294967295), "float32" == c2 && (a2.minimum = -34028234663852886e22, a2.maximum = 34028234663852886e22), "float64" == c2 && (a2.minimum = 0 - +Number.MAX_VALUE, a2.maximum = Number.MAX_VALUE), c2.includes("int") && (a2.pattern = /^-?\d+$/)), ("string_format" == f2 || "lowercase" == f2 || "uppercase" == f2) && (a2.format = "string_format" == f2 ? b2.format : f2, b2.pattern !== void 0 && b2.pattern != null && "jwt" != a2.format + "" && (a2.patterns === void 0 && (a2.patterns = ja()), a2.patterns.add(b2.pattern)), "base64" == a2.format + "" && (a2.contentEncoding = "base64")), "starts_with" == f2 && (a2.patterns === void 0 && (a2.patterns = ja()), h3 = b2.prefix.replace(new RegExp(vf, "g"), "\\$&"), c2 = a2.patterns, c2.add(ma("^" + h3 + ".*"))), "ends_with" == f2 && (a2.patterns === void 0 && (a2.patterns = ja()), h3 = b2.suffix.replace(new RegExp(vf, "g"), "\\$&"), c2 = a2.patterns, c2.add(ma(".*" + h3 + "$"))), "includes" == f2 && (a2.patterns === void 0 && (a2.patterns = ja()), c2 = b2.includes.replace(new RegExp(vf, "g"), "\\$&"), "number" == typeof b2.position ? (h3 = a2.patterns, h3.add(ma("^.{" + b2.position + "}" + c2))) : (h3 = a2.patterns, h3.add(ma(c2 + "")))), "mime_type" == f2 && (a2.mime = b2.mime);
  }, h2 = (b2, c2) => {
    var h3 = b2._zod, f2 = h3.bag;
    "string" == typeof c2.check && a(f2, c2);
    "string" == typeof c2.format && f2.format === void 0 && (f2.format = c2.format);
    b2 = c2.checks;
    if (!!Array.isArray(b2)) for (h3 = b2.length, c2 = 0; c2 < h3; c2++) a(f2, vc(b2[c2]));
  }, b = (a2, b2, r2, e2) => {
    Xa();
    if (X(a2._zod)) var g2 = a2._zod, d = g2.id !== void 0;
    else {
      d = false;
    }
    if (!d) d = { id: 0, kind: 0, handle: null, def: null, ctor: null, typeName: "", trait: "", values: null, optin: 0, optout: 0, hasChecks: false }, Ce(d, b2), d.ctor = r2, d.trait = e2, r2 = f(e2, r2), "string" == typeof b2.check && r2.add("$ZodCheck"), ((a3, b3, c2) => {
      b3.handle = a3;
      var h3, f2 = b3.ctor._zodProto;
      f2 == null && (f2 = {}), f2 = ea.create(f2), f2.id = b3.id, f2.def = b3.def, f2.bag = {}, f2.version = he, f2.traits = c2, f2.constr = b3.ctor, c2 = function(a4, c3) {
        return yb(b3, a4, c3);
      }, h3 = function(a4, c3) {
        return qa(b3, a4, c3);
      }, f2.parse = c2, f2.run = h3, vb(b3) && (f2.memoizer = { recursive: void 0, handoff: void 0, ctx: void 0, bucket: void 0, open: [] }), ba(f2, "propValues", { enumerable: true, configurable: true, get: function() {
        return zb(a3);
      } }), ba(a3, "_zod", Nb(f2)), Re(a3, b3, f2, c2, h3);
    })(a2, d, r2), 15 == d.kind && c(b2), h2(a2, b2), ((a3) => {
      var b3 = a3.kind, c2 = a3.handle._zod, f2 = a3.handle;
      if (29 == b3) {
        b3 = a3.def.getter, ((a4, b4) => {
          let f3 = /* @__PURE__ */ Symbol.for("evaluating"), c3 = { value: void 0 };
          ba(a4, "innerType", { configurable: true, get: function() {
            if (c3.value !== f3) return c3.value === void 0 && (c3.value = f3, c3.value = b4()), c3.value;
          }, set: function(b5) {
            ba(a4, "innerType", { value: b5, configurable: true, writable: true });
          } });
        })(c2, function() {
          var c3 = f2._zod, a4 = c3.def;
          a4._cachedInner === void 0 && (a4._cachedInner = b3());
          return a4._cachedInner;
        }), ia(a3, "pattern", function(a4) {
          var b4 = a4.innerType;
          if (X(b4) && X(b4._zod)) return a4 = b4._zod, a4.pattern;
        }), ia(a3, "propValues", function(a4) {
          var b4 = a4.innerType;
          if (X(b4) && X(b4._zod)) return a4 = b4._zod, a4.propValues;
        }), ia(a3, "optin", function(a4) {
          var b4 = a4.innerType;
          if (X(b4) && X(b4._zod)) return a4 = b4._zod, a4.optin;
        }), ia(a3, "optout", function(a4) {
          var b4 = a4.innerType;
          if (X(b4) && X(b4._zod)) return a4 = b4._zod, a4.optout;
        });
        return;
      }
      if (23 == b3) {
        ia(a3, "optin", function(a4) {
          var c3 = a4.def, b4 = c3.innerType;
          return X(b4) && X(b4._zod) && "defaulted" == b4._zod.optin + "" ? "defaulted" : "optional";
        }), c2.optout = "optional", ia(a3, "values", function(a4) {
          var c3 = a4.def, b4 = c3.innerType;
          if (!(!X(b4) || !X(b4._zod) || b4._zod.values === void 0)) {
            c3 = ja();
            var f3 = Array, h3 = b4._zod;
            f3 = f3.from(h3.values);
            var r3 = f3.length;
            for (b4 = 0; b4 < r3; b4++) h3 = f3[b4], c3.add(h3);
            a4.def.exact || c3.add(void 0);
            return c3;
          }
        }), ia(a3, "pattern", function(a4) {
          var b4 = a4.def, c3 = b4.innerType;
          if (!(!X(c3) || !X(c3._zod))) return a4 = c3._zod, b4 = a4.pattern, b4 == null ? void 0 : ma("^(" + Qb(b4.source + "") + ")?$");
        });
        return;
      }
      if (24 == b3) {
        ia(a3, "optin", function(a4) {
          var c3 = a4.def, b4 = c3.innerType;
          if (X(b4) && X(b4._zod)) return a4 = b4._zod, a4.optin;
        }), ia(a3, "optout", function(a4) {
          var c3 = a4.def, b4 = c3.innerType;
          if (X(b4) && X(b4._zod)) return a4 = b4._zod, a4.optout;
        }), ia(a3, "pattern", function(a4) {
          var b4 = a4.def, c3 = b4.innerType;
          if (!(!X(c3) || !X(c3._zod))) return a4 = c3._zod, b4 = a4.pattern, b4 == null ? void 0 : ma("^(" + Qb(b4.source + "") + "|null)$");
        }), ia(a3, "values", function(a4) {
          var c3 = a4.def, b4 = c3.innerType;
          if (!(!X(b4) || !X(b4._zod) || b4._zod.values === void 0)) {
            a4 = ja(), c3 = Array;
            var f3 = b4._zod;
            c3 = c3.from(f3.values);
            var h3 = c3.length;
            for (b4 = 0; b4 < h3; b4++) f3 = c3[b4], a4.add(f3);
            a4.add(null);
            return a4;
          }
        });
        return;
      }
      if (21 == b3 || 38 == b3) {
        ia(a3, "optin", function(a4) {
          var b4 = a4.def, c3 = b4.options;
          if (!!Array.isArray(c3)) {
            var h3 = c3.length;
            for (a4 = false, b4 = 0; b4 < h3; b4++) {
              var r3 = c3[b4], e3 = r3._zod, f3 = e3.optin;
              if ("defaulted" == f3 + "") return "defaulted";
              f3 === void 0 || (a4 = true);
            }
            if (a4) return "optional";
          }
        }), ia(a3, "optout", function(a4) {
          var b4 = a4.def, c3 = b4.options;
          if (!!Array.isArray(c3)) for (b4 = c3.length, a4 = 0; a4 < b4; a4++) {
            var f3 = c3[a4], h3 = f3._zod;
            if ("optional" == h3.optout + "") return "optional";
          }
        }), ia(a3, "values", function(a4) {
          var b4 = a4.def, f3 = b4.options;
          if (!!Array.isArray(f3)) {
            var r3 = ja(), e3 = f3.length;
            a4 = 0;
            while (a4 < e3) {
              var c3 = f3[a4], h3 = c3._zod;
              b4 = h3.values;
              if (b4 === void 0) return;
              for (c3 = Array.from(b4), h3 = c3.length, b4 = 0; b4 < h3; b4++) {
                var d2 = c3[b4];
                r3.add(d2);
              }
              a4++;
            }
            return r3;
          }
        }), ia(a3, "pattern", function(a4) {
          var b4 = a4.def, c3 = b4.options;
          if (!!Array.isArray(c3)) {
            var f3 = [], h3 = c3.length;
            for (a4 = 0; a4 < h3; a4++) {
              var r3 = c3[a4], e3 = r3._zod;
              b4 = e3.pattern;
              if (b4 == null) return;
              f3.push(Qb(b4.source + ""));
            }
            return ma("^(" + f3.join("|") + ")$");
          }
        });
        return;
      }
      if (32 == b3) {
        ia(a3, "values", function(a4) {
          var c3 = a4.def, b4 = c3.in;
          if (X(b4) && X(b4._zod)) return a4 = b4._zod, a4.values;
        }), ia(a3, "optin", function(a4) {
          var c3 = a4.def, b4 = c3.in;
          if (X(b4) && X(b4._zod)) return a4 = b4._zod, a4.optin;
        }), ia(a3, "optout", function(a4) {
          var c3 = a4.def, b4 = c3.out;
          if (X(b4) && X(b4._zod)) return a4 = b4._zod, a4.optout;
        });
        return;
      }
      if (27 == b3) {
        ia(a3, "optin", function(a4) {
          var c3 = a4.def, b4 = c3.innerType;
          return X(b4) && X(b4._zod) && "defaulted" == b4._zod.optin + "" ? "defaulted" : "optional";
        }), ia(a3, "optout", function(a4) {
          var c3 = a4.def, b4 = c3.innerType;
          if (X(b4) && X(b4._zod)) return a4 = b4._zod, a4.optout;
        }), ia(a3, "values", function(a4) {
          var c3 = a4.def, b4 = c3.innerType;
          if (X(b4) && X(b4._zod)) return a4 = b4._zod, a4.values;
        });
        return;
      }
      if (40 == b3) return;
      if (33 == b3) {
        ia(a3, "optin", function(a4) {
          var c3 = a4.def, b4 = c3.innerType;
          if (X(b4) && X(b4._zod)) return a4 = b4._zod, a4.optin;
        }), ia(a3, "optout", function(a4) {
          var c3 = a4.def, b4 = c3.innerType;
          if (X(b4) && X(b4._zod)) return a4 = b4._zod, a4.optout;
        }), ia(a3, "values", function(a4) {
          var c3 = a4.def, b4 = c3.innerType;
          if (X(b4) && X(b4._zod)) return a4 = b4._zod, a4.values;
        });
        return;
      }
      ia(a3, "pattern", function(a4) {
        return ((a5) => {
          var f3, c3 = a5.kind, b4 = {};
          X(a5.handle) && X(a5.handle._zod) && (f3 = a5.handle._zod, b4 = f3.bag);
          if (0 == c3) {
            if (X(b4) && b4.patterns !== void 0 && (f3 = Array.from(b4.patterns), c3 = f3.length, c3 > 0)) return f3[c3 - 1];
            if (a5.def.pattern !== void 0) return a5.def.pattern;
            a5 = "number" == typeof b4.minimum ? b4.minimum + "" : "0", c3 = "number" == typeof b4.maximum ? b4.maximum + "" : Af;
            return ma("^[\\s\\S]{" + a5 + "," + c3 + "}$");
          }
          if (1 == c3) {
            if (b4.pattern !== void 0 && b4.pattern != null) return b4.pattern;
            a5 = "string" == typeof b4.format ? b4.format + "" : Af;
            return a5.includes("int") ? ma("^-?\\d+$") : ma("^-?\\d+(?:\\.\\d+)?$");
          }
          if (2 == c3) return /^(?:true|false)$/i;
          if (3 == c3) return ma("^-?\\d+n?$");
          if (7 == c3) return ma("^undefined$");
          if (8 == c3) return ma("^null$");
          if (13 == c3) {
            var h3 = Array.from(a5.values);
            b4 = [];
            var r3 = h3.length;
            for (c3 = 0; c3 < r3; c3++) a5 = h3[c3], "string" == typeof a5 ? b4.push(Pb(a5 + "")) : a5 == null && a5 === void 0 ? b4.push("undefined") : a5 == null ? b4.push("null") : b4.push(Pb(a5 + ""));
            return ma("^(" + b4.join("|") + ")$");
          }
          if (14 == c3) {
            for (f3 = Array.from(a5.values), h3 = [], r3 = f3.length, b4 = 0; b4 < r3; b4++) a5 = f3[b4], ("string" == typeof a5 || "number" == typeof a5 || "symbol" == typeof a5) && h3.push(Pb(a5 + ""));
            return ma("^(" + h3.join("|") + ")$");
          }
          if (40 == c3) return b4 = a5.handle._zod, b4.pattern;
        })(Ja[+a4.id]);
      });
      0 != a3.optin ? c2.optin = Zc(a3) : ia(a3, "optin", function(a4) {
        return Zc(Ja[+a4.id]);
      }), 0 != a3.optout ? c2.optout = _c(a3) : ia(a3, "optout", function(a4) {
        return _c(Ja[+a4.id]);
      }), a3.values === void 0 || (c2.values = a3.values);
    })(d);
  };
  return function(a2, c2) {
    var h3 = {};
    if (c2 !== void 0 && X(c2._zodProto)) var r2 = c2._zodProto;
    var f2 = (0, function(c3) {
      if (this === void 0 || this == null || !X(this)) {
        var h4 = f2.prototype;
        h4 = ea.create(h4);
      } else {
        h4 = this;
      }
      c3 !== void 0 && X(c3) && "string" == typeof c3.type && b(h4, c3, f2, a2);
      return h4;
    });
    f2.__parent = c2, f2._zodProto = h3, c2 === void 0 || (h3 = f2.prototype, r2 = c2.prototype, ea.setPrototypeOf(h3, r2)), c2 = f2.prototype, ba(c2, "def", { configurable: true, enumerable: true, get: function() {
      let a3 = this._zod;
      return a3.def;
    } }), c2 = f2.prototype, ba(c2, "type", { configurable: true, enumerable: true, get: function() {
      let a3 = this._zod, b2 = a3.def;
      return b2.type;
    } }), c2 = f2, ba(c2, "name", Ia(a2)), c2 = f2, ba(c2, "init", Ia(function(c3, h4) {
      h4 !== void 0 && X(h4) && b(c3, h4, f2, a2);
      return c3;
    })), h3 = Symbol.hasInstance, ba(f2, h3, Ia(function(b2) {
      if (b2 == null || !X(b2)) return false;
      var c3 = b2._zod;
      if (c3 == null) return false;
      b2 = c3.traits;
      return !!b2.has(a2);
    }));
    return f2;
  };
})();
var Sa = (a) => {
  if (a == null) return {};
  if ("string" == typeof a) return { error: function() {
    return a;
  } };
  if ("function" == typeof a) return { error: a };
  if (!X(a)) return {};
  if (a.message !== void 0) {
    if (a.error !== void 0) throw la("Cannot specify both `message` and `error` params");
    a.error = a.message;
  }
  Reflect.deleteProperty(a, "message");
  if ("string" == typeof a.error) {
    var c = a.error, b = ha({}, a);
    b.error = function() {
      return c;
    };
    return b;
  }
  return a;
};
var Hd = (a) => "number" == typeof a ? (a = a | 0, a == -1 ? "(?:[01]\\d|2[0-3]):[0-5]\\d" : 0 == a ? "(?:[01]\\d|2[0-3]):[0-5]\\d:[0-5]\\d" : "(?:[01]\\d|2[0-3]):[0-5]\\d:[0-5]\\d\\.\\d{" + a + "}") : "(?:[01]\\d|2[0-3]):[0-5]\\d(?::[0-5]\\d(?:\\.\\d+)?)?";
var Id = (a) => {
  var b = void 0;
  X(a) && a != null && (b = a.precision);
  return ma("^" + Hd(b) + "$");
};
var Jd = (a) => {
  var b;
  if (X(a) && a != null) {
    b = a.precision;
    var c, f = a.offset;
    c = a.local;
  } else {
    f = false, c = false;
  }
  b = Hd(b);
  a = f ? "Z|([+-](?:[01]\\d|2[0-3]):[0-5]\\d)" : "Z", a = "(?:" + a + ")", c && (a = a + "?");
  return ma("^(?:(?:\\d\\d[2468][048]|\\d\\d[13579][26]|\\d\\d0[48]|[02468][048]00|[13579][26]00)-02-29|\\d{4}-(?:(?:0[13578]|1[02])-(?:0[1-9]|[12]\\d|3[01])|(?:0[469]|11)-(?:0[1-9]|[12]\\d|30)|(?:02)-(?:0[1-9]|1\\d|2[0-8])))T(?:" + b + a + ")$");
};
var Bb = (a) => {
  ca(a).def.coerce = true;
  var b = a._zod;
  if (!(b["~pf"] === void 0 || b["~rf"] === void 0)) b.parse = b["~pf"], b.run = b["~rf"];
};
var aa = (a, b) => {
  a = { type: a }, ha(a, Sa(b));
  return a;
};
var Kd = (a, b) => {
  if (Array.isArray(a)) {
    for (var f = {}, h2 = a.length, c = 0; c < h2; c++) ba(f, a[c], { value: a[c], writable: true, enumerable: true, configurable: true });
    a = f;
  }
  b = aa("enum", b);
  b.entries = a;
  return new Ma(b);
};
var Dc = (a, b) => {
  a == null && (a = {}), b = aa("object", b), b.shape = a;
  return new ka(b);
};
var Ec = (a, b, c) => {
  var f = aa("tuple", c);
  f.items = a, b !== void 0 && b != null && (f.rest = b);
  return new lc(f);
};
var bc = (a, b, c) => {
  if (b == null || b._zod === void 0) {
    c = new Z(aa("string", void 0));
    var f = a;
    a = c, c = b, b = f;
  }
  c = aa("record", c);
  c.keyType = a, c.valueType = b;
  return new Fb(c);
};
var Ld = (a, b) => Ub(function(b2) {
  var c = this;
  b2.addIssue = function(a2) {
    if ("string" == typeof a2) {
      var f = b2.value;
      a2 = { message: a2, code: "custom", input: f, inst: c, path: [] };
    } else {
      a2.fatal && (a2.continue = false), a2.code === void 0 && (a2.code = "custom"), true === "input" in a2 || (a2.input = b2.value), a2.inst === void 0 && (a2.inst = c), a2.continue === void 0 && (a2.continue = true);
    }
    b2.issues.push(a2);
  };
  return a(b2.value, b2);
}, b);
var Md = (a, b, c) => {
  let f = c.decode;
  return new qb({ type: "pipe", in: a, out: b, transform: f, reverseTransform: c.encode });
};
var Ta = (a, b, c, f) => {
  a == null && (a = Z);
  var h2 = aa("string", f);
  h2.format = b, h2.check = "string_format", c !== void 0 && c != null && (h2.pattern = c);
  return new a(h2);
};
var Fa = (a, b, c) => function(f) {
  return Ta(a, b, c, f);
};
var Nd = (a) => {
  var b = Tb();
  a !== void 0 && a != null && (ha(b, a), true === "localeError" in a && (ab = a.localeError), true === "customError" in a && (Cb = a.customError));
  return b;
};
var Od = (a, b, c) => Qa(a, b, c, Od);
var Pd = (a, b, c) => Ra(a, b, c, Pd);
var cc = (a, b) => {
  a = a == null ? {} : ha({}, a), a.direction = b;
  return a;
};
var Qd = (a, b, c) => Qa(a, b, cc(c, "backward"), Qd);
var Rd = (a, b, c) => Qa(a, b, cc(c, "forward"), Rd);
var Sd = (a, b, c) => Ra(a, b, cc(c, "backward"), Sd);
var Td = (a, b, c) => Ra(a, b, cc(c, "forward"), Td);
var Ud = (a) => a;
var W = (a, b) => {
  sb[a] = b;
};
var xa = (a) => sa(a.def.innerType);
var ma = (a) => new RegExp(a, Af);
var Ca = (a, b) => {
  ea.defineProperty(a, "shape", b);
};
var ea = Object;
var w = ea.prototype;
var Ga = w.hasOwnProperty;
var x = ea.prototype;
var Vd = x.isPrototypeOf;
Array.prototype.slice;
var Wd = ea.is;
var Xd = Aa("inst", "return function(payload){if(typeof payload.value==='string')return payload;payload.issues.push({expected:'string',code:'invalid_type',input:payload.value,inst:inst});return payload;}");
var Yd = Aa("inst", "return function(payload){var v=payload.value;if(typeof v==='number'&&Number.isFinite(v))return payload;var iss={expected:'number',code:'invalid_type',input:v,inst:inst};if(typeof v==='number'){if(Number.isNaN(v))iss.received='NaN';else if(!Number.isFinite(v))iss.received=String(v);}payload.issues.push(iss);return payload;}");
var Zd = Aa("inst", "return function(payload){if(typeof payload.value==='boolean')return payload;payload.issues.push({expected:'boolean',code:'invalid_type',input:payload.value,inst:inst});return payload;}");
var Gc = Aa(Af, "var recCache=new WeakMap();function isRecursive(node,stack){var cached=recCache.get(node);if(cached!==void 0)return cached;if(stack.has(node))return true;stack.add(node);var result=false;function check(child){if(!result&&child&&child._zod)result=isRecursive(child,stack);}var def=node._zod&&node._zod.def;if(!def){stack.delete(node);recCache.set(node,false);return false;}if(def.type==='lazy'){stack.delete(node);recCache.set(node,true);return true;}var shape=def.shape;if(shape)for(var key in shape)check(shape[key]);for(var k in def){var value=def[k];if(!value||typeof value!=='object')continue;if(value._zod)check(value);else if(Array.isArray(value))for(var i=0;i<value.length;i++)check(value[i]);}stack.delete(node);recCache.set(node,result);return result;}return function useLil(node){return isRecursive(node,new Set());};")();
var e = "def,inst";
var _d = Aa(e, "return function(payload){var input=payload.value;var fmt=def.format;if(fmt==='int'||fmt==='int32'||fmt==='safeint'){if(typeof input!=='number'||!Number.isInteger(input)){payload.issues.push({expected:'int',code:'invalid_type',input:input,inst:inst,continue:false});return;}}if(fmt==='safeint'&&!Number.isSafeInteger(input)){if(input>0)payload.issues.push({code:'too_big',maximum:9007199254740991,inclusive:true,origin:'number',input:input,inst:inst,continue:!def.abort});else payload.issues.push({code:'too_small',minimum:-9007199254740991,inclusive:true,origin:'number',input:input,inst:inst,continue:!def.abort});}};");
var $d = Aa(e, "return function(payload){var p=def.pattern;if(!p)return;p.lastIndex=0;if(p.test(payload.value))return;var extra={origin:'string',code:'invalid_format',format:def.format,input:payload.value,inst:inst,continue:!def.abort};extra.pattern=p.toString();payload.issues.push(extra);};");
var ae = Aa(e, "return function(payload){var v=payload.value;if(v==null||v.length==null)return;var n=typeof v==='string'?Array.from(v).length:v.length;if(n<def.minimum)payload.issues.push({origin:Array.isArray(v)?'array':'string',code:'too_small',minimum:def.minimum,inclusive:true,input:v,inst:inst,continue:!def.abort});};");
var be = Aa(e, "return function(payload){var v=payload.value;if(v==null||v.length==null)return;var n=typeof v==='string'?Array.from(v).length:v.length;if(n>def.maximum)payload.issues.push({origin:Array.isArray(v)?'array':'string',code:'too_big',maximum:def.maximum,inclusive:true,input:v,inst:inst,continue:!def.abort});};");
var ce = Aa(e, "return function(payload){var v=payload.value;var bound=def.value;var inclusive=def.inclusive!==false;var ok=inclusive?v>=bound:v>bound;if(ok)return;var origin=typeof v==='number'?'number':typeof v==='bigint'?'bigint':typeof v==='object'?'date':def.origin;if(!origin)origin='number';payload.issues.push({origin:origin,code:'too_small',minimum:typeof bound==='object'&&bound&&typeof bound.getTime==='function'?bound.getTime():bound,inclusive:inclusive,input:v,inst:inst,continue:!def.abort});};");
var de = Aa(e, "return function(payload){var v=payload.value;var bound=def.value;var inclusive=def.inclusive!==false;var ok=inclusive?v<=bound:v<bound;if(ok)return;var origin=typeof v==='number'?'number':typeof v==='bigint'?'bigint':typeof v==='object'?'date':def.origin;if(!origin)origin='number';payload.issues.push({origin:origin,code:'too_big',maximum:typeof bound==='object'&&bound&&typeof bound.getTime==='function'?bound.getTime():bound,inclusive:inclusive,input:v,inst:inst,continue:!def.abort});};");
var ee = Aa("intern,fail", "return function(data,params){var ctx=params==null?{async:false}:Object.assign({},params,{async:false});var result=intern.run({value:data,issues:[]},ctx);if(result instanceof Promise)throw new Error('Encountered Promise during synchronous parse. Use .parseAsync() instead.');if(!result.issues.length)return result.value;return fail(result,ctx);}");
var fe = Aa("intern,fail", "return function(data,params){var ctx=params==null?{async:false}:Object.assign({},params,{async:false});var result=intern.run({value:data,issues:[]},ctx);if(result instanceof Promise)throw new Error('Encountered Promise during synchronous parse. Use .parseAsync() instead.');if(!result.issues.length)return {success:true,data:result.value};return fail(result,ctx);}");
var Ja = [];
var dc = 0;
var ec = void 0;
var $a = false;
var ge = { configurable: true, get: function() {
  $a = true;
} };
var Hc = /* @__PURE__ */ new WeakMap();
var fc = /* @__PURE__ */ new WeakMap();
var ab = void 0;
var Cb = void 0;
var bb = void 0;
var he = { major: 4, minor: 4, patch: 3 };
var ua = (0, function(a) {
  var b;
  b = this === void 0 || this == null || !X(this) ? ea.create(b) : this, sc(b, a), b.stack === void 0 && ub(b, ua);
  return b;
});
var Ka = function(a) {
  var b;
  b = ea.create(b), sc(b, a), ub(b, Ka);
  return b;
};
var Ic = function(a) {
  let b = Error;
  b = new b("Encountered unidirectional transform during encode: " + a), b.name = "ZodEncodeError";
  return b;
};
var Jc = /^(?!\.)(?!.*\.\.)([A-Za-z0-9_'+\-\.]*)[A-Za-z0-9_+-]@([A-Za-z0-9][A-Za-z0-9\-]*\.)+[A-Za-z]{2,}$/;
var kb = /^([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[1-8][0-9a-fA-F]{3}-[89abAB][0-9a-fA-F]{3}-[0-9a-fA-F]{12}|00000000-0000-0000-0000-000000000000|ffffffff-ffff-ffff-ffff-ffffffffffff)$/;
var Kc = /^([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})$/;
var Lc = /^[0-9A-HJKMNP-TV-Za-hjkmnp-tv-z]{26}$/;
var Mc = /^[a-zA-Z0-9_-]{21}$/;
var Nc = /^(?:(?:25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]|[0-9])\.){3}(?:25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]|[0-9])$/;
var Oc = /^(([0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}|([0-9a-fA-F]{1,4}:){1,7}:|([0-9a-fA-F]{1,4}:){1,6}:[0-9a-fA-F]{1,4}|([0-9a-fA-F]{1,4}:){1,5}(:[0-9a-fA-F]{1,4}){1,2}|([0-9a-fA-F]{1,4}:){1,4}(:[0-9a-fA-F]{1,4}){1,3}|([0-9a-fA-F]{1,4}:){1,3}(:[0-9a-fA-F]{1,4}){1,4}|([0-9a-fA-F]{1,4}:){1,2}(:[0-9a-fA-F]{1,4}){1,5}|[0-9a-fA-F]{1,4}:((:[0-9a-fA-F]{1,4}){1,6})|:((:[0-9a-fA-F]{1,4}){1,7}|:))$/;
/^$|^(?:[0-9a-zA-Z+\/]{4})*(?:(?:[0-9a-zA-Z+\/]{2}==)|(?:[0-9a-zA-Z+\/]{3}=))?$/, e = /^(?:(?:\d\d[2468][048]|\d\d[13579][26]|\d\d0[48]|[02468][048]00|[13579][26]00)-02-29|\d{4}-(?:(?:0[13578]|1[02])-(?:0[1-9]|[12]\d|3[01])|(?:0[469]|11)-(?:0[1-9]|[12]\d|30)|(?:02)-(?:0[1-9]|1\d|2[0-8])))$/, /^(?:[01]\d|2[0-3]):[0-5]\d(?::[0-5]\d(?:\.\d+)?)?$/, /^(?:(?:\d\d[2468][048]|\d\d[13579][26]|\d\d0[48]|[02468][048]00|[13579][26]00)-02-29|\d{4}-(?:(?:0[13578]|1[02])-(?:0[1-9]|[12]\d|3[01])|(?:0[469]|11)-(?:0[1-9]|[12]\d|30)|(?:02)-(?:0[1-9]|1\d|2[0-8])))T(?:(?:[01]\d|2[0-3]):[0-5]\d(?::[0-5]\d(?:\.\d+)?)?)Z$/;
var Pc = /^P(?:(\d+W)|(?!.*W)(?=\d|T\d)(\d+Y)?(\d+M)?(\d+D)?(T(?=\d)(\d+H)?(\d+M)?(\d+([.,]\d+)?S)?)?)$/;
var g = /^[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+$/;
var ne = new RegExp("^[\\p{Extended_Pictographic}\\p{Emoji_Component}]+$", "u");
var Qc = /^\d(?:[ -]?\d){11,18}$/;
var te = /^https?$/;
var ue = /^(?=.{1,253}$)([a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?\.)+[a-zA-Z]{2,63}$/;
var ve = /^[0-9a-fA-F]{32}$/;
var we = /^[0-9a-fA-F]{40}$/;
var Rc = /^[0-9a-fA-F]{64}$/;
var xe = /^[0-9a-fA-F]{96}$/;
var ye = /^[0-9a-fA-F]{128}$/;
var Y = _("ZodType", void 0);
var Z = _("ZodString", Y);
var ga = _("ZodNumber", Y);
var cb = _("ZodBoolean", Y);
var na = _("ZodBigInt", Y);
var gc = _("ZodSymbol", Y);
var Da = _("ZodDate", Y);
var hc = _("ZodNaN", Y);
var ic = _("ZodUndefined", Y);
var Db = _("ZodNull", Y);
var jc = _("ZodAny", Y);
var La = _("ZodUnknown", Y);
var lb = _("ZodNever", Y);
var kc = _("ZodVoid", Y);
var Eb = _("ZodLiteral", Y);
var Ma = _("ZodEnum", Y);
var ka = _("ZodObject", Y);
var za = _("ZodArray", Y);
var lc = _("ZodTuple", Y);
var Fb = _("ZodRecord", Y);
var mb = _("ZodMap", Y);
var nb = _("ZodSet", Y);
var Na = _("ZodUnion", Y);
var mc = _("ZodDiscriminatedUnion", Na);
var Gb = _("ZodXor", Na);
var Hb = _("ZodIntersection", Y);
var db = _("ZodOptional", Y);
var eb = _("ZodExactOptional", db);
var nc = _("ZodNullable", Y);
var Ib = _("ZodDefault", Y);
var Jb = _("ZodPrefault", Y);
var Kb = _("ZodCatch", Y);
var oc = _("ZodNonOptional", Y);
var ob = _("ZodLazy", Y);
var Lb = _("ZodPromise", Y);
var pb = _("ZodTransform", Y);
var Ha = _("ZodPipe", Y);
var qb = _("ZodCodec", Ha);
var Mb = _("ZodPreprocess", Ha);
var Sc = _("ZodReadonly", Y);
var Ua = _("ZodCustom", Y);
var rb = _("ZodFile", Y);
var pc = _("ZodSuccess", Y);
var Va = _("ZodFunction", Y);
var qc = _("ZodTemplateLiteral", Y);
var Tc = _("ZodISODateTime", Z);
var j = _("ZodISODate", Z);
var Uc = _("ZodISOTime", Z);
var k = _("ZodISODuration", Z);
var h = _("ZodEmail", Z);
var u = _("ZodGUID", Z);
var v = _("ZodUUID", Z);
var Vc = _("ZodURL", Z);
_("ZodCUID", Z), _("ZodCUID2", Z), _("ZodULID", Z), _("ZodNanoID", Z), _("ZodBase64", Z), _("ZodIPv4", Z), _("ZodIPv6", Z);
var z = _("ZodJWT", Z);
_("ZodEmoji", Z);
var l = (a, b) => {
  let c = { errors: [] };
  tc(a.issues, [], c, b);
  return c;
};
var m = (a) => {
  var c = a.issues, f = c.slice(0);
  f.sort(function(a2, b2) {
    if (Array.isArray(a2.path)) var c2 = a2.path, f2 = c2.length;
    else {
      f2 = 0;
    }
    Array.isArray(b2.path) ? (c2 = b2.path, a2 = c2.length) : a2 = 0;
    return f2 - a2 | 0;
  });
  var h2 = [], r2 = f.length;
  for (c = 0; c < r2; c++) {
    a = f[c];
    var b = "\u2716 " + a.message;
    h2.push(b);
    if (Array.isArray(a.path)) {
      var e2 = a.path;
      b = e2.length > 0;
    } else {
      b = false;
    }
    b && (b = "  \u2192 at " + id(a.path), h2.push(b));
  }
  return h2.join("\n");
};
var n = (a, b) => {
  let c = { _errors: [] };
  Sb(a.issues, [], c, b);
  return c;
};
var o = hd;
var A = Ya();
var r = (a, b) => yd(ca(a)).get(b);
kf();
var B = (0, function(a) {
  return new Z(aa("string", a));
});
var C = (0, function(a) {
  return new ga(aa("number", a));
});
var D = (0, function(a) {
  return new cb(aa("boolean", a));
});
var E = (0, function(a) {
  return new na(aa("bigint", a));
});
var F = (0, function(a) {
  return new gc(aa("symbol", a));
});
var G = (0, function(a) {
  return new Da(aa("date", a));
});
var H = (0, function(a) {
  return new hc(aa("nan", a));
});
var I = (0, function() {
  return new ic({ type: "undefined" });
});
var J = (0, function() {
  return new Db({ type: "null" });
});
var K = (0, function() {
  return new jc({ type: "any" });
});
var L = (0, function() {
  return new La({ type: "unknown" });
});
var M = (0, function() {
  return new lb({ type: "never" });
});
var N = (0, function() {
  return new kc({ type: "void" });
});
var O = (0, function(a, b) {
  var f = a;
  if (!Array.isArray(f)) {
    var c = [f];
    f = c;
  }
  c = aa("literal", b);
  c.values = f, c = new Eb(c);
  return c;
});
var s = (0, function(a, b) {
  return Kd(a, b);
});
var t = (0, function(a, b) {
  return Dc(a, b);
});
var P = (0, function(a, b) {
  let c = a;
  c = Dc(c, b);
  return c.strict();
});
var Q = (0, function(a, b) {
  let c = a;
  c = Dc(c, b);
  return c.passthrough();
});
var R = (0, function(a, b) {
  let c = b;
  c = aa("array", c), c.element = a;
  return new za(c);
});
var S = (0, function(a, b, c) {
  return Ec(a, b, c);
});
var T = (0, function(a, b, c) {
  return bc(a, b, c);
});
var U = (0, function(a, b) {
  let c = b;
  c = aa("union", c), c.options = a;
  return new Na(c);
});
var V = (0, function(a, b) {
  let c = b;
  c = aa("union", c), c.options = a, c.inclusive = false;
  return new Gb(c);
});
var Ff = (0, function(a, b, c) {
  return ((a2, b2, c2) => {
    var h2 = aa("union", c2);
    h2.options = b2, h2.discriminator = a2, h2.inclusive = false;
    if (Array.isArray(b2)) {
      var e2 = b2.length;
      c2 = 0;
      while (c2 < e2) {
        var f = b2[c2];
        if (f === void 0) c2++;
        else if (f == null) c2++;
        else if (!X(f)) c2++;
        else {
          var r2 = f._zod;
          if (r2 === void 0) c2++;
          else if (r2 == null) c2++;
          else {
            f = r2.def;
            if (f === void 0) c2++;
            else if (f == null) c2++;
            else {
              f = fc.get(f);
              if (f === void 0) c2++;
              else if (f == null) c2++;
              else {
                if (!Ga.call(f, a2)) throw la(rf + c2 + yf);
                c2++;
              }
            }
          }
        }
      }
    }
    return new mc(h2);
  })(a + "", b, c);
});
var Gf = (0, function(a, b) {
  return new Hb({ type: "intersection", left: a, right: b });
});
var Hf = (0, function(a, b) {
  return new mb({ type: "map", keyType: a, valueType: b });
});
var If = (0, function(a, b) {
  let c = b;
  c = aa("set", c), c.valueType = a;
  return new nb(c);
});
var Jf = (0, function(a) {
  return new ob({ type: "lazy", getter: a });
});
var Kf = (0, function(a) {
  return new Lb({ type: "promise", innerType: a });
});
var Lf = (0, function(a, b) {
  return ((a2, b2) => {
    "function" == typeof a2 || (a2 = function(a3) {
      return true;
    });
    b2 = aa("custom", b2), b2.fn = a2, b2.check = "custom";
    return new Ua(b2);
  })(a, b);
});
var Mf = (0, function(a) {
  return new rb(aa("file", a));
});
var Nf = (0, function(a, b) {
  return ((a2, b2) => {
    b2 = aa("custom", b2), b2.check = "custom", b2.abort = true, b2.fn = function(b3) {
      return pa(a2, b3) ? true : false;
    }, b2 = new Ua(b2);
    let c = ca(b2).handle._zod, f = c.bag;
    f.Class = a2;
    return b2;
  })(a, b);
});
var Of = (0, function(a) {
  return new pb({ type: "transform", transform: a });
});
var Pf = (0, function(a) {
  return ((a2) => {
    var b, c = void 0;
    X(a2) && a2 != null && (b = a2.input, c = a2.output), Array.isArray(b) && (b = Ec(b, void 0, void 0)), b === void 0 && (b = new La({ type: "unknown" }), a2 = aa("array", void 0), a2.element = b, b = new za(a2)), c === void 0 && (c = new La({ type: "unknown" }));
    return new Va({ type: "function", input: b, output: c });
  })(a);
});
var Qf = (0, function(a) {
  let b = a;
  b = new ga(aa("number", b));
  return fa(b, da("number_format", { format: "int" }));
});
var Rf = (0, function(a) {
  let b = a;
  b = new ga(aa("number", b));
  return fa(b, da("number_format", { format: "int32" }));
});
var Sf = (0, function(a) {
  let b = a;
  b = new ga(aa("number", b));
  return fa(b, da("number_format", { format: "uint32" }));
});
var Tf = (0, function(a) {
  let b = a;
  b = new ga(aa("number", b));
  return fa(b, da("number_format", { format: "float32" }));
});
var Uf = (0, function(a) {
  let b = a;
  b = new ga(aa("number", b));
  return fa(b, da("number_format", { format: "float64" }));
});
var Vf = (0, function(a, b) {
  return new Ha({ type: "pipe", in: a, out: b });
});
var Wf = (0, function(a) {
  return a.optional();
});
var Xf = (0, function(a) {
  return a.nullable();
});
var Yf = Fa(h, "email", Jc);
var Zf = Fa(v, "uuid", kb);
var _f = Fa(u, "guid", Kc);
var $f = (0, function(a) {
  var b = aa("string", a);
  b.format = "url", b.check = "string_format", b.abort = false, b = new Vc(b);
  return b;
});
var ag = (0, function(a) {
  var b = aa("string", a);
  b.format = "url", b.check = "string_format", b.abort = false, b.protocol = te, b.hostname = ue, b = new Vc(b);
  return b;
});
var bg = (0, function(a, b, c) {
  return Md(a, b, c);
});
var cg = (0, function(a) {
  var b = a, c = ca(b);
  b = c.def.out;
  var f = c.def.in, h2 = c.def.reverseTransform;
  b = Md(b, f, { decode: h2, encode: c.def.transform });
  return b;
});
var dg = (0, function(a, b) {
  let c = a;
  c = new pb({ type: "transform", transform: c });
  return new Mb({ type: "pipe", in: c, out: b });
});
var eg = (0, function(a) {
  return ((a2) => {
    var r2 = Sa(a2), b = r2.truthy, c = r2.falsy;
    (b === void 0 || !Array.isArray(b)) && (b = ["true", "1", "yes", "on", "y", "enabled"]), (c === void 0 || !Array.isArray(c)) && (c = ["false", "0", "no", "off", "n", "disabled"]);
    var d = false;
    "string" == typeof r2.case && "sensitive" == r2.case + "" && (d = true);
    if (!d) {
      var f = [], e2 = b.length;
      a2 = 0;
      while (a2 < e2) {
        if ("string" == typeof b[a2]) f.push(b[a2].toLowerCase());
        else {
          var h2 = b[a2];
          f.push(h2);
        }
        a2++;
      }
      for (b = f, f = [], e2 = c.length, a2 = 0; a2 < e2; a2++) "string" == typeof c[a2] ? f.push(c[a2].toLowerCase()) : (h2 = c[a2], f.push(h2));
      c = f;
    }
    for (f = ja(), e2 = ja(), h2 = b.length, a2 = 0; a2 < h2; a2++) {
      var g2 = b[a2];
      f.add(g2);
    }
    for (h2 = c.length, a2 = 0; a2 < h2; a2++) g2 = c[a2], e2.add(g2);
    a2 = Z;
    var i = new a2({ type: "string", error: r2.error });
    a2 = cb;
    var n2, s2 = new a2({ type: "boolean", error: r2.error });
    a2 = function(a3, h3) {
      var r3 = a3 + "";
      d || (r3 = r3.toLowerCase());
      if (f.has(r3)) return true;
      if (e2.has(r3)) return false;
      for (var g3, m2, s3 = [], l2 = b.length, i2 = 0; i2 < l2; i2++) r3 = b[i2], s3.push(r3);
      for (i2 = c.length, g3 = 0; g3 < i2; g3++) l2 = c[g3], s3.push(l2);
      g3 = h3.issues, m2 = h3.value, g3.push({ code: "invalid_value", expected: "stringbool", values: s3, input: m2, inst: n2 });
      return {};
    }, h2 = function(a3, f2) {
      return true === a3 ? b[0] : c[0];
    }, n2 = new qb({ type: "pipe", in: i, out: s2, transform: a2, reverseTransform: h2, error: r2.error });
    return n2;
  })(a);
});
var fg = (0, function(a) {
  return new pc({ type: "success", innerType: a });
});
var gg = (0, function(a) {
  return ((a2) => {
    var b;
    b = new ob({ type: "lazy", getter: function() {
      let f = [], c = new Z(aa("string", a2));
      f.push(c), f.push(new ga(aa("number", void 0))), f.push(new cb(aa("boolean", void 0))), f.push(new Db({ type: "null" }));
      let h2 = b;
      c = aa("array", void 0), c.element = h2, f.push(new za(c)), c = new Z(aa("string", void 0)), f.push(bc(c, b, void 0)), c = aa("union", void 0), c.options = f;
      return new Na(c);
    } });
    return b;
  })(a);
});
var hg = (0, function(a) {
  return Ta(void 0, "hex", /^[0-9a-fA-F]*$/, a);
});
var ig = (0, function(a) {
  return Ta(void 0, "hostname", /^(?=.{1,253}\.?$)[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[-0-9a-zA-Z]{0,61}[0-9a-zA-Z])?)*\.?$/, a);
});
var jg = (0, function(a, b) {
  return ((a2, b2) => {
    var f = X(b2) && "string" == typeof b2.enc ? b2.enc + "" : "hex", h2 = a2 + "_" + f, c = a2 + "";
    a2 = Rc, "hex" == f && ("md5" == c && (a2 = ve), "sha1" == c && (a2 = we), "sha256" == c && (a2 = Rc), "sha384" == c && (a2 = xe), "sha512" == c && (a2 = ye)), "base64" == f && ("md5" == c && (a2 = /^[A-Za-z0-9+\/]{22}==$/), "sha1" == c && (a2 = /^[A-Za-z0-9+\/]{27}=$/), "sha256" == c && (a2 = /^[A-Za-z0-9+\/]{43}=$/), "sha384" == c && (a2 = /^[A-Za-z0-9+\/]{64}$/), "sha512" == c && (a2 = /^[A-Za-z0-9+\/]{86}==$/)), "base64url" == f && ("md5" == c && (a2 = /^[A-Za-z0-9_-]{22}$/), "sha1" == c && (a2 = /^[A-Za-z0-9_-]{27}$/), "sha256" == c && (a2 = /^[A-Za-z0-9_-]{43}$/), "sha384" == c && (a2 = /^[A-Za-z0-9_-]{64}$/), "sha512" == c && (a2 = /^[A-Za-z0-9_-]{86}$/));
    return Ta(void 0, h2, a2, b2);
  })(a, b);
});
var kg = (0, function(a, b, c) {
  let f = a;
  f = bc(f, b, c), ca(f).def.partial = true;
  return f;
});
var lg = (0, function(a) {
  return a.keyof();
});
var mg = (0, function(a, b) {
  return a.catch.call(a, b);
});
u = (0, function(a, b) {
  return a.default.call(a, b);
});
var ng = (0, function(a, b) {
  return a.prefault(b);
});
var og = (0, function(a) {
  return a.nonoptional();
});
var pg = (0, function(a) {
  return a.readonly();
});
z = Fa(z, "jwt", g);
var qg = Fa(void 0, "nanoid", Mc);
var rg = Fa(void 0, "ulid", Lc);
var sg = Fa(void 0, "ipv4", Nc);
var tg = Fa(void 0, "ipv6", Oc);
g = { string: function(a) {
  let b = a;
  b = new Z(aa("string", b)), Bb(b);
  return b;
}, number: function(a) {
  let b = a;
  b = new ga(aa("number", b)), Bb(b);
  return b;
}, boolean: function(a) {
  let b = a;
  b = new cb(aa("boolean", b)), Bb(b);
  return b;
}, bigint: function(a) {
  let b = a;
  b = new na(aa("bigint", b)), Bb(b);
  return b;
}, date: function(a) {
  let b = a;
  b = new Da(aa("date", b)), Bb(b);
  return b;
} };
var _k = Fa(j, "date", e);
h = { datetime: function(a) {
  return Ta(Tc, "datetime", Jd(a), a);
}, date: _k, time: function(a) {
  return Ta(Uc, "time", Id(a), a);
}, duration: Fa(k, "duration", Pc) }, v = { en: function() {
  return { localeError: jd() };
} }, e = {};
var sb = {};
W("string", B), W("number", C), W("boolean", D), W("bigint", E), W("symbol", F), W("date", G), W("nan", H), W("undefined", I), W("null", J), W("any", K), W("unknown", L), W("never", M), W("void", N), W("literal", O), W("enum", s), W("nativeEnum", s), W("object", t), W("strictObject", P), W("looseObject", Q), W("interface", t), W("array", R), W("tuple", S), W("record", T), W("union", U), W("xor", V), W("discriminatedUnion", Ff), W("intersection", Gf), W("map", Hf), W("set", If), W("lazy", Jf), W("promise", Kf), W("custom", Lf), W("file", Mf), W("instanceof", Nf), W("transform", Of), W("function", Pf), W("int", Qf), W("int32", Rf), W("uint32", Sf), W("float32", Tf), W("float64", Uf), W("pipe", Vf), W("optional", Wf), W("nullable", Xf), W("email", Yf), W("uuid", Zf), W("guid", _f), W("url", $f), W("httpUrl", ag), W("codec", bg), W("invertCodec", cg), W("preprocess", dg), W("stringbool", eg), W("success", fg), W("json", gg), W("hex", hg), W("hostname", ig), W("hash", jg), W("partialRecord", kg), W("looseRecord", function(a, b, c) {
  let f = a;
  f = bc(f, b, c), ca(f).def.mode = "loose";
  return f;
}), W("creditCard", function(a) {
  return Ta(void 0, "credit_card", Qc, a);
}), W("mac", function(a) {
  var b = X(a) && "string" == typeof a.delimiter ? a.delimiter : ":";
  b += "", b = Ta(void 0, "mac", ma(Ef + b + xf + b + Df), a);
  return b;
}), W("keyof", lg), W("catch", mg), W("default", u), W("_default", u), W("prefault", ng), W("nonoptional", og), W("readonly", pg), W("jwt", z), W("nanoid", qg), W("ulid", rg), W("ipv4", sg), W("ipv6", tg), W("coerce", g), W("iso", h), W("locales", v), W("core", e), W("parse", Od), W("safeParse", (a, b, c) => $b(a, b, c)), W("parseAsync", Pd), W("safeParseAsync", (a, b, c) => ac(a, b, c)), W("encode", Qd), W("decode", Rd), W("encodeAsync", Sd), W("decodeAsync", Td), W("treeifyError", l), W("prettifyError", m), W("formatError", n), W("flattenError", o), W("registry", () => {
  let a = /* @__PURE__ */ new WeakMap();
  return kd(a, /* @__PURE__ */ new Map());
}), W("globalRegistry", A), W("config", Nd), W("ZodType", Y), W("ZodString", Z), W("ZodNumber", ga), W("ZodBoolean", cb), W("ZodBigInt", na), W("ZodSymbol", gc), W("ZodDate", Da), W("ZodNaN", hc), W("ZodUndefined", ic), W("ZodNull", Db), W("ZodAny", jc), W("ZodUnknown", La), W("ZodNever", lb), W("ZodVoid", kc), W("ZodLiteral", Eb), W("ZodEnum", Ma), W("ZodObject", ka), W("ZodArray", za), W("ZodTuple", lc), W("ZodRecord", Fb), W("ZodMap", mb), W("ZodSet", nb), W("ZodUnion", Na), W("ZodDiscriminatedUnion", mc), W("ZodXor", Gb), W("ZodIntersection", Hb), W("ZodOptional", db), W("ZodNullable", nc), W("ZodDefault", Ib), W("ZodPrefault", Jb), W("ZodCatch", Kb), W("ZodNonOptional", oc), W("ZodLazy", ob), W("ZodPromise", Lb), W("ZodTransform", pb), W("ZodPipe", Ha), W("ZodCodec", qb), W("ZodPreprocess", Mb), W("ZodReadonly", Sc), W("ZodCustom", Ua), W("ZodFile", rb), W("ZodFunction", Va), W("ZodTemplateLiteral", qc), W("ZodSuccess", pc), W("ZodError", ua), W("ZodRealError", Ka), W("getDiscriminatedOption", r), W("ZodExactOptional", eb), W("exactOptional", function(a) {
  return new eb({ type: "optional", innerType: a, exact: true });
}), W("slugify", function() {
  return (() => da("overwrite", { transform: function(a) {
    return a.toLowerCase().trim().replace(/[^\w\s-]/g, Af).replace(/[\s_-]+/g, "-").replace(/^-+|-+$/g, Af);
  } }))();
}), W("properties", function(a) {
  return ((a2) => {
    var c = [];
    if (!X(a2) || a2 == null) return c;
    for (var h2, f = oa(a2), r2 = f.length, b = 0; b < r2; b++) h2 = f[b], c.push(da("property", { property: h2, schema: a2[f[b]] }));
    return c;
  })(a);
}), W("property", function(a, b, c) {
  let f = a;
  f = { property: f, schema: b }, ha(f, Sa(c));
  return da("property", f);
}), W("templateLiteral", function(a, b) {
  var f = b, c = aa("template_literal", f);
  c.parts = a, c = new qc(c), f = c._zod, ca(c), ba(f, "pattern", { value: ((a2) => {
    var b2, c2, r2, f2 = [], e2 = a2.length, h2 = 0;
    while (h2 < e2) {
      b2 = a2[h2];
      if (b2 == null || "string" == typeof b2 || "number" == typeof b2 || "boolean" == typeof b2 || "bigint" == typeof b2) f2.push(Pb(b2 + ""));
      else if (X(b2) && X(b2._zod)) {
        r2 = b2._zod, c2 = r2.pattern;
        if (c2 == null) {
          c2 = b2._zod, a2 = c2.traits, a2 !== void 0 ? (b2 = Array.from(a2), a2 = b2.length > 0 ? b2[0] + "" : Af) : a2 = Af;
          throw new Error("Invalid template literal part, no pattern found: " + a2);
        }
        b2 = c2.source + "";
        if (0 == b2.length) throw new Error("Invalid template literal part");
        f2.push(Qb(b2));
      } else {
        throw new Error("Invalid template literal part: " + b2);
      }
      h2++;
    }
    return ma("^" + f2.join(Af) + "$");
  })(a), writable: true, configurable: true, enumerable: true });
  return c;
}), W("stringFormat", function(a, b, c) {
  return ((a2, b2, c2) => {
    c2 = aa("string", c2), c2.check = "string_format", c2.format = a2, "function" == typeof b2 && (c2.fn = b2), X(b2) && b2 != null && "function" == typeof b2.test && (c2.pattern = b2, c2.fn === void 0 && (c2.fn = function(a3) {
      return b2.test(a3);
    }));
    return new Z(c2);
  })(a, b, c);
}), W("check", function(a, b) {
  return Ub(a, b);
}), W("with", function(a, b) {
  return Ub(a, b);
}), W("refine", function(a, b) {
  let c = b;
  c = aa("custom", c), c.fn = a, c.check = "custom";
  return new Ua(c);
}), W("superRefine", function(a, b) {
  return Ld(a, b);
}), W("trim", function() {
  return (() => da("overwrite", { transform: function(a) {
    return a.trim();
  } }))();
}), W("maxLength", function(a, b) {
  var c = a;
  c = { maximum: c }, X(b) && b != null && ha(c, Sa(b));
  return da("max_length", c);
}), W("minLength", function(a, b) {
  var c = a;
  c = { minimum: c }, X(b) && b != null && ha(c, Sa(b));
  return da("min_length", c);
}), W("ZodISODateTime", Tc), W("ZodISODate", j), W("ZodISOTime", Uc), W("ZodISODuration", k), W("$ZodError", ua), W("NEVER", { status: "aborted" }), W("fromJSONSchema", function(a, b) {
  a, b;
  return new La({ type: "unknown" });
}), W("visit", Ud), W("ZodIssueCode", { invalid_type: "invalid_type", too_big: "too_big", too_small: "too_small", invalid_format: "invalid_format", not_multiple_of: "not_multiple_of", unrecognized_keys: "unrecognized_keys", invalid_union: "invalid_union", invalid_key: "invalid_key", invalid_element: "invalid_element", invalid_value: "invalid_value", custom: "custom" }), W("TimePrecision", { Any: null, Minute: -1, Second: 0, Millisecond: 3, Microsecond: 6 }), ha(e, sb), e.$ZodError = ua, e.$ZodRealError = Ka, e.$ZodEncodeError = Ic, e.toDotPath = id, e.flattenError = o, e.formatError = n, e.treeifyError = l, e.prettifyError = m, e.getDiscriminatedOption = r, e.$ZodType = Y, e.$ZodPipe = Ha, e.$ZodCodec = qb, e.$ZodPreprocess = Mb, e.$ZodString = Z, e.$ZodCustom = Ua, e.$ZodNever = lb, e.$ZodUnknown = La, e.$ZodNumber = ga, e.$ZodString = Z, e.$ZodType = Y, e.$ZodObject = ka, e.$ZodOptional = db, e.visit = Ud, e.$ZodObject = ka, e.$ZodOptional = db, e.$ZodExactOptional = eb, e.config = Nd, e.globalConfig = Tb(), e.util = (() => {
  let b = {};
  ba(b, "value", { enumerable: true, configurable: true, get: function() {
    var a2 = Tb();
    if (X(a2) && a2.jitless) return false;
    try {
      new globalThis.Function(Af);
      return true;
    } catch {
      return false;
    }
  } });
  let a = { allowsEval: b, base64ToUint8Array: function(a2) {
    a2 = globalThis.atob(a2);
    for (var c = new globalThis.Uint8Array(a2.length), f = a2.length, b2 = 0; b2 < f; b2++) c[b2] = a2.charCodeAt(b2);
    return c;
  }, uint8ArrayToBase64: function(a2) {
    for (var f = a2.length, b2 = Af, c = 0; c < f; c++) b2 = b2 + globalThis.String.fromCharCode(a2[c]) + "";
    return globalThis.btoa(b2);
  } };
  a.base64urlToUint8Array = function(b2) {
    var c = b2.replace(/-/g, "+").replace(/_/g, "/");
    b2 = c.length % 4;
    if (0 != b2) {
      var f = 4 - b2;
      for (b2 = 0; b2 < f; b2++) c += "=";
    }
    return a.base64ToUint8Array(c);
  }, a.uint8ArrayToBase64url = function(b2) {
    return a.uint8ArrayToBase64(b2).replace(/\+/g, "-").replace(/\//g, "_").replace(/=/g, Af);
  }, a.hexToUint8Array = function(a2) {
    var c = a2.replace(/^0x/, Af), b2 = globalThis.Uint8Array;
    a2 = c.length / 2 | 0;
    var f = new b2(a2);
    for (b2 = 0; b2 < a2; b2++) {
      var h2 = b2 * 2 | 0;
      f[b2] = Number.parseInt(c.slice(h2, h2 + 2 | 0), 16);
    }
    return f;
  }, a.uint8ArrayToHex = function(a2) {
    for (var b2, f = [], h2 = a2.length, c = 0; c < h2; c++) b2 = a2[c].toString(16) + "", 1 == b2.length && (b2 = "0" + b2), f.push(b2);
    return f.join(Af);
  };
  return a;
})(), sb.util = e.util;
var of = sb;

// dist/compat.js
var import_node_module = require("node:module");
var import_meta = {};
var require2 = (0, import_node_module.createRequire)(import_meta.url);
function applyCompat(z2) {
  const util2 = {
    isPlainObject(o2) {
      if (o2 === null || typeof o2 !== "object") return false;
      const ctor = o2.constructor;
      if (ctor === void 0) return true;
      if (typeof ctor !== "function") return true;
      const prot = ctor.prototype;
      if (prot === null || typeof prot !== "object") return false;
      return Object.prototype.hasOwnProperty.call(prot, "isPrototypeOf");
    },
    shallowClone(o2) {
      if (util2.isPlainObject(o2)) return { ...o2 };
      if (Array.isArray(o2)) return [...o2];
      if (o2 instanceof Map) return new Map(o2);
      if (o2 instanceof Set) return new Set(o2);
      return o2;
    },
    floatSafeRemainder(val, step) {
      const valDec = (val.toString().split(".")[1] || "").length;
      const stepDec = (step.toString().split(".")[1] || "").length;
      const mul = 10 ** Math.max(valDec, stepDec);
      return Math.round(val * mul) % Math.round(step * mul) / mul;
    },
    jsonStringifyReplacer(_2, value) {
      return typeof value === "bigint" ? value.toString() : value;
    },
    nullish(input) {
      return input === null || input === void 0;
    },
    prefixIssues(path, issues) {
      return issues.map((iss) => ({ ...iss, path: [...path, ...iss.path ?? []] }));
    },
    issue(arg, input, inst) {
      if (typeof arg === "string") {
        return { code: "custom", message: arg, input, inst, path: [] };
      }
      return arg;
    },
    cleanEnum(obj) {
      return Object.entries(obj).filter(([k2]) => Number.isNaN(Number.parseInt(k2, 10))).map((el) => el[1]);
    },
    getEnumValues(entries) {
      const numericValues = Object.values(entries).filter((v2) => typeof v2 === "number");
      return Object.entries(entries).filter(([k2]) => numericValues.indexOf(+k2) === -1).map(([, v2]) => v2);
    },
    joinValues(array2, separator = "|") {
      return array2.map((val) => typeof val === "string" ? `"${val}"` : String(val)).join(separator);
    },
    cached(getter) {
      return {
        get value() {
          const value = getter();
          Object.defineProperty(this, "value", { value });
          return value;
        }
      };
    },
    assertNever() {
      throw new Error("Unexpected value in exhaustive check");
    },
    assert() {
    },
    assertIs() {
    },
    assertEqual(val) {
      return val;
    },
    toZod() {
      return (schema) => schema;
    }
  };
  z2.core = z2.core ?? {};
  z2.core.util = Object.assign(z2.core.util ?? {}, util2);
  z2.core.$ZodError = z2.ZodError;
  z2.core.config = z2.config;
  z2.core.globalConfig = z2.core.globalConfig ?? {};
  z2.core.globalRegistry = z2.globalRegistry;
  z2.core.registry = z2.registry;
  z2.core.parse = z2.parse;
  z2.core.safeParse = z2.safeParse;
  z2.core.clone = (inst) => inst.clone();
  z2.core.$constructor = (name, init) => {
    const ctor = function(def) {
      const inst = Object.create(ctor.prototype);
      init(inst, def ?? {});
      return inst;
    };
    Object.defineProperty(ctor, "name", { value: name });
    return ctor;
  };
  z2.float32 = z2.float32 ?? ((params) => z2.number(params));
  z2.float64 = z2.float64 ?? ((params) => z2.number(params));
  z2.int32 = z2.int32 ?? ((params) => z2.number(params).int());
  z2.uint32 = z2.uint32 ?? ((params) => z2.number(params).int().nonnegative());
  z2.int64 = z2.int64 ?? ((params) => z2.bigint(params));
  z2.uint64 = z2.uint64 ?? ((params) => z2.bigint(params));
  z2.describe = z2.describe ?? ((schema, desc) => schema.describe(desc));
  z2.meta = z2.meta ?? ((schema, value) => schema.meta(value));
  z2.mac = z2.mac ?? ((params) => z2.string(params).mac(params));
  z2.e164 = z2.e164 ?? ((params) => z2.string(params).e164(params));
  z2.cidrv4 = z2.cidrv4 ?? ((params) => z2.string(params).cidrv4(params));
  z2.cidrv6 = z2.cidrv6 ?? ((params) => z2.string(params).cidrv6(params));
  z2.base64 = z2.base64 ?? ((params) => z2.string(params).base64(params));
  z2.base64url = z2.base64url ?? ((params) => z2.string(params).base64url(params));
  z2.xid = z2.xid ?? ((params) => z2.string(params).xid(params));
  z2.ksuid = z2.ksuid ?? ((params) => z2.string(params).ksuid(params));
  z2.cuid = z2.cuid ?? ((params) => z2.string(params).cuid(params));
  z2.cuid2 = z2.cuid2 ?? ((params) => z2.string(params).cuid2(params));
  z2.emoji = z2.emoji ?? ((params) => z2.string(params).emoji(params));
  z2.uuidv4 = z2.uuidv4 ?? ((params) => z2.string(params).uuidv4(params));
  z2.uuidv6 = z2.uuidv6 ?? ((params) => z2.string(params).uuidv6(params));
  z2.uuidv7 = z2.uuidv7 ?? ((params) => z2.string(params).uuidv7(params));
  z2.stringFormat = z2.stringFormat ?? ((format, fnOrRegex, params) => {
    const regex = fnOrRegex instanceof RegExp ? fnOrRegex : void 0;
    const fn = typeof fnOrRegex === "function" ? fnOrRegex : void 0;
    if (regex) return z2.string(params).regex(regex);
    if (fn) return z2.string(params).refine(fn);
    return z2.string(params);
  });
  z2.creditCard = z2.creditCard ?? ((params) => {
    return z2.string(params).regex(/^\d(?:[ -]?\d){11,18}$/, params).refine((value) => {
      const digits = String(value).replace(/\D/g, "");
      let sum = 0;
      let alt = false;
      for (let i = digits.length - 1; i >= 0; i--) {
        let n2 = digits.charCodeAt(i) - 48;
        if (alt) {
          n2 *= 2;
          if (n2 > 9) n2 -= 9;
        }
        sum += n2;
        alt = !alt;
      }
      return sum % 10 === 0;
    }, params);
  });
  z2.templateLiteral = z2.templateLiteral ?? ((parts, params) => {
    const source = parts.map((part) => {
      if (typeof part === "string") return part.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
      if (typeof part === "number" || typeof part === "bigint" || typeof part === "boolean") return String(part);
      const type = part?.def?.type ?? part?._zod?.def?.type;
      if (type === "string") return ".*";
      if (type === "number") return "-?\\d+(?:\\.\\d+)?";
      if (type === "bigint") return "-?\\d+";
      if (type === "boolean") return "true|false";
      if (type === "null") return "null";
      if (type === "undefined") return "undefined";
      if (type === "literal") {
        const values = part.def?.values ?? part._zod?.def?.values ?? [];
        return values.map((v2) => String(v2).replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|");
      }
      if (type === "enum") {
        const values = part.def?.values ?? part.options ?? [];
        return values.map((v2) => String(v2).replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|");
      }
      return ".*";
    }).join("");
    return z2.string(params).regex(new RegExp(`^${source}$`));
  });
  z2.deepPartial = z2.deepPartial ?? ((schema) => deepPartial(z2, schema));
  z2.fromJSONSchema = (json2, params) => fromJson(z2, json2, params);
  z2.safeEncode = z2.safeEncode ?? ((schema, data, ctx) => schema.safeParse(data, { ...ctx ?? {}, direction: "backward" }));
  z2.encode = z2.encode ?? ((schema, data, ctx) => schema.encode(data, ctx));
  z2.decode = z2.decode ?? ((schema, data, ctx) => schema.decode(data, ctx));
  z2.safeDecode = z2.safeDecode ?? ((schema, data, ctx) => schema.safeParse(data, ctx));
  z2.encodeAsync = z2.encodeAsync ?? ((schema, data, ctx) => schema.encodeAsync(data, ctx));
  z2.decodeAsync = z2.decodeAsync ?? ((schema, data, ctx) => schema.decodeAsync(data, ctx));
  z2.safeEncodeAsync = z2.safeEncodeAsync ?? ((schema, data, ctx) => schema.safeParse(data, { ...ctx ?? {}, direction: "backward" }).then ? schema.safeParseAsync(data, { ...ctx ?? {}, direction: "backward" }) : Promise.resolve(schema.safeParse(data, { ...ctx ?? {}, direction: "backward" })));
  z2.safeDecodeAsync = z2.safeDecodeAsync ?? ((schema, data, ctx) => schema.safeParseAsync(data, ctx));
  z2.setErrorMap = (fn) => z2.config({ customError: fn });
  z2.compile = z2.compile ?? ((schema) => schema);
  z2.core.clone = (inst, def, params) => {
    const Ctor = inst._zod?.constr ?? inst.constructor;
    const cl = new Ctor(def ?? inst._zod?.def ?? inst.def);
    if (params?.parent) cl._zod.parent = inst;
    return cl;
  };
  if (z2.ZodTuple?.prototype && !z2.ZodTuple.prototype.rest) {
    Object.defineProperty(z2.ZodTuple.prototype, "rest", {
      configurable: true,
      value(rest) {
        const items = this.def?.items ?? this._zod?.def?.items;
        return z2.tuple(items ?? [], rest);
      }
    });
  }
  try {
    const locales2 = require2("zod/v4/locales");
    z2.locales = { ...z2.locales, ...locales2.default ?? locales2 };
  } catch {
  }
  return z2;
}
function deepPartial(z2, schema, seen = /* @__PURE__ */ new Map()) {
  if (seen.has(schema)) return seen.get(schema);
  const type = schema?.def?.type ?? schema?.type;
  if (type === "object") {
    const next = schema.partial();
    seen.set(schema, next);
    return next;
  }
  if (type === "array") return z2.array(deepPartial(z2, schema.element ?? schema.def?.element, seen));
  if (type === "optional") return z2.optional(deepPartial(z2, schema.unwrap(), seen));
  if (type === "nullable") return z2.nullable(deepPartial(z2, schema.unwrap(), seen));
  if (type === "union") return z2.union((schema.options ?? schema.def?.options ?? []).map((opt) => deepPartial(z2, opt, seen)));
  if (type === "lazy") return z2.lazy(() => deepPartial(z2, schema.unwrap(), seen));
  return schema;
}
function fromJson(z2, json2) {
  if (!json2 || typeof json2 !== "object") return z2.unknown();
  if (json2.const !== void 0) return z2.literal(json2.const);
  if (json2.enum) return z2.enum(json2.enum.map(String));
  if (json2.anyOf) return z2.union(json2.anyOf.map((item) => fromJson(z2, item)));
  if (json2.allOf) return json2.allOf.map((item) => fromJson(z2, item)).reduce((a, b) => a.and(b));
  if (json2.type === "string") {
    let s2 = z2.string();
    if (json2.format === "credit_card") s2 = s2.check(z2.creditCard());
    else if (json2.format && typeof z2[json2.format] === "function") s2 = s2.check(z2[json2.format]());
    if (typeof json2.minLength === "number") s2 = s2.min(json2.minLength);
    if (typeof json2.maxLength === "number") s2 = s2.max(json2.maxLength);
    if (json2.pattern) s2 = s2.regex(new RegExp(json2.pattern));
    return s2;
  }
  if (json2.type === "number" || json2.type === "integer") return z2.number();
  if (json2.type === "boolean") return z2.boolean();
  if (json2.type === "null") return z2.null();
  if (json2.type === "array") return z2.array(fromJson(z2, json2.items ?? {}));
  if (json2.type === "object") {
    const shape = {};
    for (const [key, value] of Object.entries(json2.properties ?? {})) {
      const field = fromJson(z2, value);
      shape[key] = json2.required?.includes(key) ? field : field.optional();
    }
    return z2.object(shape);
  }
  return z2.unknown();
}

// dist/visit.js
var RESOLVING = /* @__PURE__ */ Symbol("z.visit/resolving");
function visit(z2, schema, fnOrHandlers) {
  const fn = typeof fnOrHandlers === "function" ? fnOrHandlers : (node) => {
    const h2 = fnOrHandlers[node._zod.def.type];
    return h2 ? h2(node) : node;
  };
  const cache = /* @__PURE__ */ new Map();
  function clone(inst, def) {
    const Ctor = inst._zod?.constr ?? inst.constructor;
    return new Ctor(def);
  }
  function run(s2) {
    const cached = cache.get(s2);
    if (cached === RESOLVING) {
      return new z2.ZodLazy({
        type: "lazy",
        getter: () => cache.get(s2)
      });
    }
    if (cached !== void 0) return cached;
    cache.set(s2, RESOLVING);
    const mapped = fn(mapInner(s2));
    cache.set(s2, mapped);
    return mapped;
  }
  function mapInner(s2) {
    const def = s2._zod.def;
    const kind = def.type;
    if (kind === "object") {
      const oldShape = def.shape ?? {};
      let changed = false;
      const newShape = {};
      for (const k2 of Object.keys(oldShape)) {
        const mapped = run(oldShape[k2]);
        if (mapped !== oldShape[k2]) changed = true;
        newShape[k2] = mapped;
      }
      let newCatchall = def.catchall;
      if (def.catchall) {
        newCatchall = run(def.catchall);
        if (newCatchall !== def.catchall) changed = true;
      }
      return changed ? clone(s2, { ...def, shape: newShape, catchall: newCatchall }) : s2;
    }
    if (kind === "array") {
      const mapped = run(def.element);
      return mapped === def.element ? s2 : clone(s2, { ...def, element: mapped });
    }
    if (kind === "tuple") {
      const oldItems = def.items ?? [];
      let changed = false;
      const newItems = [];
      for (const item of oldItems) {
        const mapped = run(item);
        if (mapped !== item) changed = true;
        newItems.push(mapped);
      }
      let newRest = def.rest;
      if (def.rest) {
        newRest = run(def.rest);
        if (newRest !== def.rest) changed = true;
      }
      return changed ? clone(s2, { ...def, items: newItems, rest: newRest }) : s2;
    }
    if (kind === "record" || kind === "map") {
      const newKey = run(def.keyType);
      const newVal = run(def.valueType);
      return newKey === def.keyType && newVal === def.valueType ? s2 : clone(s2, { ...def, keyType: newKey, valueType: newVal });
    }
    if (kind === "set") {
      const newVal = run(def.valueType);
      return newVal === def.valueType ? s2 : clone(s2, { ...def, valueType: newVal });
    }
    if (kind === "union") {
      const oldOptions = def.options ?? [];
      let changed = false;
      const newOptions = [];
      for (const opt of oldOptions) {
        const mapped = run(opt);
        if (mapped !== opt) changed = true;
        newOptions.push(mapped);
      }
      return changed ? clone(s2, { ...def, options: newOptions }) : s2;
    }
    if (kind === "intersection") {
      const newLeft = run(def.left);
      const newRight = run(def.right);
      return newLeft === def.left && newRight === def.right ? s2 : clone(s2, { ...def, left: newLeft, right: newRight });
    }
    if (kind === "optional" || kind === "nullable" || kind === "default" || kind === "prefault" || kind === "catch" || kind === "readonly" || kind === "nonoptional" || kind === "promise" || kind === "success") {
      const newInner = run(def.innerType);
      return newInner === def.innerType ? s2 : clone(s2, { ...def, innerType: newInner });
    }
    if (kind === "pipe") {
      const newIn = run(def.in);
      const newOut = run(def.out);
      return newIn === def.in && newOut === def.out ? s2 : clone(s2, { ...def, in: newIn, out: newOut });
    }
    if (kind === "function") {
      const newInput = run(def.input);
      const newOutput = run(def.output);
      return newInput === def.input && newOutput === def.output ? s2 : clone(s2, { ...def, input: newInput, output: newOutput });
    }
    if (kind === "lazy") {
      const original = def.getter;
      const rest = { ...def };
      delete rest._cachedInner;
      return clone(s2, { ...rest, getter: () => run(original()) });
    }
    return s2;
  }
  return run(schema);
}
function installVisit(z2) {
  z2.core = z2.core ?? {};
  z2.core.visit = (schema, fnOrHandlers) => visit(z2, schema, fnOrHandlers);
  z2.visit = z2.core.visit;
  z2.deepPartial = (schema) => visit(z2, schema, {
    object: (s2) => s2.partial(),
    union: (s2) => {
      const def = s2._zod.def;
      return def.discriminator === void 0 ? s2 : z2.union(def.options);
    }
  });
}

// dist/async-api.js
function restack(err, callee) {
  try {
    Error.captureStackTrace(err, callee);
  } catch {
  }
  const lines = String(err.stack ?? "").split("\n");
  const header = [];
  const frames = [];
  for (const line of lines) {
    if (line.trim().startsWith("at ")) {
      if (line.includes("processTicksAndRejections")) continue;
      if (line.includes("internal/process")) continue;
      if (line.includes("async-api")) continue;
      frames.push(line);
    } else {
      header.push(line);
    }
  }
  if (!header.length) header.push(`${err.name}: ${err.message}`);
  if (frames.length) err.stack = [...header, ...frames].join("\n");
}
function throwing2(runSafe) {
  let fn;
  fn = function(data, params) {
    const result = runSafe.call(this, data, params);
    if (result?.success) return result.data;
    if (result?.error) {
      restack(result.error, fn);
      throw result.error;
    }
    return result;
  };
  return fn;
}
function throwing3(runSafe) {
  let fn;
  fn = function(schema, data, ctx) {
    const result = runSafe(schema, data, ctx);
    if (result?.success) return result.data;
    if (result?.error) {
      restack(result.error, fn);
      throw result.error;
    }
    return result;
  };
  return fn;
}
function asyncThrowing2(runSafe) {
  let fn;
  fn = async function(data, params) {
    const result = await runSafe.call(this, data, params);
    if (result?.success) return result.data;
    if (result?.error) {
      restack(result.error, fn);
      throw result.error;
    }
    return result;
  };
  return fn;
}
function asyncThrowing3(runSafe) {
  let fn;
  fn = async function(schema, data, ctx) {
    const result = await runSafe(schema, data, ctx);
    if (result?.success) return result.data;
    if (result?.error) {
      restack(result.error, fn);
      throw result.error;
    }
    return result;
  };
  return fn;
}
function installOwn(proto, key, make) {
  Object.defineProperty(proto, key, {
    configurable: true,
    enumerable: true,
    get() {
      const self = this;
      const fn = make(self);
      Object.defineProperty(self, key, {
        configurable: true,
        writable: true,
        enumerable: true,
        value: fn
      });
      return fn;
    },
    set(value) {
      Object.defineProperty(this, key, { configurable: true, writable: true, enumerable: true, value });
    }
  });
}
function installAsyncApi(target) {
  target.parse = throwing3((schema, data, ctx) => target.safeParse(schema, data, ctx));
  target.encode = throwing3(
    (schema, data, ctx) => target.safeParse(schema, data, { ...ctx ?? {}, direction: "backward" })
  );
  target.decode = throwing3(
    (schema, data, ctx) => target.safeParse(schema, data, { ...ctx ?? {}, direction: "forward" })
  );
  target.parseAsync = asyncThrowing3((schema, data, ctx) => target.safeParseAsync(schema, data, ctx));
  target.encodeAsync = asyncThrowing3(
    (schema, data, ctx) => target.safeParseAsync(schema, data, { ...ctx ?? {}, direction: "backward" })
  );
  target.decodeAsync = asyncThrowing3(
    (schema, data, ctx) => target.safeParseAsync(schema, data, { ...ctx ?? {}, direction: "forward" })
  );
  const proto = target.ZodType?.prototype;
  if (!proto) return;
  installOwn(proto, "parse", (self) => throwing2((data, params) => self.safeParse(data, params)));
  installOwn(
    proto,
    "encode",
    (self) => throwing2((data, params) => self.safeParse(data, { ...params ?? {}, direction: "backward" }))
  );
  installOwn(
    proto,
    "decode",
    (self) => throwing2((data, params) => self.safeParse(data, { ...params ?? {}, direction: "forward" }))
  );
  installOwn(proto, "parseAsync", (self) => asyncThrowing2((data, params) => self.safeParseAsync(data, params)));
  installOwn(
    proto,
    "encodeAsync",
    (self) => asyncThrowing2((data, params) => self.safeParseAsync(data, { ...params ?? {}, direction: "backward" }))
  );
  installOwn(
    proto,
    "decodeAsync",
    (self) => asyncThrowing2((data, params) => self.safeParseAsync(data, { ...params ?? {}, direction: "forward" }))
  );
}

// dist/official-json-schema.js
function assignProp(target, prop, value) {
  Object.defineProperty(target, prop, {
    value,
    writable: true,
    enumerable: true,
    configurable: true
  });
}
function getEnumValues(entries) {
  if (Array.isArray(entries)) return [...entries];
  const numericValues = Object.values(entries).filter((v2) => typeof v2 === "number");
  return Object.entries(entries).filter(([k2]) => numericValues.indexOf(+k2) === -1).map(([, v2]) => v2);
}
var globalRegistry = {
  get(schema) {
    return globalThis.__zod_globalRegistry?.get(schema);
  },
  has(schema) {
    return globalThis.__zod_globalRegistry?.has(schema) ?? false;
  },
  add(schema, meta2) {
    return globalThis.__zod_globalRegistry?.add(schema, meta2);
  },
  get _idmap() {
    return globalThis.__zod_globalRegistry?._idmap;
  }
};
function assignProps(target, ...sources) {
  for (const source of sources) {
    for (const key of Reflect.ownKeys(source)) {
      if (Object.prototype.propertyIsEnumerable.call(source, key)) {
        assignProp(target, key, source[key]);
      }
    }
  }
  return target;
}
function initializeContext(params) {
  let target = params?.target ?? "draft-2020-12";
  if (target === "draft-4") target = "draft-04";
  if (target === "draft-7") target = "draft-07";
  return {
    processors: params.processors ?? {},
    metadataRegistry: params?.metadata ?? globalRegistry,
    target,
    unrepresentable: params?.unrepresentable ?? "throw",
    override: params?.override ?? (() => {
    }),
    io: params?.io ?? "output",
    counter: 0,
    seen: /* @__PURE__ */ new Map(),
    sharedDefsExtractedFor: void 0,
    sharedEmitDoneFor: void 0,
    cycles: params?.cycles ?? "ref",
    reused: params?.reused ?? "inline",
    external: params?.external ?? void 0
  };
}
function handleUnrepresentable(schema, ctx, json2, params, message) {
  const result = typeof ctx.unrepresentable === "function" ? ctx.unrepresentable({ zodSchema: schema, path: params.path, message }) : ctx.unrepresentable;
  if (result === "any") return false;
  if (result === void 0 || result === "throw") throw new Error(message);
  Object.assign(json2, result);
  return true;
}
function process(schema, ctx, _params = { path: [], schemaPath: [] }) {
  const def = schema._zod.def;
  const seen = ctx.seen.get(schema);
  if (seen) {
    seen.count++;
    const isCycle = _params.schemaPath.includes(schema);
    if (isCycle) {
      seen.cycle = _params.path;
    }
    return seen.schema;
  }
  const result = { schema: {}, count: 1, cycle: void 0, path: _params.path };
  ctx.seen.set(schema, result);
  ctx.sharedDefsExtractedFor = void 0;
  ctx.sharedEmitDoneFor = void 0;
  const overrideSchema = schema._zod.toJSONSchema?.();
  if (overrideSchema) {
    result.schema = overrideSchema;
  } else {
    const params = {
      ..._params,
      schemaPath: [..._params.schemaPath, schema],
      path: _params.path
    };
    if (schema._zod.processJSONSchema) {
      schema._zod.processJSONSchema(ctx, result.schema, params);
    } else {
      const _json = result.schema;
      const processor = ctx.processors[def.type];
      if (!processor) {
        throw new Error(`[toJSONSchema]: Non-representable type encountered: ${def.type}`);
      }
      processor(schema, ctx, _json, params);
    }
    const parent = schema._zod.parent;
    if (parent) {
      if (!result.ref) result.ref = parent;
      process(parent, ctx, params);
      ctx.seen.get(parent).isParent = true;
    }
  }
  const meta2 = ctx.metadataRegistry.get(schema);
  if (meta2) assignProps(result.schema, meta2);
  if (ctx.io === "input" && isTransforming(schema)) {
    delete result.schema.examples;
    delete result.schema.default;
  }
  if (ctx.io === "input" && "_prefault" in result.schema) result.schema.default ??= result.schema._prefault;
  delete result.schema._prefault;
  const _result = ctx.seen.get(schema);
  return _result.schema;
}
function encodeJSONPointerSegment(segment) {
  return segment.replace(/~/g, "~0").replace(/\//g, "~1");
}
function extractDefs(ctx, schema) {
  const root = ctx.seen.get(schema);
  if (!root) throw new Error("Unprocessed schema. This is a bug in Zod.");
  if (ctx.external && ctx.sharedDefsExtractedFor === ctx.external) return;
  const idToSchema = /* @__PURE__ */ new Map();
  for (const entry of ctx.seen.entries()) {
    const id2 = ctx.metadataRegistry.get(entry[0])?.id;
    if (id2) {
      const existing = idToSchema.get(id2);
      if (existing && existing !== entry[0]) {
        throw new Error(
          `Duplicate schema id "${id2}" detected during JSON Schema conversion. Two different schemas cannot share the same id when converted together.`
        );
      }
      idToSchema.set(id2, entry[0]);
    }
  }
  const makeURI = (entry) => {
    const defsSegment = ctx.target === "draft-2020-12" ? "$defs" : "definitions";
    if (ctx.external) {
      const externalId = ctx.external.registry.get(entry[0])?.id;
      const uriGenerator = ctx.external.uri ?? ((id22) => id22);
      if (externalId) {
        return { ref: uriGenerator(externalId) };
      }
      const id2 = entry[1].defId ?? entry[1].schema.id ?? `schema${ctx.counter++}`;
      entry[1].defId = id2;
      return { defId: id2, ref: `${uriGenerator("__shared")}#/${defsSegment}/${encodeJSONPointerSegment(id2)}` };
    }
    const uriPrefix = `#`;
    const defUriPrefix = `${uriPrefix}/${defsSegment}/`;
    if (entry[1] === root && !entry[1].schema.id) {
      return { ref: uriPrefix };
    }
    const defId = entry[1].schema.id ?? `__schema${ctx.counter++}`;
    return { defId, ref: defUriPrefix + encodeJSONPointerSegment(defId) };
  };
  const extractToDef = (entry) => {
    if (entry[1].schema.$ref) {
      return;
    }
    const seen = entry[1];
    const { ref, defId } = makeURI(entry);
    seen.def = { ...seen.schema };
    if (defId) seen.defId = defId;
    const schema2 = seen.schema;
    for (const key in schema2) {
      delete schema2[key];
    }
    schema2.$ref = ref;
  };
  if (ctx.cycles === "throw") {
    for (const entry of ctx.seen.entries()) {
      const seen = entry[1];
      if (seen.cycle) {
        throw new Error(
          `Cycle detected: #/${seen.cycle?.join("/")}/<root>

Set the \`cycles\` parameter to \`"ref"\` to resolve cyclical schemas with defs.`
        );
      }
    }
  }
  for (const entry of ctx.seen.entries()) {
    const seen = entry[1];
    if (schema === entry[0]) {
      extractToDef(entry);
      continue;
    }
    if (ctx.external) {
      const ext = ctx.external.registry.get(entry[0])?.id;
      if (schema !== entry[0] && ext) {
        extractToDef(entry);
        continue;
      }
    }
    const id2 = ctx.metadataRegistry.get(entry[0])?.id;
    if (id2) {
      extractToDef(entry);
      continue;
    }
    if (seen.cycle) {
      extractToDef(entry);
      continue;
    }
    if (seen.count > 1) {
      if (ctx.reused === "ref") {
        extractToDef(entry);
        continue;
      }
    }
  }
  if (ctx.external) ctx.sharedDefsExtractedFor = ctx.external;
}
function compactTypeUnion(schema) {
  const options = schema.anyOf;
  if (!Array.isArray(options) || options.length === 0 || schema.type !== void 0) return;
  const types = [];
  for (const option of options) {
    if (!option || typeof option !== "object") return;
    compactTypeUnion(option);
    const keys = Object.keys(option);
    if (keys.length !== 1 || keys[0] !== "type") return;
    const type = option.type;
    for (const member of Array.isArray(type) ? type : [type]) {
      if (typeof member !== "string") return;
      if (!types.includes(member)) types.push(member);
    }
  }
  delete schema.anyOf;
  schema.type = types.length === 1 ? types[0] : types;
}
function finalize(ctx, schema) {
  const root = ctx.seen.get(schema);
  if (!root) throw new Error("Unprocessed schema. This is a bug in Zod.");
  const flattenRef = (zodSchema) => {
    const seen = ctx.seen.get(zodSchema);
    if (seen.ref === null) return;
    const schema2 = seen.def ?? seen.schema;
    const _cached = { ...schema2 };
    const ref = seen.ref;
    seen.ref = null;
    if (ref) {
      flattenRef(ref);
      const refSeen = ctx.seen.get(ref);
      const refSchema = refSeen.schema;
      if (refSchema.$ref && (ctx.target === "draft-07" || ctx.target === "draft-04" || ctx.target === "openapi-3.0")) {
        schema2.allOf = schema2.allOf ?? [];
        schema2.allOf.push(refSchema);
      } else {
        assignProps(schema2, refSchema);
      }
      assignProps(schema2, _cached);
      const isParentRef = zodSchema._zod.parent === ref;
      if (isParentRef) {
        for (const key in schema2) {
          if (key === "$ref" || key === "allOf") continue;
          if (!(key in _cached)) {
            delete schema2[key];
          }
        }
      }
      if (refSchema.$ref && refSeen.def) {
        for (const key in schema2) {
          if (key === "$ref" || key === "allOf") continue;
          if (key in refSeen.def && JSON.stringify(schema2[key]) === JSON.stringify(refSeen.def[key])) {
            delete schema2[key];
          }
        }
      }
    }
    const parent = zodSchema._zod.parent;
    if (parent && parent !== ref) {
      flattenRef(parent);
      const parentSeen = ctx.seen.get(parent);
      if (parentSeen?.schema.$ref) {
        schema2.$ref = parentSeen.schema.$ref;
        if (parentSeen.def) {
          for (const key in schema2) {
            if (key === "$ref" || key === "allOf") continue;
            if (key in parentSeen.def && JSON.stringify(schema2[key]) === JSON.stringify(parentSeen.def[key])) {
              delete schema2[key];
            }
          }
        }
      }
    }
    ctx.override({
      zodSchema,
      jsonSchema: schema2,
      path: seen.path ?? []
    });
  };
  if (!ctx.external || ctx.sharedEmitDoneFor !== ctx.external) {
    for (const entry of [...ctx.seen.entries()].reverse()) {
      flattenRef(entry[0]);
    }
    if (ctx.target !== "openapi-3.0") {
      for (const entry of ctx.seen.entries()) {
        compactTypeUnion(entry[1].def ?? entry[1].schema);
      }
    }
  }
  const result = {};
  if (ctx.target === "draft-2020-12") {
    result.$schema = "https://json-schema.org/draft/2020-12/schema";
  } else if (ctx.target === "draft-07") {
    result.$schema = "http://json-schema.org/draft-07/schema#";
  } else if (ctx.target === "draft-04") {
    result.$schema = "http://json-schema.org/draft-04/schema#";
  } else if (ctx.target === "openapi-3.0") {
  } else {
  }
  if (ctx.external?.uri) {
    const id2 = ctx.external.registry.get(schema)?.id;
    if (!id2) throw new Error("Schema is missing an `id` property");
    result.$id = ctx.external.uri(id2);
  }
  assignProps(result, root.defId ? root.schema : root.def ?? root.schema);
  const rootMetaId = ctx.metadataRegistry.get(schema)?.id;
  if (rootMetaId !== void 0 && result.id === rootMetaId) delete result.id;
  const defs = ctx.external?.defs ?? {};
  if (!ctx.external || ctx.sharedEmitDoneFor !== ctx.external) {
    for (const entry of ctx.seen.entries()) {
      const seen = entry[1];
      if (seen.def && seen.defId) {
        if (seen.def.id === seen.defId) delete seen.def.id;
        assignProp(defs, seen.defId, seen.def);
      }
    }
  }
  if (ctx.external) ctx.sharedEmitDoneFor = ctx.external;
  if (ctx.external) {
  } else {
    if (Object.keys(defs).length > 0) {
      if (ctx.target === "draft-2020-12") {
        result.$defs = defs;
      } else {
        result.definitions = defs;
      }
    }
  }
  try {
    const finalized = JSON.parse(JSON.stringify(result));
    Object.defineProperty(finalized, "~standard", {
      value: {
        ...schema["~standard"],
        jsonSchema: {
          input: createStandardJSONSchemaMethod(schema, "input", ctx.processors),
          output: createStandardJSONSchemaMethod(schema, "output", ctx.processors)
        }
      },
      enumerable: false,
      writable: false
    });
    return finalized;
  } catch (_err) {
    throw new Error("Error converting schema to JSON.");
  }
}
function isTransforming(_schema, _ctx) {
  const ctx = _ctx ?? { seen: /* @__PURE__ */ new Set() };
  if (ctx.seen.has(_schema)) return false;
  ctx.seen.add(_schema);
  const def = _schema._zod.def;
  if (def.type === "transform") return true;
  if (def.type === "array") return isTransforming(def.element, ctx);
  if (def.type === "set") return isTransforming(def.valueType, ctx);
  if (def.type === "lazy") return isTransforming(def.getter(), ctx);
  if (def.type === "promise" || def.type === "optional" || def.type === "nonoptional" || def.type === "nullable" || def.type === "readonly" || def.type === "default" || def.type === "prefault" || def.type === "catch") {
    return isTransforming(def.innerType, ctx);
  }
  if (def.type === "intersection") {
    return isTransforming(def.left, ctx) || isTransforming(def.right, ctx);
  }
  if (def.type === "record" || def.type === "map") {
    return isTransforming(def.keyType, ctx) || isTransforming(def.valueType, ctx);
  }
  if (def.type === "pipe") {
    if (_schema._zod.traits.has("$ZodCodec")) return true;
    return isTransforming(def.in, ctx) || isTransforming(def.out, ctx);
  }
  if (def.type === "object") {
    for (const key in def.shape) {
      if (isTransforming(def.shape[key], ctx)) return true;
    }
    return false;
  }
  if (def.type === "union") {
    for (const option of def.options) {
      if (isTransforming(option, ctx)) return true;
    }
    return false;
  }
  if (def.type === "tuple") {
    for (const item of def.items) {
      if (isTransforming(item, ctx)) return true;
    }
    if (def.rest && isTransforming(def.rest, ctx)) return true;
    return false;
  }
  return false;
}
var createStandardJSONSchemaMethod = (schema, io, processors = {}) => (params) => {
  const { libraryOptions, target } = params ?? {};
  const ctx = initializeContext({ ...libraryOptions ?? {}, target, io, processors });
  process(schema, ctx);
  extractDefs(ctx, schema);
  return finalize(ctx, schema);
};
var formatMap = {
  guid: "uuid",
  url: "uri",
  datetime: "date-time",
  json_string: "json-string",
  regex: ""
  // do not set
};
var stringProcessor = (schema, ctx, _json, _params) => {
  const json2 = _json;
  json2.type = "string";
  const { minimum, maximum, format, patterns, contentEncoding } = schema._zod.bag;
  if (typeof minimum === "number") json2.minLength = minimum;
  if (typeof maximum === "number") json2.maxLength = maximum;
  if (format) {
    json2.format = formatMap[format] ?? format;
    if (json2.format === "") delete json2.format;
    if (format === "time") {
      delete json2.format;
    }
  }
  if (contentEncoding) json2.contentEncoding = contentEncoding;
  if (patterns && patterns.size > 0) {
    const regexes = [...patterns];
    if (regexes.length === 1) json2.pattern = regexes[0].source;
    else if (regexes.length > 1) {
      json2.allOf = [
        ...regexes.map((regex) => ({
          ...ctx.target === "draft-07" || ctx.target === "draft-04" || ctx.target === "openapi-3.0" ? { type: "string" } : {},
          pattern: regex.source
        }))
      ];
    }
  }
};
var numberProcessor = (schema, ctx, _json, _params) => {
  const json2 = _json;
  const { minimum, maximum, format, multipleOf, exclusiveMaximum, exclusiveMinimum } = schema._zod.bag;
  if (typeof format === "string" && format.includes("int")) json2.type = "integer";
  else json2.type = "number";
  const exMin = typeof exclusiveMinimum === "number" && exclusiveMinimum >= (minimum ?? Number.NEGATIVE_INFINITY);
  const exMax = typeof exclusiveMaximum === "number" && exclusiveMaximum <= (maximum ?? Number.POSITIVE_INFINITY);
  const legacy = ctx.target === "draft-04" || ctx.target === "openapi-3.0";
  if (exMin) {
    if (legacy) {
      json2.minimum = exclusiveMinimum;
      json2.exclusiveMinimum = true;
    } else {
      json2.exclusiveMinimum = exclusiveMinimum;
    }
  } else if (typeof minimum === "number") {
    json2.minimum = minimum;
  }
  if (exMax) {
    if (legacy) {
      json2.maximum = exclusiveMaximum;
      json2.exclusiveMaximum = true;
    } else {
      json2.exclusiveMaximum = exclusiveMaximum;
    }
  } else if (typeof maximum === "number") {
    json2.maximum = maximum;
  }
  if (typeof multipleOf === "number") json2.multipleOf = multipleOf;
};
var booleanProcessor = (_schema, _ctx, json2, _params) => {
  json2.type = "boolean";
};
var bigintProcessor = (schema, ctx, json2, params) => {
  handleUnrepresentable(schema, ctx, json2, params, "BigInt cannot be represented in JSON Schema");
};
var symbolProcessor = (schema, ctx, json2, params) => {
  handleUnrepresentable(schema, ctx, json2, params, "Symbols cannot be represented in JSON Schema");
};
var nullProcessor = (_schema, ctx, json2, _params) => {
  if (ctx.target === "openapi-3.0") {
    json2.type = "string";
    json2.nullable = true;
    json2.enum = [null];
  } else {
    json2.type = "null";
  }
};
var undefinedProcessor = (schema, ctx, json2, params) => {
  handleUnrepresentable(schema, ctx, json2, params, "Undefined cannot be represented in JSON Schema");
};
var voidProcessor = (schema, ctx, json2, params) => {
  handleUnrepresentable(schema, ctx, json2, params, "Void cannot be represented in JSON Schema");
};
var neverProcessor = (_schema, _ctx, json2, _params) => {
  json2.not = {};
};
var anyProcessor = (_schema, _ctx, _json, _params) => {
};
var unknownProcessor = (_schema, _ctx, _json, _params) => {
};
var dateProcessor = (schema, ctx, json2, params) => {
  handleUnrepresentable(schema, ctx, json2, params, "Date cannot be represented in JSON Schema");
};
var enumProcessor = (schema, _ctx, json2, _params) => {
  const def = schema._zod.def;
  const values = getEnumValues(def.entries);
  if (values.every((v2) => typeof v2 === "number")) json2.type = "number";
  if (values.every((v2) => typeof v2 === "string")) json2.type = "string";
  json2.enum = values;
};
var literalProcessor = (schema, ctx, json2, params) => {
  const def = schema._zod.def;
  const vals = [];
  for (const val of def.values) {
    if (val === void 0) {
      if (handleUnrepresentable(schema, ctx, json2, params, "Literal `undefined` cannot be represented in JSON Schema"))
        return;
    } else if (typeof val === "bigint") {
      if (handleUnrepresentable(schema, ctx, json2, params, "BigInt literals cannot be represented in JSON Schema"))
        return;
      vals.push(Number(val));
    } else {
      vals.push(val);
    }
  }
  if (vals.length === 0) {
  } else if (vals.length === 1) {
    const val = vals[0];
    json2.type = val === null ? "null" : typeof val;
    if (ctx.target === "draft-04" || ctx.target === "openapi-3.0") {
      json2.enum = [val];
    } else {
      json2.const = val;
    }
  } else {
    if (vals.every((v2) => typeof v2 === "number")) json2.type = "number";
    if (vals.every((v2) => typeof v2 === "string")) json2.type = "string";
    if (vals.every((v2) => typeof v2 === "boolean")) json2.type = "boolean";
    if (vals.every((v2) => v2 === null)) json2.type = "null";
    json2.enum = vals;
  }
};
var nanProcessor = (schema, ctx, json2, params) => {
  handleUnrepresentable(schema, ctx, json2, params, "NaN cannot be represented in JSON Schema");
};
var templateLiteralProcessor = (schema, _ctx, json2, _params) => {
  const _json = json2;
  const pattern = schema._zod.pattern;
  if (!pattern) throw new Error("Pattern not found in template literal");
  _json.type = "string";
  _json.pattern = pattern.source;
};
var fileProcessor = (schema, _ctx, json2, _params) => {
  const _json = json2;
  const file2 = {
    type: "string",
    format: "binary",
    contentEncoding: "binary"
  };
  const { minimum, maximum, mime } = schema._zod.bag;
  if (minimum !== void 0) file2.minLength = minimum;
  if (maximum !== void 0) file2.maxLength = maximum;
  if (mime) {
    if (mime.length === 1) {
      file2.contentMediaType = mime[0];
      Object.assign(_json, file2);
    } else {
      Object.assign(_json, file2);
      _json.anyOf = mime.map((m2) => ({ contentMediaType: m2 }));
    }
  } else {
    Object.assign(_json, file2);
  }
};
var successProcessor = (_schema, _ctx, json2, _params) => {
  json2.type = "boolean";
};
var customProcessor = (schema, ctx, json2, params) => {
  handleUnrepresentable(schema, ctx, json2, params, "Custom types cannot be represented in JSON Schema");
};
var functionProcessor = (schema, ctx, json2, params) => {
  handleUnrepresentable(schema, ctx, json2, params, "Function types cannot be represented in JSON Schema");
};
var transformProcessor = (schema, ctx, json2, params) => {
  handleUnrepresentable(schema, ctx, json2, params, "Transforms cannot be represented in JSON Schema");
};
var mapProcessor = (schema, ctx, json2, params) => {
  handleUnrepresentable(schema, ctx, json2, params, "Map cannot be represented in JSON Schema");
};
var setProcessor = (schema, ctx, json2, params) => {
  handleUnrepresentable(schema, ctx, json2, params, "Set cannot be represented in JSON Schema");
};
var arrayProcessor = (schema, ctx, _json, params) => {
  const json2 = _json;
  const def = schema._zod.def;
  const { minimum, maximum } = schema._zod.bag;
  if (typeof minimum === "number") json2.minItems = minimum;
  if (typeof maximum === "number") json2.maxItems = maximum;
  json2.type = "array";
  json2.items = process(def.element, ctx, {
    ...params,
    path: [...params.path, "items"]
  });
};
function inputOptin(schema) {
  const def = schema._zod.def;
  if (def.type === "pipe" && def.in._zod.traits.has("$ZodTransform")) {
    return inputOptin(def.out);
  }
  if (def.type === "catch") {
    return inputOptin(def.innerType);
  }
  return schema._zod.optin;
}
var objectProcessor = (schema, ctx, _json, params) => {
  const json2 = _json;
  const def = schema._zod.def;
  json2.type = "object";
  json2.properties = {};
  const shape = def.shape;
  for (const key in shape) {
    assignProp(
      json2.properties,
      key,
      process(shape[key], ctx, {
        ...params,
        path: [...params.path, "properties", key]
      })
    );
  }
  const allKeys = new Set(Object.keys(shape));
  const requiredKeys = new Set(
    [...allKeys].filter((key) => {
      const field = def.shape[key];
      if (ctx.io === "input") {
        return inputOptin(field) === void 0;
      } else {
        return field._zod.optout === void 0;
      }
    })
  );
  if (requiredKeys.size > 0) {
    json2.required = Array.from(requiredKeys);
  }
  if (def.catchall?._zod.def.type === "never") {
    json2.additionalProperties = false;
  } else if (!def.catchall) {
    if (ctx.io === "output") json2.additionalProperties = false;
  } else if (def.catchall) {
    json2.additionalProperties = process(def.catchall, ctx, {
      ...params,
      path: [...params.path, "additionalProperties"]
    });
  }
};
var unionProcessor = (schema, ctx, json2, params) => {
  const def = schema._zod.def;
  const isExclusive = def.inclusive === false;
  const options = def.options.map(
    (x2, i) => process(x2, ctx, {
      ...params,
      path: [...params.path, isExclusive ? "oneOf" : "anyOf", i]
    })
  );
  if (isExclusive) {
    json2.oneOf = options;
  } else {
    json2.anyOf = options;
  }
};
var intersectionProcessor = (schema, ctx, json2, params) => {
  const def = schema._zod.def;
  const a = process(def.left, ctx, {
    ...params,
    path: [...params.path, "allOf", 0]
  });
  const b = process(def.right, ctx, {
    ...params,
    path: [...params.path, "allOf", 1]
  });
  const isSimpleIntersection = (val) => "allOf" in val && Object.keys(val).length === 1;
  const allOf = [
    ...isSimpleIntersection(a) ? a.allOf : [a],
    ...isSimpleIntersection(b) ? b.allOf : [b]
  ];
  json2.allOf = allOf;
};
var tupleProcessor = (schema, ctx, _json, params) => {
  const json2 = _json;
  const def = schema._zod.def;
  json2.type = "array";
  const prefixPath = ctx.target === "draft-2020-12" ? "prefixItems" : "items";
  const restPath = ctx.target === "draft-2020-12" ? "items" : ctx.target === "openapi-3.0" ? "items" : "additionalItems";
  const prefixItems = def.items.map(
    (x2, i) => process(x2, ctx, {
      ...params,
      path: [...params.path, prefixPath, i]
    })
  );
  const rest = def.rest ? process(def.rest, ctx, {
    ...params,
    path: [...params.path, restPath, ...ctx.target === "openapi-3.0" ? [def.items.length] : []]
  }) : null;
  let minItems = def.items.length;
  while (minItems > 0) {
    const item = def.items[minItems - 1];
    const optional2 = ctx.io === "input" ? inputOptin(item) !== void 0 : item._zod.optout === "optional";
    if (!optional2) break;
    minItems--;
  }
  const maxItems = def.items.length;
  const isClosed = !def.rest;
  if (ctx.target === "draft-2020-12") {
    json2.prefixItems = prefixItems;
    if (isClosed) {
      json2.items = false;
    } else if (rest) {
      json2.items = rest;
    }
    if (minItems > 0) json2.minItems = minItems;
    if (isClosed) json2.maxItems = maxItems;
  } else if (ctx.target === "openapi-3.0") {
    json2.items = {
      anyOf: prefixItems
    };
    if (rest) {
      json2.items.anyOf.push(rest);
    }
    if (minItems > 0) json2.minItems = minItems;
    if (isClosed) json2.maxItems = maxItems;
  } else {
    json2.items = prefixItems;
    if (isClosed) {
      json2.additionalItems = false;
    } else if (rest) {
      json2.additionalItems = rest;
    }
    if (minItems > 0) json2.minItems = minItems;
    if (isClosed) json2.maxItems = maxItems;
  }
  const { minimum, maximum } = schema._zod.bag;
  if (typeof minimum === "number") json2.minItems = minimum;
  if (typeof maximum === "number") json2.maxItems = maximum;
};
var recordProcessor = (schema, ctx, _json, params) => {
  const json2 = _json;
  const def = schema._zod.def;
  json2.type = "object";
  const keyType = def.keyType;
  const keyBag = keyType._zod.bag;
  const patterns = keyBag?.patterns;
  if (def.mode === "loose" && patterns && patterns.size > 0) {
    const valueSchema = process(def.valueType, ctx, {
      ...params,
      path: [...params.path, "patternProperties", "*"]
    });
    json2.patternProperties = {};
    for (const pattern of patterns) {
      assignProp(json2.patternProperties, pattern.source, valueSchema);
    }
  } else {
    if (ctx.target === "draft-07" || ctx.target === "draft-2020-12") {
      json2.propertyNames = process(def.keyType, ctx, {
        ...params,
        path: [...params.path, "propertyNames"]
      });
    }
    json2.additionalProperties = process(def.valueType, ctx, {
      ...params,
      path: [...params.path, "additionalProperties"]
    });
  }
  const keyValues = keyType._zod.values;
  if (keyValues && !def.partial) {
    const validKeyValues = [...keyValues].filter(
      (v2) => typeof v2 === "string" || typeof v2 === "number"
    );
    if (validKeyValues.length > 0) {
      json2.required = validKeyValues;
    }
  }
};
var nullableProcessor = (schema, ctx, json2, params) => {
  const def = schema._zod.def;
  const inner = process(def.innerType, ctx, params);
  const seen = ctx.seen.get(schema);
  if (ctx.target === "openapi-3.0") {
    seen.ref = def.innerType;
    json2.nullable = true;
  } else {
    json2.anyOf = [inner, { type: "null" }];
  }
};
var nonoptionalProcessor = (schema, ctx, _json, params) => {
  const def = schema._zod.def;
  process(def.innerType, ctx, params);
  const seen = ctx.seen.get(schema);
  seen.ref = def.innerType;
};
var UNREPRESENTABLE_DEFAULT = /* @__PURE__ */ Symbol();
function serializeDefaultValue(value, schema, ctx, json2, params) {
  let unrepresentable = false;
  const serialized = JSON.stringify(value, (_2, val) => {
    if (typeof val !== "bigint") return val;
    unrepresentable = true;
    return null;
  });
  if (!unrepresentable) return JSON.parse(serialized);
  handleUnrepresentable(schema, ctx, json2, params, "BigInt defaults cannot be represented in JSON Schema");
  return UNREPRESENTABLE_DEFAULT;
}
var defaultProcessor = (schema, ctx, json2, params) => {
  const def = schema._zod.def;
  process(def.innerType, ctx, params);
  const seen = ctx.seen.get(schema);
  seen.ref = def.innerType;
  const value = serializeDefaultValue(def.defaultValue, schema, ctx, json2, params);
  if (value !== UNREPRESENTABLE_DEFAULT) json2.default = value;
};
var prefaultProcessor = (schema, ctx, json2, params) => {
  const def = schema._zod.def;
  process(def.innerType, ctx, params);
  const seen = ctx.seen.get(schema);
  seen.ref = def.innerType;
  if (ctx.io !== "input") return;
  const value = serializeDefaultValue(def.defaultValue, schema, ctx, json2, params);
  if (value !== UNREPRESENTABLE_DEFAULT) json2._prefault = value;
};
var catchProcessor = (schema, ctx, json2, params) => {
  const def = schema._zod.def;
  process(def.innerType, ctx, params);
  const seen = ctx.seen.get(schema);
  seen.ref = def.innerType;
  let catchValue;
  try {
    catchValue = def.catchValue(void 0);
  } catch {
    handleUnrepresentable(schema, ctx, json2, params, "Dynamic catch values are not supported in JSON Schema");
    return;
  }
  json2.default = catchValue;
};
var pipeProcessor = (schema, ctx, _json, params) => {
  const def = schema._zod.def;
  const inIsTransform = def.in._zod.traits.has("$ZodTransform");
  const innerType = ctx.io === "input" ? inIsTransform ? def.out : def.in : def.out;
  process(innerType, ctx, params);
  const seen = ctx.seen.get(schema);
  seen.ref = innerType;
};
var readonlyProcessor = (schema, ctx, json2, params) => {
  const def = schema._zod.def;
  process(def.innerType, ctx, params);
  const seen = ctx.seen.get(schema);
  seen.ref = def.innerType;
  json2.readOnly = true;
};
var promiseProcessor = (schema, ctx, _json, params) => {
  const def = schema._zod.def;
  process(def.innerType, ctx, params);
  const seen = ctx.seen.get(schema);
  seen.ref = def.innerType;
};
var optionalProcessor = (schema, ctx, _json, params) => {
  const def = schema._zod.def;
  process(def.innerType, ctx, params);
  const seen = ctx.seen.get(schema);
  seen.ref = def.innerType;
};
var lazyProcessor = (schema, ctx, _json, params) => {
  const innerType = schema._zod.innerType;
  process(innerType, ctx, params);
  const seen = ctx.seen.get(schema);
  seen.ref = innerType;
};
var allProcessors = {
  string: stringProcessor,
  number: numberProcessor,
  boolean: booleanProcessor,
  bigint: bigintProcessor,
  symbol: symbolProcessor,
  null: nullProcessor,
  undefined: undefinedProcessor,
  void: voidProcessor,
  never: neverProcessor,
  any: anyProcessor,
  unknown: unknownProcessor,
  date: dateProcessor,
  enum: enumProcessor,
  literal: literalProcessor,
  nan: nanProcessor,
  template_literal: templateLiteralProcessor,
  file: fileProcessor,
  success: successProcessor,
  custom: customProcessor,
  function: functionProcessor,
  transform: transformProcessor,
  map: mapProcessor,
  set: setProcessor,
  array: arrayProcessor,
  object: objectProcessor,
  union: unionProcessor,
  intersection: intersectionProcessor,
  tuple: tupleProcessor,
  record: recordProcessor,
  nullable: nullableProcessor,
  nonoptional: nonoptionalProcessor,
  default: defaultProcessor,
  prefault: prefaultProcessor,
  catch: catchProcessor,
  pipe: pipeProcessor,
  readonly: readonlyProcessor,
  promise: promiseProcessor,
  optional: optionalProcessor,
  lazy: lazyProcessor
};
function toJSONSchema(input, params) {
  if ("_idmap" in input) {
    const registry2 = input;
    const ctx2 = initializeContext({ ...params, processors: allProcessors });
    const defs = {};
    for (const entry of registry2._idmap.entries()) {
      const [_2, schema] = entry;
      process(schema, ctx2);
    }
    const schemas = {};
    const external = {
      registry: registry2,
      uri: params?.uri,
      defs
    };
    ctx2.external = external;
    for (const entry of registry2._idmap.entries()) {
      const [key, schema] = entry;
      extractDefs(ctx2, schema);
      assignProp(schemas, key, finalize(ctx2, schema));
    }
    if (Object.keys(defs).length > 0) {
      const defsSegment = ctx2.target === "draft-2020-12" ? "$defs" : "definitions";
      schemas.__shared = {
        [defsSegment]: defs
      };
    }
    return { schemas };
  }
  const ctx = initializeContext({ ...params, processors: allProcessors });
  process(input, ctx);
  extractDefs(ctx, input);
  return finalize(ctx, input);
}
var JSONSchemaGenerator = class {
  constructor(params) {
    let normalizedTarget = params?.target ?? "draft-2020-12";
    if (normalizedTarget === "draft-4") normalizedTarget = "draft-04";
    if (normalizedTarget === "draft-7") normalizedTarget = "draft-07";
    this.ctx = initializeContext({
      processors: allProcessors,
      target: normalizedTarget,
      ...params?.metadata && { metadata: params.metadata },
      ...params?.unrepresentable && { unrepresentable: params.unrepresentable },
      ...params?.override && { override: params.override },
      ...params?.io && { io: params.io }
    });
  }
  process(schema, _params = { path: [], schemaPath: [] }) {
    return process(schema, this.ctx, _params);
  }
  emit(schema, _params) {
    if (_params) {
      if (_params.cycles) this.ctx.cycles = _params.cycles;
      if (_params.reused) this.ctx.reused = _params.reused;
      if (_params.external) this.ctx.external = _params.external;
    }
    this.ctx.sharedDefsExtractedFor = void 0;
    this.ctx.sharedEmitDoneFor = void 0;
    extractDefs(this.ctx, schema);
    const result = finalize(this.ctx, schema);
    const { "~standard": _2, ...plainResult } = result;
    return plainResult;
  }
};

// dist/regexes.js
var regexes_exports = {};
__export(regexes_exports, {
  base64: () => base64,
  base64url: () => base64url,
  bigint: () => bigint,
  boolean: () => boolean,
  browserEmail: () => browserEmail,
  cidrv4: () => cidrv4,
  cidrv6: () => cidrv6,
  creditCard: () => creditCard,
  cuid: () => cuid,
  cuid2: () => cuid2,
  date: () => date,
  datetime: () => datetime,
  domain: () => domain,
  duration: () => duration,
  e164: () => e164,
  email: () => email,
  emoji: () => emoji,
  extendedDuration: () => extendedDuration,
  guid: () => guid,
  hex: () => hex,
  hostname: () => hostname,
  html5Email: () => html5Email,
  httpProtocol: () => httpProtocol,
  idnEmail: () => idnEmail,
  integer: () => integer,
  ipv4: () => ipv4,
  ipv6: () => ipv6,
  ksuid: () => ksuid,
  lowercase: () => lowercase,
  mac: () => mac,
  md5_base64: () => md5_base64,
  md5_base64url: () => md5_base64url,
  md5_hex: () => md5_hex,
  nanoid: () => nanoid,
  null: () => _null,
  number: () => number,
  rfc5322Email: () => rfc5322Email,
  sha1_base64: () => sha1_base64,
  sha1_base64url: () => sha1_base64url,
  sha1_hex: () => sha1_hex,
  sha256_base64: () => sha256_base64,
  sha256_base64url: () => sha256_base64url,
  sha256_hex: () => sha256_hex,
  sha384_base64: () => sha384_base64,
  sha384_base64url: () => sha384_base64url,
  sha384_hex: () => sha384_hex,
  sha512_base64: () => sha512_base64,
  sha512_base64url: () => sha512_base64url,
  sha512_hex: () => sha512_hex,
  string: () => string,
  time: () => time,
  ulid: () => ulid,
  undefined: () => _undefined,
  unicodeEmail: () => unicodeEmail,
  uppercase: () => uppercase,
  uuid: () => uuid,
  uuid4: () => uuid4,
  uuid6: () => uuid6,
  uuid7: () => uuid7,
  xid: () => xid
});
function escapeRegex(str) {
  return str.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
var cuid = /^[cC][0-9a-z]{6,}$/;
var cuid2 = /^[0-9a-z]+$/;
var ulid = /^[0-9A-HJKMNP-TV-Za-hjkmnp-tv-z]{26}$/;
var xid = /^[0-9a-vA-V]{20}$/;
var ksuid = /^[A-Za-z0-9]{27}$/;
var nanoid = /^[a-zA-Z0-9_-]{21}$/;
var duration = /^P(?:(\d+W)|(?!.*W)(?=\d|T\d)(\d+Y)?(\d+M)?(\d+D)?(T(?=\d)(\d+H)?(\d+M)?(\d+([.,]\d+)?S)?)?)$/;
var extendedDuration = /^[-+]?P(?!$)(?:(?:[-+]?\d+Y)|(?:[-+]?\d+[.,]\d+Y$))?(?:(?:[-+]?\d+M)|(?:[-+]?\d+[.,]\d+M$))?(?:(?:[-+]?\d+W)|(?:[-+]?\d+[.,]\d+W$))?(?:(?:[-+]?\d+D)|(?:[-+]?\d+[.,]\d+D$))?(?:T(?=[\d+-])(?:(?:[-+]?\d+H)|(?:[-+]?\d+[.,]\d+H$))?(?:(?:[-+]?\d+M)|(?:[-+]?\d+[.,]\d+M$))?(?:[-+]?\d+(?:[.,]\d+)?S)?)??$/;
var guid = /^([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})$/;
var uuid = (version) => {
  if (!version)
    return /^([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[1-8][0-9a-fA-F]{3}-[89abAB][0-9a-fA-F]{3}-[0-9a-fA-F]{12}|00000000-0000-0000-0000-000000000000|ffffffff-ffff-ffff-ffff-ffffffffffff)$/;
  return new RegExp(
    `^([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-${version}[0-9a-fA-F]{3}-[89abAB][0-9a-fA-F]{3}-[0-9a-fA-F]{12})$`
  );
};
var uuid4 = /* @__PURE__ */ uuid(4);
var uuid6 = /* @__PURE__ */ uuid(6);
var uuid7 = /* @__PURE__ */ uuid(7);
var email = /^(?!\.)(?!.*\.\.)([A-Za-z0-9_'+\-\.]*)[A-Za-z0-9_+-]@([A-Za-z0-9][A-Za-z0-9\-]*\.)+[A-Za-z]{2,}$/;
var html5Email = /^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$/;
var rfc5322Email = /^(([^<>()\[\]\\.,;:\s@"]+(\.[^<>()\[\]\\.,;:\s@"]+)*)|(".+"))@((\[[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}])|(([a-zA-Z\-0-9]+\.)+[a-zA-Z]{2,}))$/;
var unicodeEmail = /^[^\s@"]{1,64}@[^\s@]{1,255}$/u;
var idnEmail = unicodeEmail;
var browserEmail = /^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$/;
var _emoji = `^[\\p{Extended_Pictographic}\\p{Emoji_Component}]+$`;
function emoji() {
  return new RegExp(_emoji, "u");
}
var ipv4 = /^(?:(?:25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]|[0-9])\.){3}(?:25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]|[0-9])$/;
var ipv6 = /^(([0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}|([0-9a-fA-F]{1,4}:){1,7}:|([0-9a-fA-F]{1,4}:){1,6}:[0-9a-fA-F]{1,4}|([0-9a-fA-F]{1,4}:){1,5}(:[0-9a-fA-F]{1,4}){1,2}|([0-9a-fA-F]{1,4}:){1,4}(:[0-9a-fA-F]{1,4}){1,3}|([0-9a-fA-F]{1,4}:){1,3}(:[0-9a-fA-F]{1,4}){1,4}|([0-9a-fA-F]{1,4}:){1,2}(:[0-9a-fA-F]{1,4}){1,5}|[0-9a-fA-F]{1,4}:((:[0-9a-fA-F]{1,4}){1,6})|:((:[0-9a-fA-F]{1,4}){1,7}|:))$/;
var mac = (delimiter) => {
  const escapedDelim = escapeRegex(delimiter ?? ":");
  return new RegExp(`^(?:[0-9A-F]{2}${escapedDelim}){5}[0-9A-F]{2}$|^(?:[0-9a-f]{2}${escapedDelim}){5}[0-9a-f]{2}$`);
};
var cidrv4 = /^((25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]|[0-9])\.){3}(25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]|[0-9])\/([0-9]|[1-2][0-9]|3[0-2])$/;
var cidrv6 = /^(([0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}|([0-9a-fA-F]{1,4}:){1,7}:|([0-9a-fA-F]{1,4}:){1,6}:[0-9a-fA-F]{1,4}|([0-9a-fA-F]{1,4}:){1,5}(:[0-9a-fA-F]{1,4}){1,2}|([0-9a-fA-F]{1,4}:){1,4}(:[0-9a-fA-F]{1,4}){1,3}|([0-9a-fA-F]{1,4}:){1,3}(:[0-9a-fA-F]{1,4}){1,4}|([0-9a-fA-F]{1,4}:){1,2}(:[0-9a-fA-F]{1,4}){1,5}|[0-9a-fA-F]{1,4}:((:[0-9a-fA-F]{1,4}){1,6})|:((:[0-9a-fA-F]{1,4}){1,7}|:))\/(12[0-8]|1[01][0-9]|[1-9]?[0-9])$/;
var base64 = /^$|^(?:[0-9a-zA-Z+/]{4})*(?:(?:[0-9a-zA-Z+/]{2}==)|(?:[0-9a-zA-Z+/]{3}=))?$/;
var base64url = /^[A-Za-z0-9_-]*$/;
var hostname = /^(?=.{1,253}\.?$)[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[-0-9a-zA-Z]{0,61}[0-9a-zA-Z])?)*\.?$/;
var domain = /^(?=.{1,253}$)([a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?\.)+[a-zA-Z]{2,63}$/;
var httpProtocol = /^https?$/;
var e164 = /^\+[1-9]\d{6,14}$/;
var creditCard = /^\d(?:[ -]?\d){11,18}$/;
var dateSource = `(?:(?:\\d\\d[2468][048]|\\d\\d[13579][26]|\\d\\d0[48]|[02468][048]00|[13579][26]00)-02-29|\\d{4}-(?:(?:0[13578]|1[02])-(?:0[1-9]|[12]\\d|3[01])|(?:0[469]|11)-(?:0[1-9]|[12]\\d|30)|(?:02)-(?:0[1-9]|1\\d|2[0-8])))`;
function anchor(source) {
  return new RegExp(`^${source}$`);
}
var date = /* @__PURE__ */ anchor(dateSource);
function timeSource(args) {
  const hhmm = `(?:[01]\\d|2[0-3]):[0-5]\\d`;
  const regex = typeof args.precision === "number" ? args.precision === -1 ? `${hhmm}` : args.precision === 0 ? `${hhmm}:[0-5]\\d` : `${hhmm}:[0-5]\\d\\.\\d{${args.precision}}` : `${hhmm}(?::[0-5]\\d(?:\\.\\d+)?)?`;
  return regex;
}
function time(args) {
  return new RegExp(`^${timeSource(args)}$`);
}
function datetime(args) {
  const time2 = timeSource({ precision: args.precision });
  const opts = ["Z"];
  if (args.offset) opts.push(`([+-](?:[01]\\d|2[0-3]):[0-5]\\d)`);
  const timeRegex = `${time2}(?:${opts.join("|")})${args.local ? "?" : ""}`;
  return new RegExp(`^${dateSource}T(?:${timeRegex})$`);
}
var string = (params) => {
  const regex = params ? `[\\s\\S]{${params?.minimum ?? 0},${params?.maximum ?? ""}}` : `[\\s\\S]*`;
  return new RegExp(`^${regex}$`);
};
var bigint = /^-?\d+n?$/;
var integer = /^-?\d+$/;
var number = /^-?\d+(?:\.\d+)?$/;
var boolean = /^(?:true|false)$/i;
var _null = /^null$/i;
var _undefined = /^undefined$/i;
var lowercase = /^[^A-Z]*$/;
var uppercase = /^[^a-z]*$/;
var hex = /^[0-9a-fA-F]*$/;
function fixedBase64(bodyLength, padding) {
  return new RegExp(`^[A-Za-z0-9+/]{${bodyLength}}${padding}$`);
}
function fixedBase64url(length) {
  return new RegExp(`^[A-Za-z0-9_-]{${length}}$`);
}
var md5_hex = /^[0-9a-fA-F]{32}$/;
var md5_base64 = /* @__PURE__ */ fixedBase64(22, "==");
var md5_base64url = /* @__PURE__ */ fixedBase64url(22);
var sha1_hex = /^[0-9a-fA-F]{40}$/;
var sha1_base64 = /* @__PURE__ */ fixedBase64(27, "=");
var sha1_base64url = /* @__PURE__ */ fixedBase64url(27);
var sha256_hex = /^[0-9a-fA-F]{64}$/;
var sha256_base64 = /* @__PURE__ */ fixedBase64(43, "=");
var sha256_base64url = /* @__PURE__ */ fixedBase64url(43);
var sha384_hex = /^[0-9a-fA-F]{96}$/;
var sha384_base64 = /* @__PURE__ */ fixedBase64(64, "");
var sha384_base64url = /* @__PURE__ */ fixedBase64url(64);
var sha512_hex = /^[0-9a-fA-F]{128}$/;
var sha512_base64 = /* @__PURE__ */ fixedBase64(86, "==");
var sha512_base64url = /* @__PURE__ */ fixedBase64url(86);

// dist/index.js
applyCompat(of);
installVisit(of);
installAsyncApi(of);
of.toJSONSchema = toJSONSchema;
of.core.toJSONSchema = toJSONSchema;
of.core.JSONSchemaGenerator = JSONSchemaGenerator;
of.regexes = regexes_exports;
of.core.regexes = regexes_exports;
of.compile = of.compile ?? ((schema) => schema);
if (of.ZodType && of.ZodType.prototype) {
  of.ZodType.prototype.toJSONSchema = function toJSONSchemaMethod(params) {
    return toJSONSchema(this, params);
  };
}
var index_default = of;
var string2 = of.string;
var number2 = of.number;
var boolean2 = of.boolean;
var bigint2 = of.bigint;
var symbol = of.symbol;
var date2 = of.date;
var nan = of.nan;
var $undefined = of.undefined;
var $null = of.null;
var any = of.any;
var unknown = of.unknown;
var never = of.never;
var $void = of.void;
var literal = of.literal;
var $enum = of.enum;
var nativeEnum = of.nativeEnum;
var object = of.object;
var strictObject = of.strictObject;
var looseObject = of.looseObject;
var $interface = of.interface;
var array = of.array;
var tuple = of.tuple;
var record = of.record;
var union = of.union;
var xor = of.xor;
var discriminatedUnion = of.discriminatedUnion;
var intersection = of.intersection;
var map = of.map;
var $set = of.set;
var lazy = of.lazy;
var promise = of.promise;
var custom = of.custom;
var file = of.file;
var $instanceof = of.instanceof;
var transform = of.transform;
var $function = of.function;
var int = of.int;
var int32 = of.int32;
var uint32 = of.uint32;
var float32 = of.float32;
var float64 = of.float64;
var pipe = of.pipe;
var optional = of.optional;
var nullable = of.nullable;
var email2 = of.email;
var uuid2 = of.uuid;
var guid2 = of.guid;
var url = of.url;
var httpUrl = of.httpUrl;
var codec = of.codec;
var invertCodec = of.invertCodec;
var preprocess = of.preprocess;
var stringbool = of.stringbool;
var success = of.success;
var json = of.json;
var hex2 = of.hex;
var hostname2 = of.hostname;
var hash = of.hash;
var partialRecord = of.partialRecord;
var looseRecord = of.looseRecord;
var creditCard2 = of.creditCard;
var mac2 = of.mac;
var keyof = of.keyof;
var $catch = of.catch;
var _default = of._default;
var prefault = of.prefault;
var nonoptional = of.nonoptional;
var readonly = of.readonly;
var jwt = of.jwt;
var nanoid2 = of.nanoid;
var ulid2 = of.ulid;
var ipv42 = of.ipv4;
var ipv62 = of.ipv6;
var coerce = of.coerce;
var iso = of.iso;
var locales = of.locales;
var core = of.core;
var parse = of.parse;
var safeParse = of.safeParse;
var parseAsync = of.parseAsync;
var safeParseAsync = of.safeParseAsync;
var encode = of.encode;
var decode = of.decode;
var encodeAsync = of.encodeAsync;
var decodeAsync = of.decodeAsync;
var treeifyError = of.treeifyError;
var prettifyError = of.prettifyError;
var formatError = of.formatError;
var flattenError = of.flattenError;
var registry = of.registry;
var globalRegistry2 = of.globalRegistry;
var config = of.config;
var ZodType = of.ZodType;
var ZodString = of.ZodString;
var ZodNumber = of.ZodNumber;
var ZodBoolean = of.ZodBoolean;
var ZodBigInt = of.ZodBigInt;
var ZodSymbol = of.ZodSymbol;
var ZodDate = of.ZodDate;
var ZodNaN = of.ZodNaN;
var ZodUndefined = of.ZodUndefined;
var ZodNull = of.ZodNull;
var ZodAny = of.ZodAny;
var ZodUnknown = of.ZodUnknown;
var ZodNever = of.ZodNever;
var ZodVoid = of.ZodVoid;
var ZodLiteral = of.ZodLiteral;
var ZodEnum = of.ZodEnum;
var ZodObject = of.ZodObject;
var ZodArray = of.ZodArray;
var ZodTuple = of.ZodTuple;
var ZodRecord = of.ZodRecord;
var ZodMap = of.ZodMap;
var ZodSet = of.ZodSet;
var ZodUnion = of.ZodUnion;
var ZodDiscriminatedUnion = of.ZodDiscriminatedUnion;
var ZodXor = of.ZodXor;
var ZodIntersection = of.ZodIntersection;
var ZodOptional = of.ZodOptional;
var ZodNullable = of.ZodNullable;
var ZodDefault = of.ZodDefault;
var ZodPrefault = of.ZodPrefault;
var ZodCatch = of.ZodCatch;
var ZodNonOptional = of.ZodNonOptional;
var ZodLazy = of.ZodLazy;
var ZodPromise = of.ZodPromise;
var ZodTransform = of.ZodTransform;
var ZodPipe = of.ZodPipe;
var ZodCodec = of.ZodCodec;
var ZodPreprocess = of.ZodPreprocess;
var ZodReadonly = of.ZodReadonly;
var ZodCustom = of.ZodCustom;
var ZodFile = of.ZodFile;
var ZodFunction = of.ZodFunction;
var ZodTemplateLiteral = of.ZodTemplateLiteral;
var ZodSuccess = of.ZodSuccess;
var ZodError = of.ZodError;
var ZodRealError = of.ZodRealError;
var getDiscriminatedOption = of.getDiscriminatedOption;
var ZodExactOptional = of.ZodExactOptional;
var exactOptional = of.exactOptional;
var slugify = of.slugify;
var properties = of.properties;
var property = of.property;
var templateLiteral = of.templateLiteral;
var stringFormat = of.stringFormat;
var check = of.check;
var $with = of.with;
var refine = of.refine;
var superRefine = of.superRefine;
var trim = of.trim;
var maxLength = of.maxLength;
var minLength = of.minLength;
var ZodISODateTime = of.ZodISODateTime;
var ZodISODate = of.ZodISODate;
var ZodISOTime = of.ZodISOTime;
var ZodISODuration = of.ZodISODuration;
var $ZodError = of.$ZodError;
var NEVER = of.NEVER;
var fromJSONSchema = of.fromJSONSchema;
var visit2 = of.visit;
var ZodIssueCode = of.ZodIssueCode;
var TimePrecision = of.TimePrecision;
var util = of.util;
var int64 = of.int64;
var uint64 = of.uint64;
var describe = of.describe;
var meta = of.meta;
var e1642 = of.e164;
var cidrv42 = of.cidrv4;
var cidrv62 = of.cidrv6;
var base642 = of.base64;
var base64url2 = of.base64url;
var xid2 = of.xid;
var ksuid2 = of.ksuid;
var cuid3 = of.cuid;
var cuid22 = of.cuid2;
var emoji2 = of.emoji;
var uuidv4 = of.uuidv4;
var uuidv6 = of.uuidv6;
var uuidv7 = of.uuidv7;
var deepPartial2 = of.deepPartial;
var safeEncode = of.safeEncode;
var safeDecode = of.safeDecode;
var safeEncodeAsync = of.safeEncodeAsync;
var safeDecodeAsync = of.safeDecodeAsync;
var setErrorMap = of.setErrorMap;
var compile = of.compile;
// Annotate the CommonJS export names for ESM import in node:
0 && (module.exports = {
  $ZodError,
  NEVER,
  TimePrecision,
  ZodAny,
  ZodArray,
  ZodBigInt,
  ZodBoolean,
  ZodCatch,
  ZodCodec,
  ZodCustom,
  ZodDate,
  ZodDefault,
  ZodDiscriminatedUnion,
  ZodEnum,
  ZodError,
  ZodExactOptional,
  ZodFile,
  ZodFunction,
  ZodISODate,
  ZodISODateTime,
  ZodISODuration,
  ZodISOTime,
  ZodIntersection,
  ZodIssueCode,
  ZodLazy,
  ZodLiteral,
  ZodMap,
  ZodNaN,
  ZodNever,
  ZodNonOptional,
  ZodNull,
  ZodNullable,
  ZodNumber,
  ZodObject,
  ZodOptional,
  ZodPipe,
  ZodPrefault,
  ZodPreprocess,
  ZodPromise,
  ZodReadonly,
  ZodRealError,
  ZodRecord,
  ZodSet,
  ZodString,
  ZodSuccess,
  ZodSymbol,
  ZodTemplateLiteral,
  ZodTransform,
  ZodTuple,
  ZodType,
  ZodUndefined,
  ZodUnion,
  ZodUnknown,
  ZodVoid,
  ZodXor,
  _default,
  any,
  array,
  base64,
  base64url,
  bigint,
  boolean,
  catch: null,
  check,
  cidrv4,
  cidrv6,
  codec,
  coerce,
  compile,
  config,
  core,
  creditCard,
  cuid,
  cuid2,
  custom,
  date,
  decode,
  decodeAsync,
  deepPartial,
  describe,
  discriminatedUnion,
  e164,
  email,
  emoji,
  encode,
  encodeAsync,
  enum: null,
  exactOptional,
  file,
  flattenError,
  float32,
  float64,
  formatError,
  fromJSONSchema,
  function: null,
  getDiscriminatedOption,
  globalRegistry,
  guid,
  hash,
  hex,
  hostname,
  httpUrl,
  instanceof: null,
  int,
  int32,
  int64,
  interface,
  intersection,
  invertCodec,
  ipv4,
  ipv6,
  iso,
  json,
  jwt,
  keyof,
  ksuid,
  lazy,
  literal,
  locales,
  looseObject,
  looseRecord,
  mac,
  map,
  maxLength,
  meta,
  minLength,
  nan,
  nanoid,
  nativeEnum,
  never,
  nonoptional,
  null: null,
  nullable,
  number,
  object,
  optional,
  parse,
  parseAsync,
  partialRecord,
  pipe,
  prefault,
  preprocess,
  prettifyError,
  promise,
  properties,
  property,
  readonly,
  record,
  refine,
  regexes,
  registry,
  safeDecode,
  safeDecodeAsync,
  safeEncode,
  safeEncodeAsync,
  safeParse,
  safeParseAsync,
  set,
  setErrorMap,
  slugify,
  strictObject,
  string,
  stringFormat,
  stringbool,
  success,
  superRefine,
  symbol,
  templateLiteral,
  toJSONSchema,
  transform,
  treeifyError,
  trim,
  tuple,
  uint32,
  uint64,
  ulid,
  undefined,
  union,
  unknown,
  url,
  util,
  uuid,
  uuidv4,
  uuidv6,
  uuidv7,
  visit,
  void: null,
  with: null,
  xid,
  xor,
  z
});
