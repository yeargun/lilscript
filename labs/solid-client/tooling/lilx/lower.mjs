import { eventName, isComponent, isDelegated, parseJsx } from "./parse-jsx.mjs";

const VOID = new Set(["input", "br", "hr", "img", "meta", "link"]);
const SKIP_TEMPLATE_TAGS = new Set([
  "script",
  "style",
  "textarea",
  "title",
  "noscript",
  "iframe",
  "html",
  "head",
  "body",
  "template",
]);
const BOOLEAN_ATTRIBUTES = new Set([
  "allowFullScreen",
  "async",
  "autoFocus",
  "autoPlay",
  "controls",
  "default",
  "defer",
  "disabled",
  "formNoValidate",
  "hidden",
  "inert",
  "isMap",
  "loop",
  "multiple",
  "muted",
  "noModule",
  "noValidate",
  "open",
  "playsInline",
  "readOnly",
  "required",
  "reversed",
  "selected",
]);
const ATTRIBUTE_NAMESPACES = new Map([
  ["xlink", "http://www.w3.org/1999/xlink"],
  ["xml", "http://www.w3.org/XML/1998/namespace"],
]);

export function createCtx() {
  let counter = 0;
  return {
    id(prefix) {
      counter += 1;
      return `_${prefix}${counter}`;
    },
    templates: [],
    templateByHtml: new Map(),
  };
}

export function lowerJsxToLil(node, context = createCtx()) {
  if (node.type === "element" && isComponent(node.tag)) {
    if (isBuiltinComponent(node.tag)) {
      if (node.tag === "Match")
        throw new Error("Match requires a Switch parent");
      const lowered = lowerBuiltinNodeGroup(node, context, "html");
      const root = context.id("root");
      return {
        varName: root,
        code: [
          ...lowered.code,
          `DomNode ${root} = materializeNodeGroup(${lowered.variable});`,
        ],
      };
    }
    return lowerComponent(node, context, null, "html");
  }
  return lowerElement(node, context, null, "html");
}

function lowerElement(node, context, parentVariable, parentNamespace) {
  const namespace = namespaceFor(node.tag, parentNamespace);
  const templated = tryLowerTemplate(node, context, parentVariable, namespace);
  if (templated) return templated;
  const variable = context.id("el");
  const constructor =
    namespace === "svg"
      ? "svgElement"
      : namespace === "math"
        ? "mathElement"
        : "element";
  const code = [
    `DomNode ${variable} = ${constructor}(${JSON.stringify(node.tag)});`,
  ];
  code.push(...lowerElementBody(node, context, variable, namespace));

  if (parentVariable) code.push(`append(${parentVariable}, ${variable});`);
  return { varName: variable, code };
}

function tryLowerTemplate(node, context, parentVariable, namespace) {
  const plan = planHostTemplate(node, namespace);
  if (!plan) return null;
  const wrap = templateWrap(namespace, node.tag);
  const template = internTemplate(context, serializeTemplate(plan), wrap);
  const root = context.id("el");
  plan.variable = root;
  assignWalkNeeds(plan);
  const code = [`DomNode ${root} = cloneTemplate(${template});`];
  emitTemplateWalk(plan, context, code);
  emitTemplateHoles(plan, context, code);
  if (parentVariable) code.push(`append(${parentVariable}, ${root});`);
  return { varName: root, code };
}

function internTemplate(context, html, wrap) {
  const key = `${wrap}\0${html}`;
  const existing = context.templateByHtml.get(key);
  if (existing) return existing;
  const id = context.id("tmpl");
  context.templateByHtml.set(key, id);
  context.templates.push({ id, html, wrap });
  return id;
}

function templateWrap(namespace, tag) {
  if (namespace === "svg" && tag !== "svg") return "svg";
  if (namespace === "math" && tag !== "math") return "math";
  return "html";
}

function planHostTemplate(node, namespace) {
  if (node.type !== "element" || isComponent(node.tag)) return null;
  if (SKIP_TEMPLATE_TAGS.has(node.tag)) return null;
  if (node.props.some((property) => property.type === "spread")) return null;

  const holes = [];
  const staticAttrs = [];
  for (const property of node.props) {
    if (eventName(property.name)) {
      holes.push({ type: "event", property });
      continue;
    }
    if (property.name === "ref" && property.type === "expr") {
      holes.push({ type: "ref", value: property.value.trim() });
      continue;
    }
    if (property.name?.startsWith("use:") && property.type === "expr") {
      holes.push({
        type: "use",
        name: property.name.slice(4),
        value: property.value.trim(),
      });
      continue;
    }
    if (namespacedAttribute(property.name)) {
      holes.push({
        type: property.type === "string" ? "staticNamespaced" : "dynamic",
        property,
      });
      continue;
    }
    if (property.type === "string") {
      staticAttrs.push({
        name: attributeName(property.name),
        value: property.value,
      });
      continue;
    }
    if (property.type === "bool") {
      staticAttrs.push({
        name: attributeName(property.name),
        value: "",
      });
      continue;
    }
    holes.push({ type: "dynamic", property });
  }

  const plan = {
    kind: "element",
    tag: node.tag,
    namespace,
    staticAttrs,
    holes,
    children: [],
    inserts: [],
  };

  if (VOID.has(node.tag)) return plan;

  const childNamespace = childNamespaceFor(node.tag, namespace);
  const only = singleExpressionChild(node.children);
  if (only) {
    const expression = only.value.trim();
    if (hasCallExpression(expression)) {
      plan.children.push(textPlan(" ", [{ type: "dynamicText", expression }]));
    } else {
      holes.push({ type: "textContent", expression });
    }
    return validateTemplatePlan(plan);
  }

  let seenInsert = false;
  for (const child of node.children) {
    if (child.type === "text") {
      if (seenInsert) return null;
      const value = collapseWhitespace(child.value);
      if (!value) continue;
      appendTemplateText(plan.children, textPlan(value, []));
      continue;
    }
    if (child.type === "expr") {
      if (seenInsert) return null;
      const expression = child.value.trim();
      appendTemplateText(
        plan.children,
        textPlan(" ", [
          hasCallExpression(expression)
            ? { type: "dynamicText", expression }
            : { type: "setText", expression },
        ]),
      );
      continue;
    }
    if (child.type === "element" && isComponent(child.tag)) {
      seenInsert = true;
      plan.inserts.push(child);
      continue;
    }
    if (child.type === "element") {
      if (seenInsert) return null;
      const nested = planHostTemplate(child, childNamespace);
      if (!nested) return null;
      plan.children.push(nested);
      continue;
    }
    return null;
  }

  return validateTemplatePlan(plan);
}

