(() => {
  let n = 0;
  globalThis.keep = function () { return true; };
  globalThis.side = function () { n += 1; };
  process.on('exit', () => { console.log(String(n)); });
})();
