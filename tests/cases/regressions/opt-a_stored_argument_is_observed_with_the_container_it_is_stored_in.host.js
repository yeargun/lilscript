// Host prelude for opt-a_stored_argument_is_observed_with_the_container_it_is_stored_in.lil, transcribed from src/optimizer.rs:18317.
// consume() records JSON.stringify of what it receives; the test printed it after
// the program, so it is written at exit.
(() => {
  let seen = "none";
  process.on("exit", () => process.stdout.write(seen + "\n"));
  globalThis.consume = function (object) {
    seen = JSON.stringify(object);
  };
})();
