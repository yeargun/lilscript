// Host of shared_decoders.lil (a copy of data_tables.host.js): every observation a program can make of a data
// graph (enumeration order, Object.is, prototypes, property descriptors,
// extensibility), printed per table; `keep` holds a second reference so each
// table stays a root binding.
globalThis.inspect = function inspect(name, root) {
  const seen = new Set();
  function show(v) {
    if (v === null) return "null";
    if (typeof v === "number") return Object.is(v, -0) ? "-0" : String(v);
    if (typeof v === "string") return JSON.stringify(v);
    if (typeof v !== "object") return typeof v + ":" + String(v);
    if (seen.has(v)) return "<shared>";
    seen.add(v);
    const proto = Object.getPrototypeOf(v);
    const tag = proto === null ? "N" : proto === Object.prototype ? "O" : proto === Array.prototype ? "A" : "?";
    const extensible = Object.isExtensible(v) ? "" : "!ext";
    const keys = Reflect.ownKeys(v).map((key) => {
      const d = Object.getOwnPropertyDescriptor(v, key);
      const plain = "value" in d && d.writable && d.configurable
        && (Array.isArray(v) && key === "length" ? !d.enumerable : d.enumerable);
      return String(key) + (plain ? "" : "!") + ":" + show(d.value);
    });
    return tag + extensible + "{" + keys.join(",") + "}";
  }
  console.log(name + " " + show(root));
};
globalThis.keep = function keep(value) {
  if (value === undefined) throw new Error("a table is missing");
};
