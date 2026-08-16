"use strict";

const encoded = process.env.LILSCRIPT_ALGORITHM_VECTOR;
if (!encoded) {
  throw new Error("LILSCRIPT_ALGORITHM_VECTOR is required");
}

const vector = JSON.parse(encoded);
const integers = Array.isArray(vector.ints) ? vector.ints : [];
const strings = Array.isArray(vector.strings) ? vector.strings : [];
const traceEnabled = process.env.LILSCRIPT_ALGORITHM_TRACE === "1";
const accessTrace = [];

if (traceEnabled) {
  process.once("exit", () => {
    process.stderr.write(`LILSCRIPT_ALGORITHM_TRACE=${JSON.stringify(accessTrace)}\n`);
  });
}

globalThis.algorithmInt = function algorithmInt(index) {
  if (!Number.isInteger(index) || index < 0 || index >= integers.length) {
    throw new RangeError(`algorithmInt index ${index} is out of range`);
  }
  const value = integers[index];
  if (!Number.isInteger(value) || value < -2147483648 || value > 2147483647) {
    throw new TypeError(`algorithmInt value ${value} is not signed i32`);
  }
  accessTrace.push(["int", index]);
  return value;
};

globalThis.algorithmString = function algorithmString(index) {
  if (!Number.isInteger(index) || index < 0 || index >= strings.length) {
    throw new RangeError(`algorithmString index ${index} is out of range`);
  }
  const value = strings[index];
  if (typeof value !== "string") {
    throw new TypeError(`algorithmString value at ${index} is not a string`);
  }
  accessTrace.push(["string", index]);
  return value;
};

globalThis.algorithmCount = function algorithmCount() {
  accessTrace.push(["count"]);
  return Math.max(integers.length, strings.length);
};
