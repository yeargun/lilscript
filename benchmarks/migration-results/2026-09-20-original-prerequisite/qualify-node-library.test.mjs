import assert from "node:assert/strict"
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"
import test from "node:test"
import { digest, fileIdentity, fingerprint } from "./artifact-evidence.mjs"
import { runBoundedCommand } from "./bounded-command.mjs"
import { qualifyNodeLibrary, readQualifiedDiscovery } from "./qualify-node-library.mjs"

const ownerUrl = new URL("./qualify-node-library.mjs", import.meta.url)
const ownerPath = fileURLToPath(ownerUrl)
const json = path => JSON.parse(readFileSync(path, "utf8"))
const save = (path, value) => writeFileSync(path, JSON.stringify(value, null, 2) + "\n")

function discoveryFixture(t, script = "test:types") {
  const root = temporary(t), reportPath = join(root, "report.json")
  const npmIdentity = { path: "/pinned/npm", sha256: "a".repeat(64), bytes: 54 }
  const input = { path: "package.json", sha256: "b".repeat(64), bytes: 100 }
  const declaration = { id: "original-types", executable: npmIdentity, args: ["run", script], timeoutMs: 60_000, inputs: [input] }
  const rows = [npmIdentity, input].map(expected => ({ expected, actual: { sha256: expected.sha256, bytes: expected.bytes }, matches: true }))
  const report = {
    suiteExecuted: true, errors: ["required test inventory missing or ambiguous"],
    evidence: { exitCode: 0, cases: [{ id: "original", status: "pass" }] },
    prerequisites: [{ declaration, attempted: true, status: 0, signal: null, error: null,
      supervision: { timeoutMs: 60_000, timedOut: false, childExitObserved: true, forcedPipeClosure: false },
      before: structuredClone(rows), after: structuredClone(rows), final: structuredClone(rows) }],
  }
  const inventory = { requiredCases: ["original"], prerequisites: [declaration] }, discovery = {}
  const refresh = () => {
    save(reportPath, report)
    discovery.report = inventory.capturedFrom = { path: reportPath, ...fileIdentity(reportPath) }
  }
  refresh()
  return { reportPath, npmIdentity, declaration, report, inventory, discovery, refresh,
    read: () => readQualifiedDiscovery(inventory, discovery, reportPath, npmIdentity, 90_000) }
}

test("qualified original prerequisites retain their exact script names and declarations", t => {
  for (const script of ["test:types", "check:types", "check:declarations"]) {
    const f = discoveryFixture(t, script), before = structuredClone(f.inventory)
    assert.deepEqual(f.read(), f.report)
    assert.deepEqual(f.inventory, before)
    assert.deepEqual(f.read().prerequisites[0].declaration.args, ["run", script])
  }
})

test("both the accepted receipt and inventory must pin the actual discovery report", t => {
  for (const target of ["receipt", "inventory", "file"]) {
    const f = discoveryFixture(t)
    if (target === "receipt") f.discovery.report = { ...f.discovery.report, sha256: "0".repeat(64) }
    if (target === "inventory") f.inventory.capturedFrom = { ...f.inventory.capturedFrom, bytes: 0 }
    if (target === "file") writeFileSync(f.reportPath, readFileSync(f.reportPath, "utf8") + "\n")
    assert.throws(f.read, /discovery report identity differs/)
  }
})

test("prerequisite renaming omission optional commands and enlarged bounds cannot bypass discovery", t => {
  const mutations = [
    f => { f.inventory.prerequisites = [] },
    f => { f.inventory.prerequisites = [f.declaration, f.declaration] },
    f => { f.inventory.prerequisites = [{ ...f.declaration, args: ["run", "different"] }] },
    f => { f.declaration.args = ["run", "--if-present"] },
    f => { f.declaration.args = ["run", "test:types", "--if-present"] },
    f => { f.declaration.args = ["exec", "test:types"] },
    f => { f.declaration.timeoutMs = 90_001 },
    f => { f.declaration.timeoutMs = 0 },
    f => { f.declaration.timeoutMs = 1.5 },
    f => { f.declaration.inputs = [] },
    f => { f.declaration.executable = { ...f.npmIdentity, sha256: "0".repeat(64) } },
  ]
  for (const mutate of mutations) {
    const f = discoveryFixture(t)
    mutate(f); f.refresh()
    assert.throws(f.read, { name: "AssertionError" })
  }
})