function validateTemplatePlan(plan) {
  if (
    plan.tag === "table" &&
    plan.children.some(
      (child) => child.kind === "element" && child.tag === "tr",
    )
  ) {
    return null;
  }
  if (
    plan.tag === "tr" &&
    plan.children.some(
      (child) =>
        child.kind !== "element" || (child.tag !== "td" && child.tag !== "th"),
    )
  ) {
    return null;
  }
  return plan;
}

function textPlan(value, holes) {
  return { kind: "text", value, holes, children: [], inserts: [] };
}

function appendTemplateText(children, node) {
  if (children.length && children[children.length - 1].kind === "text") {
    children.push({
      kind: "comment",
      holes: [],
      children: [],
      inserts: [],
    });
  }
  children.push(node);
}

function serializeTemplate(plan) {
  if (plan.kind === "comment") return "<!>";
  if (plan.kind === "text") return escapeTemplateText(plan.value);
  let html = `<${plan.tag}`;
  for (const attribute of plan.staticAttrs) {
    html += serializeTemplateAttribute(attribute.name, attribute.value);
  }
  if (VOID.has(plan.tag)) return `${html}>`;
  html += ">";
  for (const child of plan.children) html += serializeTemplate(child);
  html += `</${plan.tag}>`;
  return html;
}

function serializeTemplateAttribute(name, value) {
  if (value === "") return ` ${name}`;
  if (/^[a-zA-Z0-9\-._]+$/.test(value)) return ` ${name}=${value}`;
  return ` ${name}="${escapeTemplateAttribute(value)}"`;
}

function escapeTemplateText(value) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

function escapeTemplateAttribute(value) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;");
}

function assignWalkNeeds(plan) {
  if (plan.kind !== "element") {
    plan.walk = (plan.holes?.length ?? 0) > 0;
    return plan.walk;
  }
  let later = false;
  for (let index = plan.children.length - 1; index >= 0; index -= 1) {
    const child = plan.children[index];
    const descendant = assignWalkNeeds(child);
    child.walk = descendant || later;
    if (child.walk) later = true;
  }
  plan.walk = plan.holes.length > 0 || plan.inserts.length > 0 || later;
  return plan.walk;
}

function emitTemplateWalk(plan, context, code) {
  if (plan.kind !== "element") return;
  let previous = null;
  for (const child of plan.children) {
    if (!child.walk) continue;
    const variable = context.id(child.kind === "element" ? "el" : "node");
    child.variable = variable;
    code.push(
      previous
        ? `DomNode ${variable} = nextSibling(${previous});`
        : `DomNode ${variable} = firstChild(${plan.variable});`,
    );
    previous = variable;
    emitTemplateWalk(child, context, code);
  }
}

function emitTemplateHoles(plan, context, code) {
  const variable = plan.variable;
  if (variable && plan.holes) {
    for (const hole of plan.holes) {
      if (hole.type === "event") {
        code.push(...lowerEvent(variable, hole.property));
      } else if (hole.type === "ref") {
        code.push(
          `untrack(() => { (${hole.value})(${variable}); return 0; });`,
        );
      } else if (hole.type === "use") {
        code.push(`use(${variable}, ${hole.name}, ${hole.value});`);
      } else if (hole.type === "staticNamespaced") {
        code.push(lowerStaticAttribute(variable, hole.property));
      } else if (hole.type === "dynamic") {
        for (const update of lowerDynamicUpdate(variable, hole.property)) {
          code.push(`createRenderEffect(() => { ${update} });`);
        }
      } else if (hole.type === "textContent") {
        code.push(
          `stringProperty(${variable}, "textContent", "" + (${hole.expression}));`,
        );
      } else if (hole.type === "setText") {
        code.push(`setText(${variable}, "" + (${hole.expression}));`);
      } else if (hole.type === "dynamicText") {
        code.push(
          `dynamicTextNode(${variable}, () => "" + (${hole.expression}));`,
        );
      }
    }
  }
  if (plan.kind === "element") {
    for (const child of plan.children) emitTemplateHoles(child, context, code);
    for (const insert of plan.inserts) {
      code.push(
        ...lowerBuiltinChild(insert, context, plan.variable, plan.namespace)
          .code,
      );
    }
  }
}

