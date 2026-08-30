// Node's conditional stream construction cannot be expressed in LilScript.

export function hostCreateNodeReadable() {
  const stream = typeof process !== "undefined" && process.getBuiltinModule
    ? process.getBuiltinModule("node:stream")
    : typeof require === "function"
      ? require("node:stream")
      : null;
  if (!stream) return null;
  const { Readable } = stream;
  return new Readable({ read() {} });
}
