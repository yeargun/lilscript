// Content-addressed copies of the binaries a receipt depends on.
//
// `target/release/lilscript` is a build output: every `cargo build` rewrites
// it. A receipt that pins that path stops being checkable the moment the
// compiler is rebuilt, and during a migration it is rebuilt constantly — so
// pinning the mutable path meant each rebuild silently voided every earlier
// piece of evidence. Instead each run copies the binary it actually used into
// a store keyed by its own SHA-256, marks the copy read-only, and records that
// copy's path. A later rebuild changes `target/`, not the evidence.
//
// The store lives outside the repository (the binaries are large) and outside
// `/tmp` (which has already been wiped once under two recorded workspaces).

import assert from "node:assert/strict"
import { chmodSync, copyFileSync, existsSync, mkdirSync, renameSync } from "node:fs"
import { homedir } from "node:os"
import { basename, join } from "node:path"
import { fileIdentity } from "./artifact-evidence.mjs"

export const EVIDENCE_STORE = process.env.LILSCRIPT_EVIDENCE_STORE ?? join(homedir(), "lilscript-evidence")

/**
 * Preserve `path` by content and return the preserved identity.
 *
 * Idempotent: a binary already in the store is verified, not copied again.
 */
export function preserveBinary(path) {
  assert(existsSync(path), `binary to preserve does not exist: ${path}`)
  const identity = fileIdentity(path)
  const directory = join(EVIDENCE_STORE, "binaries", identity.sha256)
  const target = join(directory, basename(path))
  if (!existsSync(target)) {
    mkdirSync(directory, { recursive: true })
    // Copy under a temporary name and rename, so a concurrent reader never
    // sees a half-written binary at the content-addressed path.
    const staging = `${target}.partial-${process.pid}`
    copyFileSync(path, staging)
    chmodSync(staging, 0o555)
    renameSync(staging, target)
  }
  const preserved = fileIdentity(target)
  assert.equal(preserved.sha256, identity.sha256, `preserved copy of ${path} does not match its source`)
  return { path: target, ...preserved, preservedFrom: path }
}
