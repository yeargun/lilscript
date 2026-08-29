import assert from "node:assert/strict";
import test from "node:test";
import * as candidate from "../packages/vuelil/compiler-core.js";
import * as candidateShared from "../packages/vuelil/shared.js";
import * as oracle from "@vue/compiler-core";

const intentionalInternalTestExports = [
  "cloneLoc",
  "defaultOnError",
  "defaultOnWarn",
  "defaultParserOptions",
  "finalizeForParseResult",
  "isCompatEnabled",
  "transformFor",
  "transformIf",
  "transformMemo",
  "transformOnce",
  "transformSlotOutlet",
  "transformText",
  "traverseChildren",
];

test("compiler-core preserves its complete public surface and live export identity", () => {
  const upstreamRuntimeExports = Object.keys(oracle)
    .filter(
      name =>
        name !== "__esModule" && name !== "default" && name !== "module.exports",
    )
    .sort();
  const candidateExports = Object.keys(candidate).sort();

  assert.equal(upstreamRuntimeExports.length, 145);
  assert.equal(candidateExports.length, 158);
  assert.deepStrictEqual(
    candidateExports.filter(name => !upstreamRuntimeExports.includes(name)),
    intentionalInternalTestExports,
  );
  assert.deepStrictEqual(
    upstreamRuntimeExports.filter(name => !candidateExports.includes(name)),
    [],
  );
  for (const name of upstreamRuntimeExports) {
    if (typeof oracle[name] === "function") {
      assert.deepStrictEqual(
        [candidate[name].name, candidate[name].length],
        [oracle[name].name, oracle[name].length],
        name,
      );
    }
  }
  assert.equal(candidate.generateCodeFrame, candidateShared.generateCodeFrame);
});

const vectors = [
  { source: "" },
  { source: "plain text &amp; entities" },
  { source: "alpha {{ value + 1 }} omega" },
  { source: "<div>hello <span>world</span></div>" },
  { source: "<Comp id disabled class=\"  a  b \"/>" },
  { source: "<template v-if=\"ok\"><slot/></template>" },
  { source: "<div :foo=\"bar\" @[event].stop=\"run()\"/>" },
  { source: "<Comp #default=\"{ item }\">{{ item }}</Comp>" },
  { source: "<ul><li v-for=\"(item, key, index) in list\">{{item}}</li></ul>" },
  { source: "<!-- one --><p/><!-- two -->" },
  { source: "<div v-pre :id=\"value\">{{ raw }}<Comp/></div>" },
  { source: "<div>\r\n  <span/>\n  <span/>\n</div>" },
  {
    source: "<pre>\r\n  keep   spacing</pre>",
    options: { isPreTag: tag => tag === "pre", isIgnoreNewlineTag: tag => tag === "pre" },
  },
  { source: "<textarea>one &amp; two\n   three {{value}}</textarea>", options: { parseMode: "html" } },
  { source: "<script>if (a < b) x = '{{raw}}'</script>", options: { parseMode: "html" } },
  { source: "<style>.x > .y { color: red }</style>", options: { parseMode: "html" } },
  { source: "<script setup lang=\"ts\">const x = a < b</script>", options: { parseMode: "sfc" } },
  { source: "<template lang=\"pug\">div {{ untouched }}</template>", options: { parseMode: "sfc" } },
  {
    source: "<svg><![CDATA[a < b]]><g/></svg>",
    options: { getNamespace: tag => (tag === "svg" || tag === "g" ? 1 : 0) },
  },
  { source: "<DIV><x-y data-a='1'></X-Y></div>" },
  { source: "<img>after", options: { isVoidTag: tag => tag === "img" } },
  { source: "<div/> <div/> <div/>", options: { whitespace: "preserve" } },
  { source: "<div/> \n <!-- x --> \n <div/>", options: { whitespace: "condense" } },
  { source: "<div :[key+1]=\"foo()\"/>", options: { prefixIdentifiers: true } },
  { source: "<Comp #x=\"{ a, b }\"/>", options: { prefixIdentifiers: true } },
  { source: "<div @click=\"a++;b++\"/>", options: { prefixIdentifiers: true } },
  { source: "<slot name=\"header\"></slot><KeepAlive/>" },
  { source: "<div is=\"vue:widget\"></div>" },
  { source: "<a><b><c>text</c></b></a>" },
  { source: "{value}", options: { delimiters: ["{", "}"] } },
];

