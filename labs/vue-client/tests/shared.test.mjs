import assert from "node:assert/strict";
import { createRequire } from "node:module";
import test from "node:test";
import * as actual from "../packages/vuelil/shared.js";
import * as upstreamNamespace from "@vue/shared";

const upstream = upstreamNamespace.default ?? upstreamNamespace;
const require = createRequire(import.meta.url);
const upstreamPackage = require("@vue/shared/package.json");

const expectedExports = [
  "EMPTY_ARR",
  "EMPTY_OBJ",
  "NO",
  "NOOP",
  "PatchFlagNames",
  "PatchFlags",
  "ShapeFlags",
  "SlotFlags",
  "camelize",
  "capitalize",
  "cssVarNameEscapeSymbolsRE",
  "def",
  "escapeHtml",
  "escapeHtmlComment",
  "extend",
  "genCacheKey",
  "genPropsAccessExp",
  "generateCodeFrame",
  "getEscapedCssVarName",
  "getGlobalThis",
  "hasChanged",
  "hasOwn",
  "hyphenate",
  "includeBooleanAttr",
  "invokeArrayFns",
  "isArray",
  "isBooleanAttr",
  "isBuiltInDirective",
  "isDate",
  "isFunction",
  "isGloballyAllowed",
  "isGloballyWhitelisted",
  "isHTMLTag",
  "isIntegerKey",
  "isKnownHtmlAttr",
  "isKnownMathMLAttr",
  "isKnownSvgAttr",
  "isMap",
  "isMathMLTag",
  "isModelListener",
  "isObject",
  "isOn",
  "isPlainObject",
  "isPromise",
  "isRegExp",
  "isRenderableAttrValue",
  "isReservedProp",
  "isSSRSafeAttrName",
  "isSVGTag",
  "isSet",
  "isSpecialBooleanAttr",
  "isString",
  "isSymbol",
  "isVoidTag",
  "looseEqual",
  "looseIndexOf",
  "looseToNumber",
  "makeMap",
  "normalizeClass",
  "normalizeCssVarValue",
  "normalizeProps",
  "normalizeStyle",
  "objectToString",
  "parseStringStyle",
  "propsToAttrMap",
  "remove",
  "slotFlagsText",
  "stringifyStyle",
  "toDisplayString",
  "toHandlerKey",
  "toNumber",
  "toRawType",
  "toTypeString",
].sort();

const covered = new Set();
const mark = (...names) => names.forEach((name) => covered.add(name));

function captureCall(runtime, name, args) {
  const warnings = [];
  const errors = [];
  const originalWarn = console.warn;
  const originalError = console.error;
  console.warn = (...values) => warnings.push(values);
  console.error = (...values) => errors.push(values);
  try {
    return {
      result: runtime[name](...args),
      thrown: null,
      warnings,
      errors,
    };
  } catch (error) {
    return {
      result: undefined,
      thrown: error,
      warnings,
      errors,
    };
  } finally {
    console.warn = originalWarn;
    console.error = originalError;
  }
}

function compareCalls(name, factories) {
  mark(name);
  for (const factory of factories) {
    const actualArgs = factory();
    const upstreamArgs = factory();
    const observed = captureCall(actual, name, actualArgs);
    const reference = captureCall(upstream, name, upstreamArgs);
    assert.equal(
      observed.thrown?.constructor,
      reference.thrown?.constructor,
      `${name} throw type`,
    );
    if (!observed.thrown) {
      assert.deepStrictEqual(observed.result, reference.result, `${name} result`);
    }
    assert.deepStrictEqual(observed.warnings, reference.warnings, `${name} warnings`);
    assert.deepStrictEqual(observed.errors, reference.errors, `${name} errors`);
  }
}

function assertObjectAbi(name) {
  mark(name);
  assert.equal(Object.getPrototypeOf(actual[name]), Object.getPrototypeOf(upstream[name]));
  assert.deepStrictEqual(Reflect.ownKeys(actual[name]), Reflect.ownKeys(upstream[name]));
  assert.deepStrictEqual(
    Object.getOwnPropertyDescriptors(actual[name]),
    Object.getOwnPropertyDescriptors(upstream[name]),
  );
}

