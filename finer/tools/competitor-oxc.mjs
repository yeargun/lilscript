// Drive Rolldown's Oxc minifier over one already-bundled ESM file.
//
// Kept as its own entry so the recipe in competitor-recipes.mjs stays a command
// with pinned inputs rather than an in-process import of a port's dependency.
import { writeFileSync } from "node:fs"
import { pathToFileURL } from "node:url"
import { dirname, join } from "node:path"

const [home, input, output] = process.argv.slice(2)
const { rolldown } = await import(pathToFileURL(join(home, "rolldown/dist/index.mjs")).href)
const bundle = await rolldown({
  input,
  // The input is already bundled with its externals decided; keep them external
  // here too so the minifier sees exactly the same program.
  external: id => !id.startsWith(".") && !id.startsWith("/"),
})
const { output: chunks } = await bundle.generate({ format: "esm", minify: true, dir: dirname(output) })
writeFileSync(output, chunks.filter(chunk => chunk.type === "chunk").map(chunk => chunk.code).join("\n"))
await bundle.close()