test("baseParse differentially matches Vue across broad vectors", () => {
  for (const { source, options } of vectors) {
    assert.deepStrictEqual(
      candidate.baseParse(source, options),
      oracle.baseParse(source, options),
      source,
    );
  }
});

const malformed = [
  "some text</div>",
  "<div><span></div></span>",
  "<template><",
  "<template></",
  "<template><!--comment",
  "<template><svg><![CDATA[cdata",
  "<template><div id=></div></template>",
  "<template><div foo=bar`></div></template>",
  "<template><div =foo=bar></div></template>",
  "<template><?xml?></template>",
  "<template><div a/b></div></template>",
  "<div>{{ missing",
  "<div v-foo:[missing end />",
];

function parseWithErrors(implementation, source) {
  const errors = [];
  const ast = implementation.baseParse(source, {
    parseMode: "html",
    getNamespace: tag => (tag === "svg" ? 1 : 0),
    onError(error) {
      errors.push({ code: error.code, message: error.message, loc: error.loc });
    },
  });
  return { ast, errors };
}

test("baseParse recovery, errors, and locations match Vue", () => {
  for (const source of malformed) {
    assert.deepStrictEqual(
      parseWithErrors(candidate, source),
      parseWithErrors(oracle, source),
      source,
    );
  }
});

test("AST enums and builders match Vue object shapes", () => {
  for (const name of ["Namespaces", "NodeTypes", "ElementTypes", "ConstantTypes", "BindingTypes", "ErrorCodes"]) {
    assert.deepStrictEqual(candidate[name], oracle[name], name);
  }
  const expression = candidate.createSimpleExpression("value", true);
  const oracleExpression = oracle.createSimpleExpression("value", true);
  assert.deepStrictEqual(expression, oracleExpression);
  assert.deepStrictEqual(
    candidate.createObjectProperty("key", expression),
    oracle.createObjectProperty("key", oracleExpression),
  );
  assert.deepStrictEqual(
    candidate.createConditionalExpression(expression, expression, expression),
    oracle.createConditionalExpression(oracleExpression, oracleExpression, oracleExpression),
  );
  const builderCases = [
    ["createArrayExpression", () => [[]]],
    ["createObjectExpression", () => [[]]],
    ["createCallExpression", () => ["render"]],
    ["createFunctionExpression", () => [[]]],
    ["createCacheExpression", implementation => [1, implementation.createSimpleExpression("x")]],
    ["createBlockStatement", () => [[]]],
    ["createTemplateLiteral", () => [[]]],
    [
      "createIfStatement",
      implementation => [implementation.createSimpleExpression("ok"), implementation.createBlockStatement([])],
    ],
    [
      "createAssignmentExpression",
      implementation => [implementation.createSimpleExpression("x"), implementation.createSimpleExpression("y")],
    ],
    ["createSequenceExpression", () => [[]]],
    ["createReturnStatement", implementation => [implementation.createSimpleExpression("x")]],
    ["createVNodeCall", () => [null, "div"]],
  ];
  for (const [name, args] of builderCases) {
    assert.deepStrictEqual(candidate[name](...args(candidate)), oracle[name](...args(oracle)), name);
  }
  assert.deepStrictEqual(candidate.createRoot([], "source"), oracle.createRoot([], "source"));
});

