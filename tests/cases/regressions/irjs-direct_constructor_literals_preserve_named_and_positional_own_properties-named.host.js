// Harness of the source test: an inherited setter for _lilDirectProbe counts assignments that miss
// the own property; inspect records own-ness, the __proto__ field and the real prototype.
(() => {
  let calls = 0, trace = '';
  Object.defineProperty(Object.prototype, '_lilDirectProbe', { configurable: true, set(_) { calls++; } });
  globalThis.inspect = function (box) {
    trace = calls + ':' + Object.hasOwn(box, '_lilDirectProbe') + ':' + Object.hasOwn(box, '__proto__') + ':' + box.__proto__ + ':' + (Object.getPrototypeOf(box) === Object.prototype);
  };
  process.on('exit', () => { delete Object.prototype._lilDirectProbe; console.log('TRACE:' + trace); });
})();
