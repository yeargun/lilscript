#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { parse } from "acorn";

const root = resolve(import.meta.dirname, "../..");
const inputs = process.argv.length > 2
  ? process.argv.slice(2)
  : [
      resolve(import.meta.dirname, "node_modules/jquery/dist/jquery.min.js"),
      resolve(
        import.meta.dirname,
        "build/jquery-config-audit/lean-balanced.terser.js",
      ),
    ];

function increment(map, key, amount = 1) {
  map.set(key, (map.get(key) ?? 0) + amount);
}

function sortedObject(map, limit = Number.POSITIVE_INFINITY) {
  return Object.fromEntries(
    [...map]
      .sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0]))
      .slice(0, limit),
  );
}

function calleeShape(node) {
  if (!node) return "<none>";
  if (node.type === "Identifier") return node.name;
  if (node.type === "MemberExpression") {
    if (!node.computed && node.property.type === "Identifier") {
      return `.${node.property.name}`;
    }
    if (node.computed && node.property.type === "Literal") {
      return `[${JSON.stringify(node.property.value)}]`;
    }
    return "[computed]";
  }
  if (node.type === "FunctionExpression" || node.type === "ArrowFunctionExpression") {
    return "<inline-function>";
  }
  return `<${node.type}>`;
}

function percentile(values, fraction) {
  if (values.length === 0) return 0;
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * fraction))];
}

function analyze(path) {
  const code = readFileSync(path, "utf8");
  const ast = parse(code, {
    ecmaVersion: "latest",
    sourceType: "module",
    allowHashBang: true,
    ranges: true,
  });
  const nodes = new Map();
  const operators = new Map();
  const callees = new Map();
  const properties = new Map();
  const strings = new Map();
  const functionSizes = [];
  const functionParams = [];
  const functionStatements = [];
  let totalNodes = 0;
  let computedMembers = 0;
  let wrapperFunctions = 0;
  let restParameters = 0;

  function visit(node) {
    if (!node || typeof node !== "object") return;
    if (typeof node.type === "string") {
      totalNodes += 1;
      increment(nodes, node.type);
      if (
        node.type === "BinaryExpression" ||
        node.type === "LogicalExpression" ||
        node.type === "UnaryExpression" ||
        node.type === "UpdateExpression" ||
        node.type === "AssignmentExpression"
      ) {
        increment(operators, node.operator);
      }
      if (node.type === "CallExpression" || node.type === "NewExpression") {
        increment(callees, calleeShape(node.callee));
      }
      if (node.type === "MemberExpression") {
        computedMembers += Number(node.computed);
        if (!node.computed && node.property.type === "Identifier") {
          increment(properties, node.property.name);
        } else if (node.computed && node.property.type === "Literal") {
          increment(properties, String(node.property.value));
        }
      }
      if (node.type === "Literal" && typeof node.value === "string") {
        increment(strings, node.value, node.value.length);
      }
      if (
        node.type === "FunctionDeclaration" ||
        node.type === "FunctionExpression" ||
        node.type === "ArrowFunctionExpression"
      ) {
        functionSizes.push(node.end - node.start);
        functionParams.push(node.params.length);
        restParameters += node.params.filter((param) => param.type === "RestElement").length;
        if (node.body.type === "BlockStatement") {
          functionStatements.push(node.body.body.length);
          if (
            node.body.body.length === 1 &&
            node.body.body[0].type === "ReturnStatement" &&
            node.body.body[0].argument?.type === "CallExpression"
          ) {
            wrapperFunctions += 1;
          }
        } else {
          functionStatements.push(1);
          if (node.body.type === "CallExpression") wrapperFunctions += 1;
        }
      }
    }
    for (const [key, value] of Object.entries(node)) {
      if (key === "start" || key === "end" || key === "range") continue;
      if (Array.isArray(value)) {
        for (const child of value) visit(child);
      } else if (value && typeof value === "object") {
        visit(value);
      }
    }
  }

  visit(ast);
  return {
    path: path.startsWith(root) ? path.slice(root.length + 1) : path,
    bytes: Buffer.byteLength(code),
    totalNodes,
    functions: functionSizes.length,
    parameters: functionParams.reduce((sum, value) => sum + value, 0),
    restParameters,
    wrapperFunctions,
    functionBytes: {
      median: percentile(functionSizes, 0.5),
      p90: percentile(functionSizes, 0.9),
      maximum: Math.max(0, ...functionSizes),
    },
    functionStatements: {
      median: percentile(functionStatements, 0.5),
      p90: percentile(functionStatements, 0.9),
    },
    computedMembers,
    nodes: sortedObject(nodes),
    operators: sortedObject(operators),
    topCallees: sortedObject(callees, 30),
    topProperties: sortedObject(properties, 30),
    topStringsBySourceBytes: sortedObject(strings, 30),
  };
}

console.log(JSON.stringify(inputs.map((input) => analyze(resolve(input))), null, 2));
