(() => {
  globalThis.list = [function (a, b) { return this + ':' + a + ':' + b; }];
  globalThis.memory = ['ctx', ['x', 'y']];
  globalThis.index = 0;
})();
