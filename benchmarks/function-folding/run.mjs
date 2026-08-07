import { runPassAblation } from "../pass-ablation.mjs";

runPassAblation({
  id: "function-folding",
  source: "benchmarks/function-folding/fixture.lil",
  expected: "benchmarks/function-folding/fixture.out",
  variants: [
    ["folding enabled", "benchmarks/function-folding/enabled.toml", "enabled.js"],
    ["folding disabled", "benchmarks/function-folding/disabled.toml", "disabled.js"],
  ],
});
