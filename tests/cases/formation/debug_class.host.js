// `invariant` would print if a call reached it; `note` records each argument
// evaluation, which stays.
(() => {
  const notes = [];
  globalThis.invariant = (ok, message) => { if (!ok) console.log("INVARIANT:" + message); };
  globalThis.note = (label) => { notes.push(label); return label; };
  process.on("exit", () => { console.log("NOTES:" + notes.join(",")); });
})();
