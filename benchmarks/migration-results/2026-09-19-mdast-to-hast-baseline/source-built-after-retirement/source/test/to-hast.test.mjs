import assert from "node:assert/strict"
import { existsSync, readFileSync } from "node:fs"
import { dirname, resolve } from "node:path"
import { fileURLToPath, pathToFileURL } from "node:url"
import { describe, it } from "node:test"
import {
  defaultFootnoteBackContent as officialFootnoteBackContent,
  defaultFootnoteBackLabel as officialFootnoteBackLabel,
} from "mdast-util-to-hast"

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..")
const library = await import("../dist/to-hast.esm.js")
const { toHast } = library

function text(value) {
  return { type: "text", value }
}

function fixture() {
  return {
    type: "root",
    children: [
      { type: "heading", depth: 2, children: [text("Title")] },
      { type: "paragraph", children: [text("hi"), { type: "break" }, { type: "inlineCode", value: "x" }] },
      { type: "thematicBreak" },
      {
        type: "blockquote",
        children: [{ type: "paragraph", children: [{ type: "emphasis", children: [text("q")] }] }],
      },
      {
        type: "list",
        ordered: true,
        start: 3,
        children: [
          {
            type: "listItem",
            checked: true,
            children: [{ type: "paragraph", children: [text("done")] }],
          },
        ],
      },
      { type: "code", lang: "js", value: "ok" },
      {
        type: "paragraph",
        children: [
          { type: "strong", children: [text("b")] },
          { type: "delete", children: [text("d")] },
          { type: "link", url: "https://ex", title: "t", children: [text("a")] },
          { type: "image", url: "pic.png", alt: "pic", title: "img" },
        ],
      },
      { type: "html", value: "<br>" },
      {
        type: "table",
        align: ["left", "center", null],
        children: [
          {
            type: "tableRow",
            children: [
              { type: "tableCell", children: [text("h1")] },
              { type: "tableCell", children: [text("h2")] },
              { type: "tableCell", children: [text("h3")] },
            ],
          },
          {
            type: "tableRow",
            children: [
              { type: "tableCell", children: [text("a")] },
              { type: "tableCell", children: [text("b")] },
              { type: "tableCell", children: [text("c")] },
            ],
          },
        ],
      },
      { type: "inlineMath", value: "x^2" },
      { type: "math", value: "E=mc^2" },
      { type: "definition", identifier: "ref", label: "ref", url: "https://def", title: "Def" },
      {
        type: "paragraph",
        children: [
          { type: "linkReference", identifier: "ref", label: "ref", referenceType: "full", children: [text("go")] },
          { type: "linkReference", identifier: "missing", label: "missing", referenceType: "full", children: [text("no")] },
        ],
      },
    ],
  }
}

function find(node, pred) {
  if (pred(node)) return node
  const kids = node?.children
  if (!Array.isArray(kids)) return null
  for (const child of kids) {
    const hit = find(child, pred)
    if (hit) return hit
  }
  return null
}

