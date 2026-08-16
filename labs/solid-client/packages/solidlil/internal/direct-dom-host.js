import { reconcileDomNodesForSize } from "./reconcile-dom-size.js";

const svgNamespace = "http://www.w3.org/2000/svg";
const mathNamespace = "http://www.w3.org/1998/Math/MathML";
const xlinkNamespace = "http://www.w3.org/1999/xlink";
const xmlNamespace = "http://www.w3.org/XML/1998/namespace";

const svgElements = new Set([
  "altGlyph",
  "altGlyphDef",
  "altGlyphItem",
  "animate",
  "animateColor",
  "animateMotion",
  "animateTransform",
  "circle",
  "clipPath",
  "color-profile",
  "cursor",
  "defs",
  "desc",
  "ellipse",
  "feBlend",
  "feColorMatrix",
  "feComponentTransfer",
  "feComposite",
  "feConvolveMatrix",
  "feDiffuseLighting",
  "feDisplacementMap",
  "feDistantLight",
  "feDropShadow",
  "feFlood",
  "feFuncA",
  "feFuncB",
  "feFuncG",
  "feFuncR",
  "feGaussianBlur",
  "feImage",
  "feMerge",
  "feMergeNode",
  "feMorphology",
  "feOffset",
  "fePointLight",
  "feSpecularLighting",
  "feSpotLight",
  "feTile",
  "feTurbulence",
  "filter",
  "font",
  "font-face",
  "font-face-format",
  "font-face-name",
  "font-face-src",
  "font-face-uri",
  "foreignObject",
  "g",
  "glyph",
  "glyphRef",
  "hkern",
  "image",
  "line",
  "linearGradient",
  "marker",
  "mask",
  "metadata",
  "missing-glyph",
  "mpath",
  "path",
  "pattern",
  "polygon",
  "polyline",
  "radialGradient",
  "rect",
  "set",
  "stop",
  "svg",
  "switch",
  "symbol",
  "text",
  "textPath",
  "tref",
  "tspan",
  "use",
  "view",
  "vkern",
]);

const booleanProperties = new Set([
  "checked",
  "disabled",
  "multiple",
  "muted",
  "readOnly",
  "required",
  "selected",
]);
const stringProperties = new Set(["value"]);

let eventHosts;
let delegatedHandlers = new WeakMap();
const delegatedRoots = new Map();
const clickHandler = Symbol();
let clickCount = 0;

export const domQueryRoot = (selector) => document.querySelector(selector);
export const domCreateElement = (tag) => document.createElement(tag);
export const domCreateIntrinsicElement = (tag) =>
  svgElements.has(tag)
    ? document.createElementNS(svgNamespace, tag)
    : document.createElement(tag);
export const domCreateSvgElement = (tag) =>
  document.createElementNS(svgNamespace, tag);
export const domCreateMathElement = (tag) =>
  document.createElementNS(mathNamespace, tag);
export const domCreateText = (value) => document.createTextNode(value);
export const domCreateComment = () => document.createComment("");
export const domCreateFragment = () => document.createDocumentFragment();

function prepareTemplateRoot(html) {
  const template = document.createElement("template");
  template.innerHTML = html;
  return template.content.firstChild;
}

export const domPrepareTemplate = (html) => prepareTemplateRoot(html);
export const domPrepareSvgTemplate = (html) =>
  prepareTemplateRoot(`<svg>${html}</svg>`).firstChild;
export const domPrepareMathTemplate = (html) =>
  prepareTemplateRoot(`<math>${html}</math>`).firstChild;
export const domCloneNode = (node) => node.cloneNode(true);
export const domFirstChild = (node) => node.firstChild;
export const domNextSibling = (node) => node.nextSibling;
export const domChildNodes = (parent) => [...parent.childNodes];
export const domReleaseNode = () => {};
export const domIsFragment = (node) => node.nodeType === Node.DOCUMENT_FRAGMENT_NODE;
export const domAppendChild = (parent, child) => parent.appendChild(child);
export const domRemoveNode = (node) => node.remove();

export function domReconcile(parent, marker, current, next) {
  reconcileDomNodesForSize(marker.parentNode ?? parent, marker, current, next);
}

