import assert from "node:assert/strict"
import { createRequire } from "node:module"
import { describe, it } from "node:test"

const module = await import("@itslil/remark-breaks")
const commonjs = createRequire(import.meta.url)("@itslil/remark-breaks")
const remarkBreaks = module.default

function paragraphTree(value) {
  return {
    type: "root",
    children: [{ type: "paragraph", children: [{ type: "text", value }] }],
  }
}

describe("@itslil/remark-breaks", () => {
  it("exposes the upstream public API and arity", () => {
    assert.deepEqual(Object.keys(module), ["default"])
    assert.deepEqual(Object.keys(commonjs), ["default"])
    assert.equal(typeof remarkBreaks, "function")
    assert.equal(typeof commonjs.default, "function")
    assert.equal(remarkBreaks.length, 0)
    assert.equal(remarkBreaks().length, 1)
  })

  it("splits text on newlines into text / break / text", () => {
    const tree = paragraphTree("a\nb")
    const transform = remarkBreaks.call({})
    const out = transform(tree)
    assert.equal(out, undefined)
    const kids = tree.children[0].children
    assert.equal(kids.length, 3)
    assert.equal(kids[0].type, "text")
    assert.equal(kids[0].value, "a")
    assert.equal(kids[1].type, "break")
    assert.equal(kids[2].type, "text")
    assert.equal(kids[2].value, "b")
  })

  it("does not split inside code or inlineCode", () => {
    const tree = {
      type: "root",
      children: [
        { type: "code", value: "a\nb" },
        { type: "paragraph", children: [{ type: "inlineCode", value: "a\nb" }] },
      ],
    }
    remarkBreaks.call({})(tree)
    assert.equal(tree.children[0].value, "a\nb")
    assert.equal(tree.children[1].children[0].type, "inlineCode")
    assert.equal(tree.children[1].children[0].value, "a\nb")
  })

  it("works through processor.use(plugin)", () => {
    const tree = paragraphTree("x\ny")
    const processor = {
      use(plugin) {
        return plugin.call(this)
      },
    }
    const transform = processor.use(remarkBreaks)
    transform(tree)
    assert.equal(tree.children[0].children[1].type, "break")
  })
})
