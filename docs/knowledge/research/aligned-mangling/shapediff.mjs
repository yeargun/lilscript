#!/usr/bin/env node
/* Why is one build bigger than another?

   Two artifacts of the same program, compiled under different settings, differ
   somewhere specific. This prints an AST node histogram, a literal census and
   a few shape counts for each, and diffs them, so "the Brotli-cost build is
   2.5 KB bigger" turns into "it has N more of X".

   Usage: node shapediff.mjs <a.js> <b.js> */
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
const require = createRequire("/Users/yeargun/lilscript/benchmarks/popular/package.json");
const acorn = require("acorn");

export function shapeOf(source) {
  let ast;
  try { ast = acorn.parse(source, { ecmaVersion: 2022, sourceType: "module" }); }
  catch { ast = acorn.parse(source, { ecmaVersion: 2022, sourceType: "script" }); }

  const nodes = new Map();
  const strings = new Map();
  const numbers = new Map();
  const props = new Map();
  let functionBytes = 0, stringBytes = 0, deepest = 0;
  const bump = (map, key, by = 1) => map.set(key, (map.get(key) || 0) + by);

  const visit = (node, depth) => {
    if (!node || typeof node.type !== "string") return;
    bump(nodes, node.type);
    deepest = Math.max(deepest, depth);
    if (node.type === "Literal") {
      if (typeof node.value === "string") {
        bump(strings, node.value);
        stringBytes += node.end - node.start;
      } else if (typeof node.value === "number") bump(numbers, String(node.value));
    }
    if (node.type === "MemberExpression" && !node.computed && node.property.type === "Identifier") {
      bump(props, node.property.name);
    }
    if (/Function(Declaration|Expression)|ArrowFunctionExpression/.test(node.type)) {
      functionBytes += node.end - node.start;
    }
    for (const key of Object.keys(node)) {
      if (key === "type" || key === "start" || key === "end") continue;
      const value = node[key];
      if (Array.isArray(value)) { for (const child of value) if (child && child.type) visit(child, depth + 1); }
      else if (value && typeof value.type === "string") visit(value, depth + 1);
    }
  };
  visit(ast, 0);

  const total = (map) => [...map.values()].reduce((a, b) => a + b, 0);
  return {
    bytes: Buffer.byteLength(source),
    nodes, strings, numbers, props,
    nodeTotal: total(nodes),
    distinctStrings: strings.size,
    stringOccurrences: total(strings),
    stringBytes,
    distinctNumbers: numbers.size,
    distinctProps: props.size,
    propOccurrences: total(props),
    functionBytes,
    deepest,
  };
}

function diffMaps(a, b, label, limit = 18) {
  const keys = new Set([...a.keys(), ...b.keys()]);
  const rows = [];
  for (const key of keys) {
    const x = a.get(key) || 0, y = b.get(key) || 0;
    if (x !== y) rows.push({ key, x, y, d: y - x });
  }
  rows.sort((p, q) => Math.abs(q.d) - Math.abs(p.d));
  if (!rows.length) return;
  console.log(`\n${label} (only differences, biggest first):`);
  console.log("  " + "item".padEnd(30) + "A".padStart(8) + "B".padStart(8) + "B−A".padStart(8));
  for (const r of rows.slice(0, limit)) {
    console.log("  " + String(r.key).slice(0, 30).padEnd(30) + String(r.x).padStart(8) + String(r.y).padStart(8) +
      ((r.d > 0 ? "+" : "") + r.d).padStart(8));
  }
  if (rows.length > limit) console.log(`  … ${rows.length - limit} more`);
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const [, , fileA, fileB] = process.argv;
  if (!fileA || !fileB) { console.error("usage: node shapediff.mjs <a.js> <b.js>"); process.exit(2); }
  const A = shapeOf(readFileSync(fileA, "utf8"));
  const B = shapeOf(readFileSync(fileB, "utf8"));
  const name = (p) => p.split("/").pop();
  console.log(`A = ${name(fileA)}   B = ${name(fileB)}\n`);
  const line = (label, x, y) => console.log("  " + label.padEnd(26) + String(x).padStart(9) + String(y).padStart(9) +
    ((y - x > 0 ? "+" : "") + (y - x)).padStart(9));
  console.log("  " + "metric".padEnd(26) + "A".padStart(9) + "B".padStart(9) + "B−A".padStart(9));
  line("bytes", A.bytes, B.bytes);
  line("AST nodes", A.nodeTotal, B.nodeTotal);
  line("bytes inside functions", A.functionBytes, B.functionBytes);
  line("distinct string literals", A.distinctStrings, B.distinctStrings);
  line("string occurrences", A.stringOccurrences, B.stringOccurrences);
  line("bytes of string literals", A.stringBytes, B.stringBytes);
  line("distinct numbers", A.distinctNumbers, B.distinctNumbers);
  line("distinct property names", A.distinctProps, B.distinctProps);
  line("property occurrences", A.propOccurrences, B.propOccurrences);
  line("max AST depth", A.deepest, B.deepest);
  diffMaps(A.nodes, B.nodes, "AST node types");
  diffMaps(A.props, B.props, "property names", 12);

  /* string literals present in one and not the other */
  const onlyA = [...A.strings.keys()].filter((k) => !B.strings.has(k));
  const onlyB = [...B.strings.keys()].filter((k) => !A.strings.has(k));
  const show = (list, label) => {
    if (!list.length) return;
    console.log(`\n${label}: ${list.length}`);
    for (const s of list.sort((x, y) => y.length - x.length).slice(0, 10)) {
      console.log(`  ${JSON.stringify(s.slice(0, 70))}`);
    }
  };
  show(onlyA, "string literals only in A");
  show(onlyB, "string literals only in B");
}
