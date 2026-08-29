import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { JSDOM } from "jsdom";
import * as candidate from "../packages/vuelil/compiler-dom.js";
import * as candidateCore from "../packages/vuelil/compiler-core.js";
import * as oracle from "@vue/compiler-dom";

const compilerCoreEvidence = JSON.parse(
  readFileSync(new URL("../compatibility/compiler-core.json", import.meta.url), "utf8"),
);

test("compiler-dom facade preserves the upstream surface and compiler-core identities", () => {
  const oracleExports = Object.keys(oracle)
    .filter(name => !["__esModule", "default", "module.exports"].includes(name))
    .sort();
  assert.deepStrictEqual(Object.keys(candidate).sort(), oracleExports);
  for (const name of compilerCoreEvidence.runtimeExports) {
    assert.equal(candidate[name], candidateCore[name], name);
  }
  for (const name of ["compile", "parse", "createDOMCompilerError", "transformStyle"]) {
    assert.deepStrictEqual(
      [candidate[name].name, candidate[name].length],
      [oracle[name].name, oracle[name].length],
      name,
    );
  }
});

test("DOM errors preserve enum shape, messages, locations, and reflection", () => {
  assert.deepStrictEqual(candidate.DOMErrorCodes, oracle.DOMErrorCodes);
  assert.deepStrictEqual(candidate.DOMErrorMessages, oracle.DOMErrorMessages);
  assert.equal(
    candidate.DOMErrorCodes.X_V_HTML_NO_EXPRESSION,
    candidateCore.ErrorCodes.__EXTEND_POINT__,
  );
  assert.deepStrictEqual(
    [candidate.createDOMCompilerError.name, candidate.createDOMCompilerError.length],
    [oracle.createDOMCompilerError.name, oracle.createDOMCompilerError.length],
  );

  const codes = Object.values(oracle.DOMErrorCodes).filter(
    value => typeof value === "number",
  );
  for (const code of codes) {
    for (const loc of [undefined, {
      start: { line: 2, column: 3, offset: 4 },
      end: { line: 5, column: 6, offset: 7 },
      source: "source",
    }]) {
      const actual = candidate.createDOMCompilerError(code, loc);
      const expected = oracle.createDOMCompilerError(code, loc);
      assert.equal(actual.constructor, expected.constructor, `${code} constructor`);
      assert.equal(actual.name, expected.name, `${code} name`);
      assert.equal(actual.message, expected.message, `${code} message`);
      assert.equal(actual.code, expected.code, `${code} code`);
      assert.equal(actual.loc, loc, `${code} location identity`);
      assert.deepStrictEqual(Object.keys(actual), Object.keys(expected), `${code} keys`);
    }
  }
});

test("DOM transforms emit errors from the shared error owner with exact locations", () => {
  const vectors = [
    "<div v-html></div>",
    '<div v-html="html">child</div>',
    "<div v-text></div>",
    '<div v-text="text">child</div>',
    '<div v-model="model" />',
    '<input v-model:argument="model" />',
    '<input type="file" v-model="model" />',
    '<input v-model="model" :value="model" />',
    "<div v-show />",
    "<Transition><div/><div/></Transition>",
    "<script>sideEffect()</script>",
  ];

  function compileErrors(implementation, source) {
    const errors = [];
    implementation.compile(source, {
      onError(error) {
        errors.push({ code: error.code, message: error.message, loc: error.loc });
      },
    });
    return errors;
  }

  for (const source of vectors) {
    assert.deepStrictEqual(
      compileErrors(candidate, source),
      compileErrors(oracle, source),
      source,
    );
  }
});

