// Harness of the source test: the global Map is replaced by a counting class; the field default
// `new Map` must still run once before the constructor overwrites it.
(() => {
  let calls = 0, seen = false;
  globalThis.Map = class { constructor() { calls++; } };
  globalThis.supplied = function () { return { tag: 7 }; };
  globalThis.inspect = function (cache) { seen = cache[0].tag === 7; };
  process.on('exit', () => { console.log('TRACE:' + calls + ':' + seen); });
})();
