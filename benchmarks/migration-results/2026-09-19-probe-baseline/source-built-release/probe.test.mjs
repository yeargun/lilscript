// The probe owns its golden output and host runner. This adapter exposes those
// existing checks as named cases with observed production module loads.
import assert from "node:assert/strict"
import { spawnSync } from "node:child_process"
import { readFileSync } from "node:fs"
import { resolve } from "node:path"
import test from "node:test"

const expected = readFileSync(resolve("expected.out"), "utf8")
for (const [name, artifact] of [["optimized", "dist/probe.js"], ["unoptimized reference", "dist/probe.none.js"]]) {
  test(`${name} preserves the probe's golden behavior`, () => {
    const result = spawnSync(process.execPath, [resolve("dist/run.cjs"), resolve(artifact)], { encoding: "utf8" })
    assert.equal(result.status, 0, result.stderr)
    assert.equal(result.stdout, expected)
  })
}