test("failed skipped stale or incompletely supervised original prerequisites cannot qualify", t => {
  const mutations = [
    f => { f.report.suiteExecuted = false },
    f => { f.report.errors.push("another error") },
    f => { f.report.evidence.exitCode = 1 },
    f => { f.report.evidence.cases[0].status = "skip" },
    f => { f.report.evidence.cases = [] },
    f => { f.report.prerequisites[0].attempted = false },
    f => { f.report.prerequisites[0].status = 1 },
    f => { f.report.prerequisites[0].supervision.timedOut = true },
    f => { f.report.prerequisites[0].supervision.forcedPipeClosure = true },
    f => { f.report.prerequisites[0].supervision.childExitObserved = false },
    f => { f.report.prerequisites[0].supervision.timeoutMs = 60_001 },
    ...["before", "after", "final"].flatMap(stage => [
      f => { f.report.prerequisites[0][stage] = [] },
      f => { f.report.prerequisites[0][stage][0].matches = false },
      f => { f.report.prerequisites[0][stage][0].actual.sha256 = "0".repeat(64) },
    ]),
  ]
  for (const mutate of mutations) {
    const f = discoveryFixture(t)
    mutate(f); f.refresh()
    assert.throws(f.read, { name: "AssertionError" })
  }
})

test("Mdast Remark-Rehype and GFM original receipts pass the same prerequisite owner without execution", () => {
  for (const [directory, workload, script] of [
    ["2026-09-19-mdast-to-hast-adapter", "mdast-util-to-hastlil", "test:types"],
    ["2026-09-19-remark-rehype-adapter", "remark-rehypelil", "test:types"],
    ["2026-09-20-remark-gfm-adapter", "remark-gfmlil", "check:types"],
  ]) {
    const base = fileURLToPath(new URL(`../../benchmarks/migration-results/${directory}/existing-dist/`, import.meta.url))
    const inventoryPath = fileURLToPath(new URL(`../../benchmarks/libraries/${workload}.required-tests.json`, import.meta.url))
    const inventory = json(inventoryPath), discovery = json(join(base, "receipt.json"))
    const npm = inventory.prerequisites[0].executable.path, identity = { path: npm, ...fileIdentity(npm) }
    const report = readQualifiedDiscovery(inventory, discovery, join(base, "node/report.json"), identity, 90_000)
    assert.deepEqual(report.prerequisites[0].declaration.args, ["run", script])
  }
})

function temporary(t) {
  const root = mkdtempSync(join(tmpdir(), "lilscript-node-qualification-test-"))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  return root
}

function fixture(t) {
  const root = temporary(t)
  const workspace = join(root, "workspace")
  const discovery = join(root, "discovery")
  mkdirSync(workspace)
  mkdirSync(join(discovery, "node"), { recursive: true })
  for (const name of ["receipt.json", "before.json", "node/report.json"]) save(join(discovery, name), {})
  const inventoryPath = join(root, "required-tests.json")
  save(inventoryPath, {})
  const invoked = join(root, "compiler-invoked")
  const compiler = join(root, "lilscript")
  const codec = join(root, "lilscript-codec")
  const executable = `#!${process.execPath}\nrequire("node:fs").writeFileSync(${JSON.stringify(invoked)}, "unexpected producer invocation");process.exit(99);\n`
  for (const path of [compiler, codec]) writeFileSync(path, executable, { mode: 0o755 })
  const source = join(root, "parent-source.lil")
  writeFileSync(source, "int answer = 42;\n")
  const files = [{ path: "parent-source.lil", ...fileIdentity(source) }]
  const inputSha256 = digest(JSON.stringify(files))
  for (const name of ["inputs-before.json", "inputs-after.json"]) save(join(root, name), { files, sha256: inputSha256 })
  const parentPath = join(root, "parent.json")
  const parent = {
    passed: true, inputsStable: true, profile: "release", publicIntegration: true,
    inputSha256, binaries: [compiler, codec].map(path => ({ path, ...fileIdentity(path) })),
  }
  save(parentPath, parent)
  const recipe = {
    workload: "fixture-library",
    workspace,
    inventoryPath,
    discovery: { directory: discovery, sha256: fileIdentity(join(discovery, "receipt.json")).sha256 },
    caseCount: 1,
    testFileCount: 1,
    artifactStem: "fixture",
    esmBanner: "/*! fixture */\n",
    kind: "source-pinned-fixture-baseline",
    armLabel: "fixture-parent-refusal",
    scope: "Synthetic parent refusal only; no library qualification.",
    limitations: ["Fixture producers must never execute."],
    defaultParent: { receipt: parentPath, sha256: fileIdentity(parentPath).sha256, compiler, codec },
  }
  const entryPath = join(root, "caller.mjs")
  const refreshEntry = () => writeFileSync(entryPath,
    `import { qualifyNodeLibrary } from ${JSON.stringify(ownerUrl.href)};\nconst recipe = ${JSON.stringify(recipe)};\nawait qualifyNodeLibrary(recipe, process.argv.slice(2), ${JSON.stringify(entryPath)});\n`)
  refreshEntry()
  return { root, workspace, parent, parentPath, recipe, entryPath, invoked, refreshEntry }
}

