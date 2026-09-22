import assert from "node:assert/strict"
import { existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import test from "node:test"
import { runNodeTestEvidence } from "./node-test-evidence.mjs"
import { fileIdentity } from "./artifact-evidence.mjs"

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "lilscript-node-evidence-"))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const cwd = join(root, "work")
  mkdirSync(join(cwd, "dist"), { recursive: true })
  writeFileSync(join(cwd, "dist", "library.mjs"), "export const answer=42;\n")
  writeFileSync(join(cwd, "dist", "library.cjs"), 'if(!require.cache)throw Error("CommonJS API changed");module.exports={answer:42};\n')
  writeFileSync(join(cwd, "api.test.mjs"), `import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { createRequire } from "node:module";
import { answer } from "./dist/library.mjs";
const cjs = createRequire(import.meta.url)("./dist/library.cjs");
for (const group of ["first", "second"]) describe(group, () => {
  it("esm", () => assert.equal(answer,42));
  it("cjs", () => assert.equal(cjs.answer,42));
});
`)
  const options = { cwd, files: ["api.test.mjs"], artifactPaths: ["dist/library.mjs", "dist/library.cjs"] }
  return { root, cwd, options }
}

test("production tests capture exact ESM and CommonJS bytes without changing loaders", async (t) => {
  const f = fixture(t)
  const reference = await runNodeTestEvidence({ ...f.options, directory: join(f.root, "reference") })
  assert.equal(reference.evidence.exitCode, 0)
  assert.equal(reference.evidence.cases.filter((row) => row.type !== "suite").length, 4)
  assert.equal(new Set(reference.inventoryCandidate).size, reference.inventoryCandidate.length)
  assert.equal(reference.certification, "unverified", "a first inventory run cannot certify its own completeness")
  assert.ok(reference.evidence.loadedArtifacts.some((row) => row.format === "module"))
  assert.ok(reference.evidence.loadedArtifacts.some((row) => row.format === "commonjs"))
  const candidate = await runNodeTestEvidence({ ...f.options, requiredCases: reference.inventoryCandidate, directory: join(f.root, "candidate") })
  assert.deepEqual(candidate.errors, [])
  assert.equal(candidate.certification, "verified")
  assert.equal(candidate.runtime.sha256, fileIdentity(process.execPath).sha256)
  assert.ok(candidate.toolInputs.some(input => input.path.endsWith("/bounded-command.mjs")))
  assert.equal(candidate.runtime.supervision.timeoutMs, 10000)
})

test("a failed runtime version probe cannot certify otherwise passing original tests", async (t) => {
  const f = fixture(t)
  const reference = await runNodeTestEvidence({ ...f.options, directory: join(f.root, "reference") })
  const wrapper = join(f.root, "node-wrapper")
  writeFileSync(wrapper, `#!${process.execPath}\nconst {spawnSync}=require('node:child_process');if(process.argv[2]==='--version')process.exit(3);const result=spawnSync(${JSON.stringify(process.execPath)},process.argv.slice(2),{stdio:'inherit'});process.exit(result.status??1);\n`, { mode: 0o755 })
  const report = await runNodeTestEvidence({ ...f.options, node: wrapper, requiredCases: reference.inventoryCandidate, directory: join(f.root, "candidate") })
  assert.equal(report.evidence.exitCode, 0)
  assert.equal(report.certification, "unverified")
  assert.match(report.errors.join(";"), /runtime version probe did not complete/)
})

test("runtime replacement during passing tests invalidates the pinned evidence", async (t) => {
  const f = fixture(t)
  const reference = await runNodeTestEvidence({ ...f.options, directory: join(f.root, "reference") })
  const wrapper = join(f.root, "node-wrapper")
  writeFileSync(wrapper, `#!${process.execPath}\nconst {spawnSync}=require('node:child_process');const result=spawnSync(${JSON.stringify(process.execPath)},process.argv.slice(2),{stdio:'inherit'});if(process.argv[2]!=='--version')require('node:fs').appendFileSync(__filename,'// changed\\n');process.exit(result.status??1);\n`, { mode: 0o755 })
  const report = await runNodeTestEvidence({ ...f.options, node: wrapper, requiredCases: reference.inventoryCandidate, directory: join(f.root, "candidate") })
  assert.equal(report.evidence.exitCode, 0)
  assert.equal(report.certification, "unverified")
  assert.match(report.errors.join(";"), /tool or runtime changed during execution/)
})

