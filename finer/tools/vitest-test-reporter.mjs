import { relative } from 'node:path'
import { writeReceipt } from './artifact-evidence.mjs'

// The final collection includes skipped and unfinished cases, not just callbacks
// for successful tests. Case names match Vitest's supported `list --json` output.
export default class VitestEvidenceReporter {
  constructor({ cwd, output }) {
    this.cwd = cwd
    this.output = output
  }

  onTestRunEnd(modules, unhandledErrors, reason) {
    const cases = [], files = [], occurrences = new Map()
    for (const module of modules) {
      const file = relative(this.cwd, module.moduleId)
      files.push({ path: file, state: module.state(), errors: [module, ...module.children.allSuites()].flatMap(suite => suite.errors().map(error => error.message)) })
      for (const test of module.children.allTests()) {
        const name = test.fullName, key = JSON.stringify([file, name])
        const occurrence = (occurrences.get(key) ?? 0) + 1
        occurrences.set(key, occurrence)
        const result = test.result()
        cases.push({
          kind: 'test-case', id: JSON.stringify([file, name, occurrence]), file, name,
          status: { passed: 'pass', failed: 'fail', skipped: 'skip', pending: 'pending' }[result.state] ?? 'unknown',
          errors: result.errors?.map(error => error.message) ?? [],
        })
      }
    }
    writeReceipt(this.output, {
      schemaVersion: 1, kind: 'vitest-case-results', reason, files, cases,
      unhandledErrors: unhandledErrors.map(error => error.message ?? String(error)),
    })
  }
}
