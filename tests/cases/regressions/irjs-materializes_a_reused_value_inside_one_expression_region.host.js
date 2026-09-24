// Harness of the source test: input() counts its calls; the test printed 'calls:'+calls after the program.
(() => {
  let calls = 0;
  globalThis.input = function () { calls++; return 3; };
  process.on('exit', () => { console.log('calls:' + calls); });
})();