test("removing a required case cannot turn a passing subset into certification", async (t) => {
  const f = fixture(t)
  const reference = await runNodeTestEvidence({ ...f.options, directory: join(f.root, "reference") })
  const candidate = await runNodeTestEvidence({ ...f.options, requiredCases: reference.inventoryCandidate, nodeArguments: ["--test-name-pattern=esm"], directory: join(f.root, "candidate") })
  assert.equal(candidate.certification, "unverified")
  assert.match(candidate.errors.join(";"), /did not pass/)
})

test("tests loading a development artifact cannot certify the production artifact", async (t) => {
  const f = fixture(t)
  const reference = await runNodeTestEvidence({ ...f.options, directory: join(f.root, "reference") })
  writeFileSync(join(f.cwd, "development.mjs"), "export const answer=42;\n")
  const path = join(f.cwd, "api.test.mjs")
  writeFileSync(path, readFileSync(path, "utf8").replace("./dist/library.mjs", "./development.mjs"))
  const candidate = await runNodeTestEvidence({ ...f.options, requiredCases: reference.inventoryCandidate, directory: join(f.root, "candidate") })
  assert.equal(candidate.evidence.exitCode, 0)
  assert.equal(candidate.certification, "unverified")
  assert.match(candidate.errors.join(";"), /was not tested/)
})

test("an empty test file cannot establish a case inventory", async (t) => {
  const f = fixture(t)
  writeFileSync(join(f.cwd, "empty.test.mjs"), "// no declarations\n")
  const report = await runNodeTestEvidence({ ...f.options, files: ["empty.test.mjs"], directory: join(f.root, "empty") })
  assert.equal(report.evidence.cases.length, 0)
  assert.equal(report.certification, "unverified")
})

test("inventory discovery retains failed and skipped declarations", async (t) => {
  const f = fixture(t)
  writeFileSync(join(f.cwd, "failing.test.mjs"), `import {test} from "node:test";
test("required failure",()=>{throw Error("regression")});
test.skip("temporarily skipped",()=>{});
`)
  const report = await runNodeTestEvidence({ ...f.options, files: ["failing.test.mjs"], directory: join(f.root, "failing") })
  assert.notEqual(report.evidence.exitCode, 0)
  assert.equal(report.inventoryCandidate.length, 2)
  assert.deepEqual(report.inventoryCandidate, report.evidence.cases.map((row) => row.id))
  assert.equal(report.certification, "unverified")
})

test("artifacts executed by a nested Node process are observed as well", async (t) => {
  const f = fixture(t)
  writeFileSync(join(f.cwd, "child.test.mjs"), `import {test} from "node:test";
import assert from "node:assert/strict";
import {spawnSync} from "node:child_process";
test("child uses production",()=>{const r=spawnSync(process.execPath,["-e",'if(require("./dist/library.cjs").answer!==42)process.exit(1)'],{encoding:"utf8"});assert.equal(r.status,0,r.stderr)});
`)
  const report = await runNodeTestEvidence({ ...f.options, files: ["child.test.mjs"], artifactPaths: ["dist/library.cjs"], directory: join(f.root, "child") })
  assert.equal(report.evidence.exitCode, 0)
  assert.equal(report.evidence.cases.length, 1)
  assert.equal(report.evidence.loadedArtifacts.length, 1)
  assert.equal(report.evidence.loadedArtifacts[0].format, "commonjs")
})

test("unchanged case names cannot conceal changed required test source", async (t) => {
  const f = fixture(t)
  const reference = await runNodeTestEvidence({ ...f.options, directory: join(f.root, "reference") })
  const path = join(f.cwd, "api.test.mjs")
  writeFileSync(path, `${readFileSync(path, "utf8")}\n// changed after inventory capture\n`)
  const candidate = await runNodeTestEvidence({ ...f.options, requiredCases: reference.inventoryCandidate, requiredTestFiles: reference.testFiles, directory: join(f.root, "candidate") })
  assert.equal(candidate.evidence.exitCode, 0)
  assert.equal(candidate.certification, "unverified")
  assert.match(candidate.errors.join(";"), /required test source differs/)
})

