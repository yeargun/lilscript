// Harness of the source test: an inherited setter for the value field counts assignments that miss the own slot.
(() => {
  let calls = 0, trace = '';
  Object.defineProperty(Object.prototype, 'value', { configurable: true, set(_) { calls++; } });
  globalThis.inspect = function (slot) { trace = calls + ':' + Object.hasOwn(slot, "value") + ':' + slot.value + ':' + Object.keys(slot).length; };
  process.on('exit', () => { delete Object.prototype.value; console.log('TRACE:' + trace); });
})();
