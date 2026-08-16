import { readFileSync, writeFileSync } from "node:fs";
import { createCtx, lowerJsxToLil } from "./lower.mjs";
import { parseJsx } from "./parse-jsx.mjs";

const DOM_IMPORTS = [
  "DomEvent",
  "DomNode",
  "append",
  "appendComponentChildren",
  "attribute",
  "boolAttribute",
  "boolProperty",
  "classToggle",
  "childNodes",
  "cloneTemplate",
  "componentNode",
  "componentNodes",
  "componentProp",
  "componentProperty",
  "componentProps",
  "componentSpread",
  "createRenderEffect",
  "dynamicElementValue",
  "dynamicErrorBoundary",
  "dynamicFor",
  "dynamicForNodes",
  "dynamicForValue",
  "dynamicForValueNodes",
  "dynamicIndex",
  "dynamicIndexNodes",
  "dynamicShow",
  "dynamicShowValue",
  "dynamicSwitch",
  "dynamicSwitchValues",
  "dynamicSuspense",
  "dynamicText",
  "dynamicTextNode",
  "dynamicValue",
  "element",
  "firstChild",
  "fragment",
  "flattenNodeGroups",
  "mathElement",
  "materializeNodeGroup",
  "namespacedAttribute",
  "nextSibling",
  "nodeGroup",
  "onDelegatedClickVoid",
  "onDelegatedClickVoidPermanent",
  "onDelegatedEvent",
  "onDelegatedEventVoid",
  "onEvent",
  "query",
  "reactiveText",
  "region",
  "render",
  "portal",
  "portalNodes",
  "prepareMathTemplate",
  "prepareSvgTemplate",
  "prepareTemplate",
  "setText",
  "stringProperty",
  "spreadProps",
  "svgElement",
  "text",
  "use",
];

const REACTIVE_IMPORTS = [
  "Future",
  "Resource",
  "Selector",
  "Signal",
  "batch",
  "createMemo",
  "createResource",
  "createSelector",
  "createSignal",
  "diagnosticEffectSlots",
  "diagnosticFreeEffectSlots",
  "diagnosticFreeOwnerSlots",
  "diagnosticOwnerSlots",
  "diagnosticPendingEffects",
  "enableScheduling",
  "flushCallbacks",
  "onCleanup",
  "untrack",
];

const DOM_HOST_IMPORTS = [
  "domQueryRoot",
  "domCreateElement",
  "domCreateIntrinsicElement",
  "domCreateSvgElement",
  "domCreateMathElement",
  "domCreateText",
  "domCreateComment",
  "domCreateFragment",
  "domPrepareTemplate",
  "domPrepareSvgTemplate",
  "domPrepareMathTemplate",
  "domCloneNode",
  "domFirstChild",
  "domNextSibling",
  "domChildNodes",
  "domReleaseNode",
  "domIsFragment",
  "domAppendChild",
  "domRemoveNode",
  "domReconcile",
  "domReconcileOne",
  "domSetText",
  "domSetAttribute",
  "domSetAttributeNS",
  "domSetBoolAttribute",
  "domSetStringProperty",
  "domSetBoolProperty",
  "domToggleClass",
  "domSetStyleProperty",
  "domSpread",
  "domAddEventListener",
  "domRemoveEventListener",
  "domAddDelegatedEvent",
  "domAddDelegatedEventVoid",
  "domAddDelegatedClickVoid",
  "domSetDelegatedClickVoid",
  "domRemoveDelegatedClick",
  "domRemoveDelegatedEvent",
  "domClearDelegatedEvents",
  "domEventTarget",
  "domEventCurrentTarget",
  "domEventType",
  "domEventDefaultPrevented",
  "domEventPreventDefault",
  "domEventStopPropagation",
  "domSetEventHost",
  "domIsHead",
  "domAttachShadow",
  "domClear",
  "hostSchedule",
];

const DOM_HOST_CONTRACTS = `
extern JsValue domQueryRoot(string selector);
extern JsValue domCreateElement(string tag);
extern JsValue domCreateIntrinsicElement(string tag);
extern JsValue domCreateSvgElement(string tag);
extern JsValue domCreateMathElement(string tag);
extern JsValue domCreateText(string value);
extern JsValue domCreateComment();
extern JsValue domCreateFragment();
extern JsValue domPrepareTemplate(string html);
extern JsValue domPrepareSvgTemplate(string html);
extern JsValue domPrepareMathTemplate(string html);
extern JsValue domCloneNode(JsValue node);
extern JsValue domFirstChild(JsValue node);
extern JsValue domNextSibling(JsValue node);
extern JsValue[] domChildNodes(JsValue parent);
extern void domReleaseNode(JsValue node);
extern bool domIsFragment(JsValue node);
extern void domAppendChild(JsValue parent, JsValue child);
extern void domRemoveNode(JsValue node);
extern void domReconcile(JsValue parent, JsValue marker, JsValue[] current, JsValue[] next);
extern void domReconcileOne(JsValue parent, JsValue marker, JsValue current, JsValue next);
extern void domSetText(JsValue node, string value);
extern void domSetAttribute(JsValue node, string name, string value);
extern void domSetAttributeNS(JsValue node, string namespace, string name, string value);
extern void domSetBoolAttribute(JsValue node, string name, bool value);
extern void domSetStringProperty(JsValue node, string name, string value);
extern void domSetBoolProperty(JsValue node, string name, bool value);
extern void domToggleClass(JsValue node, string name, bool value);
extern void domSetStyleProperty(JsValue node, string name, string value);
extern JsValue domSpread(JsValue node, JsValue props, JsValue previous, bool svg);
extern JsValue domAddEventListener(JsValue node, string event, func()->void callback);
extern void domRemoveEventListener(JsValue listener);
extern JsValue domAddDelegatedEvent(JsValue node, string event, func(JsValue)->void callback);
extern JsValue domAddDelegatedEventVoid(JsValue node, string event, func()->void callback);
extern JsValue domAddDelegatedClickVoid(JsValue node, func()->void callback);
extern void domSetDelegatedClickVoid(JsValue node, func()->void callback);
extern void domRemoveDelegatedClick(JsValue listener);
extern void domRemoveDelegatedEvent(JsValue listener);
extern void domClearDelegatedEvents();
extern JsValue domEventTarget(JsValue event);
extern JsValue domEventCurrentTarget(JsValue event);
extern string domEventType(JsValue event);
extern bool domEventDefaultPrevented(JsValue event);
extern void domEventPreventDefault(JsValue event);
extern void domEventStopPropagation(JsValue event);
extern void domSetEventHost(JsValue node, JsValue host);
extern bool domIsHead(JsValue node);
extern JsValue domAttachShadow(JsValue node);
extern void domClear(JsValue node);
`.trim();

