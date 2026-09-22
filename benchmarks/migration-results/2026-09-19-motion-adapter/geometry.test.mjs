import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";
import { pathToFileURL } from "node:url";

const parent = JSON.parse(readFileSync(new URL("./existing-dist/receipt.json", import.meta.url)));
const upstream = await import(pathToFileURL(join(parent.dependencies, "motion/dist/es/index.mjs")));
const candidate = await import(pathToFileURL(join(parent.workspace, "dist/full.js")));

for (const [label, api] of [["upstream", upstream], ["candidate", candidate]]) {
  test(`${label}: copying preserves destination and leaf identity`, () => {
    const x = { min: 1, max: 2 }, y = { min: 3, max: 4 };
    const box = { x, y }, origin = { x: { min: 10, max: 20 }, y: { min: 30, max: 40 } };
    assert.equal(api.copyBoxInto(box, origin), undefined);
    assert.equal(box.x, x);
    assert.equal(box.y, y);
    assert.deepEqual(box, { x: { min: 10, max: 20 }, y: { min: 30, max: 40 } });
    assert.deepEqual(origin, { x: { min: 10, max: 20 }, y: { min: 30, max: 40 } });
    assert.notEqual(box.x, origin.x);
    assert.notEqual(box.y, origin.y);
  });

  test(`${label}: overlapping source observes the preceding destination write`, () => {
    const x = { min: 1, max: 2 }, y = { min: 3, max: 4 };
    const box = { x, y }, origin = { x: { min: 10, max: 20 }, y: x };
    api.copyBoxInto(box, origin);
    assert.equal(box.x, x);
    assert.equal(box.y, y);
    assert.equal(origin.y, x);
    assert.deepEqual(box, { x: { min: 10, max: 20 }, y: { min: 10, max: 20 } });
  });

  test(`${label}: a later getter throw preserves earlier writes and access order`, () => {
    const error = new Error("original getter failure"), events = [];
    let min = 1, max = 2;
    const axis = {
      get min() { return min; },
      set min(value) { events.push(["set min", value]); min = value; },
      get max() { return max; },
      set max(value) { events.push(["set max", value]); max = value; },
    };
    const origin = {
      get min() { events.push(["get min"]); return 5; },
      get max() { events.push(["get max"]); throw error; },
    };
    assert.throws(() => api.copyAxisInto(axis, origin), value => value === error);
    assert.deepEqual(events, [["get min"], ["set min", 5], ["get max"]]);
    assert.equal(min, 5);
    assert.equal(max, 2);
  });

  test(`${label}: a setter replacing the next leaf is visible to the next call`, () => {
    const oldY = { min: 3, max: 4 }, nextY = { min: 7, max: 8 }, events = [];
    let min = 1;
    const x = {
      get min() { return min; },
      set min(value) { events.push(["replace y", value]); min = value; box.y = nextY; },
      max: 2,
    };
    const box = { x, y: oldY };
    api.copyBoxInto(box, { x: { min: 10, max: 20 }, y: { min: 30, max: 40 } });
    assert.equal(box.x, x);
    assert.equal(box.y, nextY);
    assert.deepEqual(events, [["replace y", 10]]);
    assert.deepEqual(oldY, { min: 3, max: 4 });
    assert.deepEqual(nextY, { min: 30, max: 40 });
    assert.equal(x.min, 10);
    assert.equal(x.max, 20);
  });

  test(`${label}: aliased box axes receive both deltas in order`, () => {
    const axis = { min: 2, max: 4 }, box = { x: axis, y: axis };
    const delta = {
      x: { translate: 1, scale: 2, originPoint: 0 },
      y: { translate: 3, scale: 1, originPoint: 0 },
    };
    assert.equal(api.applyBoxDelta(box, delta), undefined);
    assert.equal(box.x, axis);
    assert.equal(box.y, axis);
    assert.deepEqual(axis, { min: 8, max: 12 });
    assert.deepEqual(delta, {
      x: { translate: 1, scale: 2, originPoint: 0 },
      y: { translate: 3, scale: 1, originPoint: 0 },
    });
  });
}
