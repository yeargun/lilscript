// The executable language and ABI table for 002.
//
// D2 is settled: typed internals meet JavaScript through explicitly declared
// boundaries with compatible adapters. That decides the *shape* of the answer,
// not what "compatible" means for any particular observation — so this module
// makes each obligation a case that runs.
//
// Every case states one observation a JavaScript caller can make, and is run
// twice: once against the upstream library, which is the independent oracle,
// and once against our artifact. The rule is deliberately asymmetric.
//
//   * upstream fails  -> the case is wrong. It is reported as a broken
//                        expectation, never as a discovery about our compiler.
//   * upstream passes,
//     ours fails      -> a real boundary obligation this artifact does not meet.
//   * both pass       -> the obligation holds for this artifact today.
//
// An expectation copied from our own output would only prove we are consistent
// with ourselves, which is why `expected` is never recorded from the candidate.

import assert from "node:assert/strict"
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath, pathToFileURL } from "node:url"
import { fileIdentity, fingerprint } from "./artifact-evidence.mjs"

const toolsDirectory = dirname(fileURLToPath(import.meta.url))
export const REPOSITORY_ROOT = resolve(toolsDirectory, "../..")

/** The decisions a case can belong to, and what each one governs. */
export const DECISIONS = {
  D1: "Value structs; mutation of caller storage requires explicit mutable references. Aliasing, partial mutation and evaluation order are observable.",
  D2: "Explicitly declared boundaries with compatible adapters. Identity, mutation, enumeration, descriptors, serialization, callback retention and function observations survive the boundary.",
  D3: "Results, explicit throws, argument errors, host effects and divergence are preserved. Engine-dependent resource timing is not.",
}

/**
 * Run one case against one subject module.
 *
 * `probe` receives the loaded module namespace and returns a plain,
 * JSON-comparable observation. Anything it throws is itself an observation:
 * a case about an expected error records `{ threw: ... }` rather than failing.
 */
async function observe(probe, namespace) {
  try {
    const value = await probe(namespace)
    return { ok: true, value }
  } catch (error) {
    return { ok: true, value: { threw: { name: error?.constructor?.name ?? "unknown", message: String(error?.message ?? error) } } }
  }
}

/** Load an ES module by path, with a cache-busting query so reruns are fresh. */
async function load(path) {
  return import(`${pathToFileURL(path).href}?boundary=${Date.now()}-${Math.random()}`)
}

/**
 * Run a boundary's whole case table.
 *
 * `upstream` and `candidate` are absolute paths to ES modules exposing the same
 * public API. Returns a receipt; `passed` requires that every case's oracle
 * held and every candidate observation matched it.
 */
export async function runBoundaryCases({ boundary, upstream, candidate, cases, directory, notes = [] }) {
  const started = new Date().toISOString()
  const receipt = {
    schema: 1, kind: "boundary-conformance", boundary, started, passed: false,
    decisions: DECISIONS,
    upstream: { path: upstream, ...fileIdentity(upstream) },
    candidate: { path: candidate, ...fileIdentity(candidate) },
    limitations: [
      "Each case observes exactly what it states. Passing does not generalize to arbitrary getters, proxies or reflection.",
      "The oracle is the upstream library as installed. Where upstream itself does not promise an observation, the case says so in `promised`.",
      "These are behavior observations. They carry no size, speed or delivery claim.",
      ...notes,
    ],
    cases: [],
  }
  const upstreamModule = await load(upstream)
  const candidateModule = await load(candidate)
  for (const testCase of cases) {
    assert(DECISIONS[testCase.decision], `case ${testCase.id} names an unknown decision ${testCase.decision}`)
    const oracle = await observe(testCase.probe, upstreamModule)
    const ours = await observe(testCase.probe, candidateModule)
    const oracleMatches = testCase.expected === undefined || fingerprint(oracle.value) === fingerprint(testCase.expected)
    const agree = fingerprint(oracle.value) === fingerprint(ours.value)
    receipt.cases.push({
      id: testCase.id, decision: testCase.decision, rule: testCase.rule,
      promised: testCase.promised ?? "upstream behavior only; not a documented promise",
      callers: testCase.callers ?? [],
      oracle: oracle.value, candidate: ours.value,
      oracleMatches, agree,
      status: !oracleMatches ? "broken-expectation" : agree ? "pass" : "fail",
    })
  }
  receipt.counts = {
    total: receipt.cases.length,
    pass: receipt.cases.filter(row => row.status === "pass").length,
    fail: receipt.cases.filter(row => row.status === "fail").length,
    broken: receipt.cases.filter(row => row.status === "broken-expectation").length,
  }
  receipt.passed = receipt.counts.total > 0 && receipt.counts.fail === 0 && receipt.counts.broken === 0
  receipt.completed = new Date().toISOString()
  receipt.fingerprint = fingerprint({ ...receipt, started: null, completed: null })
  if (directory) {
    mkdirSync(directory, { recursive: true })
    writeFileSync(join(directory, "receipt.json"), JSON.stringify(receipt, null, 2) + "\n")
  }
  return receipt
}

/** A compact description of one function's observable surface. */
export function functionObservations(value) {
  if (typeof value !== "function") return { kind: typeof value }
  let constructible = false
  try { Reflect.construct(value, []); constructible = true } catch { constructible = false }
  return {
    kind: "function", name: value.name, length: value.length,
    constructible, hasPrototype: Object.hasOwn(value, "prototype"),
  }
}

/** Own property descriptors, reduced to what a caller can actually observe. */
export function describeOwn(value) {
  if (value === null || typeof value !== "object") return { kind: typeof value, value }
  const rows = {}
  for (const key of Reflect.ownKeys(value)) {
    if (typeof key !== "string") continue
    const descriptor = Object.getOwnPropertyDescriptor(value, key)
    rows[key] = {
      enumerable: descriptor.enumerable, writable: descriptor.writable ?? null,
      configurable: descriptor.configurable, accessor: Boolean(descriptor.get || descriptor.set),
    }
  }
  return { kind: "object", keys: Object.keys(value), descriptors: rows }
}
