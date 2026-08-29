import { createRequire } from "node:module";

const require = createRequire(import.meta.url);

// ECMAScript object and module-loading primitives used by SFC owners.
export function hostCreateNullObject() {
  return Object.create(null);
}

export function hostCreateObject(prototype) {
  return Object.create(prototype);
}

export function hostDefineProperty(target, key, descriptor) {
  return Object.defineProperty(target, key, descriptor);
}

export function hostAssign(target, ...sources) {
  return Object.assign(target, ...sources);
}

export function hostKeys(target) {
  return Object.keys(target);
}

export function hostHasProperty(target, key) {
  return key in target;
}

export function hostArrayFrom(value) {
  return Array.from(value);
}

export function hostCreateMap() {
  return new Map();
}

export function hostCreateSet(value) {
  return new Set(value);
}

export function hostRequire(id) {
  return require(id);
}

export function hostJsonParse(source) {
  return JSON.parse(source);
}

export function hostJsonStringify(value) {
  return JSON.stringify(value);
}

export function hostApply(fn, receiver, args) {
  return Reflect.apply(fn, receiver, args);
}

export function hostCreateError(message) {
  return new Error(message);
}