test("importing the shared qualification owner has no CLI or filesystem effects", async t => {
  const root = temporary(t)
  const output = join(root, "must-not-exist")
  const before = readdirSync(root)
  const result = await runBoundedCommand(process.execPath, ["--input-type=module", "--eval", `
process.argv = [process.execPath, "pretend-client.mjs", ${JSON.stringify(output)}];
const before = process.exitCode;
const owner = await import(${JSON.stringify(ownerUrl.href)});
if (typeof owner.qualifyNodeLibrary !== "function" || process.exitCode !== before) throw Error("import changed CLI state");
process.stdout.write("imported\\n");
`], { cwd: root, timeoutMs: 10_000, maxBuffer: 1024 * 1024, encoding: "utf8" })
  assert.equal(result.error, undefined)
  assert.equal(result.signal, null)
  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.supervision.timedOut, false)
  assert.equal(result.stdout, "imported\n")
  assert.equal(result.stderr, "")
  assert.deepEqual(readdirSync(root), before)
  assert.equal(existsSync(output), false)
})

test("invalid data-only recipes are rejected before creating an output directory", async t => {
  const f = fixture(t)
  const cases = [
    { ...f.recipe, skipTests: true },
    { ...f.recipe, workload: "../other-workload" },
    { ...f.recipe, workspace: "relative-workspace" },
    { ...f.recipe, caseCount: 0 },
    { ...f.recipe, testFileCount: 1.5 },
    { ...f.recipe, artifactStem: "../output" },
    { ...f.recipe, esmBanner: "missing newline" },
    { ...f.recipe, limitations: [() => true] },
    { ...f.recipe, discovery: { ...f.recipe.discovery, sha256: "unverified" } },
    { ...f.recipe, defaultParent: { ...f.recipe.defaultParent, skipValidation: true } },
  ]
  for (const [index, recipe] of cases.entries()) {
    const output = join(f.root, `invalid-recipe-${index}`)
    await assert.rejects(qualifyNodeLibrary(recipe, [output], f.entryPath), { name: "AssertionError" }, `recipe ${index}`)
    assert.equal(existsSync(output), false, `recipe ${index} created output`)
  }
  assert.equal(existsSync(f.invoked), false)
})

test("malformed CLI options are rejected before creating an output directory", async t => {
  const f = fixture(t)
  const cases = [
    ["--unknown", "value"],
    ["--compiler"],
    ["--compiler", ""],
    ["--compiler", "--codec"],
    ["--compiler", "first", "--compiler", "second"],
    ["--parent-receipt", f.parentPath],
    ["--parent-sha256", "0".repeat(64)],
    ["--parent-receipt", f.parentPath, "--parent-sha256", "not-a-hash"],
  ]
  for (const [index, arguments_] of cases.entries()) {
    const output = join(f.root, `invalid-cli-${index}`)
    await assert.rejects(qualifyNodeLibrary(f.recipe, [output, ...arguments_], f.entryPath), { name: "AssertionError" }, `CLI case ${index}`)
    assert.equal(existsSync(output), false, `CLI case ${index} created output`)
  }
  await assert.rejects(qualifyNodeLibrary(f.recipe, [], f.entryPath), { name: "AssertionError" })
  assert.equal(existsSync(f.invoked), false)
})

