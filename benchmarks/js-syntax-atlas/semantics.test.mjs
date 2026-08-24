import assert from "node:assert/strict";
import test from "node:test";

test("flat, block, and IIFE temporary regions preserve ordinary value traces", () => {
  const run = (variant) => {
    const trace = [];
    const f = (value) => trace.push(["f", value]);
    const g = (value) => trace.push(["g", value]);
    const a = 2;
    const b = 7;
    if (variant === "flat") {
      let x = a + 1;
      f(x);
      let y = b + 1;
      g(y);
    } else if (variant === "blocks") {
      {
        let x = a + 1;
        f(x);
      }
      {
        let x = b + 1;
        g(x);
      }
    } else {
      (() => {
        let x = a + 1;
        f(x);
      })();
      (() => {
        let x = b + 1;
        g(x);
      })();
    }
    return trace;
  };
  assert.deepEqual(run("blocks"), run("flat"));
  assert.deepEqual(run("iifes"), run("flat"));
});

test("an IIFE is not a transparent scope around sloppy direct eval", () => {
  const collect = (source) => {
    const values = [];
    Function("f", "g", source)(
      (value) => values.push(value),
      (value) => values.push(value),
    );
    return values;
  };
  const flat = 'eval("var z=1");f(typeof z);g(typeof z)';
  const block = '{eval("var z=1");f(typeof z)}g(typeof z)';
  const iife = '(()=>{eval("var z=1");f(typeof z)})();g(typeof z)';
  assert.deepEqual(collect(flat), ["number", "number"]);
  assert.deepEqual(collect(block), ["number", "number"]);
  assert.deepEqual(collect(iife), ["number", "undefined"]);
});

test("explicit factories preserve per-iteration values and identity", () => {
  const direct = [];
  const factory = [];
  for (let i = 0; i < 4; i += 1) direct.push(() => i);
  for (let i = 0; i < 4; i += 1) factory.push(((x) => () => x)(i));
  assert.deepEqual(direct.map((fn) => fn()), [0, 1, 2, 3]);
  assert.deepEqual(factory.map((fn) => fn()), [0, 1, 2, 3]);
  assert.equal(new Set(direct).size, 4);
  assert.equal(new Set(factory).size, 4);
});

test("one var binding cannot replace per-iteration let capture", () => {
  const closures = [];
  for (var index = 0; index < 4; index += 1) closures.push(() => index);
  assert.deepEqual(closures.map((fn) => fn()), [4, 4, 4, 4]);
});

test("parameter capture snapshots while closure capture stays live", () => {
  let live = 1;
  const liveClosure = () => live;
  const snapshotClosure = ((value) => () => value)(live);
  live = 2;
  assert.equal(liveClosure(), 2);
  assert.equal(snapshotClosure(), 1);
});

test("callback factories can preserve freshness; aliasing cannot", () => {
  const x = 3;
  const direct = [() => x, () => x];
  const make = () => () => x;
  const factory = [make(), make()];
  const shared = () => x;
  assert.notEqual(direct[0], direct[1]);
  assert.notEqual(factory[0], factory[1]);
  assert.equal(shared, shared);
  assert.deepEqual(factory.map((fn) => fn()), direct.map((fn) => fn()));
});

test("numeric CSE is value-exact for stable Number operands", () => {
  const values = [0, -0, 1, -2.5, Infinity, -Infinity, NaN, Number.MAX_VALUE];
  for (const a of values) {
    for (const b of values) {
      const repeated = [a * b, a * b];
      const cachedValue = a * b;
      const cached = [cachedValue, cachedValue];
      assert.ok(Object.is(repeated[0], cached[0]), `${a} * ${b} first`);
      assert.ok(Object.is(repeated[1], cached[1]), `${a} * ${b} second`);
    }
  }
});

test("derived primitive strings have no referential-identity hazard", () => {
  const a = "alpha";
  const b = "beta";
  const repeated = [a + ":" + b, a + ":" + b];
  const value = a + ":" + b;
  const cached = [value, value];
  assert.deepEqual(cached, repeated);
  assert.equal(cached[0] === cached[1], true);
});

test("property CSE is invalid for getters", () => {
  let reads = 0;
  const object = { get x() { reads += 1; return reads; } };
  const repeated = [object.x, object.x];
  reads = 0;
  const value = object.x;
  const cached = [value, value];
  assert.deepEqual(repeated, [1, 2]);
  assert.deepEqual(cached, [1, 1]);
  assert.equal(reads, 1);
});

test("property CSE is invalid when the first consumer mutates the property", () => {
  const repeatedObject = { x: 1 };
  const repeated = [];
  repeated.push(repeatedObject.x);
  repeatedObject.x = 2;
  repeated.push(repeatedObject.x);

  const cachedObject = { x: 1 };
  const cachedValue = cachedObject.x;
  const cached = [];
  cached.push(cachedValue);
  cachedObject.x = 2;
  cached.push(cachedValue);
  assert.deepEqual(repeated, [1, 2]);
  assert.deepEqual(cached, [1, 1]);
});

