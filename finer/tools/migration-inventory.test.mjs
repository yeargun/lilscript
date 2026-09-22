import assert from 'node:assert/strict'
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'
import { collectMigrationInventory, validateMigrationInventory } from './migration-inventory.mjs'
import { fileIdentity, fingerprint, snapshotInputs } from './artifact-evidence.mjs'
import { runNodeTestEvidence } from './node-test-evidence.mjs'

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'lilscript-migration-inventory-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const siblings = join(root, 'ports'), workspace = join(siblings, 'samplelil')
  mkdirSync(join(workspace, 'src'), { recursive: true })
  mkdirSync(join(workspace, 'scripts'))
  mkdirSync(join(workspace, 'test'))
  writeFileSync(join(workspace, 'src/index.lil'), 'export int answer() { return 42; }\n')
  writeFileSync(join(workspace, 'scripts/build.mjs'), 'throw Error("inventory must not execute me");\n')
  writeFileSync(join(workspace, 'lilscript.toml'), '[javascript]\ncost_model="brotli"\n')
  writeFileSync(join(workspace, 'expected.out'), '42\n')
  writeFileSync(join(workspace, 'package.json'), JSON.stringify({ name: 'sample', version: '1.0.0', dependencies: { required: '1.0.0' }, scripts: { build: 'node scripts/build.mjs', test: 'node --test test/api.test.mjs' } }))
  const manifestPath = join(root, 'workloads.json')
  const document = { libraries: [{ id: 'samplelil', workspace, kind: 'maintained-library', required: true, package: { declaredEntryFiles: ['dist/missing.js'] } }, { id: 'missinglil', workspace: join(siblings, 'missinglil'), required: true }], additionalCoverage: [{ id: 'relatedlil', workspace: join(siblings, 'relatedlil'), kind: 'related-checkout', required: true, separateCompetitiveRow: false }] }
  writeFileSync(manifestPath, JSON.stringify(document))
  return { root, siblings, workspace, manifestPath, document, options: { manifestPath, siblings, generatedAt: '2026-09-12T00:00:00.000Z' } }
}

test('inventory preserves missing and related rows without executing scripts', t => {
  const f = fixture(t), report = collectMigrationInventory(f.options)
  assert.equal(report.rows.length, 3)
  assert.deepEqual(report.summary.missingWorkspaces, ['missinglil', 'relatedlil'])
  assert.equal(report.rows[2].separateCompetitiveRow, false)
  assert.ok(report.rows[0].gaps.some(gap => gap.code === 'dependency-metadata-missing'))
  assert.ok(report.rows[0].gaps.some(gap => gap.code === 'declared-artifact-missing'))
  assert.ok(report.rows.every(row => row.certification === 'unverified'))
  assert.ok(report.rows[0].inputs.files.some(file => file.path === 'expected.out'))
  assert.equal(report.contentSha256, collectMigrationInventory(f.options).contentSha256)
})

test('source-file imports are not misclassified as sibling directories', t => {
  const f = fixture(t)
  writeFileSync(join(f.workspace, 'scripts/build.mjs'), 'const imports=["../host.lil", "../kernel.lil", "../requiredlil/src/index.lil"];')
  const row = collectMigrationInventory(f.options).rows[0]
  assert.deepEqual(row.sourceGraphReferences.map(item => item.path), ['../requiredlil'])
})

test('config and installed dependency identities participate in the inventory', t => {
  const f = fixture(t), before = collectMigrationInventory(f.options)
  const dependency = join(f.workspace, 'node_modules/required/package.json')
  mkdirSync(join(f.workspace, 'node_modules/required'), { recursive: true })
  writeFileSync(dependency, JSON.stringify({ name: 'required', version: '1.0.0' }))
  const after = collectMigrationInventory(f.options)
  assert.notEqual(before.contentSha256, after.contentSha256)
  assert.equal(after.rows[0].dependencies[0].installed.present, true)
  writeFileSync(join(f.workspace, 'lilscript.toml'), '[javascript]\ncost_model="raw"\n')
  assert.notEqual(after.rows[0].inputs.sha256, collectMigrationInventory(f.options).rows[0].inputs.sha256)
})