test("new files selected by the original glob cannot disappear from a frozen adapter", async (t) => {
  const f = fixture(t)
  const reference = await runNodeTestEvidence({ ...f.options, requiredFilePatterns: ["*.test.mjs"], directory: join(f.root, "reference") })
  writeFileSync(join(f.cwd, "added.test.mjs"), 'import {test} from "node:test";test("new required case",()=>{});')
  const candidate = await runNodeTestEvidence({ ...f.options, requiredCases: reference.inventoryCandidate, requiredTestFiles: reference.testFiles, requiredFilePatterns: ["*.test.mjs"], directory: join(f.root, "candidate") })
  assert.equal(candidate.evidence.exitCode, 0)
  assert.equal(candidate.certification, "unverified")
  assert.match(candidate.errors.join(";"), /original test selection differs/)
})

test("imported test source identities cannot change behind stable entry files and case names", async (t) => {
  const f = fixture(t), imported = join(f.cwd, "imported.mjs")
  writeFileSync(imported, 'import {test} from "node:test";test("imported",()=>{});')
  const entry = join(f.cwd, "api.test.mjs")
  writeFileSync(entry, `${readFileSync(entry, "utf8")}\nimport "./imported.mjs";\n`)
  const requiredFixtures = [{ path: "imported.mjs", ...fileIdentity(imported) }]
  const reference = await runNodeTestEvidence({ ...f.options, directory: join(f.root, "reference") })
  writeFileSync(imported, `${readFileSync(imported, "utf8")}\n// changed imported source\n`)
  const candidate = await runNodeTestEvidence({ ...f.options, requiredCases: reference.inventoryCandidate, requiredTestFiles: reference.testFiles, requiredFixtures, directory: join(f.root, "candidate") })
  assert.equal(candidate.evidence.exitCode, null)
  assert.equal(candidate.suiteExecuted, false)
  assert.equal(candidate.certification, "unverified")
  assert.match(candidate.errors.join(";"), /required fixture differs: imported.mjs/)
})

test("missing or changed expectations cannot run self-updating tests or prerequisites", async t => {
  const f = fixture(t), expected = join(f.cwd, "tree.json"), sentinel = join(f.cwd, "checked")
  writeFileSync(expected, "{\"answer\":42}\n")
  const requiredFixtures = [{ path: "tree.json", ...fileIdentity(expected) }]
  writeFileSync(join(f.cwd, "api.test.mjs"), 'import{writeFileSync}from"node:fs";writeFileSync("tree.json","regenerated");\n')
  const command = prerequisite(f, 'import{writeFileSync}from"node:fs";writeFileSync("checked","unexpected");\n')
  for (const state of ["missing", "changed"]) {
    if (state === "missing") rmSync(expected)
    else writeFileSync(expected, "changed")
    const report = await runNodeTestEvidence({ ...f.options, requiredFixtures, prerequisites: [command], directory: join(f.root, state) })
    assert.equal(report.suiteExecuted, false)
    assert.equal(report.prerequisites[0].attempted, false)
    assert.equal(report.evidence.exitCode, null)
    assert.deepEqual(report.evidence.cases, [])
    assert.equal(existsSync(sentinel), false)
    assert.equal(report.fixtureInputs.before[0].matches, false)
    assert.match(report.errors.join(";"), /required fixture differs: tree.json/)
    if (state === "missing") assert.equal(existsSync(expected), false)
    else assert.equal(readFileSync(expected, "utf8"), "changed")
  }
})

test("a prerequisite changing an initially valid expectation prevents suite execution", async t => {
  const f = fixture(t), expected = join(f.cwd, "tree.json")
  writeFileSync(expected, "original")
  const requiredFixtures = [{ path: "tree.json", ...fileIdentity(expected) }]
  const command = prerequisite(f, 'import{writeFileSync}from"node:fs";writeFileSync("tree.json","changed");\n')
  const report = await runNodeTestEvidence({ ...f.options, requiredFixtures, prerequisites: [command], directory: join(f.root, "candidate") })
  assert.equal(report.prerequisites[0].status, 0)
  assert.equal(report.fixtureInputs.before[0].matches, true)
  assert.equal(report.fixtureInputs.afterPrerequisites[0].matches, false)
  assert.equal(report.suiteExecuted, false)
  assert.deepEqual(report.evidence.cases, [])
})