test("both real thin clients validate their recipe and CLI before refusing an existing output", async t => {
  const f = fixture(t)
  const output = join(f.root, "already-exists")
  mkdirSync(output)
  const sentinel = join(output, "keep.txt")
  writeFileSync(sentinel, "existing output must remain unchanged\n")
  const before = { names: readdirSync(output), sentinel: fileIdentity(sentinel) }
  for (const [client, options] of [
    ["2026-09-19-mdast-to-hast-baseline", ["--parent-receipt", f.parentPath, "--parent-sha256", f.recipe.defaultParent.sha256, "--compiler", f.recipe.defaultParent.compiler, "--codec", f.recipe.defaultParent.codec]],
    ["2026-09-19-remark-rehype-baseline", []],
  ]) {
    const path = fileURLToPath(new URL(`../../benchmarks/migration-results/${client}/qualify.mjs`, import.meta.url))
    const result = await runBoundedCommand(process.execPath, [path, output, ...options], {
      cwd: f.root, timeoutMs: 10_000, maxBuffer: 1024 * 1024, encoding: "utf8",
    })
    assert.equal(result.error, undefined, client)
    assert.equal(result.signal, null, client)
    assert.equal(result.supervision.timedOut, false, client)
    assert.equal(result.status, 1, `${client}: ${result.stderr}`)
    assert.match(result.stderr, /EEXIST/, client)
    assert.equal(result.stdout, "", client)
    assert.deepEqual({ names: readdirSync(output), sentinel: fileIdentity(sentinel) }, before, client)
    assert.equal(existsSync(f.invoked), false, client)
  }
})

test("parent rejection retains both caller and owner evidence without invoking producers", async t => {
  for (const reason of ["receipt-hash", "unqualified-parent", "compiler-hash"]) {
    const f = fixture(t)
    if (reason === "receipt-hash") f.recipe.defaultParent.sha256 = "0".repeat(64)
    if (reason === "unqualified-parent") f.parent.publicIntegration = false
    if (reason === "compiler-hash") f.parent.binaries[0].sha256 = "0".repeat(64)
    if (reason !== "receipt-hash") {
      save(f.parentPath, f.parent)
      f.recipe.defaultParent.sha256 = fileIdentity(f.parentPath).sha256
    }
    f.refreshEntry()
    const output = join(f.root, "refused")
    const exitCode = process.exitCode
    const receipt = await qualifyNodeLibrary(f.recipe, [output], f.entryPath)
    t.after(() => rmSync(dirname(receipt.arm), { recursive: true, force: true }))
    assert.equal(process.exitCode, exitCode, `${reason}: owner changed caller exit status`)
    assert.equal(receipt.passed, false, reason)
    assert.equal(receipt.originalTestBoundaryPassed, false, reason)
    assert.equal(receipt.packageDryRunPassed, false, reason)
    assert.equal(receipt.qualification, "unverified", reason)
    assert.equal(receipt.inputsStable, true, reason)
    assert.equal(typeof receipt.failure.message, "string", reason)
    if (reason === "unqualified-parent") {
      assert.match(receipt.failure.message, /parent\.passed === true && parent\.inputsStable === true && parent\.profile === "release" && parent\.publicIntegration === true/)
    } else {
      const rejectedPath = reason === "receipt-hash" ? f.parentPath : f.recipe.defaultParent.compiler
      assert.ok(receipt.failure.message.includes(fileIdentity(rejectedPath).sha256), `${reason}: wrong actual identity`)
      assert.ok(receipt.failure.message.includes("0".repeat(64)), `${reason}: wrong expected identity`)
    }
    for (const name of ["parent-receipt.json", "parent-inputs-before.json", "parent-inputs-after.json"]) {
      assert.equal(existsSync(join(output, name)), reason === "compiler-hash", `${reason}: wrong parent-validation stage`)
    }
    assert.equal(receipt.validationFailure, undefined, reason)
    assert.equal(receipt.archiveFailure, undefined, reason)
    assert.equal(receipt.outputManifestFailure, undefined, reason)
    assert.deepEqual(receipt.commands, [], reason)
    assert.equal(existsSync(f.invoked), false, reason)
    assert.deepEqual(json(join(output, "receipt.json")), receipt)
    const before = json(join(output, "inputs-before.json"))
    const after = json(join(output, "inputs-after.json"))
    assert.deepEqual(after, before, reason)
    assert.equal(before.sha256, fingerprint(before.files), reason)
    for (const path of [f.entryPath, ownerPath]) {
      const rows = before.files.filter(row => row.path === path)
      assert.equal(rows.length, 1, `${reason}: missing or duplicated input ${path}`)
      assert.deepEqual(rows[0], { path, ...fileIdentity(path) })
    }
    assert.deepEqual(fileIdentity(join(output, "wrapper.mjs")), fileIdentity(f.entryPath))
    assert.deepEqual(fileIdentity(join(output, "qualification-owner.mjs")), fileIdentity(ownerPath))
    for (const path of ["wrapper.mjs", "qualification-owner.mjs"]) {
      const row = receipt.outputs.find(row => row.path === path)
      assert.ok(row, `${reason}: missing archived output ${path}`)
      assert.deepEqual({ path: row.path, sha256: row.sha256, bytes: row.bytes }, { path, ...fileIdentity(join(output, path)) })
    }
  }
})
