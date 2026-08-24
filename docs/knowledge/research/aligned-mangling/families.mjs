#!/usr/bin/env node
/* Which of the raw-cost model's transform families would have helped Brotli?

   The two markedlil builds differ in four families at once, so "the raw build
   is smaller" does not say which family carries it. These probes apply one
   family at a time to the *Brotli*-cost artifact and score the result, so each
   family gets its own number under the gate codec.

   Families measured here are the byte-neutral-to-legal ones:
     - merge adjacent declarations of the same kind (`var a=1;var b=2` → `var a=1,b=2`)
     - `for(;test;)` → `while(test)` where init and update are empty
   Call-site outlining is not applied mechanically; its share is counted.

   Usage: node families.mjs <artifact.js> */
import { readFileSync, writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { createRequire } from "node:module";
import { gzipSync } from "node:zlib";
import { brotli } from "./census.mjs";
const require = createRequire("/Users/yeargun/lilscript/benchmarks/popular/package.json");
const acorn = require("acorn");

const parse = (source) => {
  try { return acorn.parse(source, { ecmaVersion: 2022, sourceType: "module" }); }
  catch { return acorn.parse(source, { ecmaVersion: 2022, sourceType: "script" }); }
};

/* Walk every statement list in the program. */
function eachBody(ast, fn) {
  const visit = (node) => {
    if (!node || typeof node.type !== "string") return;
    if (Array.isArray(node.body) && (node.type === "Program" || node.type === "BlockStatement" ||
        node.type === "StaticBlock")) fn(node.body);
    if (node.type === "SwitchCase" && Array.isArray(node.consequent)) fn(node.consequent);
    for (const key of Object.keys(node)) {
      if (key === "type" || key === "start" || key === "end") continue;
      const value = node[key];
      if (Array.isArray(value)) { for (const child of value) if (child && child.type) visit(child); }
      else if (value && typeof value.type === "string") visit(value);
    }
  };
  visit(ast);
}

const applyEdits = (source, edits) => {
  edits.sort((a, b) => a.start - b.start);
  let out = "", cursor = 0;
  for (const edit of edits) {
    if (edit.start < cursor) continue;
    out += source.slice(cursor, edit.start) + edit.text;
    cursor = edit.end;
  }
  return out + source.slice(cursor);
};

/* `var a=1;var b=2;` → `var a=1,b=2;`  — adjacent, same kind, statement level. */
export function mergeDeclarations(source) {
  const ast = parse(source);
  const edits = [];
  let merged = 0;
  eachBody(ast, (body) => {
    for (let i = 0; i + 1 < body.length; i++) {
      const a = body[i], b = body[i + 1];
      if (a.type !== "VariableDeclaration" || b.type !== "VariableDeclaration") continue;
      if (a.kind !== b.kind) continue;
      /* replace the gap `;<kind> ` between them with `,` */
      edits.push({ start: a.end - 1, end: b.declarations[0].start, text: "," });
      merged++;
      /* chain: treat b as already merged by continuing from it */
    }
  });
  return { text: applyEdits(source, edits), count: merged };
}

/* Outline repeated member calls: every `x.slice(a,b)` becomes `S1(x,a,b)`,
   with one helper declared at the top. This is the family the raw-cost model
   applies and the Brotli-cost model declines. */
export function outlineMemberCalls(source, names = ["slice", "exec", "replace"]) {
  const ast = parse(source);
  const edits = [];
  const used = new Map();
  const helperFor = (name) => `$${name[0]}${name.length}`;
  const visit = (node) => {
    if (!node || typeof node.type !== "string") return;
    if (node.type === "CallExpression" && node.callee && node.callee.type === "MemberExpression" &&
        !node.callee.computed && !node.callee.optional && !node.optional &&
        node.callee.property.type === "Identifier" && names.includes(node.callee.property.name) &&
        !node.arguments.some((a) => a.type === "SpreadElement")) {
      const name = node.callee.property.name;
      used.set(name, (used.get(name) || 0) + 1);
      /* `obj.name(` → `$helper(obj,`  and drop nothing else */
      const object = node.callee.object;
      edits.push({ start: node.start, end: object.start, text: `${helperFor(name)}(` });
      edits.push({
        start: object.end,
        end: node.arguments.length ? node.arguments[0].start : node.end - 1,
        text: node.arguments.length ? "," : "",
      });
    }
    for (const key of Object.keys(node)) {
      if (key === "type" || key === "start" || key === "end") continue;
      const value = node[key];
      if (Array.isArray(value)) { for (const child of value) if (child && child.type) visit(child); }
      else if (value && typeof value.type === "string") visit(value);
    }
  };
  visit(ast);
  if (!used.size) return { text: source, count: 0 };
  const helpers = [...used.keys()].map((n) => `${helperFor(n)}=(o,...a)=>o.${n}(...a)`).join(",");
  const body = applyEdits(source, edits);
  return { text: `let ${helpers};\n${body}`, count: [...used.values()].reduce((a, b) => a + b, 0) };
}

/* `for(;test;)` → `while(test)` when init and update are absent. */
export function forToWhile(source) {
  const ast = parse(source);
  const edits = [];
  let count = 0;
  const visit = (node) => {
    if (!node || typeof node.type !== "string") return;
    if (node.type === "ForStatement" && !node.init && !node.update && node.test) {
      edits.push({ start: node.start, end: node.test.start, text: "while(" });
      edits.push({ start: node.test.end, end: node.body.start, text: ")" });
      count++;
    }
    for (const key of Object.keys(node)) {
      if (key === "type" || key === "start" || key === "end") continue;
      const value = node[key];
      if (Array.isArray(value)) { for (const child of value) if (child && child.type) visit(child); }
      else if (value && typeof value.type === "string") visit(value);
    }
  };
  visit(ast);
  return { text: applyEdits(source, edits), count };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const file = process.argv[2];
  const source = readFileSync(file, "utf8");
  const CODEC = "/Users/yeargun/lilscript/target/release/lilscript-codec";
  const WORK = "/private/tmp/claude-501/-Users-yeargun-lilscript/68d4d12f-89ad-4d08-8494-1336a12a22e8/scratchpad";
  const score = (text) => ({
    raw: Buffer.byteLength(text), gzip9: gzipSync(Buffer.from(text), { level: 9 }).length,
    br11: brotli(text).length,
  });
  const base = score(source);
  console.log(`${file.split("/").pop()}: ${base.raw} raw / ${base.gzip9} gzip / ${base.br11} br11\n`);
  console.log("family".padEnd(30) + "sites".padStart(7) + "Δ raw".padStart(9) + "Δ gzip".padStart(9) + "Δ br11".padStart(9));

  const variants = [];
  const decls = mergeDeclarations(source);
  variants.push(["merge adjacent declarations", decls]);
  const loops = forToWhile(source);
  variants.push(["for(;t;) → while(t)", loops]);
  const outlined = outlineMemberCalls(source);
  variants.push(["outline .slice/.exec/.replace", outlined]);
  const combo = mergeDeclarations(outlineMemberCalls(source).text);
  variants.push(["outline + merge declarations", { text: combo.text, count: outlined.count + combo.count }]);

  const files = [];
  for (const [label, v] of variants) {
    /* re-parse to prove the rewrite is still valid JavaScript */
    try { parse(v.text); } catch (e) { console.log(`${label.padEnd(30)} INVALID: ${e.message}`); continue; }
    const s = score(v.text);
    const d = (k) => { const x = s[k] - base[k]; return (x > 0 ? "+" : "") + x; };
    console.log(label.padEnd(30) + String(v.count).padStart(7) + d("raw").padStart(9) + d("gzip9").padStart(9) + d("br11").padStart(9));
    const path = `${WORK}/family-${label.replace(/[^a-z]+/gi, "-")}.js`;
    writeFileSync(path, v.text);
    files.push(path);
  }
  if (files.length) {
    const json = JSON.parse(execFileSync(CODEC, ["--json", file, ...files], { encoding: "utf8" }));
    console.log("\nunder lilscript-codec:");
    const baseRow = json.artifacts[0];
    for (const a of json.artifacts) {
      console.log("  " + a.path.split("/").pop().padEnd(42) + String(a.raw).padStart(7) + String(a.gzip9).padStart(8) +
        String(a.brotli11).padStart(8) + (a === baseRow ? "  (baseline)" : `  Δbr ${a.brotli11 - baseRow.brotli11}`));
    }
  }
}
