// Host prelude for opt-stringify_elision_does_not_move_dynamic_coercion_across_a_call.lil, transcribed from src/optimizer.rs:18246.
// The test printed events.join(',') after the program; that line is written at exit.
(() => {
  const events = [];
  process.on("exit", () => process.stdout.write(events.join(",") + "\n"));
  globalThis.dynamic = {
    [Symbol.toPrimitive]() {
      events.push("coerce");
      return "value";
    },
  };
  globalThis.side = function () {
    events.push("side");
  };
  globalThis.consume = function (value) {
    events.push(value);
  };
})();
