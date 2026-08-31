import {createHash} from 'node:crypto'

export const REQUIRED_PORT_IDS = Object.freeze([
  'hast-util-to-html',
  'katex',
  'mdast-util-from-markdown',
  'mdast-util-to-hast',
  'micromark',
  'react-markdown',
  'rehype',
  'rehype-katex',
  'rehype-stringify',
  'remark',
  'remark-breaks',
  'remark-gfm',
  'remark-math',
  'remark-parse',
  'remark-rehype',
  'unified'
])

const statuses = new Set([
  'consolidated',
  'facade',
  'generated-data',
  'host-js',
  'unsupported'
])

function record(value, label) {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${label} must be an object`)
  }
  return value
}

function text(value, label) {
  if (typeof value !== 'string' || value.length === 0) {
    throw new Error(`${label} must be a non-empty string`)
  }
  return value
}

function integer(value, label, minimum = 0) {
  if (!Number.isSafeInteger(value) || value < minimum) {
    throw new Error(`${label} must be a safe integer >= ${minimum}`)
  }
}

function safeRelative(value, label) {
  text(value, label)
  if (value.startsWith('/') || value.split('/').includes('..')) {
    throw new Error(`${label} must be a safe relative path`)
  }
}

function stringArray(value, label, {allowEmpty = false} = {}) {
  if (!Array.isArray(value) || (!allowEmpty && value.length === 0)) {
    throw new Error(`${label} must be ${allowEmpty ? 'an' : 'a non-empty'} array`)
  }
  for (const [index, item] of value.entries()) text(item, `${label}[${index}]`)
  if (new Set(value).size !== value.length) {
    throw new Error(`${label} must not contain duplicates`)
  }
}

function gitObject(value, label) {
  if (typeof value !== 'string' || !/^[0-9a-f]{40}$/u.test(value)) {
    throw new Error(`${label} must be a full lowercase Git object id`)
  }
}

function assertExports(value, label) {
  if (typeof value === 'string') return text(value, label)
  const item = record(value, label)
  const keys = Object.keys(item)
  if (keys.length === 0) throw new Error(`${label} must not be empty`)
  for (const key of keys) assertExports(item[key], `${label}.${key}`)
}

export function stableValue(value) {
  if (Array.isArray(value)) return value.map(stableValue)
  if (value !== null && typeof value === 'object') {
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((key) => [key, stableValue(value[key])])
    )
  }
  return value
}

export function stableJson(value) {
  return `${JSON.stringify(stableValue(value), null, 2)}\n`
}

export function sha256(value) {
  return createHash('sha256').update(value).digest('hex')
}

export function exportTargets(packageJson) {
  const targets = []
  const visit = (value, key) => {
    if (typeof value === 'string') {
      targets.push({key, target: value})
    } else if (value && typeof value === 'object' && !Array.isArray(value)) {
      for (const [childKey, child] of Object.entries(value)) {
        visit(child, key ? `${key}.${childKey}` : childKey)
      }
    }
  }
  for (const field of ['main', 'module', 'types', 'unpkg', 'jsdelivr', 'bin']) {
    if (typeof packageJson[field] === 'string') visit(packageJson[field], field)
  }
  visit(packageJson.exports, 'exports')
  return targets
}

export function assertManifest(value) {
  const manifest = record(value, 'manifest')
  if (manifest.schemaVersion !== 3 || manifest.format !== 'lilscript-markdown-stack') {
    throw new Error('manifest has an unsupported schema or format')
  }
  safeRelative(manifest.defaultUpstreamRoot, 'manifest.defaultUpstreamRoot')

  const codec = record(manifest.codec, 'manifest.codec')
  safeRelative(codec.defaultPath, 'manifest.codec.defaultPath')
  if (
    codec.schemaVersion !== 1 ||
    codec.gzip9?.encoder !== 'upstream-stock-zlib-c' ||
    codec.gzip9?.libraryVersion !== '1.3.1' ||
    codec.gzip9?.level !== 9 ||
    codec.gzip9?.mtime !== 0 ||
    codec.brotli11?.encoder !== 'official-google-brotli-c' ||
    codec.brotli11?.libraryVersion !== '1.1.0' ||
    codec.brotli11?.quality !== 11 ||
    codec.brotli11?.lgwin !== 22 ||
    codec.brotli11?.mode !== 'generic'
  ) {
    throw new Error('manifest codec contract is not canonical')
  }
  if (!Array.isArray(manifest.disputedBaselines) || manifest.disputedBaselines.length === 0) {
    throw new Error('manifest must record disputed baselines')
  }
  for (const [index, claim] of manifest.disputedBaselines.entries()) {
    record(claim, `manifest.disputedBaselines[${index}]`)
    text(claim.portId, `manifest.disputedBaselines[${index}].portId`)
    text(claim.claim, `manifest.disputedBaselines[${index}].claim`)
    if (claim.status !== 'diagnostic-only') throw new Error('disputed baselines must be diagnostic-only')
    text(claim.reason, `manifest.disputedBaselines[${index}].reason`)
  }

  const toolchain = record(manifest.toolchain, 'manifest.toolchain')
  safeRelative(toolchain.packageLock, 'manifest.toolchain.packageLock')
  if (
    toolchain.esbuild?.version !== '0.28.1' ||
    JSON.stringify(toolchain.esbuild.options) !==
      JSON.stringify({
        bundle: true,
        format: 'esm',
        platform: 'neutral',
        jsx: 'automatic',
        legalComments: 'none',
        treeShaking: true
      }) ||
    toolchain.terser?.version !== '5.51.2' ||
    JSON.stringify(toolchain.terser.options) !==
      JSON.stringify({module: true, compress: true, mangle: true})
  ) {
    throw new Error('manifest toolchain contract is not canonical')
  }
  const graph = record(toolchain.graph, 'manifest.toolchain.graph')
  if (JSON.stringify(graph.browserPorts) !== JSON.stringify(['unified', 'react-markdown'])) {
    throw new Error('manifest browser graph ports are not canonical')
  }
  if (JSON.stringify(graph.lilBundlePorts) !== JSON.stringify(['remark', 'unified', 'react-markdown'])) {
    throw new Error('manifest Lil graph bundle ports are not canonical')
  }
  if (
    JSON.stringify(graph.retention) !== JSON.stringify({
      kind: 'public-entry-exports',
      source: 'esbuild entry points preserve every public export; standalone Lil ESM is copied byte-for-byte'
    })
  ) {
    throw new Error('manifest graph retention contract is not canonical')
  }
  const officialPackages = record(graph.officialPackages, 'manifest.toolchain.graph.officialPackages')
  if (Object.keys(officialPackages).length !== 16) {
    throw new Error('manifest must pin exactly 16 official graph packages')
  }
  const reactGraph = record(toolchain.reactGraph, 'manifest.toolchain.reactGraph')
  if (reactGraph.officialPackage !== 'react-markdown' || reactGraph.officialVersion !== '10.1.0') {
    throw new Error('manifest React official graph package is not canonical')
  }
  stringArray(reactGraph.external, 'manifest.toolchain.reactGraph.external')
  if (JSON.stringify(reactGraph.external) !== JSON.stringify(['react', 'react/*'])) {
    throw new Error('manifest React externals are not canonical')
  }
  const aliases = record(reactGraph.portAliases, 'manifest.toolchain.reactGraph.portAliases')
  if (Object.keys(aliases).length === 0) throw new Error('manifest React port aliases must not be empty')
  for (const [name, alias] of Object.entries(aliases)) {
    text(name, 'manifest React alias name')
    record(alias, `manifest React alias ${name}`)
    text(alias.portId, `manifest React alias ${name}.portId`)
    safeRelative(alias.path, `manifest React alias ${name}.path`)
  }
  const dependencies = record(reactGraph.pinnedDependencies, 'manifest.toolchain.reactGraph.pinnedDependencies')
  for (const [name, version] of Object.entries(dependencies)) {
    text(name, 'manifest React dependency name')
    text(version, `manifest React dependency ${name}`)
    if (/^[~^*><=]/u.test(version)) throw new Error(`manifest React dependency ${name} must be exact`)
  }

  if (!Array.isArray(manifest.ports) || manifest.ports.length !== 16) {
    throw new Error('manifest must contain exactly 16 ports')
  }
  const ids = []
  for (const [index, value] of manifest.ports.entries()) {
    const port = record(value, `manifest.ports[${index}]`)
    const label = `port ${text(port.id, `manifest.ports[${index}].id`)}`
    ids.push(port.id)

    const upstream = record(port.upstream, `${label}.upstream`)
    if (!/^https:\/\/github\.com\/[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+\.git$/u.test(upstream.repository)) {
      throw new Error(`${label}.upstream.repository must be an exact GitHub clone URL`)
    }
    text(upstream.tag, `${label}.upstream.tag`)
    gitObject(upstream.commit, `${label}.upstream.commit`)
    gitObject(upstream.tree, `${label}.upstream.tree`)
    safeRelative(upstream.packagePath, `${label}.upstream.packagePath`)
    text(upstream.packageName, `${label}.upstream.packageName`)
    text(upstream.packageVersion, `${label}.upstream.packageVersion`)
    if (!Array.isArray(upstream.generatedExportTargets)) {
      throw new Error(`${label}.upstream.generatedExportTargets must be an array`)
    }
    for (const [targetIndex, exception] of upstream.generatedExportTargets.entries()) {
      const targetLabel = `${label}.upstream.generatedExportTargets[${targetIndex}]`
      record(exception, targetLabel)
      text(exception.pattern, `${targetLabel}.pattern`)
      text(exception.reason, `${targetLabel}.reason`)
    }

    const sibling = record(port.port, `${label}.port`)
    text(sibling.repositoryEnv, `${label}.port.repositoryEnv`)
    text(sibling.defaultSibling, `${label}.port.defaultSibling`)
    text(sibling.packageName, `${label}.port.packageName`)
    text(sibling.packageVersion, `${label}.port.packageVersion`)
    const scripts = record(sibling.scripts, `${label}.port.scripts`)
    if (Object.keys(scripts).length === 0) throw new Error(`${label}.port.scripts must not be empty`)
    for (const [name, script] of Object.entries(scripts)) {
      text(name, `${label}.port script name`)
      text(script, `${label}.port.scripts.${name}`)
    }
    const evidence = record(sibling.evidence, `${label}.port.evidence`)
    safeRelative(evidence.path, `${label}.port.evidence.path`)
    integer(evidence.expectedPassed, `${label}.port.evidence.expectedPassed`, 1)
    integer(evidence.expectedFailed, `${label}.port.evidence.expectedFailed`)
    assertExports(sibling.exports, `${label}.port.exports`)
    stringArray(sibling.artifacts, `${label}.port.artifacts`)
    for (const artifact of sibling.artifacts) safeRelative(artifact, `${label} artifact`)
    if (sibling.officialArtifact !== null) {
      safeRelative(sibling.officialArtifact, `${label}.port.officialArtifact`)
    }

    const measurement = record(port.measurement, `${label}.measurement`)
    if (measurement.officialEntry !== upstream.packageName) {
      throw new Error(`${label}.measurement.officialEntry must be the pinned official package root`)
    }
    if (officialPackages[measurement.officialEntry] !== upstream.packageVersion) {
      throw new Error(`${label} official graph package is not pinned at the upstream version`)
    }
    safeRelative(measurement.lilEntry, `${label}.measurement.lilEntry`)
    if (!sibling.artifacts.includes(measurement.lilEntry)) {
      throw new Error(`${label}.measurement.lilEntry must be a declared standard artifact`)
    }
    if (/closed/u.test(measurement.lilEntry)) {
      throw new Error(`${label}.measurement.lilEntry must not select a closed artifact`)
    }
    const rootExport = sibling.exports['.']
    const publicImport = typeof rootExport === 'string' ? rootExport : rootExport?.import
    if (measurement.lilEntry !== publicImport?.replace(/^\.\//u, '')) {
      throw new Error(`${label}.measurement.lilEntry must be the root public import export`)
    }
    stringArray(measurement.externals, `${label}.measurement.externals`, {allowEmpty: true})
    const expectedExternals = port.id === 'react-markdown' ? ['react', 'react/*'] : []
    if (JSON.stringify(measurement.externals) !== JSON.stringify(expectedExternals)) {
      throw new Error(`${label}.measurement.externals is not canonical`)
    }
    const historical = record(measurement.historical, `${label}.measurement.historical`)
    for (const name of ['officialGraph', 'officialTerser', 'lil']) {
      text(historical[name], `${label}.measurement.historical.${name}`)
    }
    if (historical.officialTerser !== 'official-terser-mangle' || historical.lil !== 'itslil') {
      throw new Error(`${label}.measurement historical lanes are not canonical`)
    }

    const audit = record(port.sourceAudit, `${label}.sourceAudit`)
    stringArray(audit.portRoots, `${label}.sourceAudit.portRoots`)
    for (const root of audit.portRoots) safeRelative(root, `${label} source inventory root`)
    if (!Array.isArray(audit.scopes) || audit.scopes.length === 0) {
      throw new Error(`${label}.sourceAudit.scopes must be a non-empty array`)
    }
    for (const [scopeIndex, scope] of audit.scopes.entries()) {
      const scopeLabel = `${label}.sourceAudit.scopes[${scopeIndex}]`
      record(scope, scopeLabel)
      safeRelative(scope.upstream, `${scopeLabel}.upstream`)
      safeRelative(scope.port, `${scopeLabel}.port`)
    }
    if (!Array.isArray(audit.statuses)) {
      throw new Error(`${label}.sourceAudit.statuses must be an array`)
    }
    const patterns = new Set()
    for (const [statusIndex, status] of audit.statuses.entries()) {
      const statusLabel = `${label}.sourceAudit.statuses[${statusIndex}]`
      record(status, statusLabel)
      text(status.pattern, `${statusLabel}.pattern`)
      if (patterns.has(status.pattern)) throw new Error(`${label} repeats status pattern ${status.pattern}`)
      patterns.add(status.pattern)
      if (!statuses.has(status.status)) throw new Error(`${statusLabel}.status is unsupported`)
      text(status.reason, `${statusLabel}.reason`)
      if (status.target !== null) safeRelative(status.target, `${statusLabel}.target`)
    }

    const test = record(port.test, `${label}.test`)
    stringArray(test.command, `${label}.test.command`)
    text(test.packageScript, `${label}.test.packageScript`)
    if (test.packageScript !== sibling.scripts.test) {
      throw new Error(`${label}.test.packageScript must match port.scripts.test`)
    }
    if (!Array.isArray(test.summaries) || test.summaries.length === 0) {
      throw new Error(`${label}.test.summaries must be a non-empty array`)
    }
    const parsers = new Set()
    for (const [summaryIndex, summary] of test.summaries.entries()) {
      const summaryLabel = `${label}.test.summaries[${summaryIndex}]`
      record(summary, summaryLabel)
      if (!['node-test', 'jest'].includes(summary.parser)) {
        throw new Error(`${summaryLabel}.parser is unsupported`)
      }
      if (parsers.has(summary.parser)) throw new Error(`${label} repeats test parser ${summary.parser}`)
      parsers.add(summary.parser)
      integer(summary.expectedPassed, `${summaryLabel}.expectedPassed`, 1)
      integer(summary.expectedFailed, `${summaryLabel}.expectedFailed`)
    }
    integer(test.timeoutMs, `${label}.test.timeoutMs`, 1)
  }

  if (new Set(ids).size !== ids.length) throw new Error('port ids must be unique')
  if (JSON.stringify([...ids].sort()) !== JSON.stringify(REQUIRED_PORT_IDS)) {
    throw new Error(`manifest must contain ${REQUIRED_PORT_IDS.join(', ')}`)
  }
  for (const [name, alias] of Object.entries(reactGraph.portAliases)) {
    if (!ids.includes(alias.portId)) throw new Error(`manifest React alias ${name} names an unknown port`)
  }
  if (
    JSON.stringify(stableValue(officialPackages)) !==
    JSON.stringify(stableValue(Object.fromEntries(manifest.ports.map((port) => [port.upstream.packageName, port.upstream.packageVersion]))))
  ) {
    throw new Error('manifest official graph package pins differ from the port contracts')
  }
  if (manifest.disputedBaselines.some((claim) => !ids.includes(claim.portId))) {
    throw new Error('manifest disputed baseline names an unknown port')
  }
  return manifest
}

export function parseTestCounts(output, parser) {
  if (parser === 'node-test') {
    const tests = [...output.matchAll(/^# tests (\d+)$/gmu)]
    const pass = [...output.matchAll(/^# pass (\d+)$/gmu)]
    const fail = [...output.matchAll(/^# fail (\d+)$/gmu)]
    if (tests.length !== 1 || pass.length !== 1 || fail.length !== 1) {
      throw new Error('expected exactly one complete Node test summary')
    }
    const counts = {total: Number(tests[0][1]), passed: Number(pass[0][1]), failed: Number(fail[0][1])}
    if (counts.total !== counts.passed + counts.failed) throw new Error('Node test summary is inconsistent')
    return counts
  }
  if (parser !== 'jest') throw new Error(`unsupported test count parser ${parser}`)
  const matches = [...output.matchAll(/^Tests:\s+(?:([\d,]+) failed,\s+)?([\d,]+) passed,\s+([\d,]+) total\s*$/gmu)]
  if (matches.length !== 1) throw new Error('expected exactly one Jest test summary')
  const match = matches[0]
  const count = (value) => Number((value ?? '0').replaceAll(',', ''))
  const counts = {total: count(match[3]), passed: count(match[2]), failed: count(match[1])}
  if (counts.total !== counts.passed + counts.failed) throw new Error('Jest test summary is inconsistent')
  return counts
}

export function parseTestSummaries(output, expected) {
  if (!Array.isArray(expected) || expected.length === 0) {
    throw new Error('expected test summaries must be a non-empty array')
  }
  const summaries = expected.map((item) => ({parser: item.parser, ...parseTestCounts(output, item.parser)}))
  return {
    summaries,
    total: summaries.reduce((sum, item) => sum + item.total, 0),
    passed: summaries.reduce((sum, item) => sum + item.passed, 0),
    failed: summaries.reduce((sum, item) => sum + item.failed, 0)
  }
}

export function assertMeasurementReport(report, manifest) {
  record(report, 'report')
  if (report.schemaVersion !== 3 || report.format !== 'lilscript-markdown-stack-measurements') {
    throw new Error('report has an unsupported schema or format')
  }
  if (!Array.isArray(report.ports) || report.ports.length === 0) {
    throw new Error('report must contain measured ports')
  }
  if (Number.isNaN(Date.parse(report.generatedAt))) throw new Error('report.generatedAt must be an ISO timestamp')
  if (!/^[0-9a-f]{64}$/u.test(report.manifestSha256)) throw new Error('report manifest digest is invalid')
  const commands = record(report.commands, 'report.commands')
  for (const name of ['contractTests', 'checkInputs', 'measure', 'fullTests']) {
    text(commands[name], `report.commands.${name}`)
  }
  const harness = record(report.harness, 'report.harness')
  for (const path of ['run.mjs', 'contract.mjs', 'contract.test.mjs', 'package.json', manifest.toolchain.packageLock]) {
    if (!/^[0-9a-f]{64}$/u.test(harness[path])) throw new Error(`report harness digest is invalid for ${path}`)
  }
  text(report.toolchain?.node, 'report Node version')
  safeRelative(report.toolchain?.packageLock, 'report toolchain lockfile')
  if (!/^[0-9a-f]{64}$/u.test(report.toolchain?.packageLockSha256)) {
    throw new Error('report toolchain lockfile digest is invalid')
  }
  for (const name of ['esbuild', 'terser', 'graph']) {
    if (
      JSON.stringify(stableValue(report.toolchain?.[name])) !==
      JSON.stringify(stableValue(manifest.toolchain[name]))
    ) {
      throw new Error(`report ${name} provenance is invalid`)
    }
  }
  const reportedIds = new Set(report.ports.map((port) => port.id))
  const expectedClaims = manifest.disputedBaselines.filter((claim) => reportedIds.has(claim.portId))
  if (!Array.isArray(report.disputedBaselines) || report.disputedBaselines.length !== expectedClaims.length) {
    throw new Error('report disputed baselines differ from the manifest')
  }
  for (const configuredClaim of expectedClaims) {
    const claim = report.disputedBaselines.find((item) => item.portId === configuredClaim.portId)
    for (const key of ['portId', 'claim', 'status', 'reason']) {
      if (claim?.[key] !== configuredClaim[key]) throw new Error('report disputed baselines differ from the manifest')
    }
    if (
      claim.graph?.role !== 'diagnostic-disputed-graph' ||
      claim.terser?.role !== 'diagnostic-disputed-terser'
    ) {
      throw new Error(`${configuredClaim.portId} disputed baseline has no diagnostic artifacts`)
    }
  }
  if (!/^[0-9a-f]{64}$/u.test(report.codec?.sha256)) throw new Error('report codec digest is invalid')
  if (report.codec?.schemaVersion !== manifest.codec.schemaVersion) throw new Error('report codec schema is invalid')
  for (const name of ['gzip9', 'brotli11']) {
    if (JSON.stringify(stableValue(report.codec?.[name])) !== JSON.stringify(stableValue(manifest.codec[name]))) {
      throw new Error(`report ${name} provenance is invalid`)
    }
  }
  const known = new Set(manifest.ports.map((port) => port.id))
  const seen = new Set()
  for (const port of report.ports) {
    if (!known.has(port.id)) throw new Error(`report names unknown port ${port.id}`)
    if (seen.has(port.id)) throw new Error(`report repeats port ${port.id}`)
    seen.add(port.id)
    if (!Array.isArray(port.artifacts) || port.artifacts.length === 0) {
      throw new Error(`${port.id} has no measured artifacts`)
    }
    const configuredPort = manifest.ports.find((item) => item.id === port.id)
    const configured = configuredPort.port
    const artifactKeys = new Set()
    const roles = new Set([
      'official-graph',
      'official-terser',
      'lil-graph',
      'diagnostic-official',
      'diagnostic-port',
      'diagnostic-disputed-graph',
      'diagnostic-disputed-terser'
    ])
    for (const artifact of port.artifacts) {
      safeRelative(artifact.path, `${port.id} artifact path`)
      if (!roles.has(artifact.role)) {
        throw new Error(`${port.id} artifact role is invalid`)
      }
      const key = `${artifact.role}:${artifact.path}`
      if (artifactKeys.has(key)) throw new Error(`${port.id} repeats artifact ${key}`)
      artifactKeys.add(key)
      if (!/^[0-9a-f]{64}$/u.test(artifact.sha256)) throw new Error(`${port.id} artifact digest is invalid`)
      for (const metric of ['raw', 'gzip9', 'brotli11']) {
        integer(artifact[metric], `${port.id}.${artifact.path}.${metric}`)
      }
    }
    for (const path of configured.artifacts) {
      if (!port.artifacts.some((artifact) => artifact.role === 'diagnostic-port' && artifact.path === path)) {
        throw new Error(`${port.id} report omits diagnostic port artifact ${path}`)
      }
    }
    for (const role of ['official-graph', 'official-terser', 'lil-graph']) {
      if (port.artifacts.filter((artifact) => artifact.role === role).length !== 1) {
        throw new Error(`${port.id} must have exactly one ${role} artifact`)
      }
    }
    if (configured.officialArtifact !== null) {
      if (!port.artifacts.some(
        (artifact) => artifact.role === 'diagnostic-official' && artifact.path === configured.officialArtifact
      )) {
        throw new Error(`${port.id} report omits its historical official diagnostic`)
      }
    }

    const lane = record(port.lane, `${port.id}.lane`)
    if (JSON.stringify(lane.retention) !== JSON.stringify(manifest.toolchain.graph.retention)) {
      throw new Error(`${port.id} graph retention differs from the manifest`)
    }
    if (JSON.stringify(lane.externals) !== JSON.stringify(configuredPort.measurement.externals)) {
      throw new Error(`${port.id} graph externals differ from the manifest`)
    }
    const expectedPlatform = manifest.toolchain.graph.browserPorts.includes(port.id) ? 'browser' : 'neutral'
    if (lane.platform !== expectedPlatform) throw new Error(`${port.id} graph platform differs from the manifest`)
    if (lane.officialEntry?.specifier !== configuredPort.measurement.officialEntry) {
      throw new Error(`${port.id} official entry differs from the manifest`)
    }
    if (lane.lilEntry?.path !== configuredPort.measurement.lilEntry || lane.lilPostMinified !== false) {
      throw new Error(`${port.id} Lil lane is not the unminified standard public ESM graph`)
    }
    const expectedLilMode = manifest.toolchain.graph.lilBundlePorts.includes(port.id) ? 'bundle' : 'artifact'
    if (lane.lilGraphMode !== expectedLilMode) throw new Error(`${port.id} Lil graph mode differs from the manifest`)
    for (const entry of [lane.officialEntry, lane.lilEntry]) {
      if (!/^[0-9a-f]{64}$/u.test(entry?.sha256)) throw new Error(`${port.id} entry digest is invalid`)
      stringArray(entry.exports, `${port.id} entry exports`)
    }
    for (const graphName of ['officialInputs', 'lilInputs']) {
      if (!Array.isArray(lane[graphName]) || lane[graphName].length === 0) {
        throw new Error(`${port.id} ${graphName} is empty`)
      }
      for (const input of lane[graphName]) {
        text(input.path, `${port.id} graph input path`)
        integer(input.bytes, `${port.id} graph input bytes`)
        if (!/^[0-9a-f]{64}$/u.test(input.sha256)) throw new Error(`${port.id} graph input digest is invalid`)
      }
    }

    if (!Array.isArray(port.historicalChecks) || port.historicalChecks.length !== 3) {
      throw new Error(`${port.id} historical checks are incomplete`)
    }
    for (const check of port.historicalChecks) {
      text(check.lane, `${port.id} historical lane`)
      const historical = record(check.historical, `${port.id}.${check.lane}.historical`)
      const current = record(check.current, `${port.id}.${check.lane}.current`)
      for (const metric of ['raw', 'gzip9', 'brotli11']) {
        integer(historical[metric], `${port.id}.${check.lane}.historical.${metric}`)
        integer(current[metric], `${port.id}.${check.lane}.current.${metric}`)
      }
      const match = ['raw', 'gzip9', 'brotli11'].every((metric) => historical[metric] === current[metric])
      if (check.match !== match) {
        throw new Error(`${port.id}.${check.lane} historical match flag is invalid`)
      }
      if (check.match && check.explanation !== null) {
        throw new Error(`${port.id}.${check.lane} exact match must not have an explanation`)
      }
      if (!check.match) text(check.explanation, `${port.id}.${check.lane} difference explanation`)
    }
    for (const laneName of ['official-graph', 'official-terser', 'lil-graph']) {
      if (!port.historicalChecks.some((check) => check.lane === laneName)) {
        throw new Error(`${port.id} historical check omits ${laneName}`)
      }
    }

    const comparison = record(port.comparison, `${port.id}.comparison`)
    record(comparison.qualification, `${port.id}.comparison.qualification`)
    const matchArtifact = (candidate, label) => {
      record(candidate, label)
      const artifact = port.artifacts.find(
        (item) => item.path === candidate.path && item.role === candidate.role
      )
      if (!artifact || JSON.stringify(stableValue(artifact)) !== JSON.stringify(stableValue(candidate))) {
        throw new Error(`${label} does not reference an exact measured artifact`)
      }
      return artifact
    }
    const lil = matchArtifact(comparison.lil, `${port.id}.comparison.lil`)
    const official = matchArtifact(comparison.official, `${port.id}.comparison.official`)
    if (lil.role !== 'lil-graph') throw new Error(`${port.id} Lil comparison is not the generated graph`)
    if (official.role !== 'official-terser') throw new Error(`${port.id} official comparison is not Terser`)
    if (comparison.qualification?.closedArtifactsEligible !== false) {
      throw new Error(`${port.id} closed artifacts must be ineligible`)
    }
    const delta = lil.brotli11 - official.brotli11
    if (comparison.brotliDelta !== delta) throw new Error(`${port.id} Brotli delta is invalid`)
    const result = delta < 0 ? 'win' : delta > 0 ? 'loss' : 'tie'
    if (comparison.result !== result) throw new Error(`${port.id} comparison result is invalid`)
  }
  const summary = record(report.summary, 'report.summary')
  const metricTotals = (side) => Object.fromEntries(
    ['raw', 'gzip9', 'brotli11'].map((metric) => [
      metric,
      report.ports.reduce((sum, port) => sum + port.comparison[side][metric], 0)
    ])
  )
  const expectedSummary = {
    lil: metricTotals('lil'),
    official: metricTotals('official'),
    wins: report.ports.filter((port) => port.comparison.result === 'win').length,
    losses: report.ports.filter((port) => port.comparison.result === 'loss').length,
    ties: report.ports.filter((port) => port.comparison.result === 'tie').length
  }
  expectedSummary.delta = Object.fromEntries(
    ['raw', 'gzip9', 'brotli11'].map((metric) => [
      metric,
      expectedSummary.lil[metric] - expectedSummary.official[metric]
    ])
  )
  if (JSON.stringify(stableValue(summary)) !== JSON.stringify(stableValue(expectedSummary))) {
    throw new Error('report summary does not match port comparisons')
  }
  return report
}
