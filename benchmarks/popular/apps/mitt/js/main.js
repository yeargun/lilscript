import mitt from "mitt";

let calls = 0;
let last = "";
let lastType = "";

const bus = mitt();
const onFoo = (event) => {
  calls += 1;
  last = event;
};
const onAny = (type, event) => {
  calls += 1;
  lastType = type;
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
