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
import{Fragment,computed,createBlock,createElementBlock,createElementVNode,createRenderer as runtimeDomCreateRenderer,createTextVNode,createVNode,normalizeClass,openBlock,popScopeId,pushScopeId,ref,renderList,resolveComponent,toDisplayString}from"./runtime-core.js";import{extend as runtimeDomExtend,isFunction as runtimeDomIsFunction,isString as runtimeDomIsString}from"./shared.js";import{warn as nodeOpsWarn}from"./runtime-core.js";import{camelize as patchPropCamelize,isArray as patchPropIsArray,isFunction as patchPropIsFunction,isModelListener as patchPropIsModelListener,isOn as patchPropIsOn,isString as patchPropIsString}from"./shared.js";import{BaseTransition as dependencyBaseTransition,BaseTransitionPropsValidators as dependencyBaseTransitionPropsValidators,h as dependencyH}from"./runtime-core.js";import{extend as transitionExtend,isArray as transitionIsArray,isObject as transitionIsObject,toNumber as transitionToNumber}from"./shared.js";import{camelize as styleCamelize,capitalize as styleCapitalize,hyphenate as styleHyphenate,isArray as styleIsArray,isString as styleIsString}from"./shared.js";import{includeBooleanAttr as attrIncludeBooleanAttr,isSpecialBooleanAttr as attrIsSpecialBooleanAttr,isSymbol as attrIsSymbol}from"./shared.js";import{DeprecationTypes as propDeprecationTypes,compatUtils as propCompatUtils,warn as propWarn}from"./runtime-core.js";import{includeBooleanAttr as propIncludeBooleanAttr}from"./shared.js";import{ErrorCodes as eventErrorCodes,callWithAsyncErrorHandling as eventCallWithAsyncErrorHandling}from"./runtime-core.js";import{hyphenate as eventHyphenate,isArray as eventIsArray}from"./shared.js";let ta="http://www.w3.org/1999/xlink";const ea=Fragment,fa=computed,ga=createBlock,ha=createElementBlock,ia=createElementVNode,ja=createTextVNode,ka=createVNode,la=normalizeClass,ma=openBlock,na=popScopeId,oa=pushScopeId,pa=ref,qa=renderList,ra=resolveComponent,sa=toDisplayString;let A=t=>r!=null&&r?r.createHTML(t):t,H=t=>{if(t.length<3)return!1;var e=t.charCodeAt(2);return 111==t.charCodeAt(0)&&110==t.charCodeAt(1)&&e>96&&e<123},v=t=>{t=transitionToNumber(t);return+t},q=(e,a,o)=>{if(styleIsArray(o)){for(var r=+hostRead(o,"length")|0,s=0;s<r;s++)q(e,a,hostRead(o,s));return}o==null&&(o="");s=hostString(o),r=t.test(s),a.startsWith("--")?r?hostCallMethod3(e,"setProperty",a,s.replace(t,""),"important"):hostCallMethod2(e,"setProperty",a,o):(a=((t,e)=>{var o,a=hostRead(u,e);if(a!=null&&a)return hostString(a);a=hostString(hostCall1(styleCamelize,e));if("filter"!=a&&hostHasProperty(t,a))return hostWrite(u,e,a),a;var s=hostString(hostCall1(styleCapitalize,a));for(o=0;o<y.length;o++){a=(y[o]||"")+s;if(hostHasProperty(t,a))return hostWrite(u,e,a),a}return e})(e,a),r?(a=hostString(hostCall1(styleHyphenate,a)),hostCallMethod3(e,"setProperty",a,s.replace(t,""),"important")):hostWrite(e,a,o))},p=(t,e)=>{t=hostRead(t,e),t==null&&(t="");return hostString(t).split(", ")},C=t=>"auto"==t?0:hostNumber(hostString(t.slice(0,-1).replace(",",".")))*1e3,D=(t,e)=>{while(i(t)<i(e))t=t.concat(t);for(var s,r=i(e),o=0,a=0;a<r;a++)s=C(hostString(hostRead(e,a))),o=hostMathMax(o,s+C(hostString(hostRead(t,a))));return o},j=(t,e)=>{for(var o,s=e.split(new RegExp("\\s+")),r=i(s),a=0;a<r;a++)o=hostString(hostRead(s,a)),""!=o&&hostRead(t,"classList").add(o);a=hostRead(t,m),(a==null||!a)&&(a=hostCreateSet(void 0),hostWrite(t,m,a)),hostSetAdd(a,e)},l=(t,e)=>{for(var o,s=e.split(new RegExp("\\s+")),r=i(s),a=0;a<r;a++)o=hostString(hostRead(s,a)),""!=o&&hostRead(t,"classList").remove(o);a=hostRead(t,m),a!=null&&a&&(hostSetDelete(a,e),0==hostSetSize(a)&&hostWrite(t,m,void 0))},F=(t,e,a,o,s)=>{var r=!!hostCall1(attrIsSpecialBooleanAttr,e);s===void 0||(r=!!s);if(o&&e.startsWith("xlink:")){a==null?hostRemoveAttributeNS(t,ta,e.slice(6)):hostSetAttributeNS(t,ta,e,a);return}a==null||r&&!attrIncludeBooleanAttr(a)?hostRemoveAttribute(t,e):(r?a="":attrIsSymbol(a)&&(a=hostString(a)),hostSetAttribute(t,e,a))},G=(t,e,a,o,s)=>{if("innerHTML"==e||"textContent"==e){a==null||("innerHTML"==e&&(a=A(a)),hostWrite(t,e,a));return}var h=t.tagName,r=hostString(h);if("value"==e&&"PROGRESS"!=r&&!r.includes("-")){o=t.value,"OPTION"==r&&(o=hostGetAttribute(t,"value"),(o==null||!o)&&(o="")),a==null?(s=t.type,s="checkbox"===s?"on":""):s=hostString(a),(o!==s||!hostHasProperty(t,"_value"))&&hostWrite(t,"value",s),a==null&&hostRemoveAttribute(t,e),hostWrite(t,"_value",a);return}if(""===a||a==null)o=typeof t[e],"boolean"==o?(a=propIncludeBooleanAttr(a),o=!1):a==null&&"string"==o?(a="",o=!0):"number"==o?(a=0,o=!0):o=!1;else{var i,n=!1;n&&propCompatUtils.isCompatEnabled.call(propCompatUtils,propDeprecationTypes.ATTR_FALSE_VALUE,o)?(o=typeof t[e],"string"==o||"number"==o?(a="number"==o?0:"",o=!0):o=!1):o=!1}try{hostWrite(t,e,a)}catch(t){(i=!1)&&propWarn("Failed setting prop \""+e+"\" on <"+r.toLowerCase()+">: value "+hostString(a)+" is invalid.",t)}o&&(s==null||(e=hostString(s)),hostRemoveAttribute(t,e))},E=(t,e,a,o)=>{s++;var c=s;hostWrite(t,"_endId",c);var b=function(){(+hostRead(t,"_endId")|0)==c&&o()};if(a!=null){hostSetTimeout(b,+a);return}e=((t,e)=>{var a=hostGetComputedStyle(t);t=p(a,"transitionDelay");var h=p(a,"transitionDuration"),o=D(t,h);t=p(a,"animationDelay");var n=p(a,"animationDuration"),s=D(t,n);t=null;if("transition"===e){o>0?(e=i(h),t="transition"):(o=0,e=0);var r=o}else{"animation"===e?(s>0?(e=i(n),t="animation"):(s=0,e=0),r=s):(r=hostMathMax(o,s),r>0?o>s?(e=i(h),t="transition"):(e=i(n),t="animation"):e=0)}"transition"===t?(o=I,a=o.test(hostString(p(a,"transitionProperty").toString()))):a=!1;return{type:t,timeout:r,propCount:e,hasTransform:a}})(t,e);a=hostRead(e,"type");var h,n,l;if(a==null||!a){o();return}a=hostString(a)+"end";var r=0;h=+hostRead(e,"propCount")|0;var d;n=function(){hostRemoveEventListener(t,a,d,void 0),b()},d=function(e){hostRead(e,"target")===t&&(r=r+1|0,r>=h&&n())},l=function(){r<h&&n()},hostSetTimeout(l,+hostRead(e,"timeout")+1),hostAddEventListener(t,a,d,void 0)},_=(t,e)=>{var a=function(t){var r,s,h,i,o=hostRead(t,"_vts");if(o==null||!o)hostWrite(t,"_vts",hostDateNow());else{o=+o;if(o<=+hostRead(a,"attached"))return}s=hostRead(a,"value");if(eventIsArray(s))for(h=hostRead(t,"stopImmediatePropagation"),hostWrite(t,"stopImmediatePropagation",function(){hostCall0(h.bind(t)),hostWrite(t,"_stopped",!0)}),r=hostArraySlice(s),i=+hostRead(r,"length")|0,o=0;o<i;o++){if(hostRead(t,"_stopped"))break;s=hostRead(r,o),s!=null&&s&&eventCallWithAsyncErrorHandling(s,e,eventErrorCodes.NATIVE_EVENT_HANDLER,[t])}else eventCallWithAsyncErrorHandling(s,e,eventErrorCodes.NATIVE_EVENT_HANDLER,[t])};hostWrite(a,"value",t),hostWrite(a,"attached",(()=>{if(0!=n)return n;n=hostDateNow(),hostPromiseThen(O,function(){n=0});return n})());return a},B=t=>{if(t==null||!t)return!1;if(transitionIsArray(t)){for(var a=i(t),e=0;e<a;e++)if((+hostRead(hostRead(t,e),"length")|0)>1)return!0;return!1}return(+hostRead(t,"length")|0)>1},k=(t,e)=>{e===void 0&&(e=[]);if(transitionIsArray(t))for(var o=i(t),a=0;a<o;a++)hostRead(t,a).apply(void 0,e);else t!=null&&t&&t.apply(void 0,e)},i=t=>+hostRead(t,"length")|0;var r,ua=hostTrustedTypes(),va;if(ua!=null&&ua)try{r=hostCreateTrustedTypesPolicy(ua,"vue",function(t){return t})}catch(t){ua=nodeOpsWarn,ua("Error creating trusted types policy: "+hostString(t))}var Aa={insert:function(t,e,a){hostInsert(t,e,a)},remove:function(t){hostRemove(t)},createElement:(t,e,a,o)=>{t=hostString(t);var s=hostCreateElement(t,e,a);"select"==t&&o!=null&&o.multiple!=null&&hostSetAttribute(s,"multiple",o.multiple);return s},createText:function(t){return hostCreateText(hostString(t))},createComment:function(t){return hostCreateComment(hostString(t))},setText:function(t,e){hostSetNodeValue(t,hostString(e))},setElementText:function(t,e){hostSetTextContent(t,hostString(e))},parentNode:function(t){return hostParentNode(t)},nextSibling:function(t){return hostNextSibling(t)},querySelector:function(t){return hostQuerySelector(hostString(t))},setScopeId:function(t,e){hostSetAttribute(t,hostString(e),"")},insertStaticContent:(t,e,a,o,s,r)=>{var i=hostLastChild(e);a==null||(i=hostPreviousSibling(a));if(s!=null&&(s===r||hostNextSibling(s)!=null))while(!0){hostInsert(hostCloneNode(s),e,a);if(s===r)break;s=hostNextSibling(s);if(s==null)break}else{o=o!=null?hostString(o):"",t=hostString(t),"svg"==o?t="<svg>"+t+"</svg>":"mathml"==o&&(t="<math>"+t+"</math>");var h=A(t);hostSetTemplateHTML(h),h=hostTemplateContent();if("svg"==o||"mathml"==o){t=hostFirstChild(h);while(hostFirstChild(t)!=null)hostAppendChild(h,hostFirstChild(t));hostRemoveChild(h,t)}hostInsert(h,e,a)}t=hostFirstChild(e);i==null||(t=hostNextSibling(i)),e=hostLastChild(e),a==null||(e=hostPreviousSibling(a));return[t,e]}},m=hostCreateSymbol("_vtc");ua=hostConstructors(),va=ua.String;var wa=ua.String,xa=ua.Boolean;xa={type:xa,default:!0};var ya=ua.String,za=ua.Number;ya=[ya,za,ua.Object],za=ua.String;var Ba=ua.String,Ca=ua.String,Da=ua.String,Ea=ua.String,Fa=ua.String,Ga=ua.String,Ha=ua.String,w={name:va,type:wa,css:xa,duration:ya,enterFromClass:za,enterActiveClass:Ba,enterToClass:Ca,appearFromClass:Da,appearActiveClass:Ea,appearToClass:Fa,leaveFromClass:Ga,leaveActiveClass:Ha,leaveToClass:ua.String};wa=transitionExtend({},dependencyBaseTransitionPropsValidators,w);var s=0,I=new RegExp("\\b(?:transform|all)(?:,|$)");va=function(t,e){return dependencyH(dependencyBaseTransition,(t=>{var a={};for(var e in t)hostHasProperty(w,e)||hostWrite(a,e,hostRead(t,e));if(!1===hostRead(t,"css"))return a;e=hostRead(t,"name")!==void 0?hostString(hostRead(t,"name")):"v";var u=hostRead(t,"type"),o=hostRead(t,"duration"),s=e+"-enter-from",r=e+"-enter-active",h=e+"-enter-to",d=s,c=r,b=h,i=e+"-leave-from",n=e+"-leave-active",m=e+"-leave-to";hostRead(t,"enterFromClass")===void 0||(s=hostString(hostRead(t,"enterFromClass"))),hostRead(t,"enterActiveClass")===void 0||(r=hostString(hostRead(t,"enterActiveClass"))),hostRead(t,"enterToClass")===void 0||(h=hostString(hostRead(t,"enterToClass"))),hostRead(t,"appearFromClass")===void 0||(d=hostString(hostRead(t,"appearFromClass"))),hostRead(t,"appearActiveClass")===void 0||(c=hostString(hostRead(t,"appearActiveClass"))),hostRead(t,"appearToClass")===void 0||(b=hostString(hostRead(t,"appearToClass"))),hostRead(t,"leaveFromClass")===void 0||(i=hostString(hostRead(t,"leaveFromClass"))),hostRead(t,"leaveActiveClass")===void 0||(n=hostString(hostRead(t,"leaveActiveClass"))),hostRead(t,"leaveToClass")===void 0||(m=hostString(hostRead(t,"leaveToClass")));var p="";new RegExp("-from$"),t=(t=>{if(t==null)return null;if(transitionIsObject(t))return[v(hostRead(t,"enter")),v(hostRead(t,"leave"))];t=v(t);return[t,t]})(o),p=e=null;var f=e;t==null||(p=hostRead(t,0),f=hostRead(t,1)),o=hostRead(a,"onBeforeEnter"),t=hostRead(a,"onEnter");var C=hostRead(a,"onEnterCancelled"),y=hostRead(a,"onLeave"),I=hostRead(a,"onLeaveCancelled"),g=hostRead(a,"onBeforeAppear");g===void 0&&(g=o);var R=hostRead(a,"onAppear");R===void 0&&(R=t);var S=hostRead(a,"onAppearCancelled");S===void 0&&(S=C),e=function(){var t=hostRead(arguments,0),s=!!hostRead(arguments,1),e=hostRead(arguments,2);hostWrite(t,"_enterCancelled",hostRead(arguments,3));var a=h,o=r;s&&(a=b,o=c),l(t,a),l(t,o),e!=null&&e&&e()};var A=function(t,e){hostWrite(t,"_isLeaving",!1),l(t,i);var a=m;l(t,a),l(t,n),e!=null&&e&&e()},z=((t)=>function(a){let o=!!a;return function(a,r){var i=t;o&&(i=R);var n=function(){e(a,o,r)};k(i,[a,n]);var c=function(){var t=s,e=h;o&&(t=d,e=b),l(a,t),j(a,e),B(i)||E(a,u,p,n)};hostRequestAnimationFrame(function(){hostRequestAnimationFrame(c)})}})(t);t={},hostWrite(t,"onBeforeEnter",function(t){k(o,[t]),j(t,s),j(t,r)}),hostWrite(t,"onBeforeAppear",function(t){k(g,[t]),j(t,d),j(t,c)}),hostWrite(t,"onEnter",hostCall1(z,!1)),hostWrite(t,"onAppear",hostCall1(z,!0)),hostWrite(t,"onLeave",function(t,e){hostWrite(t,"_isLeaving",!0);var a=function(){A(t,e)};j(t,i),!hostRead(t,"_enterCancelled")?(hostBodyOffsetHeight(t),j(t,n)):(j(t,n),hostBodyOffsetHeight(t));var o=function(){if(!(!hostRead(t,"_isLeaving")))l(t,i),j(t,m),B(y)||E(t,u,f,a)};hostRequestAnimationFrame(function(){hostRequestAnimationFrame(o)}),k(y,[t,a])}),hostWrite(t,"onEnterCancelled",function(t){e(t,!1,void 0,!0),k(C,[t])}),hostWrite(t,"onAppearCancelled",function(t){e(t,!0,void 0,!0),k(S,[t])}),hostWrite(t,"onLeaveCancelled",function(t){A(t,void 0),k(I,[t])});return transitionExtend(a,t)})(t),hostRead(e,"slots"))},hostWrite(va,"displayName","Transition"),hostWrite(va,"props",wa);var x=hostCreateSymbol("_vod"),J=hostCreateSymbol("_vsh"),K=hostCreateSymbol("CSS_VAR_TEXT"),L=new RegExp("(?:^|;)\\s*display\\s*:");new RegExp("[^\\\\];\\s*$");var t=new RegExp("\\s*!important$"),y=["Webkit","Moz","ms"],u={},z=hostCreateSymbol("_vei"),M=new RegExp("(Once|Passive|Capture)$"),N=new RegExp("^on:?(?:Once|Passive|Capture)$"),n=0,O=hostResolvedPromise(),P=new RegExp("[A-Z]"),Q=runtimeDomExtend({patchProp:(t,e,a,o,s,r)=>{s="svg"===s;if("class"==e)((t,e,a)=>{var o=hostRead(t,m);o!=null&&o&&(o=hostArrayFrom(o),e!=null&&""!=hostString(e)&&o.unshift(e),e=o.join(" ")),e==null?hostRemoveAttribute(t,"class"):a?hostSetAttribute(t,"class",e):hostWrite(t,"className",e)})(t,o,s);else if("style"==e)((t,e,a)=>{var h=hostRead(t,"style"),s=styleIsString(a);if(a!=null&&a&&!s){if(e!=null&&e&&!styleIsString(e))for(var o in e)hostRead(a,o)==null&&q(h,o,"");if(e!=null&&e&&styleIsString(e))for(s=hostString(e).split(";"),o=0;o<s.length;o++){var r=s[o]||"";r=r.slice(0,r.indexOf(":")).trim(),hostRead(a,r)==null&&q(h,r,"")}s=!1;for(o in a){"display"==o&&(s=!0);r=hostRead(a,o);if(r!=null){var i=void 0;!styleIsString(e)&&e!=null&&e&&(i=hostRead(e,o)),("TEXTAREA"===hostRead(t,"tagName")&&("width"==o||"height"==o)&&styleIsString(r)&&i===r)||q(h,o,r)}else q(h,o,"")}}else{s?e!==a?(o=hostString(a),e=hostRead(h,K),e!=null&&e&&(o=o+(";"+hostString(e))),hostWrite(h,"cssText",o),s=L.test(o)):s=!1:(e!=null&&e&&hostRemoveAttribute(t,"style"),s=!1)}hostHasProperty(t,x)&&(e=s?hostRead(h,"display"):"",hostWrite(t,x,e),!hostRead(t,J)||hostWrite(h,"display","none"))})(t,a,o);else if(patchPropIsOn(e))patchPropIsModelListener(e)||((t,e,a,o)=>{var s=hostRead(t,z);s==null&&(s={},hostWrite(t,z,s));var r=hostRead(s,e);if(a!=null&&a&&r!=null&&r){hostWrite(r,"value",a);return}var h=(t=>{var e,a=void 0;while(!0){e=M.exec(t);if(e==null||N.test(t))break;a===void 0&&(a={}),e=hostString(hostRead(e,1)),hostWrite(a,e.toLowerCase(),!0),t=t.slice(0,t.length-e.length)}e=hostString(hostCall1(eventHyphenate,t.slice(2)));58==t.charCodeAt(2)&&(e=t.slice(3));return[e,a]})(e),n=hostRead(h,0),i=hostString(n);h=hostRead(h,1),a!=null&&a?(a=_(a,o),hostWrite(s,e,a),hostAddEventListener(t,i,a,h)):r!=null&&r&&(hostRemoveEventListener(t,i,r,h),hostWrite(s,e,void 0))})(t,e,o,r);else{e.startsWith(".")?(e=e.slice(1),a=!0):e.startsWith("^")?(e=e.slice(1),a=!1):a=((t,e,a,o)=>{if(o)return"innerHTML"==e||"textContent"==e?!0:hostHasProperty(t,e)&&H(e)&&patchPropIsFunction(a)?!0:!1;if("spellcheck"==e||"draggable"==e||"translate"==e||"autocorrect"==e)return!1;var r=hostRead(t,"tagName"),s=hostString(r);return"sandbox"==e&&"IFRAME"==s?!1:"form"==e?!1:"list"==e&&"INPUT"==s?!1:"type"==e&&"TEXTAREA"==s?!1:("width"==e||"height"==e)&&("IMG"==s||"VIDEO"==s||"CANVAS"==s||"SOURCE"==s)?!1:H(e)&&patchPropIsString(a)?!1:hostHasProperty(t,e)})(t,e,o,s);if(a){G(t,e,o,r),!hostString(hostRead(t,"tagName")).includes("-")&&("value"==e||"checked"==e||"selected"==e)&&F(t,e,o,s,"value"!=e);return}a=hostRead(t,"_isVueCE");var h=hostRead(hostRead(t,"_def"),"__asyncLoader");a!=null&&a&&(((t,e)=>{t=hostRead(hostRead(t,"_def"),"props");var o,a;if(t==null||!t)return!1;o=hostString(hostCall1(patchPropCamelize,e));if(patchPropIsArray(t)){for(a=+hostRead(t,"length")|0,e=0;e<a;e++)if(hostString(hostCall1(patchPropCamelize,hostString(hostRead(t,e))))==o)return!0;return!1}for(e=hostObjectKeys(t),a=+hostRead(e,"length")|0,t=0;t<a;t++)if(hostString(hostCall1(patchPropCamelize,hostString(hostRead(e,t))))==o)return!0;return!1})(t,e)||h!=null&&h&&(P.test(e)||!patchPropIsString(o)))?G(t,hostString(hostCall1(patchPropCamelize,e)),o,r,e):("true-value"==e?hostWrite(t,"_trueValue",o):"false-value"==e&&hostWrite(t,"_falseValue",o),F(t,e,o,s,void 0))}}},Aa),o=void 0,da=function(){(o==null||!o)&&(o=runtimeDomCreateRenderer(Q));var t=o;t=runtimeDomApply(runtimeDomRead(t,"createApp"),t,arguments);var e=runtimeDomRead(t,"mount");runtimeDomWrite(t,"mount",function(a){var o=(t=>runtimeDomIsString(t)?(t=runtimeDomQuerySelector(runtimeDomString(t)),t):t)(a);if(!(o==null||!o)){var s=runtimeDomRead(t,"_component");!runtimeDomIsFunction(s)&&!runtimeDomRead(s,"render")&&!runtimeDomRead(s,"template")&&runtimeDomWrite(s,"template",runtimeDomRead(o,"innerHTML")),1==(+runtimeDomRead(o,"nodeType")|0)&&runtimeDomSetTextContent(o,""),s=e(o,!1,runtimeDomResolveRootNamespace(o)),runtimeDomIsElement(o)&&(runtimeDomRemoveAttribute(o,"v-cloak"),runtimeDomSetAttribute(o,"data-v-app",""));return s}});return t};export{Fragment,computed,da as createApp,createBlock,createElementBlock,createElementVNode,createTextVNode,createVNode,normalizeClass,openBlock,popScopeId,pushScopeId,ref,renderList,resolveComponent,toDisplayString};export*from"./runtime-core.js";
