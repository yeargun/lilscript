// Host prelude for opt-preserves_plain_object_inherited_setter_observation.lil, transcribed from src/optimizer.rs:18170.
// Object.prototype gets an inherited accessor for `_lilPlainObjectProbe`. The
// indexed store on a JS.object() must be an ordinary [[Set]]: the inherited setter
// runs once (seen=9), no own property appears, and the getter answers -1.
(() => {
  let calls = 0, seen = 0, own = false, value = 0;
  process.on("exit", () => {
    delete Object.prototype._lilPlainObjectProbe;
    process.stdout.write("TRACE:" + calls + ":" + own + ":" + value + ":" + seen + "\n");
  });
  Object.defineProperty(Object.prototype, "_lilPlainObjectProbe", {
    configurable: true,
    get() {
      return -1;
    },
    set(item) {
      calls++;
      seen = item;
    },
  });
  globalThis.read = function () {
    return 9;
  };
  globalThis.consume = function (object) {
    own = Object.hasOwn(object, "_lilPlainObjectProbe");
    value = object._lilPlainObjectProbe;
  };
})();
