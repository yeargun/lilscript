// Generated from the mirrored src/reactivity/*.lil graph and its ECMAScript host adapter.
// The reactivity kernel owns policy. This file only exposes ECMAScript
// operations that LilScript cannot currently express with a typed ABI.

function hostCreateObject() {
  return {};
}

function hostCreateNullObject() {
  return Object.create(null);
}

function hostCreateArray(length) {
  return new Array(length);
}

function hostCreateWeakMap() {
  return new WeakMap();
}

function hostCreateWeakSet() {
  return new WeakSet();
}

function hostCreateMap() {
  return new Map();
}

function hostCreateFinalizationRegistry(callback) {
  return new FinalizationRegistry(callback);
}

function hostFinalizationRegister(registry, target, heldValue) {
  registry.register(target, heldValue);
}

function hostCreateProxy(target, handler) {
  return new Proxy(target, handler);
}

function hostCreateClass0(name, initialize) {
  return {
    [name]: class {
      constructor() {
        Reflect.apply(initialize, this, arguments);
      }
    },
  }[name];
}

function hostCreateClass1(name, initialize) {
  return {
    [name]: class {
      constructor(value) {
        Reflect.apply(initialize, this, [value]);
      }
    },
  }[name];
}

function hostCreateEffectScopeClass(
  name,
  initialize,
  active,
  pause,
  resume,
  run,
  on,
  off,
  stop,
) {
  const Scope = {
    [name]: class {
      constructor() {
        Reflect.apply(initialize, this, arguments);
      }
    },
  }[name];
  Object.defineProperty(Scope.prototype, "active", {
    configurable: true,
    enumerable: false,
    get: active,
  });
  for (const [method, value] of Object.entries({ pause, resume, run, on, off, stop })) {
    Object.defineProperty(value, "name", { configurable: true, value: method });
    Object.defineProperty(Scope.prototype, method, {
      configurable: true,
      enumerable: false,
      writable: true,
      value,
    });
  }
  return Scope;
}

function hostFunction2Rest(callback) {
  return function (first, second) {
    return Reflect.apply(callback, this, [arguments]);
  };
}

function hostRead(value, key) {
  return value == null ? undefined : value[key];
}

function hostReflectGet(target, key, receiver) {
  return Reflect.get(target, key, receiver);
}

function hostReflectSet(target, key, value, receiver) {
  return Reflect.set(target, key, value, receiver);
}

function hostReflectDelete(target, key) {
  return Reflect.deleteProperty(target, key);
}

function hostReflectHas(target, key) {
  return Reflect.has(target, key);
}

function hostReflectOwnKeys(target) {
  return Reflect.ownKeys(target);
}

function hostDefineAccessor(object, key, get, set) {
  if (typeof get === "function") {
    Object.defineProperty(get, "name", { configurable: true, value: `get ${String(key)}` });
  }
  if (typeof set === "function") {
    Object.defineProperty(set, "name", { configurable: true, value: `set ${String(key)}` });
  }
  Object.defineProperty(object, key, {
    configurable: true,
    enumerable: false,
    get,
    set,
  });
}

function hostDefineHidden(object, key, value) {
  Object.defineProperty(object, key, {
    configurable: true,
    enumerable: false,
    writable: true,
    value,
  });
}

function hostDefineMethod(object, key, value) {
  Object.defineProperty(value, "name", { configurable: true, value: String(key) });
  Object.defineProperty(object, key, {
    configurable: true,
    enumerable: false,
    writable: true,
    value,
  });
}

function hostObjectIs(left, right) {
  return Object.is(left, right);
}

function hostInstanceOf(value, constructor) {
  return value instanceof constructor;
}

function hostIsExtensible(value) {
  return Object.isExtensible(value);
}

function hostHasOwn(target, key) {
  return Object.prototype.hasOwnProperty.call(target, key);
}

function hostRawType(value) {
  return Object.prototype.toString.call(value).slice(8, -1);
}

function hostSamePrototype(left, right) {
  return Object.getPrototypeOf(left) === Object.getPrototypeOf(right);
}

function hostParseInt(value) {
  return parseInt(value, 10);
}

function hostBuiltInSymbols() {
  return Object.getOwnPropertyNames(Symbol)
    .filter(name => name !== "arguments" && name !== "caller")
    .map(name => Symbol[name])
    .filter(value => typeof value === "symbol");
}

function hostSymbol(description) {
  return Symbol(description);
}

function hostIteratorSymbol() {
  return Symbol.iterator;
}

function hostToNumber(value) {
  return Number(value);
}

function hostString(value) {
  return String(value);
}

function hostArrayApply(method, receiver, args) {
  return Reflect.apply(Array.prototype[method], receiver, args);
}

function hostArrayPrototypeMethod(method) {
  return Array.prototype[method];
}

function hostPrototypeMethodApply(receiver, method, args) {
  return Reflect.apply(Object.getPrototypeOf(receiver)[method], receiver, args);
}

function hostCollectionSize(receiver) {
  return Reflect.get(Object.getPrototypeOf(receiver), "size", receiver);
}

function hostCloneCollection(collection) {
  return collection instanceof Map ? new Map(collection) : new Set(collection);
}

function hostInfinity() {
  return Infinity;
}

function hostIteratorFacade(inner, next) {
  const iterator = Object.create(inner);
  Object.defineProperty(iterator, "next", {
    configurable: true,
    enumerable: true,
    writable: true,
    value: next,
  });
  Object.defineProperty(iterator, Symbol.iterator, {
    configurable: true,
    value() {
      return this;
    },
  });
  return iterator;
}

function hostLength(value) {
  return value.length;
}

function hostEnumerableSymbols(value) {
  return Object.getOwnPropertySymbols(value).filter(symbol =>
    Object.prototype.propertyIsEnumerable.call(value, symbol),
  );
}

function hostWarn(message, args) {
  console.warn(`[Vue warn] ${message}`, ...Array.prototype.slice.call(args, 1));
}

