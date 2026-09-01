#!/usr/bin/env node

import {spawnSync} from 'node:child_process'
import {createHash} from 'node:crypto'
import {
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync
} from 'node:fs'
import {dirname, isAbsolute, join, relative, resolve, sep} from 'node:path'
import {fileURLToPath} from 'node:url'

import {
  assertManifest,
  assertMeasurementReport,
  exportTargets,
  parseTestSummaries,
  sha256,
  stableJson,
  stableValue
} from './contract.mjs'

const here = dirname(fileURLToPath(import.meta.url))
const repository = resolve(here, '../..')
const manifestPath = join(here, 'manifest.json')
const manifestBytes = readFileSync(manifestPath)
const manifest = assertManifest(JSON.parse(manifestBytes))
const executableSuffix = process.platform === 'win32' ? '.exe' : ''

function usage() {
  return `Usage:
  node comparison/markdown-stack/run.mjs [--check]
  node comparison/markdown-stack/run.mjs --clone-upstreams [options]
  node comparison/markdown-stack/run.mjs --check-inputs [options]
  node comparison/markdown-stack/run.mjs --measure [options]
  node comparison/markdown-stack/run.mjs --run-tests [options]

Modes:
  --check             Validate the checked-in contract only (default)
  --clone-upstreams   Clone every selected upstream at its pinned tag
  --check-inputs      Verify upstream Git pins, port packages, exports, and source mappings
  --measure           Check inputs, then measure declared existing port artifacts
  --run-tests         Check inputs, then run each port's declared npm test command

Options:
  --only IDS          Comma-separated port ids
  --upstream-root P   Root containing one upstream clone per port id
  --codec PATH        lilscript-codec binary (default: target/release/lilscript-codec)
  --work-dir PATH     Generated graph/minifier lanes (default: markdown-stack/.work)
  --json PATH         Write the full JSON report
  --markdown PATH     Write a Markdown measurement report (only with --measure)
  --help              Show this text
`
}

function parseArguments(argv) {
  const options = {
    mode: 'check',
    only: manifest.ports.map((port) => port.id),
    upstreamRoot: resolve(repository, manifest.defaultUpstreamRoot),
    codec: resolve(repository, manifest.codec.defaultPath),
    workDir: join(here, '.work'),
    json: null,
    markdown: null
  }
  let explicitMode = false
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index]
    if (['--check', '--clone-upstreams', '--check-inputs', '--measure', '--run-tests'].includes(argument)) {
      if (explicitMode) throw new Error('select exactly one mode')
      options.mode = argument.slice(2)
      explicitMode = true
    } else if (argument === '--only') {
      options.only = (argv[++index] ?? '').split(',').filter(Boolean)
    } else if (argument === '--upstream-root') {
      options.upstreamRoot = resolve(argv[++index] ?? '')
    } else if (argument === '--codec') {
      options.codec = resolve(argv[++index] ?? '')
    } else if (argument === '--work-dir') {
      options.workDir = resolve(argv[++index] ?? '')
    } else if (argument === '--json') {
      options.json = resolve(argv[++index] ?? '')
    } else if (argument === '--markdown') {
      options.markdown = resolve(argv[++index] ?? '')
    } else if (argument === '--help' || argument === '-h') {
      process.stdout.write(usage())
      process.exit(0)
    } else {
      throw new Error(`unknown argument ${argument}`)
    }
  }
  const known = new Set(manifest.ports.map((port) => port.id))
  if (options.only.length === 0 || options.only.some((id) => !known.has(id))) {
    throw new Error('--only must name one or more manifest port ids')
  }
  if (options.markdown && options.mode !== 'measure') {
    throw new Error('--markdown is only valid with --measure')
  }
  return options
}

function command(program, args, {cwd = repository, inherit = false} = {}) {
  const result = spawnSync(program, args, {
    cwd,
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
    stdio: inherit ? 'inherit' : 'pipe',
    windowsHide: true
  })
  if (result.error) throw result.error
  if (result.status !== 0) {
    throw new Error(
      `${program} ${args.join(' ')} exited ${result.status}${result.signal ? ` (${result.signal})` : ''}:\n` +
        `${result.stdout ?? ''}${result.stderr ?? ''}`.trim()
    )
  }
  return result.stdout ?? ''
}

function readJson(path, label) {
  try {
    return JSON.parse(readFileSync(path, 'utf8'))
  } catch (error) {
    throw new Error(`${label} is not valid JSON at ${path}: ${error.message}`)
  }
}

function fileDigest(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex')
}

function safePath(root, value, label) {
  if (isAbsolute(value)) throw new Error(`${label} must be relative`)
  const result = resolve(root, value)
  const back = relative(root, result)
  if (back.startsWith(`..${sep}`) || back === '..' || isAbsolute(back)) {
    throw new Error(`${label} escapes ${root}`)
  }
  return result
}

function normalizeRepository(value) {
  return value.replace(/^git\+/, '').replace(/\.git$/u, '').replace(/\/$/u, '').toLowerCase()
}

function patternRegex(pattern) {
  let expression = '^'
  for (let index = 0; index < pattern.length; index += 1) {
    const character = pattern[index]
    if (character === '*' && pattern[index + 1] === '*') {
      if (pattern[index + 2] === '/') {
        expression += '(?:.*/)?'
        index += 2
      } else {
        expression += '.*'
        index += 1
      }
    } else if (character === '*') {
      expression += '[^/]*'
    } else {
      expression += character.replace(/[|\\{}()[\]^$+?.]/u, '\\$&')
    }
  }
  return new RegExp(`${expression}$`, 'u')
}

function matches(pattern, value) {
  return patternRegex(pattern).test(value)
}

function resolvePortRoot(port) {
  const configured = process.env[port.port.repositoryEnv]
  return configured ? resolve(configured) : resolve(repository, port.port.defaultSibling)
}