function lowerElementBody(
  node,
  context,
  variable,
  namespace,
  ignoredProps = new Set(),
) {
  const code = [];
  if (node.props.some((property) => property.type === "spread")) {
    code.push(...lowerHostSpread(node, variable, namespace));
    if (!VOID.has(node.tag)) {
      const childNamespace = childNamespaceFor(node.tag, namespace);
      code.push(
        ...lowerChildren(node.children, context, variable, childNamespace),
      );
    }
    return code;
  }
  const updates = [];
  for (const property of node.props) {
    if (ignoredProps.has(property.name)) continue;
    if (eventName(property.name)) {
      code.push(...lowerEvent(variable, property));
    } else if (property.name === "ref" && property.type === "expr") {
      code.push(
        `untrack(() => { (${property.value.trim()})(${variable}); return 0; });`,
      );
    } else if (property.name?.startsWith("use:") && property.type === "expr") {
      code.push(
        `use(${variable}, ${property.name.slice(4)}, ${property.value.trim()});`,
      );
    } else if (property.type === "string") {
      code.push(lowerStaticAttribute(variable, property));
    } else if (property.type === "bool") {
      code.push(lowerStaticAttribute(variable, { ...property, value: "" }));
    } else {
      updates.push(lowerDynamicUpdate(variable, property));
    }
  }
  for (const propertyUpdates of updates) {
    for (const update of propertyUpdates) {
      code.push(`createRenderEffect(() => { ${update} });`);
    }
  }

  if (!VOID.has(node.tag)) {
    const childNamespace = childNamespaceFor(node.tag, namespace);
    code.push(
      ...lowerChildren(node.children, context, variable, childNamespace),
    );
  }

  return code;
}

function lowerHostSpread(node, variable, namespace) {
  const props = `_spread_${variable.replace(/^_/, "")}`;
  const code = [`JsValue ${props} = componentProps();`];
  for (const property of node.props) {
    if (property.name?.startsWith("use:") && property.type === "expr") {
      code.push(
        `use(${variable}, ${property.name.slice(4)}, ${property.value.trim()});`,
      );
      continue;
    }
    if (property.type === "spread") {
      code.push(`componentSpread(${props}, ${property.value.trim()});`);
      continue;
    }
    code.push(
      `componentProperty(${props}, ${JSON.stringify(property.name)}, () => ${componentPropertyValue(property)});`,
    );
  }
  code.push(`spreadProps(${variable}, ${props}, ${namespace === "svg"});`);
  return code;
}

function lowerChildren(children, context, parentVariable, namespace) {
  const code = [];
  for (const child of children) {
    if (child.type === "text") {
      const value = collapseWhitespace(child.value);
      if (value)
        code.push(`append(${parentVariable}, text(${JSON.stringify(value)}));`);
    } else if (child.type === "expr") {
      const expression = child.value.trim();
      code.push(
        hasCallExpression(expression)
          ? `dynamicText(${parentVariable}, () => "" + (${expression}));`
          : `append(${parentVariable}, text("" + (${expression})));`,
      );
    } else if (child.type === "element" && isComponent(child.tag)) {
      code.push(
        ...lowerBuiltinChild(child, context, parentVariable, namespace).code,
      );
    } else if (child.type === "element") {
      code.push(
        ...lowerElement(child, context, parentVariable, namespace).code,
      );
    }
  }
  return code;
}

function lowerBuiltinChild(node, context, parentVariable, namespace) {
  if (node.tag === "Show")
    return lowerShow(node, context, parentVariable, namespace);
  if (node.tag === "For")
    return lowerFor(node, context, parentVariable, namespace);
  if (node.tag === "Index")
    return lowerIndex(node, context, parentVariable, namespace);
  if (node.tag === "Switch")
    return lowerSwitch(node, context, parentVariable, namespace);
  if (node.tag === "ErrorBoundary")
    return lowerErrorBoundary(node, context, parentVariable, namespace);
  if (node.tag === "Suspense")
    return lowerSuspense(node, context, parentVariable, namespace);
  if (node.tag === "Dynamic")
    return lowerDynamic(node, context, parentVariable, namespace);
  if (node.tag === "Portal")
    return lowerPortal(node, context, parentVariable, namespace);
  if (node.tag === "Match") throw new Error("Match requires a Switch parent");
  return lowerComponent(node, context, parentVariable, namespace);
}

function lowerComponent(node, context, parentVariable, namespace = "html") {
  const props = context.id("props");
  const component = context.id("component");
  const code = [`JsValue ${props} = componentProps();`];
  for (const property of node.props) {
    if (property.type === "spread") {
      code.push(`componentSpread(${props}, ${property.value.trim()});`);
      continue;
    }
    const value = componentPropertyValue(property);
    code.push(
      `componentProperty(${props}, ${JSON.stringify(property.name)}, () => ${value});`,
    );
  }
  const children = lowerNodeArray(
    node.children,
    context,
    namespace,
    `component <${node.tag}>`,
  );
  if (children.groups.length > 0) {
    code.push(
      `componentProperty(${props}, "children", () => {`,
      ...children.code.map((line) => `  ${line}`),
      `  DomNode[] nodes = ${nodeArrayExpression(children)};`,
      "  return nodes;",
      "});",
    );
  }
  code.push(`DomNode ${component} = componentNode(${node.tag}, ${props});`);
  if (parentVariable) code.push(`append(${parentVariable}, ${component});`);
  return { varName: component, code };
}

function lowerComponentNodeGroup(node, context, namespace = "html") {
  const props = context.id("props");
  const nodes = context.id("componentNodes");
  const code = [`JsValue ${props} = componentProps();`];
  for (const property of node.props) {
    if (property.type === "spread") {
      code.push(`componentSpread(${props}, ${property.value.trim()});`);
      continue;
    }
    code.push(
      `componentProperty(${props}, ${JSON.stringify(property.name)}, () => ${componentPropertyValue(property)});`,
    );
  }
  const children = lowerNodeArray(
    node.children,
    context,
    namespace,
    `component <${node.tag}>`,
  );
  if (children.groups.length > 0) {
    code.push(
      `componentProperty(${props}, "children", () => {`,
      ...children.code.map((line) => `  ${line}`),
      `  DomNode[] nodes = ${nodeArrayExpression(children)};`,
      "  return nodes;",
      "});",
    );
  }
  code.push(`DomNode[] ${nodes} = componentNodes(${node.tag}, ${props});`);
  return { variable: nodes, code };
}