test('recovery does not hide the missing recorded workspace or duplicate identities', t => {
  const f = fixture(t)
  f.document.libraries[0].workspaceCandidates = [f.workspace]
  f.document.libraries[0].workspace = join(f.root, 'missing-recorded')
  writeFileSync(f.manifestPath, JSON.stringify(f.document))
  const row = collectMigrationInventory(f.options).rows[0]
  assert.equal(row.workspace, f.workspace)
  assert.equal(row.selection, 'explicit-recovery-candidate')
  assert.ok(row.gaps.some(gap => gap.code === 'recorded-workspace-missing-recovered-copy-needs-audit'))
  f.document.additionalCoverage.push({ id: 'samplelil' })
  writeFileSync(f.manifestPath, JSON.stringify(f.document))
  assert.throws(() => collectMigrationInventory(f.options), /duplicate workload/)
})

test('declared missing dependency roots reject the shared input snapshot', t => {
  const f = fixture(t)
  assert.throws(() => snapshotInputs(f.workspace, { paths: ['src', 'required-source-dependency'] }), /declared input is missing/)
  mkdirSync(join(f.workspace, 'required-source-dependency'))
  const dependency = join(f.workspace, 'required-source-dependency/value.lil')
  writeFileSync(dependency, 'int required = 1;')
  const before = snapshotInputs(f.workspace, { paths: ['src', 'required-source-dependency'] })
  writeFileSync(dependency, 'int required = 2;')
  assert.notEqual(before.sha256, snapshotInputs(f.workspace, { paths: ['src', 'required-source-dependency'] }).sha256)
})

test('Vitest inventory pins original configuration and keeps additional coverage gaps', t => {
  const f = fixture(t)
  const testPath = join(f.workspace, 'test/api.test.mjs'), configPath = join(f.workspace, 'vitest.config.mjs')
  writeFileSync(testPath, 'test("original",()=>{});')
  writeFileSync(configPath, 'export default {test:{include:["test/*.mjs"]}};')
  const inventory = { schemaVersion: 1, kind: 'required-vitest-case-inventory', workload: 'samplelil', requiredCases: ['original'], testFiles: [{ path: 'test/api.test.mjs', ...fileIdentity(testPath) }], configuration: { path: 'vitest.config.mjs', ...fileIdentity(configPath) }, additionalRequiredCoverage: ['package exports'] }
  writeFileSync(join(f.root, 'cases.json'), JSON.stringify(inventory))
  f.document.libraries[0].tests = { vitestAdapter: { config: 'vitest.config.mjs', caseInventory: 'cases.json', artifacts: ['dist/missing.js'] } }
  writeFileSync(f.manifestPath, JSON.stringify(f.document))
  const row = collectMigrationInventory(f.options).rows[0]
  assert.equal(row.tests.frozenInventory.valid, true)
  assert.ok(row.gaps.some(gap => gap.code === 'additional-required-test-coverage-pending'))
  assert.ok(!row.gaps.some(gap => gap.code === 'production-test-adapter-missing'))
  writeFileSync(configPath, 'export default {test:{include:["narrowed/*.mjs"]}};')
  assert.equal(collectMigrationInventory(f.options).rows[0].tests.frozenInventory.valid, false)
})

test('Node inventory detects newly selected files from the original test glob', t => {
  const f = fixture(t), testPath = join(f.workspace, 'test/api.test.mjs')
  writeFileSync(testPath, 'test("original",()=>{});')
  const inventory = { schemaVersion: 1, kind: 'required-node-case-inventory', workload: 'samplelil', requiredCases: ['original'], testFiles: [{ path: 'test/api.test.mjs', ...fileIdentity(testPath) }], entryPatterns: ['test/*.test.mjs'] }
  writeFileSync(join(f.root, 'cases.json'), JSON.stringify(inventory))
  f.document.libraries[0].tests = { nodeAdapter: { files: ['test/api.test.mjs'], caseInventory: 'cases.json' } }
  writeFileSync(f.manifestPath, JSON.stringify(f.document))
  assert.equal(collectMigrationInventory(f.options).rows[0].tests.frozenInventory.valid, true)
  writeFileSync(join(f.workspace, 'test/added.test.mjs'), 'test("new original",()=>{});')
  const row = collectMigrationInventory(f.options).rows[0]
  assert.equal(row.tests.frozenInventory.valid, false)
  assert.ok(row.tests.frozenInventory.errors.some(error => error.includes('original test selection differs')))
})

