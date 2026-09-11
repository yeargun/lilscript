// Closure ADVANCED cannot rename properties without an externs file naming the
// members the program exchanges with code the compiler never sees. Our ports
// each wrap an npm package that already ships one: its TypeScript declarations.
//
// This harvests every property-shaped name from a port's installed `.d.ts`
// files and prints them as a `preserve_properties` list, so `internal_properties
// = "all"` can be turned on without the port's public vocabulary being renamed.
// It is deliberately over-inclusive -- a name that is preserved but did not need
// to be costs bytes; one that is renamed but was a contract is a wrong program.
//
// Measured coverage: on mdast-util-to-hastlil the declarations name 23 of the 26
// members `internal_properties = "all"` renamed away, and the three it misses
// (`dataFootnotes`, `dataFootnoteRef`, `dataFootnoteBackref`) are hast property
// names that reach HTML as `data-` attributes.
//
// SCOPE. The first version harvested every `.d.ts` under the port's
// `node_modules`, which on zodlil is 680 files and 8,419 names -- vitest's
// matchers and @types/node's entire surface included. That is not the port's
// contract, it is the contract of its test runner, and preserving it forfeits
// most of what property renaming is worth. The contract is the *wrapped*
// package's own declarations, plus the type packages those declarations import
// (one level): a remark port really does receive `node.type` and
// `node.children` from @types/mdast, and that vocabulary is as public as its
// own. Everything else is a build-time dependency the artifact never meets.
//
//   node finer/tools/externs-from-types.mjs <port> [--toml]
import { readFileSync, existsSync, readdirSync, statSync } from "node:fs"
import { join } from "node:path"
import { homedir } from "node:os"

const port = process.argv[2]
if (!port) {
  console.error("usage: node finer/tools/externs-from-types.mjs <port> [--toml]")
  process.exit(1)
}
const asToml = process.argv.includes("--toml")
// `--list` is the newline-delimited form `LILSCRIPT_PRESERVE_PROPERTIES_FILE`
// reads, so the pool can carry one list per port through a single arm.
const asList = process.argv.includes("--list")

function declarationFiles(dir, out = [], depth = 0) {
  if (depth > 4 || !existsSync(dir)) return out
  for (const entry of readdirSync(dir)) {
    if (entry === ".bin") continue
    const path = join(dir, entry)
    let stat
    try { stat = statSync(path) } catch { continue }
    if (stat.isDirectory()) declarationFiles(path, out, depth + 1)
    else if (entry.endsWith(".d.ts") || entry.endsWith(".d.mts")) out.push(path)
  }
  return out
}

