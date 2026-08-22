import { existsSync, mkdirSync, writeFileSync } from "node:fs"
import { dirname, resolve } from "node:path"
import { fileURLToPath, pathToFileURL } from "node:url"
import { spawnSync } from "node:child_process"
import { createRequire } from "node:module"

const portRoot = dirname(fileURLToPath(import.meta.url))
const labRoot = resolve(portRoot, "../..")
const repoRoot = resolve(labRoot, "../..")
const compiler = process.env.LILSCRIPT_COMPILER ?? resolve(repoRoot, "target/release/lilscript")
const codec = process.env.LILSCRIPT_CODEC ?? resolve(repoRoot, "target/release/lilscript-codec")
const require = createRequire(import.meta.url)
const officialMin = resolve(labRoot, "node_modules/mobx/dist/mobx.esm.production.min.js")
const outDir = resolve(portRoot, "dist")
const lilMin = resolve(outDir, "mobx.esm.production.min.js")

function run(program, args, cwd = portRoot) {
  const result = spawnSync(program, args, { cwd, encoding: "utf8", stdio: "inherit" })
  if (result.status !== 0) {
    process.exit(result.status ?? 1)
  }
}

function measurePath(path) {
  const result = spawnSync(codec, ["--json", path], { encoding: "utf8" })
  if (result.status !== 0) {
    throw new Error(`lilscript-codec failed for ${path}\n${result.stderr}`)
  }
  const parsed = JSON.parse(result.stdout)
  const artifact = parsed.artifacts?.[0] ?? parsed
  return {
    raw: artifact.raw,
    gzip9: artifact.gzip9 ?? artifact.gzip,
    brotli11: artifact.brotli11 ?? artifact.brotli,
  }
}

if (!existsSync(officialMin)) {
  throw new Error("install mobx@7.0.0 in benchmarks/popular to measure official production.min")
}

mkdirSync(outDir, { recursive: true })
run(compiler, [
  resolve(portRoot, "src/mobx.lil"),
  "--target",
  "js-module",
  "--config",
  resolve(portRoot, "lilscript.toml"),
  "-o",
  lilMin,
])

const loaded = await import(pathToFileURL(lilMin).href)
if (typeof loaded.observable !== "function" || typeof loaded.autorun !== "function") {
  throw new Error("Lil production.min failed to load")
}
const box = loaded.observable.box(1)
const doubled = loaded.computed(() => box.get() * 2)
const dispose = loaded.autorun(() => doubled.get())
box.set(2)
if (doubled.get() !== 4) {
  throw new Error(`Lil production.min computed failed: ${doubled.get()}`)
}
dispose()
await new Promise((resolve) => setTimeout(resolve, 80))

const official = Object.keys(require(resolve(labRoot, "node_modules/mobx/dist/index.js")))
  .filter((key) => require(resolve(labRoot, "node_modules/mobx/dist/index.js"))[key] !== undefined)
  .sort()
const lil = Object.keys(loaded)
  .filter((key) => loaded[key] !== undefined)
  .sort()
const missing = official.filter((name) => !lil.includes(name))
const extra = lil.filter((name) => !official.includes(name))
if (missing.length || extra.length) {
  throw new Error(`export surface mismatch\nmissing: ${missing.join(", ")}\nextra: ${extra.join(", ")}`)
}

const lanes = [
  { name: "lilscript-production-min", ...measurePath(lilMin) },
  { name: "official-mobx-esm-production-min", ...measurePath(officialMin) },
]
const report = { pin: "mobx@7.0.0", lanes }
writeFileSync(resolve(outDir, "compression.json"), JSON.stringify(report, null, 2) + "\n")
const lilLane = lanes[0]
const officialLane = lanes[1]
const summary = [
  `${lilLane.name}: raw=${lilLane.raw} gzip9=${lilLane.gzip9} brotli11=${lilLane.brotli11}`,
  `${officialLane.name}: raw=${officialLane.raw} gzip9=${officialLane.gzip9} brotli11=${officialLane.brotli11}`,
  `brotli ${lilLane.brotli11}/${officialLane.brotli11} = ${(lilLane.brotli11 / officialLane.brotli11).toFixed(3)}x`,
].join("\n")
writeFileSync(resolve(outDir, "compression.txt"), summary + "\n")
console.log(summary)
if (lilLane.brotli11 >= officialLane.brotli11) {
  console.error("Lil production.min Brotli did not beat official mobx.esm.production.min.js")
  process.exit(1)
}
