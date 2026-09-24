// The `lilscript.toml` schema, generated from the compiler's own source.
//
// 003 asks for one schema: the keys the compiler accepts, their types, their
// defaults and what each one does, reconciled with the documentation. Hand-kept
// docs had already drifted — nineteen accepted keys were documented nowhere — so
// the reference is generated from `src/config.rs` and
// `src/compilation_policy.rs`, using each field's own doc comment as its
// description, and `--check` fails when the committed reference and the source
// disagree or when an accepted key has no doc comment at all.
//
//   node finer/tools/config-schema.mjs            # rewrite docs/knowledge/config/schema.md
//   node finer/tools/config-schema.mjs --check    # fail on drift or an undocumented key

import { readFileSync, readdirSync, writeFileSync } from "node:fs"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"

const toolsDirectory = dirname(fileURLToPath(import.meta.url))
const root = resolve(toolsDirectory, "../..")
const OUTPUT = join(root, "docs/knowledge/config/schema.md")
const SOURCES = ["src/config.rs", "src/compilation_policy.rs"]

/** Every `pub struct` in the given Rust sources, with its fields. */
export function parseStructs(text) {
  const structs = new Map()
  const pattern = /((?:^[ \t]*(?:\/\/\/[^\n]*|#\[[^\n]*\])\n)*)^pub struct ([A-Za-z0-9_]+)(?:<[^>]*>)?\s*\{\n([\s\S]*?)^\}/gm
  for (const match of text.matchAll(pattern)) {
    const [, header, name, body] = match
    const fields = []
    let docs = [], attributes = []
    for (const line of body.split("\n")) {
      const trimmed = line.trim()
      if (trimmed.startsWith("///")) { docs.push(trimmed.replace(/^\/\/\/ ?/, "")); continue }
      if (trimmed.startsWith("#[")) { attributes.push(trimmed); continue }
      const field = /^pub ([a-z_][a-z0-9_]*):\s*(.+?),?$/.exec(trimmed)
      if (field) {
        fields.push({ name: field[1], type: field[2].replace(/,$/, ""), docs: docs.join(" ").trim(), attributes })
      }
      if (trimmed && !trimmed.startsWith("//")) { docs = []; attributes = [] }
    }
    structs.set(name, { name, fields, deny: /deny_unknown_fields/.test(header), docs: header.split("\n").filter(line => line.trim().startsWith("///")).map(line => line.trim().replace(/^\/\/\/ ?/, "")).join(" ") })
  }
  return structs
}

/** `field: value,` pairs inside `impl Default for <name>`. */
export function parseDefaults(text, name) {
  const at = text.indexOf(`impl Default for ${name} {`)
  if (at < 0) return new Map()
  const body = text.slice(at, text.indexOf("\n}\n", at))
  const defaults = new Map()
  for (const match of body.matchAll(/^\s{12}([a-z_][a-z0-9_]*):\s*(.+?),$/gm)) defaults.set(match[1], match[2])
  return defaults
}

const SECTIONS = [
  ["optimization", "OptimizationConfig"], ["javascript", "JavaScriptConfig"], ["mangle", "MangleConfig"],
  ["bundle", "BundleConfig"], ["lint", "LintConfig"], ["format", "FormatConfig"], ["policy", "PolicyConfig"],
]

/** String constants (`const NAME: &str = "…";`), with Rust's `\` line continuations joined. */
function stringConstants(text) {
  const constants = new Map()
  for (const match of text.matchAll(/^const ([A-Z_]+): &str\s*=\s*((?:"(?:[^"\\]|\\[\s\S])*"\s*)+);/gm)) {
    constants.set(match[1], rustString(match[2]))
  }
  return constants
}

/** The value of one Rust string literal (or adjacent literals), unescaped. */
function rustString(literal) {
  return [...literal.matchAll(/"((?:[^"\\]|\\[\s\S])*)"/g)].map(([, body]) =>
    body.replace(/\\\n\s*/g, "").replace(/\\(["\\])/g, "$1")).join("")
}

/** The retired-key table (`RETIRED_KEYS` in src/config.rs) and its list-entry companions. */
export function parseRetiredKeys(text) {
  const constants = stringConstants(text)
  const reason = token => token.startsWith('"') ? rustString(token) : constants.get(token) ?? token
  const start = text.indexOf("pub const RETIRED_KEYS")
  const body = text.slice(start, text.indexOf("\n];", start))
  const string = String.raw`"(?:[^"\\]|\\[\s\S])*"`
  const entries = []
  const pattern = new RegExp(String.raw`\(\s*(${string}),\s*Retirement::(NoEffect|Refused|RefusedUnless)\s*(?:\(\s*(${string}|[A-Z_]+)\s*,?\s*\)|\{([\s\S]*?)\})\s*,?\s*\)`, "g")
  for (const match of body.matchAll(pattern)) {
    const [, key, kind, simple, fields] = match
    if (kind !== "RefusedUnless") { entries.push({ key: rustString(key), kind, reason: reason(simple) }); continue }
    const value = /value:\s*RetiredValue::(?:String\((".*?")\)|Integer\((-?\d+)\))/.exec(fields)
    const then = new RegExp(String.raw`then:\s*(None|Some\(\s*(${string})\s*\))`).exec(fields)
    const refused = new RegExp(String.raw`refused:\s*(${string})`).exec(fields)
    entries.push({
      key: rustString(key), kind,
      value: value[1] ?? value[2],
      then: then[1] === "None" ? null : rustString(then[2]),
      reason: rustString(refused[1]),
    })
  }
  const list = name => {
    const at = text.indexOf(`pub const ${name}`)
    return [...text.slice(at, text.indexOf("\n];", at)).matchAll(/^\s+"([a-z-]+)",$/gm)].map(match => match[1])
  }
  return { entries, compression: list("RETIRED_COMPRESSION_DECISIONS"), optimizations: list("RETIRED_JAVASCRIPT_OPTIMIZATIONS") }
}

function firstSentence(text) {
  if (!text) return ""
  const match = /^(.+?[.!?])(\s|$)/.exec(text)
  return (match ? match[1] : text).replace(/\|/g, "\\|")
}

/** Prose documentation files, and a lookup for the first one that names a key. */
function proseIndex() {
  const files = [join(root, "docs/configuration.md"), ...readdirSync(join(root, "docs/knowledge/config")).filter(name => name.endsWith(".md")).sort().map(name => join(root, "docs/knowledge/config", name))]
    .filter(path => !path.endsWith("/schema.md"))
  const texts = files.map(path => ({ path, text: readFileSync(path, "utf8") }))
  return key => texts.find(({ text }) => new RegExp("`" + key + "`|\\b" + key + "\\s*=").test(text))?.path ?? null
}

/** Build the whole reference and the list of keys documented nowhere. */
export function buildSchema() {
  const text = SOURCES.map(path => readFileSync(join(root, path), "utf8")).join("\n")
  const describedIn = proseIndex()
  const structs = parseStructs(text)
  const undocumented = []
  const lines = [
    "# `lilscript.toml` schema",
    "",
    "Generated by `node finer/tools/config-schema.mjs` from the doc comments in `src/config.rs` and",
    "`src/compilation_policy.rs`. Do not edit by hand: change the source comment and regenerate.",
    "`node finer/tools/config-schema.mjs --check` fails when this file and the source disagree.",
    "",
    "Every table marked *closed* rejects unknown keys (`deny_unknown_fields`), so a misspelled key is an",
    "error, not a silently ignored setting.",
    "",
  ]
  const visit = (section, structName, depth) => {
    const struct = structs.get(structName)
    if (!struct) return
    const defaults = parseDefaults(text, structName)
    lines.push(`${"#".repeat(Math.min(2 + depth, 4))} \`[${section}]\`${struct.deny ? " — closed" : ""}`, "")
    if (struct.docs) lines.push(firstSentence(struct.docs), "")
    lines.push("| Key | Type | Default | Meaning |", "|---|---|---|---|")
    const nested = []
    for (const field of struct.fields) {
      if (field.attributes.some(attribute => /serde\(skip/.test(attribute))) continue
      const renamed = /rename\s*=\s*"([^"]+)"/.exec(field.attributes.join(" "))?.[1]
      const key = renamed ?? field.name
      const prose = field.docs ? null : describedIn(key)
      if (!field.docs && !prose) undocumented.push(`${section}.${key}`)
      const meaning = firstSentence(field.docs) ||
        (prose ? `See [${prose.split("/").pop()}](${prose.endsWith("/configuration.md") ? "../../configuration.md" : prose.split("/").pop()}).` : "**undocumented**")
      const defaultValue = defaults.get(field.name) ?? (/^Option</.test(field.type) ? "unset" : "")
      lines.push(`| \`${key}\` | \`${field.type.replace(/\|/g, "\\|")}\` | ${defaultValue ? `\`${defaultValue.replace(/\|/g, "\\|")}\`` : ""} | ${meaning} |`)
      const inner = /^(?:Option<)?([A-Z][A-Za-z0-9]*Config)>?$/.exec(field.type)?.[1]
      if (inner && structs.has(inner)) nested.push([`${section}.${key}`, inner])
    }
    lines.push("")
    for (const [name, inner] of nested) visit(name, inner, depth + 1)
  }
  for (const [section, structName] of SECTIONS) visit(section, structName, 0)
  const retired = parseRetiredKeys(text)
  const cell = value => value.replace(/\|/g, "\\|")
  lines.push(
    "## Retired keys",
    "",
    "Applied to the parsed file before the tables above are read (`RETIRED_KEYS` in `src/config.rs`). A",
    "*no effect* key is removed and the CLI warns `<key> has no effect in this compiler: <reason>; remove it`;",
    "`--print-policy` lists the same warnings. A *refused* key stops the build with its reason. A table",
    "path covers every key in that table.",
    "",
    "| Key | Outcome | Reason |",
    "|---|---|---|",
  )
  for (const entry of retired.entries) {
    const outcome = entry.kind === "NoEffect" ? "no effect" : entry.kind === "Refused" ? "refused" :
      `refused unless \`${cell(String(entry.value))}\`, which ${entry.then ? "has no effect" : "is kept"}`
    const why = entry.kind === "RefusedUnless" && entry.then ? `${entry.reason} (with \`${cell(String(entry.value))}\`: ${entry.then})` : entry.reason
    lines.push(`| \`${entry.key}\` | ${outcome} | ${cell(why)} |`)
  }
  lines.push(
    "",
    "Retired `javascript.compression` entries (no effect; the rest of the list keeps its exact-allowlist meaning):",
    retired.compression.map(name => `\`${name}\``).join(", ") + ".",
    "",
    "Retired `javascript.optimizations` entries (no effect):",
    retired.optimizations.map(name => `\`${name}\``).join(", ") + ".",
    "",
  )
  return { markdown: lines.join("\n"), undocumented }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const { markdown, undocumented } = buildSchema()
  if (process.argv.includes("--check")) {
    const current = readFileSync(OUTPUT, "utf8")
    const problems = []
    if (current !== markdown) problems.push("docs/knowledge/config/schema.md is out of date with the source; regenerate it")
    for (const key of undocumented) problems.push(`accepted key \`${key}\` is documented nowhere: add a doc comment to its field`)
    if (problems.length) { for (const problem of problems) console.log(`- ${problem}`); process.exit(1) }
    console.log("schema reference matches the source and every accepted key is documented")
  } else {
    writeFileSync(OUTPUT, markdown)
    console.log(`wrote ${OUTPUT}`)
    if (undocumented.length) console.log(`${undocumented.length} accepted key(s) are documented nowhere:\n  ${undocumented.join("\n  ")}`)
  }
}