let Jb="\" failed: target is readonly.",Kb="[object Map]",Lb="Set operation on key \"";function xa(l){var b=Object.create(null),a=l.split(",");for(l=0;l<a.length;l++)b[a[l]||""]=1}function ya(){if("undefined"==typeof process)var b,a=!0;else{b=process.env,a=b.NODE_ENV,a="production"!==a}return a}function _a(){var d={};if(ya()){Object.freeze(d);return}}function $a(){var d=[];if(ya()){Object.freeze(d);return}}function ab(a,l){l=+a.indexOf(l),l>-1&&a.splice(l,1)}function U(d,b){return!!Object.prototype.hasOwnProperty.call(d,b)}function A(d){return Oa.call(d)+""}function L(d){return null!==d&&"object"==typeof d}function _(b){if("string"!=typeof b)return!1;b+="";return"NaN"==b||b.startsWith("-")?!1:parseInt(b,10)+""==b}function D(d,a){return!0!==Object.is(d,a)}function w(a){hostWarn(a,arguments)}function qb(a){return new Pa(a)}function bb(){return t}function rb(m,a=!1){t!==void 0?hostRead(t,"cleanups").push(m):(m=!a,m&&w("onScopeDispose() is called when there is no active effect scope to be associated with."))}function ha(a,b=!1){var h=hostRead(a,"dep"),c=hostRead(a,"prevSub"),d=hostRead(a,"nextSub");c===void 0||(c.nextSub=d,a.prevSub=void 0),d===void 0||(d.prevSub=c,a.nextSub=void 0);var e=hostRead(h,"subsHead")===a;e&&(h.subsHead=d);if(hostRead(h,"subs")===a){h.subs=c,d=hostRead(h,"computed");if(c===void 0&&d!==void 0){d.flags=+hostRead(d,"flags")&-5,a=hostRead(d,"deps");while(a!==void 0)ha(a,!0),a=hostRead(a,"nextDep")}}b||(a=(+hostRead(h,"sc")|0)-1|0,h.sc=a,c=hostRead(h,"map"),0==a&&c!==void 0&&c.delete(hostRead(h,"key")))}function $(a){var b=hostRead(a,"deps");while(b!==void 0){var h=hostRead(b,"dep"),c=+hostRead(h,"version")|0;if(c!=(+hostRead(b,"version")|0))return!0;c=hostRead(h,"computed");if(c!==void 0&&(Ba(c),h=+hostRead(h,"version")|0,h!=(+hostRead(b,"version")|0)))return!0;b=hostRead(b,"nextDep")}return!!hostRead(a,"_dirty")}function za(a){var b=hostRead(a,"cleanup");a.cleanup=void 0;if(b!==void 0){var l=q;q=void 0;try{b()}finally{q=l}}}var V,Ba;(function(){function a(c){c=hostRead(c,"deps");while(c!==void 0){c.version=-1;var h=hostRead(c,"dep");c.prevActiveLink=hostRead(h,"activeLink"),h.activeLink=c,c=hostRead(c,"nextDep")}}function b(c){var i,h,g,j=void 0,e=hostRead(c,"depsTail"),f=j,d=e;while(d!==void 0)i=hostRead(d,"prevDep"),(+hostRead(d,"version")|0)==-1?(d===e&&(e=i),ha(d),h=hostRead(d,"prevDep"),g=hostRead(d,"nextDep"),h===void 0||(h.nextDep=g,d.prevDep=void 0),g===void 0||(g.prevDep=h,d.nextDep=void 0)):f=d,h=hostRead(d,"dep"),h.activeLink=hostRead(d,"prevActiveLink"),d.prevActiveLink=void 0,d=i;c.deps=f,c.depsTail=e}V=function(c){var p=+hostRead(c,"flags");if(0==(p&1))return hostRead(c,"fn")();c.flags=p|2,za(c),a(c);var ea=q,ia=v;q=c,v=!0;try{return hostRead(c,"fn")()}finally{p=q!==c,p&&w("Active effect was not restored correctly - this is likely a Vue internal bug."),b(c),q=ea,v=ia,c.flags=+hostRead(c,"flags")&-3}};Ba=function(c){var p,h=+hostRead(c,"flags");if(!(0!=(h&4)&&0==(h&16))){c.flags=h&-17,h=S;if((+hostRead(c,"globalVersion")|0)!=h){c.globalVersion=h,p=+hostRead(c,"flags");if(!(!hostRead(c,"isSSR")&&0!=(p&128)&&((hostRead(c,"deps")===void 0&&!hostRead(c,"_dirty"))||!$(c)))){c.flags=p|2,h=hostRead(c,"dep");var wb=q,xb=v;q=c,v=!0;try{a(c);var yb=hostRead(c,"fn")(hostRead(c,"_value"));0==(+hostRead(h,"version")|0)?p=!0:(p=yb,p=!hostObjectIs(p,hostRead(c,"_value"))),p&&(p=c,p.flags=+hostRead(c,"flags")|128,c._value=yb,h.version=(+hostRead(h,"version")|0)+1|0)}catch(c){h.version=(+hostRead(h,"version")|0)+1|0;throw c}finally{q=wb,v=xb,b(c),c.flags=+hostRead(c,"flags")&-3}}}}}})();function cb(a,m){a.fn=m,a.deps=void 0,a.depsTail=void 0,a.flags=5,a.next=void 0,a.cleanup=void 0,a.scheduler=void 0,a.onStop=void 0,t===void 0||(hostRead(t,"active")?hostRead(t,"effects").push(a):a.flags=4)}function Aa(a,b=!1){a.flags=+hostRead(a,"flags")|8,b?(a.next=R,R=a):(a.next=Q,Q=a)}function ia(){C--;if(!(C>0)){if(R!==void 0){var a=R;R=void 0;while(a!==void 0){var p=hostRead(a,"next");a.next=void 0,a.flags=+hostRead(a,"flags")&-9,a=p}}var da;while(Q!==void 0){a=Q,Q=void 0;while(a!==void 0){var b=hostRead(a,"next");a.next=void 0,p=+hostRead(a,"flags"),a.flags=p&-9;if(0!=(p&1))try{a.trigger()}catch(p){da===void 0&&(da=p)}a=b}}if(da!==void 0)throw da}}function sb(m,c){hostInstanceOf(hostRead(m,"effect"),y)&&(m=hostRead(hostRead(m,"effect"),"fn")),m=new y(m);if(c!=null)for(var d in c)m[d]=hostRead(c,d);try{V(m)}catch(c){m.stop();throw c}c=function(){return V(m)};c.effect=m;return c}function tb(a){a=hostRead(a,"effect"),a.stop()}function Ca(){N.push(v),v=!1}function ub(){N.push(v),v=!0}function Da(){0==N.length?v=!0:(v=N[N.length-1],N.pop())}function vb(m,a=!1){hostInstanceOf(q,y)?q.cleanup=m:(m=!a,m&&w("onEffectCleanup() was called when there was no active effect to associate with."))}function aa(a){return{version:0,activeLink:void 0,subs:void 0,subsHead:void 0,map:void 0,key:void 0,sc:0,__v_skip:!0,computed:a}}var ba,Ea;(function(){function a(d,e){var f=hostCreateObject();f.effect=d;if(e!==void 0)for(d in e)f[d]=hostRead(e,d);return f}function b(d,h){return{sub:d,dep:h,version:hostRead(h,"version"),nextDep:void 0,prevDep:void 0,nextSub:void 0,prevSub:void 0,prevActiveLink:void 0}}function c(d){var h=hostRead(d,"dep");h.sc=(+hostRead(h,"sc")|0)+1|0;if(0!=(+hostRead(hostRead(d,"sub"),"flags")&4)){var e=hostRead(h,"computed");if(e!==void 0&&hostRead(h,"subs")===void 0){e.flags=+hostRead(e,"flags")|20,e=hostRead(e,"deps");while(e!==void 0)c(e),e=hostRead(e,"nextDep")}e=hostRead(h,"subs");e===d||(d.prevSub=e,e===void 0||(e.nextSub=d)),e=hostRead(h,"subsHead")===void 0,e&&(h.subsHead=d),h.subs=d}}ba=function(h,d){var i,f=hostRead(h,"computed");if(!(q===void 0||!v||q===f)){var e=hostRead(h,"activeLink");if(e===void 0||hostRead(e,"sub")!==q){e=b(q,h),h.activeLink=e,hostRead(q,"deps")===void 0?(q.deps=e,q.depsTail=e):(f=hostRead(q,"depsTail"),e.prevDep=f,f.nextDep=e,q.depsTail=e),c(e),f=0==(+hostRead(q,"flags")&4);if(f&&hostRead(h,"map")!==void 0)f=qa,i=hostRead(h,"map"),hostFinalizationRegister(f,q,{dep:h,map:i,key:hostRead(h,"key")})}else{(+hostRead(e,"version")|0)==-1&&(e.version=hostRead(h,"version"),h=hostRead(e,"nextDep"),h===void 0||(h.prevDep=hostRead(e,"prevDep"),f=hostRead(e,"prevDep"),f===void 0||(f.nextDep=h),f=hostRead(q,"depsTail"),e.prevDep=f,e.nextDep=void 0,f.nextDep=e,q.depsTail=e,hostRead(q,"deps")===e&&(q.deps=h)))}h=hostRead(q,"onTrack");h===void 0||h(a(q,d));return e}};Ea=function(h,d){C++;try{var oa=hostRead(h,"subsHead");while(oa!==void 0){var pa=hostRead(oa,"sub"),xa=hostRead(pa,"onTrigger");xa!==void 0&&0==(+hostRead(pa,"flags")&8)&&xa(a(pa,d)),oa=hostRead(oa,"nextSub")}oa=hostRead(h,"subs");while(oa!==void 0){var za=hostRead(oa,"sub"),Aa=za.notify();!0===Aa&&Ea(hostRead(za,"dep")),oa=hostRead(oa,"prevSub")}}finally{ia()}}})();function ca(h,b){h.version=(+hostRead(h,"version")|0)+1|0,S++,Ea(h,b)}function x(f,d,e){if(!(!v||q===void 0)){var g=X.get(f);g===void 0&&(g=hostCreateMap(),X.set(f,g));var h=g.get(e);h===void 0&&(h=aa(),g.set(e,h),h.map=g,h.key=e),ba(h,{target:f,type:d,key:e})}}function B(h,a){if(h!==void 0)ca(h,a)}function I(f,c,b,d,e,g){var h=X.get(f);if(h===void 0){S++;return}var a={target:f,type:c,key:b,newValue:d,oldValue:e,oldTarget:g};e=function(h){B(h,a)},C++,"clear"===c?h.forEach(e):(e=Array.isArray(f),g=e&&_(b),e&&"length"===b?(f=+d,h.forEach(function(h,d){("length"===d||d===T||"symbol"!=typeof d&&+d>=f)&&B(h,a)})):((b!==void 0||h.has(void 0))&&B(h.get(b),a),g&&B(h.get(T),a),"add"===c?!e?(B(h.get(K),a),A(f)==Kb&&B(h.get(ga),a)):g&&B(h.get("length"),a):"delete"===c?e||(B(h.get(K),a),A(f)==Kb&&B(h.get(ga),a)):"set"===c&&A(f)==Kb&&B(h.get(K),a))),ia()}function db(a,b){a=X.get(a);return a===void 0?void 0:a.get(b)}function Fa(e){for(var k=[],l=0;l<hostLength(e);l++)k.push(hostRead(e,l));return k}function O(a){var b=r(a);if(b===a)return b;x(b,"iterate",T);return z(a)?b:hostArrayApply("map",b,[function(b){return E(a,b)}])}function ja(a){a=r(a),x(a,"iterate",T);return a}function E(f,a){return J(f)?M(f)?oa(P(a)):oa(a):P(a)}function ka(a,i){var b=ja(a),j=[],c=hostRead(b,i).apply(b,j);b=b!==a&&!z(a),i="entries"===i;var n=function(){var p,l=[],u=hostRead(c,"next").apply(c,l);!hostRead(u,"done")&&b&&(p=hostRead(u,"value"),i?p[1]=E(a,hostRead(p,1)):u.value=E(a,p));return u};return b?hostIteratorFacade(c,n):c}function F(a,i,b,c){var k=ja(a),d=k!==a&&!z(a),e=hostRead(k,i);if(e!==hostArrayPrototypeMethod(i))return a=e.apply(a,b),d?P(a):a;i=hostRead(b,0);var f=hostRead(b,1);b=k!==a?d?(0,function(c,l,d){return i.call(this,E(a,c),l,a)}):hostLength(i)>2?(0,function(c,l,d){return i.call(this,c,l,a)}):i:i,k=e.call(k,b,f);return d&&1==c?E(a,k):d&&2==c?hostArrayApply("map",k,[function(b){return E(a,b)}]):k}function Ga(b,k,e){var f=ja(b),d=f!==b&&!z(b),c=hostRead(e,0),g=d&&1==hostLength(e),a=g;d=f!==b?d?(0,function(){var d=hostRead(arguments,0);a&&(a=!1,d=E(b,d));return c.call(this,d,E(b,hostRead(arguments,1)),hostRead(arguments,2),b)}):hostLength(c)>3?(0,function(){return c.call(this,hostRead(arguments,0),hostRead(arguments,1),hostRead(arguments,2),b)}):c:c,e=Fa(e),e[0]=d,k=hostArrayApply(k,f,e);return a?E(b,k):k}function la(a,i,e){a=r(a),x(a,"iterate",T);var c=hostRead(a,i).apply(a,e);return(c===-1||!1===c)&&na(hostRead(e,0))?(e[0]=r(hostRead(e,0)),hostRead(a,i).apply(a,e)):c}function W(a,i,e){Ca(),C++,a=hostRead(r(a),i).apply(a,e),ia(),Da();return a}function Ha(b){if("symbol"!=typeof b)return!1;for(var l=0;l<(+hostRead(sa,"length")|0);l++)if(hostRead(sa,l)===b)return!0;return!1}var Ia=(function(){function a(c){return"__proto__"===c||"__v_isRef"===c||"__isVue"===c||Ha(c)}function b(f,j,g){var c=ta;j&&g?c=wa:j?c=va:g&&(c=ua);return c.get(f)}return function(c,e,d,j,g){if("__v_skip"===e)return hostRead(c,e);if("__v_isReactive"===e)return!j;if("__v_isReadonly"===e)return j;if("__v_isShallow"===e)return g;if("__v_raw"===e)return d===b(c,j,g)||hostSamePrototype(c,d)?c:void 0;var f=Array.isArray(c);if(!j){var h;if(f&&(h=hostRead(s,e))!==void 0)return h;if("hasOwnProperty"===e)return Qa}u(c)&&(d=c);d=hostReflectGet(c,e,d);if(a(e))return d;j||x(c,"get",e);return g?d:u(d)?(h=hostRead(d,"value"),f&&_(e)||(d=h),j&&L(d)?ma(d):d):L(d)?j?ma(d):La(d):d}})();function eb(a,b,d,c,g){var e=hostRead(a,b),k=Array.isArray(a)&&_(b);if(!g){var f=J(e);!z(d)&&!J(d)&&(e=r(e),d=r(d));if(!k&&u(e)&&!u(d))return f?(d=Lb+hostString(b)+Jb,hostRead(a,b),w(d)):e.value=d,!0}g=U(a,b);k&&(g=+b,g=g<+hostRead(a,"length")),k=u(a)?a:c,k=hostReflectSet(a,b,d,k),k&&a===r(c)&&(!g?I(a,"add",b,d,void 0,void 0):D(d,e)&&I(a,"set",b,d,e,void 0));return k}function Ja(g){let c=hostCreateObject();c.get=function(f,d,e){return Ia(f,d,e,!1,g)},c.set=function(){let c=hostRead(arguments,0),a=hostRead(arguments,1),b=hostRead(arguments,2);return eb(c,a,b,hostRead(arguments,3),g)},c.deleteProperty=function(f,b){var a=U(f,b),c=hostRead(f,b),k=hostReflectDelete(f,b);k||(a=!1),a&&I(f,"delete",b,void 0,c,void 0);return k},c.has=function(f,b){var k=hostReflectHas(f,b);("symbol"!=typeof b||!Ha(b))&&x(f,"has",b);return k},c.ownKeys=function(f){var c=K;Array.isArray(f)&&(c="length"),x(f,"iterate",c);return hostReflectOwnKeys(f)};return c}function Ka(g){let c=hostCreateObject();c.get=function(f,d,e){return Ia(f,d,e,!0,g)},c.set=function(){var c=Lb+hostString(hostRead(arguments,1))+Jb;hostRead(arguments,0),w(c);return!0},c.deleteProperty=function(f,b){w("Delete operation on key \""+hostString(b)+Jb);return!0};return c}var da=(function(){function a(d,j,g){return g?d:j?oa(d):P(d)}function b(d,i,e,j,g){var f=hostRead(d,"__v_raw"),k=r(f),Qb=A(k),h=Qb==Kb,l="entries"===i||i===hostIteratorSymbol()&&h;h="keys"===i&&h,i=hostRead(f,i).apply(f,e),d=K,h&&(d=ga),j||x(k,"iterate",d);return hostIteratorFacade(i,function(){var n=[],x=hostRead(i,"next").apply(i,n);if(!hostRead(x,"done")){var d=hostRead(x,"value"),e=a(d,j,g);l&&(e=[a(hostRead(d,0),j,g),a(hostRead(d,1),j,g)]),x.value=e}return x})}function c(f,d){var e=r(d);e!==d&&hostPrototypeMethodApply(f,"has",[e])&&(f=A(f).slice(8,-1),d="Map"==f?" as keys":"",w("Reactive "+f+" contains both the raw and reactive versions of the same object"+d+", which can lead to inconsistencies. Avoid differentiating between the raw and "+"reactive versions of an object and only use the reactive version if possible."))}return function(j,g){var d=hostCreateObject();d.get=function(b){var f=hostRead(this,"__v_raw"),c=r(f),d=r(b);j||(D(b,d)&&x(c,"get",b),x(c,"get",d));if(hostPrototypeMethodApply(c,"has",[b])){var z=[b];return a(hostRead(f,"get").apply(f,z),j,g)}if(hostPrototypeMethodApply(c,"has",[d]))return b=[d],a(hostRead(f,"get").apply(f,b),j,g);if(f!==c){var J=[b];hostRead(f,"get").apply(f,J)}},hostDefineAccessor(d,"size",function(){var f=hostRead(this,"__v_raw");j||x(r(f),"iterate",K);return hostRead(f,"size")},void 0),d.has=function(b){var f=hostRead(this,"__v_raw"),c=r(f),a=r(b);j||(D(b,a)&&x(c,"has",b),x(c,"has",a));if(b===a){var i=[b];return hostRead(f,"has").apply(f,i)}var m=[b];if(hostRead(f,"has").apply(f,m))var F=!0;else{F=[a],F=!!hostRead(f,"has").apply(f,F)}return F},d.forEach=function(nc,oc){var mc=this,pc=hostRead(mc,"__v_raw"),qc=r(pc);j||x(qc,"iterate",K),qc=[function(d,b){return nc.call(oc,a(d,j,g),a(b,j,g),mc)}];return hostRead(pc,"forEach").apply(pc,qc)},j?(d.add=function(){var e=hostRead(arguments,0),b=e?"on key \""+hostString(e)+"\" ":"";e="Add",e=e+" operation "+b+"failed: target is readonly.",r(this),w(e);return this},d.set=function(){var e=hostRead(arguments,0),b=e?"on key \""+hostString(e)+"\" ":"";e="Set",e=e+" operation "+b+"failed: target is readonly.",r(this),w(e);return this},d.delete=function(){var e=hostRead(arguments,0),b=e?"on key \""+hostString(e)+"\" ":"";e="Delete",e=e+" operation "+b+"failed: target is readonly.",r(this),w(e);return!1},d.clear=function(){var e=hostRead(arguments,0),b=e?"on key \""+hostString(e)+"\" ":"";e="Clear",e=e+" operation "+b+"failed: target is readonly.",r(this),w(e)}):(d.add=function(b){var a,f=r(this),d=r(b),c=!g&&!z(b)&&!J(b)?d:b;a=hostPrototypeMethodApply(f,"has",[c])||D(b,c)&&hostPrototypeMethodApply(f,"has",[b])||D(d,c)&&hostPrototypeMethodApply(f,"has",[d]),a||(a=[c],hostRead(f,"add").apply(f,a),I(f,"add",c,c,void 0,void 0));return this},d.set=function(b,d){!g&&!z(d)&&!J(d)&&(d=r(d));var a=r(this),e=!!hostPrototypeMethodApply(a,"has",[b]);!e?(b=r(b),e=!!hostPrototypeMethodApply(a,"has",[b])):c(a,b);var ba=hostPrototypeMethodApply(a,"get",[b]),h=[b,d];hostRead(a,"set").apply(a,h),!e?I(a,"add",b,d,void 0,void 0):D(d,ba)&&I(a,"set",b,d,ba,void 0);return this},d.delete=function(b){var f=r(this),a=!!hostPrototypeMethodApply(f,"has",[b]);!a?(b=r(b),a=!!hostPrototypeMethodApply(f,"has",[b])):c(f,b);var d=void 0,e=A(f).slice(8,-1);("Map"==e||"WeakMap"==e)&&(d=hostPrototypeMethodApply(f,"get",[b])),e=[b],e=hostRead(f,"delete").apply(f,e),a&&I(f,"delete",b,void 0,d,void 0);return e},d.clear=function(){var f=r(this),a=0!=+hostCollectionSize(f),b=hostCloneCollection(f),j=[],c=hostRead(f,"clear").apply(f,j);a&&I(f,"clear",void 0,void 0,void 0,b);return c});var e=hostIteratorSymbol();d.keys=function(){return b(this,"keys",arguments,j,g)},d.values=function(){return b(this,"values",arguments,j,g)},d.entries=function(){return b(this,"entries",arguments,j,g)},d[e]=function(){return b(this,e,arguments,j,g)};return d}})();function fb(f){var a=A(f).slice(8,-1);return"Object"==a||"Array"==a?1:"Map"==a||"Set"==a||"WeakMap"==a||"WeakSet"==a?2:0}function ea(f,j,a,b,c){if(!L(f))return j=j?"readonly":"reactive",w("value cannot be made "+j+": "+hostString(f)),f;if(hostRead(f,"__v_raw")!==void 0&&!(j&&M(f)))return f;if(hostRead(f,"__v_skip")||!hostIsExtensible(f))return f;j=c.get(f);if(j!==void 0)return j;j=fb(f);if(0==j)return f;2==j&&(a=b),j=hostCreateProxy(f,a),c.set(f,j);return j}function La(f){return J(f)?f:ea(f,!1,Ra,Va,ta)}function wb(f){return ea(f,!1,Ta,Wa,ua)}function ma(f){return ea(f,!0,Sa,Xa,va)}function xb(f){return ea(f,!0,Ua,Ya,wa)}function M(d){return J(d)?M(hostRead(d,"__v_raw")):!!hostRead(d,"__v_isReactive")}function J(d){return!!hostRead(d,"__v_isReadonly")}function z(d){return!!hostRead(d,"__v_isShallow")}function na(d){d=hostRead(d,"__v_raw");return d!==void 0&&d?!0:!1}function r(a){var b=hostRead(a,"__v_raw");return b?r(b):a}function yb(d){!U(d,"__v_skip")&&hostIsExtensible(d)&&Object.defineProperty(d,"__v_skip",{configurable:!0,enumerable:!1,writable:!1,value:!0});return d}function P(d){return L(d)?La(d):d}function oa(d){return L(d)?ma(d):d}function u(d){return!0===hostRead(d,"__v_isRef")}function Ma(d,g){if(u(d))return d;var e=hostCreateObject();if(!g){var h=r(d),f=P(d);d=h}else{f=d}h=aa();e._rawValue=d,e._value=f,e.dep=h,e.__v_isRef=!0,e.__v_isShallow=g,hostDefineAccessor(e,"value",function(){ba(h,{target:e,type:"get",key:"value"});return hostRead(e,"_value")},function(b){var d=hostRead(e,"_rawValue"),c=g||z(b)||J(b);c||(b=r(b)),D(b,d)&&(e._rawValue=b,c=!c?P(b):b,e._value=c,ca(h,{target:e,type:"set",key:"value",newValue:b,oldValue:d}))});return e}function gb(d){return Ma(d,!1)}function zb(d){return Ma(d,!0)}function Ab(a){var h=hostRead(a,"dep");h===void 0||ca(h,{target:a,type:"set",key:"value",newValue:hostRead(a,"_value")})}function pa(d){return u(d)?hostRead(d,"value"):d}function Bb(o){return"function"==typeof o?o():pa(o)}function Cb(a){return M(a)?a:hostCreateProxy(a,Za)}function Db(d){let e=hostCreateObject(),h=aa();d=d(function(){ba(h)},function(){ca(h)}),e.dep=h,e._get=hostRead(d,"get"),e._set=hostRead(d,"set"),e.__v_isRef=!0,e._value=void 0,hostDefineAccessor(e,"value",function(){let d=hostRead(e,"_get")();e._value=d;return d},function(d){hostRead(e,"_set")(d)});return e}function hb(a){let k=hostCreateObject();k._getter=a,k.__v_isRef=!0,k.__v_isReadonly=!0,k._value=void 0,hostDefineAccessor(k,"value",function(){let c=a();k._value=c;return c},void 0);return k}function Na(c,b,d){"symbol"==typeof b||(b=hostString(b));var e=r(c),a=!0;if(!Array.isArray(c)||"symbol"==typeof b||!_(b)){var k=c;while(!0){a=!na(k)||z(k),k=hostRead(k,"__v_raw");if(!a||k===void 0||!k)break}}k=hostCreateObject();k.__v_isRef=!0,k._value=void 0,k._object=c,k._key=b,k._defaultValue=d,k._raw=e,k._shallow=a,hostDefineAccessor(k,"value",function(){var j=hostRead(c,b);a&&(j=pa(j));var l=j===void 0?d:j;k._value=l;return l},function(d){let p;if(a&&u(hostRead(e,b))){p=hostRead(c,b);if(u(p)){p.value=d;return}}c[b]=d}),hostDefineAccessor(k,"dep",function(){return db(e,b)},void 0);return k}function Eb(a){var k=!na(a);k&&w("toRefs() expects a reactive object but received a plain one."),k=hostCreateObject(),Array.isArray(a)&&(k=hostCreateArray(hostLength(a)));for(var b in a)k[b]=Na(a,b,void 0);return k}function Fb(o,b,a){return u(o)?o:"function"==typeof o?hb(o):L(o)&&hostLength(arguments)>1?Na(o,b,a):gb(o)}function Gb(e,f){var d;if("function"!=typeof e){var g=hostRead(e,"get");d=hostRead(e,"set"),e=g}g=hostCreateObject();var h=aa(g);g._value=void 0,g.dep=h,g.__v_isRef=!0,g.__v_isReadonly=d===void 0,g.deps=void 0,g.depsTail=void 0,g.flags=16,g.globalVersion=-1,g.isSSR=!1,g.next=void 0,g.effect=g,g.fn=e,g.setter=d,g.notify=function(){var p=+hostRead(g,"flags")|16;g.flags=p;if(0==(p&8)&&q!==g)return Aa(g,!0),!0},e=f!=null,e&&(g.onTrack=hostRead(f,"onTrack"),g.onTrigger=hostRead(f,"onTrigger")),hostDefineAccessor(g,"value",function(){var e=ba(h,{target:g,type:"get",key:"value"});Ba(g),e===void 0||(e.version=hostRead(h,"version"));return hostRead(g,"_value")},function(c){d!==void 0?d(c):w("Write operation failed: computed value is readonly")});return g}function Hb(){return H}function ib(a,b=!1,c){c===void 0&&(c=H),c!==void 0?(b=Z.get(c),b===void 0&&(b=[],Z.set(c,b)),b.push(a)):(a=!b,a&&w("onWatcherCleanup() was called when there was no active watcher to associate with."))}function G(d,a,b){if(a<=0||!L(d)||hostRead(d,"__v_skip"))return d;var l=b.get(d);if(l!==void 0&&+l>=a)return d;b.set(d,a),a--;if(u(d))G(hostRead(d,"value"),a,b);else if(Array.isArray(d))for(l=0;l<hostLength(d);l++)G(hostRead(d,l),a,b);else if("[object Set]"==A(d)||A(d)==Kb)d.forEach(function(d){G(d,a,b)});else if("[object Object]"==A(d)){for(l in d)G(hostRead(d,l),a,b);var c=hostEnumerableSymbols(d);for(l=0;l<hostLength(c);l++)G(hostRead(d,hostRead(c,l)),a,b)}return d}function Ib(d,a=0,b){0==a&&(a=hostInfinity()),b===void 0&&(b=hostCreateMap());return G(d,a,b)}function jb(h,i,j){j==null&&(j=hostCreateObject());var a,b,c,Rb=hostRead(j,"immediate"),n=hostRead(j,"deep"),Sb=!!hostRead(j,"once"),p=hostRead(j,"scheduler"),Nb=hostRead(j,"augmentJob"),k=hostRead(j,"call"),Mb="function"==typeof i,d=!1,f=!1,Ob=function(b){var c=hostRead(j,"onWarn");"function"==typeof c?c("Invalid watch source: ",b,"A watch source can only be a getter/effect function, a ref, a reactive object, or an array of these types."):w("Invalid watch source: A watch source can only be a getter/effect function, a ref, a reactive object, or an array of these types.")},Pb=function(d){return n?d:z(d)||!1===n||0===n?G(d,1,hostCreateMap()):G(d,hostInfinity(),hostCreateMap())};if(u(h)){var l=function(){return hostRead(h,"value")};d=z(h)}else if(M(h))l=function(){return Pb(h)},d=!0;else if(Array.isArray(h)){for(f=!0,l=0;l<hostLength(h);l++){var m=hostRead(h,l);(M(m)||z(m))&&(d=!0)}l=function(){for(var e,d,f=hostCreateArray(hostLength(h)),l=0;l<hostLength(h);l++)e=hostRead(h,l),d=void 0,u(e)?d=hostRead(e,"value"):M(e)?d=Pb(e):"function"==typeof e?d="function"==typeof k?k(e,2):e():Ob(e),f[l]=d;return f}}else{"function"==typeof h?l=Mb?"function"==typeof k?function(){return k(h,2)}:h:function(){if("function"==typeof b){Ca();try{b()}finally{Da()}}var G=H;H=a;try{return"function"==typeof k?k(h,3,[c]):h(c)}finally{H=G}}:(l=function(){},Ob(h))}if(Mb&&n){var g=+n;!0===n&&(g=hostInfinity());var Qb=function(){let c=l();return G(c,g,hostCreateMap())}}else{Qb=l}var Tb=bb();m=function(){var k=a.stop;a.stop(),Tb!==void 0&&hostRead(Tb,"active")&&ab(hostRead(Tb,"effects"),a)};var e=Y;if(f){e=hostCreateArray(hostLength(h));for(var o=0;o<hostLength(h);o++)e[o]=Y}o=function(vb){var sb,ub,wb,o;if(!(0==(+hostRead(a,"flags")&1)||!hostRead(a,"dirty")&&!vb))if(Mb){var Ga=a.run;o=a.run(),sb=!!vb||n||d;if(!sb&&f)for(ub=0;ub<hostLength(o);ub++)wb=hostRead(o,ub),D(wb,hostRead(e,ub))&&(sb=!0);else{sb=sb||D(o,e)}if(sb){"function"==typeof b&&b();var ib=H;H=a;try{var jb=e;jb===Y?jb=void 0:f&&hostRead(jb,0)===Y&&(jb=[]);var qb=[o,jb,c];e=o,"function"==typeof k?k(i,3,qb):i.apply(void 0,qb)}finally{H=ib}Sb&&m()}}else a.run()};"function"==typeof Nb&&Nb(o);a=new y(Qb),Nb="function"==typeof p?function(){p(o,!1)}:o,a.scheduler=Nb,c=function(m){ib(m,!1,a)},b=function(){var c=Z.get(a);if(c!==void 0){if("function"==typeof k)k(c,4);else{for(var E=0;E<hostLength(c);E++)hostRead(c,E)()}Z.delete(a)}},a.onStop=b,Nb=a,Nb.onTrack=hostRead(j,"onTrack"),Nb=a,Nb.onTrigger=hostRead(j,"onTrigger"),Mb?Rb?o(!0):(Nb=a,e=Nb.run()):"function"==typeof p?p(function(){o(!0)},!0):(Nb=a,Nb.run()),m.pause=function(){return a.pause()},m.resume=function(){return a.resume()},m.stop=m;return m}_a();$a(),Object.assign,Array.isArray;var d=Object.prototype,Oa=d.toString;xa(",key,ref,ref_for,ref_key,onVnodeBeforeMount,onVnodeMounted,onVnodeBeforeUpdate,onVnodeUpdated,onVnodeBeforeUnmount,onVnodeUnmounted"),xa("bind,cloak,else-if,else,for,html,if,model,on,once,pre,show,slot,text,memo"),new RegExp("-\\w","g");var c=null;Object.create(c),new RegExp("\\B([A-Z])","g"),Object.create(c),Object.create(c),Object.create(c),new RegExp("^[_$a-zA-Z\\xA0-\\uFFFF][_$a-zA-Z0-9\\xA0-\\uFFFF]*$");var kb={GET:"get",HAS:"has",ITERATE:"iterate"},lb={SET:"set",ADD:"add",DELETE:"delete",CLEAR:"clear"},mb={SKIP:"__v_skip",IS_REACTIVE:"__v_isReactive",IS_READONLY:"__v_isReadonly",IS_SHALLOW:"__v_isShallow",RAW:"__v_raw",IS_REF:"__v_isRef"},t=void 0,Pa=hostCreateEffectScopeClass("EffectScope",(0,function(){var e=!!hostRead(arguments,0);this.detached=e,this._active=!0,this._on=0,this.effects=[],this.cleanups=[],this._isPaused=!1,this._warnOnRun=!0,this.parent=void 0,this.scopes=void 0,this.index=void 0,this.prevScope=void 0,this.__v_skip=!0;if(!e&&t!==void 0)if(hostRead(t,"active")){this.parent=t,e=hostRead(t,"scopes"),e===void 0&&(e=[],t.scopes=e);var l=hostLength(e);e.push(this),this.index=l}else this._active=!1,this._warnOnRun=!1}),(0,function(){return hostRead(this,"_active")}),(0,function(){if(hostRead(this,"_active")){this._isPaused=!0;var l=hostRead(this,"scopes");if(l!==void 0){var b=l.slice();for(l=0;l<hostLength(b);l++){var c=hostRead(b,l);c.pause()}}l=hostRead(this,"effects");for(var a=0;a<hostLength(l);a++)b=hostRead(l,a),b.pause()}}),(0,function(){var l;if(hostRead(this,"_active")&&hostRead(this,"_isPaused")){this._isPaused=!1,l=hostRead(this,"scopes");if(l!==void 0){var b=l.slice();for(l=0;l<hostLength(b);l++){var c=hostRead(b,l);c.resume()}}l=hostRead(this,"effects").slice();for(var a=0;a<hostLength(l);a++)b=hostRead(l,a),b.resume()}}),(0,function(m){if(hostRead(this,"_active")){var k=t;try{t=this;return m()}finally{t=k}}var a=!!hostRead(this,"_warnOnRun");a&&w("cannot run an inactive effect scope.")}),(0,function(){var b=(+hostRead(this,"_on")|0)+1|0;this._on=b,1==b&&(this.prevScope=t,t=this)}),(0,function(){var b=+hostRead(this,"_on")|0;if(b>0){b--,this._on=b;if(0==b){if(t===this)t=hostRead(this,"prevScope");else{b=t;while(b!==void 0){if(hostRead(b,"prevScope")===this){b.prevScope=hostRead(this,"prevScope");break}b=hostRead(b,"prevScope")}}this.prevScope=void 0}}}),(0,function(b){if(!(!hostRead(this,"_active"))){this._active=!1;for(var d,e,c=hostRead(this,"effects"),l=0;l<hostLength(c);l++)d=hostRead(c,l),d.stop();c.length=0;for(c=hostRead(this,"cleanups"),l=0;l<hostLength(c);l++)hostRead(c,l)();c.length=0,c=hostRead(this,"scopes");if(c!==void 0){for(d=c.slice(),l=0;l<hostLength(d);l++)e=hostRead(d,l),e.stop(!0);c.length=0}l=hostRead(this,"parent");!hostRead(this,"detached")&&l!==void 0&&!b&&(c=hostRead(l,"scopes"),b=c.pop(),b!==void 0&&b!==this&&(l=hostRead(this,"index"),c[l]=b,b.index=l)),this.parent=void 0}})),q=void 0,nb={"1":"ACTIVE","2":"RUNNING","4":"TRACKING","8":"NOTIFIED","16":"DIRTY","32":"ALLOW_RECURSE","64":"PAUSED","128":"EVALUATED",ACTIVE:1,RUNNING:2,TRACKING:4,NOTIFIED:8,DIRTY:16,ALLOW_RECURSE:32,PAUSED:64,EVALUATED:128},fa=hostCreateWeakSet(),C=0,Q=void 0,R=void 0,v=!0,N=[],y=hostCreateClass1("ReactiveEffect",function(m){cb(this,m)});c=y.prototype,hostDefineMethod(c,"pause",function(){this.flags=+hostRead(this,"flags")|64}),c=y.prototype,hostDefineMethod(c,"resume",function(){var p=+hostRead(this,"flags");0!=(p&64)&&(this.flags=p&-65,!fa.has(this)||(fa.delete(this),this.trigger()))}),c=y.prototype,hostDefineMethod(c,"notify",function(){var p=+hostRead(this,"flags");if(!(0!=(p&2)&&0==(p&32)))0==(p&8)&&Aa(this)}),c=y.prototype,hostDefineMethod(c,"run",function(){return V(this)}),c=y.prototype,hostDefineMethod(c,"stop",function(){if(0!=(+hostRead(this,"flags")&1)){var b=hostRead(this,"deps");while(b!==void 0)ha(b),b=hostRead(b,"nextDep");this.deps=void 0,this.depsTail=void 0,za(this),b=hostRead(this,"onStop"),b===void 0||b(),this.flags=+hostRead(this,"flags")&-2}}),c=y.prototype,hostDefineMethod(c,"trigger",function(){if(0!=(+hostRead(this,"flags")&64))fa.add(this);else{var d=hostRead(this,"scheduler");d!==void 0?d():$(this)&&V(this)}}),c=y.prototype,hostDefineMethod(c,"runIfDirty",function(){$(this)&&V(this)}),c=y.prototype,hostDefineAccessor(c,"dirty",function(){return $(this)},void 0);var S=0,qa=hostCreateFinalizationRegistry(function(a){var h=hostRead(a,"dep"),c=hostRead(a,"map"),b=hostRead(a,"key");hostRead(h,"subs")===void 0&&c.get(b)===h&&c.delete(b)}),X=hostCreateWeakMap(),K=hostSymbol("Object iterate"),ga=hostSymbol("Map keys iterate"),T=hostSymbol("Array iterate"),s=hostCreateNullObject(),ra=hostIteratorSymbol();s[ra]=function(){return ka(this,ra)},s.concat=function(){for(var d,e=Fa(arguments),l=0;l<hostLength(e);l++)d=hostRead(e,l),Array.isArray(d)&&(e[l]=O(d));return hostArrayApply("concat",O(this),e)},s.entries=function(){return ka(this,"entries")},s.every=function(){return F(this,"every",arguments,0)},s.filter=function(){return F(this,"filter",arguments,2)},s.find=function(){return F(this,"find",arguments,1)},s.findIndex=function(){return F(this,"findIndex",arguments,0)},s.findLast=function(){return F(this,"findLast",arguments,1)},s.findLastIndex=function(){return F(this,"findLastIndex",arguments,0)},s.forEach=function(){return F(this,"forEach",arguments,0)},s.includes=function(){return la(this,"includes",arguments)},s.indexOf=function(){return la(this,"indexOf",arguments)},s.join=function(){return hostArrayApply("join",O(this),arguments)},s.lastIndexOf=function(){return la(this,"lastIndexOf",arguments)},s.map=function(){return F(this,"map",arguments,0)},s.pop=function(){return W(this,"pop",arguments)},s.push=function(){return W(this,"push",arguments)},s.reduce=function(){return Ga(this,"reduce",arguments)},s.reduceRight=function(){return Ga(this,"reduceRight",arguments)},s.shift=function(){return W(this,"shift",arguments)},s.some=function(){return F(this,"some",arguments,0)},s.splice=function(){return W(this,"splice",arguments)},s.toReversed=function(){return hostArrayApply("toReversed",O(this),arguments)},s.toSorted=function(){return hostArrayApply("toSorted",O(this),arguments)},s.toSpliced=function(){return hostArrayApply("toSpliced",O(this),arguments)},s.unshift=function(){return W(this,"unshift",arguments)},s.values=function(){return ka(this,"values")};var sa=hostBuiltInSymbols(),Qa=(0,function(b){"symbol"==typeof b||(b=hostString(b));var a=r(this);x(a,"has",b);return U(a,b)}),Ra=Ja(!1),Sa=Ka(!1),Ta=Ja(!0),Ua=Ka(!0);c=da(!1,!1);var Va={get:function(d,b,e){if("__v_isReactive"===b)return!!1;if("__v_isReadonly"===b)return!1;if("__v_raw"===b)return d;var o=U(c,b)&&b in d?c:d;return hostReflectGet(o,b,e)}};d=da(!1,!0);var Wa={get:function(n,b,e){if("__v_isReactive"===b)return!!1;if("__v_isReadonly"===b)return!1;if("__v_raw"===b)return n;var o=U(d,b)&&b in n?d:n;return hostReflectGet(o,b,e)}},e=da(!0,!1),Xa={get:function(d,b,c){if("__v_isReactive"===b)return!!0;if("__v_isReadonly"===b)return!0;if("__v_raw"===b)return d;var o=U(e,b)&&b in d?e:d;return hostReflectGet(o,b,c)}},f=da(!0,!0),Ya={get:function(d,b,e){if("__v_isReactive"===b)return!!0;if("__v_isReadonly"===b)return!0;if("__v_raw"===b)return d;var o=U(f,b)&&b in d?f:d;return hostReflectGet(o,b,e)}},ta=hostCreateWeakMap(),ua=hostCreateWeakMap(),va=hostCreateWeakMap(),wa=hostCreateWeakMap(),Za={get:function(f,b,a){return"__v_raw"===b?f:pa(hostReflectGet(f,b,a))},set:function(){var f=hostRead(arguments,0),b=hostRead(arguments,1),d=hostRead(arguments,2),c=hostRead(arguments,3),e=hostRead(f,b);return u(e)&&!u(d)?(e.value=d,!0):hostReflectSet(f,b,d,c)}},ob={"2":"WATCH_GETTER","3":"WATCH_CALLBACK","4":"WATCH_CLEANUP",WATCH_GETTER:2,WATCH_CALLBACK:3,WATCH_CLEANUP:4},Y=hostCreateObject(),Z=hostCreateWeakMap(),H=void 0,pb=hostFunction2Rest(function(e){let c=hostRead(e,0),a=hostRead(e,1);return jb(c,a,hostRead(e,2))});Object.defineProperty(Gb,"name",{configurable:true,value:"computed"});Object.defineProperty(Db,"name",{configurable:true,value:"customRef"});Object.defineProperty(sb,"name",{configurable:true,value:"effect"});Object.defineProperty(qb,"name",{configurable:true,value:"effectScope"});Object.defineProperty(ub,"name",{configurable:true,value:"enableTracking"});Object.defineProperty(bb,"name",{configurable:true,value:"getCurrentScope"});Object.defineProperty(Hb,"name",{configurable:true,value:"getCurrentWatcher"});Object.defineProperty(na,"name",{configurable:true,value:"isProxy"});Object.defineProperty(M,"name",{configurable:true,value:"isReactive"});Object.defineProperty(J,"name",{configurable:true,value:"isReadonly"});Object.defineProperty(u,"name",{configurable:true,value:"isRef"});Object.defineProperty(z,"name",{configurable:true,value:"isShallow"});Object.defineProperty(yb,"name",{configurable:true,value:"markRaw"});Object.defineProperty(vb,"name",{configurable:true,value:"onEffectCleanup"});Object.defineProperty(rb,"name",{configurable:true,value:"onScopeDispose"});Object.defineProperty(ib,"name",{configurable:true,value:"onWatcherCleanup"});Object.defineProperty(Ca,"name",{configurable:true,value:"pauseTracking"});Object.defineProperty(Cb,"name",{configurable:true,value:"proxyRefs"});Object.defineProperty(La,"name",{configurable:true,value:"reactive"});Object.defineProperty(O,"name",{configurable:true,value:"reactiveReadArray"});Object.defineProperty(ma,"name",{configurable:true,value:"readonly"});Object.defineProperty(gb,"name",{configurable:true,value:"ref"});Object.defineProperty(Da,"name",{configurable:true,value:"resetTracking"});Object.defineProperty(wb,"name",{configurable:true,value:"shallowReactive"});Object.defineProperty(ja,"name",{configurable:true,value:"shallowReadArray"});Object.defineProperty(xb,"name",{configurable:true,value:"shallowReadonly"});Object.defineProperty(zb,"name",{configurable:true,value:"shallowRef"});Object.defineProperty(tb,"name",{configurable:true,value:"stop"});Object.defineProperty(r,"name",{configurable:true,value:"toRaw"});Object.defineProperty(P,"name",{configurable:true,value:"toReactive"});Object.defineProperty(oa,"name",{configurable:true,value:"toReadonly"});Object.defineProperty(Fb,"name",{configurable:true,value:"toRef"});Object.defineProperty(Eb,"name",{configurable:true,value:"toRefs"});Object.defineProperty(Bb,"name",{configurable:true,value:"toValue"});Object.defineProperty(x,"name",{configurable:true,value:"track"});Object.defineProperty(Ib,"name",{configurable:true,value:"traverse"});Object.defineProperty(I,"name",{configurable:true,value:"trigger"});Object.defineProperty(Ab,"name",{configurable:true,value:"triggerRef"});Object.defineProperty(pa,"name",{configurable:true,value:"unref"});Object.defineProperty(pb,"name",{configurable:true,value:"watch"});export{T as ARRAY_ITERATE_KEY,nb as EffectFlags,Pa as EffectScope,K as ITERATE_KEY,ga as MAP_KEY_ITERATE_KEY,y as ReactiveEffect,mb as ReactiveFlags,kb as TrackOpTypes,lb as TriggerOpTypes,ob as WatchErrorCodes,Gb as computed,Db as customRef,sb as effect,qb as effectScope,ub as enableTracking,bb as getCurrentScope,Hb as getCurrentWatcher,na as isProxy,M as isReactive,J as isReadonly,u as isRef,z as isShallow,yb as markRaw,vb as onEffectCleanup,rb as onScopeDispose,ib as onWatcherCleanup,Ca as pauseTracking,Cb as proxyRefs,La as reactive,O as reactiveReadArray,ma as readonly,gb as ref,Da as resetTracking,wb as shallowReactive,ja as shallowReadArray,xb as shallowReadonly,zb as shallowRef,tb as stop,r as toRaw,P as toReactive,oa as toReadonly,Fb as toRef,Eb as toRefs,Bb as toValue,x as track,Ib as traverse,I as trigger,Ab as triggerRef,pa as unref,pb as watch}