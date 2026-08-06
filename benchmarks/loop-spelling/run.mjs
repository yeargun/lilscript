import { runPassAblation } from "../pass-ablation.mjs";

runPassAblation({
  id: "loop-spelling",
  source: "benchmarks/libraries/apps/murmurhash-js/lil/main.lil",
  expected: "benchmarks/libraries/apps/murmurhash-js/expected.txt",
  variants: [
    ["Codec-selected spelling", "lilscript.toml", "selected.js"],
    [
      "Frequency heuristic",
      "tests/config/no-loop-spelling-selection.toml",
      "heuristic.js",
    ],
  ],
  strictMetrics: ["brotli"],
  nonRegressionMetrics: ["raw", "gzip"],
});