test("expectation mutation by an executed suite remains a failing final identity check", async t => {
  const f = fixture(t), expected = join(f.cwd, "tree.json")
  writeFileSync(expected, "original")
  const requiredFixtures = [{ path: "tree.json", ...fileIdentity(expected) }]
  const entry = join(f.cwd, "api.test.mjs")
  writeFileSync(entry, readFileSync(entry, "utf8") + '\nimport{writeFileSync}from"node:fs";writeFileSync("tree.json","changed");\n')
  const report = await runNodeTestEvidence({ ...f.options, requiredFixtures, directory: join(f.root, "candidate") })
  assert.equal(report.suiteExecuted, true)
  assert.equal(report.evidence.exitCode, 0)
  assert.equal(report.fixtureInputs.before[0].matches, true)
  assert.equal(report.fixtureInputs.final[0].matches, false)
  assert.equal(report.certification, "unverified")
  assert.match(report.errors.join(";"), /required fixture differs: tree.json/)
})

test("malformed or duplicate fixture declarations reject before creating evidence", async t => {
  const f = fixture(t), pin = { path: "api.test.mjs", ...fileIdentity(join(f.cwd, "api.test.mjs")) }
  for (const [index, requiredFixtures] of [null, [null], [{ ...pin, bytes: -1 }], [{ ...pin, sha256: "unknown" }], [pin, { ...pin, path: "./api.test.mjs" }]].entries()) {
    const directory = join(f.root, `invalid-${index}`)
    await assert.rejects(runNodeTestEvidence({ ...f.options, requiredFixtures, directory }), /unique paths and pinned identities/)
    assert.equal(existsSync(directory), false)
  }
})

function prerequisite(f, source, overrides = {}) {
  const path = join(f.cwd, "prerequisite.mjs")
  writeFileSync(path, source)
  return { id: "original-typecheck", executable: { path: process.execPath, ...fileIdentity(process.execPath) }, args: ["prerequisite.mjs"], inputs: [{ path: "prerequisite.mjs", ...fileIdentity(path) }], timeoutMs: 2000, ...overrides }
}

test("pinned prerequisites run before the unchanged Node case inventory", async (t) => {
  const f = fixture(t)
  const reference = await runNodeTestEvidence({ ...f.options, directory: join(f.root, "reference") })
  const command = prerequisite(f, 'import{writeFileSync}from"node:fs";writeFileSync("checked","yes");console.log("original typecheck");\n')
  const path = join(f.cwd, "api.test.mjs")
  writeFileSync(path, `${readFileSync(path, "utf8")}\nimport{readFileSync}from"node:fs";assert.equal(readFileSync("checked","utf8"),"yes");\n`)
  const report = await runNodeTestEvidence({ ...f.options, prerequisites: [command], requiredCases: reference.inventoryCandidate, directory: join(f.root, "candidate") })
  assert.equal(report.certification, "verified")
  assert.equal(report.suiteExecuted, true)
  assert.deepEqual(report.inventoryCandidate, reference.inventoryCandidate)
  assert.equal(report.prerequisites[0].status, 0)
  assert.equal(report.prerequisites[0].supervision.timeoutMs, 2000)
  assert.equal(report.prerequisites[0].command, process.execPath)
  assert.equal(readFileSync(report.prerequisites[0].stdout.path, "utf8"), "original typecheck\n")
  assert.ok(report.prerequisites[0].before.every(input => input.matches))
  assert.ok(report.prerequisites[0].after.every(input => input.matches))
  assert.ok(report.prerequisites[0].final.every(input => input.matches))
})

test("prerequisite failure retains logs and skips later commands and the original Node suite", async (t) => {
  const f = fixture(t)
  const reference = await runNodeTestEvidence({ ...f.options, directory: join(f.root, "reference") })
  const command = prerequisite(f, 'console.error("original type failure");process.exitCode=7;\n')
  const later = { ...command, id: "later", args: ["prerequisite.mjs", "later"] }
  const report = await runNodeTestEvidence({ ...f.options, prerequisites: [command, later], requiredCases: reference.inventoryCandidate, directory: join(f.root, "candidate") })
  assert.equal(report.certification, "unverified")
  assert.equal(report.suiteExecuted, false)
  assert.equal(report.prerequisites[0].status, 7)
  assert.equal(report.prerequisites[1].attempted, false)
  assert.equal(report.evidence.exitCode, null)
  assert.deepEqual(report.evidence.cases, [])
  assert.deepEqual(report.evidence.loadedArtifacts, [])
  assert.deepEqual(report.evidence.requiredCases, reference.inventoryCandidate)
  assert.equal(readFileSync(report.prerequisites[0].stderr.path, "utf8"), "original type failure\n")
  assert.match(report.errors.join(";"), /Node suite was not executed/)
})

