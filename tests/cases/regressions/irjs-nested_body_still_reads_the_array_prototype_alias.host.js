// Harness of the source test: consume logs arrays as a ':'-joined list, other values by typeof.
(() => {
  const seen = [];
  globalThis.consume = function (v) { seen.push(Array.isArray(v) ? v.join(':') : typeof v); };
  process.on('exit', () => { console.log(seen.join(',')); });
})();
