import assert from "node:assert/strict";
import test from "node:test";

import { objectiveSizeGate } from "../objective-gate.mjs";

function row(lilscriptVite) {
  return {
    costModel: "brotli",
    vite: { raw: 80, gzip: 60, brotli: 50 },
    closure: { raw: 70, gzip: 55, brotli: 45 },
    lilscriptVite,
  };
}

test("Brotli publication accepts diagnostic raw and gzip losses", () => {
  assert.equal(
    objectiveSizeGate(row({ raw: 100, gzip: 90, brotli: 45 })),
    true,
  );
});

test("Brotli publication rejects a matching-metric loss", () => {
  assert.equal(
    objectiveSizeGate(row({ raw: 60, gzip: 50, brotli: 46 })),
    false,
  );
});

test("a non-Brotli objective still gates only its matching metric", () => {
  const candidate = row({ raw: 69, gzip: 90, brotli: 90 });
  candidate.costModel = "raw";
  assert.equal(objectiveSizeGate(candidate), true);

  candidate.costModel = "gzip";
  assert.equal(objectiveSizeGate(candidate), false);
});

test("publication refuses an unmeasured objective", () => {
  assert.equal(objectiveSizeGate({ costModel: "brotli" }), null);
});
