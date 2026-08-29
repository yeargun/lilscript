import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, extname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const docsRoot = join(root, "docs");
const entry = join(docsRoot, "README.md");

function markdownFiles(directory) {
  const files = [];
  for (const name of readdirSync(directory)) {
    const path = join(directory, name);
    if (statSync(path).isDirectory()) files.push(...markdownFiles(path));
    else if (extname(path) === ".md") files.push(path);
  }
  return files;
}

function linksIn(file) {
  const source = readFileSync(file, "utf8")
    .replace(/```[\s\S]*?```/gu, "")
    .replace(/~~~[\s\S]*?~~~/gu, "")
    .replace(/`[^`\n]*`/gu, "");
  return [...source.matchAll(/\[[^\]]*\]\(([^)]+)\)/gu)]
    .map((match) => match[1].trim().replace(/^<|>$/gu, ""))
    .filter(
      (target) =>
        target &&
        !target.includes("{{") &&
        !target.startsWith("#") &&
        !/^[a-z][a-z0-9+.-]*:/iu.test(target),
    );
}

function targetPath(file, target) {
  const relativeTarget = decodeURIComponent(target.split("#", 1)[0]);
  return resolve(dirname(file), relativeTarget);
}

const files = [
  join(root, "README.md"),
  join(root, "why-lilscript.md"),
  ...markdownFiles(docsRoot),
];
const graph = new Map();
const failures = [];

for (const file of files) {
  const targets = [];
  for (const link of linksIn(file)) {
    const target = targetPath(file, link);
    if (!existsSync(target)) {
      failures.push(`${relative(root, file)}: missing ${link}`);
      continue;
    }
    if (extname(target) === ".md") targets.push(target);
  }
  graph.set(file, targets);
}

const reachable = new Set();
const pending = [entry];
while (pending.length > 0) {
  const file = pending.pop();
  if (reachable.has(file)) continue;
  reachable.add(file);
  for (const target of graph.get(file) ?? []) pending.push(target);
}

function isColdStorage(file) {
  const path = relative(docsRoot, file);
  return (
    path.startsWith(join("knowledge", "research")) ||
    path.startsWith(join("knowledge", "migration", "board", "notes")) ||
    path.startsWith(join("knowledge", "migration", "board", "briefs")) ||
    path.startsWith(join("knowledge", "migration", "board", "templates")) ||
    path === join("knowledge", "migration", "board", "JOURNAL.md")
  );
}

const canonicalFiles = files.filter(
  (file) => file.startsWith(docsRoot) && !isColdStorage(file),
);
for (const file of canonicalFiles) {
  if (!reachable.has(file)) {
    failures.push(`${relative(root, file)}: not reachable from docs/README.md`);
  }
}

if (failures.length > 0) {
  console.error(failures.join("\n"));
  process.exit(1);
}

console.log(
  `documentation graph valid: ${files.length} Markdown files, ${canonicalFiles.length} canonical pages reachable`,
);
