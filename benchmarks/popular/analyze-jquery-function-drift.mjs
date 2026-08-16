#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { parse } from "acorn";
import { TraceMap, eachMapping } from "@jridgewell/trace-mapping";

const labRoot = import.meta.dirname;
const npmSourcePath = resolve(labRoot, "node_modules/jquery/dist/jquery.js");
const npmMinPath = resolve(labRoot, "node_modules/jquery/dist/jquery.min.js");
const npmMapPath = resolve(labRoot, "node_modules/jquery/dist/jquery.min.map");
const lilPath = resolve(
  process.argv[2] ?? "build/jquery-config-audit/lean-debug-names.js",
);

function lineStarts(source) {
  const starts = [0];
  for (let index = 0; index < source.length; index += 1) {
    if (source.charCodeAt(index) === 10) starts.push(index + 1);
  }
  return starts;
}

function inferredName(node, parent) {
  if (node.id?.name) return node.id.name;
  if (parent?.type === "VariableDeclarator" && parent.id.type === "Identifier") {
    return parent.id.name;
  }
  if (parent?.type === "AssignmentExpression") {
    const target = parent.left;
    if (target.type === "Identifier") return target.name;
    if (target.type === "MemberExpression") {
      if (!target.computed && target.property.type === "Identifier") return target.property.name;
      if (target.computed && target.property.type === "Literal") return String(target.property.value);
    }
  }
  if (parent?.type === "Property") {
    if (parent.key.type === "Identifier") return parent.key.name;
    if (parent.key.type === "Literal") return String(parent.key.value);
  }
  if (parent?.type === "MethodDefinition") {
    if (parent.key.type === "Identifier") return parent.key.name;
    if (parent.key.type === "Literal") return String(parent.key.value);
  }
  return null;
}

function functionNodes(source, sourceType) {
  const ast = parse(source, { ecmaVersion: "latest", sourceType });
  const functions = [];
  function visit(node, parent = null) {
    if (!node || typeof node !== "object") return;
    if (
      node.type === "FunctionDeclaration" ||
      node.type === "FunctionExpression" ||
      node.type === "ArrowFunctionExpression"
    ) {
      functions.push({
        name: inferredName(node, parent),
        start: node.start,
        end: node.end,
        bytes: node.end - node.start,
      });
    }
    for (const [key, value] of Object.entries(node)) {
      if (["start", "end", "loc"].includes(key)) continue;
      if (Array.isArray(value)) value.forEach((child) => visit(child, node));
      else if (value && typeof value === "object") visit(value, node);
    }
  }
  visit(ast);
  return functions;
}

function innermostAt(functions, offset) {
  let best = null;
  for (const fn of functions) {
    if (fn.start <= offset && offset < fn.end && (!best || fn.bytes < best.bytes)) best = fn;
  }
  return best;
}

function add(map, key, amount) {
  map.set(key, (map.get(key) ?? 0) + amount);
}

const npmSource = readFileSync(npmSourcePath, "utf8");
const npmMin = readFileSync(npmMinPath, "utf8");
const npmFunctions = functionNodes(npmSource, "script");
const starts = lineStarts(npmSource);
const generatedStarts = lineStarts(npmMin);
const mappings = [];
eachMapping(new TraceMap(JSON.parse(readFileSync(npmMapPath, "utf8"))), (mapping) => {
  if (mapping.originalLine == null) return;
  mappings.push({
    ...mapping,
    generatedOffset: generatedStarts[mapping.generatedLine - 1] + mapping.generatedColumn,
  });
});
mappings.sort((left, right) => left.generatedOffset - right.generatedOffset);

const npmByName = new Map();
for (let index = 0; index < mappings.length; index += 1) {
  const mapping = mappings[index];
  const end = mappings[index + 1]?.generatedOffset ?? npmMin.length;
  const bytes = Math.max(0, end - mapping.generatedOffset);
  const originalOffset = starts[mapping.originalLine - 1] + mapping.originalColumn;
  const fn = innermostAt(npmFunctions, originalOffset);
  const name = mapping.name ?? fn?.name;
  if (name) add(npmByName, name, bytes);
}

const lilSource = readFileSync(lilPath, "utf8");
const lilFunctions = functionNodes(lilSource, "module");
const lilByName = new Map();
for (const fn of lilFunctions) {
  if (!fn.name) continue;
  const generated = fn.name.match(/^\$m\d+\$(.+)$/)?.[1] ?? fn.name;
  const base = generated.split("$")[0];
  add(lilByName, base, fn.bytes);
}

const rows = [];
for (const [name, lilBytes] of lilByName) {
  const npmBytes = npmByName.get(name);
  if (npmBytes == null || npmBytes === 0) continue;
  rows.push({ name, lilBytes, npmBytes, delta: lilBytes - npmBytes, ratio: lilBytes / npmBytes });
}
rows.sort((left, right) => right.delta - left.delta);

console.log("  lil   npm delta ratio  function");
for (const row of rows.slice(0, 100)) {
  console.log(
    `${String(row.lilBytes).padStart(5)} ${String(row.npmBytes).padStart(5)} ` +
      `${String(row.delta).padStart(5)} ${row.ratio.toFixed(2).padStart(5)}  ${row.name}`,
  );
}

console.log(
  JSON.stringify({
    matchedFunctions: rows.length,
    matchedLilBytes: rows.reduce((sum, row) => sum + row.lilBytes, 0),
    matchedNpmBytes: rows.reduce((sum, row) => sum + row.npmBytes, 0),
  }),
);
