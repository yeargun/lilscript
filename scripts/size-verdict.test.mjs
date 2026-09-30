import assert from "node:assert/strict";
import {test} from "node:test";
import {sizeVerdict} from "./lib/size-verdict.mjs";

test("one-byte changes stay exact and independent of artifact magnitude", () => {
  for (const reference of [1, 100, 1000000]) {
    assert.equal(sizeVerdict(reference-1, reference).label, "smaller");
    assert.equal(sizeVerdict(reference+1, reference).label, "larger");
    assert.equal(sizeVerdict(reference, reference).label, "equal");
  }
});
test("absent or ineligible evidence never supplies a winning label", () => {
  assert.equal(sizeVerdict(0, null).label, "ineligible");
  assert.equal(sizeVerdict(0, 100, false).label, "ineligible");
  assert.equal(sizeVerdict(0, 0).fraction, null);
  for (const value of [-1, NaN, Infinity, 1.5, Number.MAX_SAFE_INTEGER+1])
    assert.throws(() => sizeVerdict(value, 1));
});
