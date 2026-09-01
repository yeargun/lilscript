#!/usr/bin/env node
// Check that what a port ships is no less compact than what the compiler wrote.
//
// Three times now the whole of a port's loss has been a tool sitting between the
// compiler and the artifact, quietly re-printing the code:
//
//   006  rehypelil's build bundled without minifyWhitespace          2517 Brotli
//   028  the size harness minified only the official lane           10634 Brotli
//   030  micromarklil's build re-printed `!0` as `true`               229 Brotli
//
// Every one produced *correct* output, so nothing downstream complained -- it was
// simply bigger, and only a size comparison nobody was making would have caught
// it. This makes that comparison. For each port it reads the compiler's own
// `dist/*.raw.js` and the artifact the port ships, and reports where the shipped
// one has lost compact spellings or grown out of proportion.
//
//   node finer/tools/shipped-vs-compiled.mjs
//
// Exits non-zero if any port ships a less compact artifact than it compiled.
import {readFileSync, existsSync, readdirSync} from 'node:fs'
import {basename, dirname, join, resolve} from 'node:path'
import {fileURLToPath} from 'node:url'

const siblings = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..', '..')

// Spellings a minifier chooses and a plain re-print throws away. Counting them in
// both files turns "the bundler normalised our output" into a number.
const COMPACT = [
  ['!0', /!0/g],
  ['!1', /!1/g],
  ['void 0', /void 0/g]
]
const EXPANDED = [
  ['true', /\btrue\b/g],
  ['false', /\bfalse\b/g],
  ['undefined', /\bundefined\b/g]
]

const count = (text, pattern) => (text.match(pattern) ?? []).length
const findings = []

for (const entry of readdirSync(siblings, {withFileTypes: true})) {
  if (!entry.isDirectory() || !entry.name.includes('lil')) continue
  const dist = join(siblings, entry.name, 'dist')
  if (!existsSync(dist)) continue
  const files = readdirSync(dist)
  const raws = files.filter((name) => name.endsWith('.raw.js'))
  if (raws.length === 0) continue

  for (const raw of raws) {
    // The shipped sibling of `x.raw.js` is `x.esm.js`: same stem, bundled.
    const stem = raw.slice(0, -'.raw.js'.length)
    const shipped = `${stem}.esm.js`
    if (!files.includes(shipped)) continue
    const before = readFileSync(join(dist, raw), 'utf8')
    const after = readFileSync(join(dist, shipped), 'utf8')

    for (const [name, pattern] of COMPACT) {
      const had = count(before, pattern)
      const kept = count(after, pattern)
      // A bundle legitimately grows by pulling in dependencies, so only a
      // *complete* loss of a spelling the compiler used is reported -- that is
      // normalisation, not inlining.
      if (had > 0 && kept === 0) {
        findings.push(`${entry.name}: ${shipped} lost every \`${name}\` (${had} in ${raw})`)
      }
    }
    // Character encoding, not spelling: a bundler that defaults to an ASCII-safe
    // charset re-prints every literal non-ASCII character as a `\uXXXX` escape --
    // six ASCII bytes where the literal is two or three UTF-8 ones. On micromarklil
    // that was 2304 characters and 7021 raw bytes (034). It compresses away to
    // almost nothing, so a Brotli comparison barely sees it; only counting does.
    const literalsBefore = count(before, /[^\x00-\x7F]/g)
    const escapesAfter = count(after, /\\u[0-9A-Fa-f]{4}/g) + count(after, /\\x[0-9A-Fa-f]{2}/g)
    if (literalsBefore > 50 && count(after, /[^\x00-\x7F]/g) === 0 && escapesAfter > 50) {
      findings.push(
        `${entry.name}: ${shipped} escaped all ${literalsBefore} literal non-ASCII characters ` +
          `(${escapesAfter} escapes) — set the bundler's charset to utf8`
      )
    }

    for (const [name, pattern] of EXPANDED) {
      const gained = count(after, pattern) - count(before, pattern)
      if (gained > 20) {
        findings.push(`${entry.name}: ${shipped} gained ${gained} \`${name}\` the compiler did not emit`)
      }
    }
  }
}

for (const finding of findings) console.error(`less compact than compiled: ${finding}`)
console.log(
  findings.length === 0
    ? 'every port ships what it compiled, or more compact'
    : `${findings.length} port artifact(s) are less compact than the compiler's output`
)
process.exit(findings.length === 0 ? 0 : 1)
