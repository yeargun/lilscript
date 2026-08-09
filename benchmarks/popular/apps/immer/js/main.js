import {
  produce,
  current,
  original,
  enablePatches,
  produceWithPatches,
  applyPatches,
} from "immer";

let passed = 0;
const parts = [];

function check(cond) {
  parts.push(cond ? "1" : "0");
  if (cond) passed += 1;
}

const base1 = { keep: { x: 1 }, edit: { y: 2 }, n: 5 };
const next1 = produce(base1, (d) => {
  d.edit.y = 9;
});
check(base1.edit.y === 2);
check(next1.edit.y === 9);
check(next1.keep === base1.keep);
check(next1.edit !== base1.edit);
check(next1.n === 5);

const base2 = { list: [1, 2], meta: { ok: true } };
const next2 = produce(base2, (d) => {
  d.list.push(3);
  d.list[0] = 8;
});
check(base2.list.length === 2);
check(next2.list.join(",") === "8,2,3");
check(next2.meta === base2.meta);

const base3 = { a: 1, b: 2, nested: { c: 3 } };
const next3 = produce(base3, (d) => {
  delete d.b;
  d.nested.c = 4;
});
check(!("b" in next3) && next3.a === 1 && next3.nested.c === 4);

const base4 = { a: 1 };
const next4 = produce(base4, () => {});
check(next4 === base4);

const base5 = { a: 1 };
const next5 = produce(base5, () => ({ b: 2 }));
check(next5.b === 2 && next5.a === undefined);

const inc = produce((d) => {
  d.n = d.n + 1;
});
check(inc({ n: 10 }).n === 11);

const base7 = { a: { b: 1 }, arr: [{ z: 1 }] };
produce(base7, (d) => {
  d.a.b = 9;
  d.arr[0].z = 7;
  d.arr.push({ z: 3 });
  check(original(d.a).b === 1);
  check(current(d.a).b === 9);
  check(original(d.arr[0]).z === 1);
  check(current(d.arr[0]).z === 7);
  check(current(d).arr.length === 2);
});

const base8 = { list: [1, 2, 3] };
const next8 = produce(base8, (d) => {
  d.list.pop();
});
check(next8.list.join(",") === "1,2");

enablePatches();

const base9 = { "keep": { "x": 1 }, "edit": { "y": 2 }, "list": [1, 2], "n": 5 };
const [next9, patches9, inverse9] = produceWithPatches(base9, (d) => {
  d["edit"]["y"] = 9;
  d["list"].push(3);
  d["list"][0] = 8;
  delete d["n"];
  d["added"] = { "z": 1 };
});
check(next9["edit"]["y"] === 9 && next9["list"].join(",") === "8,2,3" && next9["added"]["z"] === 1 && !("n" in next9));
check(patches9.length === 5);
check(patches9[0].op === "replace" && patches9[0].path.join("/") === "edit/y" && patches9[0].value === 9);
check(patches9[1].op === "replace" && patches9[1].path.join("/") === "list/0" && patches9[1].value === 8);
check(patches9[2].op === "add" && patches9[2].path.join("/") === "list/2" && patches9[2].value === 3);
check(patches9[3].op === "remove" && patches9[3].path.join("/") === "n");
check(patches9[4].op === "add" && patches9[4].path.join("/") === "added" && patches9[4].value["z"] === 1);
check(JSON.stringify(applyPatches(base9, patches9)) === JSON.stringify(next9));
check(JSON.stringify(applyPatches(next9, inverse9)) === JSON.stringify(base9));

const [next10, patches10, inverse10] = produceWithPatches({ "a": 1 }, () => {});
check(next10["a"] === 1 && patches10.length === 0 && inverse10.length === 0);

const [next11, patches11] = produceWithPatches({ "a": 1 }, () => ({ "b": 2 }));
check(next11["b"] === 2 && patches11.length === 1 && patches11[0].op === "replace" && patches11[0].path.length === 0);

const [next12, patches12, inverse12] = produceWithPatches({ "list": [1, 2, 3] }, (d) => {
  d["list"].pop();
});
check(next12["list"].join(",") === "1,2");
check(patches12.length === 1 && patches12[0].op === "remove" && patches12[0].path.join("/") === "list/2");
check(applyPatches({ "list": [1, 2, 3] }, patches12)["list"].join(",") === "1,2");
check(applyPatches(next12, inverse12)["list"].join(",") === "1,2,3");

console.log(`immer:${passed}:${parts.join("")}`);