test("Transition owns matching, validation, persisted props, runtime helpers, diagnostics, reflection, and ordering", async () => {
  const internalCandidate = await import(
    `./compiler-dom-upstream.candidate.mjs?transition=${Date.now()}`
  );
  const transform = internalCandidate.transformTransition;
  const oracleTransform = oracle.DOMNodeTransforms[1];

  assert.equal(internalCandidate.DOMNodeTransforms[1], transform);
  assert.equal("transformTransition" in candidate, false);
  assert.equal("transformTransition" in oracle, false);
  assert.deepStrictEqual(
    internalCandidate.DOMNodeTransforms.map(fn => [fn.name, fn.length]),
    oracle.DOMNodeTransforms.map(fn => [fn.name, fn.length]),
  );
  assert.deepStrictEqual(
    [transform.name, transform.length],
    [oracleTransform.name, oracleTransform.length],
  );

  for (const [implementation, transitionTransform] of [
    [internalCandidate, transform],
    [oracle, oracleTransform],
  ]) {
    let matcherCalls = 0;
    const element = implementation.parse("<div/>").children[0];
    assert.equal(
      transitionTransform(element, {
        isBuiltInComponent() {
          matcherCalls += 1;
        },
      }),
      undefined,
    );
    assert.equal(matcherCalls, 0);

    const nonTransition = implementation.parse("<Transition><div/></Transition>")
      .children[0];
    assert.equal(
      transitionTransform(nonTransition, {
        isBuiltInComponent() {
          return implementation.TRANSITION_GROUP;
        },
      }),
      undefined,
    );

    const node = implementation.parse(
      "<Transition><!-- before --><div/><div/><!-- after --></Transition>",
    ).children[0];
    const first = node.children[1];
    const last = node.children[2];
    let matcherReceiver;
    let errorReceiver;
    let error;
    let warnings = 0;
    const context = {
      isBuiltInComponent(tag) {
        matcherReceiver = this;
        assert.equal(tag, "Transition");
        return implementation.TRANSITION;
      },
      onError(value) {
        errorReceiver = this;
        error = value;
      },
      onWarn() {
        warnings += 1;
      },
    };
    const exit = transitionTransform(node, context);
    assert.equal(typeof exit, "function");
    assert.equal(error, undefined);
    exit();

    assert.equal(matcherReceiver, context);
    assert.equal(errorReceiver, context);
    assert.equal(warnings, 0);
    assert.deepStrictEqual(node.children, [first, last]);
    assert.equal(error.code, implementation.DOMErrorCodes.X_TRANSITION_INVALID_CHILDREN);
    assert.equal(
      error.message,
      "<Transition> expects exactly one child element or component.",
    );
    assert.equal(error.loc.start, first.loc.start);
    assert.equal(error.loc.end, last.loc.end);
    assert.equal(error.loc.source, "");

    const persistedNode = implementation.parse(
      '<transition><div v-show="visible"/></transition>',
    ).children[0];
    const persistedExit = transitionTransform(persistedNode, {
      isBuiltInComponent(tag) {
        assert.equal(tag, "transition");
        return implementation.TRANSITION;
      },
      onError(value) {
        throw value;
      },
    });
    persistedExit();
    const persisted = persistedNode.props[0];
    assert.deepStrictEqual(Object.keys(persisted), [
      "type",
      "name",
      "nameLoc",
      "value",
      "loc",
    ]);
    assert.equal(persisted.type, candidate.NodeTypes.ATTRIBUTE);
    assert.equal(persisted.name, "persisted");
    assert.equal(persisted.nameLoc, persistedNode.loc);
    assert.equal(persisted.value, undefined);
    assert.equal(persisted.loc, persistedNode.loc);
  }

  function compileTransition(implementation, source, options = {}) {
    const errors = [];
    const warnings = [];
    const result = implementation.compile(source, {
      ...options,
      onError(error) {
        errors.push({ code: error.code, message: error.message, loc: error.loc });
      },
      onWarn(warning) {
        warnings.push({ message: warning.message, loc: warning.loc });
      },
    });
    const transition = result.ast.children[0];
    return {
      code: result.code,
      errors,
      warnings,
      childTypes: transition.children.map(child => child.type),
      persisted: transition.props.some(prop => prop.name === "persisted"),
      registeredTransition: result.ast.helpers.has(implementation.TRANSITION),
    };
  }

  for (const [source, options] of [
    ["<Transition><div/></Transition>"],
    ['<transition><div v-show="visible"/></transition>'],
    ["<Transition><div/><div/></Transition>"],
    ['<Transition><div v-for="item in items"/></Transition>'],
    ['<Transition><template v-if="ok"/></Transition>'],
    ['<Transition><div v-if="ok"/><div v-else/></Transition>'],
    ["<Transition><!-- one --><div/><!-- two --></Transition>", { whitespace: "preserve" }],
    ["<Transition>\u00a0<div/></Transition>"],
    ["<TransitionGroup><div/><div/></TransitionGroup>"],
  ]) {
    assert.deepStrictEqual(
      compileTransition(candidate, source, options),
      compileTransition(oracle, source, options),
      source,
    );
  }

  function compileWithOrdering(implementation) {
    const trace = [];
    const result = implementation.compile(
      '<Transition><div v-show="visible"/></Transition>',
      {
        nodeTransforms: [node => {
          if (
            node.type === implementation.NodeTypes.ELEMENT &&
            node.tag === "Transition"
          ) {
            trace.push(`enter:${node.props.some(prop => prop.name === "persisted")}`);
            return () => {
              trace.push(`exit:${node.props.some(prop => prop.name === "persisted")}`);
            };
          }
        }],
      },
    );
    return {
      trace,
      persisted: result.ast.children[0].props.some(prop => prop.name === "persisted"),
    };
  }

  const ordered = compileWithOrdering(candidate);
  assert.deepStrictEqual(ordered, compileWithOrdering(oracle));
  assert.deepStrictEqual(ordered, {
    trace: ["enter:false", "exit:false"],
    persisted: true,
  });
});

