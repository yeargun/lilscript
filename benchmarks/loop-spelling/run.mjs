import { runPassAblation } from "../pass-ablation.mjs";

runPassAblation({
  id: "loop-spelling",
  source: "benchmarks/loop-spelling/fixture.lil",
  expected: "benchmarks/loop-spelling/fixture.out",
  variants: [
    ["Codec-selected spelling", "lilscript.toml", "selected.js"],
    [
      "Frequency heuristic",
      "tests/config/no-loop-spelling-selection.toml",
      "heuristic.js",
    ],
  ],
  gateMetric: "brotli",
  expectation: "le",
});
