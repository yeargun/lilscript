import { describe, expect, test } from "vitest";
import {
  coreCases,
  runCoreCompatibility,
} from "../tooling/run-core-compat.mjs";

const result = runCoreCompatibility();

describe("LilScript Solid runtime compatibility", () => {
  for (const testCase of coreCases) {
    test(`${testCase.id}: ${testCase.name}`, () => {
      const actual = result.cases.find(({ id }) => id === testCase.id);
      expect(actual.modes).toEqual({ maximum: true, none: true });
      expect(actual.backends).toEqual({ js: true });
      expect(actual.passed).toBe(true);
    });
  }
});
