// Harness of the source test: an inherited setter for alpha on Object.prototype captures the write,
// so a real assignment (not a literal) must land in the setter and leave no own property.
(() => {
  let inherited = 0, seen;
  Object.defineProperty(Object.prototype, 'alpha', { set(value) { inherited = value; }, configurable: true });
  globalThis.consume = function (value) { seen = value; };
  process.on('exit', () => { console.log(inherited + ':' + Object.hasOwn(seen, 'alpha')); delete Object.prototype.alpha; });
})();
