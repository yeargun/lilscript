#!/usr/bin/env node
// Migration board tool. Reads and scaffolds docs/knowledge/migration/board/.
// The board is the durable state of the migration: what is open, what has already
// been tried, and what a fresh context or a subagent must read first.
//
// Usage:
//   node scripts/board.mjs status            ledger summary + next step of active tasks
//   node scripts/board.mjs check             structural gate; exits non-zero on drift
//   node scripts/board.mjs new <id> "title"  scaffold note + brief, print the ledger row
//   node scripts/board.mjs brief <id>        print the spawn prompt for a subagent

import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const boardDirectory = join(repositoryRoot, "docs/knowledge/migration/board");
const ledgerPath = join(boardDirectory, "LEDGER.md");
const notesDirectory = join(boardDirectory, "notes");
const briefsDirectory = join(boardDirectory, "briefs");
const templatesDirectory = join(boardDirectory, "templates");

const terminalStatuses = new Set(["landed", "rejected", "parked"]);
const statusPattern = /^(?:todo|active|landed|rejected|parked|ongoing|blocked\([a-z0-9.\-]+\))$/u;
const requiredNoteSections = ["## Question", "## Evidence", "## Log", "## Next step"];
const placeholderMarks = ["<", "TODO", "TBD"];

function readLedger() {
  if (!existsSync(ledgerPath)) {
    throw new Error(`missing ledger: ${ledgerPath}`);
  }
  const rows = [];
  let lane = "";
  for (const [index, line] of readFileSync(ledgerPath, "utf8").split("\n").entries()) {
    const laneHeading = /^##\s+([a-z0-9.\-]+)/u.exec(line);
    if (laneHeading) {
      lane = laneHeading[1];
      continue;
    }
    if (!line.startsWith("|")) continue;
    const cells = line.split(/(?<!\\)\|/u).slice(1, -1).map((cell) => cell.trim());
    const id = /^`([a-z0-9.\-]+)`$/u.exec(cells[0] ?? "")?.[1];
    if (!id || cells.length < 5) continue;
    rows.push({
      id,
      lane,
      status: cells[1],
      intent: cells[2],
      gate: cells[3],
      noteLink: /\(([^)]+)\)/u.exec(cells[4])?.[1] ?? "",
      line: index + 1,
    });
  }
  return rows;
}

function noteSection(notePath, heading) {
  if (!existsSync(notePath)) return "";
  const body = readFileSync(notePath, "utf8");
  const start = body.indexOf(`${heading}\n`);
  if (start < 0) return "";
  const rest = body.slice(start + heading.length);
  const end = rest.indexOf("\n## ");
  return (end < 0 ? rest : rest.slice(0, end)).trim();
}

function resolveNote(row) {
  return row.noteLink ? resolve(boardDirectory, row.noteLink) : "";
}

function status() {
  const rows = readLedger();
  const counts = new Map();
  for (const row of rows) {
    const key = row.status.startsWith("blocked") ? "blocked" : row.status;
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  console.log("Read order: board/README.md -> LEDGER.md -> the note of the ONE task you are doing.\n");
  console.log(`${rows.length} tasks: ${[...counts].map(([k, v]) => `${v} ${k}`).join(", ")}\n`);

  const open = rows.filter((row) => !terminalStatuses.has(row.status));
  for (const row of open) {
    console.log(`${row.status === "active" ? "*" : " "} ${row.id.padEnd(12)} ${row.status.padEnd(18)} ${row.intent.slice(0, 96)}`);
  }
  for (const row of rows.filter((entry) => entry.status === "active")) {
    const next = noteSection(resolveNote(row), "## Next step");
    console.log(`\n--- ${row.id} next step ---\n${next || "(none recorded — that is a check failure)"}`);
  }
  const blocked = open.filter((row) => row.status.startsWith("blocked"));
  if (blocked.length > 0) {
    console.log(`\nBlocked: ${blocked.map((row) => `${row.id} by ${/\(([^)]+)\)/u.exec(row.status)?.[1]}`).join(", ")}`);
  }
}

function check() {
  const rows = readLedger();
  const ids = new Set(rows.map((row) => row.id));
  const checkedNotes = new Map();
  const problems = [];

  for (const row of rows) {
    const where = `LEDGER.md:${row.line} ${row.id}`;
    if (!statusPattern.test(row.status)) {
      problems.push(`${where}: unknown status "${row.status}"`);
    }
    const blockedBy = /^blocked\(([a-z0-9.\-]+)\)$/u.exec(row.status)?.[1];
    if (blockedBy && !ids.has(blockedBy)) {
      problems.push(`${where}: blocked by "${blockedBy}", which is not a task`);
    }
    if (row.gate.length === 0) {
      problems.push(`${where}: no gate recorded — a task without a gate cannot be landed`);
    }
    const notePath = resolveNote(row);
    if (!notePath || !existsSync(notePath)) {
      problems.push(`${where}: note link does not resolve (${row.noteLink || "empty"})`);
      continue;
    }
    if (!terminalStatuses.has(row.status)) {
      const next = noteSection(notePath, "## Next step");
      if (next.length === 0 || placeholderMarks.some((mark) => next.startsWith(mark))) {
        problems.push(`${row.noteLink}: no concrete next step for a non-terminal task (${row.id})`);
      }
    }
    if (notePath.startsWith(notesDirectory)) {
      checkedNotes.set(notePath, row.noteLink);
    }
  }

  for (const [notePath, link] of checkedNotes) {
    problems.push(...checkNote(notePath, link));
  }

  for (const file of readdirSync(notesDirectory)) {
    const id = file.replace(/\.md$/u, "");
    if (!rows.some((row) => resolveNote(row) === join(notesDirectory, file))) {
      problems.push(`notes/${file}: orphan note, no ledger row links it (${id})`);
    }
  }

  if (problems.length > 0) {
    console.error(`board check failed (${problems.length}):`);
    for (const problem of problems) console.error(`  ${problem}`);
    process.exit(1);
  }
  console.log(`board check passed: ${rows.length} tasks, ${readdirSync(notesDirectory).length} notes.`);
}

