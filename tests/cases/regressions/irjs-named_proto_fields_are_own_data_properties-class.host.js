// Harness of the source test: inspect records own-ness of __proto__, its value and the real prototype.
(() => {
  let trace = '';
  globalThis.inspect = function (value) { trace = Object.hasOwn(value, '__proto__') + ':' + value.__proto__ + ':' + (Object.getPrototypeOf(value) === Object.prototype); };
  process.on('exit', () => { console.log('TRACE:' + trace); });
})();
