import { runPassAblation } from "../pass-ablation.mjs";

runPassAblation({
  id: "ir-inlining-variants",
  source: "tests/cases/ir_inlining_variant.lil",
  expected: "tests/cases/ir_inlining_variant.out",
  variants: [
    ["IR variants enabled", "lilscript.toml", "enabled.js"],
    [
      "IR variants disabled",
      "tests/config/no-ir-inlining-variants.toml",
      "disabled.js",
    ],
  ],
  gateMetric: "brotli",
});