test("compiler-core utility tranche matches representative upstream behavior", () => {
  const expressions = ["obj.foo", "obj[arr[0]]", "foo().bar", "obj?.x", "foo()", "a + b"];
  for (const source of expressions) {
    const left = candidate.createSimpleExpression(source);
    const right = oracle.createSimpleExpression(source);
    assert.equal(candidate.isMemberExpressionBrowser(left), oracle.isMemberExpressionBrowser(right), source);
    assert.equal(candidate.isMemberExpressionNode(left, {}), oracle.isMemberExpressionNode(right, {}), source);
  }
  assert.deepStrictEqual(
    candidate.advancePositionWithClone({ line: 2, column: 3, offset: 5 }, "a\nb\nc", 4),
    oracle.advancePositionWithClone({ line: 2, column: 3, offset: 5 }, "a\nb\nc", 4),
  );
  assert.equal(
    candidate.generateCodeFrame("first\nsecond\nthird", 7, 13),
    oracle.generateCodeFrame("first\nsecond\nthird", 7, 13),
  );
  for (const name of ["foo", "foo-bar", "test-测试-1"]) {
    assert.equal(candidate.toValidAssetId(name, "component"), oracle.toValidAssetId(name, "component"));
  }
});

test("publishes implemented transform, codegen, and compile APIs", () => {
  for (const name of [
    "baseCompile",
    "createTransformContext",
    "generate",
    "processSlotOutlet",
    "transform",
    "transformElement",
    "transformExpression",
    "transformSlotOutlet",
  ]) assert.equal(typeof candidate[name], "function", name);

  for (const name of ["baseCompile", "getBaseTransformPreset"]) {
    assert.equal(candidate[name].name, oracle[name].name, `${name}.name`);
    assert.equal(candidate[name].length, oracle[name].length, `${name}.length`);
  }
  for (const name of [
    "transformElement",
    "resolveComponentType",
    "buildProps",
    "buildDirectiveArgs",
  ]) {
    assert.deepStrictEqual(
      [candidate[name].name, candidate[name].length],
      [oracle[name].name, oracle[name].length],
      name,
    );
  }
  assert.deepStrictEqual(
    [candidate.processIf.name, candidate.processIf.length],
    [oracle.processIf.name, oracle.processIf.length],
  );
  for (const name of ["processFor", "createForLoopParams"]) {
    assert.deepStrictEqual(
      [candidate[name].name, candidate[name].length],
      [oracle[name].name, oracle[name].length],
      name,
    );
  }
  assert.deepStrictEqual(
    [candidate.getBaseTransformPreset()[0][2].name, candidate.getBaseTransformPreset()[0][2].length],
    [oracle.getBaseTransformPreset()[0][2].name, oracle.getBaseTransformPreset()[0][2].length],
  );
  assert.equal(candidate.getBaseTransformPreset()[0][2], candidate.transformIf);
  assert.deepStrictEqual(
    [candidate.getBaseTransformPreset()[0][4].name, candidate.getBaseTransformPreset()[0][4].length],
    [oracle.getBaseTransformPreset()[0][4].name, oracle.getBaseTransformPreset()[0][4].length],
  );
  assert.equal(candidate.getBaseTransformPreset()[0][4], candidate.transformFor);
  assert.deepStrictEqual(
    [candidate.transformText.name, candidate.transformText.length],
    ["transformText", 2],
  );
  assert.deepStrictEqual(
    [candidate.processSlotOutlet.name, candidate.processSlotOutlet.length],
    [oracle.processSlotOutlet.name, oracle.processSlotOutlet.length],
  );
  assert.deepStrictEqual(
    [candidate.transformSlotOutlet.name, candidate.transformSlotOutlet.length],
    ["transformSlotOutlet", 2],
  );

  for (const source of ["<div/>", "<div :id=\"value\">{{ value }}</div>"]) {
    assert.equal(candidate.baseCompile(source).code, oracle.baseCompile(source).code);
  }

  const helperSymbols = Object.keys(candidate)
    .map(name => candidate[name])
    .filter(value => typeof value === "symbol");
  assert.equal(helperSymbols.length, 39);
  assert.equal(new Set(helperSymbols).size, 39);
  assert.equal(candidate.getVNodeHelper(false, true), candidate.CREATE_VNODE);
  assert.equal(candidate.getVNodeHelper(false, false), candidate.CREATE_ELEMENT_VNODE);
  assert.equal(candidate.helperNameMap[candidate.CREATE_VNODE], "createVNode");

  const helperNameMap = candidate.helperNameMap;
  const customHelper = Symbol("customHelper");
  candidate.registerRuntimeHelpers({ [customHelper]: "customHelper" });
  assert.equal(candidate.helperNameMap, helperNameMap);
  assert.equal(helperNameMap[customHelper], "customHelper");
  assert.equal(candidate.registerRuntimeHelpers.length, 1);
});