test("surface, function types, arities, and constant ABI match Vue 3.5.42", () => {
  assert.equal(upstreamPackage.version, "3.5.42");
  assert.equal(expectedExports.length, 73);
  assert.deepStrictEqual(Object.keys(actual).sort(), expectedExports);
  assert.deepStrictEqual(Object.keys(upstream).sort(), expectedExports);

  for (const name of expectedExports) {
    assert.equal(typeof actual[name], typeof upstream[name], `${name} typeof`);
    if (typeof upstream[name] === "function") {
      assert.equal(actual[name].length, upstream[name].length, `${name} arity`);
    }
  }

  mark("EMPTY_ARR", "EMPTY_OBJ");
  assert.deepStrictEqual(actual.EMPTY_ARR, upstream.EMPTY_ARR);
  assert.deepStrictEqual(actual.EMPTY_OBJ, upstream.EMPTY_OBJ);
  assert.deepStrictEqual(
    Object.getOwnPropertyDescriptors(actual.EMPTY_ARR),
    Object.getOwnPropertyDescriptors(upstream.EMPTY_ARR),
  );
  assert.deepStrictEqual(
    Object.getOwnPropertyDescriptors(actual.EMPTY_OBJ),
    Object.getOwnPropertyDescriptors(upstream.EMPTY_OBJ),
  );
  assert.equal(Object.isFrozen(actual.EMPTY_ARR), Object.isFrozen(upstream.EMPTY_ARR));
  assert.equal(Object.isFrozen(actual.EMPTY_OBJ), Object.isFrozen(upstream.EMPTY_OBJ));

  for (const name of [
    "PatchFlags",
    "PatchFlagNames",
    "ShapeFlags",
    "SlotFlags",
    "slotFlagsText",
    "propsToAttrMap",
  ]) {
    assertObjectAbi(name);
  }

  mark("extend", "isArray", "objectToString");
  assert.equal(actual.extend, Object.assign);
  assert.equal(actual.isArray, Array.isArray);
  assert.equal(actual.objectToString, Object.prototype.toString);
  assert.equal(actual.extend, upstream.extend);
  assert.equal(actual.isArray, upstream.isArray);
  assert.equal(actual.objectToString, upstream.objectToString);

  mark("cssVarNameEscapeSymbolsRE");
  assert.equal(
    actual.cssVarNameEscapeSymbolsRE.source,
    upstream.cssVarNameEscapeSymbolsRE.source,
  );
  assert.equal(
    actual.cssVarNameEscapeSymbolsRE.flags,
    upstream.cssVarNameEscapeSymbolsRE.flags,
  );
  assert.equal(
    Object.getPrototypeOf(actual.cssVarNameEscapeSymbolsRE),
    Object.getPrototypeOf(upstream.cssVarNameEscapeSymbolsRE),
  );
  assert.deepStrictEqual(
    Object.getOwnPropertyDescriptors(actual.cssVarNameEscapeSymbolsRE),
    Object.getOwnPropertyDescriptors(upstream.cssVarNameEscapeSymbolsRE),
  );
});

