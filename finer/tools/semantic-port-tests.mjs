#!/usr/bin/env node
// SUPERSEDED by scripts/ports.mjs (plan task M2.6), which also pins the compiler
// copy by digest, never passes --backend, switches objectives and diffs the
// failing-test set against tests/ports/expected-failures.json. See
// docs/testing.md. Kept for history; do not extend.
//
// Runs maintained ports' own test suites against a compiler's semantic route.
//
// Each port is copied to a fresh workspace (its node_modules linked, not
// copied), the recorded source migration in finer/port-migrations/<port>.patch
// is applied, `dist/` is removed, and the port is built and tested with
// LILSCRIPT_COMPILER pointing at a wrapper around the compiler (which adds
// `--backend semantic` for a binary from before the migration).
// The report pins the compiler digest, each port's git revision and dirty
// state, and each patch digest, so a receipt of this command is replayable.
//
//   node finer/tools/semantic-port-tests.mjs --compiler <lilscript> \
//     --ports micromarklil,zodlil --out report.json [--ports-root ~]
import { createHash } from "node:crypto"
import {
  chmodSync,
  cpSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs"
import { homedir, tmpdir } from "node:os"
import { basename, dirname, join, resolve } from "node:path"
import { spawnSync } from "node:child_process"
import { fileURLToPath } from "node:url"
import { routeArgs } from "./route-args.mjs"

const repository = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..")
const args = new Map()
for (let index = 2; index < process.argv.length; index += 2) {
  args.set(process.argv[index].replace(/^--/, ""), process.argv[index + 1])
}
const compiler = resolve(args.get("compiler") ?? join(repository, "target", "release", "lilscript"))
const ports = (args.get("ports") ?? "").split(",").filter(Boolean)
const portsRoot = resolve(args.get("ports-root") ?? homedir())
const out = args.get("out")
if (!ports.length || !out) {
  console.error("usage: semantic-port-tests.mjs --compiler <path> --ports a,b --out <report.json>")
  process.exit(2)
}

const digest = (path) => createHash("sha256").update(readFileSync(path)).digest("hex")
const run = (command, argv, options) =>
  spawnSync(command, argv, { encoding: "utf8", maxBuffer: 256 << 20, ...options })

// Ports mix runners, so every recognized total is summed: node:test TAP
// (`# tests N`) and spec (`ℹ tests N`) totals, mocha (`N passing`,
// `N failing`), differential scripts (`N/M fixtures passed`), jest
// (`Tests: … N passed, M total`), vitest (`Tests  N passed (M)`) and
// upstream harnesses (`pass: N  fail: M`).
function summary(text) {
  let tests = 0
  let pass = 0
  let fail = 0
  let seen = false
  const add = (total, passed, failed) => {
    tests += total
    pass += passed
    fail += failed
    seen = true
  }
  const sum = (pattern) => [...text.matchAll(pattern)].reduce((total, match) => total + Number(match[1]), 0)
  const count = (pattern) => [...text.matchAll(pattern)].length
  if (count(/^(?:#|ℹ) tests \d+$/gm)) {
    add(sum(/^(?:#|ℹ) tests (\d+)$/gm), sum(/^(?:#|ℹ) pass (\d+)$/gm), sum(/^(?:#|ℹ) fail (\d+)$/gm))
  }
  if (count(/^\s*\d+ passing\b/gm)) {
    const passed = sum(/^\s*(\d+) passing\b/gm)
    const failed = sum(/^\s*(\d+) failing\b/gm)
    add(passed + failed, passed, failed)
  }
  for (const match of text.matchAll(/(\d+)\/(\d+) fixtures passed/g)) {
    add(Number(match[2]), Number(match[1]), Number(match[2]) - Number(match[1]))
  }
  const jest = text.match(/^Tests:\s+(?:(\d+) failed, )?(?:(\d+) skipped, )?(\d+) passed, (\d+) total/m)
  if (jest) add(Number(jest[4]), Number(jest[3]), Number(jest[1] ?? 0))
  const vitest = text.match(/Tests\s+(?:(\d+) failed \| )?(\d+) passed \((\d+)\)/)
  if (vitest) add(Number(vitest[3]), Number(vitest[2]), Number(vitest[1] ?? 0))
  for (const match of text.matchAll(/\bpass:?\s+(\d+)\s+fail:?\s+(\d+)/g)) {
    add(Number(match[1]) + Number(match[2]), Number(match[1]), Number(match[2]))
  }
  const failures = [
    ...text.matchAll(/^\s*not ok \d+ - (.*)$/gm),
    ...text.matchAll(/^\s*✖ (.*?)(?: \([\d.]+m?s\))?$/gm),
    ...text.matchAll(/^\s+● (.*)$/gm),
  ]
    .map((match) => match[1])
    .filter((name, index, all) => name !== "failing tests:" && all.indexOf(name) === index)
    .slice(0, 20)
  return seen ? { tests, pass, fail, failures } : { tests: null, pass: null, fail: null, failures }
}

const report = {
  schema: 1,
  compiler: { path: compiler, sha256: digest(compiler) },
  node: process.version,
  ports: [],
}
for (const port of ports) {
  const source = join(portsRoot, port)
  const head = run("git", ["-C", source, "rev-parse", "HEAD"]).stdout.trim()
  const dirty = run("git", ["-C", source, "status", "--porcelain"]).stdout.trim() !== ""
  // Ports import sibling ports' sources (`../hast-util-to-htmllil/src`), so
  // the workspace sits in a fresh parent beside links to the other ports.
  const parent = mkdtempSync(join(tmpdir(), `semantic-${port}-`))
  for (const sibling of readdirSync(portsRoot)) {
    if (sibling !== port && sibling.endsWith("lil")) {
      symlinkSync(join(portsRoot, sibling), join(parent, sibling))
    }
  }
  // Ports also find this repository beside them (`../lilscript`).
  symlinkSync(repository, join(parent, "lilscript"))
  const workspace = join(parent, port)
  const skip = new Set(["node_modules", ".git", "_site", "dist"])
  cpSync(source, workspace, {
    recursive: true,
    filter: (path) => path === source || !skip.has(basename(path)) || dirname(path) !== source,
  })
  if (existsSync(join(source, "node_modules"))) {
    symlinkSync(join(source, "node_modules"), join(workspace, "node_modules"))
  }
  const entry = { port, head, dirty, patch: null, build: null, test: null }
  const patch = join(repository, "finer", "port-migrations", `${port}.patch`)
  if (existsSync(patch)) {
    entry.patch = { path: `finer/port-migrations/${port}.patch`, sha256: digest(patch) }
    const applied = run("patch", ["-p1", "--forward", "-d", workspace, "-i", patch])
    if (applied.status !== 0) {
      entry.patch.error = (applied.stdout + applied.stderr).slice(0, 2000)
      report.ports.push(entry)
      continue
    }
  }
  const wrapper = join(workspace, ".lilscript-semantic")
  const route = routeArgs(compiler).map(arg => `${arg} `).join("")
  writeFileSync(wrapper, `#!/bin/sh\nexec ${JSON.stringify(compiler)} ${route}"$@"\n`)
  chmodSync(wrapper, 0o755)
  const env = { ...process.env, LILSCRIPT_COMPILER: wrapper }
  // A port without a package (the probe) is a differential build: it
  // compiles, runs and compares its own output, so that run is its test.
  if (!existsSync(join(workspace, "package.json"))) {
    const started = Date.now()
    const probe = run("node", ["scripts/build.mjs", "--compile"], { cwd: workspace, env })
    const passed = probe.status === 0
    entry.test = {
      status: probe.status,
      ms: Date.now() - started,
      tests: 1,
      pass: passed ? 1 : 0,
      fail: passed ? 0 : 1,
      failures: passed ? [] : [(probe.stdout + probe.stderr).slice(-400)],
    }
    report.ports.push(entry)
    rmSync(parent, { recursive: true, force: true })
    writeFileSync(out, JSON.stringify(report, null, 2) + "\n")
    console.log(`${port}: differential build ${passed ? "passed" : "failed"}`)
    continue
  }
  const scripts = JSON.parse(readFileSync(join(workspace, "package.json"), "utf8")).scripts ?? {}
  if (scripts.build) {
    const started = Date.now()
    // Some builds reuse existing dist output unless asked to compile.
    const script = join(workspace, "scripts", "build.mjs")
    const compiles = existsSync(script) && readFileSync(script, "utf8").includes("--compile")
    const build = compiles && !scripts.build.includes("--compile")
      ? run("node", ["scripts/build.mjs", "--compile"], { cwd: workspace, env })
      : run("npm", ["run", "build"], { cwd: workspace, env })
    entry.build = {
      status: build.status,
      ms: Date.now() - started,
      errors: (build.stdout + build.stderr)
        .split("\n")
        .filter((line) => /error|compiler \(/i.test(line) && !/^warning/.test(line))
        .slice(0, 10),
    }
  }
  const started = Date.now()
  const test = run("npm", ["test"], { cwd: workspace, env })
  entry.test = { status: test.status, ms: Date.now() - started, ...summary(test.stdout + test.stderr) }
  report.ports.push(entry)
  rmSync(parent, { recursive: true, force: true })
  writeFileSync(out, JSON.stringify(report, null, 2) + "\n")
  console.log(
    `${port}: ${entry.test.pass}/${entry.test.tests} passed` +
      (entry.test.fail ? `, ${entry.test.fail} failed: ${entry.test.failures.slice(0, 3).join("; ")}` : ""),
  )
}
writeFileSync(out, JSON.stringify(report, null, 2) + "\n")
