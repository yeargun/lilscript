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
import{BaseTransition,BaseTransitionPropsValidators,Comment,DeprecationTypes,EffectScope,ErrorCodes,ErrorTypeStrings,Fragment,KeepAlive,ReactiveEffect,Static,Suspense,Teleport,Text,TrackOpTypes,TriggerOpTypes,assertNumber,callWithAsyncErrorHandling,callWithErrorHandling,camelize,capitalize,cloneVNode,compatUtils,computed,createBlock,createCommentVNode,createElementBlock,createElementVNode,createHydrationRenderer as runtimeDomCreateHydrationRenderer,createPropsRestProxy,createRenderer as runtimeDomCreateRenderer,createSlots,createStaticVNode,createTextVNode,createVNode,customRef,defineAsyncComponent,defineComponent,defineEmits,defineExpose,defineModel,defineOptions,defineProps,defineSlots,devtools,effect,effectScope,getCurrentInstance,getCurrentScope,getCurrentWatcher,getTransitionRawChildren,guardReactiveProps,h,handleError,hasInjectionContext,hydrateOnIdle,hydrateOnInteraction,hydrateOnMediaQuery,hydrateOnVisible,initCustomFormatter,inject,isMemoSame,isProxy,isReactive,isReadonly,isRef,isRuntimeOnly as runtimeDomIsRuntimeOnly,isShallow,isVNode,markRaw,mergeDefaults,mergeModels,mergeProps,nextTick,normalizeClass,normalizeProps,normalizeStyle,onActivated,onBeforeMount,onBeforeUnmount,onBeforeUpdate,onDeactivated,onErrorCaptured,onMounted,onRenderTracked,onRenderTriggered,onScopeDispose,onServerPrefetch,onUnmounted,onUpdated,onWatcherCleanup,openBlock,popScopeId,provide,proxyRefs,pushScopeId,queuePostFlushCb,reactive,readonly,ref,registerRuntimeCompiler,renderList,renderSlot,resolveComponent,resolveDirective,resolveDynamicComponent,resolveFilter,resolveTransitionHooks,setBlockTracking,setDevtoolsHook,setTransitionHooks,shallowReactive,shallowReadonly,shallowRef,ssrContextKey,ssrUtils,stop,toDisplayString,toHandlerKey,toHandlers,toRaw,toRef,toRefs,toValue,transformVNodeArgs,triggerRef,unref,useAttrs,useId,useModel,useSSRContext,useSlots,useTemplateRef,useTransitionState,version,warn as runtimeDomWarn,watch,watchEffect,watchPostEffect,watchSyncEffect,withAsyncContext,withCtx,withDefaults,withDirectives,withMemo,withScopeId}from"./runtime-core.js";import{extend as runtimeDomExtend,isFunction as runtimeDomIsFunction,isString as runtimeDomIsString}from"./shared.js";import{warn as nodeOpsWarn}from"./runtime-core.js";import{camelize as patchPropCamelize,isArray as patchPropIsArray,isFunction as patchPropIsFunction,isModelListener as patchPropIsModelListener,isOn as patchPropIsOn,isString as patchPropIsString}from"./shared.js";import{BaseTransition as dependencyBaseTransition,BaseTransitionPropsValidators as dependencyBaseTransitionPropsValidators,h as dependencyH}from"./runtime-core.js";import{extend as transitionExtend,isArray as transitionIsArray,isObject as transitionIsObject,toNumber as transitionToNumber}from"./shared.js";import{camelize as styleCamelize,capitalize as styleCapitalize,hyphenate as styleHyphenate,isArray as styleIsArray,isString as styleIsString}from"./shared.js";import{Fragment as cssVarsFragment,Static as cssVarsStatic,getCurrentInstance as cssVarsGetCurrentInstance,onBeforeUpdate as cssVarsOnBeforeUpdate,onMounted as cssVarsOnMounted,onUnmounted as cssVarsOnUnmounted,queuePostFlushCb as cssVarsQueuePostFlushCb,watch as cssVarsWatch}from"./runtime-core.js";import{NOOP as cssVarsNoop,ShapeFlags as cssVarsShapeFlags,normalizeCssVarValue as cssVarsNormalizeCssVarValue}from"./shared.js";import{includeBooleanAttr as attrIncludeBooleanAttr,isSpecialBooleanAttr as attrIsSpecialBooleanAttr,isSymbol as attrIsSymbol}from"./shared.js";import{DeprecationTypes as propDeprecationTypes,compatUtils as propCompatUtils,warn as propWarn}from"./runtime-core.js";import{includeBooleanAttr as propIncludeBooleanAttr}from"./shared.js";import{ErrorCodes as eventErrorCodes,callWithAsyncErrorHandling as eventCallWithAsyncErrorHandling}from"./runtime-core.js";import{hyphenate as eventHyphenate,isArray as eventIsArray}from"./shared.js";import{createVNode as customElementCreateVNode,defineComponent as customElementDefineComponent,getCurrentInstance as customElementGetCurrentInstance,nextTick as customElementNextTick,unref as customElementUnref}from"./runtime-core.js";import{camelize as customElementCamelize,extend as customElementExtend,hasOwn as customElementHasOwn,hyphenate as customElementHyphenate,isArray as customElementIsArray,isPlainObject as customElementIsPlainObject,toNumber as customElementToNumber}from"./shared.js";import{getCurrentInstance as cssModuleGetCurrentInstance}from"./runtime-core.js";import{EMPTY_OBJ as dependencyEmptyObject}from"./shared.js";import{Fragment as transitionGroupFragment,createVNode as transitionGroupCreateVNode,getCurrentInstance as transitionGroupGetCurrentInstance,getTransitionRawChildren as transitionGroupGetTransitionRawChildren,onUpdated as transitionGroupOnUpdated,resolveTransitionHooks as transitionGroupResolveTransitionHooks,setTransitionHooks as transitionGroupSetTransitionHooks,toRaw as transitionGroupToRaw,useTransitionState as transitionGroupUseTransitionState}from"./runtime-core.js";import{extend as transitionGroupExtend}from"./shared.js";import{nextTick as modelNextTick,warn as modelWarn}from"./runtime-core.js";import{isArray as modelIsArray,isSet as modelIsSet,invokeArrayFns as modelInvokeArrayFns,looseEqual as modelLooseEqual,looseIndexOf as modelLooseIndexOf,looseToNumber as modelLooseToNumber}from"./shared.js";import{hyphenate as vOnHyphenate}from"./shared.js";let ub=e=>function(t,o,a){return e(this,t,o,a)},le="",me="http://www.w3.org/1999/xlink";const Bb=BaseTransition;const Cb=BaseTransitionPropsValidators;const Db=Comment;const Eb=DeprecationTypes;const Fb=EffectScope;const Gb=ErrorCodes;const Hb=ErrorTypeStrings;const Ib=Fragment;const Jb=KeepAlive;const Kb=ReactiveEffect;const Lb=Static;const Mb=Suspense;const Nb=Teleport;const Ob=Text;const Pb=TrackOpTypes;const Qb=TriggerOpTypes;const Rb=assertNumber;const Sb=callWithAsyncErrorHandling;const Tb=callWithErrorHandling;const Ub=camelize;const Vb=capitalize;const Wb=cloneVNode;const Xb=compatUtils;const Yb=computed;const Zb=createBlock;const _b=createCommentVNode;const $b=createElementBlock;const ac=createElementVNode;const bc=runtimeDomCreateHydrationRenderer;const cc=createPropsRestProxy;const dc=runtimeDomCreateRenderer;const ec=createSlots;const fc=createStaticVNode;const gc=createTextVNode;const hc=createVNode;const ic=customRef;const jc=defineAsyncComponent;const kc=defineComponent;const lc=defineEmits;const mc=defineExpose;const nc=defineModel;const oc=defineOptions;const pc=defineProps;const qc=defineSlots;const rc=devtools;const sc=effect;const tc=effectScope;const uc=getCurrentInstance;const vc=getCurrentScope;const wc=getCurrentWatcher;const xc=getTransitionRawChildren;const yc=guardReactiveProps;const zc=h;const Ac=handleError;const Bc=hasInjectionContext;const Cc=hydrateOnIdle;const Dc=hydrateOnInteraction;const Ec=hydrateOnMediaQuery;const Fc=hydrateOnVisible;const Gc=initCustomFormatter;const Hc=inject;const Ic=isMemoSame;const Jc=isProxy;const Kc=isReactive;const Lc=isReadonly;const Mc=isRef;const Nc=runtimeDomIsRuntimeOnly;const Oc=isShallow;const Pc=isVNode;const Qc=markRaw;const Rc=mergeDefaults;const Sc=mergeModels;const Tc=mergeProps;const Uc=nextTick;const Vc=normalizeClass;const Wc=normalizeProps;const Xc=normalizeStyle;const Yc=onActivated;const Zc=onBeforeMount;const _c=onBeforeUnmount;const $c=onBeforeUpdate;const ad=onDeactivated;const bd=onErrorCaptured;const cd=onMounted;const dd=onRenderTracked;const ed=onRenderTriggered;const fd=onScopeDispose;const gd=onServerPrefetch;const hd=onUnmounted;const id=onUpdated;const jd=onWatcherCleanup;const kd=openBlock;const ld=popScopeId;const md=provide;const nd=proxyRefs;const od=pushScopeId;const pd=queuePostFlushCb;const qd=reactive;const rd=readonly;const sd=ref;const td=registerRuntimeCompiler;const ud=renderList;const vd=renderSlot;const wd=resolveComponent;const xd=resolveDirective;const yd=resolveDynamicComponent;const zd=resolveFilter;const Ad=resolveTransitionHooks;const Bd=setBlockTracking;const Cd=setDevtoolsHook;const Dd=setTransitionHooks;const Ed=shallowReactive;const Fd=shallowReadonly;const Gd=shallowRef;const Hd=ssrContextKey;const Id=ssrUtils;const Jd=stop;const Kd=toDisplayString;const Ld=toHandlerKey;const Md=toHandlers;const Nd=toRaw;const Od=toRef;const Pd=toRefs;const Qd=toValue;const Rd=transformVNodeArgs;const Sd=triggerRef;const Td=unref;const Ud=useAttrs;const Vd=useId;const Wd=useModel;const Xd=useSSRContext;const Yd=useSlots;const Zd=useTemplateRef;const _d=useTransitionState;const $d=version;const ae=runtimeDomWarn;const be=watch;const ce=watchEffect;const de=watchPostEffect;const ee=watchSyncEffect;const fe=withAsyncContext;const ge=withCtx;const he=withDefaults;const ie=withDirectives;const je=withMemo;const ke=withScopeId;function la(e){let t;return !(B==null)&&!!B?B.createHTML(e):e}var x,ma,t,u,na,oa;(function(){function a(e){return +hostRead(e,"length")|0}function b(e){if("auto"==e)return 0;return hostNumber(hostString(e.slice(0,-1).replace(",",".")))*1e3}function c(e,t){e=hostRead(e,t);e==null&&(e=le);return hostString(e).split(", ")}x=function(e,t){t===void 0&&(t=[]);if(transitionIsArray(e)){var s=a(e);var o=0;for(;o<s;o=o+1)hostRead(e,o).apply(void 0,t)}else{!(e==null)&&e&&e.apply(void 0,t)}};ma=function(e){if(e==null||!e)return !1;if(transitionIsArray(e)){var o=a(e);var t=0;for(;t<o;t=t+1){if((+hostRead(hostRead(e,t),"length")|0)>1)return !0}return !1}return (+hostRead(e,"length")|0)>1};t=function(e,t){var d=t.split(new RegExp("\\s+"));var r=a(d);var o=0,s;for(;o<r;o=o+1){s=hostString(hostRead(d,o));s!=le&&hostRead(e,"classList").add(s)}o=hostRead(e,y);(o==null||!o)&&(o=hostCreateSet(void 0),hostWrite(e,y,o));hostSetAdd(o,t)};u=function(e,t){var d=t.split(new RegExp("\\s+"));var r=a(d);var o=0,s;for(;o<r;o=o+1){s=hostString(hostRead(d,o));s!=le&&hostRead(e,"classList").remove(s)}o=hostRead(e,y);!(o==null)&&!!o&&(hostSetDelete(o,t),0==hostSetSize(o)&&hostWrite(e,y,void 0))};na=function(e,t){for(;a(e)<a(t);)e=e.concat(e);var r=a(t);var s=0,o=0,d;for(;o<r;o=o+1){d=b(hostString(hostRead(t,o)));s=hostMathMax(s,d+b(hostString(hostRead(e,o))))}return s};oa=function(e,t){var o=hostGetComputedStyle(e);e=c(o,"transitionDelay");var h=c(o,"transitionDuration");var s=na(e,h);e=c(o,"animationDelay");var i=c(o,"animationDuration");var d=na(e,i);e=null;if("transition"===t){s>0?(t=a(h),e="transition"):(s=0,t=0);var r=s}else{"animation"===t?(d>0?(t=a(i),e="animation"):(d=0,t=0),r=d):(r=hostMathMax(s,d),r>0?s>d?(t=a(h),e="transition"):(t=a(i),e="animation"):t=0)}"transition"===e?(s=Ha,o=s.test(hostString(c(o,"transitionProperty").toString()))):o=!1;return {type:e,timeout:r,propCount:t,hasTransform:o}};})();var pa=(function(){function b(e){e=transitionToNumber(e);return +e}function d(e){if(e==null)return null;if(transitionIsObject(e)){var t=b(hostRead(e,"enter"));return [t,b(hostRead(e,"leave"))]}e=b(e);return [e,e]}function e(e,t,o,a){I=I+1|0;var i=I;hostWrite(e,"_endId",i);var n=function(){(+hostRead(e,"_endId")|0)==i&&a()};if(!(o==null)){hostSetTimeout(n,+o);return}t=oa(e,t);o=hostRead(t,"type");var d,r,c;if(o==null||!o){a();return}o=hostString(o)+"end";var s=0;d=+hostRead(t,"propCount")|0;var h;r=function(){hostRemoveEventListener(e,o,h,void 0);n()};h=function(t){hostRead(t,"target")===e&&(s=s+1|0,s>=d&&r())};c=function(){s<d&&r()};hostSetTimeout(c,+hostRead(t,"timeout")+1);hostAddEventListener(e,o,h,void 0)}return function(o){var s={};for(var a in o)hostHasProperty(Z,a)||hostWrite(s,a,hostRead(o,a));if(!1===hostRead(o,"css"))return s;a=!(hostRead(o,"name")===void 0)?hostString(hostRead(o,"name")):"v";var C=hostRead(o,"type");var r=hostRead(o,"duration");var h=a+"-enter-from";var i=a+"-enter-active";var n=a+"-enter-to";var b=h;var R=i;var m=n;var c=a+"-leave-from";var l=a+"-leave-active";var g=a+"-leave-to";hostRead(o,"enterFromClass")===void 0||(h=hostString(hostRead(o,"enterFromClass")));hostRead(o,"enterActiveClass")===void 0||(i=hostString(hostRead(o,"enterActiveClass")));hostRead(o,"enterToClass")===void 0||(n=hostString(hostRead(o,"enterToClass")));hostRead(o,"appearFromClass")===void 0||(b=hostString(hostRead(o,"appearFromClass")));hostRead(o,"appearActiveClass")===void 0||(R=hostString(hostRead(o,"appearActiveClass")));hostRead(o,"appearToClass")===void 0||(m=hostString(hostRead(o,"appearToClass")));hostRead(o,"leaveFromClass")===void 0||(c=hostString(hostRead(o,"leaveFromClass")));hostRead(o,"leaveActiveClass")===void 0||(l=hostString(hostRead(o,"leaveActiveClass")));hostRead(o,"leaveToClass")===void 0||(g=hostString(hostRead(o,"leaveToClass")));var T=le;var I=le;var D=le;new RegExp("-from$");o=d(r);a=null;var y=a;var k=a;o==null||(y=hostRead(o,0),k=hostRead(o,1));r=hostRead(s,"onBeforeEnter");o=hostRead(s,"onEnter");var W=hostRead(s,"onEnterCancelled");var E=hostRead(s,"onLeave");var v=hostRead(s,"onLeaveCancelled");var p=hostRead(s,"onBeforeAppear");p===void 0&&(p=r);var f=hostRead(s,"onAppear");f===void 0&&(f=o);var S=hostRead(s,"onAppearCancelled");S===void 0&&(S=W);a=function(){var e=hostRead(arguments,0);var s=!!hostRead(arguments,1);var t=hostRead(arguments,2);hostWrite(e,"_enterCancelled",hostRead(arguments,3));var o=n;var a=i;s&&(o=m,a=R);u(e,o);u(e,a);!(t==null)&&t&&t()};var j=function(e,t){hostWrite(e,"_isLeaving",!1);var o=c;u(e,o);var a=g;u(e,a);var s=l;u(e,s);!(t==null)&&t&&t()};var A=((o)=>function(s){let d=!!s;return function(s,r){var i=o;d&&(i=f);var c=function(){a(s,d,r)};x(i,[s,c]);var l=function(){var o=h;var a=n;d&&(o=b,a=m);u(s,o);t(s,a);ma(i)||e(s,C,y,c)};hostRequestAnimationFrame(function(){hostRequestAnimationFrame(l)})}})(o);o={};hostWrite(o,"onBeforeEnter",function(e){x(r,[e]);t(e,h);t(e,i)});hostWrite(o,"onBeforeAppear",function(e){x(p,[e]);t(e,b);t(e,R)});hostWrite(o,"onEnter",hostCall1(A,!1));hostWrite(o,"onAppear",hostCall1(A,!0));hostWrite(o,"onLeave",function(o,a){hostWrite(o,"_isLeaving",!0);var s=function(){j(o,a)};t(o,c);!hostRead(o,"_enterCancelled")?(hostBodyOffsetHeight(o),t(o,l)):(t(o,l),hostBodyOffsetHeight(o));var d=function(){if(!hostRead(o,"_isLeaving"))return;u(o,c);t(o,g);ma(E)||e(o,C,k,s)};hostRequestAnimationFrame(function(){hostRequestAnimationFrame(d)});x(E,[o,s])});hostWrite(o,"onEnterCancelled",function(e){a(e,!1,void 0,!0);x(W,[e])});hostWrite(o,"onAppearCancelled",function(e){a(e,!0,void 0,!0);x(S,[e])});hostWrite(o,"onLeaveCancelled",function(e){j(e,void 0);x(v,[e])});return transitionExtend(s,o)};})();function Wa(e,t,o){var a=hostRead(e,y);var s;!(a==null)&&!!a&&(a=hostArrayFrom(a),!(t==null)&&hostString(t)!=le&&a.unshift(t),t=a.join(" "));t==null?hostRemoveAttribute(e,"class"):o?hostSetAttribute(e,"class",t):hostWrite(e,"className",t)}function A(e,t){var o=t?hostRead(e,C):"none";hostWrite(hostRead(e,"style"),"display",o);hostWrite(e,J,!t)}function E(e,t){if(1!=(+hostRead(e,"nodeType")|0))return;var s=hostRead(e,"style");var o=le,a;for(e in t){a=cssVarsNormalizeCssVarValue(hostRead(t,e));e="--"+e;s.setProperty(e,a);o=o+(e+": "+a+";")}hostWrite(s,_,o)}function P(e,t){var o=+hostRead(e,"shapeFlag");o=0!=(o&+hostRead(cssVarsShapeFlags,"SUSPENSE"));if(o){var a=hostRead(e,"suspense");o=hostRead(a,"activeBranch");e=hostRead(a,"pendingBranch");(!(e==null)&&!!e)&&!hostRead(a,"isHydrating")&&hostRead(a,"effects").push(function(){P(hostRead(a,"activeBranch"),t)});e=o}for(;;){o=!(hostRead(e,"component")==null)&&!!hostRead(e,"component");if(!o){break}e=hostRead(hostRead(e,"component"),"subTree")}var s=+hostRead(e,"shapeFlag");o=hostRead(e,"el");if((0!=(s&+hostRead(cssVarsShapeFlags,"ELEMENT"))&&!(o==null))&&o){E(o,t)}else{if(hostRead(e,"type")===cssVarsFragment){o=hostRead(e,"children");s=+hostRead(o,"length")|0;e=0;for(;e<s;e=e+1)P(hostRead(o,e),t)}else{if(hostRead(e,"type")===cssVarsStatic){s=hostRead(e,"anchor");while(!(o==null)&&o){E(o,t);if(o===s)break;o=hostNextSibling(o)}}}}}function vb(e){var t=cssVarsGetCurrentInstance();var o;if(t==null||!t)return;var a;a=function(o){o===void 0&&(o=hostCall1(e,hostRead(t,"proxy")));var s=hostArrayFrom(hostQuerySelectorAll("[data-v-owner=\""+hostString(hostRead(t,"uid"))+"\"]"));var d=+hostRead(s,"length")|0;var a=0;for(;a<d;a=a+1)E(hostRead(s,a),o)};hostWrite(t,"ut",a);o=function(){var o=hostCall1(e,hostRead(t,"proxy"));var s=hostRead(t,"ce");var d;!(s==null)&&!!s?E(s,o):P(hostRead(t,"subTree"),o);hostCall1(a,o)};cssVarsOnBeforeUpdate(function(){cssVarsQueuePostFlushCb(o)});cssVarsOnMounted(function(){cssVarsWatch(o,cssVarsNoop,{flush:"post"});let e=hostCreateMutationObserver(o);e.observe(hostRead(hostRead(hostRead(t,"subTree"),"el"),"parentNode"),{childList:!0});cssVarsOnUnmounted(function(){e.disconnect()})})}var Xa=(function(){function a(e,t){var o=hostRead(L,t);var a;if(!(o==null)&&!!o)return hostString(o);o=hostString(hostCall1(styleCamelize,t));if("filter"!=o&&hostHasProperty(e,o))return hostWrite(L,t,o),o;var s=hostString(hostCall1(styleCapitalize,o));a=0;for(;a<$.length;a=a+1){o=($[a]||"")+s;if(hostHasProperty(e,o))return hostWrite(L,t,o),o}return t}function b(e,t,o){if(styleIsArray(o)){var d=+hostRead(o,"length")|0;var s=0;for(;s<d;s=s+1)b(e,t,hostRead(o,s));return}o==null&&(o=le);s=hostString(o);d=K.test(s);t.startsWith("--")?d?hostCallMethod3(e,"setProperty",t,s.replace(K,le),"important"):hostCallMethod2(e,"setProperty",t,o):(t=a(e,t),d?(t=hostString(hostCall1(styleHyphenate,t)),hostCallMethod3(e,"setProperty",t,s.replace(K,le),"important")):hostWrite(e,t,o))}function c(e,t,o,a){return "TEXTAREA"===hostRead(e,"tagName")&&("width"==t||"height"==t)&&styleIsString(a)&&o===a}return function(e,t,o){var r=hostRead(e,"style");var s=styleIsString(o);if(!(o==null)&&o&&!s){if(!(t==null)&&t){if(!styleIsString(t)){for(var a in t)hostRead(o,a)==null&&b(r,a,le)}else{s=hostString(t).split(";");a=0;for(;a<s.length;a=a+1){var d=s[a]||"";d=d.slice(0,d.indexOf(":")).trim();hostRead(o,d)==null&&b(r,d,le)}}}s=!1;for(a in o){"display"==a&&(s=!0);d=hostRead(o,a);if(!(d==null)){var h=void 0;!styleIsString(t)&&!(t==null)&&t&&(h=hostRead(t,a));c(e,a,h,d)||b(r,a,d)}else{b(r,a,le)}}}else{s?!(t===o)?(a=hostString(o),t=hostRead(r,_),!(t==null)&&!!t&&(a=a+(";"+hostString(t))),hostWrite(r,"cssText",a),s=Ja.test(a)):s=!1:(!(t==null)&&t&&hostRemoveAttribute(e,"style"),s=!1)}hostHasProperty(e,C)&&(t=s?hostRead(r,"display"):le,hostWrite(e,C,t),!hostRead(e,J)||hostWrite(r,"display","none"))};})();function qa(e,t,o,a,s){var d=!!hostCall1(attrIsSpecialBooleanAttr,t);s===void 0||(d=!!s);if(a&&t.startsWith("xlink:")){o==null?hostRemoveAttributeNS(e,me,t.slice(6)):hostSetAttributeNS(e,me,t,o);return}o==null||d&&!attrIncludeBooleanAttr(o)?hostRemoveAttribute(e,t):(d?o=le:attrIsSymbol(o)&&(o=hostString(o)),hostSetAttribute(e,t,o))}function ra(e,t,o,a,s){if("innerHTML"==t||"textContent"==t){o==null||("innerHTML"==t&&(o=la(o)),hostWrite(e,t,o));return}var h=e.tagName;var d=hostString(h);if("value"==t&&"PROGRESS"!=d&&!d.includes("-")){a=e.value;"OPTION"==d&&(a=hostGetAttribute(e,"value"),(a==null||!a)&&(a=le));o==null?(s=e.type,s="checkbox"===s?"on":le):s=hostString(o);(!(a===s)||!hostHasProperty(e,"_value"))&&hostWrite(e,"value",s);o==null&&hostRemoveAttribute(e,t);hostWrite(e,"_value",o);return}if(o===le||o==null){a=typeof e[t];"boolean"==a?(o=propIncludeBooleanAttr(o),a=!1):o==null&&"string"==a?(o=le,a=!0):"number"==a?(o=0,a=!0):a=!1}else{var i=!1,r;i&&!!propCompatUtils.isCompatEnabled.call(propCompatUtils,propDeprecationTypes.ATTR_FALSE_VALUE,a)?(a=typeof e[t],"string"==a||"number"==a?(o="number"==a?0:le,a=!0):a=!1):a=!1}try{hostWrite(e,t,o)}catch(e){r=!1;if(r){r=propWarn;var n=void 0;r("Failed setting prop \""+t+"\" on <"+d.toLowerCase()+">: value "+hostString(o)+" is invalid.",e)}}a&&(s==null||(t=hostString(s)),hostRemoveAttribute(e,t))}var Ya=(function(){function a(e){var o=void 0,t,a;for(;!0;){t=Ka.exec(e);if(t==null||La.test(e))break;o===void 0&&(o={});t=hostString(hostRead(t,1));hostWrite(o,t.toLowerCase(),!0);e=e.slice(0,e.length-t.length)}t=hostString(hostCall1(eventHyphenate,e.slice(2)));58==e.charCodeAt(2)&&(t=e.slice(3));return [t,o]}function b(){if(0!=z)return z;z=hostDateNow();hostPromiseThen(Ma,function(){z=0});return z}function c(e,t){var o;o=function(e){var a=hostRead(e,"_vts");var d,s,r,i,h;if(a==null||!a){hostWrite(e,"_vts",hostDateNow())}else{a=+a;if(a<=+hostRead(o,"attached"))return}s=hostRead(o,"value");if(eventIsArray(s)){r=hostRead(e,"stopImmediatePropagation");hostWrite(e,"stopImmediatePropagation",function(){hostCall0(r.bind(e));hostWrite(e,"_stopped",!0)});d=hostArraySlice(s);h=+hostRead(d,"length")|0;a=0;for(;a<h;a=a+1){if(hostRead(e,"_stopped"))break;s=hostRead(d,a);!(s==null)&&!!s&&eventCallWithAsyncErrorHandling(s,t,eventErrorCodes.NATIVE_EVENT_HANDLER,[e])}}else{eventCallWithAsyncErrorHandling(s,t,eventErrorCodes.NATIVE_EVENT_HANDLER,[e])}};hostWrite(o,"value",e);e=o;hostWrite(e,"attached",b());return o}return function(e,t,o,s){var d=hostRead(e,aa);d==null&&(d={},hostWrite(e,aa,d));var r=hostRead(d,t);if(!(o==null)&&o&&!(r==null)&&r){hostWrite(r,"value",o);return}var h=a(t);var n=hostRead(h,0);var i=hostString(n);h=hostRead(h,1);!(o==null)&&o?(o=c(o,s),hostWrite(d,t,o),hostAddEventListener(e,i,o,h)):!(r==null)&&r&&(hostRemoveEventListener(e,i,r,h),hostWrite(d,t,void 0))};})();function sa(e){if(e.length<3)return !1;var t=e.charCodeAt(2);return 111==e.charCodeAt(0)&&110==e.charCodeAt(1)&&t>96&&t<123}function Za(e,t,o,a){if(a){if("innerHTML"==t||"textContent"==t)return !0;if(hostHasProperty(e,t)&&sa(t)&&patchPropIsFunction(o))return !0;return !1}if("spellcheck"==t||"draggable"==t||"translate"==t||"autocorrect"==t)return !1;var d=hostRead(e,"tagName");var s=hostString(d);if("sandbox"==t&&"IFRAME"==s)return !1;if("form"==t)return !1;if("list"==t&&"INPUT"==s)return !1;if("type"==t&&"TEXTAREA"==s)return !1;if(("width"==t||"height"==t)&&("IMG"==s||"VIDEO"==s||"CANVAS"==s||"SOURCE"==s))return !1;if(sa(t)&&patchPropIsString(o))return !1;return hostHasProperty(e,t)}function _a(e,t){e=hostRead(hostRead(e,"_def"),"props");var a,o;if(e==null||!e)return !1;a=hostString(hostCall1(patchPropCamelize,t));if(patchPropIsArray(e)){o=+hostRead(e,"length")|0;t=0;for(;t<o;t=t+1){if(hostString(hostCall1(patchPropCamelize,hostString(hostRead(e,t))))==a)return !0}return !1}t=hostObjectKeys(e);o=+hostRead(t,"length")|0;e=0;for(;e<o;e=e+1){if(hostString(hostCall1(patchPropCamelize,hostString(hostRead(t,e))))==a)return !0}return !1}function $a(e,t,o,a,s,d){s="svg"===s;if("class"==t){Wa(e,a,s)}else{if("style"==t){Xa(e,o,a)}else{if(patchPropIsOn(t)){patchPropIsModelListener(t)||Ya(e,t,a,d)}else{t.startsWith(".")?(t=t.slice(1),o=!0):t.startsWith("^")?(t=t.slice(1),o=!1):o=Za(e,t,a,s);if(o){ra(e,t,a,d);!hostString(hostRead(e,"tagName")).includes("-")&&("value"==t||"checked"==t||"selected"==t)&&qa(e,t,a,s,"value"!=t);return}o=hostRead(e,"_isVueCE");var r=hostRead(hostRead(e,"_def"),"__asyncLoader");!(o==null)&&!!o&&(_a(e,t)||!(r==null)&&!!r&&(Na.test(t)||!patchPropIsString(a)))?ra(e,hostString(hostCall1(patchPropCamelize,t)),a,d,t):("true-value"==t?hostWrite(e,"_trueValue",a):"false-value"==t&&hostWrite(e,"_falseValue",a),qa(e,t,a,s,void 0))}}}}function r(e){return +hostRead(e,"length")|0}function Q(e,t){t===void 0&&(t=hostRead(e,"_parent"));var o=hostRead(e,"_app");!(t==null)&&t&&!(o==null)&&o&&(e=hostRead(hostRead(o,"_context"),"provides"),hostSetPrototypeOf(e,hostRead(hostRead(t,"_instance"),"provides")))}function ta(e,t){t===void 0&&(t=hostRead(e,"_parent"));!(t==null)&&t&&(hostWrite(hostRead(e,"_instance"),"parent",hostRead(t,"_instance")),Q(e,t))}function R(e,t){var s=r(t);var o=0,a;for(;o<s;o=o+1){a=hostRead(t,o);hostRead(e,"_setAttr").call(e,hostRead(a,"attributeName"))}}function S(e,t){if(t==null||!t)return null;var s=hostRead(e,"_styleAnchors");var o=s.get(t);var a;!(o==null)&&!!o?(a=hostRead(o,"parentNode"),a=a===hostRead(e,"shadowRoot")):a=!1;if(a)return o;!(o==null)&&o&&s.delete(t);return null}function ua(e){var t=hostRead(e,"childNodes");var a=r(t);e=0;for(;e<a;e=e+1){var o=hostRead(t,e);if(!hostIsStyleElement(o))return o}return null}function T(e,t,o,a){if(t==null||!t)return;if(!(o===void 0)&&!(o==null)&&o){var s,n,c,h,d,b,i;if(o===hostRead(e,"_def")||!!hostRead(e,"_styleChildren").has(o))return;hostRead(e,"_styleChildren").add(o)}n=hostRead(e,"_nonce");c=hostRead(e,"shadowRoot");!(a===void 0)&&!(a==null)&&a?(s=S(e,a),(s==null||!s)&&(s=S(e,hostRead(e,"_def")))):s=ua(c);h=r(t)-1|0;i=null;for(;h>=0;){d=hostCreateStyleElement();!(n==null)&&n&&d.setAttribute("nonce",n);var l=hostRead(t,h);hostWrite(d,"textContent",l);!(i==null)&&i&&(s=i);c.insertBefore(d,s);0==h&&((a==null||!a)&&hostRead(e,"_styleAnchors").set(hostRead(e,"_def"),d),!(o===void 0)&&!(o==null)&&o&&hostRead(e,"_styleAnchors").set(o,d));h=h-1;i=d}}function va(e){var s=hostCreateObject();hostWrite(e,"_slots",s);for(;;){var t=!(hostRead(e,"firstChild")==null)&&!!hostRead(e,"firstChild");if(!t){break}var o=hostRead(e,"firstChild");if(1==(+hostRead(o,"nodeType")|0)){t=o.getAttribute("slot");var a;a=!(t==null)&&!!t?hostString(t):"default"}else{a="default"}t=hostRead(s,a);t==null&&(t=[],hostWrite(s,a,t));t.push(o);e.removeChild(o)}}function wa(e){var a=[e];e=hostRead(e,"_teleportTargets");var t;if(!(e==null)&&!!e){t=hostArrayFrom(e);var o=r(t);e=0;for(;e<o;e=e+1)a.push(hostRead(t,e))}o=hostCreateSet(void 0);var d=r(a);e=0;for(;e<d;e=e+1){var s=hostRead(a,e).querySelectorAll("slot");var h=r(s);t=0;for(;t<h;t=t+1)o.add(hostRead(s,t))}return hostArrayFrom(o)}function ab(e){var l=wa(e);var n=hostRead(hostRead(hostRead(e,"_instance"),"type"),"__scopeId");var m=r(l);var d=0,o,t,a,h,b,i,c,R,g,s;for(;d<m;d=d+1){o=hostRead(l,d);t=o.getAttribute("name");t=!(t==null)&&!!t?hostString(t):"default";t=hostRead(hostRead(e,"_slots"),t);h=hostRead(o,"parentNode");if(!(t==null)&&!!t){b=r(t);s=0;for(;s<b;s=s+1){a=hostRead(t,s);if(!(n==null)&&n&&1==(+hostRead(a,"nodeType")|0)){c=hostString(n)+"-s";R=hostCreateTreeWalker(a,1);a.setAttribute(c,le);while(!0){i=R.nextNode();if(i==null||!i)break;i.setAttribute(c,le)}}h.insertBefore(a,o)}}else{for(;;){t=!(hostRead(o,"firstChild")==null)&&!!hostRead(o,"firstChild");if(!t){break}h.insertBefore(hostRead(o,"firstChild"),o)}}h.removeChild(o)}}function U(e,t){var o=hostRead(e,"_createApp")(t);hostWrite(e,"_app",o);Q(e);t=hostRead(t,"configureApp");var a;!(t==null)&&!!t&&hostCall1(t,o);hostWrite(o,"_ceVNode",X(e));hostRead(o,"mount").call(o,hostRead(e,"_root"));t=hostRead(hostRead(e,"_instance"),"exposed");if(t==null||!t)return;for(o in t)customElementHasOwn(e,o)||hostDefineAccessor(e,o,((e,t)=>function(){return customElementUnref(hostRead(e,t))})(t,o),void 0)}function xa(e,t){t=hostRead(t,"props");customElementIsArray(t)||(t==null&&(t={}),t=hostObjectKeys(t));var s=hostObjectKeys(e);var d=r(s);var a=0,o,h;for(;a<d;a=a+1){o=hostString(hostRead(s,a));!o.startsWith("_")&&!!t.includes(o)&&hostRead(e,"_setProp").call(e,o,hostRead(e,o))}s=r(t);o=0;for(;o<s;o=o+1){a=hostString(hostCall1(customElementCamelize,hostString(hostRead(t,o))));hostDefineAccessor(e,a,((e)=>function(){return hostRead(this,"_getProp").call(this,e)})(a),((e)=>function(t){let o=hostRead(this,"_setProp");o.call(this,e,t,!0,!hostRead(this,"_patching"))})(a))}}function V(e){var t=hostRead(e,"_pendingResolve");var o;if(!(t==null)&&!!t)return t;o=hostRead(e,"attributes");var a=r(o);t=0;for(;t<a;t=t+1){var s=hostRead(e,"_setAttr");s.call(e,hostRead(hostRead(o,t),"name"))}o=hostCreateMutationObserver(function(t){R(e,t)});hostWrite(e,"_ob",o);o.observe(e,{attributes:!0});o=function(t,o){hostWrite(e,"_resolved",!0);hostWrite(e,"_pendingResolve",void 0);var s=hostRead(t,"props");var a=null;if((!(s==null)&&!!s)&&!customElementIsArray(s)){for(o in s){var d=hostRead(s,o);(hostIsNumberConstructor(d)||hostIsNumberConstructor(hostRead(d,"type")))&&(hostHasProperty(hostRead(e,"_props"),o)&&(d=hostRead(e,"_props"),hostWrite(d,o,customElementToNumber(hostRead(hostRead(e,"_props"),o)))),a==null&&(a=hostCreateNullObject()),hostWrite(a,hostString(hostCall1(customElementCamelize,o)),!0))}}hostWrite(e,"_numberProps",a);xa(e,t);a=hostRead(t,"styles");!(hostRead(e,"shadowRoot")==null)&&!!hostRead(e,"shadowRoot")&&T(e,a);U(e,t)};a=hostRead(e,"_def");t=hostRead(a,"__asyncLoader");if(!(t==null)&&!!t)return t=hostCall0(t).then(function(t){hostWrite(t,"configureApp",hostRead(hostRead(e,"_def"),"configureApp"));hostWrite(e,"_def",t);hostCall2(o,t,!0)}),hostWrite(e,"_pendingResolve",t),t;hostCall2(o,a,!1)}function W(e){var o=X(e);var t=hostRead(e,"_app");var a;!(t==null)&&!!t&&hostWrite(o,"appContext",hostRead(t,"_context"));Ua(o,hostRead(e,"_root"))}function X(e){var t={};var o;(hostRead(e,"shadowRoot")==null||!hostRead(e,"shadowRoot"))&&(o=hostRead(e,"_renderSlots").bind(e),hostWrite(t,"onVnodeMounted",o),hostWrite(t,"onVnodeUpdated",o));t=customElementExtend(t,hostRead(e,"_props"));t=customElementCreateVNode(hostRead(e,"_def"),t);(hostRead(e,"_instance")==null||!hostRead(e,"_instance"))&&hostWrite(t,"ce",function(t){hostWrite(e,"_instance",t);hostWrite(t,"ce",e);hostWrite(t,"isCE",!0);var o=function(t,o){var s=hostString(t);var a={detail:o};t=hostRead(o,0);customElementIsPlainObject(t)&&(a=customElementExtend({detail:o},t,void 0));e.dispatchEvent(hostCreateCustomEvent(s,a))};hostWrite(t,"emit",function(){var e=hostString(hostRead(arguments,0));var t=hostArgumentsSlice(arguments,1);hostCall2(o,e,t);var a=hostString(hostCall1(customElementHyphenate,e));a!=e&&hostCall2(o,a,t)});ta(e)});return t}function bb(e,t,o,a){o===void 0&&(o=hostCreateObject());a===void 0&&(a=ja);hostWrite(e,"_def",t);hostWrite(e,"_props",o);hostWrite(e,"_createApp",a);hostWrite(e,"_isVueCE",!0);var s=null;hostWrite(e,"_instance",s);hostWrite(e,"_app",s);hostWrite(e,"_nonce",hostRead(t,"nonce"));hostWrite(e,"_connected",!1);hostWrite(e,"_resolved",!1);hostWrite(e,"_patching",!1);hostWrite(e,"_dirty",!1);hostWrite(e,"_numberProps",s);hostWrite(e,"_styleChildren",hostCreateWeakSet());hostWrite(e,"_pendingResolve",void 0);hostWrite(e,"_parent",void 0);hostWrite(e,"_styleAnchors",hostCreateWeakMap());hostWrite(e,"_styles",void 0);hostWrite(e,"_childStyles",void 0);hostWrite(e,"_ob",s);hostWrite(e,"_slots",void 0);var d=hostRead(e,"shadowRoot");(!(d==null)&&!!d)&&!(a===ja)?hostWrite(e,"_root",d):!(!1===hostRead(t,"shadowRoot"))?(o=hostCreateObject(),e.attachShadow(customElementExtend(o,hostRead(t,"shadowRootOptions"),{mode:"open"})),hostWrite(e,"_root",hostRead(e,"shadowRoot"))):hostWrite(e,"_root",e)}function cb(e){if(!hostRead(e,"isConnected"))return;var t,a,o;(hostRead(e,"shadowRoot")==null||!hostRead(e,"shadowRoot"))&&!hostRead(e,"_resolved")&&va(e);hostWrite(e,"_connected",!0);o=e;for(;!(o==null)&&o;){t=hostRead(o,"assignedSlot");(t==null||!t)&&(t=hostRead(o,"parentNode"));(t==null||!t)&&(t=hostRead(o,"host"));if(t==null||!t){o=t;break}if(hostInstanceOf(t,M)){hostWrite(e,"_parent",t);o=t;break}o=t}(hostRead(e,"_instance")==null||!hostRead(e,"_instance"))&&(hostRead(e,"_resolved")?U(e,hostRead(e,"_def")):(a=hostRead(o,"_pendingResolve"),!(o==null)&&o&&!(a==null)&&a?hostWrite(e,"_pendingResolve",a.then(function(){hostWrite(e,"_pendingResolve",void 0);if(hostRead(e,"isConnected"))return V(e)})):V(e)))}function db(e,t){var o=hostString(t);if(o.startsWith("data-v-"))return;t=!!e.hasAttribute(o);var a=ba;t&&(a=e.getAttribute(o));var d=customElementCamelize;var r=hostCall1(d,o);var s=hostString(r);o=hostRead(e,"_numberProps");t&&!(o==null)&&hostRead(o,s)&&(a=customElementToNumber(a));hostRead(e,"_setProp").call(e,s,a,!1,!0)}function eb(e,t){var d=hostRead(t,0);var o=hostString(d);var a=hostRead(t,1);var h=r(t)>2&&!(hostRead(t,2)===void 0)?!!hostRead(t,2):!0;t=r(t)>3&&!(hostRead(t,3)===void 0)&&!!hostRead(t,3);var s=hostRead(e,"_props");a===hostRead(s,o)||(hostWrite(e,"_dirty",!0),a===ba?delete s[o]:(hostWrite(s,o,a),"key"==o&&!(hostRead(e,"_app")==null)&&hostRead(e,"_app")&&hostWrite(hostRead(hostRead(e,"_app"),"_ceVNode"),"key",a)),t&&!(hostRead(e,"_instance")==null)&&hostRead(e,"_instance")&&W(e),h&&(t=hostRead(e,"_ob"),!(t==null)&&!!t&&(R(e,t.takeRecords()),t.disconnect()),o=hostString(hostCall1(customElementHyphenate,o)),!0===a?e.setAttribute(o,le):"string"==typeof a||"number"==typeof a?e.setAttribute(o,hostString(a)):a||e.removeAttribute(o),!(t==null)&&t&&t.observe(e,{attributes:!0})))}function ya(e,t,o){e=customElementDefineComponent(e,t);customElementIsPlainObject(e)&&(e=customElementExtend(hostCreateObject(),e,t));return hostCreateCustomElementClass(M,e,o)}function fb(e){e=hostRead(customElementGetCurrentInstance(),"ce");var t;if(!(e==null)&&!!e)return e;return null}function wb(){var e=fb();if(!(e==null)&&e)return hostRead(e,"shadowRoot");return null}function xb(e="$style"){var o=cssModuleGetCurrentInstance();var t,a;if(o==null||!o)return dependencyEmptyObject;a=o.type;t=a.__cssModules;if(t==null||!t)return dependencyEmptyObject;e=t[e];return e==null||!e?dependencyEmptyObject:e}function gb(e){var t=hostRead(e,"el");e=hostRead(t,N);var o;!(e==null)&&!!e&&e();e=hostRead(t,Oa);!(e==null)&&!!e&&e()}function hb(e){var t=ca.get(e);var o=da.get(e);var a=+hostRead(t,"left");var d=a-+hostRead(o,"left");var s=+hostRead(t,"top");s=s-+hostRead(o,"top");if(0==d&&0==s)return;t=hostRead(e,"el");a=hostRead(t,"style");o=t.getBoundingClientRect();var r=+hostRead(t,"offsetWidth");var h=+hostRead(t,"offsetHeight");t=0!=r?+hostRead(o,"width")/r:1;o=0!=h?+hostRead(o,"height")/h:1;(!hostNumberIsFinite(t)||0==t)&&(t=1);(!hostNumberIsFinite(o)||0==o)&&(o=1);hostMathAbs(t-1)<.01&&(t=1);hostMathAbs(o-1)<.01&&(o=1);t="translate("+hostString(d/t)+"px,";t=t+hostString(s/o)+"px)";hostWrite(a,"transform",t);hostWrite(a,"webkitTransform",t);hostWrite(a,"transitionDuration","0s");return e}function ib(e,t,o){var s=hostCloneNodeShallow(e);e=hostRead(e,y);var a;if(!(e==null)&&!!e){var d=hostArrayFrom(e);var n=r(d);e=0;for(;e<n;e=e+1){var h=hostString(hostRead(d,e)).split(new RegExp("\\s+"));var c=r(h);a=0;for(;a<c;a=a+1){var i=hostString(hostRead(h,a));i!=le&&hostRead(s,"classList").remove(i)}}}a=o.split(new RegExp("\\s+"));d=r(a);e=0;for(;e<d;e=e+1){o=hostString(hostRead(a,e));o!=le&&hostRead(s,"classList").add(o)}hostWrite(hostRead(s,"style"),"display","none");1!=(+hostRead(t,"nodeType")|0)&&(t=hostRead(t,"parentNode"));t.appendChild(s);e=!!hostRead(oa(s),"hasTransform");t.removeChild(s);return e}function jb(e,o){let d=transitionGroupGetCurrentInstance(),h=transitionGroupUseTransitionState();o=hostRead(o,"slots");var a=[];var s;transitionGroupOnUpdated(function(){if(0==r(a))return;var i=le;var o;if(!(hostRead(e,"moveClass")==null)&&!!hostRead(e,"moveClass")){i=hostString(hostRead(e,"moveClass"))}else{var s=!(hostRead(e,"name")==null)&&!!hostRead(e,"name")?hostString(hostRead(e,"name")):"v";i=s+"-move"}s=hostRead(hostRead(a,0),"el");if(!ib(s,hostRead(hostRead(d,"vnode"),"el"),i)){a=[];return}s=r(a);o=0;for(;o<s;o=o+1)gb(hostRead(a,o));o=0;for(;o<s;o=o+1){var h=hostRead(a,o);var R=da;var n=hostRead(h,"el").getBoundingClientRect();var m=hostRead(n,"left");R.set(h,{left:m,top:hostRead(n,"top")})}o=[];h=0;for(;h<s;h=h+1){n=hb(hostRead(a,h));n===void 0||o.push(n)}hostBodyOffsetHeight(hostRead(hostRead(d,"vnode"),"el"));h=r(o);var c=0,l;for(;c<h;c=c+1){s=hostRead(hostRead(o,c),"el");l=hostRead(s,"style");t(s,i);hostWrite(l,"transform",le);hostWrite(l,"webkitTransform",le);hostWrite(l,"transitionDuration",le);var b;b=((e)=>function(t){if(!(t==null)&&t&&!(hostRead(t,"target")===e))return;(t==null||!t||hostString(hostRead(t,"propertyName")).endsWith("transform"))&&(e.removeEventListener("transitionend",b),hostWrite(e,N,null),u(e,i))})(s);hostWrite(s,N,b);s.addEventListener("transitionend",b)}a=[]});return function(){var c=transitionGroupToRaw(e);var R=pa(c);c=hostRead(c,"tag");var i,m,t,g,n,l,b,u;(c==null||!c)&&(c=transitionGroupFragment);a=[];if(!(s===void 0)){m=r(s);b=0;for(;b<m;b=b+1){i=hostRead(s,b);t=hostRead(i,"el");(!(t==null)&&!!t)&&hostIsElement(t)&&!hostRead(t,J)&&(a.push(i),transitionGroupSetTransitionHooks(i,transitionGroupResolveTransitionHooks(i,R,h,d)),g=ca,t=t.getBoundingClientRect(),u=hostRead(t,"left"),g.set(i,{left:u,top:hostRead(t,"top")}))}}n=hostRead(o,"default");s=!(n==null)&&!!n?transitionGroupGetTransitionRawChildren(n()):[];i=r(s);l=0;for(;l<i;l=l+1){n=hostRead(s,l);hostRead(n,"key")==null||transitionGroupSetTransitionHooks(n,transitionGroupResolveTransitionHooks(n,R,h,d))}return transitionGroupCreateVNode(c,null,s)}}function v(e){var t=hostRead(hostRead(e,"props"),"onUpdate:modelValue");if(t==null||!t){}if(modelIsArray(t))return function(e){modelInvokeArrayFns(t,e)};return t}function F(e){e=hostRead(e,"modifiers");return e==null?{}:e}function Y(e,t,o){t&&(e=hostString(e).trim());o&&(e=modelLooseToNumber(e));return e}function G(e){if(hostHasProperty(e,"_value"))return hostRead(e,"_value");return hostRead(e,"value")}function za(e,t){var o=t?"_trueValue":"_falseValue";return hostHasProperty(e,o)?hostRead(e,o):t}function Aa(e,t,o,a){e=hostRead(o,"value");hostWrite(t,"_modelValue",e);a=hostRead(hostRead(a,"props"),"value");if(modelIsArray(e)){e=modelLooseIndexOf(e,a)>-1}else{if(modelIsSet(e)){e=hostSetHas(e,a)}else{if(e===hostRead(o,"oldValue"))return;e=modelLooseEqual(e,za(t,!0))}}!!hostRead(t,"checked")!=e&&hostWrite(t,"checked",e)}function kb(e,t,o){if(!o)return modelLooseEqual(e,t);if(modelIsArray(e))return modelLooseEqual(e,t);if(modelIsSet(e)){o=hostSetSize(e);if(o!=(+hostRead(t,"length")|0))return !1;var a=+hostRead(t,"length")|0;o=0;for(;o<a;o=o+1){if(!hostSetHas(e,hostRead(t,o)))return !1}return !0}return !1}function Ba(e,t){var d=!!hostRead(e,"multiple");var h=modelIsArray(t);var o;if((d&&!h)&&!modelIsSet(t)){e=modelWarn;var l=void 0;e("<select multiple v-model> expects an Array or Set value for its binding, but got "+hostRawType(t)+".");return}var i=hostRead(e,"options");var c=+hostRead(i,"length")|0;var s=0,r,a,n;for(;s<c;s=s+1){r=hostRead(i,s);o=G(r);if(d){if(h){a=typeof o;if("string"==a||"number"==a){n=+hostRead(t,"length")|0;a=0;for(;;){if(a>=n){o=!1;break}if(hostString(hostRead(t,a))==hostString(o)){o=!0;break}a=a+1}}else{o=modelLooseIndexOf(t,o)>-1}}else{o=hostSetHas(t,o)}hostWrite(r,"selected",o)}else{if(modelLooseEqual(o,t)){(+hostRead(e,"selectedIndex")|0)!=s&&hostWrite(e,"selectedIndex",s);return}}}!d&&(+hostRead(e,"selectedIndex")|0)!=-1&&hostWrite(e,"selectedIndex",-1)}function Ca(e,t){return "SELECT"==e?Sa:"TEXTAREA"==e?O:"checkbox"===t?ea:"radio"===t?fa:O}function H(e,t,o,a,s){var d=hostRead(o,"props");var r=hostString(hostRead(e,"tagName"));s=hostRead(Ca(r,hostRead(d,"type")),s);!(s==null)&&!!s&&hostCall4(s,e,t,o,a)}function lb(e,t){var a=+hostRead(e,"length")|0;var o=0;for(;o<a;o=o+1){if(hostString(hostRead(e,o))==t)return !0}return !1}function mb(e,t,o){if("stop"==t)return e.stopPropagation(),!1;if("prevent"==t)return e.preventDefault(),!1;if("self"==t)return t=hostRead(e,"target"),!(t===hostRead(e,"currentTarget"));if("ctrl"==t)return !hostRead(e,"ctrlKey");if("shift"==t)return !hostRead(e,"shiftKey");if("alt"==t)return !hostRead(e,"altKey");if("meta"==t)return !hostRead(e,"metaKey");if("left"==t)return e=hostHasProperty(e,"button")&&0!=+hostRead(e,"button"),e;if("middle"==t)return e=hostHasProperty(e,"button")&&1!=+hostRead(e,"button"),e;if("right"==t)return e=hostHasProperty(e,"button")&&2!=+hostRead(e,"button"),e;if("exact"==t){t=0;for(;t<ga.length;t=t+1){var a=ga[t]||"";if(hostRead(e,a+"Key")&&!lb(o,a))return !0}}return !1}function yb(e,t){if(e==null||!e)return e;var a=hostRead(e,"_withMods");a==null&&(a={},hostWrite(e,"_withMods",a));var s=hostString(t.join("."));var o=hostRead(a,s);var d;if(!(o==null)&&!!o)return o;o=hostEventRestWrapper(function(o,a){var d=+hostRead(t,"length")|0;var s=0;for(;s<d;s=s+1){if(mb(o,hostString(hostRead(t,s)),t))return}return hostCallWithEventArgs(e,o,a)});hostWrite(a,s,o);return o}function nb(e){return "esc"==e?"escape":"space"==e?" ":"up"==e?"arrow-up":"left"==e?"arrow-left":"right"==e?"arrow-right":"down"==e?"arrow-down":"delete"==e?"backspace":le}function ob(e,t){var s=+hostRead(e,"length")|0;var o=0,a;for(;o<s;o=o+1){a=hostString(hostRead(e,o));if(a==t||nb(a)==t)return !0}return !1}function zb(e,t){var d;var r=null;var a=hostRead(e,"_withKeys");a==null&&(a={},hostWrite(e,"_withKeys",a));var s=hostString(t.join("."));var o=hostRead(a,s);var h;if(!(o==null)&&!!o)return o;o=function(o){if(!hostHasProperty(o,"key"))return;var a=vOnHyphenate;if(ob(t,hostString(hostCall1(a,hostRead(o,"key")))))return hostCall1(e,o)};hostWrite(a,s,o);return o}function Da(){(w==null||!w)&&(w=runtimeDomCreateRenderer(ha));return w}function Ea(){ia||(w=runtimeDomCreateHydrationRenderer(ha));ia=!0;return w}function Fa(e){if(runtimeDomIsString(e))return e=runtimeDomQuerySelector(runtimeDomString(e)),e;return e}function Ab(){var e=ka;if(e)return;ka=!0;hostWrite(O,"getSSRProps",function(e){return {value:hostRead(e,"value")}});hostWrite(fa,"getSSRProps",function(e,t){var o=hostRead(t,"props");if(!(o==null)&&modelLooseEqual(hostRead(o,"value"),hostRead(e,"value")))return {checked:!0}});hostWrite(ea,"getSSRProps",function(e,t){var o=hostRead(e,"value");e=hostRead(t,"props");if(modelIsArray(o)){if(!(e==null)&&modelLooseIndexOf(o,hostRead(e,"value"))>-1)return {checked:!0}}else{if(modelIsSet(o)){if(!(e==null)&&hostSetHas(o,hostRead(e,"value")))return {checked:!0}}else{if(o)return {checked:!0}}}});hostWrite(Ta,"getSSRProps",function(e,t){var o=hostRead(t,"type");if("string"!=typeof o)return;var a=hostRead(t,"props");o=hostString(o).toUpperCase();o=hostRead(Ca(o,hostRead(a,"type")),"getSSRProps");if(!(o==null)&&!!o)return hostCall2(o,e,t)});hostWrite(Ia,"getSSRProps",function(e){return !hostRead(e,"value")?{style:{display:"none"}}:void 0})}var B=void 0;var e=hostTrustedTypes();var f;if(!(e==null)&&!!e)try{B=hostCreateTrustedTypesPolicy(e,"vue",function(e){return e})}catch(t){e=nodeOpsWarn;var ve=void 0;e("Error creating trusted types policy: "+hostString(t))}var Ga={insert:function(e,t,o){hostInsert(e,t,o)},remove:function(e){hostRemove(e)},createElement:function(e,t,o,a){e=hostString(e);var s=hostCreateElement(e,t,o);("select"==e&&!(a==null))&&!(a.multiple==null)&&hostSetAttribute(s,"multiple",a.multiple);return s},createText:function(e){return hostCreateText(hostString(e))},createComment:function(e){return hostCreateComment(hostString(e))},setText:function(e,t){hostSetNodeValue(e,hostString(t))},setElementText:function(e,t){hostSetTextContent(e,hostString(t))},parentNode:function(e){return hostParentNode(e)},nextSibling:function(e){return hostNextSibling(e)},querySelector:function(e){return hostQuerySelector(hostString(e))},setScopeId:function(e,t){hostSetAttribute(e,hostString(t),le)},insertStaticContent:function(e,t,o,a,s,d){var h=hostLastChild(t);o==null||(h=hostPreviousSibling(o));if(!(s==null)&&(s===d||!(hostNextSibling(s)==null))){while(!0){hostInsert(hostCloneNode(s),t,o);if(s===d)break;s=hostNextSibling(s);if(s==null)break}}else{a=!(a==null)?hostString(a):le;e=hostString(e);"svg"==a?e="<svg>"+e+"</svg>":"mathml"==a&&(e="<math>"+e+"</math>");var i=la(e);hostSetTemplateHTML(i);var r=hostTemplateContent();if("svg"==a||"mathml"==a){e=hostFirstChild(r);while(!(hostFirstChild(e)==null))hostAppendChild(r,hostFirstChild(e));hostRemoveChild(r,e)}hostInsert(r,t,o)}e=hostFirstChild(t);h==null||(e=hostNextSibling(h));t=hostLastChild(t);o==null||(t=hostPreviousSibling(o));return [e,t]}};var y=hostCreateSymbol("_vtc");e=hostConstructors();f=e.String;var g=e.String;var i=e.Boolean;i={type:i,default:!0};var j=e.String;var k=e.Number;j=[j,k,e.Object];k=e.String;var l=e.String;var m=e.String;var n=e.String;var o=e.String;var p=e.String;var q=e.String;var ne=e.String;var Z={name:f,type:g,css:i,duration:j,enterFromClass:k,enterActiveClass:l,enterToClass:m,appearFromClass:n,appearActiveClass:o,appearToClass:p,leaveFromClass:q,leaveActiveClass:ne,leaveToClass:e.String};g=transitionExtend({},dependencyBaseTransitionPropsValidators,Z);var I=0;var Ha=new RegExp("\\b(?:transform|all)(?:,|$)");f=function(e,t){return dependencyH(dependencyBaseTransition,pa(e),hostRead(t,"slots"))};hostWrite(f,"displayName","Transition");hostWrite(f,"props",g);var pb=f;var C=hostCreateSymbol("_vod");var J=hostCreateSymbol("_vsh");var Ia={name:"show",beforeMount:function(e,t,o){var a=hostRead(hostRead(e,"style"),"display");"none"===a&&(a=le);hostWrite(e,C,a);var s=hostRead(t,"value");t=hostRead(o,"transition");(!(t==null)&&!!t)&&s?hostCall1(hostRead(t,"beforeEnter"),e):A(e,s)},mounted:function(e,t,o){var a=hostRead(t,"value");t=hostRead(o,"transition");var s;(!(t==null)&&!!t)&&a&&hostCall1(hostRead(t,"enter"),e)},updated:function(e,t,o){var s=hostRead(t,"value");if(!s==!hostRead(t,"oldValue"))return;var a=hostRead(o,"transition");!(a==null)&&!!a?s?(hostCall1(hostRead(a,"beforeEnter"),e),A(e,!0),hostCall1(hostRead(a,"enter"),e)):hostCall2(hostRead(a,"leave"),e,function(){A(e,!1)}):A(e,s)},beforeUnmount:function(e,t){A(e,hostRead(t,"value"))}};var _=hostCreateSymbol("CSS_VAR_TEXT");var Ja=new RegExp("(?:^|;)\\s*display\\s*:");new RegExp("[^\\\\];\\s*$");var K=new RegExp("\\s*!important$");var $=["Webkit","Moz","ms"];var L={};var aa=hostCreateSymbol("_vei");var Ka=new RegExp("(Once|Passive|Capture)$");var La=new RegExp("^on:?(?:Once|Passive|Capture)$");var z=0;var Ma=hostResolvedPromise();var Na=new RegExp("[A-Z]");var ba=hostCreateObject();var M=void 0;M=hostCreateVueElementClass(function(e,t,o){bb(this,e,t,o)},{connectedCallback:function(){return cb(this)},disconnectedCallback:function(){var e=this;hostWrite(e,"_connected",!1);hostCall1(customElementNextTick,function(){if(!hostRead(e,"_connected")){var t=hostRead(e,"_ob");var o;!(t==null)&&!!t&&(t.disconnect(),hostWrite(e,"_ob",null));t=hostRead(e,"_app");!(t==null)&&!!t&&hostRead(t,"unmount").call(t);t=hostRead(e,"_instance");!(t==null)&&!!t&&hostWrite(t,"ce",void 0);o=null;hostWrite(e,"_app",o);hostWrite(e,"_instance",o);t=hostRead(e,"_teleportTargets");!(t==null)&&!!t&&(t.clear(),hostWrite(e,"_teleportTargets",void 0))}})},_setParent:function(e){ta(this,e)},_inheritParentContext:function(e){Q(this,e)},_processMutations:function(e){R(this,e)},_resolveDef:function(){return V(this)},_mount:function(e){U(this,e)},_resolveProps:function(e){xa(this,e)},_setAttr:function(e){return db(this,e)},_getProp:function(e){return hostRead(hostRead(this,"_props"),e)},_setProp:function(){return eb(this,arguments)},_update:function(){W(this)},_createVNode:function(){return X(this)},_applyStyles:function(){let e=hostRead(arguments,0),t=hostRead(arguments,1);T(this,e,t,hostRead(arguments,2))},_getStyleAnchor:function(e){return S(this,e)},_getRootStyleInsertionAnchor:function(e){return ua(e)},_parseSlots:function(){va(this)},_renderSlots:function(){ab(this)},_getSlots:function(){return wa(this)},_injectChildStyle:function(e,t){T(this,hostRead(e,"styles"),e,t)},_beginPatch:function(){hostWrite(this,"_patching",!0);hostWrite(this,"_dirty",!1)},_endPatch:function(){hostWrite(this,"_patching",!1);hostRead(this,"_dirty")&&!(hostRead(this,"_instance")==null)&&hostRead(this,"_instance")&&W(this)},_hasShadowRoot:function(){return !(!1===hostRead(hostRead(this,"_def"),"shadowRoot"))},_removeChildStyle:function(e){}});var qb=hostFunction1Rest("defineCustomElement",function(e,t){let o=hostRead(t,0);e=hostRead(t,1);return ya(o,e,hostRead(t,2))});var rb=function(e,t){return ya(e,t,Va)};var ca=hostCreateWeakMap();var da=hostCreateWeakMap();var N=hostCreateSymbol("_moveCb");var Oa=hostCreateSymbol("_enterCb");e=hostConstructors();f=transitionGroupExtend;i=e.String;f=f({},g,{tag:i,moveClass:e.String});var Jg=void 0;hostDelete(f,"mode");f={name:"TransitionGroup",props:f,setup:function(e,t){return jb(e,t)}};var sb=f;var s=hostCreateSymbol("_assign");var D=hostCreateSymbol("_initialValue");var Pa=new RegExp("[\\r\\n]","g");var Qa=new RegExp("\\r\\n?","g");var Ra=new RegExp("^0\\d");var O={created:function(e,t,o){var d=hostParentNode(e);var a=hostString(hostRead(e,"type"));d==null||("text"==a?(a=D,hostWrite(e,a,hostString(hostRead(e,"defaultValue")).replace(Pa,le))):"textarea"==a&&(a=D,hostWrite(e,a,hostString(hostRead(e,"defaultValue")).replace(Qa,"\n"))));hostWrite(e,s,v(o));a=F(t);var r=!!hostRead(a,"lazy");d=!!hostRead(a,"trim");a=!!hostRead(a,"number");var h=hostRead(o,"props");a=a||"number"===hostRead(h,"type");t=function(t){if(hostRead(hostRead(t,"target"),"composing"))return;var o=hostRead(e,s);hostCall1(o,Y(hostRead(e,"value"),d,a))};o=r?"change":"input";hostAddEventListener(e,o,t,void 0);(d||a)&&hostAddEventListener(e,"change",function(){hostWrite(e,"value",Y(hostRead(e,"value"),d,a))},void 0);r||(hostAddEventListener(e,"compositionstart",function(e){hostWrite(hostRead(e,"target"),"composing",!0)},void 0),hostAddEventListener(e,"compositionend",function(e){var t=hostRead(e,"target");!hostRead(t,"composing")||(hostWrite(t,"composing",!1),t.dispatchEvent(hostCreateEvent("input")))},void 0),hostAddEventListener(e,"change",function(e){var t=hostRead(e,"target");!hostRead(t,"composing")||(hostWrite(t,"composing",!1),t.dispatchEvent(hostCreateEvent("input")))},void 0))},mounted:function(e,t){var o=hostRead(t,"value");o==null&&(o=le);var a=hostRead(e,D);hostDelete(e,D);var d=hostString(hostRead(e,"type"));var r=F(t);var h;(!(a===void 0)&&("text"==d||"textarea"==d))&&!(hostRead(e,"value")===a)?(h=hostRead(e,s),e=hostRead(e,"value"),t=!!hostRead(r,"trim"),hostCall1(h,Y(e,t,!!hostRead(r,"number")))):hostWrite(e,"value",o)},beforeUpdate:function(e,t,o){hostWrite(e,s,v(o));if(hostRead(e,"composing"))return;var a=F(t);var r=!!hostRead(a,"lazy");var h=!!hostRead(a,"trim");a=!!hostRead(a,"number");var d=hostString(hostRead(e,"value"));var i=(a||"number"===hostRead(e,"type"))&&!Ra.test(d)?modelLooseToNumber(d):d;a=hostRead(t,"value");o=a==null?le:a;if(i===o)return;if(hostActiveElementIs(e)&&!("range"===hostRead(e,"type"))){if(r&&a===hostRead(t,"oldValue"))return;if(h&&d.trim()==hostString(o))return}hostWrite(e,"value",o)}};var ea={deep:!0,created:function(e,t,o){hostWrite(e,s,v(o));hostAddEventListener(e,"change",function(){var t=hostRead(e,"_modelValue");var a=G(e);var d=!!hostRead(e,"checked");var r=hostRead(e,s);if(modelIsArray(t)){var h=modelLooseIndexOf(t,a);var i=h!=-1;if(d&&!i){hostCall1(r,t.concat(a))}else{if(!d&&i){var o=hostArraySlice(t);hostArraySplice(o,h,1);hostCall1(r,o)}}}else{modelIsSet(t)?(o=hostCreateSet(t),d?hostSetAdd(o,a):hostSetDelete(o,a),hostCall1(r,o)):hostCall1(r,za(e,d))}},void 0)},mounted:ub(Aa),beforeUpdate:function(e,t,o){hostWrite(e,s,v(o));return Aa(this,e,t,o)}};var fa={created:function(e,t,o){let a=hostRead(t,"value");hostWrite(e,"checked",modelLooseEqual(a,hostRead(hostRead(o,"props"),"value")));hostWrite(e,s,v(o));hostAddEventListener(e,"change",function(){hostCall1(hostRead(e,s),G(e))},void 0)},beforeUpdate:function(e,t,o){hostWrite(e,s,v(o));var a=hostRead(t,"value");a===hostRead(t,"oldValue")||(a=hostRead(t,"value"),hostWrite(e,"checked",modelLooseEqual(a,hostRead(hostRead(o,"props"),"value"))))}};var Sa={deep:!0,created:function(e,t,o){hostWrite(e,"_modelValue",hostRead(t,"value"));let a=!!hostRead(F(t),"number");hostAddEventListener(e,"change",function(){var r=e;var t=[];var i=hostRead(r,"options");var c=+hostRead(i,"length")|0;var h=0,o,d;for(;h<c;h=h+1){o=hostRead(i,h);!hostRead(o,"selected")||(o=G(o),a&&(o=modelLooseToNumber(o)),t.push(o))}o=!!hostRead(r,"multiple");d=void 0;o?d=modelIsSet(hostRead(r,"_modelValue"))?hostCreateSet(t):t:0!=(+hostRead(t,"length")|0)&&(d=hostRead(t,0));o?modelIsArray(d)&&(t=hostArraySlice(t)):t=d;var n=[o,t];hostWrite(r,"_pendingValue",n);try{hostCall1(hostRead(r,s),d)}finally{d=r;t=n;hostCall1(modelNextTick,function(){hostRead(d,"_pendingValue")===t&&hostWrite(d,"_pendingValue",void 0)})}},void 0);hostWrite(e,s,v(o))},mounted:function(e,t){Ba(e,hostRead(t,"value"))},beforeUpdate:function(e,t,o){hostWrite(e,"_modelValue",hostRead(t,"value"));hostWrite(e,s,v(o))},updated:function(e,t){var a=hostRead(e,"_pendingValue");hostWrite(e,"_pendingValue",void 0);var s=!!hostRead(e,"multiple");var o;a==null||!(hostRead(a,0)===s)?o=!0:(o=hostRead(t,"value"),o=!kb(o,hostRead(a,1),s));o&&Ba(e,hostRead(t,"value"))}};var Ta={created:function(e,t,o){H(e,t,o,null,"created")},mounted:function(e,t,o){H(e,t,o,null,"mounted")},beforeUpdate:function(e,t,o,a){H(e,t,o,a,"beforeUpdate")},updated:function(e,t,o,a){H(e,t,o,a,"updated")}};var ga=["ctrl","shift","alt","meta"];new RegExp("^\\d+$");var ha=runtimeDomExtend({patchProp:$a},Ga);var w=void 0;var ia=!1;var Ua=function(){let e=Da();return runtimeDomApply(runtimeDomRead(e,"render"),e,arguments)};var tb=function(){let e=Ea();return runtimeDomApply(runtimeDomRead(e,"hydrate"),e,arguments)};var ja=function(){var e=Da();e=runtimeDomApply(runtimeDomRead(e,"createApp"),e,arguments);var t=runtimeDomRead(e,"mount");runtimeDomWrite(e,"mount",function(o){var a=Fa(o);if(a==null||!a)return;var s=runtimeDomRead(e,"_component");(!runtimeDomIsFunction(s)&&!runtimeDomRead(s,"render"))&&!runtimeDomRead(s,"template")&&runtimeDomWrite(s,"template",runtimeDomRead(a,"innerHTML"));1==(+runtimeDomRead(a,"nodeType")|0)&&runtimeDomSetTextContent(a,le);s=t(a,!1,runtimeDomResolveRootNamespace(a));runtimeDomIsElement(a)&&(runtimeDomRemoveAttribute(a,"v-cloak"),runtimeDomSetAttribute(a,"data-v-app",le));return s});return e};var Va=function(){var e=Ea();e=runtimeDomApply(runtimeDomRead(e,"createApp"),e,arguments);var t=runtimeDomRead(e,"mount");runtimeDomWrite(e,"mount",function(e){var o=Fa(e);if(!(o==null)&&!!o)return t(o,!0,runtimeDomResolveRootNamespace(o))});return e};var ka=!1;Object.defineProperties(Ab,{name:{configurable:true,value:"initDirectivesForSSR"},length:{configurable:true,value:0}});Object.defineProperties(Ga.insert,{name:{configurable:true,value:"insert"},length:{configurable:true,value:3}});Object.defineProperties(Ga.remove,{name:{configurable:true,value:"remove"},length:{configurable:true,value:1}});Object.defineProperties(Ga.createElement,{name:{configurable:true,value:"createElement"},length:{configurable:true,value:4}});Object.defineProperties(Ga.createText,{name:{configurable:true,value:"createText"},length:{configurable:true,value:1}});Object.defineProperties(Ga.createComment,{name:{configurable:true,value:"createComment"},length:{configurable:true,value:1}});Object.defineProperties(Ga.setText,{name:{configurable:true,value:"setText"},length:{configurable:true,value:2}});Object.defineProperties(Ga.setElementText,{name:{configurable:true,value:"setElementText"},length:{configurable:true,value:2}});Object.defineProperties(Ga.parentNode,{name:{configurable:true,value:"parentNode"},length:{configurable:true,value:1}});Object.defineProperties(Ga.nextSibling,{name:{configurable:true,value:"nextSibling"},length:{configurable:true,value:1}});Object.defineProperties(Ga.querySelector,{name:{configurable:true,value:"querySelector"},length:{configurable:true,value:1}});Object.defineProperties(Ga.setScopeId,{name:{configurable:true,value:"setScopeId"},length:{configurable:true,value:2}});Object.defineProperties(Ga.insertStaticContent,{name:{configurable:true,value:"insertStaticContent"},length:{configurable:true,value:6}});Object.defineProperties($a,{name:{configurable:true,value:"patchProp"},length:{configurable:true,value:6}});Object.defineProperties(ea.created,{name:{configurable:true,value:"created"},length:{configurable:true,value:3}});Object.defineProperties(ea.mounted,{name:{configurable:true,value:"setChecked"},length:{configurable:true,value:3}});Object.defineProperties(ea.beforeUpdate,{name:{configurable:true,value:"beforeUpdate"},length:{configurable:true,value:3}});Object.defineProperties(Ta.created,{name:{configurable:true,value:"created"},length:{configurable:true,value:3}});Object.defineProperties(Ta.mounted,{name:{configurable:true,value:"mounted"},length:{configurable:true,value:3}});Object.defineProperties(Ta.beforeUpdate,{name:{configurable:true,value:"beforeUpdate"},length:{configurable:true,value:4}});Object.defineProperties(Ta.updated,{name:{configurable:true,value:"updated"},length:{configurable:true,value:4}});Object.defineProperties(fa.created,{name:{configurable:true,value:"created"},length:{configurable:true,value:3}});Object.defineProperties(fa.beforeUpdate,{name:{configurable:true,value:"beforeUpdate"},length:{configurable:true,value:3}});Object.defineProperties(Sa.created,{name:{configurable:true,value:"created"},length:{configurable:true,value:3}});Object.defineProperties(Sa.mounted,{name:{configurable:true,value:"mounted"},length:{configurable:true,value:2}});Object.defineProperties(Sa.beforeUpdate,{name:{configurable:true,value:"beforeUpdate"},length:{configurable:true,value:3}});Object.defineProperties(Sa.updated,{name:{configurable:true,value:"updated"},length:{configurable:true,value:2}});Object.defineProperties(O.created,{name:{configurable:true,value:"created"},length:{configurable:true,value:3}});Object.defineProperties(O.mounted,{name:{configurable:true,value:"mounted"},length:{configurable:true,value:2}});Object.defineProperties(O.beforeUpdate,{name:{configurable:true,value:"beforeUpdate"},length:{configurable:true,value:3}});Object.defineProperties(Ia.beforeMount,{name:{configurable:true,value:"beforeMount"},length:{configurable:true,value:3}});Object.defineProperties(Ia.mounted,{name:{configurable:true,value:"mounted"},length:{configurable:true,value:3}});Object.defineProperties(Ia.updated,{name:{configurable:true,value:"updated"},length:{configurable:true,value:3}});Object.defineProperties(Ia.beforeUnmount,{name:{configurable:true,value:"beforeUnmount"},length:{configurable:true,value:2}});Object.defineProperties(zb,{name:{configurable:true,value:"withKeys"},length:{configurable:true,value:2}});Object.defineProperties(yb,{name:{configurable:true,value:"withModifiers"},length:{configurable:true,value:2}});export{Ua as render,tb as hydrate,ja as createApp,Va as createSSRApp,Ab as initDirectivesForSSR,Bb as BaseTransition,Cb as BaseTransitionPropsValidators,Db as Comment,Eb as DeprecationTypes,Fb as EffectScope,Gb as ErrorCodes,Hb as ErrorTypeStrings,Ib as Fragment,Jb as KeepAlive,Kb as ReactiveEffect,Lb as Static,Mb as Suspense,Nb as Teleport,Ob as Text,Pb as TrackOpTypes,pb as Transition,sb as TransitionGroup,Qb as TriggerOpTypes,M as VueElement,Rb as assertNumber,Sb as callWithAsyncErrorHandling,Tb as callWithErrorHandling,Ub as camelize,Vb as capitalize,Wb as cloneVNode,Xb as compatUtils,Yb as computed,Zb as createBlock,_b as createCommentVNode,qb as defineCustomElement,rb as defineSSRCustomElement,$b as createElementBlock,ac as createElementVNode,bc as createHydrationRenderer,cc as createPropsRestProxy,dc as createRenderer,ec as createSlots,fc as createStaticVNode,gc as createTextVNode,hc as createVNode,ic as customRef,jc as defineAsyncComponent,kc as defineComponent,lc as defineEmits,mc as defineExpose,nc as defineModel,oc as defineOptions,pc as defineProps,qc as defineSlots,rc as devtools,sc as effect,tc as effectScope,uc as getCurrentInstance,vc as getCurrentScope,wc as getCurrentWatcher,xc as getTransitionRawChildren,yc as guardReactiveProps,zc as h,Ac as handleError,Bc as hasInjectionContext,Cc as hydrateOnIdle,Dc as hydrateOnInteraction,Ec as hydrateOnMediaQuery,Fc as hydrateOnVisible,Gc as initCustomFormatter,Hc as inject,Ic as isMemoSame,Jc as isProxy,Kc as isReactive,Lc as isReadonly,Mc as isRef,Nc as isRuntimeOnly,Oc as isShallow,Pc as isVNode,Qc as markRaw,Rc as mergeDefaults,Sc as mergeModels,Tc as mergeProps,Uc as nextTick,Ga as nodeOps,Vc as normalizeClass,Wc as normalizeProps,Xc as normalizeStyle,Yc as onActivated,Zc as onBeforeMount,_c as onBeforeUnmount,$c as onBeforeUpdate,ad as onDeactivated,bd as onErrorCaptured,cd as onMounted,dd as onRenderTracked,ed as onRenderTriggered,fd as onScopeDispose,gd as onServerPrefetch,hd as onUnmounted,id as onUpdated,jd as onWatcherCleanup,kd as openBlock,$a as patchProp,ld as popScopeId,md as provide,nd as proxyRefs,od as pushScopeId,pd as queuePostFlushCb,qd as reactive,rd as readonly,sd as ref,td as registerRuntimeCompiler,ud as renderList,vd as renderSlot,wd as resolveComponent,xd as resolveDirective,yd as resolveDynamicComponent,zd as resolveFilter,Ad as resolveTransitionHooks,Bd as setBlockTracking,Cd as setDevtoolsHook,Dd as setTransitionHooks,Ed as shallowReactive,Fd as shallowReadonly,Gd as shallowRef,Hd as ssrContextKey,Id as ssrUtils,Jd as stop,Kd as toDisplayString,Ld as toHandlerKey,Md as toHandlers,Nd as toRaw,Od as toRef,Pd as toRefs,Qd as toValue,Rd as transformVNodeArgs,Sd as triggerRef,Td as unref,Ud as useAttrs,xb as useCssModule,vb as useCssVars,fb as useHost,Vd as useId,Wd as useModel,Xd as useSSRContext,wb as useShadowRoot,Yd as useSlots,Zd as useTemplateRef,_d as useTransitionState,ea as vModelCheckbox,Ta as vModelDynamic,fa as vModelRadio,Sa as vModelSelect,O as vModelText,Ia as vShow,$d as version,ae as warn,be as watch,ce as watchEffect,de as watchPostEffect,ee as watchSyncEffect,fe as withAsyncContext,ge as withCtx,he as withDefaults,ie as withDirectives,zb as withKeys,je as withMemo,yb as withModifiers,ke as withScopeId};export*from"./runtime-core.js";