// A note is validated once, however many ledger rows point at it: sections present,
// evidence tagged gate/diag, and every log item carrying a verdict. Log items wrap
// across lines, so they are joined before the verdict is looked for.
function checkNote(notePath, link) {
  const problems = [];
  const body = readFileSync(notePath, "utf8");
  for (const section of requiredNoteSections) {
    if (!body.includes(`${section}\n`)) {
      problems.push(`${link}: missing "${section}"`);
    }
  }
  for (const line of body.split("\n")) {
    if (!/^\|\s*\d{4}-\d{2}-\d{2}\s*\|/u.test(line)) continue;
    const cells = line.split(/(?<!\\)\|/u).slice(1, -1).map((cell) => cell.trim());
    if (!/^(?:gate|diag)$/u.test(cells.at(-1) ?? "")) {
      problems.push(`${link}: evidence row without a gate/diag tag: ${line.trim().slice(0, 64)}`);
    }
  }
  for (const item of listItems(noteSection(notePath, "## Log"))) {
    if (/^\d{4}-\d{2}-\d{2}/u.test(item) && !/\*\*(?:OPEN|LANDED|REJECTED)\*\*/u.test(item)) {
      problems.push(`${link}: log item without a verdict: ${item.slice(0, 64)}`);
    }
  }
  return problems;
}

function listItems(section) {
  const items = [];
  for (const line of section.split("\n")) {
    if (line.startsWith("- ")) items.push(line.slice(2));
    else if (items.length > 0 && line.trim().length > 0) items[items.length - 1] += ` ${line.trim()}`;
  }
  return items;
}

function scaffold(id, title) {
  if (!/^[a-z]+-\d+$/u.test(id)) {
    throw new Error(`id must look like <lane>-<nn>, got "${id}"`);
  }
  const created = [];
  for (const [directory, template] of [[notesDirectory, "note.md"], [briefsDirectory, "brief.md"]]) {
    const target = join(directory, `${id}.md`);
    if (existsSync(target)) {
      console.log(`kept existing ${target.replace(`${repositoryRoot}/`, "")}`);
      continue;
    }
    const body = readFileSync(join(templatesDirectory, template), "utf8")
      .replaceAll("{{id}}", id)
      .replaceAll("{{title}}", title);
    writeFileSync(target, body);
    created.push(target.replace(`${repositoryRoot}/`, ""));
  }
  for (const path of created) console.log(`created ${path}`);
  console.log(`\nPaste into LEDGER.md under the "${id.split("-")[0]}" lane:\n`);
  console.log(`| \`${id}\` | todo | ${title} | <gate> | [notes](notes/${id}.md) |`);
}

function spawnPrompt(id) {
  const briefPath = join(briefsDirectory, `${id}.md`);
  if (!existsSync(briefPath)) {
    throw new Error(`no brief for ${id}. Write one first: node scripts/board.mjs new ${id} "title"`);
  }
  console.log(
    [
      `Read these three files first, in order:`,
      `  1. docs/knowledge/mission.md`,
      `  2. docs/knowledge/migration/board/briefs/${id}.md`,
      `  3. docs/knowledge/migration/board/notes/${id}.md  (every REJECTED line is a dead end — do not retry it)`,
      ``,
      `Do exactly the task in the brief, within the files it says you may touch.`,
      `Run the command under "Prove it" and record what it actually printed.`,
      `Append to the note: one Evidence row per command (tagged gate or diag) and one Log`,
      `line ending in OPEN, LANDED, or REJECTED. Do not edit LEDGER.md.`,
      ``,
      `Then return at most 20 lines: what you changed, what the gate said, the next step.`,
    ].join("\n"),
  );
}

const [command, ...rest] = process.argv.slice(2);
try {
  if (command === "status" || command === undefined) status();
  else if (command === "check") check();
  else if (command === "new") scaffold(rest[0] ?? "", rest.slice(1).join(" ") || "untitled");
  else if (command === "brief") spawnPrompt(rest[0] ?? "");
  else {
    console.error(`unknown command "${command}". Try: status | check | new <id> "title" | brief <id>`);
    process.exit(2);
  }
} catch (error) {
  console.error(String(error.message ?? error));
  process.exit(2);
}
