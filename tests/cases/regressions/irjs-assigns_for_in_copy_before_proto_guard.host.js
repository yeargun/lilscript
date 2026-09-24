(() => {
  globalThis.copyTarget = {};
  globalThis.copySource = { a: 1, b: 2 };
  process.on('exit', () => { console.log(JSON.stringify(globalThis.copyTarget)); });
})();
