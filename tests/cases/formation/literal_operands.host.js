// Records the order in which the program evaluates its literal operands.
(() => {
  const trace = [];
  globalThis.step = (label) => { trace.push(label); return label; };
  globalThis.host = { h: (a) => "h:" + a, f: () => { trace.push("f"); return "f"; } };
  process.on("exit", () => { console.log("ORDER:" + trace.join(",")); });
})();
