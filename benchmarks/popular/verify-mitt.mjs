import assert from "node:assert/strict";

const implementations = [
  ["npm", (await import("mitt")).default],
  ["lilscript", (await import("./apps/mitt/lil/api.js")).default],
];

function exercise(createEmitter) {
  assert.equal(createEmitter.length, 1);
  for (const falsy of [undefined, null, false, 0, ""]) {
    assert.ok(createEmitter(falsy).all instanceof Map);
  }
  assert.ok(new createEmitter().all instanceof Map);

  const constructible = createEmitter();
  assert.equal(constructible.on.length, 2);
  assert.equal(constructible.off.length, 2);
  assert.equal(constructible.emit.length, 2);
  const noop = () => {};
  assert.equal(typeof new constructible.on("construct", noop), "object");
  assert.equal(typeof new constructible.emit("missing"), "object");
  assert.equal(typeof new constructible.off("construct", noop), "object");

  const supplied = new Map();
  const emitter = createEmitter(supplied);
  assert.equal(emitter.all, supplied);
  assert.deepEqual(Object.keys(emitter).sort(), ["all", "emit", "off", "on"]);

  const calls = [];
  const duplicate = (value) => calls.push(["duplicate", value]);
  const late = (value) => calls.push(["late", value]);
  const mutate = (value) => {
    calls.push(["mutate", value]);
    emitter.off("event", mutate);
    emitter.on("event", late);
  };
  const wildcard = (type, value) => calls.push(["*", type, value]);

  emitter.on("event", duplicate);
  emitter.on("event", duplicate);
  emitter.on("event", mutate);
  emitter.on("*", wildcard);
  emitter.emit("event", { value: 1 });
  emitter.off("event", duplicate);
  emitter.emit("event");
  emitter.off("event");
  emitter.emit("event", 3);

  const symbol = Symbol("event");
  emitter.on(symbol, duplicate);
  emitter.emit(symbol, false);

  return calls.map((call) =>
    call.map((value) =>
      typeof value === "symbol"
        ? "symbol:event"
        : value === undefined
          ? "undefined"
          : typeof value === "object"
            ? JSON.stringify(value)
            : String(value),
    ),
  );
}

const baseline = exercise(implementations[0][1]);
for (const [name, createEmitter] of implementations.slice(1)) {
  assert.deepEqual(exercise(createEmitter), baseline, name);
}

console.log(`mitt-upstream:${implementations.length}:${baseline.length}`);
