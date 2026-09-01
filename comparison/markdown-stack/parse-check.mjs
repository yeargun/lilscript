// Parse every artifact the manifest declares, and report the ones a JavaScript parser rejects.
//
// The size harness bundles each port before it measures anything, so a port that emits
// syntactically invalid JavaScript surfaces as an esbuild stack trace from `--measure` rather
// than as a build failure -- and a port whose build already failed keeps serving its last good
// artifact, which hides the breakage entirely. This runs the parser directly over every declared
// artifact so that failure is named instead of inferred.
//
//   node comparison/markdown-stack/parse-check.mjs
//
// Exits non-zero if any artifact fails to parse.
import esbuild from 'esbuild'
import {readFileSync, existsSync} from 'node:fs'
import {dirname, join, resolve} from 'node:path'
import {fileURLToPath} from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
const repository = resolve(here, '..', '..')
const manifest = JSON.parse(readFileSync(join(here, 'manifest.json'), 'utf8'))

let parsed = 0
const failures = []
for (const port of manifest.ports) {
  const root = resolve(repository, port.port.defaultSibling)
  for (const artifact of port.port.artifacts ?? []) {
    const path = join(root, artifact)
    if (!existsSync(path)) {
      failures.push(`${port.id} ${artifact}: missing`)
      continue
    }
    try {
      await esbuild.transform(readFileSync(path, 'utf8'), {
        loader: 'js',
        format: artifact.endsWith('.cjs') ? 'cjs' : undefined
      })
      parsed += 1
    } catch (error) {
      const first = error.errors?.[0]
      const at = first?.location ? `${first.location.line}:${first.location.column}` : '?'
      failures.push(`${port.id} ${artifact}:${at}: ${first?.text ?? error.message}`)
    }
  }
}

for (const failure of failures) console.error(`unparseable ${failure}`)
console.log(`${parsed} artifacts parse, ${failures.length} unparseable`)
process.exit(failures.length === 0 ? 0 : 1)