describe("@itslil/mdast-util-to-hast library", () => {
  it("exports the upstream public API", () => {
    assert.equal(typeof toHast, "function")
    assert.deepEqual(Object.keys(library).sort(), [
      "defaultFootnoteBackContent",
      "defaultFootnoteBackLabel",
      "defaultHandlers",
      "toHast",
    ])
    assert.deepEqual(Object.keys(library.defaultHandlers).sort(), [
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
        library.defaultFootnoteBackContent(0, value),
        officialFootnoteBackContent(0, value),
      )
      assert.equal(
        library.defaultFootnoteBackLabel(value, value),
        officialFootnoteBackLabel(value, value),
      )
    }
  })

  it("keeps pinned option and tree keys in the library artifact", () => {
    const source = readFileSync(resolve(root, "dist/to-hast.esm.js"), "utf8")
    assert.match(source, /allowDangerousHtml/)
    assert.match(source, /(?:\.type|["']type["'])/)
    assert.match(source, /export\{[^}]*\btoHast\b/)
  })

  it("maps a mdast fixture to hast elements", () => {
    const tree = toHast(fixture(), { allowDangerousHtml: true })
    assert.equal(tree.type, "root")
    assert.equal(Array.isArray(tree.children), true)

    const h2 = find(tree, (n) => n.tagName === "h2")
    assert.equal(h2?.children[0]?.value, "Title")

    const p = find(tree, (n) => n.tagName === "p" && n.children?.[0]?.value === "hi")
    assert.equal(p?.children[1]?.tagName, "br")
    const inlineCode = find(p, (n) => n.tagName === "code")
    assert.equal(inlineCode?.children[0]?.value, "x")

    assert.ok(find(tree, (n) => n.tagName === "hr"))
    assert.ok(find(tree, (n) => n.tagName === "blockquote"))
    assert.ok(find(tree, (n) => n.tagName === "em"))

    const ol = find(tree, (n) => n.tagName === "ol")
    assert.equal(ol?.properties?.start, 3)
    const li = find(ol, (n) => n.tagName === "li")
    assert.equal(li?.children[0]?.tagName, "input")
    assert.equal(li?.children[0]?.properties?.type, "checkbox")
    assert.equal(li?.children[0]?.properties?.disabled, true)
    assert.equal(li?.children[0]?.properties?.checked, true)

    const code = find(tree, (n) => n.tagName === "code" && n.properties?.className)
    assert.deepEqual(code?.properties?.className, ["language-js"])
    assert.equal(code?.children[0]?.value, "ok\n")
    assert.ok(find(tree, (n) => n.tagName === "pre"))

    assert.ok(find(tree, (n) => n.tagName === "strong"))
    assert.ok(find(tree, (n) => n.tagName === "del"))
    const a = find(tree, (n) => n.tagName === "a" && n.properties?.href === "https://ex")
    assert.equal(a?.properties?.title, "t")
    const img = find(tree, (n) => n.tagName === "img" && n.properties?.src === "pic.png")
    assert.equal(img?.properties?.alt, "pic")

    const raw = find(tree, (n) => n.type === "raw")
    assert.equal(raw?.value, "<br>")

    const th = find(tree, (n) => n.tagName === "th" && n.properties?.align === "center")
    assert.ok(th)
    const td = find(tree, (n) => n.tagName === "td" && n.children?.[0]?.value === "a")
    assert.equal(td?.properties?.align, "left")
    assert.ok(find(tree, (n) => n.tagName === "thead"))
    assert.ok(find(tree, (n) => n.tagName === "tbody"))

    assert.ok(find(tree, (n) => n.type === "text" && n.value === "x^2"))
    assert.ok(find(tree, (n) => n.type === "text" && n.value === "E=mc^2"))

    const ref = find(tree, (n) => n.tagName === "a" && n.properties?.href === "https://def")
    assert.equal(ref?.properties?.title, "Def")
    assert.equal(ref?.children[0]?.value, "go")
    const missing = find(tree, (n) => n.type === "text" && n.value === "[no][missing]")
    assert.ok(missing)
    assert.equal(find(tree, (n) => n.type === "definition"), null)
  })

  it("skips html when allowDangerousHtml is off", () => {
    const tree = toHast({
      type: "root",
      children: [{ type: "html", value: "<em>x</em>" }, { type: "paragraph", children: [text("z")] }],
    })
    assert.equal(find(tree, (n) => n.type === "raw"), null)
    assert.ok(find(tree, (n) => n.tagName === "p"))
  })
})

describe("@itslil/mdast-util-to-hast closed lane", () => {
  it("exists and still converts a tree", async () => {
    const closedPath = resolve(root, "dist/to-hast.closed.js")
    assert.equal(existsSync(closedPath), true)
    const closed = await import(pathToFileURL(closedPath).href)
    assert.equal(typeof closed.toHast, "function")
    const tree = closed.toHast({
      type: "root",
      children: [{ type: "paragraph", children: [text("hi")] }],
    })
    assert.equal(tree.type, "root")
    assert.equal(tree.children[0].tagName, "p")
    assert.equal(tree.children[0].children[0].value, "hi")
  })
})