test("caching an observationally pure call preserves values but changes call count", () => {
  let calls = 0;
  const calc = (x) => { calls += 1; return x * 2; };
  const repeated = [calc(4), calc(4)];
  assert.deepEqual(repeated, [8, 8]);
  assert.equal(calls, 2);
  calls = 0;
  const value = calc(4);
  const cached = [value, value];
  assert.deepEqual(cached, repeated);
  assert.equal(calls, 1);
});

test("length caching is invalid when the loop body changes array length", () => {
  const dynamicArray = [1, 2];
  const dynamicSeen = [];
  for (let i = 0; i < dynamicArray.length; i += 1) {
    dynamicSeen.push(dynamicArray[i]);
    if (i === 0) dynamicArray.push(3);
  }

  const cachedArray = [1, 2];
  const cachedSeen = [];
  for (let i = 0, length = cachedArray.length; i < length; i += 1) {
    cachedSeen.push(cachedArray[i]);
    if (i === 0) cachedArray.push(3);
  }
  assert.deepEqual(dynamicSeen, [1, 2, 3]);
  assert.deepEqual(cachedSeen, [1, 2]);
});

test("loop-invariant property hoisting needs getter and mutation proofs", () => {
  let reads = 0;
  const object = { get scale() { reads += 1; return reads; } };
  const repeated = [1, 2, 3].map((x) => x * object.scale);
  reads = 0;
  const scale = object.scale;
  const hoisted = [1, 2, 3].map((x) => x * scale);
  assert.deepEqual(repeated, [1, 4, 9]);
  assert.deepEqual(hoisted, [1, 2, 3]);
});

test("branch calculation cannot move ahead of an effectful condition", () => {
  const originalTrace = [];
  const condOriginal = () => { originalTrace.push("condition"); return true; };
  const calcOriginal = () => { originalTrace.push("calculation"); return 1; };
  if (condOriginal()) calcOriginal();

  const movedTrace = [];
  const condMoved = () => { movedTrace.push("condition"); return true; };
  const calcMoved = () => { movedTrace.push("calculation"); return 1; };
  const value = calcMoved();
  if (condMoved()) void value;
  assert.deepEqual(originalTrace, ["condition", "calculation"]);
  assert.deepEqual(movedTrace, ["calculation", "condition"]);
});

test("non-escaping object scalar replacement preserves exposed Number values", () => {
  const values = [0, -0, 1, -4, Infinity, NaN];
  for (const a of values) {
    for (const b of values) {
      const object = { x: a, y: b };
      assert.ok(Object.is(object.x + object.y, a + b));
    }
  }
});

test("a private mutable Number field can be scalar-replaced", () => {
  for (const input of [0, -0, 1, -2, Number.MAX_SAFE_INTEGER]) {
    const object = { x: input };
    object.x += 1;
    let scalar = input;
    scalar += 1;
    assert.ok(Object.is(object.x, scalar));
  }
});

test("equal-looking objects must remain distinct", () => {
  const direct = [{ x: 1 }, { x: 1 }];
  const make = () => ({ x: 1 });
  const factory = [make(), make()];
  const shared = { x: 1 };
  assert.notEqual(direct[0], direct[1]);
  assert.notEqual(factory[0], factory[1]);
  assert.equal(shared, shared);
  direct[0].x = 2;
  assert.equal(direct[1].x, 1);
});

test("freezing does not erase object identity", () => {
  const first = Object.freeze({ x: 1 });
  const second = Object.freeze({ x: 1 });
  assert.notEqual(first, second);
  const keys = new WeakMap([[first, "first"], [second, "second"]]);
  assert.equal(keys.get(first), "first");
  assert.equal(keys.get(second), "second");
});

test("moving own methods to a prototype changes observable shape and identity", () => {
  function Own() { this.f = function () { return this.x; }; }
  function Shared() {}
  Shared.prototype.f = function () { return this.x; };
  const ownA = new Own();
  const ownB = new Own();
  const sharedA = new Shared();
  const sharedB = new Shared();
  assert.equal(Object.hasOwn(ownA, "f"), true);
  assert.equal(Object.hasOwn(sharedA, "f"), false);
  assert.notEqual(ownA.f, ownB.f);
  assert.equal(sharedA.f, sharedB.f);
});

test("sharing a global RegExp couples lastIndex", () => {
  const separate = [/a/g.exec("a")?.[0], /a/g.exec("a")?.[0]];
  const shared = /a/g;
  const coupled = [shared.exec("a")?.[0], shared.exec("a")?.[0]];
  assert.deepEqual(separate, ["a", "a"]);
  assert.deepEqual(coupled, ["a", undefined]);
});

test("functions, arrays, Promises, and Symbols all expose freshness", async () => {
  assert.notEqual(() => 1, () => 1);
  assert.notEqual([1, 2], [1, 2]);
  const promises = [Promise.resolve(1), Promise.resolve(1)];
  assert.notEqual(promises[0], promises[1]);
  assert.deepEqual(await Promise.all(promises), [1, 1]);
  assert.notEqual(Symbol(), Symbol());
  assert.equal(Symbol.for("atlas"), Symbol.for("atlas"));
});

