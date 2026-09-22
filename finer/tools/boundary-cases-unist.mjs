// 002's boundary cases for the two unist boundaries that have a source-built,
// case-qualified incumbent today: `mdast-util-to-hast` and `hast-util-to-html`.
//
// Each case is one observation a JavaScript caller can make. The upstream
// library is the oracle; the candidate must agree with it. The callers listed
// on each case are why the observation is required rather than interesting.
//
// Where an observation is upstream behavior rather than a documented promise,
// `promised` says so — that distinction is what D2 has to resolve per case, and
// recording it is not the same as waiving it.

import { describeOwn, functionObservations } from "./boundary-conformance.mjs"

// A tree whose `value` is a counting accessor, so a case can observe how many
// times the boundary reads a caller's property and in what order.
function countingText(log, id, text) {
  return {
    type: "text",
    get value() { log.push(`read:${id}`); return text },
  }
}

const paragraph = (...children) => ({ type: "paragraph", children })
const text = value => ({ type: "text", value })
const root = (...children) => ({ type: "root", children })

export const MDAST_TO_HAST_CASES = [
  {
    id: "d2/function/public-name-and-arity",
    decision: "D2",
    rule: "A declared public function keeps the name, arity and constructibility its callers can reflect on.",
    promised: "upstream exports a named function; name reflection is not an explicit promise, which is the open part of D2",
    callers: ["001-MH mdast-util-to-hastlil type boundary", "001 HAST public-boundary diagnostic, which found toHtml.name reported as `kb`"],
    probe: namespace => functionObservations(namespace.toHast),
  },
  {
    id: "d2/identity/input-tree-is-not-mutated",
    decision: "D2",
    rule: "Converting a caller's tree does not write to it.",
    promised: "upstream behavior relied on by every caller that reuses a parsed tree",
    callers: ["remark-rehypelil", "react-markdownlil"],
    probe: namespace => {
      const input = root(paragraph(text("a")))
      const before = JSON.stringify(input)
      namespace.toHast(input)
      return { unchanged: JSON.stringify(input) === before }
    },
  },
  {
    id: "d2/identity/result-does-not-alias-the-input",
    decision: "D2",
    rule: "Nodes in the result are the boundary's own objects, so a caller mutating the result cannot reach back into its input.",
    promised: "upstream behavior",
    callers: ["remark-rehypelil", "rehypelil"],
    probe: namespace => {
      const leaf = text("a")
      const input = root(paragraph(leaf))
      const output = namespace.toHast(input)
      const found = JSON.stringify(output).includes("\"a\"")
      return { converted: found, sharesLeaf: output?.children?.[0]?.children?.[0] === leaf }
    },
  },
  {
    id: "d1/aliasing/one-node-used-twice-converts-twice",
    decision: "D1",
    rule: "A node the caller places at two positions is observed at both, and neither conversion is skipped or shared by accident.",
    promised: "upstream behavior; an aliasing case D1 must keep observable",
    callers: ["Motion overlapping-destination evidence in 002", "micromarklil retained token/context tuples"],
    probe: namespace => {
      const shared = text("x")
      const output = namespace.toHast(root(paragraph(shared), paragraph(shared)))
      const kids = output?.children ?? []
      const first = kids.find(node => node.type === "element")
      const second = kids.filter(node => node.type === "element")[1]
      return {
        bothPresent: Boolean(first && second),
        distinctObjects: Boolean(first && second && first.children?.[0] !== second.children?.[0]),
        sameText: JSON.stringify(first?.children?.[0]) === JSON.stringify(second?.children?.[0]),
      }
    },
  },
  {
    id: "d3/effects/caller-getter-is-read-and-its-order-is-observable",
    decision: "D3",
    rule: "Reads of a caller's accessors are preserved: the boundary reads what it needs, and the order is what upstream produces.",
    promised: "upstream behavior; D3 preserves host effects",
    callers: ["micromarklil numeric point fields read once in order"],
    probe: namespace => {
      const log = []
      namespace.toHast(root(paragraph(countingText(log, "one", "1"), countingText(log, "two", "2"))))
      return { log }
    },
  },
  {
    id: "d3/throws/a-caller-getter-that-throws-propagates",
    decision: "D3",
    rule: "An ordinary throw from a caller's accessor reaches the caller with the same constructor and message.",
    promised: "upstream behavior; D3 preserves explicit throws",
    callers: ["Motion partial-mutation evidence in 002"],
    probe: namespace => {
      const exploding = { type: "text", get value() { throw new TypeError("caller getter exploded") } }
      namespace.toHast(root(paragraph(exploding)))
      return { returnedWithoutThrowing: true }
    },
  },
  {
    id: "d2/enumeration/result-key-order",
    decision: "D2",
    rule: "The key order of a returned node is the order upstream produces, because callers enumerate and serialize it.",
    promised: "upstream behavior; jQuery's 001-JQ evidence shows property insertion order is observed",
    callers: ["remark-parselil `matches upstream property insertion order`", "rehype-stringifylil"],
    probe: namespace => {
      const output = namespace.toHast(root(paragraph(text("a"))))
      const element = output?.children?.find(node => node.type === "element")
      return { rootKeys: Object.keys(output ?? {}), elementKeys: Object.keys(element ?? {}) }
    },
  },
  {
    id: "d2/descriptors/result-properties-are-plain-data",
    decision: "D2",
    rule: "Returned properties are ordinary writable, enumerable, configurable data, not accessors or frozen slots.",
    promised: "upstream behavior relied on by every caller that edits a returned tree",
    callers: ["rehypelil plugins that mutate the tree in place"],
    probe: namespace => describeOwn(namespace.toHast(root(paragraph(text("a"))))?.children?.find(node => node.type === "element")),
  },
  {
    id: "d2/serialization/round-trips-through-json",
    decision: "D2",
    rule: "The whole result serializes to the same JSON upstream produces.",
    promised: "upstream behavior; the unist ecosystem passes trees as JSON",
    callers: ["remark-rehypelil", "react-markdownlil"],
    probe: namespace => ({ json: JSON.stringify(namespace.toHast(root(paragraph(text("a"), { type: "emphasis", children: [text("b")] })))) }),
  },
  {
    id: "d2/callbacks/a-handler-receives-the-callers-own-node",
    decision: "D2",
    rule: "A caller-supplied handler is called with the caller's own node object, not a copy of it.",
    promised: "upstream behavior; the handler contract is documented",
    callers: ["remark-rehypelil custom handlers", "react-markdownlil"],
    probe: namespace => {
      const node = paragraph(text("a"))
      let sameObject = null, argumentCount = null
      namespace.toHast(root(node), {
        handlers: {
          paragraph(state, given) {
            sameObject = given === node
            argumentCount = arguments.length
            return { type: "element", tagName: "p", properties: {}, children: [] }
          },
        },
      })
      return { sameObject, argumentCount }
    },
  },
]

