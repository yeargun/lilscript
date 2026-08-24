import assert from "node:assert/strict";
import test from "node:test";

import { catalog } from "./catalog.mjs";
import {
  assertNoBehaviorLabelSplits,
  MIN_UNIQUE_GENERATED_BEHAVIORS,
  summarizeBehaviorCoverage,
} from "./coverage.mjs";

test("generated modules own at least one hundred unique behaviors", () => {
  const entries = catalog();
  const coverage = summarizeBehaviorCoverage(entries, {
    label: "generated catalog",
    minimumUniqueBehaviors: MIN_UNIQUE_GENERATED_BEHAVIORS,
  });
  assert.equal(coverage.uniqueBehaviorTemplates, 107);
  assert.equal(coverage.caseInstances, 570);
  assert.equal(coverage.parameterVariants, 463);
  assert.equal(Object.keys(coverage.behaviorFamilies).length, 18);
  assert.deepEqual(coverage.behaviorFamilies.closure, {
    uniqueBehaviorTemplates: 2,
    caseInstances: 17,
  });
  assert.equal(coverage.variantsByBehavior["string/utf16-indexing"].length, 10);
  assert.ok(
    coverage.variantsByBehavior["loop/nested-score"].includes(
      "win-control-flow",
    ),
  );
  assert.equal(
    coverage.variantsByBehavior["nullish/optional-indexing"].length,
    10,
  );
  assert.equal(coverage.variantsByBehavior["host/callable-predicate"].length, 3);
  assert.equal(
    coverage.variantsByBehavior["host/window-identity-predicate"].length,
    3,
  );
  assert.deepEqual(coverage.variantsByBehavior["host/missing-member-fallback"], [
    "js-and-member-missing",
  ]);
  assert.deepEqual(assertNoBehaviorLabelSplits(entries), {
    strategy:
      "JavaScript source normalized for quoted/static-template string, signed numeric, and whitespace parameters",
    caseInstancesAudited: 570,
    crossBehaviorShapeCollisions: 0,
  });
});

test("parameter seeds do not inflate the behavior count", () => {
  const coverage = summarizeBehaviorCoverage([
    { name: "add-1", behavior: "constant-fold/add" },
    { name: "add-2", behavior: "constant-fold/add" },
    { name: "subtract-1", behavior: "constant-fold/subtract" },
  ]);
  assert.equal(coverage.uniqueBehaviorTemplates, 2);
  assert.equal(coverage.parameterVariants, 1);
});

test("literal-only variants cannot acquire separate behavior labels", () => {
  assert.throws(
    () =>
      assertNoBehaviorLabelSplits([
        {
          name: "left",
          behavior: "constant-fold/add",
          js: 'console.log(1 + "left");',
        },
        {
          name: "right",
          behavior: "constant-fold/subtract",
          js: 'console.log(2 + "right");',
        },
      ]),
    /differ only by literal parameters/,
  );
  assert.doesNotThrow(() =>
    assertNoBehaviorLabelSplits([
      {
        name: "subtract",
        behavior: "integer/subtract",
        js: "const values = [3]; console.log(values[0] - 1);",
      },
      {
        name: "add",
        behavior: "integer/add",
        js: "const values = [3]; console.log(values[0] + 2);",
      },
    ]),
  );
  assert.throws(
    () =>
      assertNoBehaviorLabelSplits([
        { name: "negative", behavior: "integer/negative", js: "print(-1);" },
        { name: "positive", behavior: "integer/positive", js: "print(2);" },
      ]),
    /differ only by literal parameters/,
  );
  assert.throws(
    () =>
      assertNoBehaviorLabelSplits([
        {
          name: "template-left",
          behavior: "string/template-left",
          js: "console.log(`left`);",
        },
        {
          name: "template-right",
          behavior: "string/template-right",
          js: "console.log(`right`);",
        },
      ]),
    /differ only by literal parameters/,
  );
});

test("the minimum is fail-closed", () => {
  assert.throws(
    () =>
      summarizeBehaviorCoverage(
        [{ name: "only", behavior: "constant-fold/add" }],
        { label: "fixture", minimumUniqueBehaviors: 2 },
      ),
    /Parameter variants do not count/,
  );
});

test("invalid minimums fail instead of disabling the gate", () => {
  for (const minimumUniqueBehaviors of [-1, 1.5, Number.NaN, "2"]) {
    assert.throws(
      () =>
        summarizeBehaviorCoverage([], {
          label: "fixture",
          minimumUniqueBehaviors,
        }),
      /non-negative safe integer/,
    );
  }
});

test("valid behavior family names cannot collide with object prototypes", () => {
  const coverage = summarizeBehaviorCoverage([
    { name: "construct", behavior: "constructor/guard" },
  ]);
  assert.deepEqual(coverage.behaviorFamilies, {
    constructor: { uniqueBehaviorTemplates: 1, caseInstances: 1 },
  });
});

test("property-mangling opt-outs are explicit and justified", () => {
  const excluded = catalog().filter((entry) => !entry.terserProperties);
  assert.equal(excluded.length, 13);
  assert.ok(
    excluded.every(
      (entry) =>
        typeof entry.terserPropertyReason === "string" &&
        entry.terserPropertyReason.length > 0,
    ),
  );
  assert.ok(
    excluded.some((entry) => entry.name === "amd-define-guard"),
    "open-world AMD keys must not enter the property-mangled baseline",
  );
});
