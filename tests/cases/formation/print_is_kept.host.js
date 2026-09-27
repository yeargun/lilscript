// `debugLog` would print if a call reached it; `note` records each argument
// evaluation, which stays.
(() => {
  const notes = [];
  globalThis.debugLog = (message) => { console.log("DEBUG:" + message); };
  globalThis.note = (label) => { notes.push(label); return label; };
  process.on("exit", () => { console.log("NOTES:" + notes.join(",")); });
})();