test('a missing runtime dependency cannot pass frozen production cases', async t => {
  const f = fixture(t), dependencyRoot = join(f.workspace, 'node_modules/required')
  mkdirSync(dependencyRoot, { recursive: true })
  mkdirSync(join(f.workspace, 'dist'))
  writeFileSync(join(dependencyRoot, 'package.json'), JSON.stringify({ name: 'required', version: '1.0.0', main: 'index.cjs' }))
  writeFileSync(join(dependencyRoot, 'index.cjs'), 'module.exports = 42;')
  writeFileSync(join(f.workspace, 'dist/library.cjs'), 'module.exports = require("required");')
  writeFileSync(join(f.workspace, 'test/api.test.mjs'), 'import {test} from "node:test";import assert from "node:assert/strict";import {createRequire} from "node:module";const require=createRequire(import.meta.url);test("dependency result",()=>assert.equal(require("../dist/library.cjs"),42));')
  const options = { cwd: f.workspace, files: ['test/api.test.mjs'], artifactPaths: ['dist/library.cjs'] }
  const reference = await runNodeTestEvidence({ ...options, directory: join(f.root, 'reference') })
  assert.equal(reference.evidence.exitCode, 0, readFileSync(join(f.root, 'reference/runner.log'), 'utf8'))
  rmSync(dependencyRoot, { recursive: true })
  const candidate = await runNodeTestEvidence({ ...options, requiredCases: reference.inventoryCandidate, requiredTestFiles: reference.testFiles, directory: join(f.root, 'missing-dependency') })
  assert.notEqual(candidate.evidence.exitCode, 0)
  assert.equal(candidate.certification, 'unverified')
  assert.ok(candidate.errors.some(error => /did not pass|test command failed/.test(error)))
})

test('node_modules code is an explicit limitation of the shared default snapshot', t => {
  const f = fixture(t), dependencyRoot = join(f.workspace, 'node_modules/required')
  mkdirSync(dependencyRoot, { recursive: true })
  const path = join(dependencyRoot, 'index.cjs')
  writeFileSync(path, 'module.exports = 42;')
  writeFileSync(join(dependencyRoot, 'package.json'), JSON.stringify({ name: 'required', version: '1.0.0' }))
  const before = snapshotInputs(f.workspace), inventory = collectMigrationInventory(f.options)
  writeFileSync(path, 'module.exports = 43;')
  assert.equal(before.sha256, snapshotInputs(f.workspace).sha256, 'this negative probe documents a dependency-code identity gap, not certification')
  const after = collectMigrationInventory(f.options)
  assert.notEqual(inventory.rows[0].dependencies[0].installed.content.sha256, after.rows[0].dependencies[0].installed.content.sha256, 'the migration inventory must identify direct installed code even when package metadata is unchanged')
  assert.ok(validateMigrationInventory(inventory, { manifestPath: f.manifestPath, verifyCurrentInputs: true }).some(error => error.startsWith('installed dependency changed')))
  writeFileSync(join(f.workspace, 'lilscript.toml'), '[javascript]\ncost_model="raw"\n')
  assert.ok(validateMigrationInventory(after, { manifestPath: f.manifestPath, verifyCurrentInputs: true }).some(error => error.startsWith('configuration changed')))
  assert.match(after.rows[0].dependencyEvidence, /transitive.*unverified/)
})

