import { runPassAblation } from "../pass-ablation.mjs";

runPassAblation({
  id: "finite-values",
  source: "tests/cases/interprocedural_finite_values.lil",
  expected: "tests/cases/interprocedural_finite_values.out",
  variants: [
    ["finite values enabled", "tests/config/no-inlining.toml", "enabled.js"],
    ["finite values disabled", "tests/config/no-finite-values.toml", "disabled.js"],
  ],
});
