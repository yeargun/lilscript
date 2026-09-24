// Harness of the source test: after the program it printed saved.length+':'+saved[0]()+':'+reads.
(() => {
  const flags = [true, false], saved = [];
  let reads = 0;
  globalThis.flag = function () { return flags.shift(); };
  globalThis.read = function () { reads++; return 7; };
  globalThis.save = function (f) { saved.push(f); };
  process.on('exit', () => { console.log(saved.length + ':' + saved[0]() + ':' + reads); });
})();