test('frozen case inventories reject empty cases and changed source or fixture identities', t => {
  const f = fixture(t), testPath = join(f.workspace, 'test/api.test.mjs')
  writeFileSync(testPath, 'export const sentinel = 1;')
  f.document.libraries[0].tests = { requiredCaseInventory: 'cases.json', nodeAdapter: { files: ['test/api.test.mjs'], caseInventory: 'cases.json', artifacts: ['dist/missing.js'] } }
  writeFileSync(f.manifestPath, JSON.stringify(f.document))
  const inventory = { schemaVersion: 1, kind: 'required-node-case-inventory', workload: 'samplelil', requiredCases: ['one'], testFiles: [{ path: 'test/api.test.mjs', ...fileIdentity(testPath) }], fixtures: [{ path: 'expected.out', ...fileIdentity(join(f.workspace, 'expected.out')) }] }
  const cases = join(f.root, 'cases.json')
  writeFileSync(cases, JSON.stringify(inventory))
  assert.deepEqual(collectMigrationInventory(f.options).summary.rowsWithFrozenTestInventory, ['samplelil'])
  for (const edit of [() => { inventory.requiredCases = [] }, () => { inventory.requiredCases = ['one', 'one'] }, () => { inventory.requiredCases = ['one']; writeFileSync(testPath, 'export const sentinel = 2;') }, () => { inventory.testFiles[0] = { path: 'test/api.test.mjs', ...fileIdentity(testPath) }; writeFileSync(join(f.workspace, 'expected.out'), '43\n') }]) {
    edit(); writeFileSync(cases, JSON.stringify(inventory))
    const row = collectMigrationInventory(f.options).rows[0]
    assert.equal(row.tests.frozenInventory.valid, false)
    assert.ok(row.gaps.some(gap => gap.code === 'frozen-required-case-inventory-invalid'))
  }
  writeFileSync(cases, 'null')
  assert.equal(collectMigrationInventory(f.options).rows[0].tests.frozenInventory.valid, false)
})

test('declared adapter inputs and configurations stay gaps when missing', t => {
  const f = fixture(t)
  f.document.libraries[0].configurations = [{ path: 'missing.toml' }]
  f.document.libraries[0].tests = { nodeAdapter: { files: ['test/missing.test.mjs'], sharedFixture: 'missing-fixture.mjs' } }
  writeFileSync(f.manifestPath, JSON.stringify(f.document))
  const gaps = collectMigrationInventory(f.options).rows[0].gaps.map(gap => gap.code)
  for (const code of ['declared-configuration-missing', 'declared-test-file-missing', 'shared-test-fixture-missing']) assert.ok(gaps.includes(code))
})

test('inventory validation rejects omissions, tampered hashes, role changes and completion claims', t => {
  const f = fixture(t), report = collectMigrationInventory(f.options)
  const validate = row => validateMigrationInventory(row, { manifestPath: f.manifestPath })
  const rehash = row => { row.contentSha256 = fingerprint({ manifest: row.manifest, rows: row.rows, unmanifestedSiblingCandidates: row.unmanifestedSiblingCandidates }); return row }
  assert.deepEqual(validate(report), [])
  const omitted = structuredClone(report); omitted.rows.pop()
  assert.ok(validate(rehash(omitted)).includes('required workload identity set differs'))
  const tampered = structuredClone(report); tampered.rows[0].inputs.sha256 = '0'.repeat(64)
  assert.ok(validate(tampered).includes('inventory content hash differs'))
  const role = structuredClone(report); role.rows[0].required = false
  assert.ok(validate(rehash(role)).some(error => error.startsWith('workload role differs')))
  const certified = structuredClone(report); certified.status = 'complete'; certified.rows[0].certification = 'verified'
  assert.ok(validate(rehash(certified)).some(error => error.includes('cannot certify')))
  const summary = structuredClone(report); summary.summary.rows = 0
  assert.ok(validate(summary).includes('inventory summary differs from workload rows'))
  f.document.objective = 'changed'; writeFileSync(f.manifestPath, JSON.stringify(f.document))
  assert.ok(validate(report).includes('workload manifest changed'))
})
