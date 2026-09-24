// Harness of the source test: inspect records JSON.stringify(v); the test joined records with '|'.
(() => {
  globalThis.inspect = function (v) { console.log(JSON.stringify(v)); };
})();