function componentPropertyValue(property) {
  if (property.type === "expr") return property.value.trim();
  if (property.type === "string") return JSON.stringify(property.value);
  if (property.type === "bool") return "true";
  throw new Error(`unsupported component property ${property.name}`);
}

function lowerShow(node, context, parentVariable, namespace = "html") {
  if (!parentVariable) throw new Error("Show requires a parent element");
  const condition = propertyExpression(node, "when");
  if (!condition) throw new Error("Show requires when={...}");
  const childFunction = singleExpressionChild(node.children);
  const parsedChild = childFunction
    ? parseControlChild(childFunction.value, "Show")
    : null;
  const children = lowerNodeArray(
    parsedChild ? [parsedChild.node] : node.children,
    context,
    namespace,
    "Show",
  );
  const fallback = propertyExpression(node, "fallback");
  let fallbackNodes = emptyNodeArray();
  if (fallback) {
    const fallbackNode = parseJsx(fallback.trim(), 0).node;
    fallbackNodes = lowerNodeArray(
      [fallbackNode],
      context,
      namespace,
      "Show fallback",
    );
  }
  const keyed = propertyValue(node, "keyed", "false") === "true";
  const branchValue = context.id("showValue");
  const branchAccessor = context.id("showAccessor");
  const code = [
    `dynamicShowValue(region(${parentVariable}), () => ${condition.trim()}, ${keyed}, (JsValue ${branchValue}, func()->JsValue ${branchAccessor}) => {`,
    ...(parsedChild
      ? [
          `  ${parsedChild.parameterType} ${parsedChild.parameter} = JS.assume(${keyed ? branchValue : branchAccessor});`,
        ]
      : []),
    ...children.code.map((line) => `  ${line}`),
    `  DomNode[] nodes = ${nodeArrayExpression(children)};`,
    "  return nodes;",
    "}, () => {",
    ...fallbackNodes.code.map((line) => `  ${line}`),
    `  DomNode[] fallbackNodes = ${nodeArrayExpression(fallbackNodes)};`,
    "  return fallbackNodes;",
    "});",
  ];
  return { varName: parentVariable, code };
}

function lowerSwitch(node, context, parentVariable, namespace = "html") {
  if (!parentVariable) throw new Error("Switch requires a parent element");
  const matches = node.children.filter(
    (child) => child.type === "element" && child.tag === "Match",
  );
  const invalid = node.children.find(
    (child) =>
      child.type !== "text" &&
      !(child.type === "element" && child.tag === "Match"),
  );
  if (invalid) throw new Error("Switch children must be Match elements");
  if (matches.length === 0) throw new Error("Switch requires a Match child");

  const conditions = [];
  const keyed = [];
  const branchFunctions = [];
  for (const match of matches) {
    const condition = propertyExpression(match, "when");
    if (!condition) throw new Error("Match requires when={...}");
    const childFunction = singleExpressionChild(match.children);
    const parsedChild = childFunction
      ? parseControlChild(childFunction.value, "Match")
      : null;
    const branch = lowerNodeArray(
      parsedChild ? [parsedChild.node] : match.children,
      context,
      namespace,
      "Match",
    );
    const keyedMatch = propertyValue(match, "keyed", "false") === "true";
    const branchValue = context.id("matchValue");
    const branchAccessor = context.id("matchAccessor");
    conditions.push(`() => ${condition.trim()}`);
    keyed.push(keyedMatch);
    branchFunctions.push(
      [
        `(JsValue ${branchValue}, func()->JsValue ${branchAccessor}) => {`,
        ...(parsedChild
          ? [
              `  ${parsedChild.parameterType} ${parsedChild.parameter} = JS.assume(${keyedMatch ? branchValue : branchAccessor});`,
            ]
          : []),
        ...branch.code.map((line) => `  ${line}`),
        `  DomNode[] nodes = ${nodeArrayExpression(branch)};`,
        "  return nodes;",
        "}",
      ].join(" "),
    );
  }

  const fallback = propertyExpression(node, "fallback");
  let fallbackNodes = emptyNodeArray();
  if (fallback) {
    const fallbackNode = parseJsx(fallback.trim(), 0).node;
    fallbackNodes = lowerNodeArray(
      [fallbackNode],
      context,
      namespace,
      "Switch fallback",
    );
  }
  const matchFunctions = context.id("switchMatches");
  const keyedMatches = context.id("switchKeyed");
  const childrenFunctions = context.id("switchChildren");
  return {
    varName: parentVariable,
    code: [
      `(func()->JsValue)[] ${matchFunctions} = [${conditions.join(", ")}];`,
      `bool[] ${keyedMatches} = [${keyed.join(", ")}];`,
      `(func(JsValue,func()->JsValue)->DomNode[])[] ${childrenFunctions} = [${branchFunctions.join(", ")}];`,
      `dynamicSwitchValues(region(${parentVariable}), ${matchFunctions}, ${keyedMatches}, ${childrenFunctions}, () => {`,
      ...fallbackNodes.code.map((line) => `  ${line}`),
      `  DomNode[] fallbackNodes = ${nodeArrayExpression(fallbackNodes)};`,
      "  return fallbackNodes;",
      "});",
    ],
  };
}

