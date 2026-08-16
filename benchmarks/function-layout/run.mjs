import { runPassAblation } from "../pass-ablation.mjs";

runPassAblation({
  id: "function-layout",
  source: "benchmarks/function-layout/fixture.lil",
  expected: "benchmarks/function-layout/fixture.out",
  variants: [
    [
      "layout search enabled",
      "benchmarks/function-layout/enabled.toml",
      "enabled.js",
    ],
    ["source order", "benchmarks/function-layout/disabled.toml", "disabled.js"],
  ],
  gateMetric: "brotli",
});
