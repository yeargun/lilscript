#!/usr/bin/env node

import { readFileSync, readdirSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parse } from "acorn";
import { minifySync } from "rolldown/experimental";
import { canonicalCodecSizes } from "../codec-contract.mjs";

const labRoot = dirname(fileURLToPath(import.meta.url));
const portRoot = join(labRoot, "ports/jquery");
const upstreamRoot = join(labRoot, "node_modules/jquery/src");
const lilPath = resolve(process.argv[2] ?? "/tmp/jquery-current-named.js");
const detailSelector = process.argv[3] ?? null;

function filesBelow(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? filesBelow(path) : [path];
  });
}

function metrics(source) {
  return canonicalCodecSizes(source, "jQuery submodule attribution diagnostic");
}

function visit(node, callback, parent = null, key = null) {
  if (!node || typeof node !== "object") return;
  callback(node, parent, key);
  for (const [childKey, value] of Object.entries(node)) {
    if (["start", "end", "loc", "range"].includes(childKey)) continue;
    if (Array.isArray(value)) {
      for (const child of value) visit(child, callback, node, childKey);
    } else if (value && typeof value === "object") {
      visit(value, callback, node, childKey);
    }
  }
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
  if (parent?.type === "Property" || parent?.type === "MethodDefinition") {
    if (parent.key.type === "Identifier") return parent.key.name;
    if (parent.key.type === "Literal") return String(parent.key.value);
  }
  return null;
}

function isFunction(node) {
  return ["FunctionDeclaration", "FunctionExpression", "ArrowFunctionExpression"].includes(
    node?.type,
  );
}

function collectFunctions(program, include) {
  const functions = [];
  visit(program, (node, parent) => {
    if (!isFunction(node) || !include(node, parent)) return;
    functions.push({
      name: inferredName(node, parent),
      params: node.params.length,
      node,
    });
  });
  return functions;
}

