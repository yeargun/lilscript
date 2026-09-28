// Unit tests for the pure parts of scripts/cases.mjs, scripts/ports.mjs and
// scripts/ratchet.mjs.
//   node --test scripts/verify-runners.test.mjs
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { codeOnly, composeConfig, LANES, parseTomlTables, selectLanes, walkCounts } from "./cases.mjs";
import { diffAgainstLedger, failingTests, rewriteObjective, suiteRan, testTotals } from "./ports.mjs";
import { applyLedger, catalogId, compareWithBaseline, countLosses, lossRows, selectItems, validateLedger } from "./ratchet.mjs";
import { validateIdiomDebt } from "./lib/idiom-debt.mjs";

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
  assert.equal(tables.get("javascript").has("strip_console"), false);
  assert.equal(tables.get("objective").get("codecs"), '["gzip"]');
  assert.equal(tables.get("javascript").get("candidate_search"), '"off"');
  assert.equal(tables.get("javascript").get("assume_pristine_builtins"), "false");
  assert.equal(tables.get("policy.tactics").get("naming-search"), '"off"');
  assert.equal(tables.get("mangle").get("exports"), "false");
  assert.throws(() => composeConfig(lane, [], "[javascript]\nstrip_console = false\n"), /strip_console/);
});

test("production lanes carry no tactic table", () => {
  const text = composeConfig(LANES.find((row) => row.id === "production/raw/script"), ["inlining"], null);
  assert.doesNotMatch(text, /policy\.tactics|candidate_search/);
  assert.match(text, /\[objective\]\ncodecs = \["raw"\]/);
  assert.doesNotMatch(text, /cost_model/);
});

