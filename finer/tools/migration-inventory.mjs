#!/usr/bin/env node
// Read-only workload recovery. This records inputs and gaps; it never builds,
// installs dependencies, executes package scripts, or certifies a library.
import { spawnSync } from 'node:child_process'
import { existsSync, readFileSync, readdirSync, readlinkSync, realpathSync } from 'node:fs'
import { dirname, join, relative, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { expandTestSelection } from './node-test-evidence.mjs'
import { fileIdentity, fingerprint, snapshotInputs, writeReceipt } from './artifact-evidence.mjs'

const repoDefault = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
const skipped = new Set(['.git', 'node_modules', 'dist', 'target', '.cache', '.next', 'coverage'])
const cmp = (a, b) => a < b ? -1 : a > b ? 1 : 0
const unique = values => [...new Set(values)].sort(cmp)
const readJson = path => JSON.parse(readFileSync(path, 'utf8'))

export function identifyOptional(path) {
  try { return { path, present: true, ...fileIdentity(path) } }
  catch (error) { return { path, present: false, error: error.code ?? error.message } }
}

function git(workspace, args) {
  const run = spawnSync('git', ['-C', workspace, ...args], { encoding: 'utf8', timeout: 15000, maxBuffer: 16 << 20, env: { ...process.env, GIT_OPTIONAL_LOCKS: '0' } })
  return run.status === 0 ? run.stdout.trim() : null
}

function listInputs(root) {
  const files = [], links = [], errors = []
  function visit(directory) {
    let entries
    try { entries = readdirSync(directory, { withFileTypes: true }).sort((a, b) => cmp(a.name, b.name)) }
    catch (error) { errors.push({ path: relative(root, directory), error: error.code }); return }
    for (const entry of entries) {
      if (skipped.has(entry.name)) continue
      const path = join(directory, entry.name), name = relative(root, path).split('\\').join('/')
      if (entry.isSymbolicLink()) {
        let target = null
        try { target = realpathSync(path) } catch {}
        links.push({ path: name, link: readlinkSync(path), target })
      } else if (entry.isDirectory()) visit(path)
      else if (entry.isFile()) files.push(name)
    }
  }
  visit(root)
  return { files, links, errors }
}

function isRelevant(path) {
  return /\.lil$|\.toml$|\.out$/.test(path) || /(^|\/)(package(-lock)?\.json|npm-shrinkwrap\.json|pnpm-lock\.yaml|yarn\.lock|source-graph\.lock\.json)$/.test(path)
    || /(^|\/)(scripts|tooling|test|tests|e2e)\//.test(path) && /\.(?:[cm]?[jt]sx?|json|ya?ml|html|txt|out)$/.test(path)
}

function strings(value) {
  if (typeof value === 'string') return [value]
  if (Array.isArray(value)) return value.flatMap(strings)
  if (value && typeof value === 'object') return Object.values(value).flatMap(strings)
  return []
}

export function installedDependency(workspace, name, contentCache = new Map()) {
  for (let directory = resolve(workspace); ; directory = dirname(directory)) {
    const candidate = join(directory, 'node_modules', name, 'package.json')
    if (existsSync(candidate)) {
      try {
        const pkg = readJson(candidate), realPath = realpathSync(candidate), root = dirname(realPath)
        if (!contentCache.has(root)) {
          try {
            const content = snapshotInputs(root, { exclude: ['.git', 'node_modules', '.cache'] })
            contentCache.set(root, { root, sha256: content.sha256, files: content.files.length, bytes: content.files.reduce((sum, file) => sum + file.bytes, 0), excludes: ['.git', 'node_modules', '.cache'] })
          } catch (error) { contentCache.set(root, { root, error: error.message }) }
        }
        return { present: true, path: candidate, realPath, version: pkg.version ?? null, ...fileIdentity(candidate), content: contentCache.get(root) }
      } catch (error) { return { present: false, path: candidate, error: error.message } }
    }
    if (dirname(directory) === directory) return { present: false }
  }
}

function frozenTestInventory(path, row, workspace) {
  const identity = identifyOptional(path)
  if (!identity.present) return identity
  const errors = []
  let inventory
  try { inventory = readJson(path) }
  catch (error) { return { ...identity, valid: false, errors: [`invalid JSON: ${error.message}`] } }
  if (!inventory || typeof inventory !== 'object' || Array.isArray(inventory)) return { ...identity, valid: false, errors: ['case inventory must be an object'] }
  const vitestAdapter = row.tests?.vitestAdapter
  if (inventory.schemaVersion !== 1 || inventory.kind !== (vitestAdapter ? 'required-vitest-case-inventory' : 'required-node-case-inventory') || inventory.workload !== row.id) errors.push('case inventory schema or workload differs')
  if (vitestAdapter) {
    const config = inventory.configuration
    const actual = typeof config?.path === 'string' ? identifyOptional(resolve(workspace, config.path)) : null
    if (config?.path !== vitestAdapter.config || !actual?.present || actual.sha256 !== config?.sha256) errors.push('required Vitest configuration missing or changed')
  }
  const cases = inventory.requiredCases
  if (!Array.isArray(cases) || !cases.length || cases.some(value => typeof value !== 'string' || !value.trim()) || new Set(cases).size !== cases.length) errors.push('required cases must be nonempty unique strings')
  const files = inventory.testFiles
  const expectedPaths = [...(row.tests?.nodeAdapter?.files ?? []), ...(row.tests?.nodeAdapter?.sharedFixture ? ['.lilscript-test-adapter/suite.test.mjs'] : [])]
  if (inventory.entryPatterns) {
    try {
      if (fingerprint(expandTestSelection(workspace, inventory.entryPatterns)) !== fingerprint([...expectedPaths].sort(cmp))) errors.push('original test selection differs from frozen entry files')
    } catch (error) { errors.push(`original test selection invalid: ${error.message}`) }
  }
  if (!Array.isArray(files) || !files.length || files.some(file => !file || typeof file.path !== 'string' || !file.path || !/^[a-f0-9]{64}$/.test(file.sha256 ?? '')) || new Set(files.map(file => file.path)).size !== files.length) errors.push('required test files must be nonempty and unique with identities')
  else {
    if (row.tests?.nodeAdapter && fingerprint(files.map(file => file.path).sort(cmp)) !== fingerprint(expectedPaths.sort(cmp))) errors.push('adapter test file set differs from frozen inventory')
    for (const file of files) {
      const actualPath = file.path === '.lilscript-test-adapter/suite.test.mjs' && row.tests?.nodeAdapter?.sharedFixture
        ? join(repoDefault, 'finer/tools/port-adapters', row.tests.nodeAdapter.sharedFixture) : resolve(workspace, file.path)
      const actual = identifyOptional(actualPath)
      if (!actual.present || actual.sha256 !== file.sha256) errors.push(`required test source missing or changed: ${file.path}`)
    }
  }
  const fixtures = inventory.fixtures ?? []
  if (!Array.isArray(fixtures)) errors.push('required fixtures must be an array')
  for (const fixture of Array.isArray(fixtures) ? fixtures : []) {
    if (!fixture || typeof fixture.path !== 'string' || !fixture.path || !/^[a-f0-9]{64}$/.test(fixture.sha256 ?? '')) { errors.push('required fixture identity is invalid'); continue }
    const actual = identifyOptional(resolve(workspace, fixture.path))
    if (!actual.present || actual.sha256 !== fixture.sha256) errors.push(`required fixture missing or changed: ${fixture.path}`)
  }
  return { ...identity, valid: errors.length === 0, errors, requiredCaseCount: Array.isArray(cases) ? cases.length : 0, requiredCasesSha256: fingerprint(cases ?? null), testFileCount: files?.length ?? 0, additionalRequiredCoverage: inventory.additionalRequiredCoverage ?? [] }
}

function inventoryRow(row, { siblings, manifestDirectory, contentCache }) {
  const candidates = [...new Set([row.workspace, ...(row.workspaceCandidates ?? []), join(siblings, row.id)].filter(Boolean).map(path => resolve(path)))]
    .map(path => ({ path, exists: existsSync(path), head: existsSync(path) ? git(path, ['rev-parse', 'HEAD']) : null }))
  const workspace = candidates.find(candidate => candidate.exists)?.path ?? resolve(row.workspace ?? join(siblings, row.id))
  const result = {
    id: row.id, kind: row.kind, required: row.required === true,
    separateCompetitiveRow: row.separateCompetitiveRow ?? row.kind === 'maintained-library', relatedTo: row.relatedTo ?? null,
    recordedWorkspace: row.workspace ?? null, workspace, candidates,
    selection: workspace === resolve(row.workspace ?? workspace) ? 'recorded-workspace' : 'explicit-recovery-candidate',
    historicalStatus: row.acceptanceStatus ?? row.status ?? null,
    historicalPending: row.pending ?? [], comparisons: row.comparisons ?? [],
    upstream: row.upstream ?? row.upstreamComparison ?? null,
    recordedScope: row.recordedComparisonScope ?? null,
    certification: 'unverified', gaps: [],
  }
  if (!existsSync(workspace)) { result.gaps.push({ code: 'workspace-missing' }); return result }
  if (result.selection !== 'recorded-workspace') result.gaps.push({ code: 'recorded-workspace-missing-recovered-copy-needs-audit' })
  const source = listInputs(workspace)
  result.head = git(workspace, ['rev-parse', 'HEAD'])
  const dirty = git(workspace, ['status', '--porcelain=v1', '--untracked-files=all'])
  result.gitStatus = dirty === null ? null : { sha256: fingerprint(dirty), rows: dirty ? dirty.split('\n').length : 0 }
  const packagePath = join(workspace, 'package.json')
  let pkg = null
  try { pkg = readJson(packagePath) } catch (error) { result.gaps.push({ code: 'package-metadata-missing-or-invalid', error: error.code ?? error.message }) }
  result.package = pkg ? { name: pkg.name ?? null, version: pkg.version ?? null, identity: identifyOptional(packagePath), scripts: pkg.scripts ?? {}, engines: pkg.engines ?? {} } : null
  result.sourceFiles = source.files.filter(path => path.endsWith('.lil'))
  result.configurations = unique([...source.files.filter(path => path.endsWith('.toml')), ...(row.configurations ?? []).map(config => config.path)])
    .map(path => ({ ...identifyOptional(join(workspace, path)), relativePath: path, recordedSha256: row.configurations?.find(config => config.path === path)?.sha256 ?? null }))
  for (const config of result.configurations) if (!config.present) result.gaps.push({ code: 'declared-configuration-missing', path: config.relativePath })
  result.locks = source.files.filter(path => /(^|\/)(?:package-lock\.json|npm-shrinkwrap\.json|pnpm-lock\.yaml|yarn\.lock|source-graph\.lock\.json)$/.test(path))
    .map(path => ({ ...identifyOptional(join(workspace, path)), relativePath: path }))
  result.testFiles = source.files.filter(path => /(^|\/)(?:test|tests|e2e)\//.test(path) && /\.(?:[cm]?[jt]sx?)$/.test(path))
  result.inputLinks = source.links
  result.scanErrors = source.errors
  const relevant = source.files.filter(isRelevant)
  try { result.inputs = snapshotInputs(workspace, { paths: relevant }) }
  catch (error) { result.inputs = null; result.gaps.push({ code: 'input-snapshot-failed', error: error.message }) }
  if (source.links.length) result.gaps.push({ code: 'input-symlinks-require-explicit-root-review', count: source.links.length })
  if (source.errors.length) result.gaps.push({ code: 'input-scan-incomplete', count: source.errors.length })
  const buildPath = join(workspace, 'scripts/build.mjs'), buildText = existsSync(buildPath) ? readFileSync(buildPath, 'utf8') : ''
  const scriptPaths = source.files.filter(path => /^(?:scripts|tooling)\//.test(path) && /\.[cm]?js$/.test(path))
  const observedEnvironment = unique(scriptPaths.flatMap(path => [...readFileSync(join(workspace, path), 'utf8').matchAll(/process\.env\.([A-Z][A-Z0-9_]*(?:COMPILER|LILSCRIPT|_BIN|_ROOT)[A-Z0-9_]*)/g)].map(match => match[1])))
  result.build = {
    recordedCommand: row.build?.recordedCommand ?? null,
    script: identifyOptional(buildPath), packageCommand: pkg?.scripts?.build ?? null,
    scripts: Object.fromEntries(Object.entries(pkg?.scripts ?? {}).filter(([name]) => /^(build|setup|generate|graph|check:.*(?:graph|source))/.test(name))),
    observedCompilerEnvironmentNames: observedEnvironment,
    argumentLiterals: ['--compile', '--force', '--dev', '--prod'].filter(flag => buildText.includes(`'${flag}'`) || buildText.includes(`"${flag}"`)),
    compilerOutputOutsideDistMentioned: /__compiled|join\(source|join\(src/.test(buildText),
    compilerCommandAudited: false,
  }
  if (!buildText) result.gaps.push({ code: 'default-build-script-missing-requires-adapter' })
  const testAdapter = row.tests?.nodeAdapter ?? row.tests?.vitestAdapter
  if (row.tests?.nodeAdapter && row.tests?.vitestAdapter) result.gaps.push({ code: 'ambiguous-production-test-adapter' })
  const inventoryPath = testAdapter?.caseInventory ?? row.tests?.requiredCaseInventory
  result.tests = {
    recordedCommand: row.tests?.recordedCommand ?? null, packageCommand: pkg?.scripts?.test ?? null,
    scripts: Object.fromEntries(Object.entries(pkg?.scripts ?? {}).filter(([name]) => /^(test|check)/.test(name))),
    adapter: testAdapter ?? null,
    frozenInventory: inventoryPath ? frozenTestInventory(resolve(manifestDirectory, inventoryPath), row, workspace) : null,
    sharedFixture: testAdapter?.sharedFixture ? identifyOptional(join(repoDefault, 'finer/tools/port-adapters', testAdapter.sharedFixture)) : null,
    declaredFiles: [...(testAdapter?.files ?? []), ...(row.tests?.vitestAdapter?.config ? [row.tests.vitestAdapter.config] : [])].map(path => identifyOptional(join(workspace, path))),
  }
  if (!testAdapter) result.gaps.push({ code: 'production-test-adapter-missing' })
  if (!result.tests.frozenInventory?.present) result.gaps.push({ code: 'frozen-required-case-inventory-missing' })
  else if (!result.tests.frozenInventory.valid) result.gaps.push({ code: 'frozen-required-case-inventory-invalid', errors: result.tests.frozenInventory.errors })
  for (const file of result.tests.declaredFiles) if (!file.present) result.gaps.push({ code: 'declared-test-file-missing', path: file.path })
  if (result.tests.sharedFixture && !result.tests.sharedFixture.present) result.gaps.push({ code: 'shared-test-fixture-missing', path: result.tests.sharedFixture.path })
  for (const coverage of result.tests.frozenInventory?.additionalRequiredCoverage ?? []) result.gaps.push({ code: 'additional-required-test-coverage-pending', coverage })
  if (result.testFiles.length === 0 && !result.tests.sharedFixture?.present) result.gaps.push({ code: 'test-source-inventory-empty' })
  const exports = strings({ main: pkg?.main, module: pkg?.module, browser: pkg?.browser, exports: pkg?.exports }).filter(path => path.startsWith('./') && /\.[cm]?js$/.test(path))
  const artifacts = unique([...(row.package?.declaredEntryFiles ?? []), ...(row.comparisons ?? []).map(item => item.artifact), ...(testAdapter?.artifacts ?? []), ...exports].map(path => path.replace(/^\.\//, '')))
  result.artifacts = artifacts.map(path => path.includes('*') ? { relativePath: path, pattern: true, present: null } : { ...identifyOptional(join(workspace, path)), relativePath: path })
  if (!artifacts.length) result.gaps.push({ code: 'artifact-entry-inventory-empty' })
  for (const artifact of result.artifacts) if (artifact.present === false) result.gaps.push({ code: 'declared-artifact-missing', path: artifact.relativePath })
  result.dependencies = []
  for (const role of ['dependencies', 'devDependencies', 'peerDependencies', 'optionalDependencies']) {
    for (const [name, requested] of Object.entries(pkg?.[role] ?? {}).sort(([a], [b]) => cmp(a, b))) {
      const installed = installedDependency(workspace, name, contentCache)
      const dependency = { role, name, requested, installed }
      if (/^(?:file|link):/.test(requested)) dependency.localRoot = { path: resolve(workspace, requested.replace(/^(?:file|link):/, '')), present: existsSync(resolve(workspace, requested.replace(/^(?:file|link):/, ''))) }
      result.dependencies.push(dependency)
      if (!installed.present) result.gaps.push({ code: 'dependency-metadata-missing', role, name })
      else if (!installed.content?.sha256) result.gaps.push({ code: 'dependency-content-snapshot-failed', role, name, error: installed.content?.error })
    }
  }
  result.dependencyEvidence = 'declared direct installed package contents and lock identities; nested node_modules and transitive runtime/source graph remain unverified'
  result.sourceGraphReferences = unique(scriptPaths.flatMap(path => [...readFileSync(join(workspace, path), 'utf8').matchAll(/['"](\.\.\/(?:[a-z0-9-]+lil|lil-solidjs))(?:\/[^'"]*)?['"]/g)].map(match => match[1])))
    .map(path => ({ path, resolved: resolve(workspace, path), present: existsSync(resolve(workspace, path)) }))
  for (const reference of result.sourceGraphReferences) if (!reference.present) result.gaps.push({ code: 'referenced-sibling-missing', path: reference.path })
  result.gaps.push({ code: 'baseline-build-and-production-test-replay-pending' }, { code: 'complete-dependency-graph-identity-pending' })
  return result
}

function summarize(inventory) {
  return {
    rows: inventory.length,
    libraryRows: inventory.filter(row => row.kind === 'maintained-library').length,
    additionalRows: inventory.filter(row => row.kind !== 'maintained-library').length,
    missingWorkspaces: inventory.filter(row => row.gaps.some(gap => gap.code === 'workspace-missing')).map(row => row.id),
    rowsWithMissingDependencies: inventory.filter(row => row.gaps.some(gap => gap.code === 'dependency-metadata-missing')).map(row => row.id),
    rowsWithFrozenTestInventory: inventory.filter(row => row.tests?.frozenInventory?.valid).map(row => row.id),
    rowsWithInvalidTestInventory: inventory.filter(row => row.tests?.frozenInventory?.present && !row.tests.frozenInventory.valid).map(row => row.id),
    installedPackageContents: new Set(inventory.flatMap(row => (row.dependencies ?? []).map(dependency => dependency.installed.content?.root).filter(Boolean))).size,
    sourceFiles: inventory.reduce((sum, row) => sum + (row.sourceFiles?.length ?? 0), 0),
  }
}

export function collectMigrationInventory({ manifestPath = join(repoDefault, 'benchmarks/libraries/maintained-workloads.json'), siblings = dirname(repoDefault), generatedAt = new Date().toISOString() } = {}) {
  manifestPath = resolve(manifestPath); siblings = resolve(siblings)
  const manifest = readJson(manifestPath)
  const rows = [...(manifest.libraries ?? []), ...(manifest.additionalCoverage ?? [])]
  const ids = rows.map(row => row.id)
  if (!ids.length || ids.some(id => typeof id !== 'string' || !id.trim())) throw new Error('workload identities must be nonempty strings')
  if (new Set(ids).size !== ids.length) throw new Error('duplicate workload identity')
  const contentCache = new Map()
  const inventory = rows.map(row => inventoryRow(row, { siblings, manifestDirectory: dirname(manifestPath), contentCache }))
  const discovered = readdirSync(siblings, { withFileTypes: true }).filter(entry => entry.isDirectory() && (entry.name.endsWith('lil') || entry.name === 'lil-solidjs'))
    .map(entry => entry.name).filter(name => !ids.includes(name)).sort(cmp)
  const content = { manifest: { ...identifyOptional(manifestPath), objective: manifest.objective }, rows: inventory, unmanifestedSiblingCandidates: discovered }
  return {
    schemaVersion: 1, kind: 'migration-workload-inventory', generatedAt,
    status: 'recovered-input-inventory-not-baseline-certification', ...content,
    contentSha256: fingerprint(content),
    limits: ['No package build/test command executed.', 'Observed script strings do not prove compiler interception or complete dependency discovery.', 'Direct installed package hashes exclude nested node_modules; transitive dependency resolution remains unverified.', 'Failed/missing inputs and historical pending issues remain rows.'],
    summary: summarize(inventory),
  }
}

export function validateMigrationInventory(report, { manifestPath = join(repoDefault, 'benchmarks/libraries/maintained-workloads.json'), verifyCurrentInputs = false } = {}) {
  const errors = [], manifest = readJson(manifestPath)
  const expected = [...(manifest.libraries ?? []), ...(manifest.additionalCoverage ?? [])]
  if (report?.schemaVersion !== 1 || report?.kind !== 'migration-workload-inventory') return ['unsupported inventory schema']
  if (report.status !== 'recovered-input-inventory-not-baseline-certification') errors.push('inventory cannot certify baseline or milestone completion')
  if (!Array.isArray(report.rows)) return [...errors, 'workload rows missing']
  const content = { manifest: report.manifest, rows: report.rows, unmanifestedSiblingCandidates: report.unmanifestedSiblingCandidates }
  if (fingerprint(content) !== report.contentSha256) errors.push('inventory content hash differs')
  if (report.manifest?.sha256 !== fileIdentity(manifestPath).sha256) errors.push('workload manifest changed')
  if (!expected.length || new Set(expected.map(row => row.id)).size !== expected.length) errors.push('workload manifest must be nonempty and unique')
  if (fingerprint(report.rows.map(row => row.id).sort(cmp)) !== fingerprint(expected.map(row => row.id).sort(cmp))) errors.push('required workload identity set differs')
  if (fingerprint(report.summary) !== fingerprint(summarize(report.rows))) errors.push('inventory summary differs from workload rows')
  const contentCache = new Map()
  for (const row of report.rows) {
    const original = expected.find(item => item.id === row.id)
    if (!original) continue
    if (row.required !== (original.required === true) || row.kind !== original.kind || row.separateCompetitiveRow !== (original.separateCompetitiveRow ?? original.kind === 'maintained-library')) errors.push(`workload role differs: ${row.id}`)
    if (row.certification !== 'unverified') errors.push(`inventory cannot certify workload: ${row.id}`)
    if (verifyCurrentInputs) {
      for (const config of row.configurations ?? []) {
        const actual = identifyOptional(config.path)
        if (actual.present !== config.present || actual.sha256 !== config.sha256) errors.push(`configuration changed: ${row.id}/${config.relativePath}`)
      }
      for (const dependency of row.dependencies ?? []) {
        const actual = installedDependency(row.workspace, dependency.name, contentCache)
        if (fingerprint(actual) !== fingerprint(dependency.installed)) errors.push(`installed dependency changed: ${row.id}/${dependency.name}`)
      }
      if (row.inputs) {
        try {
          const files = listInputs(row.workspace).files.filter(isRelevant)
          if (snapshotInputs(row.workspace, { paths: files }).sha256 !== row.inputs.sha256) errors.push(`workload inputs changed: ${row.id}`)
        } catch (error) { errors.push(`workload inputs unavailable: ${row.id}: ${error.message}`) }
      }
      if (row.tests?.frozenInventory?.path) {
        const actual = frozenTestInventory(row.tests.frozenInventory.path, original, row.workspace)
        if (fingerprint(actual) !== fingerprint(row.tests.frozenInventory)) errors.push(`required case inventory changed: ${row.id}`)
      }
    }
  }
  return errors
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const args = process.argv.slice(2), allowed = new Set(['--manifest', '--siblings', '--out', '--date', '--verify'])
  if (args.length % 2 || args.some((arg, index) => index % 2 === 0 && !allowed.has(arg))) throw new Error('usage: migration-inventory.mjs [--manifest FILE] [--siblings DIR] [--out FILE] [--date ISO] [--verify REPORT]')
  const options = Object.fromEntries(Array.from({ length: args.length / 2 }, (_, index) => [args[index * 2], args[index * 2 + 1]]))
  const report = options['--verify'] ? readJson(resolve(options['--verify'])) : collectMigrationInventory({ manifestPath: options['--manifest'], siblings: options['--siblings'], generatedAt: options['--date'] })
  const errors = validateMigrationInventory(report, { manifestPath: options['--manifest'], verifyCurrentInputs: Boolean(options['--verify']) })
  if (errors.length) throw new Error(errors.join('\n'))
  if (options['--out']) {
    if (existsSync(options['--out'])) throw new Error('inventory destination exists; preserve prior evidence with a new path')
    writeReceipt(resolve(options['--out']), report)
  }
  process.stdout.write(`${JSON.stringify(report.summary, null, 2)}\n`)
}