test("transformOn preserves analysis, augmentation, metadata, locations, and reflection", () => {
  assert.deepStrictEqual(
    [candidate.transformOn.name, candidate.transformOn.length],
    [oracle.transformOn.name, oracle.transformOn.length],
  );

  function apply(implementation, source, options = {}, augmented = false) {
    const errors = [];
    const root = implementation.baseParse(source);
    const node = root.children[0];
    const directive = node.props[0];
    const context = implementation.createTransformContext(root, {
      ...options,
      onError(error) {
        errors.push({ code: error.code, loc: error.loc });
      },
    });
    const augmentor = augmented
      ? result => ({
          props: [
            ...result.props,
            implementation.createObjectProperty(
              implementation.createSimpleExpression("onExtra", true, directive.loc),
              implementation.createSimpleExpression("extra", false, directive.loc),
            ),
          ],
        })
      : undefined;
    const result = implementation.transformOn(directive, node, context, augmentor);
    return { result, cached: context.cached, errors, modifiers: directive.modifiers };
  }

  const cases = [
    ["<div @foo-=\"handler\" />", {}, false],
    ["<div @click=\"foo($event)\"/>", { prefixIdentifiers: true, isTS: true }, false],
    [
      "<div @click=\"foo\"/>",
      { prefixIdentifiers: true, isTS: true, cacheHandlers: true },
      false,
    ],
    ["<div @click.stop.once=\"foo\"/>", {}, true],
    ["<div @click />", {}, false],
  ];
  for (const args of cases) {
    assert.deepStrictEqual(apply(candidate, ...args), apply(oracle, ...args), args[0]);
  }
});

test("transformVBindShorthand preserves expansion, camelization, locations, errors, and reflection", () => {
  assert.deepStrictEqual(
    [candidate.transformVBindShorthand.name, candidate.transformVBindShorthand.length],
    [oracle.transformVBindShorthand.name, oracle.transformVBindShorthand.length],
  );

  function apply(implementation, source) {
    const errors = [];
    const root = implementation.baseParse(source);
    const node = root.children[0];
    const directive = node.props[0];
    const argument = directive.arg;
    const argumentLocation = argument.loc;
    const context = implementation.createTransformContext(root, {
      onError(error) {
        errors.push({ code: error.code, loc: error.loc });
      },
    });
    implementation.transformVBindShorthand(node, context);
    return {
      directive,
      errors,
      argumentPreserved: directive.arg === argument,
      expressionUsesArgumentLocation: directive.exp?.loc === argumentLocation,
    };
  }

  for (const source of [
    "<div :id />",
    "<div v-bind:foo-bar />",
    "<div :foo--bar />",
    "<div .foo-bar />",
    "<div v-bind:[arg] />",
    "<div :123 />",
  ]) {
    assert.deepStrictEqual(apply(candidate, source), apply(oracle, source), source);
  }

  const native = apply(candidate, '<div :foo-bar="" />');
  assert.equal(native.directive.exp.content, "fooBar");
  assert.equal(native.directive.exp.isStatic, false);
  assert.equal(native.expressionUsesArgumentLocation, true);
  assert.deepStrictEqual(native.errors, []);
});

