import { runPassAblation } from "../pass-ablation.mjs";

runPassAblation({
  id: "mutation-spelling",
  source: "benchmarks/libraries/apps/js-levenshtein/lil/main.lil",
  expected: "benchmarks/libraries/apps/js-levenshtein/expected.txt",
  variants: [
    ["Mutation spelling selected", "lilscript.toml", "selected.js"],
    [
      "Assignment spelling only",
      "tests/config/no-mutation-spelling-selection.toml",
      "assignment.js",
    ],
  ],
  gateMetric: "brotli",
});
