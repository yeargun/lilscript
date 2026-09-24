(() => {
  globalThis.defineConfigurable = function (o, k, v) { Object.defineProperty(o, k, { value: v, configurable: true }); };
})();
