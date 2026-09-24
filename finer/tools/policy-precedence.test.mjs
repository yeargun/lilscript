// 003: consumer tests for every configuration axis, precedence and invalid
// limits, through the real command line.
//
// Each case writes a `lilscript.toml`, runs the compiler with `--print-policy`
// and reads the resolved receipt, so what is tested is the path a user takes —
// discovery, TOML parsing, command-line overrides and policy resolution — not a
// library function in isolation.

import assert from "node:assert/strict"
import { spawnSync } from "node:child_process"
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { dirname, join, resolve } from "node:path"
import test from "node:test"
import { fileURLToPath } from "node:url"

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..")
const COMPILER = process.env.LILSCRIPT_COMPILER ?? join(root, "target/release/lilscript")
const SOURCE = "export int answer() {\n  return 42;\n}\n"

function project(t, toml) {
  const directory = mkdtempSync(join(tmpdir(), "lilscript-precedence-"))
  t.after(() => rmSync(directory, { recursive: true, force: true }))
  writeFileSync(join(directory, "main.lil"), SOURCE)
  if (toml !== null) writeFileSync(join(directory, "lilscript.toml"), toml)
  return directory
}

function resolvePolicy(directory, args = [], { config = true } = {}) {
  const result = spawnSync(COMPILER, [join(directory, "main.lil"), "--target", "js-module", ...(config ? ["--config", join(directory, "lilscript.toml")] : []), ...args, "--print-policy"], { encoding: "utf8" })
  return { status: result.status, stderr: result.stderr, receipt: result.status === 0 ? JSON.parse(result.stdout) : null }
}

test("TOML resources have no effect and are reported as retired keys", t => {
  const { receipt } = resolvePolicy(project(t, "[compiler.resources]\nthreads = 8\ncodec_workers = 4\n"))
  assert.equal(receipt.execution.threads, null)
  assert.equal(receipt.execution.codec_workers, null)
  assert.equal(receipt.warnings.length, 1)
  assert.match(receipt.warnings[0], /^`compiler\.resources` has no effect in this compiler: .*; remove it$/)
})

test("command-line resource flags are recorded outside the policy", t => {
  const { receipt } = resolvePolicy(project(t, "[javascript]\ncost_model = \"brotli\"\n"), ["--jobs", "3", "--codec-jobs", "2"])
  assert.equal(receipt.execution.threads, 3)
  assert.equal(receipt.execution.codec_workers, 2)
})

test("thread counts never enter the policy fingerprint", t => {
  const directory = project(t, "[javascript]\ncost_model = \"brotli\"\n")
  const one = resolvePolicy(directory, ["--jobs", "1"]).receipt
  const eight = resolvePolicy(directory, ["--jobs", "8"]).receipt
  assert.equal(one.fingerprint, eight.fingerprint, "threads must not change what is compiled")
})

test("the same inputs resolve to the same fingerprint", t => {
  const directory = project(t, "[javascript]\noptimization_level = 15\n")
  assert.equal(resolvePolicy(directory).receipt.fingerprint, resolvePolicy(directory).receipt.fingerprint)
})

test("development mode overrides the configured search and changes the fingerprint", t => {
  const directory = project(t, "[javascript]\ncandidate_search = \"production\"\n")
  const production = resolvePolicy(directory).receipt
  const development = resolvePolicy(directory, ["--mode", "development"]).receipt
  assert.equal(development.policy.objective.optional_codec_probes, 0)
  assert.equal(development.policy.objective.optional_alternatives, 0)
  assert.notEqual(production.fingerprint, development.fingerprint)
  assert.equal(development.execution.mode, "Development")
})

test("effort follows optimization_level and appears in the receipt", t => {
  assert.equal(resolvePolicy(project(t, "[javascript]\noptimization_level = 8\n")).receipt.policy.effort, 8)
  assert.equal(resolvePolicy(project(t, "[javascript]\noptimization_level = 16\n")).receipt.policy.effort, 16)
})

