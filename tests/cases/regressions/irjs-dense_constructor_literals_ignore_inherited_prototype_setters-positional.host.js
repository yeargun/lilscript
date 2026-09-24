// Harness of the source test: an inherited accessor for index 0 counts assignments that miss the own slot.
(() => {
  let calls = 0, own = false, value = 0, length = 0;
  Object.defineProperty(Array.prototype, '0', { configurable: true, get() { return -1; }, set(_) { calls++; } });
  globalThis.inspect = function (box) { own = Object.hasOwn(box, 0); value = box[0]; length = box.length; };
  process.on('exit', () => { delete Array.prototype[0]; console.log('TRACE:' + calls + ':' + own + ':' + value + ':' + length); });
})();
