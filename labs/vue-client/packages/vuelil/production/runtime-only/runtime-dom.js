// Generated from src/runtime-dom/index.lil and its measured DOM host adapter.
// DOM and ECMAScript primitives used by the LilScript runtime-dom owners.

const doc = typeof document !== "undefined" ? document : null;
const templateContainer = doc && doc.createElement("template");

function hostTrustedTypes() {
  return typeof window !== "undefined" ? window.trustedTypes : undefined;
}

function hostCreateTrustedTypesPolicy(trustedTypes, name, createHTML) {
  return trustedTypes.createPolicy(name, { createHTML });
}

function hostInsert(child, parent, anchor) {
  parent.insertBefore(child, anchor || null);
}

function hostRemove(child) {
  const parent = child.parentNode;
  if (parent) parent.removeChild(child);
}

function hostCreateElement(tag, namespace, is) {
  if (namespace === "svg") {
    return doc.createElementNS("http://www.w3.org/2000/svg", tag);
  }
  if (namespace === "mathml") {
    return doc.createElementNS("http://www.w3.org/1998/Math/MathML", tag);
  }
  return is ? doc.createElement(tag, { is }) : doc.createElement(tag);
}

function hostCreateText(text) {
  return doc.createTextNode(text);
}

function hostCreateComment(text) {
  return doc.createComment(text);
}

function hostQuerySelector(selector) {
  return doc.querySelector(selector);
}

function hostSetAttribute(element, name, value) {
  element.setAttribute(name, value);
}

function hostRemoveAttribute(element, name) {
  element.removeAttribute(name);
}

function hostSetAttributeNS(element, namespace, name, value) {
  element.setAttributeNS(namespace, name, value);
}

function hostRemoveAttributeNS(element, namespace, name) {
  element.removeAttributeNS(namespace, name);
}

function hostGetAttribute(element, name) {
  return element.getAttribute(name);
}

function hostSetNodeValue(node, value) {
  node.nodeValue = value;
}

function hostSetTextContent(element, value) {
  element.textContent = value;
}

function hostParentNode(node) {
  return node.parentNode;
}

function hostNextSibling(node) {
  return node.nextSibling;
}

function hostPreviousSibling(node) {
  return node.previousSibling;
}

function hostFirstChild(node) {
  return node.firstChild;
}

function hostLastChild(node) {
  return node.lastChild;
}

function hostCloneNode(node) {
  return node.cloneNode(true);
}

function hostCloneNodeShallow(node) {
  return node.cloneNode();
}

function hostAppendChild(parent, child) {
  parent.appendChild(child);
}

function hostRemoveChild(parent, child) {
  parent.removeChild(child);
}

function hostSetTemplateHTML(value) {
  templateContainer.innerHTML = value;
}

function hostTemplateContent() {
  return templateContainer.content;
}

function hostAddEventListener(element, event, handler, options) {
  element.addEventListener(event, handler, options);
}

function hostRemoveEventListener(element, event, handler, options) {
  element.removeEventListener(event, handler, options);
}

function hostRead(value, key) {
  return value == null ? undefined : value[key];
}

function hostWrite(value, key, next) {
  value[key] = next;
}

function hostDelete(value, key) {
  return delete value[key];
}

function hostHasProperty(value, key) {
  return key in value;
}

function hostObjectKeys(value) {
  return Object.keys(value);
}

function hostArrayFrom(value) {
  return Array.from(value);
}

function hostArraySlice(value) {
  return value.slice();
}

function hostArgumentsSlice(value, start) {
  return Array.prototype.slice.call(value, start);
}

function hostArraySplice(value, index, count) {
  value.splice(index, count);
}

function hostCreateSet(value) {
  return new Set(value);
}

function hostSetHas(value, item) {
  return value.has(item);
}

function hostSetAdd(value, item) {
  value.add(item);
}

function hostSetDelete(value, item) {
  value.delete(item);
}

function hostSetSize(value) {
  return value.size;
}

function hostCall0(fn) {
  return Reflect.apply(fn, undefined, []);
}

function hostCall1(fn, first) {
  return Reflect.apply(fn, undefined, [first]);
}

function hostCall2(fn, first, second) {
  return Reflect.apply(fn, undefined, [first, second]);
}

function hostCall3(fn, first, second, third) {
  return Reflect.apply(fn, undefined, [first, second, third]);
}

