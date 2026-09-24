// Harness of the source test: an inherited setter for index 0 counts assignments that miss the own slot.
(() => {
  let calls = 0, trace = '';
  Object.defineProperty(Array.prototype, '0', { configurable: true, set(_) { calls++; } });
  globalThis.inspect = function (slot) { trace = calls + ':' + Object.hasOwn(slot, 0) + ':' + slot[0] + ':' + slot.length; };
  process.on('exit', () => { delete Array.prototype[0]; console.log('TRACE:' + trace); });
})();
