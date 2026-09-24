(function () {
  let calls = 0;
  globalThis.measure = (x) => { calls += 1; return x; };
  process.on("exit", () => console.log("calls=" + calls));
})();