test("map creation and general predicates match deterministic edge vectors", () => {
  mark("makeMap");
  const actualMap = actual.makeMap(",foo,__proto__,toString");
  const upstreamMap = upstream.makeMap(",foo,__proto__,toString");
  for (const key of ["", "foo", "bar", "__proto__", "toString", Symbol("foo")]) {
    assert.equal(actualMap(key), upstreamMap(key));
  }

  compareCalls("NOOP", [() => [], () => [1, 2]]);
  compareCalls("NO", [() => [], () => [1]]);
  compareCalls("isOn", [
    () => ["onClick"],
    () => ["onclick"],
    () => ["on-click"],
    () => ["on"],
    () => [""],
  ]);
  compareCalls("isModelListener", [
    () => ["onUpdate:modelValue"],
    () => ["onupdate:modelValue"],
    () => ["onUpdate:"],
  ]);
  compareCalls("hasOwn", [
    () => [{ own: 1 }, "own"],
    () => [Object.create({ inherited: 1 }), "inherited"],
    () => [{ [Symbol.for("owned")]: true }, Symbol.for("owned")],
    () => [Object.create(null, { x: { value: 1, enumerable: true } }), "x"],
  ]);
  compareCalls("isMap", [
    () => [new Map()],
    () => [new Set()],
    () => [{ [Symbol.toStringTag]: "Map" }],
    () => [null],
  ]);
  compareCalls("isSet", [() => [new Set()], () => [new Map()], () => [null]]);
  compareCalls("isDate", [
    () => [new Date(0)],
    () => [new Date(Number.NaN)],
    () => [/x/],
  ]);
  compareCalls("isRegExp", [() => [/x/gi], () => [new Date()], () => [null]]);
  compareCalls("isFunction", [
    () => [function named() {}],
    () => [class Value {}],
    () => [() => 1],
    () => [{}],
  ]);
  compareCalls("isString", [() => ["x"], () => [new String("x")], () => [Symbol()]]);
  compareCalls("isSymbol", [() => [Symbol("x")], () => ["Symbol(x)"], () => [null]]);
  compareCalls("isObject", [() => [{}], () => [[]], () => [null], () => [() => {}]]);
  compareCalls("isPromise", [
    () => [Promise.resolve(1)],
    () => [{ then() {}, catch() {} }],
    () => [{ then() {}, catch: 1 }],
    () => [null],
  ]);
  compareCalls("toTypeString", [
    () => [null],
    () => [undefined],
    () => [new Map()],
    () => [{ [Symbol.toStringTag]: "Custom" }],
  ]);
  compareCalls("toRawType", [
    () => [[]],
    () => [new Date(0)],
    () => [Object.create(null)],
  ]);
  compareCalls("isPlainObject", [
    () => [{}],
    () => [Object.create(null)],
    () => [new (class Value {})()],
    () => [[]],
  ]);
  compareCalls(
    "isIntegerKey",
    ["0", "1", "01", "-0", "-1", "1e2", "NaN", "Infinity", "", 1].map(
      (value) => () => [value],
    ),
  );
  compareCalls("isReservedProp", [
    () => [""],
    () => ["key"],
    () => ["onVnodeUnmounted"],
    () => ["class"],
  ]);
  compareCalls("isBuiltInDirective", [
    () => ["if"],
    () => ["else-if"],
    () => ["memo"],
    () => ["custom"],
  ]);
});

test("string transforms, mutation helpers, and numeric conversion match", () => {
  compareCalls("camelize", [
    () => ["foo-bar-baz"],
    () => ["alreadyCamel"],
    () => ["a--b"],
    () => ["-é-x"],
    () => [""],
  ]);
  compareCalls("hyphenate", [
    () => ["fooBar"],
    () => ["XMLHttpRequest"],
    () => ["already-kebab"],
    () => [""],
  ]);
  compareCalls("capitalize", [() => ["hello"], () => ["éclair"], () => [""], () => ["1x"]]);
  compareCalls("toHandlerKey", [() => ["click"], () => ["update:modelValue"], () => [""]]);
  compareCalls("hasChanged", [
    () => [Number.NaN, Number.NaN],
    () => [-0, 0],
    () => [1, 1],
    () => [{}, {}],
  ]);

  mark("remove");
  for (const vector of [
    { values: [1, 2, 1], value: 1 },
    { values: [1, 2], value: 3 },
    { values: [Number.NaN], value: Number.NaN },
  ]) {
    const left = [...vector.values];
    const right = [...vector.values];
    assert.equal(actual.remove(left, vector.value), upstream.remove(right, vector.value));
    assert.deepStrictEqual(left, right);
  }

  mark("invokeArrayFns");
  const invoke = (runtime) => {
    const events = [];
    const functions = [
      function first(...args) {
        events.push(["first", Array.isArray(this), this === functions, args]);
      },
      function second(...args) {
        events.push(["second", Array.isArray(this), this === functions, args]);
      },
    ];
    const result = runtime.invokeArrayFns(functions, "x", 2);
    return { result, events };
  };
  assert.deepStrictEqual(invoke(actual), invoke(upstream));

  mark("def");
  for (const writable of [undefined, false, true]) {
    const left = {};
    const right = {};
    const leftKey = Symbol("hidden");
    const rightKey = Symbol("hidden");
    if (writable === undefined) {
      actual.def(left, leftKey, 7);
      upstream.def(right, rightKey, 7);
    } else {
      actual.def(left, leftKey, 7, writable);
      upstream.def(right, rightKey, 7, writable);
    }
    assert.deepStrictEqual(
      Object.getOwnPropertyDescriptor(left, leftKey),
      Object.getOwnPropertyDescriptor(right, rightKey),
    );
  }

  compareCalls("looseToNumber", [
    () => ["12.5px"],
    () => [""],
    () => ["  -0"],
    () => [null],
    () => [Symbol.for("to-number")],
  ]);
  compareCalls("toNumber", [
    () => ["12.5"],
    () => [""],
    () => ["0x10"],
    () => ["12px"],
    () => [12],
    () => [Symbol.for("to-number")],
  ]);

  mark("getGlobalThis");
  assert.equal(actual.getGlobalThis(), upstream.getGlobalThis());
  assert.equal(actual.getGlobalThis(), actual.getGlobalThis());
});

