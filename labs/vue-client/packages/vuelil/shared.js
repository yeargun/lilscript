import {
  camelize,
  capitalize,
  normalizeClass,
  normalizeProps,
  normalizeStyle,
  toDisplayString,
  toHandlerKey,
} from "../../artifacts/shared-runtime.generated.js";

for (const [value, name] of [
  [camelize, ""],
  [capitalize, ""],
  [normalizeClass, "normalizeClass"],
  [normalizeProps, "normalizeProps"],
  [normalizeStyle, "normalizeStyle"],
  [toDisplayString, "toDisplayString"],
  [toHandlerKey, ""],
]) {
  Object.defineProperty(value, "name", { configurable: true, value: name });
}

export * from "../../artifacts/shared-runtime.generated.js";
