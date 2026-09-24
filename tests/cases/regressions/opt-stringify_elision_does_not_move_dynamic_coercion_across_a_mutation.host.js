// Host prelude for opt-stringify_elision_does_not_move_dynamic_coercion_across_a_mutation.lil, transcribed from src/optimizer.rs:18281.
// The test printed events.join(',') after the program; that line is written at exit.
(() => {
  const events = [];
  process.on("exit", () => process.stdout.write(events.join(",") + "\n"));
  globalThis.dynamic = {
    toString() {
      events.push("toString");
      return "value";
    },
  };
  globalThis.object = {
    set property(value) {
      events.push("set");
    },
  };
  globalThis.consume = function (value) {
    events.push(value);
  };
})();
