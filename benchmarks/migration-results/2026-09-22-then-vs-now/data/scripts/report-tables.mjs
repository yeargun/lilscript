// Markdown tables from the comparison arms.
import { readFileSync, existsSync } from "node:fs"
const SP = "/tmp/claude-1000/-home-azureuser-lilscript/3de072ae-233a-45f7-a7d6-9d07057b517c/scratchpad"
const ports = ["markedlil", "zodlil", "katexlil", "probelil"]
const read = (arm, port) => {
  const path = `${SP}/cmp/out/${arm}/${port}.json`
  if (!existsSync(path)) return null
  return JSON.parse(readFileSync(path, "utf8").replace(/"seconds":\./, '"seconds":0.'))
}
const fmt = (n) => n.toLocaleString("en-US")
const pct = (a, b) => {
  const share = (Math.abs(a - b) / b) * 100
  const sign = a - b >= 0 ? "+" : "−"
  return `${sign}${fmt(Math.abs(a - b))} (${sign}${share.toFixed(share < 0.1 ? 2 : 1)}%)`
}
const cell = (r, key) => (r && r.status === 0 ? fmt(r.sizes.artifacts[0][key]) : r ? "fails" : "–")
for (const [key, label] of [["brotli11", "Brotli 11"], ["gzip9", "gzip 9"], ["raw", "raw"]]) {
  console.log(`\n**${label} bytes**\n`)
  console.log("| Library | Pre-migration (06b8da20) | Now, default route (b79efb1) | Now, semantic route (b79efb1) | Default route: now vs before | Semantic vs default route now |")
  console.log("|---|---:|---:|---:|---:|---:|")
  for (const port of ports) {
    // katexlil's pre-migration arm drops the one config line that compiler rejects.
    const old = port === "katexlil" ? read("old-legacy-samecfg", port) : read("old-legacy", port)
    const now = read("new-legacy", port), sem = read("new-semantic", port)
    const v = (r) => (r && r.status === 0 ? r.sizes.artifacts[0][key] : null)
    const d1 = v(old) != null && v(now) != null ? pct(v(now), v(old)) : "–"
    const d2 = v(sem) != null && v(now) != null ? pct(v(sem), v(now)) : "–"
    const mark = port === "katexlil" ? "\\*" : ""
    const semMark = port === "katexlil" ? " †" : ""
    console.log(`| ${port} | ${cell(old, key)}${mark} | ${cell(now, key)} | ${cell(sem, key)}${semMark} | ${d1} | ${d2} |`)
  }
}
console.log("\n**Compile time on this host (seconds, wall clock, one run)**\n")
console.log("| Library | Pre-migration | Now, default route | Now, semantic route |")
console.log("|---|---:|---:|---:|")
for (const port of ports) {
  const t = (r) => (r ? (r.status === 0 ? Number(r.seconds).toFixed(1) : "fails") : "–")
  const old = port === "katexlil" ? read("old-legacy-samecfg", port) : read("old-legacy", port)
  console.log(`| ${port} | ${t(old)} | ${t(read("new-legacy", port))} | ${t(read("new-semantic", port))} |`)
}

// katexlil's shipped config names `name_ordering`, which the pre-migration
// compiler does not know. The like-for-like pair drops that one line for both.
{
  // The current compiler accepts `name_ordering` and ignores it (it says so), so
  // its shipped-config build is the like-for-like "now" arm.
  const old = read("old-legacy-samecfg", "katexlil"), now = read("new-legacy", "katexlil")
  const sem = read("new-semantic", "katexlil")
  const v = (r, key) => (r && r.status === 0 ? r.sizes.artifacts[0][key] : null)
  console.log("\n**katexlil, like for like** (the pre-migration compiler rejects the `name_ordering` line, which the current compiler accepts and ignores; the pre-migration arm drops it)\n")
  console.log("| Codec | Pre-migration | Now, default route | Change | Now, semantic route | Semantic vs default now |")
  console.log("|---|---:|---:|---:|---:|---:|")
  for (const [key, label] of [["brotli11", "Brotli 11"], ["gzip9", "gzip 9"], ["raw", "raw"]]) {
    const a = v(old, key), b = v(now, key), c = v(sem, key)
    console.log(`| ${label} | ${a != null ? fmt(a) : "–"} | ${b != null ? fmt(b) : "–"} | ${a != null && b != null ? pct(b, a) : "–"} | ${c != null ? fmt(c) : "–"} | ${b != null && c != null ? pct(c, b) : "–"} |`)
  }
  const t = (r) => (r ? (r.status === 0 ? Number(r.seconds).toFixed(1) : "fails") : "–")
  console.log(`| compile seconds | ${t(old)} | ${t(now)} | | ${t(sem)} | |`)
}