test("compiler helpers and code frames match", () => {
  compareCalls("genPropsAccessExp", [
    () => ["validName"],
    () => ["$valid_1"],
    () => ["éclair"],
    () => ["hyphen-name"],
    () => ["quote\"line\n"],
    () => [""],
  ]);

  function cacheFunction(value) {
    return value;
  }
  compareCalls("genCacheKey", [
    () => ["source", { a: 1, nested: [true, null] }],
    () => ["source", { fn: cacheFunction }],
    () => ["source", undefined],
  ]);

  const source = [
    "<template>",
    "  <div>",
    "    <span>value</span>",
    "  </div>",
    "</template>",
  ].join("\n");
  compareCalls("generateCodeFrame", [
    () => [source],
    () => [source, source.indexOf("span"), source.indexOf("span") + 4],
    () => [source.replaceAll("\n", "\r\n"), 12, 45],
    () => [source, -Infinity, 0],
    () => [source, 0, Infinity],
    () => [source, Infinity, 0],
    () => ["a\nb\nc", 1.5, 2.5],
  ]);
});

test("class and style normalization match, including identity and mutation", () => {
  compareCalls("parseStringStyle", [
    () => ["color:red; font-size: 12px"],
    () => ["background:url(data:image/svg+xml;a;b); color: blue"],
    () => ["/* before */color: red;broken;--x:a:b"],
    () => [""],
  ]);
  compareCalls("normalizeStyle", [
    () => ["color:red"],
    () => [{ color: "red" }],
    () => [["color:red", { fontSize: "12px" }, [null, "opacity:0"]]],
    () => [null],
    () => [false],
  ]);
  compareCalls("stringifyStyle", [
    () => ["color: blue;"],
    () => [{ color: "blue", fontSize: "14px", opacity: 0, bad: true }],
    () => [{ "--custom-color": "red", WebkitTransform: "scale(1)" }],
    () => [null],
  ]);
  compareCalls("normalizeClass", [
    () => [" foo "],
    () => [["foo", undefined, true, false, ["bar"], { baz: 1, no: 0 }]],
    () => [{ inherited: true }],
    () => [null],
  ]);
  compareCalls("normalizeProps", [
    () => [null],
    () => [{ class: ["one", { two: true }], style: ["color:red", { opacity: 0 }] }],
    () => [{ class: "already", style: "color:blue" }],
    () => [{ class: 0, style: null }],
  ]);

  const style = { color: "red" };
  const props = { class: "x" };
  assert.equal(actual.normalizeStyle(style), style);
  assert.equal(actual.normalizeProps(props), props);
});

