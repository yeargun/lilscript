// Host prelude for opt-closed_record_projection_preserves_null_prototype_observations.lil, transcribed from src/optimizer.rs:18492.
// pollutePrototype() replaces Object.prototype.toString with 99. A record is a
// null-prototype object, so value.toString stays absent (-1) and JSON keeps {"safe":1}.
// The test printed 'TRACE:'+calls after the program; that line is written at exit.
(() => {
  let calls = 0;
  process.on("exit", () => process.stdout.write("TRACE:" + calls + "\n"));
  globalThis.pollutePrototype = function () {
    calls++;
    Object.prototype.toString = 99;
  };
})();
