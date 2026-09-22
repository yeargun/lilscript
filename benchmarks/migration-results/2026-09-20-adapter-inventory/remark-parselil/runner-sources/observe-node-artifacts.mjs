// Passive observation at Node's supported synchronous loader boundary. Return
// the downstream result unchanged, including CommonJS source and behavior.
import { randomUUID } from "node:crypto"
import { mkdirSync, writeFileSync } from "node:fs"
import * as module from "node:module"
import { join, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { digest } from "./artifact-evidence.mjs"

export function observeNodeArtifacts({ paths, directory }) {
  if (typeof module.registerHooks !== "function") throw new Error("production artifact observation requires Node registerHooks (Node 22.15 or newer)")
  const watched = new Set(paths.map((path) => resolve(path)))
  mkdirSync(directory, { recursive: true })
  return module.registerHooks({
    load(url, context, nextLoad) {
      const result = nextLoad(url, context)
      if (!url.startsWith("file:")) return result
      const path = fileURLToPath(url)
      if (!watched.has(path)) return result
      const source = result.source
      if (source == null) throw new Error(`Node supplied no observable source for required artifact: ${path}`)
      const bytes = typeof source === "string" ? Buffer.from(source) : ArrayBuffer.isView(source) ? Buffer.from(source.buffer, source.byteOffset, source.byteLength) : Buffer.from(source)
      const receipt = { schemaVersion: 1, kind: "node-module-load", path, url, format: result.format, sha256: digest(bytes), bytes: bytes.length, pid: process.pid }
      writeFileSync(join(directory, `${process.pid}-${randomUUID()}.json`), JSON.stringify(receipt))
      return result
    },
  })
}