test("ignore-side-effect tags owns detection, diagnostics, removal, locations, reflection, and ordering", async () => {
  const internalCandidate = await import(
    `./compiler-dom-upstream.candidate.mjs?side-effects=${Date.now()}`
  );
  const transform = internalCandidate.ignoreSideEffectTags;
  assert.deepStrictEqual(
    [transform.name, transform.length],
    ["ignoreSideEffectTags", 2],
  );
  assert.equal(internalCandidate.DOMNodeTransforms.includes(transform), false);

  for (const tag of ["script", "style"]) {
    const node = candidate.parse(`<${tag}>sideEffect()</${tag}>`).children[0];
    const events = [];
    let error;
    transform(node, {
      onError(value) {
        events.push("error");
        error = value;
      },
      onWarn() {
        events.push("warning");
      },
      removeNode() {
        events.push("remove");
      },
    });
    const expected = oracle.createDOMCompilerError(
      oracle.DOMErrorCodes.X_IGNORED_SIDE_EFFECT_TAG,
      node.loc,
    );
    assert.deepStrictEqual(events, ["error", "remove"], tag);
    assert.equal(error.code, expected.code, tag);
    assert.equal(error.message, expected.message, tag);
    assert.equal(error.loc, node.loc, `${tag} location identity`);
    assert.deepStrictEqual(Object.keys(error), Object.keys(expected), tag);
  }

  for (const node of [
    {
      type: candidate.NodeTypes.ELEMENT,
      tagType: candidate.ElementTypes.ELEMENT,
      tag: "SCRIPT",
    },
    {
      type: candidate.NodeTypes.ELEMENT,
      tagType: candidate.ElementTypes.COMPONENT,
      tag: "script",
    },
    {
      type: candidate.NodeTypes.ELEMENT,
      tagType: candidate.ElementTypes.ELEMENT,
      tag: "scripted",
    },
    {
      type: candidate.NodeTypes.ROOT,
      tagType: candidate.ElementTypes.ELEMENT,
      tag: "style",
    },
  ]) {
    let calls = 0;
    transform(node, {
      onError() {
        calls += 1;
      },
      onWarn() {
        calls += 1;
      },
      removeNode() {
        calls += 1;
      },
    });
    assert.equal(calls, 0, JSON.stringify(node));
  }

  let removedAfterThrow = false;
  assert.throws(
    () => transform(
      {
        type: candidate.NodeTypes.ELEMENT,
        tagType: candidate.ElementTypes.ELEMENT,
        tag: "script",
        loc: candidate.locStub,
      },
      {
        onError(error) {
          throw error;
        },
        removeNode() {
          removedAfterThrow = true;
        },
      },
    ),
    error => error.code === candidate.DOMErrorCodes.X_IGNORED_SIDE_EFFECT_TAG,
  );
  assert.equal(removedAfterThrow, false);

  function compileWithTrace(implementation) {
    const errors = [];
    const warnings = [];
    const events = [];
    const result = implementation.compile(
      '<script>run()</script><div style="color: red"/><style>.x{}</style><Script/>',
      {
        onError(error) {
          events.push(`error:${error.code}`);
          errors.push({ code: error.code, message: error.message, loc: error.loc });
        },
        onWarn(warning) {
          events.push("warning");
          warnings.push({ message: warning.message, loc: warning.loc });
        },
        nodeTransforms: [node => {
          if (node.type === implementation.NodeTypes.ROOT) {
            events.push("custom:root");
          } else if (node.type === implementation.NodeTypes.ELEMENT) {
            events.push(`custom:${node.tag}:${node.props[0]?.name ?? "none"}`);
          }
        }],
      },
    );
    return {
      code: result.code,
      children: result.ast.children.map(node => node.tag),
      errors,
      warnings,
      events,
    };
  }

  const actual = compileWithTrace(candidate);
  assert.deepStrictEqual(actual, compileWithTrace(oracle));
  assert.deepStrictEqual(actual.children, ["div", "Script"]);
  assert.deepStrictEqual(actual.warnings, []);
  assert.deepStrictEqual(actual.events, [
    "custom:root",
    `error:${candidate.DOMErrorCodes.X_IGNORED_SIDE_EFFECT_TAG}`,
    "custom:div:bind",
    `error:${candidate.DOMErrorCodes.X_IGNORED_SIDE_EFFECT_TAG}`,
    "custom:Script:none",
  ]);
});

