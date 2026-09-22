import { execFileSync } from "node:child_process"
import { readFileSync } from "node:fs"

const json = execFileSync("npm", ["pack", "--dry-run", "--json"], { encoding: "utf8" })
const result = JSON.parse(json)[0]
const file = "remark-rehype"
const required = new Set([
  `dist/${file}.esm.js`,
  `dist/${file}.cjs`,
  `dist/${file}.umd.js`,
  `dist/${file}.closed.js`,
  `dist/${file}.d.ts`,
  "LICENSE",
  "NOTICE.md",
  "README.md",
])
const files = new Set(result.files.map(({ path }) => path))
for (const path of required) {
  if (!files.has(path)) throw new Error(`npm tarball is missing ${path}`)
}
const manifest = JSON.parse(readFileSync("package.json", "utf8"))
if (manifest.name !== "@itslil/remark-rehype") throw new Error("unexpected package name")
const expectedDependencies = ["@types/hast", "@types/mdast", "unified", "vfile"]
if (JSON.stringify(Object.keys(manifest.dependencies ?? {}).sort()) !== JSON.stringify(expectedDependencies)) {
  throw new Error("unexpected declaration dependencies")
}
console.log(`npm pack: ${result.entryCount} files, ${result.size} bytes packed, ${result.unpackedSize} bytes unpacked`)
