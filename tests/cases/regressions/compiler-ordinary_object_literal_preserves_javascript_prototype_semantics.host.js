// Host of compiler.rs ordinary_object_literal_preserves_javascript_prototype_semantics:
// `let alphaSets=0,betaSets=0;` inherited setters for `alpha`/`beta` on Object.prototype,
// `inspect` writes the observations, and the prototype is cleaned up after the program.
{
  let alphaSets = 0, betaSets = 0;
  Object.defineProperty(Object.prototype, 'alpha', { configurable: true, set() { alphaSets++; } });
  Object.defineProperty(Object.prototype, 'beta', { configurable: true, set() { betaSets++; } });
  globalThis.inspect = function inspect(value) {
    console.log([
      Object.getPrototypeOf(value) === Object.prototype,
      Object.hasOwn(value, 'alpha'),
      Object.hasOwn(value, '__proto__'),
      Object.hasOwn(value, 'beta'),
      value.alpha,
      value.__proto__,
      alphaSets,
      betaSets,
    ].join(':'));
  };
  queueMicrotask(() => { delete Object.prototype.alpha; delete Object.prototype.beta; });
}
