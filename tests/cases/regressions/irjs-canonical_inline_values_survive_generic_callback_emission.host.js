(() => {
  let calls = 0;
  globalThis.read = function () { calls++; return 7; };
  process.on('exit', () => { console.log('calls:' + calls); });
})();
