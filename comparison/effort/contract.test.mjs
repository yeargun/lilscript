import assert from "node:assert/strict";
import test from "node:test";

import {
  assertEffortConfig,
  assertSampledEffortFrontier,
  parseSelectionExplanation,
} from "./contract.mjs";

const validSelection = {
  codec: "gzip",
  transfer_bytes: 42,
  candidates_evaluated: 16,
  compiler_time_micros: 123,
};

test("explanation parsing accepts a complete selection record", () => {
  assert.deepEqual(
    parseSelectionExplanation(
      JSON.stringify({ javascript_selection: validSelection }),
      "fixture",
    ),
    validSelection,
  );
});

test("explanation parsing fails closed on missing or invalid effort metrics", () => {
  for (const patch of [
    { candidates_evaluated: undefined },
    { candidates_evaluated: 0 },
    { candidates_evaluated: -1 },
    { candidates_evaluated: 1.5 },
    { compiler_time_micros: undefined },
    { compiler_time_micros: -1 },
    { transfer_bytes: undefined },
    { transfer_bytes: -1 },
  ]) {
    const selection = { ...validSelection, ...patch };
    assert.throws(
      () =>
        parseSelectionExplanation(
          JSON.stringify({ javascript_selection: selection }),
          "fixture",
        ),
      /must be a safe integer/,
    );
  }
});

test("effort configs retain the objective and configured search ceiling", () => {
  const config = `
[javascript]
optimization_level = 9
cost_model = "brotli"
candidate_search = "always"
candidate_limit = 1536
`;
  assert.doesNotThrow(() =>
    assertEffortConfig(config, {
      label: "fixture",
      objective: "brotli",
      level: 9,
    }),
  );
  for (const changed of [
    config.replace("optimization_level = 9", "optimization_level = 8"),
    config.replace('cost_model = "brotli"', 'cost_model = "raw"'),
    config.replace('candidate_search = "always"', 'candidate_search = "off"'),
    config.replace("candidate_limit = 1536", "candidate_limit = 384"),
    config.replace("candidate_limit = 1536\n", ""),
    `${config}optimizations = []\n`,
  ]) {
    assert.throws(() =>
      assertEffortConfig(changed, {
        label: "fixture",
        objective: "brotli",
        level: 9,
      }),
    );
  }
});

test("sampled effort frontier accepts equal or improving successive levels", () => {
  const points = [65, 55, 52, 52, 52, 52].map((selectedBytes, index) => ({
    level: index * 3,
    selectedBytes,
  }));
  assert.equal(
    assertSampledEffortFrontier(points, {
      label: "brotli-closure",
      objective: "brotli",
    }).selectedBytes,
    52,
  );
});

test("sampled effort frontier rejects regression from the best lower level", () => {
  const points = [65, 55, 52, 55, 52, 52].map((selectedBytes, index) => ({
    level: index * 3,
    selectedBytes,
  }));
  assert.throws(
    () =>
      assertSampledEffortFrontier(points, {
        label: "brotli-closure",
        objective: "brotli",
      }),
    /sampled level 9 regressed brotli from best lower sampled level 6 \(52 bytes\) to 55 bytes/,
  );
});
