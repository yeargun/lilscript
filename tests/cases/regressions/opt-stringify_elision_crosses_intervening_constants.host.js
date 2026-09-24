// Host prelude for opt-stringify_elision_crosses_intervening_constants.lil, transcribed from src/optimizer.rs:18231.
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
  globalThis.consume = function (value) {
    events.push(value);
  };
})();
