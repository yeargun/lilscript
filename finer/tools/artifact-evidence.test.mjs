import assert from "node:assert/strict"
import { mkdtempSync, mkdirSync, readFileSync, readdirSync, rmSync, symlinkSync, unlinkSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import test from "node:test"
import { runCompilerReceipt } from "./compiler-receipt.mjs"
import {
  digest, fileIdentity, fingerprint, invokeCompiler, snapshotInputs, snapshotDependencyTree,
  validateBuild, validateInvocation, validateMeasurements, validateTests,
} from "./artifact-evidence.mjs"

function fixture(t, body = 'writeFileSync(output, "export const answer=42;\\n")') {
  const root = mkdtempSync(join(tmpdir(), "lilscript-evidence-test-"))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const cwd = join(root, "work")
  mkdirSync(join(cwd, "src"), { recursive: true })
  mkdirSync(join(cwd, "dist"))
  writeFileSync(join(cwd, "src", "index.lil"), "export int answer() { return 42; }\n")
  writeFileSync(join(cwd, "lilscript.toml"), '[javascript]\ncost_model = "brotli"\n')
  const compiler = join(root, "compiler.mjs")
  writeFileSync(compiler, `#!/usr/bin/env node\nimport { writeFileSync } from "node:fs";\nconst args=process.argv.slice(2);const output=args[args.indexOf("-o")+1];\n${body}\n`, { mode: 0o755 })
  const options = {
    compiler, compilerSha256: fileIdentity(compiler).sha256, cwd,
    args: ["src/index.lil", "-o", join(cwd, "dist", "index.js")],
    receiptDirectory: join(root, "receipts"), inputPaths: ["src", "lilscript.toml"],
    contractSha256: fingerprint({ target: "js-module", objective: "brotli" }),
  }
  return { root, cwd, options, output: options.args.at(-1) }
}

test("a real fast invocation produces a complete receipt without a duration floor", (t) => {
  const f = fixture(t)
  const { receipt, exitCode } = invokeCompiler(f.options)
  assert.equal(exitCode, 0)
  assert.deepEqual(receipt.validationErrors, [])
  assert.deepEqual(validateBuild({ exitCode: 0, invocations: [receipt], ...f.options }), [])
  assert.equal(receipt.output.sha256, fileIdentity(f.output).sha256)
  const recorded = JSON.parse(readFileSync(join(f.options.receiptDirectory, readdirSync(f.options.receiptDirectory).find((p) => p.endsWith(".json"))), "utf8"))
  assert.equal(recorded.id, receipt.id)
  // Duration is deliberately irrelevant even for sub-millisecond receipts.
  assert.deepEqual(validateInvocation({ ...receipt, elapsedMs: 0.01 }, f.options), [])
})

test("timing text cannot substitute for a compiler invocation", () => {
  assert.match(validateBuild({ exitCode: 0, invocations: [], timing: { lines: 1, wall_ms: 50000 } }).join(";"), /no compiler invocation/)
})

test("the production wrapper forwards explicit arguments and records its actual cwd", (t) => {
  const f = fixture(t)
  const settings = join(f.root, "settings.json")
  writeFileSync(settings, JSON.stringify(f.options))
  const previous = process.cwd()
  try {
    process.chdir(f.cwd)
    assert.equal(runCompilerReceipt(settings, f.options.args), 0)
  } finally {
    process.chdir(previous)
  }
  const name = readdirSync(f.options.receiptDirectory).find((path) => path.endsWith(".json"))
  const row = JSON.parse(readFileSync(join(f.options.receiptDirectory, name), "utf8"))
  assert.equal(row.cwd, f.cwd)
  assert.deepEqual(row.args, f.options.args)
})

test("an exit-zero compiler that writes nothing cannot inherit old dist bytes", (t) => {
  const f = fixture(t, "// deliberately writes no output")
  writeFileSync(f.output, "stale output")
  const { receipt, exitCode } = invokeCompiler(f.options)
  assert.notEqual(exitCode, 0)
  assert.match(receipt.validationErrors.join(";"), /no measured output/)
  assert.equal(readFileSync(receipt.output.previous.savedAt, "utf8"), "stale output")
})

test("a failed compiler cannot be accepted even if it writes output", (t) => {
  const f = fixture(t, 'writeFileSync(output,"bad");process.exit(7)')
  const result = invokeCompiler(f.options)
  assert.equal(result.exitCode, 7)
  assert.match(result.receipt.validationErrors.join(";"), /invocation failed/)
})

test("the exact intended compiler is required before touching an old output", (t) => {
  const f = fixture(t)
  writeFileSync(f.output, "preserve me")
  assert.throws(() => invokeCompiler({ ...f.options, compilerSha256: digest("another compiler") }), /identity changed/)
  assert.equal(readFileSync(f.output, "utf8"), "preserve me")
})

test("changes to inputs during compilation reject the receipt", (t) => {
  const f = fixture(t, 'writeFileSync("src/index.lil", "changed");writeFileSync(output,"answer")')
  const result = invokeCompiler(f.options)
  assert.notEqual(result.exitCode, 0)
  assert.match(result.receipt.validationErrors.join(";"), /inputs changed/)
})

test("post-build output substitution and changed contracts are detected", (t) => {
  const f = fixture(t)
  const { receipt } = invokeCompiler(f.options)
  writeFileSync(f.output, "development output")
  assert.match(validateInvocation(receipt, { ...f.options, verifyOutput: true }).join(";"), /output changed/)
  assert.match(validateInvocation(receipt, { ...f.options, contractSha256: digest("different config") }).join(";"), /contract identity/)
})

test("source identity includes configuration and generated files without git", (t) => {
  const f = fixture(t)
  const before = snapshotInputs(f.cwd)
  writeFileSync(join(f.cwd, "lilscript.toml"), '[javascript]\ncost_model = "raw"\n')
  const after = snapshotInputs(f.cwd)
  assert.equal(before.files.length, after.files.length)
  assert.notEqual(before.sha256, after.sha256)
  writeFileSync(join(f.cwd, "src", "generated.lil"), "generated input")
  assert.notEqual(after.sha256, snapshotInputs(f.cwd).sha256)
  writeFileSync(f.output, "ignored output changes")
  const saved = snapshotInputs(f.cwd)
  writeFileSync(f.output, "another ignored output")
  assert.equal(saved.sha256, snapshotInputs(f.cwd).sha256)
})

test("an explicit configuration outside the port is captured as an input", (t) => {
  const f = fixture(t)
  const config = join(f.root, "shared-control.toml")
  writeFileSync(config, '[optimization]\npreset="none"\n')
  const result = invokeCompiler({ ...f.options, args: [...f.options.args, "--config", config] })
  assert.equal(result.exitCode, 0)
  assert.equal(result.receipt.configuration.sha256, fileIdentity(config).sha256)
  assert.equal(result.receipt.configuration.location, config)
})

test("dependency snapshots include nested installed code and file and directory symlink destinations", t => {
  const f = fixture(t), root = join(f.root, "dependencies"), nested = join(root, "node_modules", "child")
  mkdirSync(nested, { recursive: true })
  writeFileSync(join(nested, "code.js"), "abc", { mode: 0o755 })
  symlinkSync("node_modules/child/code.js", join(root, "file-link"))
  symlinkSync("node_modules/child", join(root, "directory-link"))
  const before = snapshotDependencyTree(root)
  assert.deepEqual(before.files.map(row => row.path), ["", "directory-link", "directory-link/code.js", "file-link", "node_modules", "node_modules/child", "node_modules/child/code.js"])
  assert.equal(before.bytes, 9)
  assert.equal(before.sha256, fingerprint(before.files))
  assert.equal(before.files.find(row => row.path === "file-link").symlink, "node_modules/child/code.js")
  assert.equal(before.files.find(row => row.path === "directory-link").resolved, nested)
  assert(before.files.filter(row => row.kind === "file").every(row => row.executable && row.sha256 === digest("abc")))
  writeFileSync(join(nested, "code.js"), "changed")
  assert.notEqual(snapshotDependencyTree(root).sha256, before.sha256)
})

test("dependency snapshots enforce exact byte and entry caps and reject invalid bounds", t => {
  const f = fixture(t), root = join(f.root, "dependencies")
  mkdirSync(root)
  writeFileSync(join(root, "code.js"), "abc")
  const actual = snapshotDependencyTree(root, { maxBytes: 3, maxEntries: 2 })
  assert.equal(actual.bytes, 3)
  assert.equal(actual.files.length, 2)
  assert.throws(() => snapshotDependencyTree(root, { maxBytes: 2 }), /byte limit/)
  assert.throws(() => snapshotDependencyTree(root, { maxEntries: 1 }), /entry limit/)
  for (const key of ["maxBytes", "maxEntries"]) for (const value of [0, -1, 1.5, Infinity]) assert.throws(() => snapshotDependencyTree(root, { [key]: value }), /positive integers/)
})

test("dependency snapshots reject ancestor cycles and missing symlink destinations", t => {
  const f = fixture(t), root = join(f.root, "dependencies")
  mkdirSync(root)
  symlinkSync(".", join(root, "cycle"))
  assert.throws(() => snapshotDependencyTree(root), /symlink cycle/)
  unlinkSync(join(root, "cycle"))
  symlinkSync("missing", join(root, "broken"))
  assert.throws(() => snapshotDependencyTree(root), { code: "ENOENT" })
})

test("output declarations cannot move source inputs or escape via a symlink", (t) => {
  const f = fixture(t)
  assert.throws(() => invokeCompiler({ ...f.options, args: ["-o", "src/index.lil"] }), /outside the declared/)
  assert.throws(() => invokeCompiler({ ...f.options, outputRoots: ["."] }), /subdirectories/)
  mkdirSync(join(f.root, "outside"))
  symlinkSync(join(f.root, "outside"), join(f.cwd, "dist", "escape"))
  assert.throws(() => invokeCompiler({ ...f.options, args: ["-o", "dist/escape/new/answer.js"] }), /parent escapes/)
})

function testEvidence(f) {
  writeFileSync(f.output, "production")
  const artifact = { path: f.output, ...fileIdentity(f.output) }
  return { exitCode: 0, requiredCases: ["api", "behavior"], cases: [{ id: "api", status: "pass" }, { id: "behavior", status: "pass" }], requiredArtifacts: [artifact], loadedArtifacts: [artifact] }
}

test("all required tests must execute and pass against the production bytes", (t) => {
  const f = fixture(t)
  const report = testEvidence(f)
  assert.deepEqual(validateTests(report), [])
  assert.match(validateTests({ ...report, cases: [] }).join(";"), /executed test report missing/)
  assert.match(validateTests({ ...report, cases: report.cases.slice(0, 1) }).join(";"), /required case did not pass: behavior/)
  assert.match(validateTests({ ...report, cases: [{ id: "api", status: "pass" }, { id: "behavior", status: "skip" }] }).join(";"), /did not pass/)
  assert.match(validateTests({ ...report, exitCode: 1 }).join(";"), /test command failed/)
  assert.match(validateTests({ ...report, loadedArtifacts: [{ sha256: digest("development") }] }).join(";"), /was not tested/)
  writeFileSync(f.output, "substituted after tests")
  assert.match(validateTests(report).join(";"), /changed after testing/)
})

test("unknown or ambiguous test evidence fails closed", (t) => {
  const report = testEvidence(fixture(t))
  for (const change of [{ requiredCases: [] }, { cases: null }, { cases: {} }, { cases: [null] }, { loadedArtifacts: null }, { requiredArtifacts: null }, { cases: [...report.cases, report.cases[0]] }]) assert.ok(validateTests({ ...report, ...change }).length)
})

test("canonical measurements reject missing, duplicated, or incomplete rows", (t) => {
  const f = fixture(t)
  writeFileSync(f.output, "123")
  const row = { path: f.output, raw: 3, gzip9: 23, brotli11: 7 }
  const report = {
    schemaVersion: 1,
    codecs: {
      gzip9: { encoder: "upstream-stock-zlib-c", libraryVersion: "1.3.1", level: 9, mtime: 0 },
      brotli11: { encoder: "official-google-brotli-c", libraryVersion: "1.1.0", quality: 11, lgwin: 22, mode: "generic" },
    }, artifacts: [row],
  }
  assert.equal(validateMeasurements(report, [f.output]).get(f.output).brotli11, 7)
  assert.throws(() => validateMeasurements({ ...report, artifacts: [] }, [f.output]), /omitted/)
  assert.throws(() => validateMeasurements({ ...report, artifacts: [row, row] }, [f.output]), /duplicate/)
  assert.throws(() => validateMeasurements({ ...report, artifacts: [{ ...row, brotli11: null }] }, [f.output]), /invalid brotli11/)
  assert.throws(() => validateMeasurements({ ...report, codecs: { ...report.codecs, brotli11: { ...report.codecs.brotli11, quality: 5 } } }, [f.output]), /codec settings/)
})