// `name:`, `name?:`, `readonly name:`, `name(` -- a member position in a
// declaration. Over-approximate on purpose: a parameter name caught here only
// costs bytes.
const MEMBER = /(?:^|[\s;{(,|])(?:readonly\s+)?([A-Za-z_$][\w$]*)\s*\??\s*[:(]/gm

// The port's own hand-written JavaScript is an externs source in the strict
// Closure sense: it is not compiler input, it calls into the artifact, and
// nothing in the IR can see it. katexlil's `cli.js` and `contrib/` call
// `macroExpander.consumeArgs(..)`, which is a documented KaTeX extension point
// that katex's published `.d.ts` never names -- so renaming it compiled
// cleanly, shipped, and failed with `e.consumeArgs is not a function`.
//
// `dist/` is excluded deliberately and is the one exclusion that matters: it
// holds *our* output, so harvesting it would read back the mangled spellings and
// preserve them, which looks like a pass and means nothing.
const JS_MEMBER = /\.([A-Za-z_$][\w$]*)\b|(?:^|[\s;{(,])([A-Za-z_$][\w$]*)\s*:|\[\s*["']([A-Za-z_$][\w$]*)["']\s*\]/gm
const JS_SKIP = new Set(["node_modules", "dist", ".git", "target", "build"])
function interopFiles(dir, out = [], depth = 0) {
  if (depth > 5 || !existsSync(dir)) return out
  for (const entry of readdirSync(dir)) {
    if (JS_SKIP.has(entry) || entry.startsWith(".")) continue
    const path = join(dir, entry)
    let stat
    try { stat = statSync(path) } catch { continue }
    if (stat.isDirectory()) interopFiles(path, out, depth + 1)
    else if (/\.(m|c)?js$/.test(entry)) out.push(path)
  }
  return out
}

// The package the port wraps: `zodlil` -> `zod`, `hast-util-to-htmllil` ->
// `hast-util-to-html`. Verified against the installed tree rather than assumed,
// and `package.json` is consulted when the directory name does not map.
const modules = join(homedir(), port, "node_modules")
function wrappedPackage() {
  const stripped = port.replace(/lil$/, "")
  // `solidlil` wraps `solid-js`; `micromarklil` does not install `micromark`
  // itself but does install `micromark-util-types`, which is where that
  // package's construct vocabulary is declared.
  const candidates = [stripped, `${stripped}-js`, `@types/${stripped}`, `${stripped}-util-types`]
  for (const candidate of candidates) {
    if (existsSync(join(modules, candidate))) return candidate
  }
  const manifest = join(homedir(), port, "package.json")
  if (existsSync(manifest)) {
    const json = JSON.parse(readFileSync(manifest, "utf8"))
    const declared = { ...json.dependencies, ...json.devDependencies, ...json.peerDependencies }
    for (const name of Object.keys(declared)) {
      if (name.replace(/^@[^/]+\//, "") === stripped) return name
    }
  }
  return null
}
// One level of type closure: a package named by an `import`/`export ... from`
// in the wrapped package's own declarations contributes its vocabulary too.
function typeImports(files) {
  const out = new Set()
  const FROM = /\bfrom\s*["']([^"'.][^"']*)["']/g
  for (const file of files) {
    let text
    try { text = readFileSync(file, "utf8") } catch { continue }
    FROM.lastIndex = 0
    let match
    while ((match = FROM.exec(text))) {
      const specifier = match[1]
      const pkg = specifier.startsWith("@")
        ? specifier.split("/").slice(0, 2).join("/")
        : specifier.split("/")[0]
      if (existsSync(join(modules, pkg))) out.add(pkg)
      const typed = "@types/" + pkg.replace(/^@/, "").replace("/", "__")
      if (existsSync(join(modules, typed))) out.add(typed)
    }
  }
  return out
}

// The port's own published declarations are the contract by construction: they
// are what a consumer compiles against. They are a root whether or not the
// upstream package is installed -- several ports (rehype-katexlil,
// remark-breakslil) test against their own dist and never install it.
// A type-only package is installed *because* the port exchanges its vocabulary:
// `remark-breakslil` publishes four member names of its own and reads mdast
// nodes all day, and `@types/mdast` is the only place `children` and `position`
// are written down. `@types/node` is excluded -- it describes the runtime, not
// this port's contract, and the runtime surface is `js_externs::NOT_OURS`'s job.
function vocabularyPackages() {
  const out = new Set()
  if (!existsSync(modules)) return out
  for (const entry of readdirSync(modules)) {
    if (entry.endsWith("-types") && entry !== "undici-types") out.add(entry)
  }
  const typed = join(modules, "@types")
  if (existsSync(typed)) {
    for (const entry of readdirSync(typed)) {
      if (entry !== "node") out.add(`@types/${entry}`)
    }
  }
  return out
}

const wrapped = wrappedPackage()
const published = declarationFiles(join(homedir(), port, "dist"))
if (!wrapped && published.length === 0) {
  console.error(
    `${port}: no contract to harvest -- neither an installed upstream nor published declarations`,
  )
  process.exit(1)
}
const names = new Set()
const own = [...(wrapped ? declarationFiles(join(modules, wrapped)) : []), ...published]
const roots = new Set([
  ...(wrapped ? [wrapped] : []),
  ...typeImports(own),
  ...vocabularyPackages(),
])
const files = [...new Set([...published, ...[...roots].flatMap((pkg) => declarationFiles(join(modules, pkg)))])]
for (const file of files) {
  let text
  try { text = readFileSync(file, "utf8") } catch { continue }
  MEMBER.lastIndex = 0
  let match
  while ((match = MEMBER.exec(text))) names.add(match[1])
}
// Hand-written JS in the port, unless `--types-only` asks for the declarations
// alone (useful for seeing how much of the contract the types actually state).
let interop = []
if (!process.argv.includes("--types-only")) {
  interop = interopFiles(join(homedir(), port))
  for (const file of interop) {
    let text
    try { text = readFileSync(file, "utf8") } catch { continue }
    JS_MEMBER.lastIndex = 0
    let match
    while ((match = JS_MEMBER.exec(text))) {
      const name = match[1] ?? match[2] ?? match[3]
      if (name) names.add(name)
    }
  }
}
const sorted = [...names].sort()
if (asList) {
  console.log(`# generated by finer/tools/externs-from-types.mjs for ${port}`)
  console.log(`# roots: ${[...roots].sort().join(", ") || "(published only)"}`)
  for (const name of sorted) console.log(name)
} else if (asToml) {
  console.log("# generated by finer/tools/externs-from-types.mjs -- the upstream")
  console.log(`# package's own type declarations: ${[...roots].sort().join(", ") || "(published only)"}`)
  console.log(`# harvested from ${files.length} .d.ts files.`)
  console.log("preserve_properties = [")
  for (const name of sorted) console.log(`  ${JSON.stringify(name)},`)
  console.log("]")
} else {
  console.error(
    `${port}: wraps ${wrapped ?? "(none installed)"}; published ${published.length}; ` +
      `roots ${[...roots].sort().join(", ") || "(published only)"}; ` +
      `${files.length} .d.ts + ${interop.length} interop js, ${sorted.length} member names`,
  )
  console.log(JSON.stringify(sorted))
}