test("tag and attribute tables match positive, negative, and case-sensitive vectors", () => {
  const mapVectors = {
    isHTMLTag: ["html", "picture", "tfoot", "SVG", "custom-element"],
    isSVGTag: ["svg", "linearGradient", "foreignObject", "lineargradient", "div"],
    isMathMLTag: ["math", "annotation-xml", "semantics", "Math", "div"],
    isVoidTag: ["br", "input", "wbr", "div", "BR"],
    isSpecialBooleanAttr: ["itemscope", "readonly", "disabled", "readOnly"],
    isBooleanAttr: ["itemscope", "disabled", "selected", "aria-hidden", "DISABLED"],
    isKnownHtmlAttr: ["accept", "http-equiv", "srcset", "aria-label", "viewBox"],
    isKnownSvgAttr: ["viewBox", "xlink:href", "stroke-width", "aria-label", "viewbox"],
    isKnownMathMLAttr: ["accent", "mathvariant", "xlink:href", "aria-label", "viewBox"],
  };
  for (const [name, values] of Object.entries(mapVectors)) {
    compareCalls(
      name,
      values.map((value) => () => [value]),
    );
  }

  compareCalls("includeBooleanAttr", [
    () => [true],
    () => [false],
    () => [""],
    () => [0],
    () => [1],
    () => [null],
    () => [{}],
  ]);
  compareCalls("isRenderableAttrValue", [
    () => [""],
    () => [0],
    () => [false],
    () => [1n],
    () => [Symbol()],
    () => [null],
    () => [{}],
  ]);
  compareCalls("isSSRSafeAttrName", [
    () => ["data-safe"],
    () => ["bad name"],
    () => ["bad=name"],
    () => ["data-safe"],
    () => ["hasOwnProperty"],
    () => ["hasOwnProperty"],
  ]);
});

test("HTML and CSS escaping match", () => {
  compareCalls("escapeHtml", [
    () => ["plain"],
    () => [`"'&<>`],
    () => [true],
    () => [0],
    () => [{ toString: () => "<object>" }],
    () => [Symbol("x")],
  ]);
  compareCalls("escapeHtmlComment", [
    () => ["<!-- Hello --><!-- World! -->"],
    () => ["--<!--><img src=x>"],
    () => ["Hello World"],
    () => ["--!>-->"] ,
  ]);
  compareCalls("getEscapedCssVarName", [
    () => ["plain-name_1", false],
    () => [`a b!"\\[]`, false],
    () => [`a b!"\\[]`, true],
    () => ["", true],
  ]);
});

test("loose equality and display stringification match recursive collections", () => {
  compareCalls("looseEqual", [
    () => [1, "1"],
    () => [true, 1],
    () => [Number.NaN, Number.NaN],
    () => [new Date(123), new Date(123)],
    () => [/foo/gi, /foo/gi],
    () => [/foo/g, /foo/i],
    () => [[1, { value: "2" }], ["1", { value: 2 }]],
    () => [{ a: 1, b: [2] }, { b: ["2"], a: "1" }],
    () => [new Map([[{ id: 1 }, { value: "2" }]]), new Map([[{ id: "1" }, { value: 2 }]])],
    () => [new Set([{ id: 1 }, "x"]), new Set(["x", { id: "1" }])],
    () => [new Set(), new Map()],
    () => [Symbol.for("x"), Symbol.for("x")],
    () => [Symbol("x"), Symbol("x")],
    () => [null, undefined],
  ]);
  compareCalls("looseIndexOf", [
    () => [[1, { id: 2 }, "3"], 3],
    () => [[{ id: 1 }, { id: 2 }], { id: "2" }],
    () => [[], 1],
  ]);
  compareCalls("toDisplayString", [
    () => [null],
    () => [Number.NaN],
    () => [Symbol("hello")],
    () => [{ foo: 1, nested: [true, null] }],
    () => [{ __v_isRef: true, value: { x: 1 } }],
    () => [{ foo: 1, toString: () => "override" }],
    () => [Object.assign(Object.create(null), { bar: 1 })],
    () => [new Map([[Symbol(), "first"], [Symbol("named"), { x: 1 }]])],
    () => [new Set([1, Symbol("x"), new Map([["key", "value"]])])],
  ]);
});

test("CSS variable normalization and global allow-list aliases match", () => {
  compareCalls("normalizeCssVarValue", [
    () => [null],
    () => [undefined],
    () => [""],
    () => ["  "],
    () => [0],
    () => [Number.NaN],
    () => [Symbol.for("css")],
    () => [1n],
    () => [Infinity],
    () => [{}],
    () => [[]],
  ]);

  compareCalls("isGloballyAllowed", [
    () => ["Infinity"],
    () => ["globalThis"],
    () => ["Symbol"],
    () => ["window"],
  ]);
  mark("isGloballyWhitelisted");
  assert.equal(actual.isGloballyWhitelisted, actual.isGloballyAllowed);
  assert.equal(upstream.isGloballyWhitelisted, upstream.isGloballyAllowed);
});

test("every runtime export is exercised", () => {
  assert.deepStrictEqual([...covered].sort(), expectedExports);
});