function recordDeclarationOwners() {
  const owners = new Map();
  const record = (name, path) => {
    const paths = owners.get(name) ?? new Set();
    paths.add(path);
    owners.set(name, paths);
  };
  for (const path of filesBelow(portRoot).filter((path) => path.endsWith(".lil"))) {
    const source = readFileSync(path, "utf8");
    for (const match of source.matchAll(/\b(?:class|struct)\s+([A-Za-z_$][\w$]*)/gu)) {
      record(match[1], path);
    }
    for (const line of source.split("\n")) {
      const match = line.match(
        /^\s*(?:export\s+)?(?:pure\s+)?(?:async\s+)?(?:[A-Za-z_$][\w$]*(?:\s*<[^;{}=]+>)?(?:\[\])?\??|func\([^;{}=]*\)(?:->[^;{}=]+)?)\s+([A-Za-z_$][\w$]*)\s*\(/u,
      );
      if (match && !["if", "for", "while", "switch", "catch"].includes(match[1])) {
        record(match[1], path);
      }
    }
  }
  return owners;
}

const generatedPrefix = /^\$m(\d+)\$([A-Za-z_$][\w$]*)/u;
const declarationOwners = recordDeclarationOwners();
const lilSource = readFileSync(lilPath, "utf8");
const lilProgram = parse(lilSource, { ecmaVersion: "latest", sourceType: "module" });
const moduleEvidence = new Map();

function generatedIdentity(name) {
  const match = name?.match(generatedPrefix);
  if (!match) return null;
  const moduleId = Number(match[1]);
  const generatedBase = match[2];
  const candidates = [generatedBase];
  for (
    let index = generatedBase.lastIndexOf("$");
    index > 0;
    index = generatedBase.lastIndexOf("$", index - 1)
  ) {
    candidates.push(generatedBase.slice(0, index));
  }
  for (const candidate of candidates) {
    const owners = declarationOwners.get(candidate);
    if (owners?.size === 1) {
      const owner = [...owners][0];
      const evidence = moduleEvidence.get(moduleId) ?? new Map();
      evidence.set(owner, (evidence.get(owner) ?? 0) + 1);
      moduleEvidence.set(moduleId, evidence);
      return { moduleId, generatedBase, sourceName: candidate, owner };
    }
  }
  return { moduleId, generatedBase, sourceName: generatedBase.split("$", 1)[0], owner: null };
}

const lilFunctions = collectFunctions(
  lilProgram,
  (node) => node.type === "FunctionDeclaration" && generatedIdentity(node.id?.name),
);
for (const fn of lilFunctions) generatedIdentity(fn.name);
const moduleOwners = new Map(
  [...moduleEvidence].map(([moduleId, evidence]) => [
    moduleId,
    [...evidence].sort((left, right) => right[1] - left[1])[0]?.[0],
  ]),
);

const standardGlobals = new Set([
  "Array",
  "BigInt",
  "Boolean",
  "Date",
  "Error",
  "Function",
  "Infinity",
  "JSON",
  "Map",
  "Math",
  "NaN",
  "Number",
  "Object",
  "Promise",
  "Proxy",
  "RangeError",
  "ReferenceError",
  "RegExp",
  "Set",
  "String",
  "Symbol",
  "TypeError",
  "URL",
  "WeakMap",
  "WeakSet",
  "console",
  "decodeURIComponent",
  "document",
  "encodeURIComponent",
  "eval",
  "globalThis",
  "isFinite",
  "parseFloat",
  "parseInt",
  "setTimeout",
  "undefined",
  "window",
  // A function-local implicit binding. Declaring it in the synthetic module
  // wrapper is a strict-mode syntax error and would change its semantics.
  "arguments",
]);

function addPatternNames(pattern, names) {
  if (!pattern) return;
  if (pattern.type === "Identifier") names.add(pattern.name);
  else if (pattern.type === "RestElement") addPatternNames(pattern.argument, names);
  else if (pattern.type === "AssignmentPattern") addPatternNames(pattern.left, names);
  else if (pattern.type === "ArrayPattern") pattern.elements.forEach((item) => addPatternNames(item, names));
  else if (pattern.type === "ObjectPattern") {
    pattern.properties.forEach((property) => addPatternNames(property.value ?? property.argument, names));
  }
}

function identifierIsReference(node, parent, key) {
  if (!parent) return false;
  if (
    (parent.type === "MemberExpression" && key === "property" && !parent.computed) ||
    (parent.type === "Property" && key === "key" && !parent.computed) ||
    (parent.type === "MethodDefinition" && key === "key" && !parent.computed) ||
    (parent.type === "VariableDeclarator" && key === "id") ||
    ((parent.type === "FunctionDeclaration" || parent.type === "FunctionExpression") && key === "id") ||
    ((parent.type === "LabeledStatement" || parent.type === "BreakStatement" || parent.type === "ContinueStatement") && key === "label")
  ) {
    return false;
  }
  return true;
}

function normalizedFunctionCode(source, node) {
  const declared = new Set();
  const referenced = new Set();
  visit(node, (child, parent, key) => {
    if (isFunction(child)) {
      if (child.id) declared.add(child.id.name);
      child.params.forEach((param) => addPatternNames(param, declared));
    }
    if (child.type === "VariableDeclarator") addPatternNames(child.id, declared);
    if (child.type === "ClassDeclaration" && child.id) declared.add(child.id.name);
    if (child.type === "CatchClause") addPatternNames(child.param, declared);
    if (child.type === "Identifier" && identifierIsReference(child, parent, key)) {
      referenced.add(child.name);
    }
  });
  const external = [...referenced]
    .filter((name) => !declared.has(name) && !standardGlobals.has(name))
    .sort();
  const original = source.slice(node.start, node.end);
  // Give free module bindings equal access to Oxc's identifier mangler on both
  // sides. Built-ins and the implicit `arguments` binding stay untouched.
  const wrapper = `${external.length ? `let ${external.join(",")};` : ""}export default ${original}`;
  const result = minifySync("function.js", wrapper, {
    module: true,
    compress: true,
    mangle: true,
    codegen: true,
  });
  if (result.errors.length) {
    throw new Error(result.errors.map((error) => error.message ?? String(error)).join("\n"));
  }
  const program = parse(result.code, { ecmaVersion: "latest", sourceType: "module" });
  const exported = program.body.find((statement) => statement.type === "ExportDefaultDeclaration");
  if (!exported) throw new Error(`Oxc removed retained function: ${result.code}`);
  return result.code.slice(exported.declaration.start, exported.declaration.end);
}

const upstreamCache = new Map();
function upstreamFunctions(path) {
  if (upstreamCache.has(path)) return upstreamCache.get(path);
  if (!path) return [];
  let source;
  try {
    source = readFileSync(path, "utf8");
  } catch {
    upstreamCache.set(path, []);
    return [];
  }
  const program = parse(source, { ecmaVersion: "latest", sourceType: "script" });
  let factory = null;
  visit(program, (node) => {
    if (
      !factory &&
      node.type === "CallExpression" &&
      node.callee.type === "Identifier" &&
      node.callee.name === "define"
    ) {
      factory = node.arguments.find(isFunction) ?? null;
    }
  });
  if (!factory) {
    upstreamCache.set(path, []);
    return [];
  }
  const functions = collectFunctions(factory.body, (node) => node !== factory).map((fn) => ({
    ...fn,
    code: normalizedFunctionCode(source, fn.node),
  }));
  upstreamCache.set(path, functions);
  return functions;
}

function matchingNames(sourceName) {
  const names = [sourceName];
  if (/^fn[A-Z]/u.test(sourceName)) {
    names.push(sourceName[2].toLowerCase() + sourceName.slice(3));
  }
  if (sourceName.endsWith("Impl")) names.push(sourceName.slice(0, -4));
  if (sourceName.endsWith("Factory")) names.push(sourceName.slice(0, -7));
  return names;
}

const rows = [];
for (const fn of lilFunctions) {
  const identity = generatedIdentity(fn.name);
  if (!identity) continue;
  const owner = identity.owner ?? moduleOwners.get(identity.moduleId);
  if (!owner) continue;
  const relativeModule = relative(portRoot, owner);
  const upstreamPath = join(upstreamRoot, relativeModule.replace(/\.lil$/u, ".js"));
  const sourceName = identity.generatedBase.includes("$")
    ? identity.generatedBase.slice(identity.generatedBase.lastIndexOf("$") + 1)
    : identity.sourceName;
  const candidates = upstreamFunctions(upstreamPath).filter((candidate) =>
    matchingNames(sourceName).includes(candidate.name),
  );
  if (candidates.length === 0) continue;
  const upstream =
    candidates.find((candidate) => candidate.params === fn.params) ?? candidates[0];
  const lilCode = normalizedFunctionCode(lilSource, fn.node);
  const lilMetrics = metrics(lilCode);
  const upstreamMetrics = metrics(upstream.code);
  rows.push({
    module: relativeModule,
    name: sourceName,
    lil: lilMetrics,
    upstream: upstreamMetrics,
    delta: lilMetrics.raw - upstreamMetrics.raw,
    lilCode,
    upstreamCode: upstream.code,
  });
}

const modules = new Map();
for (const row of rows) {
  const total = modules.get(row.module) ?? {
    functions: 0,
    lilRaw: 0,
    upstreamRaw: 0,
    delta: 0,
  };
  total.functions += 1;
  total.lilRaw += row.lil.raw;
  total.upstreamRaw += row.upstream.raw;
  total.delta += row.delta;
  modules.set(row.module, total);
}

console.log("Matched function deltas after isolated Oxc compression:");
console.log("  delta    lil     js  module :: function");
for (const row of rows.sort((left, right) => right.delta - left.delta).slice(0, 120)) {
  console.log(
    `${String(row.delta).padStart(7)} ${String(row.lil.raw).padStart(6)} ` +
      `${String(row.upstream.raw).padStart(6)}  ${row.module} :: ${row.name}`,
  );
}

console.log("\nAggregate matched-function deltas by source module:");
console.log("  delta    lil     js funcs  module");
for (const [module, total] of [...modules].sort((left, right) => right[1].delta - left[1].delta)) {
  console.log(
    `${String(total.delta).padStart(7)} ${String(total.lilRaw).padStart(6)} ` +
      `${String(total.upstreamRaw).padStart(6)} ${String(total.functions).padStart(5)}  ${module}`,
  );
}

console.log(
  `\nmatched ${rows.length} functions across ${modules.size} modules; ` +
    `isolated raw delta ${rows.reduce((sum, row) => sum + row.delta, 0)}`,
);

if (detailSelector) {
  const row = rows.find(
    (candidate) => `${candidate.module}::${candidate.name}` === detailSelector,
  );
  if (!row) throw new Error(`no matched function named ${detailSelector}`);
  console.log(`\n--- LilScript ${detailSelector} ---\n${row.lilCode}`);
  console.log(`\n--- upstream ${detailSelector} ---\n${row.upstreamCode}`);
}