test("transformModel preserves assignment analysis, props, modifiers, caching, errors, locations, and reflection", () => {
  assert.deepStrictEqual(
    [candidate.transformModel.name, candidate.transformModel.length],
    [oracle.transformModel.name, oracle.transformModel.length],
  );

  function apply(implementation, source, options = {}, scopeIdentifier) {
    const errors = [];
    const root = implementation.baseParse(source);
    const node = root.children[0];
    const directive = node.props[0];
    const context = implementation.createTransformContext(root, {
      ...options,
      onError(error) {
        errors.push({ code: error.code, loc: error.loc });
      },
    });
    if (scopeIdentifier) context.identifiers[scopeIdentifier] = 1;
    const result = implementation.transformModel(directive, node, context);
    return { result, cached: context.cached, errors };
  }

  const cases = [
    ['<input v-model="model" />'],
    ['<input v-model="model[index]" />'],
    ['<Comp v-model:foo-value.trim.bar-baz="model" />'],
    ['<Comp v-model:[key].trim="model" />'],
    [
      '<input v-model="model" />',
      { inline: true, bindingMetadata: { model: candidate.BindingTypes.SETUP_REF } },
    ],
    [
      '<input v-model="model" />',
      {
        inline: true,
        isTS: true,
        bindingMetadata: { model: candidate.BindingTypes.SETUP_LET },
      },
    ],
    [
      '<input v-model="model" />',
      {
        inline: true,
        bindingMetadata: { model: candidate.BindingTypes.SETUP_MAYBE_REF },
      },
    ],
    [
      '<input v-model="model" />',
      { prefixIdentifiers: true, cacheHandlers: true },
    ],
    [
      '<input v-model="model" />',
      { bindingMetadata: { model: candidate.BindingTypes.PROPS } },
    ],
    [
      '<input v-model="model" />',
      { bindingMetadata: { model: candidate.BindingTypes.SETUP_CONST } },
    ],
    ['<input v-model="a + b" />'],
    ['<input v-model />'],
    ['<input v-model="item" />', { prefixIdentifiers: true }, 'item'],
  ];
  for (const [source, options, scopeIdentifier] of cases) {
    assert.deepStrictEqual(
      apply(candidate, source, options, scopeIdentifier),
      apply(oracle, source, options, scopeIdentifier),
      source,
    );
  }
});

test("noop directive transform preserves shape, locations, and reflection", () => {
  const location = {
    start: { line: 2, column: 3, offset: 7 },
    end: { line: 2, column: 9, offset: 13 },
    source: "v-noop",
  };
  const directive = { loc: location };
  const node = { loc: location };
  const context = { loc: location };

  assert.deepStrictEqual(
    candidate.noopDirectiveTransform(directive, node, context),
    oracle.noopDirectiveTransform(directive, node, context),
  );
  assert.equal(directive.loc, location);
  assert.equal(node.loc, location);
  assert.equal(context.loc, location);
  assert.deepStrictEqual(
    [candidate.noopDirectiveTransform.name, candidate.noopDirectiveTransform.length],
    [oracle.noopDirectiveTransform.name, oracle.noopDirectiveTransform.length],
  );
});

test("compat filter transform exactly rewrites scanned and nested expressions", () => {
  const candidateTransform = candidate.getBaseTransformPreset(true)[0][5];
  const oracleTransform = oracle.getBaseTransformPreset(true)[0][5];
  assert.deepStrictEqual(
    [candidateTransform.name, candidateTransform.length],
    [oracleTransform.name, oracleTransform.length],
  );

  const location = {
    start: { line: 2, column: 4, offset: 8 },
    end: { line: 2, column: 40, offset: 44 },
    source: "filter expression",
  };
  const sources = [
    "value / total | percent",
    "/a|b\\// | regex",
    `"a | b" + 'c | d' | quoted`,
    "`a | ${value}` | template",
    "value | first | second(arg)",
  ];
  for (const source of [
    ...sources.map(expression => `{{ ${expression} }}`),
    '<div :id="value | normalize" />',
  ]) {
    const options = {
      compatConfig: { MODE: 2 },
      prefixIdentifiers: true,
      onWarn() {},
    };
    assert.equal(
      candidate.baseCompile(source, options).code,
      oracle.baseCompile(source, options).code,
      source,
    );
  }

  function run(implementation, transform) {
    const simple = source => ({
      type: implementation.NodeTypes.SIMPLE_EXPRESSION,
      content: source,
      isStatic: false,
      constType: implementation.ConstantTypes.NOT_CONSTANT,
      loc: location,
      ast: { stale: true },
    });
    const node = {
      type: implementation.NodeTypes.INTERPOLATION,
      content: {
        type: implementation.NodeTypes.COMPOUND_EXPRESSION,
        children: [
          simple(sources[0]),
          " + ",
          {
            type: implementation.NodeTypes.COMPOUND_EXPRESSION,
            children: [simple(sources[1]), " + ", simple(sources[2])],
            loc: location,
          },
          " + ",
          {
            type: implementation.NodeTypes.INTERPOLATION,
            content: {
              type: implementation.NodeTypes.COMPOUND_EXPRESSION,
              children: [simple(sources[3]), " + ", simple(sources[4])],
              loc: location,
            },
            loc: location,
          },
        ],
        loc: location,
      },
      loc: location,
    };
    const helpers = [];
    const filters = new Set();
    const warnings = [];
    transform(node, {
      compatConfig: { MODE: 2 },
      filters,
      helper(symbol) {
        helpers.push(implementation.helperNameMap[symbol]);
        return symbol;
      },
      onWarn(warning) {
        warnings.push({ code: warning.code, loc: warning.loc });
      },
    });
    return { node, helpers, filters: [...filters], warnings };
  }

  assert.deepStrictEqual(
    run(candidate, candidateTransform),
    run(oracle, oracleTransform),
  );
});