export const HAST_TO_HTML_CASES = [
  {
    id: "d2/function/public-name-and-arity",
    decision: "D2",
    rule: "A declared public function keeps the name, arity and constructibility its callers can reflect on.",
    promised: "upstream documents the named export but does not explicitly promise function-name reflection; 001's installed-CJS supplement already fails on exactly this",
    callers: ["001 HAST source-built-package-cjs receipt, where the unchanged public-name expectation failed with `kb` against `toHtml`"],
    probe: namespace => functionObservations(namespace.toHtml),
  },
  {
    id: "d2/identity/input-tree-is-not-mutated",
    decision: "D2",
    rule: "Serializing a caller's tree does not write to it.",
    promised: "upstream behavior relied on by every caller that serializes twice",
    callers: ["rehype-stringifylil", "rehypelil"],
    probe: namespace => {
      const input = { type: "element", tagName: "p", properties: { className: ["a"] }, children: [{ type: "text", value: "a" }] }
      const before = JSON.stringify(input)
      namespace.toHtml(input)
      return { unchanged: JSON.stringify(input) === before }
    },
  },
  {
    id: "d3/effects/caller-getter-is-read-and-its-order-is-observable",
    decision: "D3",
    rule: "Reads of a caller's accessors are preserved in upstream's order.",
    promised: "upstream behavior",
    callers: ["micromarklil numeric point fields"],
    probe: namespace => {
      const log = []
      namespace.toHtml({ type: "element", tagName: "p", properties: {}, children: [countingText(log, "one", "1"), countingText(log, "two", "2")] })
      return { log }
    },
  },
  {
    id: "d3/throws/a-caller-getter-that-throws-propagates",
    decision: "D3",
    rule: "An ordinary throw from a caller's accessor reaches the caller unchanged.",
    promised: "upstream behavior",
    callers: ["Motion partial-mutation evidence in 002"],
    probe: namespace => {
      namespace.toHtml({ type: "element", tagName: "p", properties: {}, children: [{ type: "text", get value() { throw new RangeError("caller getter exploded") } }] })
      return { returnedWithoutThrowing: true }
    },
  },
  {
    id: "d3/errors/unknown-node-type",
    decision: "D3",
    rule: "An argument error for an unusable node is the same error upstream raises.",
    promised: "upstream behavior; D3 preserves argument errors",
    callers: ["rehype-stringifylil", "hast-util-to-htmllil official suite"],
    probe: namespace => ({ html: namespace.toHtml({ type: "no-such-node-type" }) }),
  },
  {
    id: "d2/serialization/character-references-and-quotes",
    decision: "D2",
    rule: "Escaping decisions are byte-for-byte upstream's, because output is the whole contract here.",
    promised: "documented; the official suite asserts it",
    callers: ["hast-util-to-htmllil official suite", "rehype-stringifylil"],
    probe: namespace => ({
      escaped: namespace.toHtml({ type: "element", tagName: "p", properties: { title: `a"b'c<d>e&f` }, children: [{ type: "text", value: `<&>"' ` }] }),
    }),
  },
  {
    id: "d2/options/an-unknown-option-is-ignored-not-rejected",
    decision: "D2",
    rule: "An option the boundary does not know is ignored, as upstream ignores it, so a caller passing a newer option is not broken.",
    promised: "upstream behavior",
    callers: ["rehypelil", "rehype-stringifylil"],
    probe: namespace => ({ html: namespace.toHtml({ type: "text", value: "a" }, { thisOptionDoesNotExist: true }) }),
  },
  {
    id: "d2/arguments/an-array-of-nodes-is-accepted",
    decision: "D2",
    rule: "The documented array form of the first argument works identically.",
    promised: "documented",
    callers: ["hast-util-to-htmllil official suite"],
    probe: namespace => ({ html: namespace.toHtml([{ type: "text", value: "a" }, { type: "text", value: "b" }]) }),
  },
]