test("prerequisite timeout cannot turn unexecuted tests into a pass", async (t) => {
  const f = fixture(t)
  const command = prerequisite(f, "setInterval(()=>{},1000);\n", { timeoutMs: 40 })
  const report = await runNodeTestEvidence({ ...f.options, prerequisites: [command], requiredCases: ["required-original-case"], directory: join(f.root, "candidate") })
  assert.equal(report.certification, "unverified")
  assert.equal(report.suiteExecuted, false)
  assert.equal(report.prerequisites[0].supervision.timedOut, true)
  assert.deepEqual(report.inventoryCandidate, [])
})

test("changed pinned prerequisite scripts are refused before command execution", async (t) => {
  const f = fixture(t)
  const command = prerequisite(f, 'import{writeFileSync}from"node:fs";writeFileSync("should-not-run","");\n')
  writeFileSync(join(f.cwd, "prerequisite.mjs"), `${readFileSync(join(f.cwd, "prerequisite.mjs"), "utf8")}\n// changed\n`)
  const report = await runNodeTestEvidence({ ...f.options, prerequisites: [command], requiredCases: ["required-original-case"], directory: join(f.root, "candidate") })
  assert.equal(report.prerequisites[0].attempted, false)
  assert.equal(report.suiteExecuted, false)
  assert.equal(existsSync(join(f.cwd, "should-not-run")), false)
  assert.match(report.errors.join(";"), /input differs before execution/)
})

test("a prerequisite mutating a pinned input blocks the Node suite even with exit zero", async (t) => {
  const f = fixture(t)
  const command = prerequisite(f, 'import{appendFileSync}from"node:fs";appendFileSync("prerequisite.mjs","// changed\\n");\n')
  const report = await runNodeTestEvidence({ ...f.options, prerequisites: [command], requiredCases: ["required-original-case"], directory: join(f.root, "candidate") })
  assert.equal(report.prerequisites[0].status, 0)
  assert.equal(report.suiteExecuted, false)
  assert.equal(report.certification, "unverified")
  assert.match(report.errors.join(";"), /input changed during execution/)
})

test("passing Node tests cannot conceal a changed prerequisite input", async (t) => {
  const f = fixture(t)
  const reference = await runNodeTestEvidence({ ...f.options, directory: join(f.root, "reference") })
  const command = prerequisite(f, "// original prerequisite\n")
  const path = join(f.cwd, "api.test.mjs")
  writeFileSync(path, `${readFileSync(path, "utf8")}\nimport{appendFileSync}from"node:fs";appendFileSync("prerequisite.mjs","// changed\\n");\n`)
  const report = await runNodeTestEvidence({ ...f.options, prerequisites: [command], requiredCases: reference.inventoryCandidate, directory: join(f.root, "candidate") })
  assert.equal(report.evidence.exitCode, 0)
  assert.equal(report.suiteExecuted, true)
  assert.equal(report.certification, "unverified")
  assert.match(report.errors.join(";"), /input differs after test execution/)
})

test("prerequisite schema rejects empty, unpinned, ambiguous and unbounded commands", async (t) => {
  const f = fixture(t)
  const command = prerequisite(f, "// original prerequisite\n")
  const invalid = [null, [{}], [{ ...command, id: "" }], [command, command], [{ ...command, executable: { ...command.executable, path: "" } }], [{ ...command, executable: { ...command.executable, sha256: "wrong" } }], [{ ...command, args: [] }], [{ ...command, args: "node script" }], [{ ...command, inputs: [] }], [{ ...command, inputs: [...command.inputs, ...command.inputs] }], [{ ...command, timeoutMs: 0 }], [{ ...command, timeoutMs: Infinity }], [command, { ...command, id: "same-command" }]]
  for (const [index, prerequisites] of invalid.entries()) await assert.rejects(runNodeTestEvidence({ ...f.options, prerequisites, directory: join(f.root, `invalid-${index}`) }), /prerequisite/)
})
