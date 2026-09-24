// Harness of the source test: read() returns an object whose ToPrimitive counts its calls.
(() => {
  let calls = 0;
  globalThis.read = function () { return { [Symbol.toPrimitive]() { calls++; return 3; } }; };
  process.on('exit', () => { console.log('coercions:' + calls); });
})();
