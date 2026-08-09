import assert from "node:assert/strict";
import npmDefault, { clsx as npmNamed } from "clsx";
import lilDefault, { clsx as lilNamed } from "./apps/clsx/lil/api.js";

assert.equal(npmDefault, npmNamed);
assert.equal(lilDefault, lilNamed);

let state = 0x6d2b79f5;
function random() {
  state = Math.imul(state ^ (state >>> 15), 1 | state);
  state ^= state + Math.imul(state ^ (state >>> 7), 61 | state);
  return ((state ^ (state >>> 14)) >>> 0) / 4294967296;
}

const leaves = [
  undefined,
  null,
  false,
  true,
  "",
  "a",
  "two words",
  0,
  -0,
  1,
  -2.5,
  Number.NaN,
  Number.POSITIVE_INFINITY,
  1n,
  Symbol("ignored"),
  () => "ignored",
];

function value(depth) {
  if (depth === 0 || random() < 0.45) {
    return leaves[Math.floor(random() * leaves.length)];
  }
  if (random() < 0.55) {
    return Array.from({ length: Math.floor(random() * 5) }, () => value(depth - 1));
  }
  const object = Object.create(random() < 0.2 ? { inherited: random() < 0.5 } : null);
  const count = Math.floor(random() * 5);
  for (let index = 0; index < count; index += 1) {
    object[`k${index}`] = value(0);
  }
  return object;
}

for (let sample = 0; sample < 10_000; sample += 1) {
  const args = Array.from({ length: Math.floor(random() * 7) }, () => value(4));
  assert.equal(lilDefault(...args), npmDefault(...args), `sample ${sample}`);
}

console.log("clsx-differential:10000");
