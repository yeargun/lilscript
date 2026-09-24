// Host prelude for opt-stringify_elision_does_not_move_dynamic_coercion_across_a_getter.lil, transcribed from src/optimizer.rs:18266.
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
    get property() {
      events.push("get");
      return 1;
    },
  };
  globalThis.consume = function (value) {
    events.push(value);
  };
})();
