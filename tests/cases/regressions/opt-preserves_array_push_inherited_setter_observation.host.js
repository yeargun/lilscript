// Host prelude for opt-preserves_array_push_inherited_setter_observation.lil, transcribed from src/optimizer.rs:18147.
// Array.prototype gets an inherited index-0 accessor. push performs [[Set]], so
// it must run the inherited setter once (seen=9) and create no own element;
// consume() then reads the getter (-1) and length 1.
// The test removed the accessor in a `finally` right after the program. consume()
// is the program's last statement, so it removes the accessor there (and again at
// exit), which keeps Node's own array work out of the polluted state.
(() => {
  let calls = 0, seen = 0, own = false, value = 0, length = 0;
  const clean = () => {
    delete Array.prototype[0];
  };
  process.on("exit", () => {
    clean();
    process.stdout.write("TRACE:" + calls + ":" + own + ":" + value + ":" + length + ":" + seen + "\n");
  });
  globalThis.read = function () {
    return 9;
  };
  globalThis.consume = function (array) {
    own = Object.hasOwn(array, 0);
    value = array[0];
    length = array.length;
    clean();
  };
  Object.defineProperty(Array.prototype, "0", {
    configurable: true,
    get() {
      return -1;
    },
    set(item) {
      calls++;
      seen = item;
    },
  });
})();
