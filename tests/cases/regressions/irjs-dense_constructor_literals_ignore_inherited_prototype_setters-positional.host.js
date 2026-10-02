// Harness of the source test: an inherited accessor for the value field counts assignments that miss the own slot.
(() => {
  let calls = 0, own = false, value = 0, length = 0;
  Object.defineProperty(Object.prototype, 'value', { configurable: true, get() { return -1; }, set(_) { calls++; } });
  globalThis.inspect = function (box) { own = Object.hasOwn(box, "value"); value = box.value; length = Object.keys(box).length; };
  process.on('exit', () => { delete Object.prototype.value; console.log('TRACE:' + calls + ':' + own + ':' + value + ':' + length); });
})();
