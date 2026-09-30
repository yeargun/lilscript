globalThis.numericInput = {
  valueOf() { console.log("coerce"); return 2; }
};
globalThis.stringInput = {
  get length() { console.log("length"); return 3; }
};
