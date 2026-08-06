import { runPassAblation } from "../pass-ablation.mjs";

runPassAblation({
  id: "closure-factory-variants",
  source: "tests/cases/closure_factory_variant.lil",
  expected: "tests/cases/closure_factory_variant.out",
  variants: [
    ["Factory IR variants enabled", "lilscript.toml", "selected.js"],
    [
      "Factory IR variants disabled",
      "tests/config/no-ir-closure-factory-variants.toml",
      "inlined.js",
    ],
  ],
});
