import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "node",
    include: ["tests/lsx-runtime.test.mjs"],
    testTimeout: 60_000,
  },
});
