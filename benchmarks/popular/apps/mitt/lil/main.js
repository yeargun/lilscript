import mitt from "./api.js";

let calls = 0;
let last = "";

const bus = mitt();
const onFoo = (event) => {
  calls += 1;
  last = event;
};
const onAny = (_type, event) => {
  calls += 1;
  last = event;
};

bus.on("foo", onFoo);
bus.on("*", onAny);
bus.emit("foo", "bar");
bus.emit("baz", "qux");
bus.off("foo", onFoo);
bus.emit("foo", "nope");

const CUSTOM = Symbol("custom");
bus.on(CUSTOM, onFoo);
bus.emit(CUSTOM, "sym");

let passed = 0;
if (calls === 6) passed += 1;
if (last === "sym") passed += 1;
if (bus.all.size === 3) passed += 1;
console.log(`mitt:${passed}:${calls}`);