test("style transform owns static detection, parsed expressions, locations, node wiring, and reflection", async () => {
  const internalCandidate = await import(
    `./compiler-dom-upstream.candidate.mjs?style=${Date.now()}`
  );
  assert.equal(candidate.DOMNodeTransforms[0], candidate.transformStyle);
  assert.equal(
    internalCandidate.DOMNodeTransforms[0],
    internalCandidate.transformStyle,
  );
  assert.deepStrictEqual(
    [candidate.transformStyle.name, candidate.transformStyle.length],
    [oracle.transformStyle.name, oracle.transformStyle.length],
  );
  assert.deepStrictEqual(
    [internalCandidate.transformStyle.name, internalCandidate.transformStyle.length],
    [oracle.transformStyle.name, oracle.transformStyle.length],
  );

  function apply(implementation, source) {
    const root = implementation.parse(source);
    const node = root.children[0];
    const originalProps = [...node.props];
    implementation.transformStyle(node);
    return {
      props: node.props,
      replacements: node.props.map((prop, index) => prop !== originalProps[index]),
      transformedLocations: node.props.map((prop, index) =>
        prop !== originalProps[index]
          ? [
              prop.loc === originalProps[index].loc,
              prop.arg.loc === originalProps[index].loc,
              prop.exp.loc === originalProps[index].loc,
            ]
          : undefined,
      ),
    };
  }

  for (const source of [
    '<div style="color: red; font-size: 12px"/>',
    '<div style="background: url(&quot;data:image/png;base64,a;b&quot;); --theme: a:b"/>',
    "<div style/>",
    '<div :style="styles"/>',
    '<div id="app"/>',
  ]) {
    assert.deepStrictEqual(apply(candidate, source), apply(oracle, source), source);
  }

  const candidateRoot = candidate.parse('<div style="color: red"/>');
  const oracleRoot = oracle.parse('<div style="color: red"/>');
  candidate.transformStyle(candidateRoot);
  oracle.transformStyle(oracleRoot);
  assert.deepStrictEqual(candidateRoot, oracleRoot);

  function compileStyle(implementation) {
    const result = implementation.compile('<div style="color: red"/>');
    const vnode = result.ast.children[0].codegenNode;
    return {
      code: result.code,
      props: vnode.props,
      patchFlag: vnode.patchFlag,
    };
  }
  const compiled = compileStyle(candidate);
  assert.deepStrictEqual(compiled, compileStyle(oracle));
  assert.equal(compiled.props.type, candidate.NodeTypes.JS_OBJECT_EXPRESSION);
});

test("v-html owns errors, child removal, property AST, locations, map wiring, and reflection", async () => {
  const candidateTransform = candidate.DOMDirectiveTransforms.html;
  const oracleTransform = oracle.DOMDirectiveTransforms.html;
  const internalCandidate = await import(
    `./compiler-dom-upstream.candidate.mjs?vhtml=${Date.now()}`
  );
  assert.equal(
    internalCandidate.DOMDirectiveTransforms.html,
    internalCandidate.transformVHtml,
  );
  assert.deepStrictEqual(
    [candidateTransform.name, candidateTransform.length],
    [oracleTransform.name, oracleTransform.length],
  );
  assert.deepStrictEqual(
    [internalCandidate.transformVHtml.name, internalCandidate.transformVHtml.length],
    [oracleTransform.name, oracleTransform.length],
  );

  function apply(implementation, source) {
    const errors = [];
    const root = implementation.parse(source);
    const node = root.children[0];
    const directive = node.props[0];
    const expression = directive.exp;
    const result = implementation.DOMDirectiveTransforms.html(
      directive,
      node,
      {
        onError(error) {
          errors.push({ code: error.code, message: error.message, loc: error.loc });
        },
      },
    );
    const property = result.props[0];
    return {
      result,
      errors,
      children: node.children,
      keyUsesDirectiveLocation: property.key.loc === directive.loc,
      valueUsesDirectiveExpression: expression
        ? property.value === expression
        : undefined,
    };
  }

  for (const source of [
    '<div v-html="test"/>',
    '<div v-html="test">hello</div>',
    "<div v-html></div>",
    "<div v-html>hello</div>",
  ]) {
    assert.deepStrictEqual(apply(candidate, source), apply(oracle, source), source);
  }
});

