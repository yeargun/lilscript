globalThis.receiver = () => "left/right";
globalThis.separator = () => "/";
const original = Object.getOwnPropertyDescriptor(String.prototype, "split");
const first = library.splitValue();
const second = library.splitValue();
first[0] = "changed";
events.push(["built-in", first === second, first, second]);
const written = library.mutateSplit();
events.push(["built-in-mutation", written, second, written === second]);
const shared = ["original", "shared"];
let lookups = 0;
let calls = 0;
try {
  Object.defineProperty(String.prototype, "split", {
    configurable: true,
    get() {
      lookups++;
      return function (separator) {
        calls++;
        events.push(["host-call", String(this), arguments.length, separator]);
        return shared;
      };
    }
  });
  const a = library.splitValue();
  const b = library.splitValue();
  a[1] = "changed";
  events.push(["host-alias", a === b, b === shared, b[1]]);
  const c = library.mutateSplit();
  events.push(["host-mutation", c === shared, shared[0], a[0]]);
  library.unusedSplit();
  library.unusedSplit();
  events.push(["calls", lookups, calls]);
} finally {
  Object.defineProperty(String.prototype, "split", original);
}
