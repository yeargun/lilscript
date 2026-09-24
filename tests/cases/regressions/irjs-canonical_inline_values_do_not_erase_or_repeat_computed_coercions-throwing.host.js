// Harness of the source test: ToPrimitive throws a marker once; the test caught it around the program and printed String(error===marker)+':'+calls.
(() => {
  let calls = 0, caught = '';
  const marker = {};
  globalThis.read = function () { return { [Symbol.toPrimitive]() { calls++; throw marker; } }; };
  process.on('uncaughtException', (error) => { caught = String(error === marker) + ':' + calls; });
  process.on('exit', () => { console.log(caught); });
})();