export function domReconcileOne(parent, marker, current, next) {
  if (current === next) return;
  (current?.parentNode ?? marker.parentNode ?? parent).replaceChild(next, current);
}

export const domSetText = (node, value) => {
  node.data = value;
};
export const domSetAttribute = (node, name, value) =>
  node.setAttribute(name, value);
export const domSetAttributeNS = (node, namespace, name, value) =>
  node.setAttributeNS(namespace, name, value);
export const domSetBoolAttribute = (node, name, value) => {
  if (value) node.setAttribute(name, "");
  else node.removeAttribute(name);
};
export const domSetStringProperty = (node, name, value) => {
  node[name] = value;
};
export const domSetBoolProperty = (node, name, value) => {
  node[name] = value;
};
export const domToggleClass = (node, name, value) => {
  for (const token of name.trim().split(/\s+/)) {
    if (token) node.classList.toggle(token, value);
  }
};
export const domSetStyleProperty = (node, name, value) => {
  if (value === "") node.style.removeProperty(name);
  else node.style.setProperty(name, value);
};

function updateClassList(node, value, previous = {}) {
  value ??= {};
  for (const name of Object.keys(previous)) {
    if (!value[name]) {
      domToggleClass(node, name, false);
      delete previous[name];
    }
  }
  for (const name of Object.keys(value)) {
    const enabled = Boolean(value[name]);
    if (previous[name] === enabled) continue;
    domToggleClass(node, name, enabled);
    previous[name] = enabled;
  }
  return previous;
}

function updateStyle(node, value, previous = {}) {
  if (typeof value === "string") {
    node.style.cssText = value;
    return value;
  }
  if (typeof previous === "string") {
    node.style.cssText = "";
    previous = {};
  }
  value ??= {};
  for (const name of Object.keys(previous)) {
    if (value[name] == null) node.style.removeProperty(name);
  }
  for (const name of Object.keys(value)) {
    if (previous[name] !== value[name]) node.style.setProperty(name, value[name]);
  }
  return { ...value };
}

function assign(node, name, value, previous, svg) {
  if (name === "children" || name === "ref") return value;
  if (name === "classList") return updateClassList(node, value, previous);
  if (name === "style") return updateStyle(node, value, previous);
  if (/^on[A-Z]/.test(name)) {
    const event = name.slice(2).toLowerCase();
    if (previous !== value) {
      if (typeof previous === "function") node.removeEventListener(event, previous);
      if (typeof value === "function") node.addEventListener(event, value);
    }
    return value;
  }
  if (name === "className") name = "class";
  if (name === "htmlFor") name = "for";
  if (booleanProperties.has(name)) {
    node[name] = Boolean(value);
    return value;
  }
  if (stringProperties.has(name)) {
    node[name] = value == null ? "" : String(value);
    return value;
  }
  const separator = svg ? name.indexOf(":") : -1;
  const prefix = separator > 0 ? name.slice(0, separator) : "";
  const namespace =
    prefix === "xlink" ? xlinkNamespace : prefix === "xml" ? xmlNamespace : null;
  if (namespace) {
    const local = name.slice(separator + 1);
    if (value == null || value === false) node.removeAttributeNS(namespace, local);
    else node.setAttributeNS(namespace, local, String(value));
  } else if (value == null || value === false) node.removeAttribute(name);
  else node.setAttribute(name, value === true ? "" : String(value));
  return value;
}

export function domSpread(node, props, previous, svg) {
  for (const name of Object.keys(previous)) {
    if (!(name in props)) {
      assign(node, name, null, previous[name], svg);
      delete previous[name];
    }
  }
  for (const name of Object.keys(props)) {
    if (name === "children" || name === "ref") continue;
    const value = props[name];
    if (previous[name] === value && name !== "classList" && name !== "style") {
      continue;
    }
    previous[name] = assign(node, name, value, previous[name], svg);
  }
  return previous;
}

export function domAddEventListener(node, event, callback) {
  const registration = { node, event, callback };
  node.addEventListener(event, callback);
  return registration;
}

export function domRemoveEventListener(registration) {
  registration?.node.removeEventListener(registration.event, registration.callback);
}

