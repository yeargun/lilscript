// Harness of the source test: String.prototype.endsWith is instrumented to count its calls;
// the test printed the count after the program.
(() => {
  globalThis.readBool = () => false;
  globalThis.readString = () => "a";
  let calls = 0;
  const endsWith = String.prototype.endsWith;
  String.prototype.endsWith = function (...args) { calls++; return endsWith.call(this, ...args); };
  process.on('exit', () => { console.log(calls); });
})();
