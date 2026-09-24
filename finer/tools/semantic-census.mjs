// How much of the language the new semantic backend carries, measured.
//
// This is the migration's own progress metric. Every `tests/cases/*.lil`
// program is compiled for JavaScript and for C, the
// result is executed, and its output is compared with the case's `.out` file,
// which was written against the legacy backend and is therefore an independent
// expectation for the new one.
//
// Each row lands in exactly one state, and the states are not equally good:
//
//   passed            emitted, ran, and printed exactly the expected output
//   diagnosed         the compiler refused the program with a diagnostic —
//                     unsupported, but honestly so (a negative case that holds)
//   wrong-output      emitted and ran, but printed something else — a
//                     miscompile, the one state that must never be tolerated
//   execution-failed  emitted, but crashed or timed out when run
//   native-build      C was emitted but the C compiler rejected it
//
// Derived from benchmarks/migration-results/2026-09-19-artifact-service/
// support-census.mjs, which pinned a `/tmp` binary that no longer exists.

import { spawnSync } from "node:child_process"
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { fileIdentity, fingerprint } from "./artifact-evidence.mjs"
import { preserveBinary } from "./preserved-binaries.mjs"
import { routeArgs } from "./route-args.mjs"

const toolsDirectory = dirname(fileURLToPath(import.meta.url))
const root = resolve(toolsDirectory, "../..")
const identify = path => ({ path, ...fileIdentity(path) })

// `js` is a classic script (sloppy frames, global scope); `js-module` is an ES
// module (strict, module scope). The corpus programs are scripts, but both are
// supported delivery modes and they refuse different things.
export const TARGETS = ["js", "js-module", "c"]
const extension = target => ({ js: "js", "js-module": "mjs", c: "c" })[target]

export function classify({ compile, nativeBuild, execution, expected }) {
  if (compile.status !== 0) return "diagnosed"
  if (nativeBuild && nativeBuild.status !== 0) return "native-build"
  if (!execution || execution.status !== 0 || execution.signal) return "execution-failed"
  return execution.stdout === expected ? "passed" : "wrong-output"
}

export function runCensus({ compiler, directory, config = join(root, "tests/config/no-optimization.toml"), timeoutMs = 30_000, cases } = {}) {
  const preserved = preserveBinary(compiler)
  mkdirSync(directory, { recursive: true })
  const node = process.execPath
  const env = { ...process.env, PATH: `${dirname(node)}:/usr/local/bin:/usr/bin:/bin`, NODE_OPTIONS: "" }
  const run = (label, command, args) => {
    const started = performance.now()
    const result = spawnSync(command, args, { env, cwd: root, encoding: "utf8", timeout: timeoutMs, maxBuffer: 16 * 1024 * 1024 })
    writeFileSync(join(directory, `${label}.stdout`), result.stdout ?? "")
    writeFileSync(join(directory, `${label}.stderr`), result.stderr ?? "")
    return { status: result.status, signal: result.signal, error: result.error?.message ?? null, ms: Number((performance.now() - started).toFixed(1)), stdout: result.stdout ?? "", stderrHead: (result.stderr ?? "").split("\n").slice(0, 3).join("\n") }
  }
  const sources = cases ?? readdirSync(join(root, "tests/cases")).filter(name => name.endsWith(".lil")).sort()
  const receipt = {
    schema: 1, kind: "semantic-backend-language-census", started: new Date().toISOString(),
    backend: "semantic", mode: "development", compiler: preserved, node: identify(node), config: identify(config),
    scope: "the existing tests/cases runtime corpus; not a complete feature inventory, not a library baseline",
    rows: [],
  }
  for (const name of sources) {
    const source = join(root, "tests/cases", name)
    const stem = name.slice(0, -4)
    const expectedPath = source.replace(/\.lil$/, ".out")
    const expected = readFileSync(expectedPath, "utf8")
    for (const target of TARGETS) {
      const label = `${stem}-${target}`
      const destination = join(directory, `${label}.${extension(target)}`)
      const compile = run(`${label}-compile`, preserved.path, [source, "--config", config, ...routeArgs(preserved.path), "--mode", "development", "--target", target, "--output", destination])
      let nativeBuild = null, execution = null
      if (compile.status === 0) {
        let executable = destination
        if (target === "c") {
          executable = join(directory, `${label}.exe`)
          nativeBuild = run(`${label}-cc`, "/usr/bin/cc", ["-std=c11", "-O1", "-fno-fast-math", "-ffp-contract=off", destination, "-lm", "-o", executable])
        }
        if (!nativeBuild || nativeBuild.status === 0) execution = run(`${label}-run`, target === "c" ? executable : node, target === "c" ? [] : [destination])
      }
      const state = classify({ compile, nativeBuild, execution, expected })
      receipt.rows.push({
        case: stem, target, state, source: identify(source), expected: identify(expectedPath),
        compileMs: compile.ms, diagnostic: state === "diagnosed" ? compile.stderrHead : null,
        artifact: compile.status === 0 ? identify(destination) : null,
        mismatch: state === "wrong-output" ? { expectedHead: expected.slice(0, 200), actualHead: execution.stdout.slice(0, 200) } : null,
      })
    }
  }
  receipt.summary = Object.fromEntries(TARGETS.map(target => {
    const rows = receipt.rows.filter(row => row.target === target)
    return [target, Object.fromEntries(["passed", "diagnosed", "wrong-output", "execution-failed", "native-build"].map(state => [state, rows.filter(row => row.state === state).length]))]
  }))
  // A census passes when nothing is miscompiled. Unsupported programs are
  // expected mid-migration; a wrong answer is not.
  receipt.miscompiles = receipt.rows.filter(row => row.state === "wrong-output").map(row => `${row.case}/${row.target}`)
  receipt.passed = receipt.miscompiles.length === 0
  receipt.completed = new Date().toISOString()
  receipt.fingerprint = fingerprint({ ...receipt, started: null, completed: null })
  writeFileSync(join(directory, "receipt.json"), JSON.stringify(receipt, null, 2) + "\n")
  return receipt
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const argv = process.argv.slice(2)
  const flag = name => { const at = argv.indexOf(name); return at < 0 ? null : argv[at + 1] }
  const compiler = resolve(flag("--compiler") ?? join(root, "target/release/lilscript"))
  const directory = resolve(flag("--out") ?? join(root, `benchmarks/migration-results/${new Date().toISOString().slice(0, 10)}-semantic-census`))
  const receipt = runCensus({ compiler, directory })
  for (const target of TARGETS) {
    const counts = receipt.summary[target]
    console.log(`${target.padEnd(9)} passed ${String(counts.passed).padStart(3)}  diagnosed ${String(counts.diagnosed).padStart(3)}  wrong-output ${String(counts["wrong-output"]).padStart(2)}  execution-failed ${String(counts["execution-failed"]).padStart(2)}  native-build ${String(counts["native-build"]).padStart(2)}`)
  }
  console.log(receipt.miscompiles.length ? `MISCOMPILES: ${receipt.miscompiles.join(", ")}` : "no miscompiles: every emitted program printed exactly its expected output")
  if (!receipt.passed) process.exitCode = 1
}
