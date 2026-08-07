import { runPassAblation } from "../pass-ablation.mjs";

runPassAblation({
  id: "profile-guided",
  source: "benchmarks/profile-guided/fixture.lil",
  expected: "benchmarks/profile-guided/fixture.out",
  variants: [
    ["Profile-guided specialization", "benchmarks/profile-guided/enabled.toml", "profiled.js"],
    ["Static higher-order call", "benchmarks/profile-guided/disabled.toml", "static.js"],
  ],
  strictMetrics: ["raw", "gzip", "brotli"],
});
