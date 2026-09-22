// Consumer behavior for each independently selected ESM objective artifact.
// Copied into the isolated port workspace before its input snapshot is taken.
import assert from "node:assert/strict"
import { describe, it } from "node:test"
import { marked as official } from "marked"
import { loadSpecCases } from "../scripts/spec.mjs"

const cases = loadSpecCases()
const optionSets = [{}, { breaks: true }, { pedantic: true }, { gfm: false }]
const optionKeys = ["async", "breaks", "gfm", "pedantic", "silent"]

for (const profile of ["esm", "gzip", "bytes"]) {
  const library = await import(new URL(`../dist/marked.${profile}.js`, import.meta.url))
  describe(`complete ${profile} ESM consumer profile`, () => {
    it("uses the frozen 660-example corpus", () => assert.equal(cases.length, 660))
    it("preserves a list item whose contents are a closing bracket", () => {
      for (const options of optionSets) {
        assert.equal(library.parse("- ]", options), "<ul>\n<li>]</li>\n</ul>\n")
      }
    })
    for (const options of optionSets) {
      for (const method of ["parse", "parseInline"]) {
        it(`${method} matches upstream on every corpus example with ${JSON.stringify(options)}`, () => {
          for (const example of cases) {
            assert.equal(library[method](example.markdown, options), official[method](example.markdown, options), `${example.file}#${example.example}`)
          }
        })
      }
    }
    it("preserves the declared parse API and option spellings", () => {
      for (const name of ["parse", "parseInline", "setOptions", "options", "getDefaults", "marked"]) assert.equal(typeof library[name], "function", name)
      assert.equal(library.default, library.marked)
      assert.equal(library.marked.parse, library.marked)
      assert.equal(library.marked.options, library.marked.setOptions)
      assert.equal(library.defaults, library.marked.defaults)
      assert.deepEqual(Object.keys(library.getDefaults()).sort(), optionKeys)
      for (const options of [null, {}, { breaks: true }, { gfm: false }, { pedantic: true }, { silent: true }]) {
        assert.equal(library.marked("a\nb ~~c~~", options), official.parse("a\nb ~~c~~", options))
        assert.equal(library.marked.parseInline("**x**", options), official.parseInline("**x**", options))
      }
      assert.throws(() => library.marked(1), /marked\(\): input must be a string/)
    })
    it("preserves live defaults and factory defaults through every setter", () => {
      const factory = library.getDefaults()
      try {
        assert.equal(library.setOptions({ breaks: true }), library.marked)
        assert.equal(library.defaults.breaks, true)
        assert.equal(library.marked.defaults.breaks, true)
        assert.equal(library.getDefaults().breaks, false)
        assert.equal(library.parse("a\nb"), official.parse("a\nb", { breaks: true }))
        assert.equal(library.options({ breaks: false, gfm: false }), library.marked)
        assert.equal(library.marked.parseInline("~~x~~"), official.parseInline("~~x~~", { gfm: false }))
        assert.equal(library.marked.setOptions({ pedantic: true }), library.marked)
        assert.equal(library.marked.defaults.pedantic, true)
      } finally {
        library.marked.setOptions(factory)
      }
      assert.equal(library.parse("a\nb"), official.parse("a\nb"))
    })
  })
}