test("an optimization level above 16 is refused", t => {
  const { status, stderr } = resolvePolicy(project(t, "[javascript]\noptimization_level = 17\n"))
  assert.notEqual(status, 0)
  assert.match(stderr, /between 0 and 16/)
})

test("an explicit tactic permission reaches the resolved policy", t => {
  const off = resolvePolicy(project(t, "[policy.tactics]\nproperty-mangling = \"off\"\n"))
  if (off.status !== 0) {
    // The tactic identifier spelling is owned by the policy registry; report
    // it rather than silently passing.
    assert.fail(`tactic permission was refused: ${off.stderr.split("\n")[0]}`)
  }
  const tactic = off.receipt.policy.tactics.find(row => row.id === "property-mangling" || row.id === "PropertyMangling")
  assert(tactic, `receipt lists tactics: ${off.receipt.policy.tactics.map(row => row.id).join(", ")}`)
  assert.match(JSON.stringify(tactic.state), /off|Off/)
})

test("an unknown key is an error, not a silently ignored setting", t => {
  const { status, stderr } = resolvePolicy(project(t, "[javascript]\nno_such_setting = true\n"))
  assert.notEqual(status, 0)
  assert.match(stderr, /unknown field `no_such_setting`/)
})

test("an invalid resource flag is refused", t => {
  const flag = spawnSync(COMPILER, [join(project(t, null), "main.lil"), "--jobs", "0", "--print-policy"], { encoding: "utf8" })
  assert.notEqual(flag.status, 0, "--jobs 0 must be refused by the command line")
})

test("a sibling-line optimizer knob is a retired key: accepted, reported, and without effect", t => {
  const accepted = resolvePolicy(project(t, "[javascript]\nname_ordering = \"idiom-converged\"\nterminal_cleanup_chain = true\n")).receipt
  assert.equal(accepted.warnings.length, 2)
  assert.match(accepted.warnings[0], /javascript\.name_ordering/)
  const baseline = resolvePolicy(project(t, "")).receipt
  assert.equal(accepted.fingerprint, baseline.fingerprint)
})

test("the route selector, runtime priorities, constraints and the positional ABI are refused", t => {
  for (const [toml, message] of [
    ["[compiler]\nbackend = \"semantic\"\n", /there is one compiler; remove \[compiler\] backend/],
    ["[javascript]\npriority = \"balanced\"\n", /size is the objective/],
    ["[policy.constraints]\nmax_startup_work = 1\n", /runtime estimators/],
    ["[javascript]\npublic_aggregate_abi = \"positional\"\n", /\(D2\)/],
    ["[optimization]\nfor_of_specialize_family = 8\n", /for_of_specialize_family = 8/],
  ]) {
    const { status, stderr } = resolvePolicy(project(t, toml))
    assert.notEqual(status, 0, toml)
    assert.match(stderr, message)
  }
})

test("a lilscript.toml beside the input is discovered without --config", t => {
  const directory = project(t, "[javascript]\noptimization_level = 9\n")
  const { receipt } = resolvePolicy(directory, [], { config: false })
  assert.equal(receipt.policy.effort, 9)
  assert.equal(receipt.config, join(directory, "lilscript.toml"))
})

test("an explicit --config wins over the discovered file", t => {
  const directory = project(t, "[javascript]\noptimization_level = 9\n")
  const other = join(directory, "other")
  mkdirSync(other)
  writeFileSync(join(other, "explicit.toml"), "[javascript]\noptimization_level = 4\n")
  const result = spawnSync(COMPILER, [join(directory, "main.lil"), "--target", "js-module", "--config", join(other, "explicit.toml"), "--print-policy"], { encoding: "utf8" })
  assert.equal(result.status, 0, result.stderr)
  assert.equal(JSON.parse(result.stdout).policy.effort, 4)
})