test("sharing an array couples consumer mutation", () => {
  const first = [1, 2];
  const second = [1, 2];
  first.push(3);
  assert.deepEqual(second, [1, 2]);
  const shared = [1, 2];
  const left = shared;
  const right = shared;
  left.push(3);
  assert.deepEqual(right, [1, 2, 3]);
});

test("each tagged-template site has a distinct cached template object", () => {
  const seen = [];
  const tag = (strings) => { seen.push(strings); };
  tag`x`;
  tag`x`;
  assert.notEqual(seen[0], seen[1]);

  seen.length = 0;
  for (let i = 0; i < 2; i += 1) tag`x`;
  assert.equal(seen[0], seen[1]);
  assert.equal(Object.isFrozen(seen[0]), true);
});

test("a shared primitive binding preserves distinct function identity", () => {
  const repeatedF = () => 123456;
  const repeatedG = () => 123456;
  const sharedValue = 123456;
  const sharedF = () => sharedValue;
  const sharedG = () => sharedValue;
  assert.notEqual(repeatedF, repeatedG);
  assert.notEqual(sharedF, sharedG);
  assert.deepEqual([sharedF(), sharedG()], [repeatedF(), repeatedG()]);
});

test("factoring computation code differs from hoisting its result", () => {
  let calculations = 0;
  const calculate = () => { calculations += 1; return 12; };
  const helper = () => calculate();
  const helperF = () => helper();
  const helperG = () => helper();
  assert.deepEqual([helperF(), helperG()], [12, 12]);
  assert.equal(calculations, 2);

  calculations = 0;
  const hoisted = calculate();
  const hoistedF = () => hoisted;
  const hoistedG = () => hoisted;
  assert.deepEqual([hoistedF(), hoistedG()], [12, 12]);
  assert.equal(calculations, 1);
  assert.notEqual(hoistedF, hoistedG);
});

test("an IIFE changes legacy caller observation", () => {
  const observed = Function(`
    function probe(){let caller=probe.caller;return caller&&caller.name}
    function outer(){return[probe(),(()=>probe())()]}
    return outer()
  `)();
  assert.deepEqual(observed, ["outer", ""]);
});

test("wrapping a region changes which function a return exits", () => {
  const direct = Function("x", "if(x)return 1;return 2");
  const wrapped = Function("x", "(()=>{if(x)return 1})();return 2");
  assert.equal(direct(true), 1);
  assert.equal(wrapped(true), 2);
});

test("arrow and regular IIFEs capture arguments differently", () => {
  const inspect = Function(`
    return function outer(a){
      return[arguments.length,(()=>arguments.length)(),function(){return arguments.length}()]
    }
  `)();
  assert.deepEqual(inspect(1), [1, 1, 0]);
});

test("arrow and regular nested functions observe new.target differently", () => {
  const Constructor = Function(`
    return function C(){
      return[new.target===C,(()=>new.target===C)(),function(){return new.target}()]
    }
  `)();
  assert.deepEqual(new Constructor(), [true, true, undefined]);
});

test("sharing iterator and generator objects couples progress", () => {
  const array = [1, 2];
  const separateIterators = [array.values(), array.values()];
  assert.deepEqual(separateIterators.map((iterator) => iterator.next().value), [1, 1]);
  const sharedIterator = array.values();
  assert.deepEqual([sharedIterator.next().value, sharedIterator.next().value], [1, 2]);

  function* values() { yield 1; yield 2; }
  const separateGenerators = [values(), values()];
  assert.deepEqual(separateGenerators.map((iterator) => iterator.next().value), [1, 1]);
  const sharedGenerator = values();
  assert.deepEqual([sharedGenerator.next().value, sharedGenerator.next().value], [1, 2]);
});

test("Maps, typed arrays, Dates, Errors, and bound functions retain identity", () => {
  const maps = [new Map([[1, 2]]), new Map([[1, 2]])];
  maps[0].set(3, 4);
  assert.notEqual(maps[0], maps[1]);
  assert.equal(maps[1].has(3), false);

  const arrays = [new Uint8Array([1, 2]), new Uint8Array([1, 2])];
  arrays[0][0] = 9;
  assert.notEqual(arrays[0], arrays[1]);
  assert.equal(arrays[1][0], 1);

  const dates = [new Date(0), new Date(0)];
  dates[0].setTime(1);
  assert.notEqual(dates[0], dates[1]);
  assert.equal(dates[1].getTime(), 0);

  assert.notEqual(new Error("x"), new Error("x"));
  const object = { method() { return this; } };
  const bounds = [object.method.bind(object), object.method.bind(object)];
  assert.notEqual(bounds[0], bounds[1]);
  assert.equal(bounds[0](), object);
  assert.equal(bounds[1](), object);
});

test("stateful and nondeterministic-looking calls cannot share one result", () => {
  let state = 0;
  const next = () => ++state;
  const repeated = [next(), next()];
  state = 0;
  const value = next();
  const cached = [value, value];
  assert.deepEqual(repeated, [1, 2]);
  assert.deepEqual(cached, [1, 1]);
});
