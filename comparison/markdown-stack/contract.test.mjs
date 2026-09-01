import assert from 'node:assert/strict'
import {spawnSync} from 'node:child_process'
import {readFileSync} from 'node:fs'
import {dirname, join} from 'node:path'
import test from 'node:test'
import {fileURLToPath} from 'node:url'

import {
  REQUIRED_PORT_IDS,
  assertManifest,
  assertMeasurementReport,
  exportTargets,
  parseTestCounts,
  parseTestSummaries,
  sha256
} from './contract.mjs'

const here = dirname(fileURLToPath(import.meta.url))
const manifestBytes = readFileSync(join(here, 'manifest.json'))
const manifest = JSON.parse(manifestBytes)
const packageJson = JSON.parse(readFileSync(join(here, 'package.json')))
const packageLock = JSON.parse(readFileSync(join(here, 'package-lock.json')))

test('the manifest pins all 16 markdown-stack ports and canonical graph tools', () => {
  assert.doesNotThrow(() => assertManifest(manifest))
  assert.deepEqual(manifest.ports.map((port) => port.id).sort(), REQUIRED_PORT_IDS)
  assert.equal(manifest.toolchain.esbuild.version, '0.28.1')
  assert.equal(manifest.toolchain.terser.version, '5.51.2')
  assert.deepEqual(manifest.toolchain.terser.options, {module: true, compress: true, mangle: true})
  for (const port of manifest.ports) {
    assert.match(port.upstream.commit, /^[0-9a-f]{40}$/u)
    assert.match(port.upstream.tree, /^[0-9a-f]{40}$/u)
    assert.match(port.upstream.repository, /^https:\/\/github\.com\//u)
    assert.match(port.port.defaultSibling, /^\.\.\/[a-z0-9-]+lil$/u)
    assert.equal(port.measurement.officialEntry, port.upstream.packageName)
    assert.ok(port.port.artifacts.includes(port.measurement.lilEntry))
    assert.equal(port.measurement.lilEntry.includes('closed'), false)
    assert.deepEqual(port.port.scripts.test, port.test.packageScript)
  }
})

test('the harness package and lock pin every graph input exactly', () => {
  const expected = {
    esbuild: manifest.toolchain.esbuild.version,
    terser: manifest.toolchain.terser.version,
    ...manifest.toolchain.graph.officialPackages,
    [manifest.toolchain.reactGraph.officialPackage]: manifest.toolchain.reactGraph.officialVersion,
    ...manifest.toolchain.reactGraph.pinnedDependencies
  }
  for (const [name, version] of Object.entries(expected)) {
    assert.equal(packageJson.dependencies[name], version, `${name} package pin`)
    assert.equal(packageLock.packages[''].dependencies[name], version, `${name} lock root pin`)
    assert.equal(packageLock.packages[`node_modules/${name}`].version, version, `${name} resolved pin`)
  }
})

test('every concrete JavaScript export target is measured', () => {
  for (const port of manifest.ports) {
    const targets = exportTargets({exports: port.port.exports})
      .map((item) => item.target.replace(/^\.\//u, ''))
      .filter((target) => !target.includes('*') && /\.(?:cjs|mjs|js)$/u.test(target))
    for (const target of targets) {
      assert.ok(port.port.artifacts.includes(target), `${port.id} does not measure ${target}`)
    }
  }
})

test('non-mapped runtime statuses are explicit and justified', () => {
  for (const port of manifest.ports) {
    for (const status of port.sourceAudit.statuses) {
      assert.notEqual(status.pattern, '*')
      assert.ok(status.reason.length >= 24, `${port.id} has a vague reason for ${status.pattern}`)
      if (status.status !== 'unsupported') assert.ok(status.target)
    }
  }
})

test('site evidence and full npm test counts are separate exact contracts', () => {
  const expectedFullPasses = {
    'hast-util-to-html': 455,
    katex: 1247,
    'mdast-util-from-markdown': 744,
    'mdast-util-to-hast': 148,
    micromark: 1958,
    'react-markdown': 116,
    'rehype-katex': 63,
    'rehype-stringify': 10,
    rehype: 146,
    'remark-breaks': 20,
    'remark-gfm': 19,
    'remark-math': 60,
    'remark-parse': 16,
    'remark-rehype': 17,
    remark: 500,
    unified: 215
  }
  assert.equal(
    manifest.ports.reduce((sum, port) => sum + port.port.evidence.expectedPassed, 0),
    5598
  )
  assert.equal(
    manifest.ports.reduce(
      (sum, port) => sum + port.test.summaries.reduce((subtotal, item) => subtotal + item.expectedPassed, 0),
      0
    ),
    5734
  )
  assert.ok(manifest.ports.every((port) => port.test.command.join(' ') === 'npm test'))
  assert.deepEqual(
    Object.fromEntries(manifest.ports.map((port) => [
      port.id,
      port.test.summaries.reduce((sum, item) => sum + item.expectedPassed, 0)
    ])),
    Object.fromEntries(manifest.ports.map((port) => [port.id, expectedFullPasses[port.id]]))
  )
  assert.ok(manifest.ports.every((port) => port.test.summaries.every((item) => item.expectedFailed === 0)))
  const katex = manifest.ports.find((port) => port.id === 'katex')
  assert.deepEqual(katex.test.summaries, [
    {parser: 'node-test', expectedPassed: 17, expectedFailed: 0},
    {parser: 'jest', expectedPassed: 1230, expectedFailed: 0}
  ])
})

test('test count parsers aggregate Node and Jest summaries', () => {
  const output = [
    '# tests 14',
    '# pass 14',
    '# fail 0',
    'Tests:       1,230 passed, 1,230 total'
  ].join('\n')
  assert.deepEqual(parseTestCounts(output, 'node-test'), {total: 14, passed: 14, failed: 0})
  assert.deepEqual(parseTestCounts(output, 'jest'), {total: 1230, passed: 1230, failed: 0})
  assert.deepEqual(parseTestSummaries(output, [{parser: 'node-test'}, {parser: 'jest'}]), {
    summaries: [
      {parser: 'node-test', total: 14, passed: 14, failed: 0},
      {parser: 'jest', total: 1230, passed: 1230, failed: 0}
    ],
    total: 1244,
    passed: 1244,
    failed: 0
  })
})

test('test count parsers fail closed', () => {
  assert.throws(() => parseTestCounts('ok', 'node-test'), /exactly one/u)
  assert.throws(() => parseTestCounts('ok', 'jest'), /exactly one/u)
  assert.throws(
    () => parseTestCounts('# tests 2\n# pass 1\n# fail 0\n', 'node-test'),
    /inconsistent/u
  )
  assert.throws(
    () => parseTestCounts('# tests 1\n# tests 1\n# pass 1\n# fail 0\n', 'node-test'),
    /exactly one/u
  )
  assert.throws(() => parseTestCounts('Tests: 1 passed, 1 total', 'tap'), /unsupported/u)
})

test('every comparison retains all root exports and only React is external', () => {
  const react = manifest.ports.find((port) => port.id === 'react-markdown')
  assert.equal(react.port.officialArtifact, null)
  assert.equal(manifest.toolchain.graph.retention.kind, 'public-entry-exports')
  assert.deepEqual(react.measurement.externals, ['react', 'react/*'])
  assert.ok(manifest.ports.filter((port) => port.id !== 'react-markdown').every(
    (port) => port.measurement.externals.length === 0
  ))
  assert.ok(manifest.ports.every((port) => !port.measurement.lilEntry.includes('closed')))
})

test('measurement reports require canonical graphs, explanations, and exact totals', () => {
  const micromark = manifest.ports.find((port) => port.id === 'micromark')
  const artifact = (path, role, brotli11 = 5) => ({
    path,
    role,
    sha256: 'a'.repeat(64),
    raw: 3,
    gzip9: 4,
    brotli11
  })
  const artifacts = [
    artifact('.generated/official-graph.js', 'official-graph'),
    artifact('.generated/official-terser.js', 'official-terser', 6),
    artifact('.generated/lil-graph.js', 'lil-graph'),
    artifact('.generated/diagnostic-browser-graph.js', 'diagnostic-disputed-graph'),
    artifact('.generated/diagnostic-browser-terser.js', 'diagnostic-disputed-terser'),
    artifact(micromark.port.officialArtifact, 'diagnostic-official'),
    ...micromark.port.artifacts.map((path) => artifact(path, 'diagnostic-port'))
  ]
  const lil = artifacts.find((item) => item.role === 'lil-graph')
  const official = artifacts.find((item) => item.role === 'official-terser')
  const fixture = {
    schemaVersion: 3,
    format: 'lilscript-markdown-stack-measurements',
    generatedAt: '2026-08-30T00:00:00.000Z',
    manifestSha256: sha256(manifestBytes),
    commands: {
      contractTests: 'npm test',
      checkInputs: 'node run.mjs --check-inputs',
      measure: 'node run.mjs --measure',
      fullTests: 'node run.mjs --run-tests'
    },
    harness: {
      'run.mjs': 'f'.repeat(64),
      'contract.mjs': 'f'.repeat(64),
      'contract.test.mjs': 'f'.repeat(64),
      'package.json': 'f'.repeat(64),
      'package-lock.json': 'f'.repeat(64)
    },
    toolchain: {
      node: process.version,
      packageLock: manifest.toolchain.packageLock,
      packageLockSha256: 'c'.repeat(64),
      esbuild: manifest.toolchain.esbuild,
      terser: manifest.toolchain.terser,
      graph: manifest.toolchain.graph
    },
    codec: {
      path: 'target/release/lilscript-codec',
      sha256: 'b'.repeat(64),
      schemaVersion: manifest.codec.schemaVersion,
      gzip9: manifest.codec.gzip9,
      brotli11: manifest.codec.brotli11
    },
    ports: [{
      id: 'micromark',
      lane: {
        retention: manifest.toolchain.graph.retention,
        platform: 'neutral',
        externals: [],
        officialEntry: {
          specifier: 'micromark',
          resolvedPath: 'node_modules/micromark/index.js',
          sha256: 'd'.repeat(64),
          exports: ['compile', 'micromark']
        },
        lilEntry: {
          path: 'dist/micromark.esm.js',
          sha256: 'e'.repeat(64),
          exports: ['compile', 'micromark']
        },
        officialInputs: [{path: 'node_modules/micromark/index.js', bytes: 3, sha256: 'd'.repeat(64)}],
        lilInputs: [{path: '../micromarklil/dist/micromark.esm.js', bytes: 3, sha256: 'e'.repeat(64)}],
        lilGraphMode: 'artifact',
        lilPostMinified: false
      },
      artifacts,
      historicalChecks: [
        {
          lane: 'official-graph',
          historical: {raw: 3, gzip9: 4, brotli11: 5},
          current: {raw: 3, gzip9: 4, brotli11: 5},
          match: true,
          explanation: null
        },
        {
          lane: 'official-terser',
          historical: {raw: 2, gzip9: 4, brotli11: 6},
          current: {raw: 3, gzip9: 4, brotli11: 6},
          match: false,
          explanation: 'Pinned graph inputs differ.'
        },
        {
          lane: 'lil-graph',
          historical: {raw: 3, gzip9: 4, brotli11: 5},
          current: {raw: 3, gzip9: 4, brotli11: 5},
          match: true,
          explanation: null
        }
      ],
      comparison: {
        qualification: {kind: 'fixture', closedArtifactsEligible: false},
        lil,
        official,
        brotliDelta: -1,
        result: 'win'
      }
    }],
    disputedBaselines: manifest.disputedBaselines.map((claim) => ({
      ...claim,
      graph: artifacts.find((item) => item.role === 'diagnostic-disputed-graph'),
      terser: artifacts.find((item) => item.role === 'diagnostic-disputed-terser')
    })),
    summary: {
      lil: {raw: 3, gzip9: 4, brotli11: 5},
      official: {raw: 3, gzip9: 4, brotli11: 6},
      delta: {raw: 0, gzip9: 0, brotli11: -1},
      wins: 1,
      losses: 0,
      ties: 0
    }
  }
  assert.doesNotThrow(() => assertMeasurementReport(fixture, manifest))
  fixture.ports[0].historicalChecks[1].explanation = null
  assert.throws(() => assertMeasurementReport(fixture, manifest), /difference explanation/u)
  fixture.ports[0].historicalChecks[1].explanation = 'Pinned graph inputs differ.'
  fixture.summary.wins = 0
  assert.throws(() => assertMeasurementReport(fixture, manifest), /summary does not match/u)
})

test('the CLI check mode has no sibling or upstream dependency', () => {
  const result = spawnSync(process.execPath, [join(here, 'run.mjs'), '--check'], {encoding: 'utf8'})
  assert.equal(result.status, 0, result.stderr)
  assert.match(result.stdout, /manifest valid: 16 ports/u)
})

test('manifest validation rejects incomplete and weak contracts', () => {
  const missing = structuredClone(manifest)
  missing.ports.pop()
  assert.throws(() => assertManifest(missing), /exactly 16 ports/u)

  const shortCommit = structuredClone(manifest)
  shortCommit.ports[0].upstream.commit = 'deadbeef'
  assert.throws(() => assertManifest(shortCommit), /full lowercase Git object id/u)

  const closed = structuredClone(manifest)
  closed.ports.find((port) => port.id === 'react-markdown').measurement.lilEntry =
    'dist/react-markdown.closed.js'
  assert.throws(() => assertManifest(closed), /must not select a closed artifact/u)

  const external = structuredClone(manifest)
  external.ports.find((port) => port.id === 'micromark').measurement.externals = ['react']
  assert.throws(() => assertManifest(external), /externals is not canonical/u)

  const duplicateParser = structuredClone(manifest)
  duplicateParser.ports[0].test.summaries.push(duplicateParser.ports[0].test.summaries[0])
  assert.throws(() => assertManifest(duplicateParser), /repeats test parser/u)
})
