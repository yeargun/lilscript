// Harness of the source test: an inherited accessor for _lilProtoProbe counts assignments that
// miss the own property and reads -1 when the property is not own.
(() => {
  let calls = 0, own = false, value = 0;
  Object.defineProperty(Object.prototype, '_lilProtoProbe', { configurable: true, get() { return -1; }, set(_) { calls++; } });
  globalThis.inspect = function (box) { own = Object.hasOwn(box, '_lilProtoProbe'); value = box._lilProtoProbe; };
  process.on('exit', () => { delete Object.prototype._lilProtoProbe; console.log('TRACE:' + calls + ':' + own + ':' + value); });
})();
