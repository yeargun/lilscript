// DOM and ECMAScript primitives used by the LilScript runtime-dom owners.

const doc = typeof document !== "undefined" ? document : null;
const templateContainer = doc && doc.createElement("template");

export function hostTrustedTypes() {
  return typeof window !== "undefined" ? window.trustedTypes : undefined;
}

export function hostCreateTrustedTypesPolicy(trustedTypes, name, createHTML) {
  return trustedTypes.createPolicy(name, { createHTML });
}

export function hostInsert(child, parent, anchor) {
  parent.insertBefore(child, anchor || null);
}

export function hostRemove(child) {
  const parent = child.parentNode;
  if (parent) parent.removeChild(child);
}

export function hostCreateElement(tag, namespace, is) {
  if (namespace === "svg") {
    return doc.createElementNS("http://www.w3.org/2000/svg", tag);
  }
  if (namespace === "mathml") {
    return doc.createElementNS("http://www.w3.org/1998/Math/MathML", tag);
  }
  return is ? doc.createElement(tag, { is }) : doc.createElement(tag);
}

export function hostCreateText(text) {
  return doc.createTextNode(text);
}

export function hostCreateComment(text) {
  return doc.createComment(text);
}

export function hostQuerySelector(selector) {
  return doc.querySelector(selector);
}

export function hostSetAttribute(element, name, value) {
  element.setAttribute(name, value);
}

export function hostRemoveAttribute(element, name) {
  element.removeAttribute(name);
}

export function hostSetAttributeNS(element, namespace, name, value) {
  element.setAttributeNS(namespace, name, value);
}

export function hostRemoveAttributeNS(element, namespace, name) {
  element.removeAttributeNS(namespace, name);
}

export function hostGetAttribute(element, name) {
  return element.getAttribute(name);
}

export function hostSetNodeValue(node, value) {
  node.nodeValue = value;
}

export function hostSetTextContent(element, value) {
  element.textContent = value;
}

export function hostParentNode(node) {
  return node.parentNode;
}

export function hostNextSibling(node) {
  return node.nextSibling;
}

export function hostPreviousSibling(node) {
  return node.previousSibling;
}

export function hostFirstChild(node) {
  return node.firstChild;
}

export function hostLastChild(node) {
  return node.lastChild;
}

export function hostCloneNode(node) {
  return node.cloneNode(true);
}

export function hostCloneNodeShallow(node) {
  return node.cloneNode();
}

export function hostAppendChild(parent, child) {
  parent.appendChild(child);
}

export function hostRemoveChild(parent, child) {
  parent.removeChild(child);
}

export function hostSetTemplateHTML(value) {
  templateContainer.innerHTML = value;
}

export function hostTemplateContent() {
  return templateContainer.content;
}

export function hostAddEventListener(element, event, handler, options) {
  element.addEventListener(event, handler, options);
}

export function hostRemoveEventListener(element, event, handler, options) {
  element.removeEventListener(event, handler, options);
}

export function hostRead(value, key) {
  return value == null ? undefined : value[key];
}

export function hostWrite(value, key, next) {
  value[key] = next;
}

export function hostDelete(value, key) {
  return delete value[key];
}

export function hostHasProperty(value, key) {
  return key in value;
}

export function hostObjectKeys(value) {
  return Object.keys(value);
}

export function hostArrayFrom(value) {
  return Array.from(value);
}

export function hostArraySlice(value) {
  return value.slice();
}

export function hostArgumentsSlice(value, start) {
  return Array.prototype.slice.call(value, start);
}

export function hostArraySplice(value, index, count) {
  value.splice(index, count);
}

export function hostCreateSet(value) {
  return new Set(value);
}

export function hostSetHas(value, item) {
  return value.has(item);
}

export function hostSetAdd(value, item) {
  value.add(item);
}

export function hostSetDelete(value, item) {
  value.delete(item);
}

export function hostSetSize(value) {
  return value.size;
}

export function hostCall0(fn) {
  return Reflect.apply(fn, undefined, []);
}

export function hostCall1(fn, first) {
  return Reflect.apply(fn, undefined, [first]);
}

export function hostCall2(fn, first, second) {
  return Reflect.apply(fn, undefined, [first, second]);
}

export function hostCall3(fn, first, second, third) {
  return Reflect.apply(fn, undefined, [first, second, third]);
}

export function hostCall4(fn, first, second, third, fourth) {
  return Reflect.apply(fn, undefined, [first, second, third, fourth]);
}

export function hostCallMethod0(receiver, name) {
  return Reflect.apply(receiver[name], receiver, []);
}

export function hostCallMethod1(receiver, name, first) {
  return Reflect.apply(receiver[name], receiver, [first]);
}

export function hostCallMethod2(receiver, name, first, second) {
  return Reflect.apply(receiver[name], receiver, [first, second]);
}

export function hostCallMethod3(receiver, name, first, second, third) {
  return Reflect.apply(receiver[name], receiver, [first, second, third]);
}

export function hostCallWithEventArgs(fn, event, args) {
  return Reflect.apply(fn, undefined, [event, ...args]);
}

export function hostApply(fn, receiver, args) {
  return Reflect.apply(fn, receiver, args);
}

export function hostFunctionRest(callback) {
  return function (...args) {
    return callback(args);
  };
}

