// The derivation is what every frozen inventory rests on, so its refusals
// matter as much as its parses. Each case here is a real `npm test` script from
// this fleet, or a shape the parser must refuse rather than guess at.

import assert from "node:assert/strict"
import test from "node:test"
import { deriveAdapter, reconstruct } from "./adapter-derivation.mjs"

const scriptsOf = entries => Object.fromEntries(entries)

test("a plain node --test command parses", () => {
  const derived = deriveAdapter("node --test test/*.test.mjs")
  assert.equal(derived.supported, true)
  assert.deepEqual(derived.patterns, ["test/*.test.mjs"])
  assert.deepEqual(derived.nodeArguments, [])
  assert.deepEqual(derived.prerequisites, [])
})

test("npm run segments become prerequisites in order", () => {
  const derived = deriveAdapter("npm run check:sources && node --test test/*.test.mjs test/official/index.js && npm run check:types")
  assert.equal(derived.supported, true)
  assert.deepEqual(derived.prerequisites.map(row => row.script), ["check:sources", "check:types"])
  assert.deepEqual(derived.patterns, ["test/*.test.mjs", "test/official/index.js"])
})

test("runner flags are kept and separated from the file list", () => {
  const derived = deriveAdapter("node --conditions development --experimental-loader=./test/load-jsx.js --no-warnings --test test/*.test.mjs test/official/test.jsx")
  assert.equal(derived.supported, true)
  assert.deepEqual(derived.nodeArguments, ["--conditions", "development", "--experimental-loader=./test/load-jsx.js", "--no-warnings"])
  assert.deepEqual(derived.patterns, ["test/*.test.mjs", "test/official/test.jsx"])
})

test("a build segment is dropped and the omission is recorded", () => {
  const derived = deriveAdapter("npm run build && npm run check:types && node --test test/*.test.mjs")
  assert.equal(derived.supported, true)
  assert.deepEqual(derived.prerequisites.map(row => row.script), ["check:types"])
  assert.equal(derived.deviations.length, 1)
  assert.match(derived.deviations[0], /dropped original build segment/)
})

test("keeping the build segment is possible when a caller asks for it", () => {
  const derived = deriveAdapter("npm run build && node --test test/*.test.mjs", { dropBuild: false })
  assert.deepEqual(derived.prerequisites.map(row => row.script), ["build"])
  assert.deepEqual(derived.deviations, [])
})

test("another runner alongside the built-in one is recorded as uncovered, not hidden", () => {
  const derived = deriveAdapter("node --test test/*.test.mjs && node --experimental-vm-modules ./node_modules/jest/bin/jest.js --config test/official/jest.config.mjs --runInBand")
  assert.equal(derived.supported, true)
  assert.deepEqual(derived.patterns, ["test/*.test.mjs"])
  assert.equal(derived.uncovered.length, 1)
  assert.match(derived.deviations.at(-1), /does not cover those cases/)
})

test("a command with no built-in runner at all is unsupported", () => {
  const derived = deriveAdapter("node scripts/build.mjs --dev && node --experimental-vm-modules node_modules/jest/bin/jest.js --config jest.config.cjs")
  assert.equal(derived.supported, false)
  assert.equal(derived.patterns.length, 0)
  assert(derived.unsupported.every(note => /does not run the built-in test runner/.test(note)), derived.unsupported.join("; "))
})

test("a wrapper script holding the runner is followed", () => {
  const scripts = scriptsOf([
    ["build", "node scripts/build.mjs"],
    ["test:artifact", "npm run test:upstream && node --test test/shader-processing.test.mjs"],
    ["test:upstream", "node scripts/test-upstream.mjs a b"],
  ])
  const derived = deriveAdapter("npm run build && npm run test:artifact", { scripts })
  assert.equal(derived.supported, true)
  assert.deepEqual(derived.patterns, ["test/shader-processing.test.mjs"])
  // The wrapper's own driver is not a built-in-runner segment, so it runs as
  // a prerequisite that must pass rather than being skipped as uncovered.
  assert.deepEqual(derived.prerequisites.map(row => row.script), ["test:upstream"])
  assert.deepEqual(derived.uncovered, [])
  assert.match(derived.deviations.find(note => note.startsWith("followed")), /test:artifact/)
})

test("a wrapper that merely calls node stays an ordinary prerequisite", () => {
  const scripts = scriptsOf([["check:sources", "node scripts/shared-sources.mjs"]])
  const derived = deriveAdapter("npm run check:sources && node --test test/*.test.mjs", { scripts })
  assert.equal(derived.supported, true)
  assert.deepEqual(derived.prerequisites.map(row => row.script), ["check:sources"])
  assert.deepEqual(derived.unsupported, [])
})

test("a wrapper cycle cannot loop", () => {
  const scripts = scriptsOf([["a", "npm run b && node --test x.test.mjs"], ["b", "npm run a"]])
  const derived = deriveAdapter("npm run a", { scripts })
  assert.equal(derived.supported, true)
  assert.deepEqual(derived.patterns, ["x.test.mjs"])
})

test("no test script is refused rather than crashing", () => {
  for (const value of [undefined, null, "", "   ", 42]) {
    const derived = deriveAdapter(value)
    assert.equal(derived.supported, false)
    assert.deepEqual(derived.unsupported, ["package.json declares no test script"])
  }
})

test("shell metacharacters are refused rather than guessed at", () => {
  for (const script of ["node --test test/*.test.mjs > out.txt", "node --test $FILES", "node --test a.mjs; rm -rf /", "node --test \"a b.mjs\"", "node --test a.mjs | tee log"]) {
    const derived = deriveAdapter(script)
    assert.equal(derived.supported, false, script)
  }
})

test("two built-in runner segments are refused", () => {
  const derived = deriveAdapter("node --test a.test.mjs && node --test b.test.mjs")
  assert.equal(derived.supported, false)
  assert(derived.unsupported.some(note => /more than one node --test segment/.test(note)))
})

test("a runner segment naming no files is refused", () => {
  assert.equal(deriveAdapter("node --test").supported, false)
  assert.equal(deriveAdapter("node --test --watch").supported, false)
})

test("the reconstruction is the original command, whatever the parse did with it", () => {
  for (const script of [
    "node --test test/*.test.mjs",
    "npm run build && npm run check:types && node --test test/*.test.mjs test/official/index.js",
    "node --test test/*.test.mjs && node --experimental-vm-modules jest.js",
  ]) {
    assert.equal(reconstruct(deriveAdapter(script)), script.split("&&").map(part => part.trim()).join(" && "))
  }
})
