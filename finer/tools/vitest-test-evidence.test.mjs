import assert from 'node:assert/strict'
import { existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import test from 'node:test'
import { fileIdentity } from './artifact-evidence.mjs'
import { runVitestTestEvidence } from './vitest-test-evidence.mjs'

const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
const vitestPackage = resolve(process.env.LILSCRIPT_VITEST_PACKAGE ?? join(repository, '../zodlil/node_modules/vitest'))
const available = existsSync(join(vitestPackage, 'dist/node.js')) && typeof RegExp.escape === 'function'
const options = { skip: available ? false : 'requires Node 24 and an installed Vitest 4 package; set LILSCRIPT_VITEST_PACKAGE' }

function fixture(t, source) {
  const root = mkdtempSync(join(tmpdir(), 'lilscript-vitest-evidence-')), cwd = join(root, 'workspace')
  t.after(() => rmSync(root, { recursive: true, force: true }))
  mkdirSync(join(cwd, 'dist'), { recursive: true }); mkdirSync(join(cwd, 'test'))
  symlinkSync(dirname(vitestPackage), join(cwd, 'node_modules'), 'dir')
  writeFileSync(join(cwd, 'package.json'), '{"type":"module"}')
  writeFileSync(join(cwd, 'vitest.config.mjs'), 'export default {test:{include:["test/**/*.test.mjs"]}};')
  writeFileSync(join(cwd, 'dist/index.js'), 'export const value=42;\n')
  writeFileSync(join(cwd, 'dist/development.js'), 'export const value=42;\n')
  writeFileSync(join(cwd, 'test/api.test.mjs'), source)
  const run = (name, extra = {}) => runVitestTestEvidence({ cwd, config: 'vitest.config.mjs', vitestPackage, artifactPaths: ['dist/index.js'], directory: join(root, name), timeoutMs: 30000, ...extra })
  const expected = { requiredConfig: fileIdentity(join(cwd, 'vitest.config.mjs')), requiredTestFiles: [{ path: 'test/api.test.mjs', ...fileIdentity(join(cwd, 'test/api.test.mjs')) }] }
  return { root, cwd, run, expected }
}

test('Vitest observes exact native production bytes and full duplicate-name case identities', options, async t => {
  const f = fixture(t, 'import {describe,it,expect} from "vitest";import {value} from "../dist/index.js";describe("suite",()=>{it("same",()=>expect(value).toBe(42));it("same",()=>expect(value+1).toBe(43));});')
  const discovered = await f.run('discovery', { collectOnly: true })
  assert.deepEqual(discovered.errors, [], readFileSync(join(f.root, 'discovery/runner.log'), 'utf8'))
  assert.equal(discovered.certification, 'unverified')
  assert.equal(discovered.inventoryCandidate.length, 2)
  assert.equal(new Set(discovered.inventoryCandidate).size, 2)
  const result = await f.run('execution', { ...f.expected, requiredCases: discovered.inventoryCandidate })
  assert.deepEqual(result.errors, [], readFileSync(join(f.root, 'execution/runner.log'), 'utf8'))
  assert.equal(result.certification, 'verified')
  assert.ok(result.evidence.loadedArtifacts.some(row => row.kind === 'node-module-load' && row.sha256 === fileIdentity(join(f.cwd, 'dist/index.js')).sha256))
})

test('Vitest discovery retains skipped and todo declarations and execution cannot certify them', options, async t => {
  const f = fixture(t, 'import {it,expect} from "vitest";import {value} from "../dist/index.js";it("live",()=>expect(value).toBe(42));it.skip("skipped",()=>{});it.todo("todo");')
  const discovered = await f.run('discovery', { collectOnly: true })
  assert.deepEqual(discovered.errors, [], readFileSync(join(f.root, 'discovery/runner.log'), 'utf8'))
  assert.equal(discovered.inventoryCandidate.length, 3)
  const result = await f.run('execution', { ...f.expected, requiredCases: discovered.inventoryCandidate })
  assert.equal(result.certification, 'unverified')
  assert.ok(result.errors.some(error => error.includes('required case did not pass')))
})

test('passing development imports cannot qualify an unobserved production artifact', options, async t => {
  const f = fixture(t, 'import {it,expect} from "vitest";import {value} from "../dist/development.js";it("answer",()=>expect(value).toBe(42));')
  const result = await f.run('execution', { ...f.expected, requiredCases: [JSON.stringify(['test/api.test.mjs', 'answer', 1])] })
  assert.equal(result.evidence.exitCode, 0, readFileSync(join(f.root, 'execution/runner.log'), 'utf8'))
  assert.equal(result.certification, 'unverified')
  assert.ok(result.errors.some(error => error.includes('production artifact was not tested')))
})

test('mocked production modules are not confused with loaded production bytes', options, async t => {
  const f = fixture(t, 'import {it,expect,vi} from "vitest";vi.mock("../dist/index.js",()=>({value:42}));import {value} from "../dist/index.js";it("answer",()=>expect(value).toBe(42));')
  const result = await f.run('execution', { ...f.expected, requiredCases: [JSON.stringify(['test/api.test.mjs', 'answer', 1])] })
  assert.equal(result.evidence.exitCode, 0, readFileSync(join(f.root, 'execution/runner.log'), 'utf8'))
  assert.equal(result.certification, 'unverified')
  assert.ok(result.errors.some(error => error.includes('production artifact was not tested')))
})

test('missing cases and changed test/configuration identities cannot qualify', options, async t => {
  const f = fixture(t, 'import {it,expect} from "vitest";import {value} from "../dist/index.js";it("answer",()=>expect(value).toBe(42));')
  const result = await f.run('execution', { requiredConfig: { sha256: '0'.repeat(64) }, requiredTestFiles: [{ path: 'test/api.test.mjs', sha256: '0'.repeat(64) }], requiredCases: [JSON.stringify(['test/api.test.mjs', 'missing', 1])] })
  assert.equal(result.certification, 'unverified')
  assert.ok(result.errors.some(error => error.includes('required case did not pass')))
  assert.ok(result.errors.some(error => error.includes('required test source differs')))
  assert.ok(result.errors.some(error => error.includes('required Vitest configuration')))
})

test('passing tests cannot hide direct imports of a forbidden upstream implementation', options, async t => {
  const f = fixture(t, 'import {it,expect} from "vitest";import {value} from "../dist/index.js";import {original} from "../vendor/original.js";it("answer",()=>expect(original+value).toBe(84));')
  mkdirSync(join(f.cwd, 'vendor'))
  writeFileSync(join(f.cwd, 'vendor/original.js'), 'export const original=42;')
  const result = await f.run('execution', { ...f.expected, requiredCases: [JSON.stringify(['test/api.test.mjs', 'answer', 1])], forbiddenSources: [{ root: 'vendor' }] })
  assert.equal(result.evidence.exitCode, 0, readFileSync(join(f.root, 'execution/runner.log'), 'utf8'))
  assert.equal(result.certification, 'unverified')
  assert.deepEqual(result.sourceRoutes.modules.map(row => row.path), ['vendor/original.js'])
  assert.ok(result.errors.some(error => error.includes('forbidden implementation')))
})

test('native dependencies cannot bypass the forbidden upstream implementation check', options, async t => {
  const f = fixture(t, 'import {it,expect} from "vitest";import {value} from "../dist/index.js";it("answer",()=>expect(value).toBe(42));')
  mkdirSync(join(f.cwd, 'vendor'))
  writeFileSync(join(f.cwd, 'vendor/original.js'), 'export const original=42;')
  writeFileSync(join(f.cwd, 'dist/index.js'), 'export {original as value} from "../vendor/original.js";')
  const result = await f.run('execution', { ...f.expected, requiredCases: [JSON.stringify(['test/api.test.mjs', 'answer', 1])], forbiddenSources: [{ root: 'vendor' }] })
  assert.equal(result.evidence.exitCode, 0, readFileSync(join(f.root, 'execution/runner.log'), 'utf8'))
  assert.equal(result.certification, 'unverified')
  assert.ok(result.nativeSourceRoutes.some(row => row.path === 'vendor/original.js'))
  assert.ok(result.errors.some(error => error.includes('forbidden implementation loaded through native Node')))
})
