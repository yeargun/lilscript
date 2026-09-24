// Harness of the source test: consume logs strings verbatim, other values by typeof.
// The test compiled with lower_known_js_host_calls, which bound the extern objectToStringTag by
// name to Object.prototype.toString.call, so its harness never defined it. An extern is host
// provided (future-architecture L1: no meaning from names), so this prelude defines that binding.
(() => {
  const seen = [];
  globalThis.objectToStringTag = function (value) { return Object.prototype.toString.call(value); };
  globalThis.consume = function (v) { seen.push(typeof v === "string" ? v : typeof v); };
  process.on('exit', () => { console.log(seen.join(',')); });
})();