test("compiler compat configuration matches Vue warnings, modes, and reflection", () => {
  assert.deepStrictEqual(candidate.CompilerDeprecationTypes, oracle.CompilerDeprecationTypes);
  assert.deepStrictEqual(
    [candidate.isCompatEnabled.name, candidate.isCompatEnabled.length],
    ["isCompatEnabled", 2],
  );
  for (const name of ["checkCompatEnabled", "warnDeprecation"]) {
    assert.deepStrictEqual(
      [candidate[name].name, candidate[name].length],
      [oracle[name].name, oracle[name].length],
      name,
    );
  }

  const filter = candidate.CompilerDeprecationTypes.COMPILER_FILTERS;
  for (const [compatConfig, expected] of [
    [undefined, false],
    [{}, false],
    [{ MODE: 3 }, false],
    [{ MODE: 3, [filter]: true }, true],
    [{ MODE: 2 }, true],
    [{ MODE: 2, [filter]: false }, false],
    [{ MODE: 2, [filter]: "suppress-warning" }, true],
  ]) {
    assert.equal(candidate.isCompatEnabled(filter, { compatConfig }), expected);
  }

  const location = {
    start: { line: 1, column: 1, offset: 0 },
    end: { line: 1, column: 2, offset: 1 },
    source: "x",
  };
  const normalize = error => ({
    name: error.name,
    message: error.message,
    code: error.code,
    loc: error.loc,
  });
  function warnings(implementation, method, key, compatConfig, ...args) {
    const result = [];
    const context = { compatConfig, onWarn: warning => result.push(normalize(warning)) };
    const enabled = implementation[method](key, context, location, ...args);
    return { enabled, result };
  }

  for (const key of Object.values(candidate.CompilerDeprecationTypes)) {
    const args = key === candidate.CompilerDeprecationTypes.COMPILER_V_BIND_SYNC
      ? ["value"]
      : [];
    assert.deepStrictEqual(
      warnings(candidate, "warnDeprecation", key, { MODE: 2 }, ...args),
      warnings(oracle, "warnDeprecation", key, { MODE: 2 }, ...args),
      key,
    );
  }
  for (const compatConfig of [
    { MODE: 2 },
    { MODE: 2, [filter]: false },
    { MODE: 2, [filter]: "suppress-warning" },
  ]) {
    assert.deepStrictEqual(
      warnings(candidate, "checkCompatEnabled", filter, compatConfig),
      warnings(oracle, "checkCompatEnabled", filter, compatConfig),
    );
  }
  const sync = candidate.CompilerDeprecationTypes.COMPILER_V_BIND_SYNC;
  assert.deepStrictEqual(
    warnings(candidate, "checkCompatEnabled", sync, { MODE: 2 }, "value"),
    warnings(oracle, "checkCompatEnabled", sync, { MODE: 2 }, "value"),
  );
});