function hostCall4(fn, first, second, third, fourth) {
  return Reflect.apply(fn, undefined, [first, second, third, fourth]);
}

function hostCallMethod0(receiver, name) {
  return Reflect.apply(receiver[name], receiver, []);
}

function hostCallMethod1(receiver, name, first) {
  return Reflect.apply(receiver[name], receiver, [first]);
}

function hostCallMethod2(receiver, name, first, second) {
  return Reflect.apply(receiver[name], receiver, [first, second]);
}

function hostCallMethod3(receiver, name, first, second, third) {
  return Reflect.apply(receiver[name], receiver, [first, second, third]);
}

function hostCallWithEventArgs(fn, event, args) {
  return Reflect.apply(fn, undefined, [event, ...args]);
}

function hostApply(fn, receiver, args) {
  return Reflect.apply(fn, receiver, args);
}

function hostFunctionRest(callback) {
  return function (...args) {
    return callback(args);
  };
}

function hostFunction1Rest(name, callback) {
  const fn = function (first) {
    return Reflect.apply(callback, undefined, [this, arguments]);
  };
  Object.defineProperty(fn, "name", { configurable: true, value: name });
  return fn;
}

function hostEventRestWrapper(callback) {
  return function (event, ...args) {
    return callback(event, args);
  };
}

function hostString(value) {
  return String(value);
}

function hostRawType(value) {
  return Object.prototype.toString.call(value).slice(8, -1);
}

function hostDateNow() {
  return Date.now();
}

function hostResolvedPromise() {
  return Promise.resolve();
}

function hostPromiseThen(promise, callback) {
  return promise.then(callback);
}

function hostCreateSymbol(description) {
  return Symbol(description);
}

function hostCreateObject() {
  return {};
}

function hostCreateNullObject() {
  return Object.create(null);
}

function hostCreateMap() {
  return new Map();
}

function hostCreateWeakMap() {
  return new WeakMap();
}

function hostCreateWeakSet() {
  return new WeakSet();
}

function hostSetPrototypeOf(value, prototype) {
  Object.setPrototypeOf(value, prototype);
}

function hostGetPrototypeOf(value) {
  return Object.getPrototypeOf(value);
}

function hostDefineValue(value, key, next, writable = true) {
  Object.defineProperty(value, key, {
    configurable: false,
    enumerable: false,
    writable,
    value: next,
  });
}

function hostDefineAccessor(value, key, get, set) {
  Object.defineProperty(value, key, {
    configurable: false,
    enumerable: false,
    get,
    set,
  });
}

function hostDefineMethod(prototype, key, method) {
  Object.defineProperty(method, "name", { configurable: true, value: key });
  Object.defineProperty(prototype, key, {
    configurable: true,
    enumerable: false,
    writable: true,
    value: method,
  });
}

