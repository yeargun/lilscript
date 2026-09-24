// The compiler has one route and no `--backend` flag. Binaries from before
// the migration (such as the frozen reference, whose default is the old
// route) list `--backend` in their help and need `--backend semantic`; ask
// the binary rather than assume.
import { spawnSync } from "node:child_process"

const cache = new Map()

export function routeArgs(compiler) {
  if (!cache.has(compiler)) {
    const help = spawnSync(compiler, ["--help"], { encoding: "utf8" })
    cache.set(compiler, /--backend\b/.test(help.stdout ?? "") ? ["--backend", "semantic"] : [])
  }
  return cache.get(compiler)
}
