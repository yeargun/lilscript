// Rebuild each arm's shipped ESM exactly as the port's build script does, then
// measure it with the canonical codec: katex.esm.js (banner + font metrics +
// compiled code, exports filtered) and marked.esm.js (banner + compiled code).
// zodlil ships dist/zod.core.js as compiled; probelil ships no stitched file.
import { readFileSync, writeFileSync, existsSync, mkdirSync } from "node:fs"
import { execFileSync } from "node:child_process"
const SP = "/tmp/claude-1000/-home-azureuser-lilscript/3de072ae-233a-45f7-a7d6-9d07057b517c/scratchpad"
const arms = ["old-legacy", "old-legacy-samecfg", "new-legacy", "new-legacy-samecfg", "new-semantic"]
const katexBanner = "/*! @itslil/katex 0.16.22 | LilScript reimplementation of katex | MIT */\n"
const markedBanner = "/*! @itslil/marked 18.0.10 | LilScript reimplementation of marked 18.0.10 | MIT */\n"
function stitchKatex(compiled) {
  const metrics = readFileSync(`${process.env.HOME}/katexlil/src/fontMetricsData.js`, "utf8")
    .replace(/\bexport\s*\{[^}]*\}/g, "")
    .replace(/\bexport\s+default\s+/, "var fontMetricsData=")
    .replace(/\bvar e=/, "var fontMetricsData=")
    .trim()
  const body = compiled.replace(/import\{default as generatedFontMetricsData\}from["']\.\/fontMetricsData\.js["'];/, "")
  return `${metrics}\nvar generatedFontMetricsData=fontMetricsData;${body}\nconst version="0.16.22";export{version};`
}
const allowed = new Set("ParseError,SETTINGS_SCHEMA,__defineFunction,__defineMacro,__defineSymbol,__domTree,__parse,__renderToDomTree,__renderToHTMLTree,__setFontMetrics,default,render,renderToString,version".split(","))
const filterExports = (source) => source.replace(/export\s*\{([^}]*)\}/g, (_, body) => {
  const entries = body.split(",").filter((entry) => { const parts = entry.trim().split(/\s+as\s+/); return allowed.has(parts[parts.length - 1]) })
  return entries.length ? `export{${entries.join(",")}}` : ""
})
const rows = []
for (const arm of arms) {
  for (const [port, make, name] of [
    ["katexlil", (raw) => filterExports(`${katexBanner}${stitchKatex(raw).trimEnd()}\n`), "katex.esm.js"],
    ["markedlil", (raw) => `${markedBanner}${raw.trimEnd()}\n`, "marked.esm.js"],
    ["zodlil", (raw) => raw, "zod.core.js"],
  ]) {
    const raw = `${SP}/cmp/out/${arm}/${port}.js`
    if (!existsSync(raw)) continue
    mkdirSync(`${SP}/cmp/shipped/${arm}`, { recursive: true })
    const out = `${SP}/cmp/shipped/${arm}/${name}`
    writeFileSync(out, make(readFileSync(raw, "utf8")))
    const m = JSON.parse(execFileSync(`${SP}/bin/lilscript-codec`, ["--json", out], { encoding: "utf8" })).artifacts[0]
    rows.push({ arm, port, artifact: name, raw: m.raw, gzip9: m.gzip9, brotli11: m.brotli11 })
  }
}
writeFileSync(`${SP}/cmp/shipped.json`, JSON.stringify(rows, null, 1))
for (const r of rows) console.log(r.arm.padEnd(20), r.port.padEnd(10), r.artifact.padEnd(14), "raw", r.raw, "br", r.brotli11)
