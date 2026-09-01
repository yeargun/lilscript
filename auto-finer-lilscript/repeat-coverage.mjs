#!/usr/bin/env node
// Report how much of an artifact sits inside long back-references.
//
// Brotli pays for a byte once and then charges only a reference for every later
// copy inside its window, so two artifacts of identical raw size can compress
// very differently. Identifier-level statistics miss this entirely: an artifact
// can reuse names better than its competitor and still lose, because the repeats
// Brotli actually feeds on are whole phrases. This measures that directly, as a
// greedy LZ77 proxy -- the share of bytes covered by a match of at least N to
// something already seen.
//
//   node auto-finer-lilscript/repeat-coverage.mjs a.js b.js
import { readFileSync } from "node:fs"

function coverage(text, minimum) {
  const seen = new Map()
  let covered = 0
  let at = 0
  while (at < text.length) {
    const key = text.slice(at, at + minimum)
    const previous = seen.get(key)
    let length = 0
    if (previous !== undefined) {
      length = minimum
      while (at + length < text.length && text[previous + length] === text[at + length] && length < 4096) {
        length += 1
      }
    }
    if (!seen.has(key)) seen.set(key, at)
    if (length >= minimum) {
      covered += length
      for (let step = 1; step < length; step += 1) {
        const inner = text.slice(at + step, at + step + minimum)
        if (!seen.has(inner)) seen.set(inner, at + step)
      }
      at += length
    } else {
      at += 1
    }
  }
  return covered / text.length
}

const files = process.argv.slice(2)
if (files.length === 0) {
  console.error("usage: repeat-coverage.mjs <file...>")
  process.exit(2)
}
const percent = (value) => `${(value * 100).toFixed(1)}%`
console.log(`${"file".padEnd(40)}${"bytes".padStart(9)}${">=8".padStart(8)}${">=16".padStart(8)}${">=32".padStart(8)}`)
for (const file of files) {
  const text = readFileSync(file, "utf8")
  console.log(
    file.slice(-40).padEnd(40) + String(text.length).padStart(9) +
    percent(coverage(text, 8)).padStart(8) + percent(coverage(text, 16)).padStart(8) +
    percent(coverage(text, 32)).padStart(8)
  )
}