function lowerErrorBoundary(node, context, parentVariable, namespace = "html") {
  if (!parentVariable)
    throw new Error("ErrorBoundary requires a parent element");
  const fallback = propertyExpression(node, "fallback");
  if (!fallback) throw new Error("ErrorBoundary requires fallback={...}");

  const children = lowerNodeArray(
    node.children,
    context,
    namespace,
    "ErrorBoundary",
  );
  const parsedFallback = parseErrorBoundaryFallback(fallback);
  const fallbackNodes = lowerNodeArray(
    [parsedFallback.node],
    context,
    namespace,
    "ErrorBoundary fallback",
  );
  const error = context.id("boundaryError");
  const reset = context.id("boundaryReset");
  const aliases = [];
  if (parsedFallback.error) {
    aliases.push(
      `${parsedFallback.error.type} ${parsedFallback.error.name} = JS.assume(${error});`,
    );
  }
  if (parsedFallback.reset) {
    aliases.push(`func()->void ${parsedFallback.reset.name} = ${reset};`);
  }
  return {
    varName: parentVariable,
    code: [
      `dynamicErrorBoundary(region(${parentVariable}), () => {`,
      ...children.code.map((line) => `  ${line}`),
      `  DomNode[] nodes = ${nodeArrayExpression(children)};`,
      "  return nodes;",
      `}, (JsValue ${error}, func()->void ${reset}) => {`,
      ...aliases.map((line) => `  ${line}`),
      ...fallbackNodes.code.map((line) => `  ${line}`),
      `  DomNode[] fallbackNodes = ${nodeArrayExpression(fallbackNodes)};`,
      "  return fallbackNodes;",
      "});",
    ],
  };
}

function lowerSuspense(node, context, parentVariable, namespace = "html") {
  if (!parentVariable) throw new Error("Suspense requires a parent element");
  const children = lowerNodeArray(
    node.children,
    context,
    namespace,
    "Suspense",
  );
  const fallback = propertyExpression(node, "fallback");
  let fallbackNodes = emptyNodeArray();
  if (fallback) {
    fallbackNodes = lowerNodeArray(
      [parseJsx(fallback.trim(), 0).node],
      context,
      namespace,
      "Suspense fallback",
    );
  }
  return {
    varName: parentVariable,
    code: [
      `dynamicSuspense(region(${parentVariable}), () => {`,
      ...children.code.map((line) => `  ${line}`),
      `  DomNode[] nodes = ${nodeArrayExpression(children)};`,
      "  return nodes;",
      "}, () => {",
      ...fallbackNodes.code.map((line) => `  ${line}`),
      `  DomNode[] fallbackNodes = ${nodeArrayExpression(fallbackNodes)};`,
      "  return fallbackNodes;",
      "});",
    ],
  };
}

function lowerDynamic(node, context, parentVariable, namespace = "html") {
  if (!parentVariable) throw new Error("Dynamic requires a parent element");
  const component = propertyExpression(node, "component");
  if (!component) throw new Error("Dynamic requires component={...}");
  const variable = context.id("dynamic");
  const props = context.id("dynamicProps");
  const code = [`JsValue ${props} = componentProps();`];
  for (const property of node.props) {
    if (property.name === "component") continue;
    if (property.type === "spread") {
      code.push(`componentSpread(${props}, ${property.value.trim()});`);
    } else {
      code.push(
        `componentProperty(${props}, ${JSON.stringify(property.name)}, () => ${componentPropertyValue(property)});`,
      );
    }
  }
  const children = lowerNodeArray(node.children, context, namespace, "Dynamic");
  if (children.groups.length > 0) {
    code.push(
      `componentProperty(${props}, "children", () => {`,
      ...children.code.map((line) => `  ${line}`),
      `  DomNode[] nodes = ${nodeArrayExpression(children)};`,
      "  return nodes;",
      "});",
    );
  }
  return {
    varName: parentVariable,
    code: [
      ...code,
      `dynamicValue(${parentVariable}, () => ${component.trim()}, ${props}, (DomNode ${variable}) => {`,
      `  spreadProps(${variable}, ${props}, ${namespace === "svg"});`,
      ...(children.groups.length > 0
        ? [`  appendComponentChildren(${variable}, ${props});`]
        : []),
      "});",
    ],
  };
}

function lowerPortal(node, context, parentVariable, namespace = "html") {
  if (!parentVariable) throw new Error("Portal requires a parent element");
  const mount = propertyExpression(node, "mount")?.trim() || 'query("body")';
  const svg = propertyValue(node, "isSVG", "false");
  const useShadow = propertyValue(node, "useShadow", "false");
  const ref = propertyExpression(node, "ref");
  const childNamespace = svg === "true" ? "svg" : namespace;
  const children = lowerNodeArray(
    node.children,
    context,
    childNamespace,
    "Portal",
  );
  return {
    varName: parentVariable,
    code: [
      `portalNodes(${parentVariable}, ${mount}, ${svg}, ${useShadow}, (DomNode node) => { ${ref ? `(${ref.trim()})(node);` : ""} }, () => {`,
      ...children.code.map((line) => `  ${line}`),
      `  DomNode[] nodes = ${nodeArrayExpression(children)};`,
      "  return nodes;",
      "});",
    ],
  };
}

