import assert from "node:assert/strict"
import { readFileSync } from "node:fs"
import { dirname, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { describe, it } from "node:test"
import {
  defaultFootnoteBackContent as officialFootnoteBackContent,
  defaultFootnoteBackLabel as officialFootnoteBackLabel,
} from "mdast-util-to-hast"
import remarkRehype, {
  defaultFootnoteBackContent,
  defaultFootnoteBackLabel,
  defaultHandlers,
} from "../dist/remark-rehype.esm.js"

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..")

const paragraph = {
  type: "root",
  children: [
    {
      type: "paragraph",
      children: [{ type: "text", value: "hi" }],
    },
  ],
}

describe("remark-rehype", () => {
  it("exports the plugin as default", () => {
    assert.equal(typeof remarkRehype, "function")
  })

  it("exports the mdast-util-to-hast public helpers", () => {
    assert.equal(typeof defaultFootnoteBackContent, "function")
    assert.equal(typeof defaultFootnoteBackLabel, "function")
    assert.equal(typeof defaultHandlers, "object")
    assert.equal(defaultFootnoteBackContent(0, 2)[1].children[0].value, "2")
    assert.deepEqual(Object.keys(defaultHandlers).sort(), [
      "blockquote",
      "break",
      "code",
      "definition",
      "delete",
      "emphasis",
      "footnoteDefinition",
      "footnoteReference",
      "heading",
      "html",
      "image",
      "imageReference",
      "inlineCode",
      "link",
      "linkReference",
      "list",
      "listItem",
      "paragraph",
      "root",
      "strong",
      "table",
      "tableCell",
      "tableRow",
      "text",
      "thematicBreak",
      "toml",
      "yaml",
    ])
  })

  it("preserves numeric footnote helper arguments like upstream", () => {
    for (const value of [1.5, 2.5, Number.POSITIVE_INFINITY, Number.NaN]) {
      assert.deepEqual(
        defaultFootnoteBackContent(0, value),
        officialFootnoteBackContent(0, value),
      )
      assert.equal(
        defaultFootnoteBackLabel(value, value),
        officialFootnoteBackLabel(value, value),
      )
    }
  })

  it("transforms a paragraph fixture into a p element", () => {
    const transform = remarkRehype.call({})
    const hast = transform(paragraph)
    assert.equal(hast.type, "root")
    assert.equal(hast.children[0].type, "element")
    assert.equal(hast.children[0].tagName, "p")
    assert.equal(hast.children[0].children[0].value, "hi")
  })

  it("maps unknown value nodes and task lists like upstream", () => {
    const transform = remarkRehype.call({})
    const hast = transform({
      type: "root",
      children: [
        { type: "inlineMath", value: "x" },
        {
          type: "list",
          ordered: false,
          spread: false,
          children: [
            {
              type: "listItem",
              spread: false,
              checked: true,
              children: [{ type: "paragraph", children: [{ type: "text", value: "done" }] }],
            },
          ],
        },
      ],
    })
    const tags = JSON.stringify(hast)
    assert.equal(hast.children[0].type, "text")
    assert.equal(hast.children[0].value, "x")
    assert.match(tags, /checkbox/)
    assert.match(tags, /contains-task-list/)
  })

  it("keeps pinned keys in the library artifact", () => {
    const src = readFileSync(resolve(root, "dist/remark-rehype.esm.js"), "utf8")
    assert.match(src, /allowDangerousHtml/)
    assert.match(src, /tagName/)
    assert.match(src, /className/)
    assert.match(src, /type/)
  })
})
