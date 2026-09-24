const descriptor = Object.getOwnPropertyDescriptor(String.prototype, "indexOf");
const sentinel = {};
let mode;
let depth = 0;
function install(label, throwLookup = false) {
  Object.defineProperty(String.prototype, "indexOf", {
    configurable: true,
    get() {
      events.push(["get", label]);
      if (throwLookup) throw sentinel;
      const callable = function (...args) {
        events.push([
          "call", label, String(this), args.length, args[0],
          args.length === 1 ? "omitted" : args[1] === undefined ? "undefined" : args[1]
        ]);
        if (mode === "overflow") return 4294967297;
        if (mode === "coerce") return { valueOf() { events.push(["coerce"]); throw sentinel; } };
        return args.length;
      };
      Object.defineProperty(callable, "call", { get() { throw Error("unexpected .call lookup"); } });
      return callable;
    }
  });
}
globalThis.needle = () => {
  events.push(["needle", mode]);
  if (mode === "argument-throw") throw sentinel;
  if (mode === "replace") install("new");
  if (mode === "reenter" && depth === 0) {
    depth++;
    events.push(["nested", library.explicit("inner")]);
    depth--;
  }
  return "x";
};
globalThis.position = () => {
  events.push(["position"]);
  return mode === "undefined" ? undefined : 0;
};
try {
  for (const name of ["omitted", "explicit", "undefined", "replace"]) {
    mode = name;
    install(name === "replace" ? "old" : name);
    events.push(["result", name === "explicit" || name === "undefined"
      ? library.explicit("abc") : library.omitted("abc")]);
  }
  mode = "after-replace";
  events.push(["result", library.omitted("abc")]);
  mode = "getter-throw";
  install("throw", true);
  try { library.omitted("abc"); } catch (error) { events.push(["caught", error === sentinel]); }
  mode = "argument-throw";
  install("arg");
  try { library.omitted("abc"); } catch (error) { events.push(["caught", error === sentinel]); }
  mode = "reenter";
  install("reentry");
  events.push(["result", library.omitted("outer")]);
  mode = "overflow";
  install("overflow");
  events.push(["result", library.omitted("abc")]);
  mode = "coerce";
  install("coerce");
  try { library.omitted("abc"); } catch (error) { events.push(["caught", error === sentinel]); }
} finally {
  Object.defineProperty(String.prototype, "indexOf", descriptor);
}