function lowerNodeArray(children, context, namespace, label) {
  const code = [];
  const groups = [];
  for (const child of children) {
    if (child.type === "text") {
      const value = collapseWhitespace(child.value);
      if (!value) continue;
      const variable = context.id("text");
      code.push(`DomNode ${variable} = text(${JSON.stringify(value)});`);
      groups.push({ variable, array: false });
    } else if (child.type === "expr") {
      const variable = context.id("text");
      const expression = child.value.trim();
      code.push(
        hasCallExpression(expression)
          ? `DomNode ${variable} = reactiveText(() => "" + (${expression}));`
          : `DomNode ${variable} = text("" + (${expression}));`,
      );
      groups.push({ variable, array: false });
    } else if (child.type === "element" && !isComponent(child.tag)) {
      const lowered = lowerElement(child, context, null, namespace);
      code.push(...lowered.code);
      groups.push({ variable: lowered.varName, array: false });
    } else if (
      child.type === "element" &&
      isComponent(child.tag) &&
      !isBuiltinComponent(child.tag)
    ) {
      const lowered = lowerComponentNodeGroup(child, context, namespace);
      code.push(...lowered.code);
      groups.push({ variable: lowered.variable, array: true });
    } else if (
      child.type === "element" &&
      isComponent(child.tag) &&
      child.tag !== "Match"
    ) {
      const lowered = lowerBuiltinNodeGroup(child, context, namespace);
      code.push(...lowered.code);
      groups.push({ variable: lowered.variable, array: true });
    } else {
      throw new Error(`${label} contains an unsupported child`);
    }
  }
  return { code, groups };
}