test("objective rewriting replaces codecs or a port's cost_model, and inserts or appends codecs", () => {
  assert.equal(rewriteObjective('[objective]\ncodecs = ["brotli"]\n', "raw"), '[objective]\ncodecs = ["raw"]\n');
  assert.equal(rewriteObjective('[javascript]\ncost_model = "brotli"\n', "raw"), '[javascript]\ncost_model = "raw"\n');
  assert.equal(rewriteObjective("[package]\nname = 'x'\n[objective]\n[effort]\nlevel = 13\n", "gzip"), "[package]\nname = 'x'\n[objective]\ncodecs = [\"gzip\"]\n[effort]\nlevel = 13\n");
  assert.equal(rewriteObjective("[mangle]\nexports = false\n", "raw"), '[mangle]\nexports = false\n\n[objective]\ncodecs = ["raw"]\n');
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

test("a suite that ran no test reports no ledgered test as passing", () => {
  // Every jest suite failed to load (mobxlil, 2026-09-27): no test ran.
  const crashed = "FAIL tests/a.test.js\n  ● Test suite failed to run\n\nTest Suites: 37 failed, 37 total\nTests:       0 total\n";
  assert.equal(suiteRan({ ...testTotals(crashed), failing: failingTests(crashed) }), false);
  assert.equal(suiteRan(undefined), false);
  const ran = "Tests:       1 failed, 2 passed, 3 total\n";
  assert.equal(suiteRan({ ...testTotals(ran) }), true);
});

// The generic corpus ratchet (scripts/ratchet.mjs): verdicts, the baseline
// comparison and the ledger, on synthetic tables.
const pass = (size) => ({ state: "pass", size });
const lanes = (raw, gzip9, brotli11) => ({ raw: pass(raw), gzip9: pass(gzip9), brotli11: pass(brotli11) });
const bar = (raw, gzip9, brotli11) => ({ raw: { size: raw }, gzip9: { size: gzip9 }, brotli11: { size: brotli11 } });

test("ratchet: an item loses to a bar only when strictly larger, per metric", () => {
  const table = { "apps/a": lanes(100, 60, 50), "cases/catalog/loop/x": lanes(10, 10, 10) };
  const bars = { "apps/a": { competitor: bar(100, 61, 40), closure: bar(90, 60, 50) }, "cases/catalog/loop/x": { old: bar(9, 10, 11) } };
  const rows = lossRows(table, bars).map((row) => `${row.id} ${row.metric} ${row.bar} +${row.loss}`).sort();
  assert.deepEqual(rows, ["apps/a brotli11 competitor +10", "apps/a raw closure +10", "cases/catalog/loop/x raw old +1"]);
  const counts = countLosses(lossRows(table, bars));
  assert.equal(counts.apps.brotli11.competitor, 1);
  assert.equal(counts.apps.raw.closure, 1);
  assert.equal(counts.cases.raw.old, 1);
  assert.equal(counts.algorithms.raw.old, 0);
});

test("ratchet: growth, a new loss and a broken lane block; shrinking is an improvement", () => {
  const bars = { "apps/a": { competitor: bar(50, 50, 50) }, "apps/b": { competitor: bar(50, 50, 50) }, "apps/c": { competitor: bar(50, 50, 50) } };
  const baseline = { "apps/a": lanes(60, 40, 55), "apps/b": lanes(40, 40, 70), "apps/c": lanes(40, 40, 40) };
  const now = {
    "apps/a": lanes(61, 40, 55),
    "apps/b": lanes(40, 40, 60),
    "apps/c": { raw: pass(40), gzip9: { state: "wrong-output" }, brotli11: pass(51) },
    "apps/d": lanes(99, 99, 99),
  };
  const result = compareWithBaseline(now, baseline, bars);
  assert.deepEqual(result.grown.map((row) => `${row.id} ${row.metric} ${row.before}->${row.loss}`).sort(), ["apps/a raw 10->11", "apps/c brotli11 0->1"]);
  assert.deepEqual(result.shrunk.map((row) => `${row.id} ${row.metric} ${row.before}->${row.loss}`), ["apps/b brotli11 20->10"]);
  assert.deepEqual(result.broken.map((row) => `${row.id} ${row.metric} ${row.state}`), ["apps/c gzip9 wrong-output"]);
  assert.deepEqual(result.added, ["apps/d"]);
  assert.deepEqual(result.countChanges, [{ set: "apps", metric: "brotli11", bar: "competitor", before: 2, now: 3 }]);
});

test("ratchet: the ledger needs owners and reasons, covers by glob, bar and metric, and reports stale entries", () => {
  assert.deepEqual(validateLedger({ entries: [{ kind: "loss", items: "apps/*", reason: "r" }] }), ["ledger entry 1: no owner (a plan task)"]);
  assert.match(validateLedger({ entries: [{ kind: "failure", items: "x", bars: ["old"], reason: "r", owner: "M7" }] })[0], /names no bar/);
  const ledger = {
    entries: [
      { kind: "loss", items: ["cases/catalog/loop/*"], bars: ["competitor"], reason: "r", owner: "M10.9" },
      { kind: "loss", items: "apps/*", metrics: ["brotli11"], reason: "r", owner: "M7" },
      { kind: "failure", items: "cases/catalog/record/*", reason: "r", owner: "M10.8" },
      { kind: "loss", items: "algorithms/*", reason: "r", owner: "M7" },
    ],
  };
  const losses = [
    { id: "cases/catalog/loop/sum-1", metric: "raw", bar: "competitor" },
    { id: "cases/catalog/loop/sum-1", metric: "raw", bar: "old" },
    { id: "apps/a", metric: "brotli11", bar: "closure" },
    { id: "apps/a", metric: "gzip9", bar: "closure" },
  ];
  const failures = [{ id: "cases/catalog/record/json-1", metric: "raw", state: "refused" }, { id: "apps/b", metric: "raw", state: "crashed" }];
  const { unledgeredLosses, unledgeredFailures, stale } = applyLedger(ledger, losses, failures);
  assert.deepEqual(unledgeredLosses.map((row) => `${row.id} ${row.metric} ${row.bar}`), ["cases/catalog/loop/sum-1 raw old", "apps/a gzip9 closure"]);
  assert.deepEqual(unledgeredFailures.map((row) => row.id), ["apps/b"]);
  assert.deepEqual(stale.map(({ index }) => index), [3]);
});

test("ratchet: catalog ids carry the behavior family; filters take substrings and globs", () => {
  assert.equal(catalogId("loop/sum", "loop-sum-3"), "cases/catalog/loop/loop-sum-3");
  const items = ["apps/a", "cases/catalog/loop/x", "cases/canonical/control/dead-branch"].map((id) => ({ id }));
  assert.deepEqual(selectItems(items, "cases/*/loop/*,apps/").map((item) => item.id), ["apps/a", "cases/catalog/loop/x"]);
  assert.equal(selectItems(items, null).length, 3);
});

test("the idiom debt ledger validates, and a malformed entry is refused (NO4)", () => {
  const ledger = JSON.parse(readFileSync(new URL("../tests/idiom-debt.json", import.meta.url), "utf8"));
  assert.deepEqual(validateIdiomDebt(ledger, fileURLToPath(new URL("..", import.meta.url))), []);
  const entry = {
    id: "NO4-001", port: "katexlil", recorded: "2026-09-27",
    idiomatic: { description: "x instanceof C" }, workaround: { description: "C.prototype.isPrototypeOf(x)" },
    case: "comparison/cases/canonical/identity/snapshot-write", codec: "brotli11",
    sizes: { idiomatic: 120, workaround: 100, binary: "head-d1d48c4c" }, owner: "M10.7", status: "open",
  };
  assert.deepEqual(validateIdiomDebt({ schema: 1, about: "a", entries: [entry] }), []);
  assert.match(validateIdiomDebt({ schema: 1, about: "a", entries: [{ ...entry, owner: "" }] }).join("\n"), /owner/);
  assert.match(validateIdiomDebt({ schema: 1, about: "a", entries: [{ ...entry, case: "somewhere.lil" }] }).join("\n"), /regression case/);
  assert.match(validateIdiomDebt({ schema: 1, about: "a", entries: [{ ...entry, sizes: { ...entry.sizes, idiomatic: 90 } }] }).join("\n"), /paid/);
});

test("ratchet: a lane that starts compiling may lose without growth; its losses still need the ledger", () => {
  const bars = { "cases/catalog/record/r": { competitor: bar(10, 10, 10) } };
  const baseline = { "cases/catalog/record/r": { raw: { state: "refused" }, gzip9: { state: "refused" }, brotli11: { state: "refused" } } };
  const now = { "cases/catalog/record/r": lanes(12, 12, 12) };
  const result = compareWithBaseline(now, baseline, bars);
  assert.deepEqual(result.grown, []);
  assert.deepEqual(result.countChanges, []);
  assert.equal(result.fixed.length, 3);
  const { unledgeredLosses } = applyLedger({ entries: [] }, lossRows(now, bars), []);
  assert.equal(unledgeredLosses.length, 3);
});

test("the case runner reads a build's walk counts and stops from its explain report", () => {
  const report = {
    search: { codec_probes: 3, structures: 2, terminal: { objectives: [{
      examined: 9, judged: 4, pruned: 2, passes: 2, starts: [{ name: "level-0" }],
      stops: [{ level: 0, size: 120, sha256: "aa" }, { level: 1, size: 110, sha256: "bb" }],
    }] } },
  };
  const stderr = `warning: a retired key\n${JSON.stringify(report, null, 2)}\n`;
  assert.deepEqual(walkCounts(stderr), {
    searchProbes: 3, structures: 2, examined: 9, judged: 4, pruned: 2, passes: 2, starts: 1,
    stops: [{ level: 0, size: 120, sha256: "aa" }, { level: 1, size: 110, sha256: "bb" }],
  });
  assert.equal(walkCounts("no report"), null);
  // A level-0 build has no walk: its counts are zero.
  assert.equal(walkCounts(JSON.stringify({ search: { terminal: { objectives: [] } } }, null, 2)).judged, 0);
});