test("v-text owns errors, textContent AST, display conversion, constants, children, locations, map wiring, and reflection", async () => {
  const candidateTransform = candidate.DOMDirectiveTransforms.text;
  const oracleTransform = oracle.DOMDirectiveTransforms.text;
  const internalCandidate = await import(
    `./compiler-dom-upstream.candidate.mjs?vtext=${Date.now()}`
  );
  assert.equal(
    internalCandidate.DOMDirectiveTransforms.text,
    internalCandidate.transformVText,
  );
  assert.deepStrictEqual(
    [candidateTransform.name, candidateTransform.length],
    [oracleTransform.name, oracleTransform.length],
  );
  assert.deepStrictEqual(
    [internalCandidate.transformVText.name, internalCandidate.transformVText.length],
    [oracleTransform.name, oracleTransform.length],
  );

  function apply(implementation, source, constType) {
    const errors = [];
    const helpers = [];
    const root = implementation.parse(source);
    const node = root.children[0];
    const directive = node.props[0];
    if (directive.exp && constType !== undefined) directive.exp.constType = constType;
    const expression = directive.exp;
    const result = implementation.DOMDirectiveTransforms.text(
      directive,
      node,
      {
        constantCache: new WeakMap(),
        helperString(helper) {
          const name = implementation.helperNameMap[helper];
          helpers.push(name);
          return `_${name}`;
        },
        onError(error) {
          errors.push({
            code: error.code,
            message: error.message,
            loc: error.loc,
            usesDirectiveLocation: error.loc === directive.loc,
          });
        },
      },
    );
    const property = result.props[0];
    return {
      result,
      errors,
      helpers,
      children: node.children,
      propertyUsesLocStub: property.loc === implementation.locStub,
      keyUsesLocStub: property.key.loc === implementation.locStub,
      valueUsesDirectiveExpression: expression
        ? property.value === expression
        : undefined,
      callUsesDirectiveLocation: property.value.type === 14
        ? property.value.loc === directive.loc
        : undefined,
      fallbackUsesLocStub: expression
        ? undefined
        : property.value.loc === implementation.locStub,
    };
  }

  for (const [source, constType] of [
    ['<div v-text="test"/>', undefined],
    ['<div v-text="test"/>', 3],
    ["<div v-text></div>", undefined],
    ['<div v-text="test">hello<span/></div>', undefined],
  ]) {
    assert.deepStrictEqual(
      apply(candidate, source, constType),
      apply(oracle, source, constType),
      `${source} constType=${constType}`,
    );
  }
});