export function hostFunction1Rest(name, callback) {
  const fn = function (first) {
    return Reflect.apply(callback, undefined, [this, arguments]);
  };
  Object.defineProperty(fn, "name", { configurable: true, value: name });
  return fn;
}

export function hostEventRestWrapper(callback) {
  return function (event, ...args) {
    return callback(event, args);
  };
}

export function hostString(value) {
  return String(value);
}

export function hostRawType(value) {
  return Object.prototype.toString.call(value).slice(8, -1);
}

export function hostDateNow() {
  return Date.now();
}

export function hostResolvedPromise() {
  return Promise.resolve();
}

export function hostPromiseThen(promise, callback) {
  return promise.then(callback);
}

export function hostCreateSymbol(description) {
  return Symbol(description);
}

export function hostCreateObject() {
  return {};
}

export function hostCreateNullObject() {
  return Object.create(null);
}

export function hostCreateMap() {
  return new Map();
}

export function hostCreateWeakMap() {
  return new WeakMap();
}

export function hostCreateWeakSet() {
  return new WeakSet();
}

export function hostSetPrototypeOf(value, prototype) {
  Object.setPrototypeOf(value, prototype);
}

export function hostGetPrototypeOf(value) {
  return Object.getPrototypeOf(value);
}

export function hostDefineValue(value, key, next, writable = true) {
  Object.defineProperty(value, key, {
    configurable: false,
    enumerable: false,
    writable,
    value: next,
  });
}

export function hostDefineAccessor(value, key, get, set) {
  Object.defineProperty(value, key, {
    configurable: false,
    enumerable: false,
    get,
    set,
  });
}

export function hostDefineMethod(prototype, key, method) {
  Object.defineProperty(method, "name", { configurable: true, value: key });
  Object.defineProperty(prototype, key, {
    configurable: true,
    enumerable: false,
    writable: true,
    value: method,
  });
}

export function hostCreateVueElementClass(initialize, methods) {
  const Base = typeof HTMLElement !== "undefined" ? HTMLElement : class {};
  class VueElement extends Base {
    constructor(def, ...args) {
      super();
      Reflect.apply(initialize, this, [def, args[0], args[1]]);
    }
  }
  for (const [name, method] of Object.entries(methods)) {
    hostDefineMethod(VueElement.prototype, name, method);
  }
  return VueElement;
}

export function hostCreateCustomElementClass(base, definition, createApp) {
  class VueCustomElement extends base {
    static def = definition;
    constructor(initialProps) {
      super(definition, initialProps, createApp);
    }
  }
  return VueCustomElement;
}

export function hostInstanceOf(value, constructor) {
  return typeof constructor === "function" && value instanceof constructor;
}

export function hostIsElement(value) {
  return typeof Element !== "undefined" && value instanceof Element;
}

export function hostIsStyleElement(value) {
  return typeof HTMLStyleElement !== "undefined" && value instanceof HTMLStyleElement;
}

export function hostIsClosedShadowRoot(value) {
  return (
    typeof window !== "undefined" &&
    typeof window.ShadowRoot === "function" &&
    value instanceof window.ShadowRoot &&
    value.mode === "closed"
  );
}

export function hostResolveRootNamespace(value) {
  if (typeof SVGElement !== "undefined" && value instanceof SVGElement) return "svg";
  if (typeof MathMLElement === "function" && value instanceof MathMLElement) return "mathml";
  return undefined;
}

export function hostIsDocumentOrShadowRoot(value) {
  return (
    (typeof Document !== "undefined" && value instanceof Document) ||
    (typeof ShadowRoot !== "undefined" && value instanceof ShadowRoot)
  );
}

export function hostCreateMutationObserver(callback) {
  return new MutationObserver(callback);
}

export function hostCreateCustomEvent(name, options) {
  return new CustomEvent(name, options);
}

export function hostCreateEvent(name) {
  return new Event(name);
}

export function hostCreateStyleElement() {
  return document.createElement("style");
}

export function hostCreateTreeWalker(root, whatToShow) {
  return document.createTreeWalker(root, whatToShow);
}

export function hostQuerySelectorAll(selector) {
  return document.querySelectorAll(selector);
}

export function hostGetComputedStyle(element) {
  return window.getComputedStyle(element);
}

export function hostRequestAnimationFrame(callback) {
  return requestAnimationFrame(callback);
}

export function hostSetTimeout(callback, timeout) {
  return setTimeout(callback, timeout);
}

export function hostNumber(value) {
  return Number(value);
}

export function hostNumberIsFinite(value) {
  return Number.isFinite(value);
}

export function hostMathAbs(value) {
  return Math.abs(value);
}

export function hostMathMax(left, right) {
  return Math.max(left, right);
}

export function hostBodyOffsetHeight(node) {
  const targetDocument = node ? node.ownerDocument : document;
  return targetDocument.body.offsetHeight;
}

export function hostConstructors() {
  return { Array, Boolean, Function, Number, Object, String };
}

export function hostIsNumberConstructor(value) {
  return value === Number;
}

export function hostActiveElementIs(element) {
  const root = element.getRootNode();
  return (
    ((typeof Document !== "undefined" && root instanceof Document) ||
      (typeof ShadowRoot !== "undefined" && root instanceof ShadowRoot)) &&
    root.activeElement === element
  );
}

export function hostWarn(message, error) {
  if (error === undefined) console.warn(`[Vue warn]: ${message}`);
  else console.warn(`[Vue warn]: ${message}`, error);
}
