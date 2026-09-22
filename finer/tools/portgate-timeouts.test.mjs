import assert from "node:assert/strict"
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"
import test from "node:test"
import { fileIdentity } from "./artifact-evidence.mjs"
import { runBoundedCommand } from "./bounded-command.mjs"

const portgate = join(dirname(fileURLToPath(import.meta.url)), "portgate.mjs")

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "lilscript-portgate-timeouts-"))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const workspace = join(root, "samplelil")
  for (const path of ["src", "scripts", "test"]) mkdirSync(join(workspace, path), { recursive: true })
  writeFileSync(join(workspace, "src/entry.lil"), "export int answer(){return 42;}\n")
  writeFileSync(join(workspace, "scripts/build.mjs"), `import{mkdirSync}from"node:fs";import{spawnSync}from"node:child_process";import{resolve}from"node:path";mkdirSync("dist");const r=spawnSync(process.env.LILSCRIPT_COMPILER,[resolve("src/entry.lil"),"-o",resolve("dist/library.mjs")],{stdio:"inherit"});process.exit(r.status??1);\n`)
  writeFileSync(join(workspace, "test/api.test.mjs"), `import{test}from"node:test";import assert from"node:assert/strict";import{answer}from"../dist/library.mjs";
test("answer",()=>assert.equal(answer,42));
`)
  const compiler = join(root, "test-compiler"), codec = join(root, "test-codec")
  writeFileSync(compiler, `#!${process.execPath}\nconst fs=require("node:fs"),args=process.argv.slice(2);fs.writeFileSync(args[args.indexOf("-o")+1],"export const answer=42;\\n");\n`, { mode: 0o755 })
  // This stub exercises phase ownership only, not compression correctness.
  const codecs = { gzip9: { encoder: "upstream-stock-zlib-c", libraryVersion: "1.3.1", level: 9, mtime: 0 }, brotli11: { encoder: "official-google-brotli-c", libraryVersion: "1.1.0", quality: 11, lgwin: 22, mode: "generic" } }
  writeFileSync(codec, `#!${process.execPath}\nconst fs=require("node:fs");console.log(JSON.stringify({schemaVersion:1,codecs:${JSON.stringify(codecs)},artifacts:process.argv.slice(3).map(path=>({path,raw:fs.statSync(path).size,gzip9:1,brotli11:1}))}));\n`, { mode: 0o755 })
  const inventory = { schemaVersion: 1, kind: "required-node-case-inventory", workload: "samplelil", requiredCases: [JSON.stringify(["test/api.test.mjs", 2, 1, 0, "test", "answer", 1])], testFiles: [{ path: "test/api.test.mjs", ...fileIdentity(join(workspace, "test/api.test.mjs")) }], entryPatterns: ["test/*.test.mjs"] }
  const manifest = join(root, "workloads.json")
  writeFileSync(join(root, "cases.json"), JSON.stringify(inventory))
  writeFileSync(manifest, JSON.stringify({ libraries: [{ id: "samplelil", required: true, workspace, tests: { nodeAdapter: { files: ["test/api.test.mjs"], artifacts: ["dist/library.mjs"], caseInventory: "cases.json" } } }] }))
  return { root, workspace, compiler, codec, manifest }
}

async function record(f, out, flags) {
  return runBoundedCommand(process.execPath, [portgate, "record", "--ports", "samplelil", "--manifest", f.manifest, "--compiler", f.compiler, "--codec", f.codec, "--out", out, ...flags], { timeoutMs: 15000, encoding: "utf8", maxBuffer: 1 << 20 })
}

test("portgate default phase bounds inherit --timeout and each override reaches its owner", async t => {
  const f = fixture(t)
  for (const [index, flags, expected] of [[0, ["--timeout", "3"], { build: 3000, test: 3000, codec: 3000 }], [1, ["--timeout", "3", "--build-timeout", "4", "--test-timeout", "5", "--codec-timeout", "1.5"], { build: 4000, test: 5000, codec: 1500 }]]) {
    const out = join(f.root, `arm-${index}`), result = await record(f, out, flags)
    assert.equal(result.status, 0, result.stderr)
    const manifest = JSON.parse(readFileSync(join(out, "manifest.json")))
    const row = JSON.parse(readFileSync(join(out, "samplelil.json")))
    const tests = JSON.parse(readFileSync(join(out, "tests/samplelil/report.json")))
    assert.deepEqual(manifest.timeoutsMs, expected)
    assert.equal(row.build.supervision.timeoutMs, expected.build)
    assert.equal(row.measurement.supervision.timeoutMs, expected.codec)
    assert.equal(tests.supervision.timeoutMs, expected.test)
    assert.equal(row.tests.certification, "verified")
  }
})

test("invalid portgate phase bounds fail before creating the arm workspace", async t => {
  const f = fixture(t)
  const invalid = [["--timeout", "0"], ["--build-timeout", "-1"], ["--test-timeout", "Infinity"], ["--codec-timeout", "0.0001"], ["--codec-timeout", "2147483.648"], ["--test-timeout"]]
  for (const [index, flags] of invalid.entries()) {
    const out = join(f.root, `invalid-${index}`), result = await record(f, out, flags)
    assert.notEqual(result.status, 0)
    assert.match(result.stderr, /timeout must specify/)
    assert.equal(existsSync(out), false)
    assert.equal(existsSync(join(f.workspace, "dist")), false)
  }
})

test("portgate rejects explicit null prerequisites instead of silently skipping their schema", async t => {
  const f = fixture(t), path = join(f.root, "cases.json")
  const inventory = JSON.parse(readFileSync(path))
  inventory.prerequisites = null
  writeFileSync(path, JSON.stringify(inventory))
  const out = join(f.root, "null-prerequisites"), result = await record(f, out, ["--timeout", "3"])
  assert.notEqual(result.status, 0)
  assert.match(result.stderr, /prerequisites must be an array/)
  assert.equal(existsSync(join(out, "tests/samplelil/report.json")), false)
})