test("v-model owns native selection, diagnostics, SSR delegation, AST identities, map wiring, and reflection", async () => {
  const candidateTransform = candidate.DOMDirectiveTransforms.model;
  const oracleTransform = oracle.DOMDirectiveTransforms.model;
  const internalCandidate = await import(
    `./compiler-dom-upstream.candidate.mjs?vmodel=${Date.now()}`
  );

  assert.equal(
    internalCandidate.DOMDirectiveTransforms.model,
    internalCandidate.transformModel,
  );
  assert.notEqual(candidateTransform, candidate.transformModel);
  assert.notEqual(oracleTransform, oracle.transformModel);
  assert.deepStrictEqual(
    [candidateTransform.name, candidateTransform.length],
    [oracleTransform.name, oracleTransform.length],
  );
  assert.deepStrictEqual(
    [internalCandidate.transformModel.name, internalCandidate.transformModel.length],
    [oracleTransform.name, oracleTransform.length],
  );

  function apply(implementation, source, rawOptions = {}) {
    const { customElement, ...options } = rawOptions;
    const root = implementation.baseParse(source);
    const node = root.children[0];
    const directive = node.props.find(
      prop =>
        prop.type === implementation.NodeTypes.DIRECTIVE &&
        prop.name === "model",
    );
    const value = node.props.find(
      prop =>
        prop.type === implementation.NodeTypes.DIRECTIVE &&
        prop.name === "bind" &&
        prop.arg?.isStatic &&
        prop.arg.content === "value",
    );
    const errors = [];
    const helpers = [];
    const helperResult = {};
    const helperReceivers = [];
    const customElementReceivers = [];
    let context;
    context = implementation.createTransformContext(root, {
      ...options,
      isCustomElement(tag) {
        customElementReceivers.push(this === context);
        return tag === customElement;
      },
      onError(error) {
        errors.push({
          code: error.code,
          message: error.message,
          keys: Object.keys(error),
          loc: error.loc,
          receiverIsContext: this === context,
          usesArgumentLocation: error.loc === directive.arg?.loc,
          usesDirectiveLocation: error.loc === directive.loc,
          usesValueLocation: error.loc === value?.loc,
        });
      },
    });
    context.helper = function helper(value) {
      helperReceivers.push(this === context);
      helpers.push(implementation.helperNameMap[value]);
      return helperResult;
    };

    const result = implementation.DOMDirectiveTransforms.model(
      directive,
      node,
      context,
    );
    const update = result.props.find(
      prop => prop.key?.content === "onUpdate:modelValue",
    );
    return {
      resultKeys: Object.keys(result),
      props: result.props,
      errors,
      helpers,
      helperReceivers,
      customElementReceivers,
      needRuntimeUsesHelperResult: result.needRuntime === helperResult,
      containsModelValue: result.props.some(
        prop => prop.key?.content === "modelValue",
      ),
      updateUsesDirectiveExpression:
        update?.value?.children?.includes(directive.exp) ?? false,
      updatePropertyUsesLocStub: update?.loc === implementation.locStub,
      updateKeyUsesLocStub: update?.key?.loc === implementation.locStub,
      updateValueUsesLocStub: update?.value?.loc === implementation.locStub,
    };
  }

  const cases = [
    ['<input v-model="model"/>', {}, "vModelText"],
    ['<input type="text" v-model="model"/>', {}, "vModelText"],
    ['<input type="radio" v-model="model"/>', {}, "vModelRadio"],
    ['<input type="checkbox" v-model="model"/>', {}, "vModelCheckbox"],
    ['<input :type="kind" v-model="model"/>', {}, "vModelDynamic"],
    ['<input v-bind="attrs" v-model="model"/>', {}, "vModelDynamic"],
    ['<input v-bind:[key]="value" v-model="model"/>', {}, "vModelDynamic"],
    ['<select v-model="model"/>', {}, "vModelSelect"],
    ['<textarea v-model="model"/>', {}, "vModelText"],
    ['<my-input v-model="model"/>', { customElement: "my-input" }, "vModelText"],
    [
      '<my-input type="checkbox" v-model="model"/>',
      { customElement: "my-input" },
      "vModelCheckbox",
    ],
    ['<input v-model="model"/>', { ssr: true, inSSR: true }, "vModelText"],
    ['<Comp v-model="model"/>', { ssr: true, inSSR: true }, undefined],
  ];
  for (const [source, options, runtime] of cases) {
    const actual = apply(candidate, source, options);
    assert.deepStrictEqual(actual, apply(oracle, source, options), source);
    assert.deepStrictEqual(actual.helpers, runtime ? [runtime] : [], source);
    assert.equal(actual.needRuntimeUsesHelperResult, runtime !== undefined, source);
    assert.equal(actual.containsModelValue, runtime === undefined, source);
    assert.equal(actual.updateUsesDirectiveExpression, true, source);
    assert.equal(actual.updatePropertyUsesLocStub, true, source);
    assert.equal(actual.updateKeyUsesLocStub, true, source);
    assert.equal(actual.updateValueUsesLocStub, true, source);
  }

  for (const source of [
    '<input v-model:value="model"/>',
    '<input type="file" v-model="model"/>',
    '<input v-model="model" :value="other"/>',
    '<textarea v-model="model" :value="other"/>',
    '<span v-model="model"/>',
    '<input v-model="model" value="static"/>',
  ]) {
    assert.deepStrictEqual(apply(candidate, source), apply(oracle, source), source);
  }

  const argumentError = apply(candidate, '<input v-model:value="model"/>');
  assert.equal(argumentError.errors[0].usesArgumentLocation, true);
  assert.equal(argumentError.errors[0].receiverIsContext, true);

  const fileError = apply(candidate, '<input type="file" v-model="model"/>');
  assert.deepStrictEqual(fileError.helpers, []);
  assert.equal(fileError.errors[0].usesDirectiveLocation, true);

  const duplicateError = apply(
    candidate,
    '<input v-model="model" :value="other"/>',
  );
  assert.equal(duplicateError.errors[0].usesValueLocation, true);

  const invalidError = apply(candidate, '<span v-model="model"/>');
  assert.equal(invalidError.errors[0].usesDirectiveLocation, true);
});

