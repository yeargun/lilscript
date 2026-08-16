#!/usr/bin/env node

import { readFileSync, readdirSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parse } from "acorn";

const labRoot = dirname(fileURLToPath(import.meta.url));
const portRoot = join(labRoot, "ports/jquery");
const input = resolve(
  process.argv[2] ?? join(labRoot, "build/jquery-config-audit/lean-debug-names.js"),
);

function filesBelow(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? filesBelow(path) : [path];
  });
}

const declarationOwners = new Map();
function record(name, path) {
  const owners = declarationOwners.get(name) ?? new Set();
  owners.add(path);
  declarationOwners.set(name, owners);
}

for (const path of filesBelow(portRoot).filter((path) => path.endsWith(".lil"))) {
  const source = readFileSync(path, "utf8");
  for (const match of source.matchAll(/\b(?:class|struct)\s+([A-Za-z_$][\w$]*)/g)) {
    record(match[1], path);
  }
  for (const line of source.split("\n")) {
    const match = line.match(
      /^\s*(?:export\s+)?(?:pure\s+)?(?:async\s+)?(?:[A-Za-z_$][\w$]*(?:\s*<[^;{}=]+>)?(?:\[\])?\??|func\([^;{}=]*\)(?:->[^;{}=]+)?)\s+([A-Za-z_$][\w$]*)\s*\(/,
    );
    if (match && !["if", "for", "while", "switch", "catch"].includes(match[1])) {
      record(match[1], path);
    }
  }
}

const source = readFileSync(input, "utf8");
const program = parse(source, { ecmaVersion: "latest", sourceType: "module" });
const prefix = /^\$m(\d+)\$([A-Za-z_$][\w$]*)/;
const moduleEvidence = new Map();
const functionRows = [];

function ownerForGeneratedName(name) {
  const match = name?.match(prefix);
  if (!match) return null;
  const moduleId = Number(match[1]);
  const generatedBase = match[2];
  const candidates = [generatedBase];
  for (let index = generatedBase.lastIndexOf("$"); index > 0; index = generatedBase.lastIndexOf("$", index - 1)) {
    candidates.push(generatedBase.slice(0, index));
  }
  for (const candidate of candidates) {
    const owners = declarationOwners.get(candidate);
    if (owners?.size === 1) {
      const owner = [...owners][0];
      const evidence = moduleEvidence.get(moduleId) ?? new Map();
      evidence.set(owner, (evidence.get(owner) ?? 0) + 1);
      moduleEvidence.set(moduleId, evidence);
      return { moduleId, owner, generatedBase };
    }
  }
  return { moduleId, owner: null, generatedBase };
}

function functionName(node, fallback = null) {
  if (node.type === "FunctionDeclaration") return node.id?.name ?? fallback;
  return fallback;
}

function visit(node, inheritedModule = null, fallbackName = null) {
  if (!node || typeof node !== "object") return;
  const name = functionName(node, fallbackName);
  const direct = ownerForGeneratedName(name);
  const moduleId = direct?.moduleId ?? inheritedModule;
  if (
    node.type === "FunctionDeclaration" ||
    node.type === "FunctionExpression" ||
    node.type === "ArrowFunctionExpression"
  ) {
    functionRows.push({
      name: name ?? "<closure>",
      moduleId,
      start: node.start,
      end: node.end,
      bytes: node.end - node.start,
      topLevel: inheritedModule == null,
      node,
    });
  }

  for (const [key, value] of Object.entries(node)) {
    if (["start", "end", "loc"].includes(key)) continue;
    if (Array.isArray(value)) {
      for (const child of value) visit(child, moduleId, null);
    } else if (value && typeof value === "object") {
      let childName = null;
      if (
        node.type === "VariableDeclarator" &&
        key === "init" &&
        node.id?.type === "Identifier"
      ) {
        childName = node.id.name;
        ownerForGeneratedName(childName);
      }
      visit(value, moduleId, childName);
    }
  }
}

visit(program);

const moduleOwners = new Map();
for (const [moduleId, evidence] of moduleEvidence) {
  const ranked = [...evidence].sort((left, right) => right[1] - left[1]);
  if (ranked.length > 0) moduleOwners.set(moduleId, ranked[0][0]);
}

// Top-level generated functions include all their nested closures, so summing
// just those spans avoids double-counting closure bodies.
const totals = new Map();
for (const row of functionRows.filter((row) => row.topLevel && row.moduleId != null)) {
  const owner = moduleOwners.get(row.moduleId) ?? `<module ${row.moduleId}>`;
  const total = totals.get(owner) ?? { bytes: 0, functions: 0, rows: [] };
  total.bytes += row.bytes;
  total.functions += 1;
  total.rows.push(row);
  totals.set(owner, total);
}

const runtimeFunctionCounts = new Map();
for (const row of functionRows.filter((row) => row.moduleId != null)) {
  const owner = moduleOwners.get(row.moduleId) ?? `<module ${row.moduleId}>`;
  runtimeFunctionCounts.set(owner, (runtimeFunctionCounts.get(owner) ?? 0) + 1);
}

function syntaxCounts(root) {
  const counts = {
    assignments: 0,
    conditionals: 0,
    functions: 0,
    i32Coercions: 0,
    typeOf: 0,
  };
  function walk(node) {
    if (!node || typeof node !== "object") return;
    if (node.type === "AssignmentExpression") counts.assignments += 1;
    if (node.type === "ConditionalExpression") counts.conditionals += 1;
    if (
      node.type === "FunctionDeclaration" ||
      node.type === "FunctionExpression" ||
      node.type === "ArrowFunctionExpression"
    ) {
      counts.functions += 1;
    }
    if (node.type === "UnaryExpression" && node.operator === "typeof") {
      counts.typeOf += 1;
    }
    if (node.type === "BinaryExpression" && node.operator === "|" && node.right?.value === 0) {
      counts.i32Coercions += 1;
    }
    for (const [key, value] of Object.entries(node)) {
      if (["start", "end", "loc"].includes(key)) continue;
      if (Array.isArray(value)) value.forEach(walk);
      else if (value && typeof value === "object") walk(value);
    }
  }
  walk(root);
  return counts;
}

const syntaxTotals = new Map();
for (const row of functionRows.filter((row) => row.topLevel && row.moduleId != null)) {
  const owner = moduleOwners.get(row.moduleId) ?? `<module ${row.moduleId}>`;
  const current = syntaxTotals.get(owner) ?? {
    bytes: 0,
    assignments: 0,
    conditionals: 0,
    functions: 0,
    i32Coercions: 0,
    typeOf: 0,
  };
  const counts = syntaxCounts(row.node);
  current.bytes += row.bytes;
  for (const key of ["assignments", "conditionals", "functions", "i32Coercions", "typeOf"]) {
    current[key] += counts[key];
  }
  syntaxTotals.set(owner, current);
}

console.log("Largest emitted source modules (top-level function spans):");
for (const [owner, total] of [...totals].sort((left, right) => right[1].bytes - left[1].bytes).slice(0, 30)) {
  console.log(
    `${String(total.bytes).padStart(7)}  ${String(total.functions).padStart(3)}  ${relative(portRoot, owner)}`,
  );
}

console.log("\nLargest generated functions:");
for (const row of functionRows
  .filter((row) => row.topLevel)
  .sort((left, right) => right.bytes - left.bytes)
  .slice(0, 40)) {
  const owner = row.moduleId == null ? "<host>" : relative(portRoot, moduleOwners.get(row.moduleId) ?? `<module ${row.moduleId}>`);
  console.log(`${String(row.bytes).padStart(7)}  ${owner.padEnd(38)}  ${row.name}`);
}

console.log("\nMost runtime functions by source module (including closures):");
for (const [owner, count] of [...runtimeFunctionCounts]
  .sort((left, right) => right[1] - left[1])
  .slice(0, 30)) {
  console.log(`${String(count).padStart(5)}  ${relative(portRoot, owner)}`);
}

console.log("\nLargest dynamic/control-flow costs by source module:");
console.log("  bytes  funcs  assign typeof  i32  cond  module");
for (const [owner, counts] of [...syntaxTotals]
  .sort((left, right) =>
    (right[1].typeOf * 12 + right[1].assignments + right[1].i32Coercions * 4) -
    (left[1].typeOf * 12 + left[1].assignments + left[1].i32Coercions * 4)
  )
  .slice(0, 40)) {
  console.log(
    `${String(counts.bytes).padStart(7)} ${String(counts.functions).padStart(6)} ${String(counts.assignments).padStart(7)} ${String(counts.typeOf).padStart(6)} ${String(counts.i32Coercions).padStart(4)} ${String(counts.conditionals).padStart(5)}  ${relative(portRoot, owner)}`,
  );
}