export function compileLilx(
  source,
  {
    filename = "input.lilx",
    reactiveImport = "./reactive",
    domImport = "./web",
    hostImport,
    directDom = false,
    persistentDelegation = false,
  } = {},
) {
  const context = createCtx();
  const replacements = [];
  let index = 0;
  while (index < source.length) {
    const start = findJsxStart(source, index);
    if (start < 0) break;
    try {
      const { node, end } = parseJsx(source, start);
      const lowered = lowerJsxToLil(node, context);
      replacements.push({
        start,
        end,
        text: loweredExpression(lowered),
      });
      index = end;
    } catch (error) {
      error.message = `${filename}: ${error.message}`;
      throw error;
    }
  }

  let output = "";
  let cursor = 0;
  for (const replacement of replacements) {
    output += source.slice(cursor, replacement.start);
    output += replacement.text;
    cursor = replacement.end;
  }
  output += source.slice(cursor);
  output = output.replace(
    /import\s*\{[^}]*\}\s*from\s*["']solidlil["']\s*;?\s*/g,
    "",
  );

  const header = [
    hostImport
      ? `import extern { ${DOM_HOST_IMPORTS.join(", ")} } from ${JSON.stringify(hostImport)};`
      : "",
    hostImport ? DOM_HOST_CONTRACTS : "",
    `import { ${REACTIVE_IMPORTS.join(", ")} } from ${JSON.stringify(reactiveImport)};`,
    `import { ${(directDom ? DOM_IMPORTS.filter((name) => name !== "DomNode") : DOM_IMPORTS).join(", ")} } from ${JSON.stringify(domImport)};`,
    "extern void hostSchedule(func()->void callback);",
    "enableScheduling(hostSchedule);",
    ...context.templates.map((template) => {
      const helper =
        template.wrap === "svg"
          ? "prepareSvgTemplate"
          : template.wrap === "math"
            ? "prepareMathTemplate"
            : "prepareTemplate";
      return `JsValue ${template.id} = ${helper}(${JSON.stringify(template.html)});`;
    }),
  ].filter(Boolean).join("\n") + "\n";

  output = output
    .replace(/extern void hostSchedule\(func\(\)->void callback\);\s*/g, "")
    .replace(/extern string hostTrim\(string value\);\s*/g, "")
    .replace(/enableScheduling\(hostSchedule\);\s*/g, "")
    .replace(/hostTrim\(/g, "_$H.T(");
  if (directDom) {
    output = output
      .replace(/\bDomNode\b/g, "JsValue")
      .replace(
        /flattenNodeGroups\(\[nodeGroup\(([A-Za-z_][A-Za-z0-9_]*)\)\]\)/g,
        "[$1]",
      );
  }
  if (persistentDelegation) {
    output = output.replaceAll(
      "onDelegatedClickVoid(",
      "onDelegatedClickVoidPermanent(",
    );
  }
  return `${header}${output}\n`;
}

function loweredExpression(lowered) {
  if (lowered.code.length === 1) {
    const match = lowered.code[0].match(
      /^DomNode \w+ = (cloneTemplate\(_tmpl\d+\));$/,
    );
    if (match && lowered.code[0].includes(` ${lowered.varName} = `)) {
      return match[1];
    }
  }
  return `( () => {\n${lowered.code.map((line) => `  ${line}`).join("\n")}\n  return ${lowered.varName};\n})()`;
}

function findJsxStart(source, from) {
  for (let index = from; index < source.length; index += 1) {
    if (source[index] !== "<") continue;
    const next = source[index + 1];
    if (!next || !/[A-Za-z]/.test(next)) continue;
    const before = source.slice(Math.max(0, index - 48), index);
    if (
      /(Signal|Map|Set|Array|func|Promise|createMemo|createSignal)\s*$/.test(
        before,
      )
    ) {
      continue;
    }
    if (
      /[A-Za-z0-9_]\s*$/.test(before) &&
      !/(return|=|\(|:|,)\s*$/.test(before)
    ) {
      continue;
    }
    return index;
  }
  return -1;
}

export function compileLilxFile(inputPath, outputPath, options) {
  const source = readFileSync(inputPath, "utf8");
  const output = compileLilx(source, { filename: inputPath, ...options });
  if (outputPath) writeFileSync(outputPath, output);
  return output;
}
