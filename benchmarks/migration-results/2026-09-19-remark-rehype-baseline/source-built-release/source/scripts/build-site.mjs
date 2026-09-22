import { cp, mkdir, rm, writeFile } from "node:fs/promises"
import { existsSync } from "node:fs"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { spawnSync } from "node:child_process"

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..")
const output = join(root, "_site")
const file = "remark-rehype"

if (!existsSync(join(root, "dist", `${file}.esm.js`))) {
  const built = spawnSync(process.execPath, [join(root, "scripts", "build.mjs"), "--compile"], {
    cwd: root,
    stdio: "inherit",
  })
  if (built.status !== 0) process.exit(built.status ?? 1)
}

await rm(output, { recursive: true, force: true })
await mkdir(output, { recursive: true })
await cp(join(root, "site"), output, { recursive: true })
await cp(join(root, "dist", `${file}.esm.js`), join(output, `${file}.js`))
await writeFile(join(output, ".nojekyll"), "")
console.log(`Built GitHub Pages site at ${output}`)
