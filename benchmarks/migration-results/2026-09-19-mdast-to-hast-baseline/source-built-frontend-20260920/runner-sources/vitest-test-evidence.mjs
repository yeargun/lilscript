import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs'
import { dirname, isAbsolute, join, relative, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { digest, fileIdentity, validateTests, writeReceipt } from './artifact-evidence.mjs'
import { runBoundedCommand } from './bounded-command.mjs'

const toolsDirectory = dirname(fileURLToPath(import.meta.url))

export async function runVitestTestEvidence({ cwd, config, vitestPackage, artifactPaths, requiredCases, requiredTestFiles, requiredFixtures = [], requiredConfig, forbiddenSources = [], collectOnly = false, directory, node = process.execPath, timeoutMs = 2700000 }) {
  cwd = resolve(cwd)
  directory = resolve(directory)
  config = resolve(cwd, config)
  vitestPackage = resolve(vitestPackage ?? join(cwd, 'node_modules/vitest'))
  const part = relative(cwd, directory)
  if (part !== '..' && !part.startsWith('../') && !isAbsolute(part)) throw new Error('test evidence must be outside the input workspace')
  if (existsSync(directory)) throw new Error('test evidence directory already exists')
  if (!Array.isArray(artifactPaths) || !artifactPaths.length) throw new Error('production artifacts must be explicit')
  if (!Array.isArray(forbiddenSources) || forbiddenSources.some(rule => !rule || typeof rule.root !== 'string' || !rule.root || rule.except && (!Array.isArray(rule.except) || rule.except.some(path => typeof path !== 'string' || !path)))) throw new Error('invalid forbidden source inventory')
  const requiredArtifacts = artifactPaths.map(path => ({ path: resolve(cwd, path), ...fileIdentity(resolve(cwd, path)) }))
  if (new Set(requiredArtifacts.map(artifact => artifact.path)).size !== requiredArtifacts.length) throw new Error('duplicate production artifact')
  const configuration = { path: config, ...fileIdentity(config) }
  const runner = { path: join(vitestPackage, 'dist/node.js'), ...fileIdentity(join(vitestPackage, 'dist/node.js')) }
  const packageIdentity = { path: join(vitestPackage, 'package.json'), ...fileIdentity(join(vitestPackage, 'package.json')) }
  const version = JSON.parse(readFileSync(packageIdentity.path, 'utf8')).version
  if (!version.startsWith('4.')) throw new Error(`Vitest 4 structured reporter API required; found ${version}`)
  const toolInputs = ['vitest-test-evidence.mjs', 'vitest-evidence-runner.mjs', 'vitest-test-reporter.mjs', 'observe-vitest-sources.mjs', 'observe-node-artifacts.mjs', 'artifact-evidence.mjs', 'bounded-command.mjs'].map(name => ({ path: join(toolsDirectory, name), ...fileIdentity(join(toolsDirectory, name)) }))
  const runtime = { path: node, ...fileIdentity(node) }
  const versionProbe = await runBoundedCommand(node, ['--version'], { encoding: 'utf8', timeoutMs: Math.min(timeoutMs, 10000) })
  runtime.version = versionProbe.stdout.trim()
  runtime.supervision = versionProbe.supervision
  mkdirSync(directory, { recursive: true })
  const loadedDirectory = join(directory, 'loaded'), nativeRoutesDirectory = join(directory, 'native-routes'), preload = join(directory, 'observe.mjs')
  writeFileSync(preload, `import { observeNodeArtifacts } from ${JSON.stringify(pathToFileURL(join(toolsDirectory, 'observe-node-artifacts.mjs')).href)};\nimport { observeVitestSources } from ${JSON.stringify(pathToFileURL(join(toolsDirectory, 'observe-vitest-sources.mjs')).href)};\nobserveNodeArtifacts(${JSON.stringify({ paths: requiredArtifacts.map(artifact => artifact.path), directory: loadedDirectory })});\nobserveVitestSources(${JSON.stringify({ cwd, rules: forbiddenSources, directory: nativeRoutesDirectory })});\n`)
  const settingsPath = join(directory, 'settings.json'), resultsPath = join(directory, 'results.json'), routesPath = join(directory, 'routes.json')
  const settings = { cwd, config, vitestNode: runner.path, artifactPaths: requiredArtifacts.map(artifact => artifact.path), forbiddenSources, results: resultsPath, routes: routesPath, collectOnly }
  writeReceipt(settingsPath, settings)
  const environment = { ...process.env }
  delete environment.NODE_TEST_CONTEXT
  const inheritedNodeOptions = environment.NODE_OPTIONS ?? ''
  environment.NODE_OPTIONS = `${inheritedNodeOptions} --import=${pathToFileURL(preload).href}`.trim()
  const args = [join(toolsDirectory, 'vitest-evidence-runner.mjs'), settingsPath]
  const started = Date.now()
  const result = await runBoundedCommand(node, args, { cwd, env: environment, encoding: 'utf8', timeoutMs, maxBuffer: 1 << 28 })
  const elapsedMs = Date.now() - started
  writeFileSync(join(directory, 'runner.log'), `${result.stdout ?? ''}\n${result.stderr ?? ''}`)
  const results = existsSync(resultsPath) ? JSON.parse(readFileSync(resultsPath, 'utf8')) : null
  const sourceRoutes = existsSync(routesPath) ? JSON.parse(readFileSync(routesPath, 'utf8')) : null
  const nativeSourceRoutes = existsSync(nativeRoutesDirectory) ? readdirSync(nativeRoutesDirectory).filter(name => name.endsWith('.json')).sort().map(name => JSON.parse(readFileSync(join(nativeRoutesDirectory, name), 'utf8'))) : []
  const cases = results?.cases ?? []
  const loadedArtifacts = existsSync(loadedDirectory) ? readdirSync(loadedDirectory).filter(name => name.endsWith('.json')).sort().map(name => JSON.parse(readFileSync(join(loadedDirectory, name), 'utf8'))) : []
  const evidence = { exitCode: result.status, requiredCases: requiredCases ?? [], cases, requiredArtifacts, loadedArtifacts }
  const errors = collectOnly ? [] : validateTests(evidence)
  if (versionProbe.status !== 0 || versionProbe.error || versionProbe.signal) errors.push('Node runtime version probe did not complete')
  if (!sourceRoutes) errors.push('Vitest source route observation missing')
  for (const module of sourceRoutes?.modules ?? []) errors.push(`forbidden implementation entered Vitest transform pipeline: ${module.path}`)
  for (const path of new Set(nativeSourceRoutes.map(row => row.path))) errors.push(`forbidden implementation loaded through native Node: ${path}`)
  if (results?.reason !== (collectOnly ? 'collected' : 'passed') || results?.unhandledErrors?.length || results?.files?.some(file => file.errors.length)) errors.push('Vitest did not report a clean completed run')
  if (!cases.length || new Set(cases.map(row => row.id)).size !== cases.length) errors.push('Vitest case collection missing or ambiguous')
  if (result.error || result.signal) errors.push(`Vitest process did not complete: ${result.error?.message ?? result.signal}`)
  const testFiles = (results?.files ?? []).map(file => {
    const path = resolve(cwd, file.path)
    return { path: file.path, ...(existsSync(path) ? fileIdentity(path) : { missing: true }) }
  })
  if (!Array.isArray(requiredTestFiles) || !requiredTestFiles.length) { if (!collectOnly) errors.push('required test file inventory missing') }
  else {
    const expected = new Map(requiredTestFiles.map(file => [file.path, file.sha256]))
    if (expected.size !== requiredTestFiles.length || expected.size !== testFiles.length) errors.push('required test file inventory differs')
    for (const file of testFiles) if (expected.get(file.path) !== file.sha256) errors.push(`required test source differs: ${file.path}`)
  }
  for (const fixture of requiredFixtures) {
    const path = resolve(cwd, fixture.path)
    if (!existsSync(path) || fileIdentity(path).sha256 !== fixture.sha256) errors.push(`required fixture differs: ${fixture.path}`)
  }
  if (fileIdentity(config).sha256 !== configuration.sha256) errors.push('Vitest configuration changed during execution')
  if ((!requiredConfig && !collectOnly) || requiredConfig && requiredConfig.sha256 !== configuration.sha256) errors.push('required Vitest configuration missing or changed')
  for (const input of [...toolInputs, runner, packageIdentity, runtime]) if (!existsSync(input.path) || fileIdentity(input.path).sha256 !== input.sha256) errors.push(`evidence tool or runtime changed during execution: ${input.path}`)
  const report = {
    schemaVersion: 1, kind: collectOnly ? 'vitest-case-discovery' : 'vitest-production-tests', cwd, args, configuration, testFiles,
    runtime, toolInputs, supervision: result.supervision,
    vitest: { version, runner, package: packageIdentity }, sourceRoutes, nativeSourceRoutes, forbiddenSources,
    observer: { preload, ...fileIdentity(preload), inheritedNodeOptionsSha256: digest(inheritedNodeOptions) },
    executionContract: { productionArtifacts: 'explicit paths externalized to native Node imports and passively observed at the Node load boundary', tests: 'original config and complete unfiltered test selection; no test name/file filters', overrides: { watch: false, update: false, cache: false, fileParallelism: false, maxWorkers: 1 } },
    result: { reason: results?.reason ?? null, unhandledErrors: results?.unhandledErrors ?? [], processError: result.error?.message, signal: result.signal, elapsedMs },
    evidence, errors, certification: collectOnly || errors.length ? 'unverified' : 'verified',
    inventoryCandidate: cases.map(row => row.id),
  }
  writeReceipt(join(directory, 'report.json'), report)
  return report
}
