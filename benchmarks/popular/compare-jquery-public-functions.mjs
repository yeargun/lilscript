#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { parse } from "acorn";

const inputs = process.argv.length > 2
  ? process.argv.slice(2)
  : [
      "node_modules/jquery/dist/jquery.min.js",
      "build/jquery-config-audit/lean-balanced.terser.js",
    ];

function staticProperty(member) {
  if (!member || member.type !== "MemberExpression") return null;
  if (!member.computed && member.property.type === "Identifier") return member.property.name;
  if (member.computed && member.property.type === "Literal" && typeof member.property.value === "string") {
    return member.property.value;
  }
  return null;
}

function propertyName(property) {
  if (!property?.computed && property?.key?.type === "Identifier") return property.key.name;
  if (property?.key?.type === "Literal" && typeof property.key.value === "string") return property.key.value;
  return null;
}

function isFunction(node) {
  return node && ["FunctionExpression", "ArrowFunctionExpression"].includes(node.type);
}

function analyze(path) {
  const absolute = resolve(import.meta.dirname, path);
  const source = readFileSync(absolute, "utf8");
  const ast = parse(source, { ecmaVersion: "latest", sourceType: "module" });
  const found = new Map();
  const add = (name, node, kind) => {
    if (!name || !isFunction(node)) return;
    const row = { bytes: node.end - node.start, kind, start: node.start };
    const rows = found.get(name) ?? [];
    rows.push(row);
    found.set(name, rows);
  };

  function visit(node) {
    if (!node || typeof node !== "object") return;
    if (node.type === "AssignmentExpression") {
      add(staticProperty(node.left), node.right, "assignment");
    } else if (node.type === "Property") {
      add(propertyName(node), node.value, node.method ? "method" : "property");
    }
    for (const [key, value] of Object.entries(node)) {
      if (["start", "end", "loc"].includes(key)) continue;
      if (Array.isArray(value)) value.forEach(visit);
      else visit(value);
    }
  }
  visit(ast);
  return { path, found };
}

const reports = inputs.map(analyze);
const names = new Set(reports.flatMap((report) => [...report.found.keys()]));
const rows = [...names].map((name) => {
  const values = reports.map((report) => {
    const matches = report.found.get(name) ?? [];
    return matches.reduce((sum, match) => sum + match.bytes, 0);
  });
  return { name, values, delta: values[1] - values[0] };
}).filter((row) => row.values.some(Boolean));

console.log(`property\t${reports.map((report) => report.path).join("\t")}\tdelta`);
for (const row of rows.sort((left, right) => right.delta - left.delta).slice(0, 100)) {
  console.log(`${row.name}\t${row.values.join("\t")}\t${row.delta}`);
}