function parentFor(node) {
  return eventHosts?.get(node) ?? node.parentNode ?? node.host;
}

function eventTarget(event) {
  let target = event.target;
  if (event.composed && target?.shadowRoot) {
    target = event.composedPath?.()[0] ?? target;
  }
  return target;
}

function dispatch(event) {
  const target = eventTarget(event);
  const state = { current: target, event, target };
  let node = target;
  while (node && node !== document) {
    const entry = delegatedHandlers.get(node)?.get(event.type);
    if (entry && !node.disabled) {
      state.current = node;
      if (entry.usesEvent) entry.callback(state);
      else entry.callback();
    }
    if (event.cancelBubble) return;
    node = parentFor(node);
  }
}

function dispatchClick(event) {
  let node = eventTarget(event);
  while (node && node !== document) {
    const callback = node[clickHandler];
    if (callback && !node.disabled) callback();
    if (event.cancelBubble) return;
    node = parentFor(node);
  }
}

function addDelegatedEvent(node, event, callback, usesEvent) {
  let handlers = delegatedHandlers.get(node);
  if (!handlers) {
    handlers = new Map();
    delegatedHandlers.set(node, handlers);
  }
  const previous = handlers.get(event);
  const registration = { callback, event, target: node, usesEvent };
  handlers.set(event, registration);
  if (!previous) {
    const root = delegatedRoots.get(event);
    if (root) root.count += 1;
    else {
      const listener = (value) => dispatch(value);
      delegatedRoots.set(event, { count: 1, listener });
      document.addEventListener(event, listener);
    }
  }
  return registration;
}

export const domAddDelegatedEvent = (node, event, callback) =>
  addDelegatedEvent(node, event, callback, true);
export const domAddDelegatedEventVoid = (node, event, callback) =>
  addDelegatedEvent(node, event, callback, false);

export function domAddDelegatedClickVoid(node, callback) {
  const previous = node[clickHandler];
  node[clickHandler] = callback;
  if (!previous && clickCount++ === 0) {
    document.addEventListener("click", dispatchClick);
  }
  return [node, callback];
}

export function domSetDelegatedClickVoid(node, callback) {
  node[clickHandler] = callback;
  if (clickCount === 0) {
    clickCount = 1;
    document.addEventListener("click", dispatchClick);
  }
}

export function domRemoveDelegatedClick(registration) {
  if (!registration) return;
  const [node, callback] = registration;
  if (node[clickHandler] !== callback) return;
  delete node[clickHandler];
  if (--clickCount === 0) document.removeEventListener("click", dispatchClick);
}

export function domRemoveDelegatedEvent(registration) {
  if (!registration) return;
  const handlers = delegatedHandlers.get(registration.target);
  if (handlers?.get(registration.event) !== registration) return;
  handlers.delete(registration.event);
  if (handlers.size === 0) delegatedHandlers.delete(registration.target);
  const root = delegatedRoots.get(registration.event);
  root.count -= 1;
  if (root.count === 0) {
    document.removeEventListener(registration.event, root.listener);
    delegatedRoots.delete(registration.event);
  }
}

export function domClearDelegatedEvents() {
  for (const [event, root] of delegatedRoots) {
    document.removeEventListener(event, root.listener);
  }
  delegatedRoots.clear();
  delegatedHandlers = new WeakMap();
  if (clickCount) document.removeEventListener("click", dispatchClick);
  clickCount = 0;
}

export const domEventTarget = (state) => state.target;
export const domEventCurrentTarget = (state) => state.current;
export const domEventType = (state) => state.event.type;
export const domEventDefaultPrevented = (state) => state.event.defaultPrevented;
export const domEventPreventDefault = (state) => state.event.preventDefault();
export const domEventStopPropagation = (state) => state.event.stopPropagation();
export const domSetEventHost = (node, host) =>
  (eventHosts ??= new WeakMap()).set(node, host);
export const domIsHead = (node) => node === document.head;
export const domAttachShadow = (node) => node.attachShadow({ mode: "open" });
export const domClear = (node) => node.replaceChildren();
export const hostSchedule = (callback) => queueMicrotask(callback);
