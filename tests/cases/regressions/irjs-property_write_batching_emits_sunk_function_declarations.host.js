// Harness of the source test: after the program it printed `${seen.alpha}:${seen.first(3)}:${seen.beta}:${seen.second(4)}`.
(() => {
  let seen;
  globalThis.input = function () { return {}; };
  globalThis.consume = function (value) { seen = value; };
  process.on('exit', () => { console.log(`${seen.alpha}:${seen.first(3)}:${seen.beta}:${seen.second(4)}`); });
})();
