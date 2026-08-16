import { runPassAblation } from "../pass-ablation.mjs";

runPassAblation({
  id: "function-subsumption",
  source: "benchmarks/function-subsumption/fixture.lil",
  expected: "benchmarks/function-subsumption/fixture.out",
  variants: [
    [
      "subsumption enabled",
      "benchmarks/function-subsumption/enabled.toml",
      "enabled.js",
    ],
    [
      "subsumption disabled",
      "benchmarks/function-subsumption/disabled.toml",
      "disabled.js",
    ],
  ],
  gateMetric: "brotli",
});
