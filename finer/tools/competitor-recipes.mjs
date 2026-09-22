// Runnable, pinned competitor recipes.
//
// 001 requires eligible competitor comparisons — Terser, Oxc/Rolldown and
// Closure ADVANCED where applicable — to be *runnable recipes* with equivalent
// APIs and host assumptions, recording installed tool content and not only
// lockfiles. `finer/tools/baselines.mjs` carries numbers; a number nobody can
// reconstruct is not a bar, so this module carries the construction instead.
//
// Every recipe states four things a reviewer can disagree with in the open:
//   * which upstream program is bundled, and what stays external
//   * which minifier runs, at which installed version and binary identity
//   * what the minifier is allowed to rename
//   * which codec measured the result
//
// It does not decide who wins. It produces a receipt whose `competitors` array
// carries a pinned artifact identity per tool, which is what a closure claim in
// 013 is allowed to cite.

import assert from "node:assert/strict"
import { execFileSync } from "node:child_process"
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs"
import { homedir } from "node:os"
import { basename, dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { fileIdentity, fingerprint } from "./artifact-evidence.mjs"
import { baselines } from "./baselines.mjs"
import { preserveBinary } from "./preserved-binaries.mjs"

const toolsDirectory = dirname(fileURLToPath(import.meta.url))
export const REPOSITORY_ROOT = resolve(toolsDirectory, "../..")
const CODEC = join(REPOSITORY_ROOT, "target/release/lilscript-codec")

// How far the reconstructed bar's raw size may sit from ours before the two
// stop being the same program. The +/-20-25% convention comes from
// finer/tools/rebuild-bars.mjs, which found the same disparity.
const MASS_BAND = [0.8, 1.25]

// Where each tool is installed. These are ports' own dev dependencies rather
// than a private toolbox, so a recipe measures what the project already has.
const TOOL_HOMES = {
  esbuild: join(homedir(), "unifiedlil/node_modules"),
  terser: join(homedir(), "unifiedlil/node_modules"),
  rolldown: join(homedir(), "markedlil/node_modules"),
  "google-closure-compiler": join(homedir(), "mobxlil/node_modules"),
}

function toolIdentity(name) {
  const home = TOOL_HOMES[name]
  assert(home, `no installed location recorded for ${name}`)
  const packagePath = join(home, name, "package.json")
  assert(existsSync(packagePath), `${name} is not installed at ${home}`)
  const manifest = JSON.parse(readFileSync(packagePath, "utf8"))
  const binary = join(home, ".bin", name)
  return {
    name, version: manifest.version, home,
    package: { path: packagePath, ...fileIdentity(packagePath) },
    binary: existsSync(binary) ? { path: binary, ...fileIdentity(binary) } : null,
  }
}

/**
 * The eligible recipes.
 *
 * `renames` states the mangling policy in words, because "minified" is not a
 * single setting and a comparison that hides it is not a comparison.
 */
export const RECIPES = {
  terser: {
    id: "terser",
    tool: "terser",
    renames: "locals and top-level module bindings (`-m --module`); property names untouched",
    // Compress defaults: the baseline has `pure_getters` off and `unsafe` is
    // not eligible (013). `passes=3` matches the pinned bars in baselines.mjs.
    run: ({ tool, input, output }) => execFileSync(join(tool.home, ".bin/terser"),
      [input, "-c", "passes=3", "-m", "--module", "-o", output], { encoding: "utf8" }),
  },
  esbuild: {
    id: "esbuild",
    tool: "esbuild",
    renames: "locals and top-level bindings; property names untouched",
    run: ({ tool, input, output }) => execFileSync(join(tool.home, ".bin/esbuild"),
      [input, "--minify", "--format=esm", "--target=es2020", `--outfile=${output}`], { encoding: "utf8" }),
  },
  oxc: {
    id: "oxc",
    tool: "rolldown",
    renames: "locals and top-level bindings through Rolldown's Oxc minifier; property names untouched",
    // Rolldown carries the Oxc minifier. Driving it through its own JS API
    // keeps the installed package the pinned input, with no extra wrapper.
    run: ({ tool, input, output }) => execFileSync(process.execPath, [join(toolsDirectory, "competitor-oxc.mjs"), tool.home, input, output], { encoding: "utf8" }),
  },
  closure: {
    id: "closure-advanced",
    tool: "google-closure-compiler",
    renames: "ADVANCED: locals, top-level bindings and property names, except what the externs preserve",
    // ADVANCED renames properties, so it needs the boundary's externs or it
    // deletes the public API. `externs` is supplied by the caller per boundary.
    run: ({ tool, input, output, externs }) => {
      const linux = join(dirname(tool.home), "node_modules/google-closure-compiler-linux/compiler")
      const binary = existsSync(linux) ? linux : join(tool.home, ".bin/google-closure-compiler")
      const args = ["--compilation_level", "ADVANCED", "--language_in", "ECMASCRIPT_2020", "--language_out", "ECMASCRIPT_2020",
        "--module_resolution", "NODE", "--js", input, "--js_output_file", output]
      if (externs) args.push("--externs", externs)
      return execFileSync(binary, args, { encoding: "utf8" })
    },
  },
}

/** Canonical raw/gzip/Brotli sizes from the repository's own codec. */
export function measure(path, codec = CODEC) {
  assert(existsSync(codec), `codec binary not found: ${codec}`)
  const report = JSON.parse(execFileSync(codec, ["--json", path], { encoding: "utf8" }))
  const row = report.artifacts[0]
  return { raw: row.raw ?? row.bytes, gzip: row.gzip9, brotli: row.brotli11 }
}

/** Bare specifiers our own artifact imports; the bar must leave them external. */
export function externalsOf(path) {
  const found = new Set()
  for (const match of readFileSync(path, "utf8").matchAll(/\bfrom\s*["']([^"']+)["']/g)) {
    if (!match[1].startsWith(".") && !match[1].startsWith("/")) found.add(match[1])
  }
  return [...found].sort()
}

/** The entry file an installed upstream package publishes for ESM consumers. */
export function upstreamEntry(directory) {
  const manifest = JSON.parse(readFileSync(join(directory, "package.json"), "utf8"))
  const exported = manifest.exports?.["."]
  const pick = value => (typeof value === "string" ? value : value?.import?.default ?? value?.import ?? value?.default)
  return pick(exported) ?? manifest.module ?? manifest.main ?? "index.js"
}

/**
 * Bundle one upstream package into a single ESM file, leaving external exactly
 * what our own artifact leaves external. This is the program every competitor
 * then minifies, so all of them see the same input.
 */
export function bundleUpstream({ packageDirectory, entry, externals, output }) {
  const tool = toolIdentity("esbuild")
  const args = [join(packageDirectory, entry), "--bundle", "--format=esm", "--target=es2020", `--outfile=${output}`,
    ...externals.map(name => `--external:${name}`)]
  execFileSync(join(tool.home, ".bin/esbuild"), args, { encoding: "utf8" })
  return { tool, args, output: { path: output, ...fileIdentity(output) } }
}

/**
 * Run every eligible recipe for one boundary and return a receipt.
 *
 * A recipe that fails is recorded with its reason and excluded from
 * `competitors`; it is never silently dropped, and an empty `competitors`
 * array makes the receipt ineligible rather than a free win.
 */
export function runCompetitors({ boundary, packageDirectory, ourArtifact, ourArtifactSource = "committed distribution", directory, recipes = ["terser", "esbuild", "oxc"], externs = null }) {
  mkdirSync(directory, { recursive: true })
  const started = new Date().toISOString()
  const codec = preserveBinary(CODEC)
  const receipt = {
    schema: 1, kind: "competitor-recipe-run", boundary, started, passed: false, codec,
    ourArtifact: { path: ourArtifact, source: ourArtifactSource, ...fileIdentity(ourArtifact) },
    upstream: { directory: packageDirectory }, competitors: [], refused: [],
    limitations: [
      "Byte construction only. These artifacts are not executed, so this receipt is not behavior evidence.",
      "Each recipe states its own renaming policy; the comparison is only as fair as that statement.",
      "The bundle leaves external exactly what our artifact imports. A boundary that bundles a different dependency graph is not comparable and says so.",
      `Our side of this comparison is the ${ourArtifactSource}. A committed distribution is only evidence about the compiler that produced it, which may not be the current one.`,
    ],
  }
  try {
    const manifest = JSON.parse(readFileSync(join(packageDirectory, "package.json"), "utf8"))
    const entry = upstreamEntry(packageDirectory)
    const externals = externalsOf(ourArtifact)
    receipt.upstream = { directory: packageDirectory, name: manifest.name, version: manifest.version, entry, externals }
    const bundle = join(directory, "upstream.bundle.mjs")
    receipt.bundle = bundleUpstream({ packageDirectory, entry, externals, output: bundle })
    receipt.ourSizes = measure(ourArtifact, codec.path)
    // The delivered-byte claim of this receipt is our artifact's own sizes.
    receipt.measurements = { ours: receipt.ourSizes }
    receipt.bundleSizes = measure(bundle, codec.path)
    receipt.measurements.reconstructedBundle = receipt.bundleSizes

    for (const name of recipes) {
      const recipe = RECIPES[name]
      if (!recipe) { receipt.refused.push({ id: name, reason: "no such recipe" }); continue }
      const output = join(directory, `${recipe.id}.min.js`)
      try {
        const tool = toolIdentity(recipe.tool)
        if (recipe.id === "closure-advanced" && !externs) {
          receipt.refused.push({ id: recipe.id, reason: "ADVANCED renames properties and no externs were supplied for this boundary; running it without them would delete the public API and produce a dishonest number" })
          continue
        }
        recipe.run({ tool, input: bundle, output, externs })
        assert(existsSync(output), `${recipe.id} produced no output`)
        receipt.competitors.push({
          id: `${tool.name}@${tool.version}/${recipe.id}`, recipe: recipe.id, tool,
          renames: recipe.renames, artifact: output, ...fileIdentity(output), sizes: measure(output, codec.path),
        })
      } catch (error) {
        receipt.refused.push({ id: recipe.id, reason: (error.stderr || error.message || String(error)).slice(0, 600) })
      }
    }
    // Two artifacts are the same boundary when they publish the same names.
    // A raw-size gulf is reported, but it is not the test: a port that emits a
    // much smaller program is the outcome we want, not a sign of cheating.
    const ourExports = exportedNames(ourArtifact)
    receipt.publicSurface = { ours: ourExports }
    for (const competitor of receipt.competitors) {
      competitor.exports = exportedNames(competitor.artifact)
      competitor.surfaceMatches = sameNames(ourExports, competitor.exports)
      competitor.missingFromOurs = competitor.exports.filter(name => !ourExports.includes(name))
      competitor.extraInOurs = ourExports.filter(name => !competitor.exports.includes(name))
      competitor.rawRatio = Number((competitor.sizes.raw / receipt.ourSizes.raw).toFixed(3))
    }
    // Two signals, both necessary. The names say whether it is the same API;
    // the raw mass says whether it is the same program. Several ports bundle a
    // pinned source graph rather than the npm package's dependency tree, and
    // the npm entry then yields a much smaller program — so a bar at 63% of our
    // raw size is not a bar we lost to, it is a different program.
    for (const competitor of receipt.competitors) {
      competitor.massMatches = competitor.rawRatio >= MASS_BAND[0] && competitor.rawRatio <= MASS_BAND[1]
      competitor.comparable = competitor.surfaceMatches && competitor.massMatches
    }
    receipt.comparable = receipt.competitors.length > 0 && receipt.competitors.every(competitor => competitor.comparable)
    if (!receipt.comparable) {
      const bySurface = receipt.competitors.filter(competitor => !competitor.surfaceMatches)
      const byMass = receipt.competitors.filter(competitor => competitor.surfaceMatches && !competitor.massMatches)
      if (bySurface.length) {
        const missing = [...new Set(bySurface.flatMap(competitor => competitor.missingFromOurs))].sort()
        const extra = [...new Set(bySurface.flatMap(competitor => competitor.extraInOurs))].sort()
        receipt.limitations.push(`NOT COMPARABLE (public surface): our artifact publishes ${ourExports.length} names, the reconstructed upstream ${bySurface[0].exports.length}.${missing.length ? ` Absent from ours: ${missing.join(", ")}.` : ""}${extra.length ? ` Only in ours: ${extra.join(", ")}.` : ""} A reduced export set is its own boundary, not a whole-library win (001).`)
      }
      if (byMass.length) {
        receipt.limitations.push(`NOT COMPARABLE (program mass): the reconstructed upstream is ${(byMass[0].rawRatio * 100).toFixed(1)}% of our artifact's raw size (${byMass[0].sizes.raw} against ${receipt.ourSizes.raw}), outside the ${MASS_BAND[0]}-${MASS_BAND[1]} band. This port bundles a different dependency graph than the npm entry does, so the two are not the same program.`)
      }
    }
    receipt.passed = receipt.competitors.length > 0
  } catch (error) {
    receipt.failure = { message: error.message, stack: error.stack }
  }
  receipt.completed = new Date().toISOString()
  receipt.fingerprint = fingerprint(receipt)
  writeFileSync(join(directory, "receipt.json"), JSON.stringify(receipt, null, 2) + "\n")
  return receipt
}

/**
 * The names an ES module publishes, read statically.
 *
 * Static reading cannot see a name produced at run time, and a `export *`
 * re-export is recorded as such rather than expanded; both are stated as
 * limitations instead of being guessed at.
 */
export function exportedNames(path) {
  const source = readFileSync(path, "utf8")
  const names = new Set()
  for (const match of source.matchAll(/export\s*{([^}]*)}/g)) {
    for (const part of match[1].split(",")) {
      const text = part.trim()
      if (!text) continue
      const alias = /\sas\s+([^\s]+)$/.exec(text)
      names.add((alias ? alias[1] : text).replace(/^["']|["']$/g, ""))
    }
  }
  for (const match of source.matchAll(/export\s+(?:async\s+)?(?:function\*?|class|const|let|var)\s+([A-Za-z_$][\w$]*)/g)) names.add(match[1])
  if (/export\s+default\b/.test(source)) names.add("default")
  if (/export\s*\*/.test(source)) names.add("*")
  return [...names].sort()
}

const sameNames = (left, right) => left.length === right.length && left.every((name, index) => name === right[index])

/** Every boundary the pinned baselines table can reconstruct from npm. */
export function reconstructableBoundaries() {
  const rows = []
  for (const [port, spec] of Object.entries(baselines(REPOSITORY_ROOT))) {
    const upstream = port.replace(/lil$/, "")
    const packageDirectory = join(homedir(), port, "node_modules", upstream)
    const ourArtifact = join(homedir(), port, spec.artifact ?? "")
    rows.push({
      port, upstream, packageDirectory, ourArtifact, pinnedTerserBrotli: spec.terserBrotli ?? null,
      reconstructable: Boolean(spec.artifact) && existsSync(packageDirectory) && existsSync(ourArtifact),
    })
  }
  return rows
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const argv = process.argv.slice(2)
  if (argv[0] === "--list") {
    for (const row of reconstructableBoundaries()) {
      console.log(`${row.reconstructable ? "ok " : "-- "} ${row.port.padEnd(30)} ${row.upstream.padEnd(26)} pinned=${row.pinnedTerserBrotli ?? "none"}`)
    }
    process.exit(0)
  }
  const port = argv[0]
  if (!port) { console.error("usage: competitor-recipes.mjs <port> [--out <dir>] [--recipes terser,esbuild,oxc,closure]"); process.exit(2) }
  const flag = name => { const at = argv.indexOf(name); return at < 0 ? null : argv[at + 1] }
  const row = reconstructableBoundaries().find(entry => entry.port === port)
  assert(row, `no baseline row for ${port}`)
  const directory = resolve(flag("--out") ?? join(REPOSITORY_ROOT, `benchmarks/migration-results/${new Date().toISOString().slice(0, 10)}-competitors/${port}`))
  const recipes = (flag("--recipes") ?? "terser,esbuild,oxc").split(",").filter(Boolean)
  const ours = flag("--ours")
  const receipt = runCompetitors({
    boundary: port, packageDirectory: row.packageDirectory,
    ourArtifact: ours ? resolve(ours) : row.ourArtifact,
    ourArtifactSource: ours ? "source-built artifact" : "committed distribution",
    directory, recipes, externs: flag("--externs"),
  })
  console.log(JSON.stringify({
    boundary: port, passed: receipt.passed, comparable: receipt.comparable, ours: receipt.ourSizes, oursFrom: receipt.ourArtifact?.source,
    competitors: receipt.competitors?.map(entry => ({ id: entry.id, brotli: entry.sizes.brotli, comparable: entry.comparable, surfaceMatches: entry.surfaceMatches, massMatches: entry.massMatches, rawRatio: entry.rawRatio })),
    limitations: receipt.limitations?.filter(note => note.startsWith("NOT COMPARABLE")),
    refused: receipt.refused, pinnedTerserBrotli: row.pinnedTerserBrotli, failure: receipt.failure?.message,
  }, null, 1))
  if (!receipt.passed) process.exitCode = 1
}
