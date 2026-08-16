import assert from "node:assert/strict";
import test from "node:test";

import {
  declaredCostModel,
  objectiveRelationPasses,
} from "./pass-ablation.mjs";

test("a Brotli ablation ignores diagnostic raw and gzip losses", () => {
  const enabled = { raw: 110, gzip: 105, brotli: 80 };
  const disabled = { raw: 100, gzip: 100, brotli: 81 };
  assert.equal(
    objectiveRelationPasses(enabled, disabled, "brotli", "lt"),
    true,
  );
});

test("strict and non-regression expectations apply only to the selected metric", () => {
  const enabled = { raw: 200, gzip: 120, brotli: 90 };
  const disabled = { raw: 100, gzip: 100, brotli: 90 };
  assert.equal(
    objectiveRelationPasses(enabled, disabled, "brotli", "lt"),
    false,
  );
  assert.equal(
    objectiveRelationPasses(enabled, disabled, "brotli", "le"),
    true,
  );
});

test("ablation configs must explicitly attest their selected objective", () => {
  assert.equal(
    declaredCostModel('[javascript]\ncost_model = "brotli"\n'),
    "brotli",
  );
  assert.equal(
    declaredCostModel("[javascript]\ncandidate_search = 'off'\n"),
    null,
  );
});
