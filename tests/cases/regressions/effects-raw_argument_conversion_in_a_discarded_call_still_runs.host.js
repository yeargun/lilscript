(function () {
  globalThis.input = () => ({ valueOf() { console.log("valueOf"); return 2; } });
})();
