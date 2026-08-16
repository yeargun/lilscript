import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "node",
    include: ["tests/**/*.test.mjs"],
    // LSX has its own mixed node:test + official-Solid differential command.
    exclude: ["tests/lilx.test.mjs", "tests/lsx-runtime.test.mjs"],
    // Production compiler candidate search is intentionally exercised here.
    // Keep the timeout above a cold optimized compile on CI runners.
    testTimeout: 60_000,
  },
});
