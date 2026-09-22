import { readFileSync } from 'node:fs'
import { pathToFileURL } from 'node:url'
import { relative, isAbsolute } from 'node:path'
import VitestEvidenceReporter from './vitest-test-reporter.mjs'
import { digest, writeReceipt } from './artifact-evidence.mjs'
import { matchesForbiddenSource } from './observe-vitest-sources.mjs'

const settings = JSON.parse(readFileSync(process.argv[2], 'utf8'))
const { createVitest, startVitest } = await import(pathToFileURL(settings.vitestNode).href)
const reporter = new VitestEvidenceReporter({ cwd: settings.cwd, output: settings.results })
const forbiddenModules = new Map()
const overrides = { plugins: [{
  name: 'lilscript-production-route-observer', enforce: 'pre',
  transform(source, id) {
    const path = id.split('?')[0]
    if (isAbsolute(path) && matchesForbiddenSource(settings.cwd, settings.forbiddenSources, path)) {
      forbiddenModules.set(path, { path: relative(settings.cwd, path), sha256: digest(source), bytes: Buffer.byteLength(source), observation: 'Vite transform input; not proof of raw Node module execution' })
    }
    return null
  },
}] }
const options = {
  root: settings.cwd,
  config: settings.config,
  watch: false,
  update: false,
  cache: false,
  fileParallelism: false,
  maxWorkers: 1,
  reporters: [reporter],
  server: { deps: { external: settings.artifactPaths.map(path => new RegExp(`^${RegExp.escape(path)}$`)) } },
}
try { if (settings.collectOnly) {
  process.env.TEST = 'true'
  process.env.VITEST = 'true'
  process.env.NODE_ENV ??= 'test'
  const context = await createVitest('test', options, overrides)
  try {
    const { testModules, unhandledErrors } = await context.collect()
    reporter.onTestRunEnd(testModules, unhandledErrors, 'collected')
  } finally { await context.close() }
} else {
  const context = await startVitest('test', [], options, overrides)
  await context.close()
} } finally { writeReceipt(settings.routes, { kind: 'vitest-forbidden-source-observations', modules: [...forbiddenModules.values()].sort((a, b) => a.path.localeCompare(b.path)) }) }