function resolveUpstreamRoot(port, options) {
  return join(options.upstreamRoot, port.id)
}

function assertRegularFile(path, label) {
  if (!existsSync(path) || !lstatSync(path).isFile()) {
    throw new Error(`${label} is missing or not a regular file: ${path}`)
  }
}

function gitValue(root, args, label) {
  try {
    return command('git', ['-C', root, ...args]).trim()
  } catch (error) {
    throw new Error(`${label}: ${error.message}`)
  }
}

function verifyUpstreamGit(root, port) {
  if (!existsSync(root)) {
    throw new Error(`${port.id}: upstream clone is missing at ${root}`)
  }
  const commit = gitValue(root, ['rev-parse', 'HEAD^{commit}'], `${port.id} upstream commit`)
  const tree = gitValue(root, ['rev-parse', 'HEAD^{tree}'], `${port.id} upstream tree`)
  const tagCommit = gitValue(
    root,
    ['rev-parse', `refs/tags/${port.upstream.tag}^{commit}`],
    `${port.id} upstream tag`
  )
  const remote = gitValue(root, ['remote', 'get-url', 'origin'], `${port.id} upstream remote`)
  const dirty = gitValue(root, ['status', '--porcelain'], `${port.id} upstream status`)
  if (commit !== port.upstream.commit || tagCommit !== port.upstream.commit) {
    throw new Error(
      `${port.id}: HEAD/tag resolve to ${commit}/${tagCommit}, expected ${port.upstream.commit}`
    )
  }
  if (tree !== port.upstream.tree) {
    throw new Error(`${port.id}: tree ${tree} does not match ${port.upstream.tree}`)
  }
  if (normalizeRepository(remote) !== normalizeRepository(port.upstream.repository)) {
    throw new Error(`${port.id}: origin ${remote} does not match ${port.upstream.repository}`)
  }
  if (dirty !== '') throw new Error(`${port.id}: upstream clone must be clean`)
  return {commit, tree, tag: port.upstream.tag, repository: port.upstream.repository}
}

function concreteTargetPath(packageRoot, target) {
  if (target.startsWith('/') || target === '.' || target === '..') return null
  const value = target.startsWith('./') ? target.slice(2) : target
  if (!value.includes('*')) return safePath(packageRoot, value, `package target ${target}`)
  const prefix = value.slice(0, value.indexOf('*')).replace(/\/$/u, '')
  return safePath(packageRoot, prefix || '.', `package target ${target}`)
}

