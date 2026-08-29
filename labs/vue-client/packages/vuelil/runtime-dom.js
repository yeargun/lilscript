// Generated from src/runtime-dom/index.lil and its measured DOM host adapter.
// DOM and ECMAScript primitives used by the LilScript runtime-dom kernel.

const doc = typeof document !== "undefined" ? document : null;
const templateContainer = doc && doc.createElement("template");

let trustedTypesPolicy;
const trustedTypes = typeof window !== "undefined" && window.trustedTypes;
if (trustedTypes) {
  try {
    trustedTypesPolicy = trustedTypes.createPolicy("vue", { createHTML: value => value });
  } catch (error) {
    console.warn(`[Vue warn]: Error creating trusted types policy: ${error}`);
  }
}

function hostUnsafeToTrustedHTML(value) {
  return trustedTypesPolicy ? trustedTypesPolicy.createHTML(value) : value;
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

import{nextTick as dependencyNextTick}from"./runtime-core.js";import{camelize as dependencyCamelize,capitalize as dependencyCapitalize,hyphenate as dependencyHyphenate,includeBooleanAttr as dependencyIncludeBooleanAttr,isArray as dependencyIsArray,isFunction as dependencyIsFunction,isModelListener as dependencyIsModelListener,isOn as dependencyIsOn,isSet as dependencyIsSet,isSpecialBooleanAttr as dependencyIsSpecialBooleanAttr,isString as dependencyIsString,isSymbol as dependencyIsSymbol,looseEqual as dependencyLooseEqual,looseIndexOf as dependencyLooseIndexOf,looseToNumber as dependencyLooseToNumber}from"./shared.js";let $jsMethod3=e=>function(a,b,c){return e(this,a,b,c)},svgNS="http://www.w3.org/2000/svg",mathmlNS="http://www.w3.org/1998/Math/MathML",xlinkNS="http://www.w3.org/1999/xlink";function valueLength(value){return +hostRead(value,"length")|0}function isFunctionValue(value){}function same(left,right){return left===right}function unsafeToTrustedHTML(value){return hostUnsafeToTrustedHTML(value)}function patchClass(element,value,isSVG){var v5=hostRead(element,vtcKey);if(!(v5==null)){var v10=hostArrayFrom(v5);if(!(value==null)){var v18=""!=hostString(value);var v19=v18,v21,v23,v41,v45,v60,v61,v62}else{v19=!1}if(v19){v21=[value];v23=valueLength(v10);v62=0;for(;v62<v23;v62=v62+1)hostWrite(v21,valueLength(v21),hostRead(v10,v62));v41=v21.join(" ");v60=v41}else{v45=v10.join(" ");v60=v45}v61=v60}else{v61=value}if(v61==null){hostRemoveAttribute(element,"class")}else{if(isSVG){hostSetAttribute(element,"class",v61)}else{hostWrite(element,"className",v61)}}}function callStringTransform(transform,value){return hostString(hostCall1(transform,value))}function autoPrefix(style,rawName){var v4=hostRead(stylePrefixCache,rawName);if(!(v4==null)){var v12=""!=hostString(v4);var v13=v12,v18,v25,v26,v34,v44,v57}else{v13=!1}if(v13)return hostString(v4);v18=callStringTransform(dependencyCamelize,rawName);if("filter"!=v18){v25=hostHasProperty(style,v18);v26=v25}else{v26=!1}if(v26)return hostWrite(stylePrefixCache,rawName,v18),v18;v34=callStringTransform(dependencyCapitalize,v18);v57=0;for(;v57<stylePrefixes.length;v57=v57+1){v44=(stylePrefixes[v57]||"")+v34;if(hostHasProperty(style,v44))return hostWrite(stylePrefixCache,rawName,v44),v44}return rawName}function setStyle(style,name,value){if(dependencyIsArray(value)){var v6=valueLength(value);var v82=0,v24,v41,v48,v66,v81,v83;for(;v82<v6;v82=v82+1)setStyle(style,name,hostRead(value,v82));return}if(value==null){v81=""}else{v81=value}v24=hostString(v81);if(semicolonPattern.test(v24))hostWarn("Unexpected semicolon at the end of '"+name+"' style value: '"+v24+"'",void 0);v41=importantPattern.test(v24);if(v41){v48=hostString(v24.replace(importantPattern,""));v83=v48}else{v83=v24}if(name.startsWith("--")){if(v41){hostCallMethod3(style,"setProperty",name,v83,"important")}else{hostCallMethod2(style,"setProperty",name,v83)}}else{v66=autoPrefix(style,name);if(v41){hostCallMethod3(style,"setProperty",callStringTransform(dependencyHyphenate,v66),v83,"important")}else{hostWrite(style,v66,v81)}}}function preserveTextareaResize(element,key,previous,next){if("TEXTAREA"==hostString(hostRead(element,"tagName"))){if("width"==key){var v19=!0,v20,v23,v24,v28,v29}else{v19="height"==key}v20=v19}else{v20=!1}if(v20){v23=dependencyIsString(next);v24=v23}else{v24=!1}if(v24){v28=same(previous,next);v29=v28}else{v29=!1}return v29}function patchStyle(element,previous,next){var v5=hostRead(element,"style");var v7=dependencyIsString(next);if(!(next==null)){var v14=!!next;var v15=v14,v18,v19,v25,v26,v31,v42,v50,v58,v71,v77,v81,v89,v92,v93,v95,v113,v116,v122,v123,v129,v136,v142,v143,v154,v161,v167,v168,v173,v174,v175,v176,v177,v178,v179,v180,v181}else{v15=!1}if(v15){v18=!v7;v19=v18}else{v19=!1}if(v19){if(!(previous==null)){v25=!!previous;v26=v25}else{v26=!1}if(v26){if(!dependencyIsString(previous)){for(v31 in previous){if(hostRead(next,v31)==null)setStyle(v5,v31,"")}}else{v42=hostString(previous).split(";");v178=0;for(;v178<v42.length;v178=v178+1){v50=v42[v178]||"";v58=v50.slice(0,v50.indexOf(":")).trim();if(hostRead(next,v58)==null)setStyle(v5,v58,"")}}}v177=!1;for(v71 in next){if("display"==v71){v176=!0}else{v176=v177}v77=hostRead(next,v71);if(!(v77==null)){v81=void 0;if(!dependencyIsString(previous)){v89=!(previous==null)}else{v89=!1}if(v89){v92=!!previous;v93=v92}else{v93=!1}if(v93){v95=hostRead(previous,v71);v179=v95}else{v179=v81}if(!preserveTextareaResize(element,v71,v179,v77))setStyle(v5,v71,v77)}else{setStyle(v5,v71,"")}v177=v176}v175=v177}else{if(v7){if(!same(previous,next)){v113=hostString(next);v116=hostRead(v5,CSS_VAR_TEXT);if(!(v116==null)){v122=!!v116;v123=v122}else{v123=!1}if(v123){v129=v113+(";"+hostString(v116));v180=v129}else{v180=v113}hostWrite(v5,"cssText",v180);v136=displayPattern.test(v180);v173=v136}else{v173=!1}v174=v173}else{if(!(previous==null)){v142=!!previous;v143=v142}else{v143=!1}if(v143)hostRemoveAttribute(element,"style");v174=!1}v175=v174}if(hostHasProperty(element,vShowOriginalDisplay)){if(v175){v154=hostRead(v5,"display");v181=v154}else{v181=""}hostWrite(element,vShowOriginalDisplay,v181);v161=hostRead(element,vShowHidden);if(!(v161==null)){v167=!!v161;v168=v167}else{v168=!1}if(v168)hostWrite(v5,"display","none")}}function patchAttr(element,key,value,isSVG,instance,booleanValue){var v7=!!booleanValue;if(booleanValue===void 0){var v13=!!hostCall1(dependencyIsSpecialBooleanAttr,key);var v57=v13,v18,v19,v40,v41,v42,v52,v58,v59}else{v57=v7}if(isSVG){v18=key.startsWith("xlink:");v19=v18}else{v19=!1}if(v19){if(value==null){hostRemoveAttributeNS(element,xlinkNS,key.slice(6))}else{hostSetAttributeNS(element,xlinkNS,key,value)}return}if(value==null){v42=!0}else{if(v57){v40=!dependencyIncludeBooleanAttr(value);v41=v40}else{v41=!1}v42=v41}if(v42){hostRemoveAttribute(element,key)}else{if(v57){v59=""}else{if(dependencyIsSymbol(value)){v52=hostString(value);v58=v52}else{v58=value}v59=v58}hostSetAttribute(element,key,v59)}}function patchDOMProp(element,key,value,parentComponent,attrName){if("innerHTML"==key){var v12=!0,v21,v29,v36,v37,v42,v43,v46,v52,v62,v63,v64,v77,v86,v87,v108,v112,v117,v123,v124,v136,v161,v165,v166,v167,v168,v169,v170,v171,v172,v173,v174,v175,v176,v177,v178}else{v12="textContent"==key}if(v12){if(!(value==null)){if("innerHTML"==key){v21=hostUnsafeToTrustedHTML(value);v169=v21}else{v169=value}hostWrite(element,key,v169)}return}v29=hostString(hostRead(element,"tagName"));if("value"==key){v36="PROGRESS"!=v29;v37=v36}else{v37=!1}if(v37){v42=!v29.includes("-");v43=v42}else{v43=!1}if(v43){v46=hostRead(element,"value");if("OPTION"==v29){v52=hostGetAttribute(element,"value");v171=v52}else{v171=v46}if("OPTION"==v29){if(v171==null){v63=!0}else{v62=!v171;v63=v62}v64=v63}else{v64=!1}if(v64){v170=""}else{v170=v171}if(value==null){if(same(hostRead(element,"type"),"checkbox")){v172="on"}else{v172=""}v173=v172}else{v77=hostString(value);v173=v77}if(!same(v170,v173)){v87=!0}else{v86=!hostHasProperty(element,"_value");v87=v86}if(v87)hostWrite(element,"value",v173);if(value==null)hostRemoveAttribute(element,key);hostWrite(element,"_value",value);return}if(same(value,"")){v108=!0}else{v108=value==null}if(v108){v112=typeof hostRead(element,key);if("boolean"==v112){v117=dependencyIncludeBooleanAttr(value);v167=v117;v176=!1}else{if(value==null){v123="string"==v112;v124=v123}else{v124=!1}if(v124){v166="";v175=!0}else{if("number"==v112){v165=0;v174=!0}else{v165=value;v174=!1}v166=v165;v175=v174}v167=v166;v176=v175}v168=v167;v177=v176}else{v168=value;v177=!1}try{hostWrite(element,key,v168)}catch(v136){if(!v177)hostWarn("Failed setting prop \""+key+"\" on <"+v29.toLowerCase()+">: value "+hostString(v168)+" is invalid.",v136)}if(v177){if(!(attrName==null)){v161=hostString(attrName);v178=v161}else{v178=key}hostRemoveAttribute(element,v178)}}function getNow(){if(0!=cachedNow)return cachedNow;cachedNow=hostDateNow();hostPromiseThen(resolvedPromise,function(){cachedNow=0});return cachedNow}function parseEventName(rawName){var v51=rawName,v53=void 0,v6,v12,v13,v20,v34,v39,v47,v52,v54;for(;!0;){v6=optionModifierPattern.exec(v51);if(v6==null){v13=!0}else{v12=optionModifierEventPattern.test(v51);v13=v12}if(v13)break;if(v53===void 0){v52={}}else{v52=v53}v20=hostString(hostRead(v6,1));hostWrite(v52,v20.toLowerCase(),!0);v34=v51.slice(0,v51.length-v20.length);v51=v34;v53=v52}v39=callStringTransform(dependencyHyphenate,v51.slice(2));if(58==v51.charCodeAt(2)){v47=v51.slice(3);v54=v47}else{v54=v39}return [v54,v53]}function sanitizeEventValue(value,propName){if(dependencyIsFunction(value)){var v7=!0,v6}else{v6=dependencyIsArray(value);v7=v6}if(v7)return value;hostWarn("Wrong type passed as event handler to "+propName+" - did you forget @ or : "+"in front of your prop?\nExpected function or array of functions, received type "+typeof value+".",void 0);return function(){}}function createInvoker(initialValue){var invoker;invoker=function(event){var v5$2=hostRead(event,"_vts");if(v5$2==null){var v12$2=!0,v11,v18,v27,v32,v41,v43,v50,v56,v57,v60,v66,v67,v78}else{v11=!v5$2;v12$2=v11}if(v12$2){hostWrite(event,"_vts",hostDateNow())}else{v18=+v5$2;if(v18<=+hostRead(invoker,"attached"))return}v27=hostRead(invoker,"value");if(dependencyIsArray(v27)){v32=hostRead(event,"stopImmediatePropagation");hostWrite(event,"stopImmediatePropagation",function(){hostCall0(v32.bind(event));hostWrite(event,"_stopped",!0)});v41=hostArraySlice(v27);v43=valueLength(v41);v78=0;for(;v78<v43;v78=v78+1){v50=hostRead(event,"_stopped");if(!(v50==null)){v56=!!v50;v57=v56}else{v57=!1}if(v57)break;v60=hostRead(v41,v78);if(!(v60==null)){v66=!!v60;v67=v66}else{v67=!1}if(v67)hostCall1(v60,event)}}else{hostCall1(v27,event)}};hostWrite(invoker,"value",initialValue);let v10=invoker;hostWrite(v10,"attached",getNow());return invoker}function addEventListener(element,event,handler,options){hostAddEventListener(element,event,handler,options)}function removeEventListener(element,event,handler,options){hostRemoveEventListener(element,event,handler,options)}function patchEvent(element,rawName,previous,next,instance){var v7=hostRead(element,invokerKey);if(v7==null){var v10={};hostWrite(element,invokerKey,v10);var v86=v10,v17,v23,v24,v28,v29,v32,v33,v41,v45,v48,v54,v55,v60,v75,v76}else{v86=v7}v17=hostRead(v86,rawName);if(!(next==null)){v23=!!next;v24=v23}else{v24=!1}if(v24){v28=!(v17==null);v29=v28}else{v29=!1}if(v29){v32=!!v17;v33=v32}else{v33=!1}if(v33){hostWrite(v17,"value",sanitizeEventValue(next,rawName));return}v41=parseEventName(rawName);v45=hostString(hostRead(v41,0));v48=hostRead(v41,1);if(!(next==null)){v54=!!next;v55=v54}else{v55=!1}if(v55){v60=createInvoker(sanitizeEventValue(next,rawName));hostWrite(v86,rawName,v60);hostAddEventListener(element,v45,v60,v48)}else{if(!(v17==null)){v75=!!v17;v76=v75}else{v76=!1}if(v76){hostRemoveEventListener(element,v45,v17,v48);hostWrite(v86,rawName,void 0)}}}function nativeOn(key){if(key.length<3)return !1;var v8=key.charCodeAt(2);if(111==key.charCodeAt(0)){var v20=110==key.charCodeAt(1),v25,v30}else{v20=!1}if(v20){v25=v8>96}else{v25=!1}if(v25){v30=v8<123}else{v30=!1}return v30}function shouldSetAsProp(element,key,value,isSVG){if(isSVG){if("innerHTML"==key){var v12=!0,v19,v20,v23,v24,v34,v39,v44,v49,v56,v57,v69,v70,v78,v79,v88,v96,v97,v101,v102,v106,v107,v108,v114,v115}else{v12="textContent"==key}if(v12)return !0;if(hostHasProperty(element,key)){v19=nativeOn(key);v20=v19}else{v20=!1}if(v20){v23=dependencyIsFunction(value);v24=v23}else{v24=!1}if(v24)return !0;return !1}if("spellcheck"==key){v34=!0}else{v34="draggable"==key}if(v34){v39=!0}else{v39="translate"==key}if(v39){v44=!0}else{v44="autocorrect"==key}if(v44)return !1;v49=hostString(hostRead(element,"tagName"));if("sandbox"==key){v56="IFRAME"==v49;v57=v56}else{v57=!1}if(v57)return !1;if("form"==key)return !1;if("list"==key){v69="INPUT"==v49;v70=v69}else{v70=!1}if(v70)return !1;if("type"==key){v78="TEXTAREA"==v49;v79=v78}else{v79=!1}if(v79)return !1;if("width"==key){v88=!0}else{v88="height"==key}if(v88){if("IMG"==v49){v97=!0}else{v96="VIDEO"==v49;v97=v96}if(v97){v102=!0}else{v101="CANVAS"==v49;v102=v101}if(v102){v107=!0}else{v106="SOURCE"==v49;v107=v106}v108=v107}else{v108=!1}if(v108)return !1;if(nativeOn(key)){v114=dependencyIsString(value);v115=v114}else{v115=!1}if(v115)return !1;return hostHasProperty(element,key)}function shouldSetAsPropForVueCE(element,key){var v7=hostRead(hostRead(element,"_def"),"props");if(v7==null)return !1;var v13=callStringTransform(dependencyCamelize,key);if(dependencyIsArray(v7)){var v17=valueLength(v7);var v56=0,v22,v36,v38,v43,v57;for(;v56<v17;v56=v56+1){v22=dependencyCamelize;if(callStringTransform(v22,hostString(hostRead(v7,v56)))==v13)return !0}return !1}v36=hostObjectKeys(v7);v38=valueLength(v36);v57=0;for(;v57<v38;v57=v57+1){v43=dependencyCamelize;if(callStringTransform(v43,hostString(hostRead(v36,v57)))==v13)return !0}return !1}function patchProp(element,originalKey,previous,next,namespace,parentComponent){if(!(namespace==null)){var v13=same(namespace,"svg");var v14=v13,v59,v83,v88,v89,v101,v107,v113,v114,v125,v126,v134,v135,v136,v137,v138,v169,v170,v171,v172}else{v14=!1}if("class"==originalKey){patchClass(element,next,v14)}else{if("style"==originalKey){patchStyle(element,previous,next)}else{if(dependencyIsOn(originalKey)){if(!dependencyIsModelListener(originalKey))patchEvent(element,originalKey,previous,next,parentComponent)}else{if(originalKey.startsWith(".")){v170=originalKey.slice(1);v172=!0}else{if(originalKey.startsWith("^")){v169=originalKey.slice(1);v171=!1}else{v59=shouldSetAsProp(element,originalKey,next,v14);v169=originalKey;v171=v59}v170=v169;v172=v171}if(v172){patchDOMProp(element,v170,next,parentComponent,void 0);if(!hostString(hostRead(element,"tagName")).includes("-")){if("value"==v170){v83=!0}else{v83="checked"==v170}if(v83){v88=!0}else{v88="selected"==v170}v89=v88}else{v89=!1}if(v89)patchAttr(element,v170,next,v14,parentComponent,"value"!=v170)}else{v101=hostRead(element,"_isVueCE");v107=hostRead(hostRead(element,"_def"),"__asyncLoader");if(!(v101==null)){v113=!!v101;v114=v113}else{v114=!1}if(v114){if(shouldSetAsPropForVueCE(element,v170)){v137=!0}else{if(!(v107==null)){v125=!!v107;v126=v125}else{v126=!1}if(v126){if(upperCasePattern.test(v170)){v135=!0}else{v134=!dependencyIsString(next);v135=v134}v136=v135}else{v136=!1}v137=v136}v138=v137}else{v138=!1}if(v138){patchDOMProp(element,callStringTransform(dependencyCamelize,v170),next,parentComponent,v170)}else{if("true-value"==v170){hostWrite(element,"_trueValue",next)}else{if("false-value"==v170)hostWrite(element,"_falseValue",next)}patchAttr(element,v170,next,v14,parentComponent,void 0)}}}}}}function hasModifier(modifiers,expected){var v3=valueLength(modifiers);var v19=0;for(;v19<v3;v19=v19+1){if(hostString(hostRead(modifiers,v19))==expected)return !0}return !1}function modifierStops(event,modifier,modifiers){if("stop"==modifier)return hostCallMethod0(event,"stopPropagation"),!1;if("prevent"==modifier)return hostCallMethod0(event,"preventDefault"),!1;if("self"==modifier){var v22=hostRead(event,"target");return !same(v22,hostRead(event,"currentTarget"))}if("ctrl"==modifier)return !hostRead(event,"ctrlKey");if("shift"==modifier)return !hostRead(event,"shiftKey");if("alt"==modifier)return !hostRead(event,"altKey");if("meta"==modifier)return !hostRead(event,"metaKey");if("left"==modifier){if(hostHasProperty(event,"button")){var v72=0!=+hostRead(event,"button");var v73=v72,v86,v87,v100,v101,v109,v117,v128,v129,v135}else{v73=!1}return v73}if("middle"==modifier){if(hostHasProperty(event,"button")){v86=1!=+hostRead(event,"button");v87=v86}else{v87=!1}return v87}if("right"==modifier){if(hostHasProperty(event,"button")){v100=2!=+hostRead(event,"button");v101=v100}else{v101=!1}return v101}if("exact"==modifier){v109=["ctrl","shift","alt","meta"];v135=0;for(;v135<v109.length;v135=v135+1){v117=v109[v135]||"";if(hostRead(event,v117+"Key")){v128=!hasModifier(modifiers,v117);v129=v128}else{v129=!1}if(v129)return !0}}return !1}function withModifiers(fn,modifiers){if(fn==null){var v8=!0,v7,v12,v15,v24,v27,v33,v34,v40,v46}else{v7=!fn;v8=v7}if(v8)return fn;v12=hostRead(fn,"_withMods");if(v12==null){v15={};hostWrite(fn,"_withMods",v15);v46=v15}else{v46=v12}v24=hostString(modifiers.join("."));v27=hostRead(v46,v24);if(!(v27==null)){v33=!!v27;v34=v33}else{v34=!1}if(v34)return v27;v40=hostEventRestWrapper(function(event,args){var v6$2=valueLength(modifiers);var v27$2=0;for(;v27$2<v6$2;v27$2=v27$2+1){if(modifierStops(event,hostString(hostRead(modifiers,v27$2)),modifiers))return}return hostCallWithEventArgs(fn,event,args)});hostWrite(v46,v24,v40);return v40}function keyAlias(key){if("esc"==key)return "escape";if("space"==key)return " ";if("up"==key)return "arrow-up";if("left"==key)return "arrow-left";if("right"==key)return "arrow-right";if("down"==key)return "arrow-down";if("delete"==key)return "backspace";return ""}function withKeys(fn,modifiers){var v4=hostRead(fn,"_withKeys");if(v4==null){var v7={};hostWrite(fn,"_withKeys",v7);var v37=v7,v16,v19,v25,v26,v31}else{v37=v4}v16=hostString(modifiers.join("."));v19=hostRead(v37,v16);if(!(v19==null)){v25=!!v19;v26=v25}else{v26=!1}if(v26)return v19;v31=function(event){if(!hostHasProperty(event,"key"))return;var v9=dependencyHyphenate;var v14=callStringTransform(v9,hostString(hostRead(event,"key")));var v16$2=valueLength(modifiers);var v41=0,v24,v32,v33;for(;v41<v16$2;v41=v41+1){v24=hostString(hostRead(modifiers,v41));if(v24==v14){v33=!0}else{v32=keyAlias(v24)==v14;v33=v32}if(v33)return hostCall1(fn,event)}};hostWrite(v37,v16,v31);return v31}function setDisplay(element,value){var v4=hostRead(element,"style");if(value){var v10=hostRead(element,vShowOriginalDisplay);var v21=v10}else{v21="none"}hostWrite(v4,"display",v21);hostWrite(element,vShowHidden,!value)}function modelAssigner(vnode){var v6=hostRead(hostRead(vnode,"props"),"onUpdate:modelValue");if(dependencyIsArray(v6))return function(value){var v4=valueLength(v6);var v18=0;for(;v18<v4;v18=v18+1)hostCall1(hostRead(v6,v18),value)};return v6}function bindingModifiers(binding){var v3=hostRead(binding,"modifiers");if(v3==null)return {};return v3}function castModelValue(value,trim,numeric){if(trim){var v6=hostString(value).trim();var v12=v6,v9,v11}else{v12=value}if(numeric){v9=dependencyLooseToNumber(v12);v11=v9}else{v11=v12}return v11}function elementModelValue(element){if(hostHasProperty(element,"_value"))return hostRead(element,"_value");return hostRead(element,"value")}function checkboxValue(element,checked){if(checked){var v12="_trueValue"}else{v12="_falseValue"}if(hostHasProperty(element,v12))return hostRead(element,v12);return checked}function setChecked(_self,element,binding,vnode){var v6=hostRead(binding,"value");hostWrite(element,"_modelValue",v6);var v17=hostRead(hostRead(vnode,"props"),"value");if(dependencyIsArray(v6)){var v25=dependencyLooseIndexOf(v6,v17)>-1;var v54=v25,v30,v41,v53}else{if(dependencyIsSet(v6)){v30=hostSetHas(v6,v17);v53=v30}else{if(same(v6,hostRead(binding,"oldValue")))return;v41=dependencyLooseEqual(v6,checkboxValue(element,!0));v53=v41}v54=v53}if(!!hostRead(element,"checked")!=v54)hostWrite(element,"checked",v54)}function sameSelectValue(value,assigned,multiple){if(!multiple)return dependencyLooseEqual(value,assigned);if(dependencyIsArray(value))return dependencyLooseEqual(value,assigned);if(dependencyIsSet(value)){if(hostSetSize(value)!=valueLength(assigned))return !1;var v22=valueLength(assigned);var v39=0;for(;v39<v22;v39=v39+1){if(!hostSetHas(value,hostRead(assigned,v39)))return !1}return !0}return !1}function setSelected(element,value){var v5=!!hostRead(element,"multiple");var v7=dependencyIsArray(value);if(v5){var v11=!v7;var v12=v11,v16,v17,v29,v31,v38,v40,v45,v52,v53,v56,v77,v80,v112,v113,v119,v120,v121,v122,v123}else{v12=!1}if(v12){v16=!dependencyIsSet(value);v17=v16}else{v17=!1}if(v17){hostWarn("<select multiple v-model> expects an Array or Set value for its binding, but got "+hostRawType(value)+".",void 0);return}v29=hostRead(element,"options");v31=valueLength(v29);v119=0;for(;v119<v31;v119=v119+1){v38=hostRead(v29,v119);v40=elementModelValue(v38);if(v5){if(v7){v45=typeof v40;if("string"==v45){v53=!0}else{v52="number"==v45;v53=v52}if(v53){v56=valueLength(value);v123=0;for(;;){if(v123>=v56){v120=!1;break}if(hostString(hostRead(value,v123))==hostString(v40)){v120=!0;break}v123=v123+1}v121=v120}else{v77=dependencyLooseIndexOf(value,v40)>-1;v121=v77}v122=v121}else{v80=hostSetHas(value,v40);v122=v80}hostWrite(v38,"selected",v122)}else{if(dependencyLooseEqual(v40,value)){if((+hostRead(element,"selectedIndex")|0)!=v119)hostWrite(element,"selectedIndex",v119);return}}}if(!v5){v112=(+hostRead(element,"selectedIndex")|0)!=-1;v113=v112}else{v113=!1}if(v113)hostWrite(element,"selectedIndex",-1)}function resolveDynamicModel(tagName,type){if("SELECT"==tagName)return vModelSelect;if("TEXTAREA"==tagName)return vModelText;if(same(type,"checkbox"))return vModelCheckbox;if(same(type,"radio"))return vModelRadio;return vModelText}function callModelHook(element,binding,vnode,previousVNode,hook){var v7=hostRead(vnode,"props");var v11=hostString(hostRead(element,"tagName"));var v18=hostRead(resolveDynamicModel(v11,hostRead(v7,"type")),hook);if(!(v18==null)){var v24=!!v18;var v25=v24}else{v25=!1}if(v25)hostCall4(v18,element,binding,vnode,previousVNode)}function initVShowForSSR(){hostWrite(vShow,"getSSRProps",function(binding){if(!hostRead(binding,"value"))return {style:{display:"none"}}})}function initVModelForSSR(){hostWrite(vModelText,"getSSRProps",function(binding){return {value:hostRead(binding,"value")}});hostWrite(vModelRadio,"getSSRProps",function(binding,vnode){var v5$2=hostRead(vnode,"props");if(!(v5$2==null)){var v12$2=hostRead(v5$2,"value");var v16=dependencyLooseEqual(v12$2,hostRead(binding,"value"));var v17$2=v16}else{v17$2=!1}if(v17$2)return {checked:!0}});hostWrite(vModelCheckbox,"getSSRProps",function(binding,vnode){var v5$2=hostRead(binding,"value");var v8$2=hostRead(vnode,"props");if(dependencyIsArray(v5$2)){if(!(v8$2==null)){var v22=dependencyLooseIndexOf(v5$2,hostRead(v8$2,"value"))>-1;var v23=v22,v37,v38}else{v23=!1}if(v23)return {checked:!0}}else{if(dependencyIsSet(v5$2)){if(!(v8$2==null)){v37=hostSetHas(v5$2,hostRead(v8$2,"value"));v38=v37}else{v38=!1}if(v38)return {checked:!0}}else{if(v5$2)return {checked:!0}}}});hostWrite(vModelDynamic,"getSSRProps",function(binding,vnode){var v5$2=hostRead(vnode,"type");if("string"!=typeof v5$2)return;var v13$2=hostRead(vnode,"props");var v16=hostString(v5$2).toUpperCase();var v23=hostRead(resolveDynamicModel(v16,hostRead(v13$2,"type")),"getSSRProps");if(!(v23==null)){var v29=!!v23;var v30=v29}else{v30=!1}if(v30)return hostCall2(v23,binding,vnode)})}function initDirectivesForSSR(){if(ssrDirectivesInitialized)return;ssrDirectivesInitialized=!0;initVModelForSSR();initVShowForSSR()}var vtcKey=hostCreateSymbol("_vtc");var vShowOriginalDisplay=hostCreateSymbol("_vod");var vShowHidden=hostCreateSymbol("_vsh");var CSS_VAR_TEXT=hostCreateSymbol("CSS_VAR_TEXT");var nodeOps={insert:function(child,parent,anchor){hostInsert(child,parent,anchor)},remove:function(child){hostRemove(child)},createElement:function(tagValue,namespace,isValue,props){var v5=hostString(tagValue);var v9=hostCreateElement(v5,namespace,isValue);if("select"==v5){var v17=!(props==null),v23$2,v24$2}else{v17=!1}if(v17){v23$2=!(hostRead(props,"multiple")==null);v24$2=v23$2}else{v24$2=!1}if(v24$2)hostSetAttribute(v9,"multiple",hostRead(props,"multiple"));return v9},createText:function(text){return hostCreateText(hostString(text))},createComment:function(text){return hostCreateComment(hostString(text))},setText:function(node,text){hostSetNodeValue(node,hostString(text))},setElementText:function(element,text){hostSetTextContent(element,hostString(text))},parentNode:function(node){return hostParentNode(node)},nextSibling:function(node){return hostNextSibling(node)},querySelector:function(selector){return hostQuerySelector(hostString(selector))},setScopeId:function(element,id){hostSetAttribute(element,hostString(id),"")},insertStaticContent:function(content,parent,anchor,namespace,start,end){var v7=hostLastChild(parent);if(!(anchor==null)){var v12$2=hostPreviousSibling(anchor);var v111$2=v12$2,v24$2,v25,v26$2,v37,v45$2,v47$2,v55$2,v63,v67,v74$2,v75,v77$2,v94,v99$2,v101$2,v106,v110$2,v112,v113$2,v114$2,v115$2,v116}else{v111$2=v7}if(!(start==null)){if(same(start,end)){v25=!0}else{v24$2=!(hostNextSibling(start)==null);v25=v24$2}v26$2=v25}else{v26$2=!1}if(v26$2){v110$2=start;while(!0){hostInsert(hostCloneNode(v110$2),parent,anchor);if(same(v110$2,end))break;v37=hostNextSibling(v110$2);if(v37==null)break;v110$2=v37}}else{if(!(namespace==null)){v45$2=hostString(namespace);v112=v45$2}else{v112=""}v47$2=hostString(content);if("svg"==v112){v55$2="<svg>"+v47$2+"</svg>";v114$2=v55$2}else{if("mathml"==v112){v63="<math>"+v47$2+"</math>";v113$2=v63}else{v113$2=v47$2}v114$2=v113$2}hostSetTemplateHTML(hostUnsafeToTrustedHTML(v114$2));v67=hostTemplateContent();if("svg"==v112){v75=!0}else{v74$2="mathml"==v112;v75=v74$2}if(v75){v77$2=hostFirstChild(v67);while(!(hostFirstChild(v77$2)==null))hostAppendChild(v67,hostFirstChild(v77$2));hostRemoveChild(v67,v77$2)}hostInsert(v67,parent,anchor)}v94=hostFirstChild(parent);if(!(v111$2==null)){v99$2=hostNextSibling(v111$2);v115$2=v99$2}else{v115$2=v94}v101$2=hostLastChild(parent);if(!(anchor==null)){v106=hostPreviousSibling(anchor);v116=v106}else{v116=v101$2}return [v115$2,v116]}};var displayPattern=new RegExp("(?:^|;)\\s*display\\s*:");var semicolonPattern=new RegExp("[^\\\\];\\s*$");var importantPattern=new RegExp("\\s*!important$");var stylePrefixes=["Webkit","Moz","ms"];var stylePrefixCache={};var optionModifierPattern=new RegExp("(Once|Passive|Capture)$");var optionModifierEventPattern=new RegExp("^on:?(?:Once|Passive|Capture)$");var invokerKey=hostCreateSymbol("_vei");var cachedNow=0;var resolvedPromise=hostResolvedPromise();var upperCasePattern=new RegExp("[A-Z]");var vShow={name:"show",beforeMount:function(element,binding,vnode){var v9=hostRead(hostRead(element,"style"),"display");if(same(v9,"none")){var v45$2="",v21$2,v24$2,v30$2,v31,v34,v35$2}else{v45$2=v9}hostWrite(element,vShowOriginalDisplay,v45$2);v21$2=hostRead(binding,"value");v24$2=hostRead(vnode,"transition");if(!(v24$2==null)){v30$2=!!v24$2;v31=v30$2}else{v31=!1}if(v31){v34=!!v21$2;v35$2=v34}else{v35$2=!1}if(v35$2){hostCall1(hostRead(v24$2,"beforeEnter"),element)}else{setDisplay(element,v21$2)}},mounted:function(element,binding,vnode){var v6$2=hostRead(binding,"value");var v9=hostRead(vnode,"transition");if(!(v9==null)){var v15$2=!!v9;var v16$2=v15$2,v19,v20$2}else{v16$2=!1}if(v16$2){v19=!!v6$2;v20$2=v19}else{v20$2=!1}if(v20$2)hostCall1(hostRead(v9,"enter"),element)},updated:function(element,binding,vnode){var v6$2=hostRead(binding,"value");if(!v6$2==!hostRead(binding,"oldValue"))return;var v20$2=hostRead(vnode,"transition");if(!(v20$2==null)){var v26$2=!!v20$2;var v27$2=v26$2}else{v27$2=!1}if(v27$2){if(v6$2){hostCall1(hostRead(v20$2,"beforeEnter"),element);setDisplay(element,!0);hostCall1(hostRead(v20$2,"enter"),element)}else{hostCall2(hostRead(v20$2,"leave"),element,function(){setDisplay(element,!1)})}}else{setDisplay(element,v6$2)}},beforeUnmount:function(element,binding){setDisplay(element,hostRead(binding,"value"))}};var assignKey=hostCreateSymbol("_assign");var initialValueKey=hostCreateSymbol("_initialValue");var textLineBreaks=new RegExp("[\\r\\n]","g");var textareaLineBreaks=new RegExp("\\r\\n?","g");var leadingZero=new RegExp("^0\\d");var vModelText={created:function(element,binding,vnode){var v5=hostParentNode(element);var v9=hostString(hostRead(element,"type"));if(!(v5==null)){if("text"==v9){var v21$2=initialValueKey;hostWrite(element,v21$2,hostString(hostRead(element,"defaultValue")).replace(textLineBreaks,""))}else{if("textarea"==v9){var v36$2=initialValueKey;hostWrite(element,v36$2,hostString(hostRead(element,"defaultValue")).replace(textareaLineBreaks,"\n"))}}}hostWrite(element,assignKey,modelAssigner(vnode));var v49$2=bindingModifiers(binding);var v53=!!hostRead(v49$2,"lazy");var v57=!!hostRead(v49$2,"trim");var v61=!!hostRead(v49$2,"number");var v64$2=hostRead(vnode,"props");if(v61){var v72=!0,v71$2,v77$2,v89,v120$2}else{v71$2=same(hostRead(v64$2,"type"),"number");v72=v71$2}v77$2=function(event){if(hostRead(hostRead(event,"target"),"composing"))return;var v15$3=hostRead(element,assignKey);hostCall1(v15$3,castModelValue(hostRead(element,"value"),v57,v72))};if(v53){v120$2="change"}else{v120$2="input"}hostAddEventListener(element,v120$2,v77$2,void 0);if(v57){v89=!0}else{v89=v72}if(v89)hostAddEventListener(element,"change",function(){hostWrite(element,"value",castModelValue(hostRead(element,"value"),v57,v72))},void 0);if(!v53){hostAddEventListener(element,"compositionstart",function(event){hostWrite(hostRead(event,"target"),"composing",!0)},void 0);hostAddEventListener(element,"compositionend",function(event){var v4$2=hostRead(event,"target");var v7=hostRead(v4$2,"composing");if(!(v7==null)){var v13$2=!!v7;var v14=v13$2,v29$2}else{v14=!1}if(v14){hostWrite(v4$2,"composing",!1);v29$2=new (hostRead(hostRead(hostRead(v4$2,"ownerDocument"),"defaultView"),"Event"))("input");hostCallMethod1(v4$2,"dispatchEvent",v29$2)}},void 0);hostAddEventListener(element,"change",function(event){var v4$2=hostRead(event,"target");var v7=hostRead(v4$2,"composing");if(!(v7==null)){var v13$2=!!v7;var v14=v13$2,v29$2}else{v14=!1}if(v14){hostWrite(v4$2,"composing",!1);v29$2=new (hostRead(hostRead(hostRead(v4$2,"ownerDocument"),"defaultView"),"Event"))("input");hostCallMethod1(v4$2,"dispatchEvent",v29$2)}},void 0)}},mounted:function(element,binding){var v5=hostRead(binding,"value");if(v5==null){var v64$2="",v12$2,v19,v21$2,v32$2,v33$2,v34,v41$2,v42$2,v45$2,v48,v52}else{v64$2=v5}v12$2=hostRead(element,initialValueKey);hostDelete(element,initialValueKey);v19=hostString(hostRead(element,"type"));v21$2=bindingModifiers(binding);if(!(v12$2===void 0)){if("text"==v19){v33$2=!0}else{v32$2="textarea"==v19;v33$2=v32$2}v34=v33$2}else{v34=!1}if(v34){v41$2=!same(hostRead(element,"value"),v12$2);v42$2=v41$2}else{v42$2=!1}if(v42$2){v45$2=hostRead(element,assignKey);v48=hostRead(element,"value");v52=!!hostRead(v21$2,"trim");hostCall1(v45$2,castModelValue(v48,v52,!!hostRead(v21$2,"number")))}else{hostWrite(element,"value",v64$2)}},beforeUpdate:function(element,binding,vnode){hostWrite(element,assignKey,modelAssigner(vnode));if(hostRead(element,"composing"))return;var v15$2=bindingModifiers(binding);var v19=!!hostRead(v15$2,"lazy");var v23$2=!!hostRead(v15$2,"trim");var v27$2=!!hostRead(v15$2,"number");var v31=hostString(hostRead(element,"value"));if(v27$2){var v40=!0,v39$2,v45$2,v46,v48,v51$2,v68,v69,v76$2,v77$2,v85$2,v86,v93$2,v94}else{v39$2=same(hostRead(element,"type"),"number");v40=v39$2}if(v40){v45$2=!leadingZero.test(v31);v46=v45$2}else{v46=!1}if(v46){v48=dependencyLooseToNumber(v31);v93$2=v48}else{v93$2=v31}v51$2=hostRead(binding,"value");if(v51$2==null){v94=""}else{v94=v51$2}if(same(v93$2,v94))return;if(hostActiveElementIs(element)){v68=!same(hostRead(element,"type"),"range");v69=v68}else{v69=!1}if(v69){if(v19){v76$2=same(v51$2,hostRead(binding,"oldValue"));v77$2=v76$2}else{v77$2=!1}if(v77$2)return;if(v23$2){v85$2=v31.trim()==hostString(v94);v86=v85$2}else{v86=!1}if(v86)return}hostWrite(element,"value",v94)}};var vModelCheckbox={deep:!0,created:function(element,_binding,vnode){hostWrite(element,assignKey,modelAssigner(vnode));hostAddEventListener(element,"change",function(){var v4$2=hostRead(element,"_modelValue");var v6$2=elementModelValue(element);var v10$2=!!hostRead(element,"checked");var v13$3=hostRead(element,assignKey);if(dependencyIsArray(v4$2)){var v18$2=dependencyLooseIndexOf(v4$2,v6$2);var v22=v18$2!=-1;if(v10$2){var v26$2=!v22;var v27$2=v26$2,v29$2,v42$2,v44$2,v55$2}else{v27$2=!1}if(v27$2){v29$2=hostArraySlice(v4$2);hostWrite(v29$2,valueLength(v29$2),v6$2);hostCall1(v13$3,v29$2)}else{if(!v10$2){v42$2=v22}else{v42$2=!1}if(v42$2){v44$2=hostArraySlice(v4$2);hostArraySplice(v44$2,v18$2,1);hostCall1(v13$3,v44$2)}}}else{if(dependencyIsSet(v4$2)){v55$2=hostCreateSet(v4$2);if(v10$2){hostSetAdd(v55$2,v6$2)}else{hostSetDelete(v55$2,v6$2)}hostCall1(v13$3,v55$2)}else{hostCall1(v13$3,checkboxValue(element,v10$2))}}},void 0)},mounted:$jsMethod3(setChecked),beforeUpdate:function(element,binding,vnode){hostWrite(element,assignKey,modelAssigner(vnode));return setChecked(this,element,binding,vnode)}};var vModelRadio={created:function(element,binding,vnode){let v8$2=hostRead(binding,"value");hostWrite(element,"checked",dependencyLooseEqual(v8$2,hostRead(hostRead(vnode,"props"),"value")));hostWrite(element,assignKey,modelAssigner(vnode));hostAddEventListener(element,"change",function(){hostCall1(hostRead(element,assignKey),elementModelValue(element))},void 0)},beforeUpdate:function(element,binding,vnode){hostWrite(element,assignKey,modelAssigner(vnode));var v11=hostRead(binding,"value");if(!same(v11,hostRead(binding,"oldValue"))){var v21$2=hostRead(binding,"value");hostWrite(element,"checked",dependencyLooseEqual(v21$2,hostRead(hostRead(vnode,"props"),"value")))}}};var vModelSelect={deep:!0,created:function(element,binding,vnode){hostWrite(element,"_modelValue",hostRead(binding,"value"));let v15$2=!!hostRead(bindingModifiers(binding),"number");hostAddEventListener(element,"change",function(){var element$2=element;var v3=[];var v6$2=hostRead(element$2,"options");var v8$3=valueLength(v6$2);var v81$2=0,v15$3,v21$3,v24$2,v36$2,v45$2,v46,v53,v58$2,v59,v61,v75,v76$2,v82,v83$2,v84,v85$2,v86;for(;v81$2<v8$3;v81$2=v81$2+1){v15$3=hostRead(v6$2,v81$2);if(hostRead(v15$3,"selected")){v21$3=elementModelValue(v15$3);if(v15$2){v24$2=dependencyLooseToNumber(v21$3);v82=v24$2}else{v82=v21$3}hostWrite(v3,valueLength(v3),v82)}}v36$2=!!hostRead(element$2,"multiple");if(v36$2){if(dependencyIsSet(hostRead(element$2,"_modelValue"))){v45$2=hostCreateSet(v3);v85$2=v45$2}else{v85$2=v3}v84=v85$2}else{v46=void 0;if(0!=valueLength(v3)){v53=hostRead(v3,0);v83$2=v53}else{v83$2=v46}v84=v83$2}if(v36$2){v58$2=dependencyIsArray(v84);v59=v58$2}else{v59=!1}if(v59){v61=hostArraySlice(v3);v86=v61}else{v86=v84}var pending=[v36$2,v86];hostWrite(element$2,"_pendingValue",pending);try{hostCall1(hostRead(element$2,assignKey),v84)}finally{v75=element$2;v76$2=pending;hostCall1(dependencyNextTick,function(){if(same(hostRead(v75,"_pendingValue"),v76$2))hostWrite(v75,"_pendingValue",void 0)})}},void 0);hostWrite(element,assignKey,modelAssigner(vnode))},mounted:function(element,binding){setSelected(element,hostRead(binding,"value"))},beforeUpdate:function(element,binding,vnode){hostWrite(element,"_modelValue",hostRead(binding,"value"));hostWrite(element,assignKey,modelAssigner(vnode))},updated:function(element,binding){var v5=hostRead(element,"_pendingValue");hostWrite(element,"_pendingValue",void 0);var v13$2=!!hostRead(element,"multiple");if(v5==null){var v23$2=!0,v22,v27$2,v33$2,v34}else{v22=!same(hostRead(v5,0),v13$2);v23$2=v22}if(v23$2){v34=!0}else{v27$2=hostRead(binding,"value");v33$2=!sameSelectValue(v27$2,hostRead(v5,1),v13$2);v34=v33$2}if(v34)setSelected(element,hostRead(binding,"value"))}};var vModelDynamic={created:function(element,binding,vnode){callModelHook(element,binding,vnode,null,"created")},mounted:function(element,binding,vnode){callModelHook(element,binding,vnode,null,"mounted")},beforeUpdate:function(element,binding,vnode,previousVNode){callModelHook(element,binding,vnode,previousVNode,"beforeUpdate")},updated:function(element,binding,vnode,previousVNode){callModelHook(element,binding,vnode,previousVNode,"updated")}};var ssrDirectivesInitialized=!1;Object.defineProperties(unsafeToTrustedHTML,{name:{configurable:true,value:"unsafeToTrustedHTML"},length:{configurable:true,value:1}});Object.defineProperties(nodeOps.insert,{name:{configurable:true,value:"insert"},length:{configurable:true,value:3}});Object.defineProperties(nodeOps.remove,{name:{configurable:true,value:"remove"},length:{configurable:true,value:1}});Object.defineProperties(nodeOps.createElement,{name:{configurable:true,value:"createElement"},length:{configurable:true,value:4}});Object.defineProperties(nodeOps.createText,{name:{configurable:true,value:"createText"},length:{configurable:true,value:1}});Object.defineProperties(nodeOps.createComment,{name:{configurable:true,value:"createComment"},length:{configurable:true,value:1}});Object.defineProperties(nodeOps.setText,{name:{configurable:true,value:"setText"},length:{configurable:true,value:2}});Object.defineProperties(nodeOps.setElementText,{name:{configurable:true,value:"setElementText"},length:{configurable:true,value:2}});Object.defineProperties(nodeOps.parentNode,{name:{configurable:true,value:"parentNode"},length:{configurable:true,value:1}});Object.defineProperties(nodeOps.nextSibling,{name:{configurable:true,value:"nextSibling"},length:{configurable:true,value:1}});Object.defineProperties(nodeOps.querySelector,{name:{configurable:true,value:"querySelector"},length:{configurable:true,value:1}});Object.defineProperties(nodeOps.setScopeId,{name:{configurable:true,value:"setScopeId"},length:{configurable:true,value:2}});Object.defineProperties(nodeOps.insertStaticContent,{name:{configurable:true,value:"insertStaticContent"},length:{configurable:true,value:6}});Object.defineProperties(patchClass,{name:{configurable:true,value:"patchClass"},length:{configurable:true,value:3}});Object.defineProperties(patchStyle,{name:{configurable:true,value:"patchStyle"},length:{configurable:true,value:3}});Object.defineProperties(patchAttr,{name:{configurable:true,value:"patchAttr"},length:{configurable:true,value:4}});Object.defineProperties(patchDOMProp,{name:{configurable:true,value:"patchDOMProp"},length:{configurable:true,value:4}});Object.defineProperties(addEventListener,{name:{configurable:true,value:"addEventListener"},length:{configurable:true,value:3}});Object.defineProperties(removeEventListener,{name:{configurable:true,value:"removeEventListener"},length:{configurable:true,value:3}});Object.defineProperties(patchEvent,{name:{configurable:true,value:"patchEvent"},length:{configurable:true,value:4}});Object.defineProperties(patchProp,{name:{configurable:true,value:"patchProp"},length:{configurable:true,value:6}});Object.defineProperties(withModifiers,{name:{configurable:true,value:"withModifiers"},length:{configurable:true,value:2}});Object.defineProperties(withKeys,{name:{configurable:true,value:"withKeys"},length:{configurable:true,value:2}});Object.defineProperties(vShow.beforeMount,{name:{configurable:true,value:"beforeMount"},length:{configurable:true,value:3}});Object.defineProperties(vShow.mounted,{name:{configurable:true,value:"mounted"},length:{configurable:true,value:3}});Object.defineProperties(vShow.updated,{name:{configurable:true,value:"updated"},length:{configurable:true,value:3}});Object.defineProperties(vShow.beforeUnmount,{name:{configurable:true,value:"beforeUnmount"},length:{configurable:true,value:2}});Object.defineProperties(vModelText.created,{name:{configurable:true,value:"created"},length:{configurable:true,value:3}});Object.defineProperties(vModelText.mounted,{name:{configurable:true,value:"mounted"},length:{configurable:true,value:2}});Object.defineProperties(vModelText.beforeUpdate,{name:{configurable:true,value:"beforeUpdate"},length:{configurable:true,value:3}});Object.defineProperties(vModelCheckbox.created,{name:{configurable:true,value:"created"},length:{configurable:true,value:3}});Object.defineProperties(vModelCheckbox.mounted,{name:{configurable:true,value:"setChecked"},length:{configurable:true,value:3}});Object.defineProperties(vModelCheckbox.beforeUpdate,{name:{configurable:true,value:"beforeUpdate"},length:{configurable:true,value:3}});Object.defineProperties(vModelRadio.created,{name:{configurable:true,value:"created"},length:{configurable:true,value:3}});Object.defineProperties(vModelRadio.beforeUpdate,{name:{configurable:true,value:"beforeUpdate"},length:{configurable:true,value:3}});Object.defineProperties(vModelSelect.created,{name:{configurable:true,value:"created"},length:{configurable:true,value:3}});Object.defineProperties(vModelSelect.mounted,{name:{configurable:true,value:"mounted"},length:{configurable:true,value:2}});Object.defineProperties(vModelSelect.beforeUpdate,{name:{configurable:true,value:"beforeUpdate"},length:{configurable:true,value:3}});Object.defineProperties(vModelSelect.updated,{name:{configurable:true,value:"updated"},length:{configurable:true,value:2}});Object.defineProperties(vModelDynamic.created,{name:{configurable:true,value:"created"},length:{configurable:true,value:3}});Object.defineProperties(vModelDynamic.mounted,{name:{configurable:true,value:"mounted"},length:{configurable:true,value:3}});Object.defineProperties(vModelDynamic.beforeUpdate,{name:{configurable:true,value:"beforeUpdate"},length:{configurable:true,value:4}});Object.defineProperties(vModelDynamic.updated,{name:{configurable:true,value:"updated"},length:{configurable:true,value:4}});Object.defineProperties(initVShowForSSR,{name:{configurable:true,value:"initVShowForSSR"},length:{configurable:true,value:0}});Object.defineProperties(initVModelForSSR,{name:{configurable:true,value:"initVModelForSSR"},length:{configurable:true,value:0}});Object.defineProperties(initDirectivesForSSR,{name:{configurable:true,value:"initDirectivesForSSR"},length:{configurable:true,value:0}});export{nodeOps,patchProp,withModifiers,withKeys,vShow,vModelText,vModelCheckbox,vModelRadio,vModelSelect,vModelDynamic,initDirectivesForSSR};export*from"./runtime-core.js";
