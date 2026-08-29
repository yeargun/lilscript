// ECMAScript primitives used by the LilScript runtime-core kernel.

export function hostCreateObject() {
  return {};
}

export function hostCreateNullObject() {
  return Object.create(null);
}

export function hostObjectCreate(prototype) {
  return Object.create(prototype);
}

export function hostCreateArray(length) {
  return new Array(length);
}

export function hostCreateMap() {
  return new Map();
}

export function hostMapGet(map, key) {
  return map.get(key);
}

export function hostMapHas(map, key) {
  return map.has(key);
}

export function hostMapSet(map, key, value) {
  map.set(key, value);
}

export function hostCreateSet(values) {
  return new Set(values);
}

export function hostCreateWeakSet() {
  return new WeakSet();
}

export function hostCreateWeakMap() {
  return new WeakMap();
}

export function hostCreateProxy(target, handler) {
  return new Proxy(target, handler);
}

export function hostWeakSetHas(set, value) {
  return set.has(value);
}

export function hostWeakSetAdd(set, value) {
  set.add(value);
}

export function hostArrayFrom(value) {
  return Array.from(value);
}

export function hostArraySlice(value, start) {
  return Array.prototype.slice.call(value, start);
}

export function hostArrayInsert(array, index, value) {
  array.splice(index, 0, value);
}

export function hostArrayRemove(array, index) {
  array.splice(index, 1);
}

export function hostArrayConcat(left, right) {
  return Array.prototype.concat.call([], left, right);
}

export function hostArraySort(array, compare) {
  return array.sort(compare);
}

export function hostArrayIndexOf(array, value) {
  return array.indexOf(value);
}

export function hostArrayIncludes(array, value) {
  return array.includes(value);
}

export function hostObjectKeys(value) {
  return Object.keys(value);
}

export function hostReflectOwnKeys(value) {
  return Reflect.ownKeys(value);
}

export function hostEnumerableKeys(value) {
  const keys = [];
  for (const key in value) keys.push(key);
  return keys;
}

export function hostAssign(target, source) {
  return Object.assign(target, source);
}

export function hostHasOwn(value, key) {
  return Object.prototype.hasOwnProperty.call(value, key);
}

export function hostHasProperty(value, key) {
  return key in value;
}

export function hostGetPrototype(value) {
  return Object.getPrototypeOf(value);
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

export function hostDefineAccessor(value, key, get, set) {
  if (typeof get === "function") {
    Object.defineProperty(get, "name", { configurable: true, value: `get ${String(key)}` });
  }
  if (typeof set === "function") {
    Object.defineProperty(set, "name", { configurable: true, value: `set ${String(key)}` });
  }
  Object.defineProperty(value, key, {
    configurable: true,
    enumerable: true,
    get,
    set,
  });
}

export function hostDefineData(value, key, next) {
  Object.defineProperty(value, key, {
    configurable: true,
    enumerable: true,
    writable: true,
    value: next,
  });
}

export function hostSetFunctionName(fn, name) {
  Object.defineProperty(fn, "name", { configurable: true, value: name });
  return fn;
}

export function hostSymbolFor(key) {
  return Symbol.for(key);
}

export function hostIteratorSymbol() {
  return Symbol.iterator;
}

export function hostPromiseResolve() {
  return Promise.resolve();
}

export function hostPromiseThen(promise, callback) {
  return promise.then(callback);
}

export function hostBind(fn, receiver) {
  return fn.bind(receiver);
}

export function hostFunction1Rest(callback) {
  return function (first) {
    return Reflect.apply(callback, undefined, [this, arguments]);
  };
}

export function hostCallMethod0(receiver, name) {
  return Reflect.apply(receiver[name], receiver, []);
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

export function hostCall5(fn, first, second, third, fourth, fifth) {
  return Reflect.apply(fn, undefined, [first, second, third, fourth, fifth]);
}

export function hostCall6(fn, first, second, third, fourth, fifth, sixth) {
  return Reflect.apply(fn, undefined, [first, second, third, fourth, fifth, sixth]);
}

export function hostApply(fn, receiver, args) {
  return Reflect.apply(fn, receiver, args);
}

export function hostNumber(value) {
  return Number(value);
}

export function hostString(value) {
  return String(value);
}

export function hostNumberIsInteger(value) {
  return Number.isInteger(value);
}

export function hostIsDev() {
  return typeof globalThis.__DEV__ !== "boolean" || globalThis.__DEV__;
}

export function hostObjectIs(left, right) {
  return Object.is(left, right);
}

export function hostInfinity() {
  return Infinity;
}

export function hostWarn(message) {
  console.warn(`[Vue warn]: ${message}`);
}
