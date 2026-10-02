import assert from "node:assert/strict"
import test from "node:test"
import * as full from "motionlil/full"
import * as upstream from "motion-utils"

function identityTrace(api) {
  const first = { value: 1 }, other = { value: 1 }, callback = () => {}
  const values = [first, callback, NaN, 0, first]
  const label = value => value === first ? "first"
    : value === other ? "other" : value === callback ? "callback"
      : Number.isNaN(value) ? "NaN" : Object.is(value, -0) ? "-0" : value
  const observations = []
  for (const [operation, value] of [
    ["addUniqueItem", first], ["addUniqueItem", other],
    ["addUniqueItem", callback], ["addUniqueItem", NaN],
    ["addUniqueItem", -0], ["removeItem", first],
    ["removeItem", callback], ["removeItem", NaN],
  ]) {
    observations.push([api[operation](values, value), values.map(label)])
  }
  const sparse = Array(2)
  api.addUniqueItem(sparse, undefined)
  observations.push([Object.keys(sparse), sparse.length])
  api.removeItem(sparse, undefined)
  observations.push([Object.keys(sparse), sparse.length])
  return observations
}

test("array utilities preserve upstream identity, NaN and sparse-array behavior", () => {
  assert.deepEqual(identityTrace(full), identityTrace(upstream))
})

test("array utilities retain receiver, method order and strict index comparison", () => {
  function trace(api) {
    const seen = []
    const value = {}
    const values = {
      indexOf(item) {
        seen.push(["indexOf", this === values, item === value])
        return { valueOf() { seen.push("coerce"); return -1 } }
      },
      push(item) { seen.push(["push", this === values, item === value]) },
      splice(index, count) { seen.push(["splice", this === values, index, count]) },
    }
    api.addUniqueItem(values, value)
    api.removeItem(values, value)
    return seen
  }
  assert.deepEqual(trace(full), trace(upstream))
})

test("moveItem retains number indices, sparse spreading and item identity", () => {
  const value = {}, callback = () => {}
  for (const input of [[value, callback, NaN], [, value, , callback]]) {
    const before = input.slice()
    for (const [from, to] of [
      [0, 2], [-1, 0], [1.75, -0.25], [-3.25, 1],
      [NaN, 0], [Infinity, 1], [-Infinity, 1],
      [0, Infinity], [0, -Infinity], [0, NaN],
      [4294967297, 0], [0, -4294967297],
    ]) {
      const actual = full.moveItem(input, from, to)
      assert.deepEqual(actual, upstream.moveItem(input, from, to))
      assert.notEqual(actual, input)
      assert.deepEqual(input, before)
    }
  }
})
