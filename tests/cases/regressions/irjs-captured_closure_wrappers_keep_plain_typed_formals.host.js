(() => {
  globalThis.read = function () { return 7; };
  globalThis.touch = function () {};
  globalThis.invoke = function (callback) { return callback(11); };
})();