test("v-on owns modifiers, event rewrites, helper wrapping, caching, AST locations, map wiring, and reflection", async () => {
  const candidateTransform = candidate.DOMDirectiveTransforms.on;
  const oracleTransform = oracle.DOMDirectiveTransforms.on;
  const internalCandidate = await import(
    `./compiler-dom-upstream.candidate.mjs?von=${Date.now()}`
  );
  assert.equal(
    internalCandidate.DOMDirectiveTransforms.on,
    internalCandidate.transformOn,
  );
  assert.notEqual(candidateTransform, candidate.transformOn);
  assert.notEqual(oracleTransform, oracle.transformOn);
  assert.deepStrictEqual(
    [candidateTransform.name, candidateTransform.length],
    [oracleTransform.name, oracleTransform.length],
  );
  assert.deepStrictEqual(
    [internalCandidate.transformOn.name, internalCandidate.transformOn.length],
    [oracleTransform.name, oracleTransform.length],
  );

  function apply(source, cacheHandlers = false) {
    const root = candidate.baseParse(source);
    const node = root.children[0];
    const directive = node.props[0];
    const helpers = [];
    let cacheInput;
    const cached = { type: candidate.NodeTypes.JS_CACHE_EXPRESSION };
    const result = candidateTransform(directive, node, {
      cacheHandlers,
      inVOnce: false,
      prefixIdentifiers: false,
      identifiers: {},
      expressionPlugins: [],
      isTS: false,
      helper(helper) {
        helpers.push(candidate.helperNameMap[helper]);
        return helper;
      },
      helperString(helper) {
        return `_${candidate.helperNameMap[helper]}`;
      },
      cache(value) {
        cacheInput = value;
        return cached;
      },
      onError(error) {
        throw error;
      },
      onWarn(error) {
        throw error;
      },
    });
    return { directive, result, helpers, cacheInput, cached };
  }

  const guarded = apply('<button @click.stop.capture="run"/>');
  const guardedProperty = guarded.result.props[0];
  assert.deepStrictEqual(guarded.helpers, ["withModifiers"]);
  assert.equal(guardedProperty.key.content, "onClickCapture");
  assert.equal(guardedProperty.key.loc, candidate.locStub);
  assert.equal(guardedProperty.loc, candidate.locStub);
  assert.equal(guardedProperty.value.loc, candidate.locStub);
  assert.equal(guardedProperty.value.arguments[0], guarded.directive.exp);
  assert.equal(guardedProperty.value.arguments[1], '["stop"]');

  for (const [source, expected] of [
    ['<button @click.right="run"/>', "onContextmenu"],
    ['<button @click.middle="run"/>', "onMouseup"],
  ]) {
    const transformed = apply(source);
    assert.equal(transformed.result.props[0].key.content, expected);
    assert.equal(transformed.result.props[0].key.loc, candidate.locStub);
  }

  const cachedResult = apply("<input @keyup.enter.capture/>", true);
  const cachedProperty = cachedResult.result.props[0];
  assert.deepStrictEqual(cachedResult.helpers, ["withKeys"]);
  assert.equal(cachedProperty.key.content, "onKeyupCapture");
  assert.equal(cachedProperty.value, cachedResult.cached);
  assert.equal(cachedResult.cacheInput.callee, candidate.V_ON_WITH_KEYS);
  assert.equal(
    cachedResult.cacheInput.arguments[0].loc,
    cachedResult.directive.loc,
  );
  assert.equal(cachedProperty.key.isHandlerKey, true);
});

test("v-show owns errors, runtime registration, empty props, needRuntime identity, locations, map wiring, and reflection", async () => {
  const candidateTransform = candidate.DOMDirectiveTransforms.show;
  const oracleTransform = oracle.DOMDirectiveTransforms.show;
  const internalCandidate = await import(
    `./compiler-dom-upstream.candidate.mjs?vshow=${Date.now()}`
  );
  assert.equal(
    internalCandidate.DOMDirectiveTransforms.show,
    internalCandidate.transformShow,
  );
  assert.deepStrictEqual(
    [candidateTransform.name, candidateTransform.length],
    [oracleTransform.name, oracleTransform.length],
  );
  assert.deepStrictEqual(
    [internalCandidate.transformShow.name, internalCandidate.transformShow.length],
    [oracleTransform.name, oracleTransform.length],
  );

  function apply(implementation, source) {
    const errors = [];
    const helpers = [];
    const runtime = {};
    const root = implementation.parse(source);
    const node = root.children[0];
    const directive = node.props[0];
    const result = implementation.DOMDirectiveTransforms.show(
      directive,
      node,
      {
        helper(helper) {
          helpers.push(helper);
          return runtime;
        },
        onError(error) {
          errors.push({
            code: error.code,
            message: error.message,
            loc: error.loc,
            usesDirectiveLocation: error.loc === directive.loc,
          });
        },
      },
    );
    return {
      resultKeys: Object.keys(result),
      props: result.props,
      errors,
      registeredVShowOnce:
        helpers.length === 1 && helpers[0] === implementation.V_SHOW,
      needRuntimeUsesHelperResult: result.needRuntime === runtime,
    };
  }

  for (const source of ['<div v-show="visible"/>', "<div v-show/>"]) {
    assert.deepStrictEqual(apply(candidate, source), apply(oracle, source), source);
  }
});

