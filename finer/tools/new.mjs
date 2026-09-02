#!/usr/bin/env node
// Open the next hypothesis folder, or check that folders and log.md agree.
//
//   node finer/tools/new.mjs <slug> --lane <lang|port|compiler|measure>
//       creates finer/hypotheses/NNN-<slug>/README.md from the template and
//       prints the log.md row to paste at the top of the table
//   node finer/tools/new.mjs check
//       every folder has a log.md row and every row has a folder; folders
//       opened from the template also carry a Status line and stay under the
//       size budget. Exits non-zero on drift.
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"

const finer = resolve(dirname(fileURLToPath(import.meta.url)), "..")
const hypotheses = join(finer, "hypotheses")
const logPath = join(finer, "log.md")
const templatePath = join(finer, "templates", "hypothesis.md")
const lanes = ["lang", "port", "compiler", "measure"]
// Folders below this number predate the template; they are held to the row
// rule only. Everything from here on carries a Status line and a size budget.
const templatedFrom = 38
// Folders from here on were opened after the owner's brief of 2026-09-01 made reading the
// competitors' source a precondition (objective.md §10): they carry a Prior art section.
const priorArtFrom = 40
const sizeBudget = 12_000

const folders = () => readdirSync(hypotheses).filter((name) => /^\d{3}-/u.test(name)).sort()
const fail = (message) => { console.error(message); process.exit(1) }

function check() {
  const log = readFileSync(logPath, "utf8")
  const rows = new Set([...log.matchAll(/^\| \[(\d{3})\]\(hypotheses\/\d{3}-[^)]+\) \| (\w+) \|/gmu)].map((m) => m[1]))
  const badLane = [...log.matchAll(/^\| \[(\d{3})\]\([^)]+\) \| (\w+) \|/gmu)].filter((m) => !lanes.includes(m[2]))
  const failures = badLane.map((m) => `log.md row ${m[1]}: lane "${m[2]}" is not one of ${lanes.join(", ")}`)
  for (const name of folders()) {
    const number = name.slice(0, 3)
    const readme = join(hypotheses, name, "README.md")
    if (!existsSync(readme)) { failures.push(`${name}: no README.md`); continue }
    if (!rows.has(number)) failures.push(`${name}: no row in log.md`)
    rows.delete(number)
    if (Number(number) < templatedFrom) continue
    const text = readFileSync(readme, "utf8")
    if (!/^\*\*Status: /mu.test(text)) failures.push(`${name}: no "**Status: ...**" line`)
    if (Number(number) >= priorArtFrom && !/^## Prior art$/mu.test(text)) {
      failures.push(`${name}: no "## Prior art" section (objective.md §10)`)
    }
    if (text.length > sizeBudget) failures.push(`${name}: README is ${text.length} bytes; split it or move data to out/`)
    if (/<[^>`\n]*>/u.test(text) && /^\*\*Status: OPEN/mu.test(text) === false) {
      failures.push(`${name}: template placeholders remain but Status is not OPEN`)
    }
  }
  for (const orphan of rows) failures.push(`log.md row ${orphan} has no folder`)
  if (failures.length) fail(failures.join("\n"))
  console.log(`ok: ${folders().length} folders, log.md in step`)
}

function create(slug, lane) {
  if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/u.test(slug)) fail("slug must be kebab-case, naming the finding")
  if (!lanes.includes(lane)) fail(`--lane must be one of ${lanes.join(", ")}`)
  const last = folders().at(-1)
  const number = String(Number(last?.slice(0, 3) ?? 0) + 1).padStart(3, "0")
  const name = `${number}-${slug}`
  const dir = join(hypotheses, name)
  if (existsSync(dir)) fail(`${dir} exists`)
  mkdirSync(dir)
  const text = readFileSync(templatePath, "utf8")
    .replaceAll("{{number}}", number)
    .replaceAll("{{lane}}", lane)
    .replaceAll("{{title}}", slug.replaceAll("-", " "))
    .replaceAll("{{date}}", new Date().toISOString().slice(0, 10))
  writeFileSync(join(dir, "README.md"), text)
  console.log(`opened finer/hypotheses/${name}/README.md`)
  console.log("log.md row, newest first so it goes at the top of the table:")
  console.log(`| [${number}](hypotheses/${name}/README.md) | ${lane} | <question> | **OPEN** |`)
}

const argv = process.argv.slice(2)
if (argv[0] === "check") check()
else if (argv[0] && !argv[0].startsWith("--")) {
  const at = argv.indexOf("--lane")
  create(argv[0], at === -1 ? "" : argv[at + 1])
} else fail("usage: new.mjs <slug> --lane <lane> | new.mjs check")
