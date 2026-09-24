(() => {
  let next = 0;
  globalThis.input = function () { return ++next; };
})();