function hostCreateVueElementClass(initialize, methods) {
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

function hostCreateCustomElementClass(base, definition, createApp) {
  class VueCustomElement extends base {
    static def = definition;
    constructor(initialProps) {
      super(definition, initialProps, createApp);
    }
  }
  return VueCustomElement;
}

function hostInstanceOf(value, constructor) {
  return typeof constructor === "function" && value instanceof constructor;
}

function hostIsElement(value) {
  return typeof Element !== "undefined" && value instanceof Element;
}

function hostIsStyleElement(value) {
  return typeof HTMLStyleElement !== "undefined" && value instanceof HTMLStyleElement;
}

function hostIsClosedShadowRoot(value) {
  return (
    typeof window !== "undefined" &&
    typeof window.ShadowRoot === "function" &&
    value instanceof window.ShadowRoot &&
    value.mode === "closed"
  );
}

function hostResolveRootNamespace(value) {
  if (typeof SVGElement !== "undefined" && value instanceof SVGElement) return "svg";
  if (typeof MathMLElement === "function" && value instanceof MathMLElement) return "mathml";
  return undefined;
}

function hostIsDocumentOrShadowRoot(value) {
  return (
    (typeof Document !== "undefined" && value instanceof Document) ||
    (typeof ShadowRoot !== "undefined" && value instanceof ShadowRoot)
  );
}

function hostCreateMutationObserver(callback) {
  return new MutationObserver(callback);
}

function hostCreateCustomEvent(name, options) {
  return new CustomEvent(name, options);
}

function hostCreateEvent(name) {
  return new Event(name);
}

function hostCreateStyleElement() {
  return document.createElement("style");
}

function hostCreateTreeWalker(root, whatToShow) {
  return document.createTreeWalker(root, whatToShow);
}

function hostQuerySelectorAll(selector) {
  return document.querySelectorAll(selector);
}

function hostGetComputedStyle(element) {
  return window.getComputedStyle(element);
}

function hostRequestAnimationFrame(callback) {
  return requestAnimationFrame(callback);
}

function hostSetTimeout(callback, timeout) {
  return setTimeout(callback, timeout);
}

function hostNumber(value) {
  return Number(value);
}

function hostNumberIsFinite(value) {
  return Number.isFinite(value);
}

function hostMathAbs(value) {
  return Math.abs(value);
}

function hostMathMax(left, right) {
  return Math.max(left, right);
}

function hostBodyOffsetHeight(node) {
  const targetDocument = node ? node.ownerDocument : document;
  return targetDocument.body.offsetHeight;
}

function hostConstructors() {
  return { Array, Boolean, Function, Number, Object, String };
}

function hostIsNumberConstructor(value) {
  return value === Number;
}

function hostActiveElementIs(element) {
  const root = element.getRootNode();
  return (
    ((typeof Document !== "undefined" && root instanceof Document) ||
      (typeof ShadowRoot !== "undefined" && root instanceof ShadowRoot)) &&
    root.activeElement === element
  );
}

function hostWarn(message, error) {
  if (error === undefined) console.warn(`[Vue warn]: ${message}`);
  else console.warn(`[Vue warn]: ${message}`, error);
}

const runtimeDomApply=hostApply;const runtimeDomIsElement=hostIsElement;const runtimeDomQuerySelector=hostQuerySelector;const runtimeDomRead=hostRead;const runtimeDomRemoveAttribute=hostRemoveAttribute;const runtimeDomResolveRootNamespace=hostResolveRootNamespace;const runtimeDomSetAttribute=hostSetAttribute;const runtimeDomSetTextContent=hostSetTextContent;const runtimeDomString=hostString;const runtimeDomWrite=hostWrite;
import{Fragment,computed,createElementBlock,createElementVNode,createRenderer as runtimeDomCreateRenderer,nextTick,openBlock,reactive,renderList,toDisplayString}from"./runtime-core.js";import{extend as runtimeDomExtend,isFunction as runtimeDomIsFunction,isString as runtimeDomIsString}from"./shared.js";import{warn as nodeOpsWarn}from"./runtime-core.js";import{camelize as patchPropCamelize,isArray as patchPropIsArray,isFunction as patchPropIsFunction,isModelListener as patchPropIsModelListener,isOn as patchPropIsOn,isString as patchPropIsString}from"./shared.js";import{BaseTransition as dependencyBaseTransition,BaseTransitionPropsValidators as dependencyBaseTransitionPropsValidators,h as dependencyH}from"./runtime-core.js";import{extend as transitionExtend,isArray as transitionIsArray,isObject as transitionIsObject,toNumber as transitionToNumber}from"./shared.js";import{camelize as styleCamelize,capitalize as styleCapitalize,hyphenate as styleHyphenate,isArray as styleIsArray,isString as styleIsString}from"./shared.js";import{includeBooleanAttr as attrIncludeBooleanAttr,isSpecialBooleanAttr as attrIsSpecialBooleanAttr,isSymbol as attrIsSymbol}from"./shared.js";import{DeprecationTypes as propDeprecationTypes,compatUtils as propCompatUtils,warn as propWarn}from"./runtime-core.js";import{includeBooleanAttr as propIncludeBooleanAttr}from"./shared.js";import{ErrorCodes as eventErrorCodes,callWithAsyncErrorHandling as eventCallWithAsyncErrorHandling}from"./runtime-core.js";import{hyphenate as eventHyphenate,isArray as eventIsArray}from"./shared.js";const ie=Fragment,se=computed,oe=createElementBlock,ue=createElementVNode,ae=nextTick,ce=openBlock,le=reactive,de=renderList,he=toDisplayString;function k(e){e=transitionToNumber(e);return+e}function E(e){return y!=null&&y?y.createHTML(e):e}function U(e){if(e.length<3)return!1;var t=e.charCodeAt(2);return 111==e.charCodeAt(0)&&110==e.charCodeAt(1)&&t>96&&t<123}function Q(){if(0!=p)return p;p=hostDateNow(),hostPromiseThen(F,function(){p=0});return p}function ne(e){return runtimeDomIsString(e)?(e=runtimeDomQuerySelector(runtimeDomString(e)),e):e}function X(e,t,s,o){return"TEXTAREA"===hostRead(e,"tagName")&&("width"==t||"height"==t)&&styleIsString(o)&&s===o}function ee(e,t,s,o){if(o)return"innerHTML"==t||"textContent"==t?!0:hostHasProperty(e,t)&&U(t)&&patchPropIsFunction(s)?!0:!1;if("spellcheck"==t||"draggable"==t||"translate"==t||"autocorrect"==t)return!1;var i=hostRead(e,"tagName"),r=hostString(i);return"sandbox"==t&&"IFRAME"==r?!1:"form"==t?!1:"list"==t&&"INPUT"==r?!1:"type"==t&&"TEXTAREA"==r?!1:("width"==t||"height"==t)&&("IMG"==r||"VIDEO"==r||"CANVAS"==r||"SOURCE"==r)?!1:U(t)&&patchPropIsString(s)?!1:hostHasProperty(e,t)}function K(e,t){var o,s=hostRead(w,t);if(s!=null&&s)return hostString(s);s=hostString(hostCall1(styleCamelize,t));if("filter"!=s&&hostHasProperty(e,s))return hostWrite(w,t,s),s;var r=hostString(hostCall1(styleCapitalize,s));for(o=0;o<j.length;o++){s=(j[o]||"")+r;if(hostHasProperty(e,s))return hostWrite(w,t,s),s}return t}function J(e){var t,s=void 0;while(!0){t=H.exec(e);if(t==null||P.test(e))break;s===void 0&&(s={}),t=hostString(hostRead(t,1)),hostWrite(s,t.toLowerCase(),!0),e=e.slice(0,e.length-t.length)}t=hostString(hostCall1(eventHyphenate,e.slice(2)));58==e.charCodeAt(2)&&(t=e.slice(3));return[t,s]}function g(e,t,s){if(styleIsArray(s)){for(var r=+hostRead(s,"length")|0,o=0;o<r;o++)g(e,t,hostRead(s,o));return}s==null&&(s="");o=hostString(s),r=v.test(o),t.startsWith("--")?r?hostCallMethod3(e,"setProperty",t,o.replace(v,""),"important"):hostCallMethod2(e,"setProperty",t,s):(t=K(e,t),r?(t=hostString(hostCall1(styleHyphenate,t)),hostCallMethod3(e,"setProperty",t,o.replace(v,""),"important")):hostWrite(e,t,s))}function m(e,t){e=hostRead(e,t),e==null&&(e="");return hostString(e).split(", ")}function N(e){return"auto"==e?0:hostNumber(hostString(e.slice(0,-1).replace(",",".")))*1e3}function B(e,t){var s=hostGetComputedStyle(e);e=m(s,"transitionDelay");var n=m(s,"transitionDuration"),o=R(e,n);e=m(s,"animationDelay");var h=m(s,"animationDuration"),r=R(e,h);e=null;if("transition"===t){o>0?(t=a(n),e="transition"):(o=0,t=0);var i=o}else{"animation"===t?(r>0?(t=a(h),e="animation"):(r=0,t=0),i=r):(i=hostMathMax(o,r),i>0?o>r?(t=a(n),e="transition"):(t=a(h),e="animation"):t=0)}"transition"===e?(o=A,s=o.test(hostString(m(s,"transitionProperty").toString()))):s=!1;return{type:e,timeout:i,propCount:t,hasTransform:s}}function R(e,t){while(a(e)<a(t))e=e.concat(e);for(var r,i=a(t),o=0,s=0;s<i;s++)r=N(hostString(hostRead(t,s))),o=hostMathMax(o,r+N(hostString(hostRead(e,s))));return o}function c(e,t){for(var o,r=t.split(new RegExp("\\s+")),i=a(r),s=0;s<i;s++)o=hostString(hostRead(r,s)),""!=o&&hostRead(e,"classList").add(o);s=hostRead(e,h),(s==null||!s)&&(s=hostCreateSet(void 0),hostWrite(e,h,s)),hostSetAdd(s,t)}function d(e,t){for(var o,r=t.split(new RegExp("\\s+")),i=a(r),s=0;s<i;s++)o=hostString(hostRead(r,s)),""!=o&&hostRead(e,"classList").remove(o);s=hostRead(e,h),s!=null&&s&&(hostSetDelete(s,t),0==hostSetSize(s)&&hostWrite(e,h,void 0))}function W(e,t,s){var o=hostRead(e,h);o!=null&&o&&(o=hostArrayFrom(o),t!=null&&""!=hostString(t)&&o.unshift(t),t=o.join(" ")),t==null?hostRemoveAttribute(e,"class"):s?hostSetAttribute(e,"class",t):hostWrite(e,"className",t)}function S(e,t,s,o,r){var i=!!hostCall1(attrIsSpecialBooleanAttr,t);r===void 0||(i=!!r);if(o&&t.startsWith("xlink:")){s==null?hostRemoveAttributeNS(e,"http://www.w3.org/1999/xlink",t.slice(6)):hostSetAttributeNS(e,"http://www.w3.org/1999/xlink",t,s);return}s==null||i&&!attrIncludeBooleanAttr(s)?hostRemoveAttribute(e,t):(i?s="":attrIsSymbol(s)&&(s=hostString(s)),hostSetAttribute(e,t,s))}function O(e,t,s,o,r){if("innerHTML"==t||"textContent"==t){s==null||("innerHTML"==t&&(s=E(s)),hostWrite(e,t,s));return}var n=e.tagName,i=hostString(n);if("value"==t&&"PROGRESS"!=i&&!i.includes("-")){o=e.value,"OPTION"==i&&(o=hostGetAttribute(e,"value"),(o==null||!o)&&(o="")),s==null?(r=e.type,r="checkbox"===r?"on":""):r=hostString(s),(o!==r||!hostHasProperty(e,"_value"))&&hostWrite(e,"value",r),s==null&&hostRemoveAttribute(e,t),hostWrite(e,"_value",s);return}if(""===s||s==null)o=typeof e[t],"boolean"==o?(s=propIncludeBooleanAttr(s),o=!1):s==null&&"string"==o?(s="",o=!0):"number"==o?(s=0,o=!0):o=!1;else{var h,a=!1;a&&propCompatUtils.isCompatEnabled.call(propCompatUtils,propDeprecationTypes.ATTR_FALSE_VALUE,o)?(o=typeof e[t],"string"==o||"number"==o?(s="number"==o?0:"",o=!0):o=!1):o=!1}try{hostWrite(e,t,s)}catch(e){(h=!1)&&propWarn("Failed setting prop \""+t+"\" on <"+i.toLowerCase()+">: value "+hostString(s)+" is invalid.",e)}o&&(r==null||(t=hostString(r)),hostRemoveAttribute(e,t))}function Z(e,t,s){var n=hostRead(e,"style"),r=styleIsString(s);if(s!=null&&s&&!r){if(t!=null&&t&&!styleIsString(t))for(var o in t)hostRead(s,o)==null&&g(n,o,"");if(t!=null&&t&&styleIsString(t))for(r=hostString(t).split(";"),o=0;o<r.length;o++){var i=r[o]||"";i=i.slice(0,i.indexOf(":")).trim(),hostRead(s,i)==null&&g(n,i,"")}r=!1;for(o in s){"display"==o&&(r=!0);i=hostRead(s,o);if(i!=null){var h=void 0;!styleIsString(t)&&t!=null&&t&&(h=hostRead(t,o)),X(e,o,h,i)||g(n,o,i)}else g(n,o,"")}}else{r?t!==s?(o=hostString(s),t=hostRead(n,L),t!=null&&t&&(o=o+(";"+hostString(t))),hostWrite(n,"cssText",o),r=D.test(o)):r=!1:(t!=null&&t&&hostRemoveAttribute(e,"style"),r=!1)}hostHasProperty(e,z)&&(t=r?hostRead(n,"display"):"",hostWrite(e,z,t),!hostRead(e,C)||hostWrite(n,"display","none"))}function $(e,t,s,o){var r=hostRead(e,q);r==null&&(r={},hostWrite(e,q,r));var i=hostRead(r,t);if(s!=null&&s&&i!=null&&i){hostWrite(i,"value",s);return}var n=J(t),a=hostRead(n,0),h=hostString(a);n=hostRead(n,1),s!=null&&s?(s=_(s,o),hostWrite(r,t,s),hostAddEventListener(e,h,s,n)):i!=null&&i&&(hostRemoveEventListener(e,h,i,n),hostWrite(r,t,void 0))}function I(e,t,s,o){b++;var a=b;hostWrite(e,"_endId",a);var d=function(){(+hostRead(e,"_endId")|0)==a&&o()};if(s!=null){hostSetTimeout(d,+s);return}t=B(e,t);s=hostRead(t,"type");var i,n,l;if(s==null||!s){o();return}s=hostString(s)+"end";var r=0;i=+hostRead(t,"propCount")|0;var h;n=function(){hostRemoveEventListener(e,s,h,void 0),d()},h=function(t){hostRead(t,"target")===e&&(r=r+1|0,r>=i&&n())},l=function(){r<i&&n()},hostSetTimeout(l,+hostRead(t,"timeout")+1),hostAddEventListener(e,s,h,void 0)}function _(e,t){var s=function(e){var i,r,n,h,o=hostRead(e,"_vts");if(o==null||!o)hostWrite(e,"_vts",hostDateNow());else{o=+o;if(o<=+hostRead(s,"attached"))return}r=hostRead(s,"value");if(eventIsArray(r))for(n=hostRead(e,"stopImmediatePropagation"),hostWrite(e,"stopImmediatePropagation",function(){hostCall0(n.bind(e)),hostWrite(e,"_stopped",!0)}),i=hostArraySlice(r),h=+hostRead(i,"length")|0,o=0;o<h;o++){if(hostRead(e,"_stopped"))break;r=hostRead(i,o),r!=null&&r&&eventCallWithAsyncErrorHandling(r,t,eventErrorCodes.NATIVE_EVENT_HANDLER,[e])}else eventCallWithAsyncErrorHandling(r,t,eventErrorCodes.NATIVE_EVENT_HANDLER,[e])};hostWrite(s,"value",e),hostWrite(s,"attached",Q());return s}function V(e){var s={};for(var t in e)hostHasProperty(x,t)||hostWrite(s,t,hostRead(e,t));if(!1===hostRead(e,"css"))return s;t=hostRead(e,"name")!==void 0?hostString(hostRead(e,"name")):"v";var A=hostRead(e,"type"),o=hostRead(e,"duration"),r=t+"-enter-from",i=t+"-enter-active",n=t+"-enter-to",m=r,R=i,p=n,h=t+"-leave-from",a=t+"-leave-active",S=t+"-leave-to";hostRead(e,"enterFromClass")===void 0||(r=hostString(hostRead(e,"enterFromClass"))),hostRead(e,"enterActiveClass")===void 0||(i=hostString(hostRead(e,"enterActiveClass"))),hostRead(e,"enterToClass")===void 0||(n=hostString(hostRead(e,"enterToClass"))),hostRead(e,"appearFromClass")===void 0||(m=hostString(hostRead(e,"appearFromClass"))),hostRead(e,"appearActiveClass")===void 0||(R=hostString(hostRead(e,"appearActiveClass"))),hostRead(e,"appearToClass")===void 0||(p=hostString(hostRead(e,"appearToClass"))),hostRead(e,"leaveFromClass")===void 0||(h=hostString(hostRead(e,"leaveFromClass"))),hostRead(e,"leaveActiveClass")===void 0||(a=hostString(hostRead(e,"leaveActiveClass"))),hostRead(e,"leaveToClass")===void 0||(S=hostString(hostRead(e,"leaveToClass")));var u="";new RegExp("-from$"),e=Y(o),u=t=null;var b=t;e==null||(u=hostRead(e,0),b=hostRead(e,1)),o=hostRead(s,"onBeforeEnter"),e=hostRead(s,"onEnter");var W=hostRead(s,"onEnterCancelled"),E=hostRead(s,"onLeave"),P=hostRead(s,"onLeaveCancelled"),g=hostRead(s,"onBeforeAppear");g===void 0&&(g=o);var y=hostRead(s,"onAppear");y===void 0&&(y=e);var C=hostRead(s,"onAppearCancelled");C===void 0&&(C=W),t=function(){var e=hostRead(arguments,0),r=!!hostRead(arguments,1),t=hostRead(arguments,2);hostWrite(e,"_enterCancelled",hostRead(arguments,3));var s=n,o=i;r&&(s=p,o=R),d(e,s),d(e,o),t!=null&&t&&t()};var v=function(e,t){hostWrite(e,"_isLeaving",!1),d(e,h),d(e,S),d(e,a),t!=null&&t&&t()},D=((e)=>function(s){let o=!!s;return function(s,i){var h=e;o&&(h=y);var a=function(){t(s,o,i)};l(h,[s,a]);var R=function(){var e=r,t=n;o&&(e=m,t=p),d(s,e),c(s,t),T(h)||I(s,A,u,a)};hostRequestAnimationFrame(function(){hostRequestAnimationFrame(R)})}})(e);e={},hostWrite(e,"onBeforeEnter",function(e){l(o,[e]),c(e,r),c(e,i)}),hostWrite(e,"onBeforeAppear",function(e){l(g,[e]),c(e,m),c(e,R)}),hostWrite(e,"onEnter",hostCall1(D,!1)),hostWrite(e,"onAppear",hostCall1(D,!0)),hostWrite(e,"onLeave",function(e,t){hostWrite(e,"_isLeaving",!0);var s=function(){v(e,t)};c(e,h),!hostRead(e,"_enterCancelled")?(hostBodyOffsetHeight(e),c(e,a)):(c(e,a),hostBodyOffsetHeight(e));var o=function(){if(!(!hostRead(e,"_isLeaving")))d(e,h),c(e,S),T(E)||I(e,A,b,s)};hostRequestAnimationFrame(function(){hostRequestAnimationFrame(o)}),l(E,[e,s])}),hostWrite(e,"onEnterCancelled",function(e){t(e,!1,void 0,!0),l(W,[e])}),hostWrite(e,"onAppearCancelled",function(e){t(e,!0,void 0,!0),l(C,[e])}),hostWrite(e,"onLeaveCancelled",function(e){v(e,void 0),l(P,[e])});return transitionExtend(s,e)}function te(e,t){e=hostRead(hostRead(e,"_def"),"props");var o,s;if(e==null||!e)return!1;o=hostString(hostCall1(patchPropCamelize,t));if(patchPropIsArray(e)){for(s=+hostRead(e,"length")|0,t=0;t<s;t++)if(hostString(hostCall1(patchPropCamelize,hostString(hostRead(e,t))))==o)return!0;return!1}for(t=hostObjectKeys(e),s=+hostRead(t,"length")|0,e=0;e<s;e++)if(hostString(hostCall1(patchPropCamelize,hostString(hostRead(t,e))))==o)return!0;return!1}function T(e){if(e==null||!e)return!1;if(transitionIsArray(e)){for(var s=a(e),t=0;t<s;t++)if((+hostRead(hostRead(e,t),"length")|0)>1)return!0;return!1}return(+hostRead(e,"length")|0)>1}function l(e,t=[]){if(transitionIsArray(e))for(var o=a(e),s=0;s<o;s++)hostRead(e,s).apply(void 0,t);else e!=null&&e&&e.apply(void 0,t)}function Y(e){if(e==null)return null;if(transitionIsObject(e))return[k(hostRead(e,"enter")),k(hostRead(e,"leave"))];e=k(e);return[e,e]}function a(e){return+hostRead(e,"length")|0}var y,e=hostTrustedTypes(),t;if(e!=null&&e)try{y=hostCreateTrustedTypesPolicy(e,"vue",function(e){return e})}catch(t){e=nodeOpsWarn,e("Error creating trusted types policy: "+hostString(t))}var o={insert:function(e,t,s){hostInsert(e,t,s)},remove:function(e){hostRemove(e)},createElement:function(e,t,s,o){e=hostString(e);var r=hostCreateElement(e,t,s);"select"==e&&o!=null&&o.multiple!=null&&hostSetAttribute(r,"multiple",o.multiple);return r},createText:function(e){return hostCreateText(hostString(e))},createComment:function(e){return hostCreateComment(hostString(e))},setText:function(e,t){hostSetNodeValue(e,hostString(t))},setElementText:function(e,t){hostSetTextContent(e,hostString(t))},parentNode:function(e){return hostParentNode(e)},nextSibling:function(e){return hostNextSibling(e)},querySelector:function(e){return hostQuerySelector(hostString(e))},setScopeId:function(e,t){hostSetAttribute(e,hostString(t),"")},insertStaticContent:function(e,t,s,o,r,i){var h=hostLastChild(t);s==null||(h=hostPreviousSibling(s));if(r!=null&&(r===i||hostNextSibling(r)!=null))while(!0){hostInsert(hostCloneNode(r),t,s);if(r===i)break;r=hostNextSibling(r);if(r==null)break}else{o=o!=null?hostString(o):"",e=hostString(e),"svg"==o?e="<svg>"+e+"</svg>":"mathml"==o&&(e="<math>"+e+"</math>");var n=E(e);hostSetTemplateHTML(n),n=hostTemplateContent();if("svg"==o||"mathml"==o){e=hostFirstChild(n);while(hostFirstChild(e)!=null)hostAppendChild(n,hostFirstChild(e));hostRemoveChild(n,e)}hostInsert(n,t,s)}e=hostFirstChild(t);h==null||(e=hostNextSibling(h)),t=hostLastChild(t),s==null||(t=hostPreviousSibling(s));return[e,t]}},h=hostCreateSymbol("_vtc");e=hostConstructors(),t=e.String;var n=e.String,r=e.Boolean;r={type:r,default:!0};var i=e.String,s=e.Number;i=[i,s,e.Object],s=e.String;var u=e.String,pe=e.String,fe=e.String,me=e.String,ge=e.String,ye=e.String,be=e.String,x={name:t,type:n,css:r,duration:i,enterFromClass:s,enterActiveClass:u,enterToClass:pe,appearFromClass:fe,appearActiveClass:me,appearToClass:ge,leaveFromClass:ye,leaveActiveClass:be,leaveToClass:e.String};n=transitionExtend({},dependencyBaseTransitionPropsValidators,x);var b=0,A=new RegExp("\\b(?:transform|all)(?:,|$)");t=function(e,t){return dependencyH(dependencyBaseTransition,V(e),hostRead(t,"slots"))},hostWrite(t,"displayName","Transition"),hostWrite(t,"props",n);var z=hostCreateSymbol("_vod"),C=hostCreateSymbol("_vsh"),L=hostCreateSymbol("CSS_VAR_TEXT"),D=new RegExp("(?:^|;)\\s*display\\s*:");new RegExp("[^\\\\];\\s*$");var v=new RegExp("\\s*!important$"),j=["Webkit","Moz","ms"],w={},q=hostCreateSymbol("_vei"),H=new RegExp("(Once|Passive|Capture)$"),P=new RegExp("^on:?(?:Once|Passive|Capture)$"),p=0,F=hostResolvedPromise(),M=new RegExp("[A-Z]"),G=runtimeDomExtend({patchProp:function(e,t,s,o,r,i){r="svg"===r;if("class"==t)W(e,o,r);else if("style"==t)Z(e,s,o);else if(patchPropIsOn(t))patchPropIsModelListener(t)||$(e,t,o,i);else{t.startsWith(".")?(t=t.slice(1),s=!0):t.startsWith("^")?(t=t.slice(1),s=!1):s=ee(e,t,o,r);if(s){O(e,t,o,i),!hostString(hostRead(e,"tagName")).includes("-")&&("value"==t||"checked"==t||"selected"==t)&&S(e,t,o,r,"value"!=t);return}s=hostRead(e,"_isVueCE");var n=hostRead(hostRead(e,"_def"),"__asyncLoader");s!=null&&s&&(te(e,t)||n!=null&&n&&(M.test(t)||!patchPropIsString(o)))?O(e,hostString(hostCall1(patchPropCamelize,t)),o,i,t):("true-value"==t?hostWrite(e,"_trueValue",o):"false-value"==t&&hostWrite(e,"_falseValue",o),S(e,t,o,r,void 0))}}},o),f=void 0,re=function(){(f==null||!f)&&(f=runtimeDomCreateRenderer(G));var e=f;e=runtimeDomApply(runtimeDomRead(e,"createApp"),e,arguments);var t=runtimeDomRead(e,"mount");runtimeDomWrite(e,"mount",function(s){var o=ne(s);if(!(o==null||!o)){var r=runtimeDomRead(e,"_component");!runtimeDomIsFunction(r)&&!runtimeDomRead(r,"render")&&!runtimeDomRead(r,"template")&&runtimeDomWrite(r,"template",runtimeDomRead(o,"innerHTML")),1==(+runtimeDomRead(o,"nodeType")|0)&&runtimeDomSetTextContent(o,""),r=t(o,!1,runtimeDomResolveRootNamespace(o)),runtimeDomIsElement(o)&&(runtimeDomRemoveAttribute(o,"v-cloak"),runtimeDomSetAttribute(o,"data-v-app",""));return r}});return e};export{Fragment,computed,re as createApp,createElementBlock,createElementVNode,nextTick,openBlock,reactive,renderList,toDisplayString};export*from"./runtime-core.js";
