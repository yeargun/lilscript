import { randomUUID } from 'node:crypto'
import { mkdirSync, writeFileSync } from 'node:fs'
import { registerHooks } from 'node:module'
import { isAbsolute, join, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { digest } from './artifact-evidence.mjs'

export function matchesForbiddenSource(cwd, rules, path) {
  const inside = root => {
    const part = relative(resolve(cwd, root), path)
    return part !== '..' && !part.startsWith('../') && !isAbsolute(part)
  }
  return rules.some(rule => inside(rule.root) && !(rule.except ?? []).some(inside))
}

export function observeVitestSources({ cwd, rules, directory }) {
  mkdirSync(directory, { recursive: true })
  return registerHooks({ load(url, context, nextLoad) {
    const result = nextLoad(url, context)
    if (url.startsWith('file:') && matchesForbiddenSource(cwd, rules, fileURLToPath(url))) {
      const source = result.source
      const bytes = source == null ? null : typeof source === 'string' ? Buffer.from(source) : ArrayBuffer.isView(source) ? Buffer.from(source.buffer, source.byteOffset, source.byteLength) : Buffer.from(source)
      const record = { path: relative(cwd, fileURLToPath(url)), sha256: bytes && digest(bytes), bytes: bytes?.length ?? null, observation: 'Native Node module load', pid: process.pid }
      writeFileSync(join(directory, `${process.pid}-${randomUUID()}.json`), JSON.stringify(record))
    }
    return result
  } })
}
