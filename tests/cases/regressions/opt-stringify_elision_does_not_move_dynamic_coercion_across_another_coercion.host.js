// Host prelude for opt-stringify_elision_does_not_move_dynamic_coercion_across_another_coercion.lil, transcribed from src/optimizer.rs:18296.
// The test printed events.join(',') after the program; that line is written at exit.
(() => {
  const events = [];
  process.on("exit", () => process.stdout.write(events.join(",") + "\n"));
  globalThis.dynamic = {
    toString() {
      events.push("first");
      return "value";
    },
  };
  globalThis.other = {
    valueOf() {
      events.push("second");
      return 2;
    },
  };
  globalThis.consume = function (value) {
    events.push(value);
  };
})();
