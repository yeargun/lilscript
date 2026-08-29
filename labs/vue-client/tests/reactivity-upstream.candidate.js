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

let Kb="\" failed: target is readonly.",Lb="[object Map]",Mb="Set operation on key \"";function ya(l){var b=Object.create(null),a=l.split(",");for(l=0;l<a.length;l++)b[a[l]||""]=1}function za(){if("undefined"==typeof process)var b,a=!0;else{b=process.env,a=b.NODE_ENV,a="production"!==a}return a}function $a(){var d={};if(za()){Object.freeze(d);return}}function ab(){var d=[];if(za()){Object.freeze(d);return}}function bb(a,l){l=+a.indexOf(l),l>-1&&a.splice(l,1)}function U(d,b){return!!Object.prototype.hasOwnProperty.call(d,b)}function A(d){return Pa.call(d)+""}function K(d){return null!==d&&"object"==typeof d}function _(b){if("string"!=typeof b)return!1;b+="";return"NaN"==b||b.startsWith("-")?!1:parseInt(b,10)+""==b}function C(d,a){return!0!==Object.is(d,a)}function w(a){hostWarn(a,arguments)}function rb(a){return new Qa(a)}function cb(){return t}function sb(m,a=!1){t!==void 0?hostRead(t,"cleanups").push(m):(m=!a,m&&w("onScopeDispose() is called when there is no active effect scope to be associated with."))}function ha(a,b=!1){var h=hostRead(a,"dep"),c=hostRead(a,"prevSub"),d=hostRead(a,"nextSub");c===void 0||(c.nextSub=d,a.prevSub=void 0),d===void 0||(d.prevSub=c,a.nextSub=void 0);var e=hostRead(h,"subsHead")===a;e&&(h.subsHead=d);if(hostRead(h,"subs")===a){h.subs=c,d=hostRead(h,"computed");if(c===void 0&&d!==void 0){d.flags=+hostRead(d,"flags")&-5,a=hostRead(d,"deps");while(a!==void 0)ha(a,!0),a=hostRead(a,"nextDep")}}b||(a=(+hostRead(h,"sc")|0)-1|0,h.sc=a,c=hostRead(h,"map"),0==a&&c!==void 0&&c.delete(hostRead(h,"key")))}function $(a){var b=hostRead(a,"deps");while(b!==void 0){var h=hostRead(b,"dep"),c=+hostRead(h,"version")|0;if(c!=(+hostRead(b,"version")|0))return!0;c=hostRead(h,"computed");if(c!==void 0&&(Ca(c),h=+hostRead(h,"version")|0,h!=(+hostRead(b,"version")|0)))return!0;b=hostRead(b,"nextDep")}return!!hostRead(a,"_dirty")}function Aa(a){var b=hostRead(a,"cleanup");a.cleanup=void 0;if(b!==void 0){var l=q;q=void 0;try{b()}finally{q=l}}}var V,Ca;(function(){function a(c){c=hostRead(c,"deps");while(c!==void 0){c.version=-1;var h=hostRead(c,"dep");c.prevActiveLink=hostRead(h,"activeLink"),h.activeLink=c,c=hostRead(c,"nextDep")}}function b(c){var i,h,g,j=void 0,e=hostRead(c,"depsTail"),f=j,d=e;while(d!==void 0)i=hostRead(d,"prevDep"),(+hostRead(d,"version")|0)==-1?(d===e&&(e=i),ha(d),h=hostRead(d,"prevDep"),g=hostRead(d,"nextDep"),h===void 0||(h.nextDep=g,d.prevDep=void 0),g===void 0||(g.prevDep=h,d.nextDep=void 0)):f=d,h=hostRead(d,"dep"),h.activeLink=hostRead(d,"prevActiveLink"),d.prevActiveLink=void 0,d=i;c.deps=f,c.depsTail=e}V=function(c){var p=+hostRead(c,"flags");if(0==(p&1))return hostRead(c,"fn")();c.flags=p|2,Aa(c),a(c);var ea=q,ia=v;q=c,v=!0;try{return hostRead(c,"fn")()}finally{p=q!==c,p&&w("Active effect was not restored correctly - this is likely a Vue internal bug."),b(c),q=ea,v=ia,c.flags=+hostRead(c,"flags")&-3}};Ca=function(c){var p,h=+hostRead(c,"flags");if(!(0!=(h&4)&&0==(h&16))){c.flags=h&-17,h=S;if((+hostRead(c,"globalVersion")|0)!=h){c.globalVersion=h,p=+hostRead(c,"flags");if(!(!hostRead(c,"isSSR")&&0!=(p&128)&&((hostRead(c,"deps")===void 0&&!hostRead(c,"_dirty"))||!$(c)))){c.flags=p|2,h=hostRead(c,"dep");var wb=q,xb=v;q=c,v=!0;try{a(c);var yb=hostRead(c,"fn")(hostRead(c,"_value"));0==(+hostRead(h,"version")|0)?p=!0:(p=yb,p=!hostObjectIs(p,hostRead(c,"_value"))),p&&(p=c,p.flags=+hostRead(c,"flags")|128,c._value=yb,h.version=(+hostRead(h,"version")|0)+1|0)}catch(c){h.version=(+hostRead(h,"version")|0)+1|0;throw c}finally{q=wb,v=xb,b(c),c.flags=+hostRead(c,"flags")&-3}}}}}})();function db(a,m){a.fn=m,a.deps=void 0,a.depsTail=void 0,a.flags=5,a.next=void 0,a.cleanup=void 0,a.scheduler=void 0,a.onStop=void 0,t===void 0||(hostRead(t,"active")?hostRead(t,"effects").push(a):a.flags=4)}function Ba(a,b=!1){a.flags=+hostRead(a,"flags")|8,b?(a.next=R,R=a):(a.next=Q,Q=a)}function ia(){P++}function ja(){P--;if(!(P>0)){if(R!==void 0){var a=R;R=void 0;while(a!==void 0){var p=hostRead(a,"next");a.next=void 0,a.flags=+hostRead(a,"flags")&-9,a=p}}var da;while(Q!==void 0){a=Q,Q=void 0;while(a!==void 0){var b=hostRead(a,"next");a.next=void 0,p=+hostRead(a,"flags"),a.flags=p&-9;if(0!=(p&1))try{a.trigger()}catch(p){da===void 0&&(da=p)}a=b}}if(da!==void 0)throw da}}function tb(m,c){hostInstanceOf(hostRead(m,"effect"),y)&&(m=hostRead(hostRead(m,"effect"),"fn")),m=new y(m);if(c!=null)for(var d in c)m[d]=hostRead(c,d);try{V(m)}catch(c){m.stop();throw c}c=function(){return V(m)};c.effect=m;return c}function ub(a){a=hostRead(a,"effect"),a.stop()}function Da(){M.push(v),v=!1}function vb(){M.push(v),v=!0}function Ea(){0==M.length?v=!0:(v=M[M.length-1],M.pop())}function wb(m,a=!1){hostInstanceOf(q,y)?q.cleanup=m:(m=!a,m&&w("onEffectCleanup() was called when there was no active effect to associate with."))}function aa(a){return{version:0,activeLink:void 0,subs:void 0,subsHead:void 0,map:void 0,key:void 0,sc:0,__v_skip:!0,computed:a}}var ba,Fa;(function(){function a(d,e){var f=hostCreateObject();f.effect=d;if(e!==void 0)for(d in e)f[d]=hostRead(e,d);return f}function b(d,h){return{sub:d,dep:h,version:hostRead(h,"version"),nextDep:void 0,prevDep:void 0,nextSub:void 0,prevSub:void 0,prevActiveLink:void 0}}function c(d){var h=hostRead(d,"dep");h.sc=(+hostRead(h,"sc")|0)+1|0;if(0!=(+hostRead(hostRead(d,"sub"),"flags")&4)){var e=hostRead(h,"computed");if(e!==void 0&&hostRead(h,"subs")===void 0){e.flags=+hostRead(e,"flags")|20,e=hostRead(e,"deps");while(e!==void 0)c(e),e=hostRead(e,"nextDep")}e=hostRead(h,"subs");e===d||(d.prevSub=e,e===void 0||(e.nextSub=d)),e=hostRead(h,"subsHead")===void 0,e&&(h.subsHead=d),h.subs=d}}ba=function(h,d){var i,f=hostRead(h,"computed");if(!(q===void 0||!v||q===f)){var e=hostRead(h,"activeLink");if(e===void 0||hostRead(e,"sub")!==q){e=b(q,h),h.activeLink=e,hostRead(q,"deps")===void 0?(q.deps=e,q.depsTail=e):(f=hostRead(q,"depsTail"),e.prevDep=f,f.nextDep=e,q.depsTail=e),c(e),f=0==(+hostRead(q,"flags")&4);if(f&&hostRead(h,"map")!==void 0)f=ra,i=hostRead(h,"map"),hostFinalizationRegister(f,q,{dep:h,map:i,key:hostRead(h,"key")})}else{(+hostRead(e,"version")|0)==-1&&(e.version=hostRead(h,"version"),h=hostRead(e,"nextDep"),h===void 0||(h.prevDep=hostRead(e,"prevDep"),f=hostRead(e,"prevDep"),f===void 0||(f.nextDep=h),f=hostRead(q,"depsTail"),e.prevDep=f,e.nextDep=void 0,f.nextDep=e,q.depsTail=e,hostRead(q,"deps")===e&&(q.deps=h)))}h=hostRead(q,"onTrack");h===void 0||h(a(q,d));return e}};Fa=function(h,d){ia();try{var oa=hostRead(h,"subsHead");while(oa!==void 0){var pa=hostRead(oa,"sub"),qa=hostRead(pa,"onTrigger");qa!==void 0&&0==(+hostRead(pa,"flags")&8)&&qa(a(pa,d)),oa=hostRead(oa,"nextSub")}oa=hostRead(h,"subs");while(oa!==void 0){var za=hostRead(oa,"sub"),Aa=za.notify();!0===Aa&&Fa(hostRead(za,"dep")),oa=hostRead(oa,"prevSub")}}finally{ja()}}})();function ca(h,b){h.version=(+hostRead(h,"version")|0)+1|0,S++,Fa(h,b)}function x(f,d,e){if(!(!v||q===void 0)){var g=X.get(f);g===void 0&&(g=hostCreateMap(),X.set(f,g));var h=g.get(e);h===void 0&&(h=aa(),g.set(e,h),h.map=g,h.key=e),ba(h,{target:f,type:d,key:e})}}function B(h,a){if(h!==void 0)ca(h,a)}function H(f,c,b,d,e,g){var h=X.get(f);if(h===void 0){S++;return}var a={target:f,type:c,key:b,newValue:d,oldValue:e,oldTarget:g};e=function(h){B(h,a)},ia(),"clear"===c?h.forEach(e):(e=Array.isArray(f),g=e&&_(b),e&&"length"===b?(f=+d,h.forEach(function(h,d){("length"===d||d===T||"symbol"!=typeof d&&+d>=f)&&B(h,a)})):((b!==void 0||h.has(void 0))&&B(h.get(b),a),g&&B(h.get(T),a),"add"===c?!e?(B(h.get(J),a),A(f)==Lb&&B(h.get(ga),a)):g&&B(h.get("length"),a):"delete"===c?e||(B(h.get(J),a),A(f)==Lb&&B(h.get(ga),a)):"set"===c&&A(f)==Lb&&B(h.get(J),a))),ja()}function eb(a,b){a=X.get(a);return a===void 0?void 0:a.get(b)}function Ga(e){for(var k=[],l=0;l<hostLength(e);l++)k.push(hostRead(e,l));return k}function N(a){var b=r(a);if(b===a)return b;x(b,"iterate",T);return z(a)?b:hostArrayApply("map",b,[function(b){return D(a,b)}])}function ka(a){a=r(a),x(a,"iterate",T);return a}function D(f,a){return I(f)?L(f)?pa(O(a)):pa(a):O(a)}function la(a,i){var b=ka(a),j=[],c=hostRead(b,i).apply(b,j);b=b!==a&&!z(a),i="entries"===i;var n=function(){var p,l=[],u=hostRead(c,"next").apply(c,l);!hostRead(u,"done")&&b&&(p=hostRead(u,"value"),i?p[1]=D(a,hostRead(p,1)):u.value=D(a,p));return u};return b?hostIteratorFacade(c,n):c}function E(a,i,b,c){var k=ka(a),d=k!==a&&!z(a),e=hostRead(k,i);if(e!==hostArrayPrototypeMethod(i))return a=e.apply(a,b),d?O(a):a;i=hostRead(b,0);var f=hostRead(b,1);b=k!==a?d?(0,function(c,l,d){return i.call(this,D(a,c),l,a)}):hostLength(i)>2?(0,function(c,l,d){return i.call(this,c,l,a)}):i:i,k=e.call(k,b,f);return d&&1==c?D(a,k):d&&2==c?hostArrayApply("map",k,[function(b){return D(a,b)}]):k}function Ha(b,k,e){var f=ka(b),d=f!==b&&!z(b),c=hostRead(e,0),g=d&&1==hostLength(e),a=g;d=f!==b?d?(0,function(){var d=hostRead(arguments,0);a&&(a=!1,d=D(b,d));return c.call(this,d,D(b,hostRead(arguments,1)),hostRead(arguments,2),b)}):hostLength(c)>3?(0,function(){return c.call(this,hostRead(arguments,0),hostRead(arguments,1),hostRead(arguments,2),b)}):c:c,e=Ga(e),e[0]=d,k=hostArrayApply(k,f,e);return a?D(b,k):k}function ma(a,i,e){a=r(a),x(a,"iterate",T);var c=hostRead(a,i).apply(a,e);return(c===-1||!1===c)&&oa(hostRead(e,0))?(e[0]=r(hostRead(e,0)),hostRead(a,i).apply(a,e)):c}function W(a,i,e){Da(),ia(),a=hostRead(r(a),i).apply(a,e),ja(),Ea();return a}function Ia(b){if("symbol"!=typeof b)return!1;for(var l=0;l<(+hostRead(ta,"length")|0);l++)if(hostRead(ta,l)===b)return!0;return!1}var Ja=(function(){function a(c){return"__proto__"===c||"__v_isRef"===c||"__isVue"===c||Ia(c)}function b(f,j,g){var c=ua;j&&g?c=xa:j?c=wa:g&&(c=va);return c.get(f)}return function(c,e,d,j,g){if("__v_skip"===e)return hostRead(c,e);if("__v_isReactive"===e)return!j;if("__v_isReadonly"===e)return j;if("__v_isShallow"===e)return g;if("__v_raw"===e)return d===b(c,j,g)||hostSamePrototype(c,d)?c:void 0;var f=Array.isArray(c);if(!j){var h;if(f&&(h=hostRead(s,e))!==void 0)return h;if("hasOwnProperty"===e)return Ra}u(c)&&(d=c);d=hostReflectGet(c,e,d);if(a(e))return d;j||x(c,"get",e);return g?d:u(d)?(h=hostRead(d,"value"),f&&_(e)||(d=h),j&&K(d)?na(d):d):K(d)?j?na(d):Ma(d):d}})();function fb(a,b,d,c,g){var e=hostRead(a,b),k=Array.isArray(a)&&_(b);if(!g){var f=I(e);!z(d)&&!I(d)&&(e=r(e),d=r(d));if(!k&&u(e)&&!u(d))return f?(d=Mb+hostString(b)+Kb,hostRead(a,b),w(d)):e.value=d,!0}g=U(a,b);k&&(g=+b,g=g<+hostRead(a,"length")),k=u(a)?a:c,k=hostReflectSet(a,b,d,k),k&&a===r(c)&&(!g?H(a,"add",b,d,void 0,void 0):C(d,e)&&H(a,"set",b,d,e,void 0));return k}function Ka(g){let c=hostCreateObject();c.get=function(f,d,e){return Ja(f,d,e,!1,g)},c.set=function(){let c=hostRead(arguments,0),a=hostRead(arguments,1),b=hostRead(arguments,2);return fb(c,a,b,hostRead(arguments,3),g)},c.deleteProperty=function(f,b){var a=U(f,b),c=hostRead(f,b),k=hostReflectDelete(f,b);k||(a=!1),a&&H(f,"delete",b,void 0,c,void 0);return k},c.has=function(f,b){var k=hostReflectHas(f,b);("symbol"!=typeof b||!Ia(b))&&x(f,"has",b);return k},c.ownKeys=function(f){var c=J;Array.isArray(f)&&(c="length"),x(f,"iterate",c);return hostReflectOwnKeys(f)};return c}function La(g){let c=hostCreateObject();c.get=function(f,d,e){return Ja(f,d,e,!0,g)},c.set=function(){var c=Mb+hostString(hostRead(arguments,1))+Kb;hostRead(arguments,0),w(c);return!0},c.deleteProperty=function(f,b){w("Delete operation on key \""+hostString(b)+Kb);return!0};return c}var da=(function(){function a(d,j,g){return g?d:j?pa(d):O(d)}function b(d,i,e,j,g){var f=hostRead(d,"__v_raw"),k=r(f),Rb=A(k),h=Rb==Lb,l="entries"===i||i===hostIteratorSymbol()&&h;h="keys"===i&&h,i=hostRead(f,i).apply(f,e),d=J,h&&(d=ga),j||x(k,"iterate",d);return hostIteratorFacade(i,function(){var n=[],x=hostRead(i,"next").apply(i,n);if(!hostRead(x,"done")){var d=hostRead(x,"value"),e=a(d,j,g);l&&(e=[a(hostRead(d,0),j,g),a(hostRead(d,1),j,g)]),x.value=e}return x})}function c(f,d){var e=r(d);e!==d&&hostPrototypeMethodApply(f,"has",[e])&&(f=A(f).slice(8,-1),d="Map"==f?" as keys":"",w("Reactive "+f+" contains both the raw and reactive versions of the same object"+d+", which can lead to inconsistencies. Avoid differentiating between the raw and "+"reactive versions of an object and only use the reactive version if possible."))}return function(j,g){var d=hostCreateObject();d.get=function(b){var f=hostRead(this,"__v_raw"),c=r(f),d=r(b);j||(C(b,d)&&x(c,"get",b),x(c,"get",d));if(hostPrototypeMethodApply(c,"has",[b])){var z=[b];return a(hostRead(f,"get").apply(f,z),j,g)}if(hostPrototypeMethodApply(c,"has",[d]))return b=[d],a(hostRead(f,"get").apply(f,b),j,g);if(f!==c){var I=[b];hostRead(f,"get").apply(f,I)}},hostDefineAccessor(d,"size",function(){var f=hostRead(this,"__v_raw");j||x(r(f),"iterate",J);return hostRead(f,"size")},void 0),d.has=function(b){var f=hostRead(this,"__v_raw"),c=r(f),a=r(b);j||(C(b,a)&&x(c,"has",b),x(c,"has",a));if(b===a){var i=[b];return hostRead(f,"has").apply(f,i)}var m=[b];if(hostRead(f,"has").apply(f,m))var E=!0;else{E=[a],E=!!hostRead(f,"has").apply(f,E)}return E},d.forEach=function(oc,pc){var nc=this,qc=hostRead(nc,"__v_raw"),rc=r(qc);j||x(rc,"iterate",J),rc=[function(d,b){return oc.call(pc,a(d,j,g),a(b,j,g),nc)}];return hostRead(qc,"forEach").apply(qc,rc)},j?(d.add=function(){var e=hostRead(arguments,0),b=e?"on key \""+hostString(e)+"\" ":"";e="Add",e=e+" operation "+b+"failed: target is readonly.",r(this),w(e);return this},d.set=function(){var e=hostRead(arguments,0),b=e?"on key \""+hostString(e)+"\" ":"";e="Set",e=e+" operation "+b+"failed: target is readonly.",r(this),w(e);return this},d.delete=function(){var e=hostRead(arguments,0),b=e?"on key \""+hostString(e)+"\" ":"";e="Delete",e=e+" operation "+b+"failed: target is readonly.",r(this),w(e);return!1},d.clear=function(){var e=hostRead(arguments,0),b=e?"on key \""+hostString(e)+"\" ":"";e="Clear",e=e+" operation "+b+"failed: target is readonly.",r(this),w(e)}):(d.add=function(b){var a,f=r(this),d=r(b),c=!g&&!z(b)&&!I(b)?d:b;a=hostPrototypeMethodApply(f,"has",[c])||C(b,c)&&hostPrototypeMethodApply(f,"has",[b])||C(d,c)&&hostPrototypeMethodApply(f,"has",[d]),a||(a=[c],hostRead(f,"add").apply(f,a),H(f,"add",c,c,void 0,void 0));return this},d.set=function(b,d){!g&&!z(d)&&!I(d)&&(d=r(d));var a=r(this),e=!!hostPrototypeMethodApply(a,"has",[b]);!e?(b=r(b),e=!!hostPrototypeMethodApply(a,"has",[b])):c(a,b);var ba=hostPrototypeMethodApply(a,"get",[b]),h=[b,d];hostRead(a,"set").apply(a,h),!e?H(a,"add",b,d,void 0,void 0):C(d,ba)&&H(a,"set",b,d,ba,void 0);return this},d.delete=function(b){var f=r(this),a=!!hostPrototypeMethodApply(f,"has",[b]);!a?(b=r(b),a=!!hostPrototypeMethodApply(f,"has",[b])):c(f,b);var d=void 0,e=A(f).slice(8,-1);("Map"==e||"WeakMap"==e)&&(d=hostPrototypeMethodApply(f,"get",[b])),e=[b],e=hostRead(f,"delete").apply(f,e),a&&H(f,"delete",b,void 0,d,void 0);return e},d.clear=function(){var f=r(this),a=0!=+hostCollectionSize(f),b=hostCloneCollection(f),j=[],c=hostRead(f,"clear").apply(f,j);a&&H(f,"clear",void 0,void 0,void 0,b);return c});var e=hostIteratorSymbol();d.keys=function(){return b(this,"keys",arguments,j,g)},d.values=function(){return b(this,"values",arguments,j,g)},d.entries=function(){return b(this,"entries",arguments,j,g)},d[e]=function(){return b(this,e,arguments,j,g)};return d}})();function gb(f){var a=A(f).slice(8,-1);return"Object"==a||"Array"==a?1:"Map"==a||"Set"==a||"WeakMap"==a||"WeakSet"==a?2:0}function ea(f,j,a,b,c){if(!K(f))return j=j?"readonly":"reactive",w("value cannot be made "+j+": "+hostString(f)),f;if(hostRead(f,"__v_raw")!==void 0&&!(j&&L(f)))return f;if(hostRead(f,"__v_skip")||!hostIsExtensible(f))return f;j=c.get(f);if(j!==void 0)return j;j=gb(f);if(0==j)return f;2==j&&(a=b),j=hostCreateProxy(f,a),c.set(f,j);return j}function Ma(f){return I(f)?f:ea(f,!1,Sa,Wa,ua)}function xb(f){return ea(f,!1,Ua,Xa,va)}function na(f){return ea(f,!0,Ta,Ya,wa)}function yb(f){return ea(f,!0,Va,Za,xa)}function L(d){return I(d)?L(hostRead(d,"__v_raw")):!!hostRead(d,"__v_isReactive")}function I(d){return!!hostRead(d,"__v_isReadonly")}function z(d){return!!hostRead(d,"__v_isShallow")}function oa(d){d=hostRead(d,"__v_raw");return d!==void 0&&d?!0:!1}function r(a){var b=hostRead(a,"__v_raw");return b?r(b):a}function zb(d){!U(d,"__v_skip")&&hostIsExtensible(d)&&Object.defineProperty(d,"__v_skip",{configurable:!0,enumerable:!1,writable:!1,value:!0});return d}function O(d){return K(d)?Ma(d):d}function pa(d){return K(d)?na(d):d}function u(d){return!0===hostRead(d,"__v_isRef")}function Na(d,g){if(u(d))return d;var e=hostCreateObject();if(!g){var h=r(d),f=O(d);d=h}else{f=d}h=aa();e._rawValue=d,e._value=f,e.dep=h,e.__v_isRef=!0,e.__v_isShallow=g,hostDefineAccessor(e,"value",function(){ba(h,{target:e,type:"get",key:"value"});return hostRead(e,"_value")},function(b){var d=hostRead(e,"_rawValue"),c=g||z(b)||I(b);c||(b=r(b)),C(b,d)&&(e._rawValue=b,c=!c?O(b):b,e._value=c,ca(h,{target:e,type:"set",key:"value",newValue:b,oldValue:d}))});return e}function hb(d){return Na(d,!1)}function Ab(d){return Na(d,!0)}function Bb(a){var h=hostRead(a,"dep");h===void 0||ca(h,{target:a,type:"set",key:"value",newValue:hostRead(a,"_value")})}function qa(d){return u(d)?hostRead(d,"value"):d}function Cb(o){return"function"==typeof o?o():qa(o)}function Db(a){return L(a)?a:hostCreateProxy(a,_a)}function Eb(d){let e=hostCreateObject(),h=aa();d=d(function(){ba(h)},function(){ca(h)}),e.dep=h,e._get=hostRead(d,"get"),e._set=hostRead(d,"set"),e.__v_isRef=!0,e._value=void 0,hostDefineAccessor(e,"value",function(){let d=hostRead(e,"_get")();e._value=d;return d},function(d){hostRead(e,"_set")(d)});return e}function ib(a){let k=hostCreateObject();k._getter=a,k.__v_isRef=!0,k.__v_isReadonly=!0,k._value=void 0,hostDefineAccessor(k,"value",function(){let c=a();k._value=c;return c},void 0);return k}function Oa(c,b,d){"symbol"==typeof b||(b=hostString(b));var e=r(c),a=!0;if(!Array.isArray(c)||"symbol"==typeof b||!_(b)){var k=c;while(!0){a=!oa(k)||z(k),k=hostRead(k,"__v_raw");if(!a||k===void 0||!k)break}}k=hostCreateObject();k.__v_isRef=!0,k._value=void 0,k._object=c,k._key=b,k._defaultValue=d,k._raw=e,k._shallow=a,hostDefineAccessor(k,"value",function(){var j=hostRead(c,b);a&&(j=qa(j));var l=j===void 0?d:j;k._value=l;return l},function(d){let p;if(a&&u(hostRead(e,b))){p=hostRead(c,b);if(u(p)){p.value=d;return}}c[b]=d}),hostDefineAccessor(k,"dep",function(){return eb(e,b)},void 0);return k}function Fb(a){var k=!oa(a);k&&w("toRefs() expects a reactive object but received a plain one."),k=hostCreateObject(),Array.isArray(a)&&(k=hostCreateArray(hostLength(a)));for(var b in a)k[b]=Oa(a,b,void 0);return k}function Gb(o,b,a){return u(o)?o:"function"==typeof o?ib(o):K(o)&&hostLength(arguments)>1?Oa(o,b,a):hb(o)}function Hb(e,f){var d;if("function"!=typeof e){var g=hostRead(e,"get");d=hostRead(e,"set"),e=g}g=hostCreateObject();var h=aa(g);g._value=void 0,g.dep=h,g.__v_isRef=!0,g.__v_isReadonly=d===void 0,g.deps=void 0,g.depsTail=void 0,g.flags=16,g.globalVersion=-1,g.isSSR=!1,g.next=void 0,g.effect=g,g.fn=e,g.setter=d,g.notify=function(){var p=+hostRead(g,"flags")|16;g.flags=p;if(0==(p&8)&&q!==g)return Ba(g,!0),!0},e=f!=null,e&&(g.onTrack=hostRead(f,"onTrack"),g.onTrigger=hostRead(f,"onTrigger")),hostDefineAccessor(g,"value",function(){var e=ba(h,{target:g,type:"get",key:"value"});Ca(g),e===void 0||(e.version=hostRead(h,"version"));return hostRead(g,"_value")},function(c){d!==void 0?d(c):w("Write operation failed: computed value is readonly")});return g}function Ib(){return G}function jb(a,b=!1,c){c===void 0&&(c=G),c!==void 0?(b=Z.get(c),b===void 0&&(b=[],Z.set(c,b)),b.push(a)):(a=!b,a&&w("onWatcherCleanup() was called when there was no active watcher to associate with."))}function F(d,a,b){if(a<=0||!K(d)||hostRead(d,"__v_skip"))return d;var l=b.get(d);if(l!==void 0&&+l>=a)return d;b.set(d,a),a--;if(u(d))F(hostRead(d,"value"),a,b);else if(Array.isArray(d))for(l=0;l<hostLength(d);l++)F(hostRead(d,l),a,b);else if("[object Set]"==A(d)||A(d)==Lb)d.forEach(function(d){F(d,a,b)});else if("[object Object]"==A(d)){for(l in d)F(hostRead(d,l),a,b);var c=hostEnumerableSymbols(d);for(l=0;l<hostLength(c);l++)F(hostRead(d,hostRead(c,l)),a,b)}return d}function Jb(d,a=0,b){0==a&&(a=hostInfinity()),b===void 0&&(b=hostCreateMap());return F(d,a,b)}function kb(h,i,j){j==null&&(j=hostCreateObject());var a,b,c,Sb=hostRead(j,"immediate"),n=hostRead(j,"deep"),Tb=!!hostRead(j,"once"),p=hostRead(j,"scheduler"),Ob=hostRead(j,"augmentJob"),k=hostRead(j,"call"),Nb="function"==typeof i,d=!1,f=!1,Pb=function(b){var c=hostRead(j,"onWarn");"function"==typeof c?c("Invalid watch source: ",b,"A watch source can only be a getter/effect function, a ref, a reactive object, or an array of these types."):w("Invalid watch source: A watch source can only be a getter/effect function, a ref, a reactive object, or an array of these types.")},Qb=function(d){return n?d:z(d)||!1===n||0===n?F(d,1,hostCreateMap()):F(d,hostInfinity(),hostCreateMap())};if(u(h)){var l=function(){return hostRead(h,"value")};d=z(h)}else if(L(h))l=function(){return Qb(h)},d=!0;else if(Array.isArray(h)){for(f=!0,l=0;l<hostLength(h);l++){var m=hostRead(h,l);(L(m)||z(m))&&(d=!0)}l=function(){for(var e,d,f=hostCreateArray(hostLength(h)),l=0;l<hostLength(h);l++)e=hostRead(h,l),d=void 0,u(e)?d=hostRead(e,"value"):L(e)?d=Qb(e):"function"==typeof e?d="function"==typeof k?k(e,2):e():Pb(e),f[l]=d;return f}}else{"function"==typeof h?l=Nb?"function"==typeof k?function(){return k(h,2)}:h:function(){if("function"==typeof b){Da();try{b()}finally{Ea()}}var F=G;G=a;try{return"function"==typeof k?k(h,3,[c]):h(c)}finally{G=F}}:(l=function(){},Pb(h))}if(Nb&&n){var g=+n;!0===n&&(g=hostInfinity());var Rb=function(){let c=l();return F(c,g,hostCreateMap())}}else{Rb=l}var Ub=cb();m=function(){var k=a.stop;a.stop(),Ub!==void 0&&hostRead(Ub,"active")&&bb(hostRead(Ub,"effects"),a)};var e=Y;if(f){e=hostCreateArray(hostLength(h));for(var o=0;o<hostLength(h);o++)e[o]=Y}o=function(vb){var sb,ub,wb,o;if(!(0==(+hostRead(a,"flags")&1)||!hostRead(a,"dirty")&&!vb))if(Nb){var Ga=a.run;o=a.run(),sb=!!vb||n||d;if(!sb&&f)for(ub=0;ub<hostLength(o);ub++)wb=hostRead(o,ub),C(wb,hostRead(e,ub))&&(sb=!0);else{sb=sb||C(o,e)}if(sb){"function"==typeof b&&b();var ib=G;G=a;try{var jb=e;jb===Y?jb=void 0:f&&hostRead(jb,0)===Y&&(jb=[]);var kb=[o,jb,c];e=o,"function"==typeof k?k(i,3,kb):i.apply(void 0,kb)}finally{G=ib}Tb&&m()}}else a.run()};"function"==typeof Ob&&Ob(o);a=new y(Rb),Ob="function"==typeof p?function(){p(o,!1)}:o,a.scheduler=Ob,c=function(m){jb(m,!1,a)},b=function(){var c=Z.get(a);if(c!==void 0){if("function"==typeof k)k(c,4);else{for(var D=0;D<hostLength(c);D++)hostRead(c,D)()}Z.delete(a)}},a.onStop=b,Ob=a,Ob.onTrack=hostRead(j,"onTrack"),Ob=a,Ob.onTrigger=hostRead(j,"onTrigger"),Nb?Sb?o(!0):(Ob=a,e=Ob.run()):"function"==typeof p?p(function(){o(!0)},!0):(Ob=a,Ob.run()),m.pause=function(){return a.pause()},m.resume=function(){return a.resume()},m.stop=m;return m}$a();ab(),Object.assign,Array.isArray;var d=Object.prototype,Pa=d.toString;ya(",key,ref,ref_for,ref_key,onVnodeBeforeMount,onVnodeMounted,onVnodeBeforeUpdate,onVnodeUpdated,onVnodeBeforeUnmount,onVnodeUnmounted"),ya("bind,cloak,else-if,else,for,html,if,model,on,once,pre,show,slot,text,memo"),new RegExp("-\\w","g");var c=null;Object.create(c),new RegExp("\\B([A-Z])","g"),Object.create(c),Object.create(c),Object.create(c),new RegExp("^[_$a-zA-Z\\xA0-\\uFFFF][_$a-zA-Z0-9\\xA0-\\uFFFF]*$");var lb={GET:"get",HAS:"has",ITERATE:"iterate"},mb={SET:"set",ADD:"add",DELETE:"delete",CLEAR:"clear"},nb={SKIP:"__v_skip",IS_REACTIVE:"__v_isReactive",IS_READONLY:"__v_isReadonly",IS_SHALLOW:"__v_isShallow",RAW:"__v_raw",IS_REF:"__v_isRef"},t=void 0,Qa=hostCreateEffectScopeClass("EffectScope",(0,function(){var e=!!hostRead(arguments,0);this.detached=e,this._active=!0,this._on=0,this.effects=[],this.cleanups=[],this._isPaused=!1,this._warnOnRun=!0,this.parent=void 0,this.scopes=void 0,this.index=void 0,this.prevScope=void 0,this.__v_skip=!0;if(!e&&t!==void 0)if(hostRead(t,"active")){this.parent=t,e=hostRead(t,"scopes"),e===void 0&&(e=[],t.scopes=e);var l=hostLength(e);e.push(this),this.index=l}else this._active=!1,this._warnOnRun=!1}),(0,function(){return hostRead(this,"_active")}),(0,function(){if(hostRead(this,"_active")){this._isPaused=!0;var l=hostRead(this,"scopes");if(l!==void 0){var b=l.slice();for(l=0;l<hostLength(b);l++){var c=hostRead(b,l);c.pause()}}l=hostRead(this,"effects");for(var a=0;a<hostLength(l);a++)b=hostRead(l,a),b.pause()}}),(0,function(){var l;if(hostRead(this,"_active")&&hostRead(this,"_isPaused")){this._isPaused=!1,l=hostRead(this,"scopes");if(l!==void 0){var b=l.slice();for(l=0;l<hostLength(b);l++){var c=hostRead(b,l);c.resume()}}l=hostRead(this,"effects").slice();for(var a=0;a<hostLength(l);a++)b=hostRead(l,a),b.resume()}}),(0,function(m){if(hostRead(this,"_active")){var k=t;try{t=this;return m()}finally{t=k}}var a=!!hostRead(this,"_warnOnRun");a&&w("cannot run an inactive effect scope.")}),(0,function(){var b=(+hostRead(this,"_on")|0)+1|0;this._on=b,1==b&&(this.prevScope=t,t=this)}),(0,function(){var b=+hostRead(this,"_on")|0;if(b>0){b--,this._on=b;if(0==b){if(t===this)t=hostRead(this,"prevScope");else{b=t;while(b!==void 0){if(hostRead(b,"prevScope")===this){b.prevScope=hostRead(this,"prevScope");break}b=hostRead(b,"prevScope")}}this.prevScope=void 0}}}),(0,function(b){if(!(!hostRead(this,"_active"))){this._active=!1;for(var d,e,c=hostRead(this,"effects"),l=0;l<hostLength(c);l++)d=hostRead(c,l),d.stop();c.length=0;for(c=hostRead(this,"cleanups"),l=0;l<hostLength(c);l++)hostRead(c,l)();c.length=0,c=hostRead(this,"scopes");if(c!==void 0){for(d=c.slice(),l=0;l<hostLength(d);l++)e=hostRead(d,l),e.stop(!0);c.length=0}l=hostRead(this,"parent");!hostRead(this,"detached")&&l!==void 0&&!b&&(c=hostRead(l,"scopes"),b=c.pop(),b!==void 0&&b!==this&&(l=hostRead(this,"index"),c[l]=b,b.index=l)),this.parent=void 0}})),q=void 0,ob={"1":"ACTIVE","2":"RUNNING","4":"TRACKING","8":"NOTIFIED","16":"DIRTY","32":"ALLOW_RECURSE","64":"PAUSED","128":"EVALUATED",ACTIVE:1,RUNNING:2,TRACKING:4,NOTIFIED:8,DIRTY:16,ALLOW_RECURSE:32,PAUSED:64,EVALUATED:128},fa=hostCreateWeakSet(),P=0,Q=void 0,R=void 0,v=!0,M=[],y=hostCreateClass1("ReactiveEffect",function(m){db(this,m)});c=y.prototype,hostDefineMethod(c,"pause",function(){this.flags=+hostRead(this,"flags")|64}),c=y.prototype,hostDefineMethod(c,"resume",function(){var p=+hostRead(this,"flags");0!=(p&64)&&(this.flags=p&-65,!fa.has(this)||(fa.delete(this),this.trigger()))}),c=y.prototype,hostDefineMethod(c,"notify",function(){var p=+hostRead(this,"flags");if(!(0!=(p&2)&&0==(p&32)))0==(p&8)&&Ba(this)}),c=y.prototype,hostDefineMethod(c,"run",function(){return V(this)}),c=y.prototype,hostDefineMethod(c,"stop",function(){if(0!=(+hostRead(this,"flags")&1)){var b=hostRead(this,"deps");while(b!==void 0)ha(b),b=hostRead(b,"nextDep");this.deps=void 0,this.depsTail=void 0,Aa(this),b=hostRead(this,"onStop"),b===void 0||b(),this.flags=+hostRead(this,"flags")&-2}}),c=y.prototype,hostDefineMethod(c,"trigger",function(){if(0!=(+hostRead(this,"flags")&64))fa.add(this);else{var d=hostRead(this,"scheduler");d!==void 0?d():$(this)&&V(this)}}),c=y.prototype,hostDefineMethod(c,"runIfDirty",function(){$(this)&&V(this)}),c=y.prototype,hostDefineAccessor(c,"dirty",function(){return $(this)},void 0);var S=0,ra=hostCreateFinalizationRegistry(function(a){var h=hostRead(a,"dep"),c=hostRead(a,"map"),b=hostRead(a,"key");hostRead(h,"subs")===void 0&&c.get(b)===h&&c.delete(b)}),X=hostCreateWeakMap(),J=hostSymbol("Object iterate"),ga=hostSymbol("Map keys iterate"),T=hostSymbol("Array iterate"),s=hostCreateNullObject(),sa=hostIteratorSymbol();s[sa]=function(){return la(this,sa)},s.concat=function(){for(var d,e=Ga(arguments),l=0;l<hostLength(e);l++)d=hostRead(e,l),Array.isArray(d)&&(e[l]=N(d));return hostArrayApply("concat",N(this),e)},s.entries=function(){return la(this,"entries")},s.every=function(){return E(this,"every",arguments,0)},s.filter=function(){return E(this,"filter",arguments,2)},s.find=function(){return E(this,"find",arguments,1)},s.findIndex=function(){return E(this,"findIndex",arguments,0)},s.findLast=function(){return E(this,"findLast",arguments,1)},s.findLastIndex=function(){return E(this,"findLastIndex",arguments,0)},s.forEach=function(){return E(this,"forEach",arguments,0)},s.includes=function(){return ma(this,"includes",arguments)},s.indexOf=function(){return ma(this,"indexOf",arguments)},s.join=function(){return hostArrayApply("join",N(this),arguments)},s.lastIndexOf=function(){return ma(this,"lastIndexOf",arguments)},s.map=function(){return E(this,"map",arguments,0)},s.pop=function(){return W(this,"pop",arguments)},s.push=function(){return W(this,"push",arguments)},s.reduce=function(){return Ha(this,"reduce",arguments)},s.reduceRight=function(){return Ha(this,"reduceRight",arguments)},s.shift=function(){return W(this,"shift",arguments)},s.some=function(){return E(this,"some",arguments,0)},s.splice=function(){return W(this,"splice",arguments)},s.toReversed=function(){return hostArrayApply("toReversed",N(this),arguments)},s.toSorted=function(){return hostArrayApply("toSorted",N(this),arguments)},s.toSpliced=function(){return hostArrayApply("toSpliced",N(this),arguments)},s.unshift=function(){return W(this,"unshift",arguments)},s.values=function(){return la(this,"values")};var ta=hostBuiltInSymbols(),Ra=(0,function(b){"symbol"==typeof b||(b=hostString(b));var a=r(this);x(a,"has",b);return U(a,b)}),Sa=Ka(!1),Ta=La(!1),Ua=Ka(!0),Va=La(!0);c=da(!1,!1);var Wa={get:function(d,b,e){if("__v_isReactive"===b)return!!1;if("__v_isReadonly"===b)return!1;if("__v_raw"===b)return d;var o=U(c,b)&&b in d?c:d;return hostReflectGet(o,b,e)}};d=da(!1,!0);var Xa={get:function(n,b,e){if("__v_isReactive"===b)return!!1;if("__v_isReadonly"===b)return!1;if("__v_raw"===b)return n;var o=U(d,b)&&b in n?d:n;return hostReflectGet(o,b,e)}},e=da(!0,!1),Ya={get:function(d,b,c){if("__v_isReactive"===b)return!!0;if("__v_isReadonly"===b)return!0;if("__v_raw"===b)return d;var o=U(e,b)&&b in d?e:d;return hostReflectGet(o,b,c)}},f=da(!0,!0),Za={get:function(d,b,e){if("__v_isReactive"===b)return!!0;if("__v_isReadonly"===b)return!0;if("__v_raw"===b)return d;var o=U(f,b)&&b in d?f:d;return hostReflectGet(o,b,e)}},ua=hostCreateWeakMap(),va=hostCreateWeakMap(),wa=hostCreateWeakMap(),xa=hostCreateWeakMap(),_a={get:function(f,b,a){return"__v_raw"===b?f:qa(hostReflectGet(f,b,a))},set:function(){var f=hostRead(arguments,0),b=hostRead(arguments,1),d=hostRead(arguments,2),c=hostRead(arguments,3),e=hostRead(f,b);return u(e)&&!u(d)?(e.value=d,!0):hostReflectSet(f,b,d,c)}},pb={"2":"WATCH_GETTER","3":"WATCH_CALLBACK","4":"WATCH_CLEANUP",WATCH_GETTER:2,WATCH_CALLBACK:3,WATCH_CLEANUP:4},Y=hostCreateObject(),Z=hostCreateWeakMap(),G=void 0,qb=hostFunction2Rest(function(e){let c=hostRead(e,0),a=hostRead(e,1);return kb(c,a,hostRead(e,2))});Object.defineProperty(Hb,"name",{configurable:true,value:"computed"});Object.defineProperty(Eb,"name",{configurable:true,value:"customRef"});Object.defineProperty(tb,"name",{configurable:true,value:"effect"});Object.defineProperty(rb,"name",{configurable:true,value:"effectScope"});Object.defineProperty(vb,"name",{configurable:true,value:"enableTracking"});Object.defineProperty(cb,"name",{configurable:true,value:"getCurrentScope"});Object.defineProperty(Ib,"name",{configurable:true,value:"getCurrentWatcher"});Object.defineProperty(oa,"name",{configurable:true,value:"isProxy"});Object.defineProperty(L,"name",{configurable:true,value:"isReactive"});Object.defineProperty(I,"name",{configurable:true,value:"isReadonly"});Object.defineProperty(u,"name",{configurable:true,value:"isRef"});Object.defineProperty(z,"name",{configurable:true,value:"isShallow"});Object.defineProperty(zb,"name",{configurable:true,value:"markRaw"});Object.defineProperty(wb,"name",{configurable:true,value:"onEffectCleanup"});Object.defineProperty(sb,"name",{configurable:true,value:"onScopeDispose"});Object.defineProperty(jb,"name",{configurable:true,value:"onWatcherCleanup"});Object.defineProperty(Da,"name",{configurable:true,value:"pauseTracking"});Object.defineProperty(Db,"name",{configurable:true,value:"proxyRefs"});Object.defineProperty(Ma,"name",{configurable:true,value:"reactive"});Object.defineProperty(N,"name",{configurable:true,value:"reactiveReadArray"});Object.defineProperty(na,"name",{configurable:true,value:"readonly"});Object.defineProperty(hb,"name",{configurable:true,value:"ref"});Object.defineProperty(Ea,"name",{configurable:true,value:"resetTracking"});Object.defineProperty(xb,"name",{configurable:true,value:"shallowReactive"});Object.defineProperty(ka,"name",{configurable:true,value:"shallowReadArray"});Object.defineProperty(yb,"name",{configurable:true,value:"shallowReadonly"});Object.defineProperty(Ab,"name",{configurable:true,value:"shallowRef"});Object.defineProperty(ub,"name",{configurable:true,value:"stop"});Object.defineProperty(r,"name",{configurable:true,value:"toRaw"});Object.defineProperty(O,"name",{configurable:true,value:"toReactive"});Object.defineProperty(pa,"name",{configurable:true,value:"toReadonly"});Object.defineProperty(Gb,"name",{configurable:true,value:"toRef"});Object.defineProperty(Fb,"name",{configurable:true,value:"toRefs"});Object.defineProperty(Cb,"name",{configurable:true,value:"toValue"});Object.defineProperty(x,"name",{configurable:true,value:"track"});Object.defineProperty(Jb,"name",{configurable:true,value:"traverse"});Object.defineProperty(H,"name",{configurable:true,value:"trigger"});Object.defineProperty(Bb,"name",{configurable:true,value:"triggerRef"});Object.defineProperty(qa,"name",{configurable:true,value:"unref"});Object.defineProperty(qb,"name",{configurable:true,value:"watch"});export{T as ARRAY_ITERATE_KEY,ob as EffectFlags,Qa as EffectScope,J as ITERATE_KEY,ga as MAP_KEY_ITERATE_KEY,y as ReactiveEffect,nb as ReactiveFlags,lb as TrackOpTypes,mb as TriggerOpTypes,pb as WatchErrorCodes,Hb as computed,Eb as customRef,tb as effect,rb as effectScope,vb as enableTracking,cb as getCurrentScope,Ib as getCurrentWatcher,oa as isProxy,L as isReactive,I as isReadonly,u as isRef,z as isShallow,zb as markRaw,wb as onEffectCleanup,sb as onScopeDispose,jb as onWatcherCleanup,Da as pauseTracking,Db as proxyRefs,Ma as reactive,N as reactiveReadArray,na as readonly,hb as ref,Ea as resetTracking,xb as shallowReactive,ka as shallowReadArray,yb as shallowReadonly,Ab as shallowRef,ub as stop,r as toRaw,O as toReactive,pa as toReadonly,Gb as toRef,Fb as toRefs,Cb as toValue,x as track,Jb as traverse,H as trigger,Bb as triggerRef,qa as unref,qb as watch,ja as endBatch,eb as getDepFromReactive,ia as startBatch,X as targetMap}