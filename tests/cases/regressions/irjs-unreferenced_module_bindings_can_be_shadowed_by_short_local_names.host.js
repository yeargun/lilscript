(() => {
  globalThis.keep = function (cb) { cb(2); };
})();
