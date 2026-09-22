// A receipt for one command: what ran, on which pinned inputs, and whether it
// passed.
//
// Milestones 003 onwards close on test suites — cargo tests, node tests, the
// CLI — rather than on library builds. Their evidence needs the same properties
// as a library baseline: the exact command, the identity of every input it
// depended on (a compiler binary is preserved by content so a rebuild cannot
// void the receipt), the exit status, and the output kept for review.
//
//   node finer/tools/command-receipt.mjs --out <dir> --input <file> [--input ...]
//        [--binary <file>] [--env NAME=VALUE ...] -- <command> [args...]
//
// `--binary` preserves the file in the content-addressed store and exposes it
// to the command as LILSCRIPT_COMPILER; `--input` pins a file by identity;
// `--env` sets a variable the command depends on (a toolchain path, say) and
// records it, and any executable it names is pinned like an input.

import { spawnSync } from "node:child_process"
import { existsSync, mkdirSync, statSync, writeFileSync } from "node:fs"
import { isAbsolute, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { digest, fileIdentity, fingerprint } from "./artifact-evidence.mjs"
import { preserveBinary } from "./preserved-binaries.mjs"

export function runCommandReceipt({ command, args, directory, inputs = [], binary = null, cwd = process.cwd(), env = {}, timeoutMs = 3_600_000 }) {
  mkdirSync(directory, { recursive: true })
  const preserved = binary ? preserveBinary(binary) : null
  const environment = Object.entries(env)
  const toolPaths = environment.map(([, value]) => value).filter(value => isAbsolute(value) && existsSync(value) && statSync(value).isFile())
  const pinned = [...inputs, ...toolPaths].map(path => ({ path: resolve(path), ...fileIdentity(resolve(path)) }))
  const started = new Date().toISOString()
  const clock = performance.now()
  const result = spawnSync(command, args, {
    cwd, encoding: "utf8", timeout: timeoutMs, maxBuffer: 1 << 28,
    env: { ...process.env, ...env, ...(preserved ? { LILSCRIPT_COMPILER: preserved.path } : {}) },
  })
  const wallSeconds = Number(((performance.now() - clock) / 1000).toFixed(3))
  writeFileSync(join(directory, "stdout.txt"), result.stdout ?? "")
  writeFileSync(join(directory, "stderr.txt"), result.stderr ?? "")
  const receipt = {
    schema: 1, kind: "command-receipt", started, completed: new Date().toISOString(), wallSeconds,
    command, args, cwd,
    environment: Object.fromEntries(environment),
    compiler: preserved,
    inputs: { files: [...pinned, ...(preserved ? [{ path: preserved.path, sha256: preserved.sha256, bytes: preserved.bytes }] : [])] },
    status: result.status, signal: result.signal, error: result.error?.message ?? null,
    stdout: { sha256: digest(Buffer.from(result.stdout ?? "")), bytes: Buffer.byteLength(result.stdout ?? "") },
    stderr: { sha256: digest(Buffer.from(result.stderr ?? "")), bytes: Buffer.byteLength(result.stderr ?? "") },
    passed: result.status === 0 && !result.signal && !result.error,
  }
  receipt.fingerprint = fingerprint({ ...receipt, started: null, completed: null, wallSeconds: null })
  writeFileSync(join(directory, "receipt.json"), JSON.stringify(receipt, null, 2) + "\n")
  return receipt
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const argv = process.argv.slice(2)
  const split = argv.indexOf("--")
  if (split < 0) { console.error("usage: command-receipt.mjs --out <dir> [--input f]... [--binary f] [--env NAME=VALUE]... -- <command> [args...]"); process.exit(2) }
  const options = argv.slice(0, split), [command, ...args] = argv.slice(split + 1)
  const values = name => options.flatMap((value, index) => (value === name ? [options[index + 1]] : []))
  const env = Object.fromEntries(values("--env").map(pair => [pair.slice(0, pair.indexOf("=")), pair.slice(pair.indexOf("=") + 1)]))
  const receipt = runCommandReceipt({ command, args, directory: resolve(values("--out")[0]), inputs: values("--input"), binary: values("--binary")[0] ?? null, env })
  console.log(JSON.stringify({ passed: receipt.passed, status: receipt.status, wallSeconds: receipt.wallSeconds, directory: resolve(values("--out")[0]) }))
  if (!receipt.passed) process.exitCode = 1
}
