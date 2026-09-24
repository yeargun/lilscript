// Host prelude for opt-keeps_borrowed_array_call_after_extern_can_shadow_method.lil, transcribed from src/optimizer.rs:14280.
// expose() installs an own `push` on the array it receives. JS.push must still
// reach the intrinsic Array.prototype.push, so the own method never runs.
// The test printed 'TRACE:'+shadowed after the program; that line is written at exit.
(() => {
  let shadowed = false;
  process.on("exit", () => process.stdout.write("TRACE:" + shadowed + "\n"));
  globalThis.expose = function (value) {
    value.push = () => {
      shadowed = true;
      return 99;
    };
  };
})();
