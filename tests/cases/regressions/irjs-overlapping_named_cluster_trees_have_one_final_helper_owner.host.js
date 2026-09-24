(() => {
  const values = [1, 1];
  globalThis.read = function () { return values.shift(); };
})();
