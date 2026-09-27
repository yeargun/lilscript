// Unit tests for the pure parts of scripts/cases.mjs and scripts/ports.mjs.
//   node --test scripts/verify-runners.test.mjs
import assert from "node:assert/strict";
import { test } from "node:test";
import { codeOnly, composeConfig, LANES, parseTomlTables, selectLanes } from "./cases.mjs";
import { diffAgainstLedger, failingTests, rewriteObjective, testTotals } from "./ports.mjs";

test("feature detection ignores comments and string text but not template expressions", () => {
  const code = codeOnly('// export JsValue\nprint("extern try");\n/* Regex */ int a = 1;\nprint(`x ${JS.string(a)} throw`);');
  assert.doesNotMatch(code, /export|JsValue|extern|try|Regex|throw/);
  assert.match(code, /JS\.string/);
});

test("lane selection unions items and intersects components", () => {
  assert.equal(selectLanes("all").length, 18);
  assert.equal(selectLanes("production").length, 9);
  assert.deepEqual(selectLanes("production/*/c,formation-only/raw/script").map((lane) => lane.id), [
    "formation-only/raw/script", "production/brotli/c", "production/gzip/c", "production/raw/c",
  ]);
  assert.throws(() => selectLanes("prod/uction"), /mode\/codec\/target/);
  assert.throws(() => selectLanes("nothing"), /matches no lane/);
});

test("a case configuration merges into the lane's tables, one [javascript] table", () => {
  const lane = LANES.find((row) => row.id === "formation-only/gzip/module");
  const text = composeConfig(lane, ["inlining", "naming-search"], '# comment\n[javascript]\nassume_pristine_builtins = false # why\n[mangle]\nexports = false\n');
  assert.equal(text.match(/^\[javascript\]$/gm).length, 1);
  const tables = parseTomlTables(text);
  assert.equal(tables.get("javascript").get("strip_console"), "false");
  assert.equal(tables.get("javascript").get("cost_model"), '"gzip"');
  assert.equal(tables.get("javascript").get("candidate_search"), '"off"');
  assert.equal(tables.get("javascript").get("assume_pristine_builtins"), "false");
  assert.equal(tables.get("policy.tactics").get("naming-search"), '"off"');
  assert.equal(tables.get("mangle").get("exports"), "false");
  assert.throws(() => composeConfig(lane, [], "[javascript]\nstrip_console = true\n"), /strip_console/);
});

test("production lanes carry no tactic table", () => {
  const text = composeConfig(LANES.find((row) => row.id === "production/raw/script"), ["inlining"], null);
  assert.doesNotMatch(text, /policy\.tactics|candidate_search/);
  assert.match(text, /cost_model = "raw"/);
});

test("objective rewriting replaces, inserts or appends cost_model", () => {
  assert.equal(rewriteObjective('[javascript]\ncost_model = "brotli"\n', "raw"), '[javascript]\ncost_model = "raw"\n');
  assert.equal(rewriteObjective("[package]\nname = 'x'\n[javascript]\nlevel = 13\n", "gzip"), "[package]\nname = 'x'\n[javascript]\ncost_model = \"gzip\"\nlevel = 13\n");
  assert.equal(rewriteObjective("[mangle]\nexports = false\n", "raw"), '[mangle]\nexports = false\n\n[javascript]\ncost_model = "raw"\n');
});

test("failing-test names from node:test, jest and TAP", () => {
  const spec = "✖ site (12.1ms)\n  ✖ has a lab (3ms)\n✖ failing tests:\n\ntest at test/a.mjs:1:1\n✖ has a lab (3ms)\nℹ tests 3\nℹ pass 1\nℹ fail 2\n";
  assert.deepEqual(failingTests(spec), ["has a lab", "site"]);
  assert.deepEqual(testTotals(spec), { tests: 3, pass: 1, fail: 2 });
  const jest = "  ● Default debug names - production\n\n  ● Console\n\nTests:       3 failed, 11 skipped, 766 passed, 780 total\n";
  assert.deepEqual(failingTests(jest), ["Default debug names - production"]);
  assert.deepEqual(testTotals(jest), { tests: 780, pass: 766, fail: 3 });
  assert.deepEqual(failingTests("not ok 1 - real\nnot ok 2 - later # TODO not yet\n"), ["real"]);
  const vitest = "   × no built-in pattern is ReDoS-vulnerable 80196ms\n FAIL  tests/redos.test.ts > no built-in pattern is ReDoS-vulnerable\n";
  assert.deepEqual(failingTests(vitest), ["no built-in pattern is ReDoS-vulnerable", "tests/redos.test.ts > no built-in pattern is ReDoS-vulnerable"]);
});

test("the port ledger: regressions, removals, and intermittent entries", () => {
  const entries = [
    { tests: ["a", "b"], reason: "r", owner: "M1" },
    { tests: ["slow"], intermittent: true, reason: "timeout under load", owner: "M2.6" },
  ];
  assert.deepEqual(diffAgainstLedger(["a", "new"], entries, true), {
    ledgered: ["a"],
    regressions: ["new"],
    nowPassing: ["b"],
  });
  assert.deepEqual(diffAgainstLedger(["slow"], entries, true).regressions, []);
  assert.deepEqual(diffAgainstLedger([], entries, true).nowPassing, ["a", "b"]);
  assert.deepEqual(diffAgainstLedger([], entries, false).nowPassing, []);
});