function verifyPackageTargets(packageRoot, packageJson, exceptions, label) {
  const used = new Set()
  const targets = exportTargets(packageJson)
  if (targets.length === 0) throw new Error(`${label}: package declares no export targets`)
  for (const {key, target} of targets) {
    const path = concreteTargetPath(packageRoot, target)
    if (path && existsSync(path)) continue
    const matching = exceptions.filter((item) => matches(item.pattern, target.replace(/^\.\//u, '')))
    if (matching.length !== 1) {
      throw new Error(`${label}: ${key} target ${target} is missing and has ${matching.length} exceptions`)
    }
    used.add(matching[0].pattern)
  }
  for (const exception of exceptions) {
    if (!used.has(exception.pattern)) {
      throw new Error(`${label}: generated export exception ${exception.pattern} is stale`)
    }
  }
  return targets
}

function verifyPackage(root, expected, {upstream = false} = {}) {
  const packagePath = upstream ? expected.upstream.packagePath : 'package.json'
  const packageRoot = upstream ? safePath(root, packagePath, `${expected.id} package path`) : root
  const packageJsonPath = join(packageRoot, 'package.json')
  assertRegularFile(packageJsonPath, `${expected.id} package.json`)
  const packageJson = readJson(packageJsonPath, `${expected.id} package.json`)
  const contract = upstream ? expected.upstream : expected.port
  if (packageJson.name !== contract.packageName || packageJson.version !== contract.packageVersion) {
    throw new Error(
      `${expected.id}: package is ${packageJson.name}@${packageJson.version}, expected ` +
        `${contract.packageName}@${contract.packageVersion}`
    )
  }
  if (!upstream && JSON.stringify(stableValue(packageJson.exports)) !== JSON.stringify(stableValue(contract.exports))) {
    throw new Error(`${expected.id}: port package exports differ from the manifest`)
  }
  const targets = verifyPackageTargets(
    packageRoot,
    packageJson,
    upstream ? contract.generatedExportTargets : [],
    `${expected.id} ${upstream ? 'upstream' : 'port'}`
  )
  if (
    !upstream &&
    JSON.stringify(stableValue(packageJson.scripts)) !== JSON.stringify(stableValue(contract.scripts))
  ) {
    throw new Error(`${expected.id}: package scripts differ from the manifest`)
  }
  let packageLockSha256 = null
  if (!upstream) {
    const lockPath = join(root, 'package-lock.json')
    assertRegularFile(lockPath, `${expected.id} package lock`)
    packageLockSha256 = fileDigest(lockPath)
  }
  return {
    packageRoot,
    packageJson,
    packageJsonSha256: fileDigest(packageJsonPath),
    packageLockSha256,
    targets
  }
}

function runtimeFiles(path) {
  const stat = lstatSync(path)
  if (stat.isFile()) {
    const name = path.replaceAll('\\', '/')
    return /\.(?:cjs|js|jsx|lil|mjs|ts|tsx)$/u.test(name) && !/\.d\.ts$/u.test(name) ? [path] : []
  }
  if (!stat.isDirectory()) throw new Error(`runtime audit root is not a file or directory: ${path}`)
  const files = []
  for (const entry of readdirSync(path, {withFileTypes: true}).sort((a, b) => a.name.localeCompare(b.name))) {
    const child = join(path, entry.name)
    if (entry.isDirectory()) files.push(...runtimeFiles(child))
    else if (
      entry.isFile() &&
      /\.(?:cjs|js|jsx|lil|mjs|ts|tsx)$/u.test(entry.name) &&
      !/\.d\.ts$/u.test(entry.name)
    ) files.push(child)
  }
  return files
}

function mappedLilPath(source, sourceRoot, portRoot) {
  if (lstatSync(sourceRoot).isFile()) return portRoot
  const inner = relative(sourceRoot, source).replaceAll('\\', '/')
  return join(portRoot, inner.replace(/\.(?:js|ts|tsx)$/u, '.lil'))
}

function auditSources(upstreamPackageRoot, portRoot, port) {
  const modules = []
  const seen = new Set()
  const usedRules = new Set()
  for (const scope of port.sourceAudit.scopes) {
    const sourceRoot = safePath(upstreamPackageRoot, scope.upstream, `${port.id} audit source`)
    if (!existsSync(sourceRoot)) throw new Error(`${port.id}: audit source is missing: ${scope.upstream}`)
    const targetRoot = safePath(portRoot, scope.port, `${port.id} audit target`)
    for (const source of runtimeFiles(sourceRoot)) {
      const upstreamPath = relative(upstreamPackageRoot, source).replaceAll('\\', '/')
      if (seen.has(upstreamPath)) throw new Error(`${port.id}: audit scopes overlap at ${upstreamPath}`)
      seen.add(upstreamPath)
      const target = mappedLilPath(source, sourceRoot, targetRoot)
      const targetPath = relative(portRoot, target).replaceAll('\\', '/')
      const matchingRules = port.sourceAudit.statuses.filter((rule) => matches(rule.pattern, upstreamPath))
      if (existsSync(target) && lstatSync(target).isFile()) {
        if (matchingRules.length !== 0) {
          throw new Error(`${port.id}: ${upstreamPath} is mapped but also has a status rule`)
        }
        modules.push({
          upstreamPath,
          status: 'mapped',
          target: targetPath,
          upstreamSha256: fileDigest(source),
          targetSha256: fileDigest(target),
          reason: 'same-relative .lil module exists'
        })
        continue
      }
      if (matchingRules.length !== 1) {
        throw new Error(`${port.id}: ${upstreamPath} has no mapping and ${matchingRules.length} status rules`)
      }
      const rule = matchingRules[0]
      usedRules.add(rule.pattern)
      if (rule.target !== null) {
        assertRegularFile(safePath(portRoot, rule.target, `${port.id} status target`), `${port.id} status target`)
      }
      modules.push({
        upstreamPath,
        status: rule.status,
        target: rule.target,
        upstreamSha256: fileDigest(source),
        targetSha256: rule.target === null ? null : fileDigest(safePath(portRoot, rule.target, `${port.id} status target`)),
        reason: rule.reason
      })
    }
  }
  if (modules.length === 0) throw new Error(`${port.id}: source audit found no runtime modules`)
  for (const rule of port.sourceAudit.statuses) {
    if (!usedRules.has(rule.pattern)) throw new Error(`${port.id}: status rule ${rule.pattern} is stale`)
  }
  return modules.sort((left, right) => left.upstreamPath.localeCompare(right.upstreamPath))
}

function inventoryPortSources(portRoot, port) {
  const files = []
  const seen = new Set()
  for (const configuredRoot of port.sourceAudit.portRoots) {
    const root = safePath(portRoot, configuredRoot, `${port.id} source inventory root`)
    if (!existsSync(root)) throw new Error(`${port.id}: source inventory root is missing: ${configuredRoot}`)
    for (const path of runtimeFiles(root)) {
      const sourcePath = relative(portRoot, path).replaceAll('\\', '/')
      if (seen.has(sourcePath)) throw new Error(`${port.id}: source inventory roots overlap at ${sourcePath}`)
      seen.add(sourcePath)
      files.push({path: sourcePath, sha256: fileDigest(path)})
    }
  }
  if (files.length === 0) throw new Error(`${port.id}: source inventory found no runtime files`)
  return files.sort((left, right) => left.path.localeCompare(right.path))
}

function portGitRecord(root) {
  const head = gitValue(root, ['rev-parse', 'HEAD^{commit}'], `${root} port commit`)
  const tree = gitValue(root, ['rev-parse', 'HEAD^{tree}'], `${root} port tree`)
  const lines = gitValue(root, ['status', '--porcelain'], `${root} port status`)
    .split('\n')
    .filter(Boolean)
  return {head, tree, dirty: lines.length !== 0, changedPaths: lines.length}
}

function verifyInputs(port, options) {
  const upstreamRoot = resolveUpstreamRoot(port, options)
  const portRoot = resolvePortRoot(port)
  if (!existsSync(portRoot)) throw new Error(`${port.id}: sibling port is missing at ${portRoot}`)
  const upstreamGit = verifyUpstreamGit(upstreamRoot, port)
  const upstreamPackage = verifyPackage(upstreamRoot, port, {upstream: true})
  const portPackage = verifyPackage(portRoot, port)
  const evidencePath = safePath(portRoot, port.port.evidence.path, `${port.id} evidence`)
  assertRegularFile(evidencePath, `${port.id} evidence`)
  const evidence = readJson(evidencePath, `${port.id} evidence`)
  const expectedPin = `${port.upstream.packageName}@${port.upstream.packageVersion}`
  if (
    evidence.pin !== expectedPin ||
    evidence.package !== port.port.packageName ||
    evidence.spec?.pass !== port.port.evidence.expectedPassed ||
    evidence.spec?.total !== port.port.evidence.expectedPassed + port.port.evidence.expectedFailed
  ) {
    throw new Error(`${port.id}: ${port.port.evidence.path} differs from the manifest evidence contract`)
  }
  if (!Array.isArray(evidence.size)) throw new Error(`${port.id}: evidence has no size rows`)
  const sizeIds = new Set()
  for (const row of evidence.size) {
    if (typeof row.id !== 'string' || sizeIds.has(row.id)) {
      throw new Error(`${port.id}: evidence size ids must be unique strings`)
    }
    sizeIds.add(row.id)
    for (const metric of ['raw', 'gzip9', 'brotli11']) {
      if (!Number.isSafeInteger(row[metric]) || row[metric] < 0) {
        throw new Error(`${port.id}: evidence ${row.id}.${metric} is invalid`)
      }
    }
  }
  const requiredSizeIds = [
    port.measurement.historical.officialGraph,
    port.measurement.historical.officialTerser,
    port.measurement.historical.lil
  ]
  for (const id of requiredSizeIds) {
    if (!sizeIds.has(id)) throw new Error(`${port.id}: evidence size row ${id} is missing`)
  }
  const modules = auditSources(upstreamPackage.packageRoot, portRoot, port)
  const sourceFiles = inventoryPortSources(portRoot, port)
  return {
    id: port.id,
    upstream: {
      ...upstreamGit,
      package: `${upstreamPackage.packageJson.name}@${upstreamPackage.packageJson.version}`,
      packageJsonSha256: upstreamPackage.packageJsonSha256
    },
    port: {
      path: port.port.defaultSibling,
      package: `${portPackage.packageJson.name}@${portPackage.packageJson.version}`,
      packageJsonSha256: portPackage.packageJsonSha256,
      packageLockSha256: portPackage.packageLockSha256,
      evidence: {
        path: port.port.evidence.path,
        sha256: fileDigest(evidencePath),
        spec: {
          passed: evidence.spec.pass,
          failed: evidence.spec.total - evidence.spec.pass,
          total: evidence.spec.total,
          label: evidence.spec.label
        },
        comparison: evidence.comparison,
        sizes: evidence.size
      },
      git: portGitRecord(portRoot)
    },
    sourceAudit: {
      total: modules.length,
      mapped: modules.filter((module) => module.status === 'mapped').length,
      statuses: Object.fromEntries(
        [...new Set(modules.map((module) => module.status))]
          .sort()
          .map((status) => [status, modules.filter((module) => module.status === status).length])
      ),
      modules,
      portInventory: {total: sourceFiles.length, files: sourceFiles}
    }
  }
}

function verifyCodec(codec, paths) {
  assertRegularFile(codec, 'lilscript-codec')
  const output = command(codec, ['--json', ...paths])
  const report = JSON.parse(output)
  if (report.schemaVersion !== manifest.codec.schemaVersion || report.artifacts?.length !== paths.length) {
    throw new Error('lilscript-codec returned an unsupported or incomplete report')
  }
  if (
    JSON.stringify(stableValue(report.codecs.gzip9)) !== JSON.stringify(stableValue(manifest.codec.gzip9)) ||
    JSON.stringify(stableValue(report.codecs.brotli11)) !== JSON.stringify(stableValue(manifest.codec.brotli11))
  ) {
    throw new Error('lilscript-codec provenance differs from the manifest')
  }
  return report
}

async function loadToolchain() {
  const packagePath = join(here, 'package.json')
  const lockPath = safePath(here, manifest.toolchain.packageLock, 'toolchain lockfile')
  const packageJson = readJson(packagePath, 'harness package.json')
  const packageLock = readJson(lockPath, 'harness package lock')
  const expected = {
    esbuild: manifest.toolchain.esbuild.version,
    terser: manifest.toolchain.terser.version,
    ...manifest.toolchain.graph.officialPackages,
    [manifest.toolchain.reactGraph.officialPackage]: manifest.toolchain.reactGraph.officialVersion,
    ...manifest.toolchain.reactGraph.pinnedDependencies
  }
  for (const [name, version] of Object.entries(expected)) {
    if (packageJson.dependencies?.[name] !== version || packageLock.packages?.['']?.dependencies?.[name] !== version) {
      throw new Error(`harness dependency ${name}@${version} is not pinned in package.json and package-lock.json`)
    }
    const installed = readJson(join(here, 'node_modules', name, 'package.json'), `installed ${name}`)
    if (installed.version !== version) {
      throw new Error(`installed ${name}@${installed.version} does not match ${version}; run npm ci`)
    }
  }
  const [{build}, {minify}] = await Promise.all([import('esbuild'), import('terser')])
  return {
    build,
    minify,
    packageLockSha256: fileDigest(lockPath)
  }
}

function evidenceSize(input, id) {
  const row = input.port.evidence.sizes.find((item) => item.id === id)
  if (!row) throw new Error(`${input.id}: evidence size row ${id} is missing`)
  return row
}

function addLocation(locations, port, path, artifact, role) {
  assertRegularFile(path, `${port.id} ${role}`)
  locations.push({port, path, artifact, role, sha256: fileDigest(path)})
}

function generatedPath(options, port, name) {
  const directory = join(options.workDir, port.id)
  mkdirSync(directory, {recursive: true})
  return join(directory, name)
}

async function minifyFile(toolchain, sourcePath, outputPath) {
  const result = await toolchain.minify(readFileSync(sourcePath, 'utf8'), manifest.toolchain.terser.options)
  if (typeof result.code !== 'string') throw new Error(`Terser produced no code for ${sourcePath}`)
  writeFileSync(outputPath, result.code)
}

function graphAliases(port) {
  if (port.id !== 'react-markdown') return {}
  const graph = manifest.toolchain.reactGraph
  const aliases = {}
  for (const [name, configured] of Object.entries(graph.portAliases)) {
    const dependency = manifest.ports.find((item) => item.id === configured.portId)
    if (!dependency) throw new Error(`react-markdown alias ${name} names an unknown port`)
    const root = resolvePortRoot(dependency)
    const checked = verifyPackage(root, dependency)
    if (name !== checked.packageJson.name && !name.startsWith(`${checked.packageJson.name}/`)) {
      throw new Error(`react-markdown alias ${name} has the wrong package name`)
    }
    aliases[name] = safePath(root, configured.path, `react-markdown alias ${name}`)
    assertRegularFile(aliases[name], `react-markdown alias ${name}`)
  }
  return aliases
}

function graphInputs(metafile, workingDirectory) {
  return Object.entries(metafile.inputs)
    .map(([input, details]) => {
      const path = resolve(workingDirectory, input)
      assertRegularFile(path, `graph input ${input}`)
      return {
        path: relative(repository, path).replaceAll('\\', '/'),
        sha256: fileDigest(path),
        bytes: details.bytes
      }
    })
    .sort((left, right) => left.path.localeCompare(right.path))
}

function graphExports(metafile) {
  const outputs = Object.values(metafile.outputs)
  if (outputs.length !== 1 || !Array.isArray(outputs[0].exports) || outputs[0].exports.length === 0) {
    throw new Error('graph build did not preserve public exports')
  }
  return [...outputs[0].exports].sort()
}

async function buildGraphs(port, options, toolchain) {
  const officialEntry = fileURLToPath(import.meta.resolve(port.measurement.officialEntry))
  const portRoot = resolvePortRoot(port)
  const lilEntry = safePath(portRoot, port.measurement.lilEntry, `${port.id} Lil public entry`)
  assertRegularFile(officialEntry, `${port.id} official public entry`)
  assertRegularFile(lilEntry, `${port.id} Lil public entry`)

  const officialGraph = generatedPath(options, port, 'official-graph.js')
  const lilGraph = generatedPath(options, port, 'lil-graph.js')
  const platform = manifest.toolchain.graph.browserPorts.includes(port.id)
    ? 'browser'
    : manifest.toolchain.esbuild.options.platform
  const buildOptions = {
    ...manifest.toolchain.esbuild.options,
    absWorkingDir: here,
    platform,
    external: port.measurement.externals,
    nodePaths: [join(here, 'node_modules')],
    logLevel: 'silent',
    metafile: true
  }
  const officialBuild = await toolchain.build({
    ...buildOptions,
    entryPoints: [port.measurement.officialEntry],
    outfile: officialGraph
  })
  const lilBundled = manifest.toolchain.graph.lilBundlePorts.includes(port.id)
  // A port whose Lil lane is not bundled is measured as the file the compiler wrote,
  // which is already minified. Bundling reformats it: esbuild re-prints every token
  // with whitespace and renames colliding identifiers, so `unified`'s 14580-byte
  // compiler output arrives here as 20869 bytes across 588 lines -- and is then
  // compared against an official graph that Terser has minified. Restoring whitespace
  // and identifiers puts the bundled ports back at the density the compiler emitted,
  // and matches how the other thirteen are measured. `minifySyntax` stays off: that
  // would be optimisation the compiler did not do, and the Lil lane must not borrow it.
  const lilBuild = await toolchain.build({
    ...buildOptions,
    entryPoints: [lilEntry],
    outfile: lilGraph,
    alias: graphAliases(port),
    write: lilBundled,
    ...(lilBundled
      ? {minifyWhitespace: true, minifyIdentifiers: true, minifySyntax: false}
      : {})
  })
  if (!lilBundled) copyFileSync(lilEntry, lilGraph)

  const officialTerser = generatedPath(options, port, 'official-terser.js')
  await minifyFile(toolchain, officialGraph, officialTerser)
  let disputedBrowserGraph = null
  let disputedBrowserTerser = null
  if (port.id === 'micromark') {
    disputedBrowserGraph = generatedPath(options, port, 'diagnostic-browser-graph.js')
    await toolchain.build({
      ...buildOptions,
      platform: 'browser',
      entryPoints: [port.measurement.officialEntry],
      outfile: disputedBrowserGraph,
      metafile: false
    })
    disputedBrowserTerser = generatedPath(options, port, 'diagnostic-browser-terser.js')
    await minifyFile(toolchain, disputedBrowserGraph, disputedBrowserTerser)
  }
  return {
    officialEntry,
    officialGraph,
    officialTerser,
    lilEntry,
    lilGraph,
    officialInputs: graphInputs(officialBuild.metafile, here),
    lilInputs: lilBundled
      ? graphInputs(lilBuild.metafile, here)
      : [{
          path: relative(repository, lilEntry).replaceAll('\\', '/'),
          sha256: fileDigest(lilEntry),
          bytes: lstatSync(lilEntry).size
        }],
    officialExports: graphExports(officialBuild.metafile),
    lilExports: graphExports(lilBuild.metafile),
    lilMode: lilBundled ? 'bundle' : 'artifact',
    disputedBrowserGraph,
    disputedBrowserTerser
  }
}

async function measurementReport(selected, inputReports, options) {
  const toolchain = await loadToolchain()
  const locations = []
  const generated = new Map()
  for (const port of selected) {
    const root = resolvePortRoot(port)
    const paths = await buildGraphs(port, options, toolchain)
    generated.set(port.id, paths)
    addLocation(locations, port, paths.officialGraph, '.generated/official-graph.js', 'official-graph')
    addLocation(locations, port, paths.officialTerser, '.generated/official-terser.js', 'official-terser')
    addLocation(locations, port, paths.lilGraph, '.generated/lil-graph.js', 'lil-graph')
    if (paths.disputedBrowserGraph !== null) {
      addLocation(
        locations,
        port,
        paths.disputedBrowserGraph,
        '.generated/diagnostic-browser-graph.js',
        'diagnostic-disputed-graph'
      )
      addLocation(
        locations,
        port,
        paths.disputedBrowserTerser,
        '.generated/diagnostic-browser-terser.js',
        'diagnostic-disputed-terser'
      )
    }
    if (port.port.officialArtifact !== null) {
      addLocation(
        locations,
        port,
        safePath(root, port.port.officialArtifact, `${port.id} historical official artifact`),
        port.port.officialArtifact,
        'diagnostic-official'
      )
    }
    for (const artifact of port.port.artifacts) {
      addLocation(locations, port, safePath(root, artifact, `${port.id} artifact`), artifact, 'diagnostic-port')
    }
  }

  const measured = verifyCodec(options.codec, locations.map((location) => location.path))
  const measuredLocations = locations.map((location, index) => {
    const measurement = measured.artifacts[index]
    const bytes = readFileSync(location.path)
    const digest = sha256(bytes)
    if (measurement.path !== location.path || measurement.raw !== bytes.length || digest !== location.sha256) {
      throw new Error(`${location.port.id}: codec did not measure ${location.artifact} exactly`)
    }
    return {
      port: location.port,
      artifact: {
        path: location.artifact,
        role: location.role,
        sha256: digest,
        raw: measurement.raw,
        gzip9: measurement.gzip9,
        brotli11: measurement.brotli11
      }
    }
  })

  const byPort = []
  for (const port of selected) {
    const input = inputReports.find((item) => item.id === port.id)
    const artifacts = measuredLocations
      .filter((item) => item.port.id === port.id)
      .map((item) => item.artifact)
    const officialGraph = artifacts.find((item) => item.role === 'official-graph')
    const officialTerser = artifacts.find((item) => item.role === 'official-terser')
    const lil = artifacts.find((item) => item.role === 'lil-graph')
    if (!officialGraph || !officialTerser || !lil) throw new Error(`${port.id}: canonical graph artifacts are incomplete`)
    const historicalOfficial = artifacts.find((item) => item.role === 'diagnostic-official')
    const historicalLil = artifacts.find(
      (item) => item.role === 'diagnostic-port' && item.path === port.measurement.lilEntry
    )
    const terserWasPinned = input.port.evidence.comparison.includes(`Terser ${manifest.toolchain.terser.version}`)
    const historicalChecks = [
      historicalCheck(
        'official-graph',
        evidenceSize(input, port.measurement.historical.officialGraph),
        officialGraph,
        historicalOfficial
          ? `The historical row is the retained ${port.port.officialArtifact} artifact (SHA-256 ${historicalOfficial.sha256}); the canonical graph is freshly resolved from the harness lock, so a difference identifies graph-input drift.`
          : 'No historical graph artifact or dependency lock was retained; the canonical graph is freshly resolved from the harness lock.'
      ),
      historicalCheck(
        'official-terser',
        evidenceSize(input, port.measurement.historical.officialTerser),
        officialTerser,
        port.id === 'rehype'
          ? 'The historical raw and Brotli values reproduce by minifying the retained historical graph with module=false; the canonical lane uses a fresh graph and the required module=true.'
          : terserWasPinned
            ? 'The Terser version/options match, but the historical row used its prior graph without a retained artifact hash; the canonical row minifies the freshly locked graph.'
            : 'The historical row did not pin its Terser version and full options; the canonical row uses Terser 5.51.2 with module=true, compress=true, and mangle=true.'
      ),
      historicalCheck(
        'lil-graph',
        evidenceSize(input, port.measurement.historical.lil),
        lil,
        manifest.toolchain.graph.lilBundlePorts.includes(port.id)
          ? 'The historical row used an earlier full-graph build without an artifact hash; the canonical graph is freshly bundled from the current standard ESM and locked runtime dependencies.'
          : `The historical row no longer matches the current standard ESM artifact (SHA-256 ${historicalLil.sha256}); the canonical lane copies that artifact byte-for-byte without post-minification.`
      )
    ]
    const paths = generated.get(port.id)
    const delta = lil.brotli11 - officialTerser.brotli11
    byPort.push({
      id: port.id,
      input,
      lane: {
        retention: manifest.toolchain.graph.retention,
        platform: manifest.toolchain.graph.browserPorts.includes(port.id)
          ? 'browser'
          : manifest.toolchain.esbuild.options.platform,
        externals: port.measurement.externals,
        officialEntry: {
          specifier: port.measurement.officialEntry,
          resolvedPath: relative(here, paths.officialEntry).replaceAll('\\', '/'),
          sha256: fileDigest(paths.officialEntry),
          exports: paths.officialExports
        },
        lilEntry: {
          path: port.measurement.lilEntry,
          sha256: fileDigest(paths.lilEntry),
          exports: paths.lilExports
        },
        officialInputs: paths.officialInputs,
        lilInputs: paths.lilInputs,
        lilGraphMode: paths.lilMode,
        lilPostMinified: false
      },
      artifacts,
      historicalChecks,
      comparison: {
        qualification: {
          kind: 'all-public-root-exports-retained',
          sitePassed: input.port.evidence.spec.passed,
          contractedFullPassed: port.test.summaries.reduce((sum, item) => sum + item.expectedPassed, 0),
          closedArtifactsEligible: false
        },
        lil,
        official: officialTerser,
        brotliDelta: delta,
        result: delta < 0 ? 'win' : delta > 0 ? 'loss' : 'tie'
      }
    })
  }
  const summary = {
    lil: metricTotals(byPort.map((port) => port.comparison.lil)),
    official: metricTotals(byPort.map((port) => port.comparison.official)),
    wins: byPort.filter((port) => port.comparison.result === 'win').length,
    losses: byPort.filter((port) => port.comparison.result === 'loss').length,
    ties: byPort.filter((port) => port.comparison.result === 'tie').length
  }
  summary.delta = Object.fromEntries(
    ['raw', 'gzip9', 'brotli11'].map((metric) => [metric, summary.lil[metric] - summary.official[metric]])
  )
  return assertMeasurementReport(
    {
      schemaVersion: 3,
      format: 'lilscript-markdown-stack-measurements',
      generatedAt: new Date().toISOString(),
      manifestSha256: sha256(manifestBytes),
      commands: {
        contractTests: 'npm --prefix comparison/markdown-stack test',
        checkInputs: 'node comparison/markdown-stack/run.mjs --check-inputs --json comparison/markdown-stack/.work/input-audit.json',
        measure: `node comparison/markdown-stack/run.mjs ${process.argv.slice(2).join(' ')}`,
        fullTests: 'node comparison/markdown-stack/run.mjs --run-tests --json comparison/markdown-stack/.work/tests.json'
      },
      harness: Object.fromEntries(
        ['run.mjs', 'contract.mjs', 'contract.test.mjs', 'package.json', manifest.toolchain.packageLock]
          .map((path) => [path, fileDigest(join(here, path))])
      ),
      toolchain: {
        node: process.version,
        packageLock: manifest.toolchain.packageLock,
        packageLockSha256: toolchain.packageLockSha256,
        esbuild: manifest.toolchain.esbuild,
        terser: manifest.toolchain.terser,
        graph: manifest.toolchain.graph
      },
      codec: {
        path: relative(repository, options.codec).replaceAll('\\', '/'),
        sha256: fileDigest(options.codec),
        schemaVersion: measured.schemaVersion,
        ...measured.codecs
      },
      ports: byPort,
      disputedBaselines: manifest.disputedBaselines.filter(
        (claim) => byPort.some((port) => port.id === claim.portId)
      ).map((claim) => {
        const measuredPort = byPort.find((port) => port.id === claim.portId)
        return {
          ...claim,
          graph: measuredPort.artifacts.find((artifact) => artifact.role === 'diagnostic-disputed-graph'),
          terser: measuredPort.artifacts.find((artifact) => artifact.role === 'diagnostic-disputed-terser')
        }
      }),
      summary
    },
    manifest
  )
}

function historicalCheck(lane, expected, observed, explanation) {
  const metrics = ['raw', 'gzip9', 'brotli11']
  const historical = Object.fromEntries(metrics.map((metric) => [metric, expected[metric]]))
  const current = Object.fromEntries(metrics.map((metric) => [metric, observed[metric]]))
  const differences = metrics.filter((metric) => historical[metric] !== current[metric])
  const match = differences.length === 0
  const reason = differences.length === 1 && differences[0] === 'gzip9'
    ? 'Raw and Brotli match; only gzip differs. The historical row has no compressor artifact hash, while the canonical value uses the recorded stock zlib 1.3.1 codec binary.'
    : explanation
  return {lane, historical, current, match, explanation: match ? null : reason}
}

function metricTotals(artifacts) {
  return Object.fromEntries(
    ['raw', 'gzip9', 'brotli11'].map((metric) => [
      metric,
      artifacts.reduce((sum, artifact) => sum + artifact[metric], 0)
    ])
  )
}

function markdownReport(report) {
  const lines = [
    '# Markdown stack measurement',
    '',
    `Generated: ${report.generatedAt}`,
    '',
    `Manifest SHA-256: \`${report.manifestSha256}\``,
    '',
    '## Canonical comparison',
    '',
    '| Port | Official Terser raw | Official Terser gzip | Official Terser Brotli | Lil raw | Lil gzip | Lil Brotli | Brotli delta | Result |',
    '|---|---:|---:|---:|---:|---:|---:|---:|---|'
  ]
  for (const port of report.ports) {
    const comparison = port.comparison
    lines.push(
      `| ${port.id} | ${comparison.official.raw} | ${comparison.official.gzip9} | ` +
        `${comparison.official.brotli11} | ${comparison.lil.raw} | ${comparison.lil.gzip9} | ` +
        `${comparison.lil.brotli11} | ${comparison.brotliDelta > 0 ? '+' : ''}${comparison.brotliDelta} | ` +
        `${comparison.result} |`
    )
  }
  lines.push(
    `| **Total** | **${report.summary.official.raw}** | **${report.summary.official.gzip9}** | ` +
      `**${report.summary.official.brotli11}** | **${report.summary.lil.raw}** | **${report.summary.lil.gzip9}** | ` +
      `**${report.summary.lil.brotli11}** | ` +
      `**${report.summary.delta.brotli11 > 0 ? '+' : ''}${report.summary.delta.brotli11}** | ` +
      `**${report.summary.wins}W / ${report.summary.losses}L / ${report.summary.ties}T** |`,
    '',
    'Positive delta means the standard Lil graph is larger. Results use Brotli.',
    '',
    '## Canonical artifact hashes',
    '',
    '| Port | Official graph SHA-256 | Official Terser SHA-256 | Lil graph SHA-256 |',
    '|---|---|---|---|'
  )
  for (const port of report.ports) {
    const artifact = (role) => port.artifacts.find((item) => item.role === role)
    lines.push(
      `| ${port.id} | \`${artifact('official-graph').sha256}\` | ` +
        `\`${artifact('official-terser').sha256}\` | \`${artifact('lil-graph').sha256}\` |`
    )
  }
  lines.push(
    '',
    '## Baseline discrepancies',
    '',
    '| Port | Lane | Historical raw/gzip/Brotli | Canonical raw/gzip/Brotli | Explanation |',
    '|---|---|---|---|---|'
  )
  let exactChecks = 0
  for (const port of report.ports) {
    for (const check of port.historicalChecks) {
      if (check.match) {
        exactChecks += 1
        continue
      }
      lines.push(
        `| ${port.id} | \`${check.lane}\` | ${check.historical.raw}/${check.historical.gzip9}/${check.historical.brotli11} | ` +
          `${check.current.raw}/${check.current.gzip9}/${check.current.brotli11} | ` +
          `${check.explanation} |`
      )
    }
  }
  lines.push(
    '',
    `${exactChecks} historical lanes matched all three metrics exactly; only differences are listed above.`,
    '',
    '## Disputed claims',
    ''
  )
  for (const claim of report.disputedBaselines) {
    lines.push(
      `- **${claim.portId}: ${claim.claim} (${claim.status}).** ${claim.reason} ` +
        `Reproduced diagnostic: ${claim.terser.raw}/${claim.terser.gzip9}/${claim.terser.brotli11} bytes ` +
        `(raw/gzip/Brotli), SHA-256 \`${claim.terser.sha256}\`.`
    )
  }
  lines.push(
    '',
    '## Reproduction',
    '',
    '```sh',
    report.commands.contractTests,
    report.commands.checkInputs,
    report.commands.measure,
    report.commands.fullTests,
    '```',
    '',
    `Node: \`${report.toolchain.node}\`; esbuild: \`${report.toolchain.esbuild.version}\`; ` +
      `Terser: \`${report.toolchain.terser.version}\`; lock SHA-256: \`${report.toolchain.packageLockSha256}\`; ` +
      `codec SHA-256: \`${report.codec.sha256}\`.`,
    '',
    `Harness SHA-256: ${Object.entries(report.harness).map(([path, digest]) => `\`${path}\` ${digest}`).join('; ')}.`,
    '',
    'esbuild receives each exact official public root entry directly, which preserves every root export. Standalone standard Lil ESM files are copied byte-for-byte; only entries with runtime imports are bundled to complete their graph, and no Lil graph is post-minified. The equivalent official graph is minified once with pinned Terser options. React and `react/*` are external only for React Markdown, on both sides. All other sibling artifacts, including every closed build, are diagnostic and can never be selected.',
    ''
  )
  return lines.join('\n')
}

function writeOutput(path, value) {
  mkdirSync(dirname(path), {recursive: true})
  writeFileSync(path, value)
  console.error(`wrote ${path}`)
}

function cloneUpstreams(selected, options) {
  mkdirSync(options.upstreamRoot, {recursive: true})
  for (const port of selected) {
    const destination = resolveUpstreamRoot(port, options)
    if (existsSync(destination)) throw new Error(`${port.id}: clone destination already exists: ${destination}`)
    console.error(`[${port.id}] cloning ${port.upstream.repository} tag ${port.upstream.tag}`)
    command('git', [
      'clone',
      '--depth',
      '1',
      '--branch',
      port.upstream.tag,
      port.upstream.repository,
      destination
    ])
    verifyUpstreamGit(destination, port)
  }
}

function testReport(selected, inputReports) {
  const ports = []
  for (const port of selected) {
    const root = resolvePortRoot(port)
    console.error(`[${port.id}] ${port.test.command.join(' ')}`)
    const result = spawnSync(port.test.command[0], port.test.command.slice(1), {
      cwd: root,
      encoding: 'utf8',
      maxBuffer: 128 * 1024 * 1024,
      timeout: port.test.timeoutMs,
      windowsHide: true
    })
    const output = `${result.stdout ?? ''}\n${result.stderr ?? ''}`.replace(/\u001b\[[0-9;]*m/gu, '')
    if (result.error) throw result.error
    if (result.status !== 0) throw new Error(`${port.id}: tests exited ${result.status}\n${output}`)
    const counts = parseTestSummaries(output, port.test.summaries)
    for (const [index, observed] of counts.summaries.entries()) {
      const expected = port.test.summaries[index]
      if (observed.passed !== expected.expectedPassed || observed.failed !== expected.expectedFailed) {
        throw new Error(
          `${port.id} ${observed.parser}: observed ${observed.passed} passed/${observed.failed} failed; expected ` +
            `${expected.expectedPassed}/${expected.expectedFailed}`
        )
      }
    }
    ports.push({id: port.id, input: inputReports.find((input) => input.id === port.id), counts})
  }
  return {
    schemaVersion: 2,
    format: 'lilscript-markdown-stack-tests',
    generatedAt: new Date().toISOString(),
    manifestSha256: sha256(manifestBytes),
    ports
  }
}

const options = parseArguments(process.argv.slice(2))
const selected = manifest.ports.filter((port) => options.only.includes(port.id))

if (options.mode === 'check') {
  console.log(`markdown-stack manifest valid: ${manifest.ports.length} ports`)
} else if (options.mode === 'clone-upstreams') {
  cloneUpstreams(selected, options)
  console.log(`cloned and verified ${selected.length} pinned upstream repositories`)
} else {
  const inputs = selected.map((port) => verifyInputs(port, options))
  if (options.mode === 'check-inputs') {
    const report = {
      schemaVersion: 2,
      format: 'lilscript-markdown-stack-input-audit',
      generatedAt: new Date().toISOString(),
      manifestSha256: sha256(manifestBytes),
      ports: inputs
    }
    if (options.json) writeOutput(options.json, stableJson(report))
    const moduleCount = inputs.reduce((sum, input) => sum + input.sourceAudit.total, 0)
    const dirtyCount = inputs.filter((input) => input.port.git.dirty).length
    console.log(
      `markdown-stack inputs valid: ${inputs.length} ports, ${moduleCount} runtime modules, ` +
        `${dirtyCount} dirty port worktrees recorded`
    )
  } else if (options.mode === 'measure') {
    const report = await measurementReport(selected, inputs, options)
    if (options.json) writeOutput(options.json, stableJson(report))
    if (options.markdown) writeOutput(options.markdown, markdownReport(report))
    if (!options.json) process.stdout.write(stableJson(report))
  } else if (options.mode === 'run-tests') {
    const report = testReport(selected, inputs)
    if (options.json) writeOutput(options.json, stableJson(report))
    else process.stdout.write(stableJson(report))
  }
}