function hasCallExpression(expression) {
  return /[A-Za-z_$][A-Za-z0-9_$]*\s*\(/.test(expression);
}

function lowerBuiltinNodeGroup(node, context, namespace) {
  const fragment = context.id("fragment");
  const nodes = context.id("nodes");
  return {
    variable: nodes,
    code: [
      `DomNode ${fragment} = fragment();`,
      ...lowerBuiltinChild(node, context, fragment, namespace).code,
      `DomNode[] ${nodes} = childNodes(${fragment});`,
    ],
  };
}

function emptyNodeArray() {
  return { code: [], groups: [] };
}

function singleExpressionChild(children) {
  const meaningful = children.filter(
    (child) => child.type !== "text" || collapseWhitespace(child.value),
  );
  if (meaningful.length !== 1 || meaningful[0].type !== "expr") return null;
  return meaningful[0];
}

function parseControlChild(source, label) {
  const arrow = parseTypedArrow(source, `${label} render`);
  if (!arrow || arrow.parameters.length !== 1) return null;
  const [parameter] = arrow.parameters;
  const body = unwrapArrowElement(arrow.body, `${label} render`);
  return {
    parameterType: parameter.type,
    parameter: parameter.name,
    node: parseJsx(body, 0).node,
  };
}

function nodeArrayExpression(nodes) {
  if (nodes.groups.length === 0) return "[]";
  const groups = nodes.groups.map(({ variable, array }) =>
    array ? variable : `nodeGroup(${variable})`,
  );
  return `flattenNodeGroups([${groups.join(", ")}])`;
}

function isBuiltinComponent(tag) {
  return new Set([
    "Show",
    "For",
    "Index",
    "Switch",
    "Match",
    "ErrorBoundary",
    "Suspense",
    "Dynamic",
    "Portal",
  ]).has(tag);
}

function parseErrorBoundaryFallback(source) {
  const arrow = parseTypedArrow(source, "ErrorBoundary fallback");
  if (!arrow) {
    return { error: null, reset: null, node: parseJsx(source.trim(), 0).node };
  }
  if (arrow.parameters.length < 1 || arrow.parameters.length > 2) {
    throw new Error(
      "ErrorBoundary fallback must accept an error and optional reset callback",
    );
  }
  const [error, reset] = arrow.parameters;
  if (reset && compactType(reset.type) !== "func()->void") {
    throw new Error(
      "ErrorBoundary reset parameter must have type func()->void",
    );
  }
  return {
    error,
    reset: reset || null,
    node: parseJsx(unwrapArrowElement(arrow.body, "ErrorBoundary fallback"), 0)
      .node,
  };
}

function lowerFor(node, context, parentVariable, namespace = "html") {
  if (!parentVariable) throw new Error("For requires a parent element");
  const each = propertyExpression(node, "each");
  if (!each) throw new Error("For requires each={...}");
  const childFunction = node.children.find((child) => child.type === "expr");
  if (!childFunction) throw new Error("For requires {(T item) => <...>} child");
  const parsed = parseForChild(childFunction.value);
  const item = lowerNodeArray([parsed.element], context, namespace, "For row");
  const fallback = propertyExpression(node, "fallback");
  let fallbackNodes = emptyNodeArray();
  if (fallback) {
    const fallbackNode = parseJsx(fallback.trim(), 0).node;
    fallbackNodes = lowerNodeArray(
      [fallbackNode],
      context,
      namespace,
      "For fallback",
    );
  }
  const singleValue =
    !parsed.hasIndex &&
    !fallback &&
    item.groups.length === 1 &&
    !item.groups[0].array;
  if (singleValue) {
    return {
      varName: parentVariable,
      code: [
        `dynamicForValue(${parentVariable}, ${each.trim()}, (${parsed.typeName} ${parsed.item}) => {`,
        ...item.code.map((line) => `  ${line}`),
        `  return ${item.groups[0].variable};`,
        "});",
      ],
    };
  }
  const dynamicFunction = parsed.hasIndex
    ? "dynamicForNodes"
    : "dynamicForValueNodes";
  const parameters = parsed.hasIndex
    ? `${parsed.typeName} ${parsed.item}, Signal<int> ${parsed.index}`
    : `${parsed.typeName} ${parsed.item}`;
  return {
    varName: parentVariable,
    code: [
      `${dynamicFunction}(${parentVariable}, ${each.trim()}, (${parameters}) => {`,
      ...item.code.map((line) => `  ${line}`),
      `  DomNode[] nodes = ${nodeArrayExpression(item)};`,
      "  return nodes;",
      "}, () => {",
      ...fallbackNodes.code.map((line) => `  ${line}`),
      `  DomNode[] fallbackNodes = ${nodeArrayExpression(fallbackNodes)};`,
      "  return fallbackNodes;",
      "});",
    ],
  };
}

function lowerIndex(node, context, parentVariable, namespace = "html") {
  if (!parentVariable) throw new Error("Index requires a parent element");
  const each = propertyExpression(node, "each");
  if (!each) throw new Error("Index requires each={...}");
  const childFunction = node.children.find((child) => child.type === "expr");
  if (!childFunction)
    throw new Error(
      "Index requires {(Signal<T> item, int index) => <...>} child",
    );
  const parsed = parseIndexChild(childFunction.value);
  const item = lowerNodeArray(
    [parsed.element],
    context,
    namespace,
    "Index row",
  );
  const fallback = propertyExpression(node, "fallback");
  let fallbackNodes = emptyNodeArray();
  if (fallback) {
    const fallbackNode = parseJsx(fallback.trim(), 0).node;
    fallbackNodes = lowerNodeArray(
      [fallbackNode],
      context,
      namespace,
      "Index fallback",
    );
  }
  return {
    varName: parentVariable,
    code: [
      `dynamicIndexNodes(${parentVariable}, ${each.trim()}, (Signal<${parsed.typeName}> ${parsed.item}, int ${parsed.index}) => {`,
      ...item.code.map((line) => `  ${line}`),
      `  DomNode[] nodes = ${nodeArrayExpression(item)};`,
      "  return nodes;",
      "}, () => {",
      ...fallbackNodes.code.map((line) => `  ${line}`),
      `  DomNode[] fallbackNodes = ${nodeArrayExpression(fallbackNodes)};`,
      "  return fallbackNodes;",
      "});",
    ],
  };
}

function parseForChild(source) {
  const arrow = parseTypedArrow(source, "For");
  if (!arrow || arrow.parameters.length < 1 || arrow.parameters.length > 2) {
    throw new Error(`unsupported For child: ${source.slice(0, 100)}`);
  }
  const [itemParameter, indexParameter] = arrow.parameters;
  if (indexParameter && compactType(indexParameter.type) !== "Signal<int>") {
    throw new Error("For index parameter must have type Signal<int>");
  }
  const rest = unwrapArrowElement(arrow.body, "For");
  return {
    typeName: itemParameter.type,
    item: itemParameter.name,
    index: indexParameter?.name || "index",
    hasIndex: Boolean(indexParameter),
    element: parseJsx(rest, 0).node,
  };
}

function parseIndexChild(source) {
  const arrow = parseTypedArrow(source, "Index");
  if (!arrow || arrow.parameters.length < 1 || arrow.parameters.length > 2)
    throw new Error(`unsupported Index child: ${source.slice(0, 100)}`);
  const [itemParameter, indexParameter] = arrow.parameters;
  const signalType = compactType(itemParameter.type);
  if (!signalType.startsWith("Signal<") || !signalType.endsWith(">")) {
    throw new Error("Index value parameter must have type Signal<T>");
  }
  if (indexParameter && compactType(indexParameter.type) !== "int") {
    throw new Error("Index index parameter must have type int");
  }
  const rest = unwrapArrowElement(arrow.body, "Index");
  return {
    typeName: signalType.slice("Signal<".length, -1),
    item: itemParameter.name,
    index: indexParameter?.name || "index",
    element: parseJsx(rest, 0).node,
  };
}

function parseTypedArrow(source, label) {
  const value = source.trim();
  if (!value.startsWith("(")) return null;
  let depth = 0;
  let close = -1;
  for (let position = 0; position < value.length; position += 1) {
    if (value[position] === "(") depth += 1;
    else if (value[position] === ")") {
      depth -= 1;
      if (depth === 0) {
        close = position;
        break;
      }
    }
  }
  if (close < 0) throw new Error(`unclosed ${label} parameter list`);
  const after = value.slice(close + 1);
  const arrow = after.match(/^\s*=>\s*/);
  if (!arrow) return null;
  const declarations = splitTopLevel(value.slice(1, close), ",");
  const parameters = declarations.map((declaration) => {
    const match = declaration
      .trim()
      .match(/^(.*\S)\s+([A-Za-z_][A-Za-z0-9_]*)$/s);
    if (!match) throw new Error(`invalid ${label} parameter: ${declaration}`);
    return { type: match[1].trim(), name: match[2] };
  });
  return {
    parameters,
    body: after.slice(arrow[0].length).trim(),
  };
}

function splitTopLevel(source, separator) {
  const parts = [];
  let start = 0;
  let parentheses = 0;
  let brackets = 0;
  let angles = 0;
  for (let position = 0; position < source.length; position += 1) {
    const character = source[position];
    if (character === "(") parentheses += 1;
    else if (character === ")") parentheses -= 1;
    else if (character === "[") brackets += 1;
    else if (character === "]") brackets -= 1;
    else if (character === "<") angles += 1;
    else if (character === ">" && source[position - 1] !== "-") angles -= 1;
    else if (
      character === separator &&
      parentheses === 0 &&
      brackets === 0 &&
      angles === 0
    ) {
      parts.push(source.slice(start, position));
      start = position + 1;
    }
  }
  parts.push(source.slice(start));
  return parts.filter((part) => part.trim());
}

function compactType(type) {
  return type.replace(/\s+/g, "");
}

function unwrapArrowElement(source, label) {
  let rest = source;
  if (rest.startsWith("(")) {
    let depth = 0;
    let end = -1;
    for (let position = 0; position < rest.length; position += 1) {
      if (rest[position] === "(") depth += 1;
      else if (rest[position] === ")") {
        depth -= 1;
        if (depth === 0) {
          end = position;
          break;
        }
      }
    }
    if (end < 0) throw new Error(`unclosed parenthesized ${label} child`);
    rest = rest.slice(1, end).trim();
  }
  return rest;
}

function lowerEvent(variable, property) {
  const event = eventName(property.name);
  const usesEvent = /\b(?:e|event)\s*\./.test(property.value);
  const body = wrapVoid(property.value);
  return [
    isDelegated(event)
      ? usesEvent
        ? `onDelegatedEvent(${variable}, ${JSON.stringify(event)}, (DomEvent event) => { ${body} });`
        : event === "click"
          ? `onDelegatedClickVoid(${variable}, () => { ${body} });`
          : `onDelegatedEventVoid(${variable}, ${JSON.stringify(event)}, () => { ${body} });`
      : `onEvent(${variable}, ${JSON.stringify(event)}, () => { ${body} });`,
  ];
}

function lowerDynamicUpdate(variable, property) {
  if (property.name === "classList" && property.type === "expr") {
    return parseClassList(property.value).map(
      ([name, expression]) =>
        `classToggle(${variable}, ${JSON.stringify(name)}, ${expression});`,
    );
  }
  if (property.name === "checked") {
    return [`boolProperty(${variable}, "checked", ${property.value.trim()});`];
  }
  if (property.name === "value") {
    return [
      `stringProperty(${variable}, "value", "" + (${property.value.trim()}));`,
    ];
  }
  if (BOOLEAN_ATTRIBUTES.has(property.name)) {
    return [
      `boolAttribute(${variable}, ${JSON.stringify(attributeName(property.name))}, ${property.value.trim()});`,
    ];
  }
  const namespaced = namespacedAttribute(property.name);
  if (namespaced) {
    return [
      `namespacedAttribute(${variable}, ${JSON.stringify(namespaced.namespace)}, ${JSON.stringify(namespaced.name)}, "" + (${property.value.trim()}));`,
    ];
  }
  const name = attributeName(property.name);
  return [
    `attribute(${variable}, ${JSON.stringify(name)}, "" + (${property.value.trim()}));`,
  ];
}

function lowerStaticAttribute(variable, property) {
  const namespaced = namespacedAttribute(property.name);
  if (namespaced) {
    return `namespacedAttribute(${variable}, ${JSON.stringify(namespaced.namespace)}, ${JSON.stringify(namespaced.name)}, ${JSON.stringify(property.value)});`;
  }
  return `attribute(${variable}, ${JSON.stringify(attributeName(property.name))}, ${JSON.stringify(property.value)});`;
}

function namespacedAttribute(name) {
  const separator = name.indexOf(":");
  if (separator < 0) return null;
  const prefix = name.slice(0, separator);
  const namespace = ATTRIBUTE_NAMESPACES.get(prefix);
  if (!namespace) throw new Error(`unsupported attribute namespace ${prefix}:`);
  return { namespace, name };
}

function parseClassList(expression) {
  let body = expression.trim();
  if (body.startsWith("{") && body.endsWith("}")) body = body.slice(1, -1);
  const entries = [];
  for (const item of body.split(",")) {
    const piece = item.trim();
    if (!piece) continue;
    const separator = piece.indexOf(":");
    if (separator < 0) continue;
    entries.push([
      piece
        .slice(0, separator)
        .trim()
        .replace(/^["']|["']$/g, ""),
      piece.slice(separator + 1).trim(),
    ]);
  }
  return entries;
}

function propertyExpression(node, name) {
  const property = node.props.find((item) => item.name === name);
  return property?.type === "expr" ? property.value : null;
}

function propertyValue(node, name, fallback) {
  const property = node.props.find((item) => item.name === name);
  if (!property) return fallback;
  if (property.type === "expr") return property.value.trim();
  if (property.type === "bool") return "true";
  if (property.type === "string") return JSON.stringify(property.value);
  return fallback;
}

function wrapVoid(expression) {
  let value = expression.trim();
  const wasArrow = value.includes("=>");
  if (value.includes("=>")) {
    const arrow = value.indexOf("=>");
    value = value.slice(arrow + 2).trim();
    if (value.startsWith("{") && value.endsWith("}")) {
      value = value.slice(1, -1);
    }
  }
  value = value
    .replace(/\be\.preventDefault\(\)/g, "event.preventDefault()")
    .replace(/\be\.stopPropagation\(\)/g, "event.stopPropagation()")
    .trim();
  if (!wasArrow && /^[A-Za-z_][A-Za-z0-9_.]*$/.test(value)) value += "()";
  if (!value.endsWith(";")) value = `${value};`;
  return value.replace(/;\s*;+/g, ";");
}

function collapseWhitespace(value) {
  if (!value.includes("\n") && !value.includes("\r")) {
    return value.replace(/\s+/g, " ");
  }
  const lines = value.replace(/\r/g, "").split("\n");
  let output = "";
  for (let index = 0; index < lines.length; index += 1) {
    let line = lines[index].replace(/\t/g, " ").replace(/ +/g, " ");
    if (index !== 0) line = line.replace(/^ +/, "");
    if (index !== lines.length - 1) line = line.replace(/ +$/, "");
    if (!line) continue;
    output += line;
    if (index !== lines.length - 1 && !line.endsWith(" ")) output += " ";
  }
  return output;
}

function attributeName(name) {
  if (name === "className") return "class";
  if (name === "htmlFor") return "for";
  return name.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`);
}

function namespaceFor(tag, parentNamespace) {
  if (tag === "svg") return "svg";
  if (tag === "math") return "math";
  return parentNamespace;
}

function childNamespaceFor(tag, namespace) {
  if (namespace === "svg" && tag === "foreignObject") return "html";
  return namespace;
}
