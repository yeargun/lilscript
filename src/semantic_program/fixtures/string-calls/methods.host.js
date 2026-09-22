const originals = {
  slice: Object.getOwnPropertyDescriptor(String.prototype, "slice"),
  split: Object.getOwnPropertyDescriptor(String.prototype, "split")
};
const sentinel = {};
let mode = "plain";
let depth = 0;
function install(kind, label, behavior = "callable") {
  Object.defineProperty(String.prototype, kind, {
    configurable: true,
    get() {
      events.push(["get", kind, label]);
      if (behavior === "getter-throw") throw sentinel;
      if (behavior === "noncallable") return 0;
      const callable = function (...args) {
        events.push(["call", kind, label, String(this), args.length,
          args[0], kind === "split" ? "separator-only" :
          args.length === 1 ? "omitted" : args[1] === undefined ? "undefined" : args[1]]);
        if (mode === "call-throw") throw sentinel;
        return kind === "slice" ? label + ":" + String(this) : [label, String(this)];
      };
      Object.defineProperty(callable, "call", { get() { throw Error("unexpected callable.call lookup"); } });
      return callable;
    }
  });
}
globalThis.receiver = () => { events.push(["receiver", depth]); return depth ? "inner" : "outer"; };
globalThis.begin = () => {
  events.push(["begin", depth]);
  if (mode === "begin-throw") throw sentinel;
  if (mode === "replace-slice") install("slice", "new");
  if (mode === "reenter" && depth === 0) {
    depth++;
    events.push(["nested", library.sliceExplicit()]);
    depth--;
  }
  return 1;
};
globalThis.ending = () => {
  events.push(["ending", depth]);
  if (mode === "ending-throw") throw sentinel;
  return mode === "undefined" ? undefined : 3;
};
globalThis.separator = () => {
  events.push(["separator", depth]);
  if (mode === "separator-throw") throw sentinel;
  if (mode === "replace-split") install("split", "new");
  return "/";
};
function run(name, kind, method, nextMode = "plain", behavior = "callable", label = name) {
  events.push(["case", name]);
  mode = nextMode;
  install(kind, label, behavior);
  try {
    const value = library[method]();
    events.push(["result", value === undefined ? "void" : value]);
  } catch (error) {
    events.push(["caught", error === sentinel ? "sentinel" : error instanceof TypeError ? "TypeError" : "unexpected"]);
  }
}
try {
  run("omitted", "slice", "sliceOmitted");
  run("explicit", "slice", "sliceExplicit");
  run("undefined", "slice", "sliceExplicit", "undefined");
  run("replacement", "slice", "sliceExplicit", "replace-slice", "callable", "old");
  events.push(["case", "after-replacement"]); mode = "plain";
  events.push(["result", library.sliceOmitted()]);
  run("getter-throw", "slice", "sliceExplicit", "plain", "getter-throw");
  run("begin-throw", "slice", "sliceExplicit", "begin-throw");
  run("ending-throw", "slice", "sliceExplicit", "ending-throw");
  run("noncallable", "slice", "sliceExplicit", "plain", "noncallable");
  run("call-throw", "slice", "sliceExplicit", "call-throw");
  run("reentry", "slice", "sliceExplicit", "reenter");
  run("discard-slice", "slice", "unusedSlice");
  run("discard-slice-throws", "slice", "unusedSlice", "call-throw");
  run("split", "split", "splitValue");
  run("replace-split", "split", "splitValue", "replace-split", "callable", "old");
  run("split-getter-throw", "split", "splitValue", "plain", "getter-throw");
  run("separator-throw", "split", "splitValue", "separator-throw");
  run("split-noncallable", "split", "splitValue", "plain", "noncallable");
  run("discard-split", "split", "unusedSplit");
  run("discard-split-throws", "split", "unusedSplit", "call-throw");
} finally {
  for (const kind of ["slice", "split"]) Object.defineProperty(String.prototype, kind, originals[kind]);
}