test("DOM runtime helpers retain unique identities and core registrations", () => {
  const helpers = {
    V_MODEL_RADIO: "vModelRadio",
    V_MODEL_CHECKBOX: "vModelCheckbox",
    V_MODEL_TEXT: "vModelText",
    V_MODEL_SELECT: "vModelSelect",
    V_MODEL_DYNAMIC: "vModelDynamic",
    V_ON_WITH_MODIFIERS: "withModifiers",
    V_ON_WITH_KEYS: "withKeys",
    V_SHOW: "vShow",
    TRANSITION: "Transition",
    TRANSITION_GROUP: "TransitionGroup",
  };
  const symbols = Object.keys(helpers).map(name => candidate[name]);
  assert.ok(symbols.every(value => typeof value === "symbol"));
  assert.equal(new Set(symbols).size, symbols.length);
  for (const [name, runtimeName] of Object.entries(helpers)) {
    assert.equal(candidate.helperNameMap[candidate[name]], runtimeName, name);
  }
  assert.equal(
    candidate.parserOptions.isBuiltInComponent("Transition"),
    candidate.TRANSITION,
  );
  assert.equal(
    candidate.parserOptions.isBuiltInComponent("transition-group"),
    candidate.TRANSITION_GROUP,
  );
});

test("DOM parsing preserves text modes, namespaces, entities, and errors", () => {
  const vectors = [
    "<textarea>some<div>&amp;</div>{{ value }}</textarea>",
    "<style>some<div>&amp;</div></style>",
    "<svg><![CDATA[some text]]><foreignObject><div/></foreignObject></svg>",
    '<math><annotation-xml encoding="text/html"><div/></annotation-xml></math>',
    "<pre>\n  keep   spacing</pre>",
    "<img>after",
    "<div>{{ a &lt; b }}</div>",
  ];
  for (const source of vectors) {
    assert.deepStrictEqual(candidate.parse(source), oracle.parse(source), source);
  }

  function parseWithErrors(implementation, source) {
    const errors = [];
    const ast = implementation.parse(source, {
      onError(error) {
        errors.push({ code: error.code, message: error.message, loc: error.loc });
      },
    });
    return { ast, errors };
  }
  for (const source of ["some text</div>", "<svg><![CDATA[unterminated"] ) {
    assert.deepStrictEqual(
      parseWithErrors(candidate, source),
      parseWithErrors(oracle, source),
      source,
    );
  }
});

test("browser entity decoding matches the upstream decoder vectors", async () => {
  const dom = new JSDOM("<!doctype html>");
  const previousDocument = globalThis.document;
  globalThis.document = dom.window.document;
  try {
    const { decodeHtmlBrowser } = await import(
      `./compiler-dom-upstream.candidate.mjs?decoder=${Date.now()}`
    );
    const vectors = [
      [" abc  123 ", false, " abc  123 "],
      ["&amp;amp;", false, "&amp;"],
      ["&Eacute;", false, "\u00c9"],
      ["&#x86;", false, "\u2020"],
      ["<strong>&amp;</strong>", true, "<strong>&</strong>"],
      ['"', true, '"'],
      ["'", true, "'"],
    ];
    for (const [raw, asAttr, expected] of vectors) {
      assert.equal(decodeHtmlBrowser(raw, asAttr), expected, raw);
    }
    assert.deepStrictEqual(
      [decodeHtmlBrowser.name, decodeHtmlBrowser.length],
      ["decodeHtmlBrowser", 1],
    );
  } finally {
    globalThis.document = previousDocument;
    dom.window.close();
  }
});

test("foundation compile composition matches supported upstream templates", () => {
  const vectors = [
    "<div/>",
    "<div>{{ value }}</div>",
    '<svg><foreignObject><div :id="value"/></foreignObject></svg>',
    '<button @click="run">go</button>',
  ];
  for (const source of vectors) {
    assert.equal(candidate.compile(source).code, oracle.compile(source).code, source);
  }
});

test("stringifyStatic preserves thresholds, reflection, default wiring, and scope IDs", async () => {
  const internalCandidate = await import(
    `./compiler-dom-upstream.candidate.mjs?stringify=${Date.now()}`
  );
  assert.deepStrictEqual(
    [internalCandidate.stringifyStatic.name, internalCandidate.stringifyStatic.length],
    ["stringifyStatic", 3],
  );
  assert.equal(internalCandidate.StringifyThresholds.ELEMENT_WITH_BINDING_COUNT, 5);
  assert.equal(internalCandidate.StringifyThresholds.NODE_COUNT, 20);

  const result = candidate.compile(`<div>${"<span/>".repeat(20)}</div>`, {
    hoistStatic: true,
    mode: "module",
    prefixIdentifiers: true,
    scopeId: "data-v-test",
    transformHoist() {
      throw new Error("compiler-dom must replace caller transformHoist");
    },
  });
  const staticCall = result.ast.cached[0].value.elements[0];
  assert.equal(staticCall.callee, candidate.CREATE_STATIC);
  assert.deepStrictEqual(staticCall.arguments, [
    JSON.stringify("<span data-v-test></span>".repeat(20)),
    "20",
  ]);
});
