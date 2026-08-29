// The reactivity kernel owns policy. This file only exposes ECMAScript
// operations that LilScript cannot currently express with a typed ABI.

export function hostCreateObject() {
  return {};
}

export function hostCreateNullObject() {
  return Object.create(null);
}

export function hostCreateArray(length) {
  return new Array(length);
}

export function hostCreateWeakMap() {
  return new WeakMap();
}

export function hostCreateWeakSet() {
  return new WeakSet();
}

export function hostCreateMap() {
  return new Map();
}

export function hostCreateFinalizationRegistry(callback) {
  return new FinalizationRegistry(callback);
}

export function hostFinalizationRegister(registry, target, heldValue) {
  registry.register(target, heldValue);
}

export function hostCreateProxy(target, handler) {
  return new Proxy(target, handler);
}

export function hostCreateClass0(name, initialize) {
  return {
    [name]: class {
      constructor() {
        Reflect.apply(initialize, this, arguments);
      }
    },
  }[name];
}

export function hostCreateClass1(name, initialize) {
  return {
    [name]: class {
      constructor(value) {
        Reflect.apply(initialize, this, [value]);
      }
    },
  }[name];
}

export function hostCreateEffectScopeClass(
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

export function hostFunction2Rest(callback) {
  return function (first, second) {
    return Reflect.apply(callback, this, [arguments]);
  };
}

export function hostRead(value, key) {
  return value == null ? undefined : value[key];
}

export function hostReflectGet(target, key, receiver) {
  return Reflect.get(target, key, receiver);
}

export function hostReflectSet(target, key, value, receiver) {
  return Reflect.set(target, key, value, receiver);
}

export function hostReflectDelete(target, key) {
  return Reflect.deleteProperty(target, key);
}

export function hostReflectHas(target, key) {
  return Reflect.has(target, key);
}

export function hostReflectOwnKeys(target) {
  return Reflect.ownKeys(target);
}

export function hostDefineAccessor(object, key, get, set) {
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

export function hostDefineHidden(object, key, value) {
  Object.defineProperty(object, key, {
    configurable: true,
    enumerable: false,
    writable: true,
    value,
  });
}

export function hostDefineMethod(object, key, value) {
  Object.defineProperty(value, "name", { configurable: true, value: String(key) });
  Object.defineProperty(object, key, {
    configurable: true,
    enumerable: false,
    writable: true,
    value,
  });
}

export function hostObjectIs(left, right) {
  return Object.is(left, right);
}

export function hostInstanceOf(value, constructor) {
  return value instanceof constructor;
}

export function hostIsExtensible(value) {
  return Object.isExtensible(value);
}

export function hostHasOwn(target, key) {
  return Object.prototype.hasOwnProperty.call(target, key);
}

export function hostRawType(value) {
  return Object.prototype.toString.call(value).slice(8, -1);
}

export function hostSamePrototype(left, right) {
  return Object.getPrototypeOf(left) === Object.getPrototypeOf(right);
}

export function hostParseInt(value) {
  return parseInt(value, 10);
}

export function hostBuiltInSymbols() {
  return Object.getOwnPropertyNames(Symbol)
    .filter(name => name !== "arguments" && name !== "caller")
    .map(name => Symbol[name])
    .filter(value => typeof value === "symbol");
}

export function hostSymbol(description) {
  return Symbol(description);
}

export function hostIteratorSymbol() {
  return Symbol.iterator;
}

export function hostToNumber(value) {
  return Number(value);
}

export function hostString(value) {
  return String(value);
}

export function hostArrayApply(method, receiver, args) {
  return Reflect.apply(Array.prototype[method], receiver, args);
}

export function hostArrayPrototypeMethod(method) {
  return Array.prototype[method];
}

export function hostPrototypeMethodApply(receiver, method, args) {
  return Reflect.apply(Object.getPrototypeOf(receiver)[method], receiver, args);
}

export function hostCollectionSize(receiver) {
  return Reflect.get(Object.getPrototypeOf(receiver), "size", receiver);
}

export function hostCloneCollection(collection) {
  return collection instanceof Map ? new Map(collection) : new Set(collection);
}

export function hostInfinity() {
  return Infinity;
}

export function hostIteratorFacade(inner, next) {
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

export function hostLength(value) {
  return value.length;
}

export function hostEnumerableSymbols(value) {
  return Object.getOwnPropertySymbols(value).filter(symbol =>
    Object.prototype.propertyIsEnumerable.call(value, symbol),
  );
}

export function hostWarn(message, args) {
  console.warn(`[Vue warn] ${message}`, ...Array.prototype.slice.call(args, 1));
}
