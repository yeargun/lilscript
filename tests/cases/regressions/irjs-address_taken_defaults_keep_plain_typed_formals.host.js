// Harness of the source test: retain() prints callback(9) at once.
(() => {
  globalThis.retain = function (callback) { console.log(callback(9)); };
})();
