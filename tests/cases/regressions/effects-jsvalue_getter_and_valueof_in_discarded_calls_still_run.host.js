(function () {
  globalThis.probe = {
    valueOf() { console.log("valueOf"); return 1; },
    get field() { console.log("get"); return 2; },
  };
})();
