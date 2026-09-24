globalThis.receiver = () => "receiver";
globalThis.begin = () => 1;
globalThis.separator = () => "/";
const originalSlice = Object.getOwnPropertyDescriptor(String.prototype, "slice");
const originalSplit = Object.getOwnPropertyDescriptor(String.prototype, "split");
const sentinel = {};
let mode;
const splitResult = { get length() {
  events.push(["length-get"]);
  if (mode === "split-getter-throw") throw sentinel;
  return 7;
} };
const sliceResult = { [Symbol.toPrimitive](hint) {
  events.push(["coerce", hint]);
  if (mode === "slice-coerce-throw") throw sentinel;
  return "coerced";
} };
try {
  Object.defineProperty(String.prototype, "split", { configurable: true, value: function (separator) {
    events.push(["split-call", String(this), arguments.length, separator]);
    return mode === "split-null" ? null : splitResult;
  } });
  Object.defineProperty(String.prototype, "slice", { configurable: true, value: function (begin) {
    events.push(["slice-call", String(this), arguments.length, begin]);
    return sliceResult;
  } });
  for (const [name, method] of [
    ["split-object", "discardSplitLength"],
    ["split-null", "discardSplitLength"],
    ["split-getter-throw", "discardSplitLength"],
    ["slice-coerce", "coercedSlice"],
    ["slice-coerce-discard", "discardCoercedSlice"],
    ["slice-coerce-throw", "discardCoercedSlice"]
  ]) {
    mode = name;
    events.push(["case", name]);
    try {
      const result = library[method]();
      events.push(["result", result === undefined ? "void" : result]);
    } catch (error) {
      events.push(["caught", error === sentinel ? "sentinel" : error instanceof TypeError ? "TypeError" : "unexpected"]);
    }
  }
} finally {
  Object.defineProperty(String.prototype, "slice", originalSlice);
  Object.defineProperty(String.prototype, "split", originalSplit);
}
