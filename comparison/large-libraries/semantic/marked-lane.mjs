#!/usr/bin/env node

import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

function option(name) {
  const index = process.argv.indexOf(name);
  if (index === -1 || !process.argv[index + 1]) {
    throw new Error(`missing ${name}`);
  }
  return resolve(process.argv[index + 1]);
}

const root = option("--root");
const artifact = option("--artifact");
const requireFromProject = createRequire(resolve(root, "package.json"));
const officialModule = await import(
  pathToFileURL(requireFromProject.resolve("marked")).href
);
const official = officialModule.marked ?? officialModule.default ?? officialModule;
const spec = await import(pathToFileURL(resolve(root, "scripts/spec.mjs")).href);
const candidateModule = await import(`${pathToFileURL(artifact).href}?semantic-lane`);
const candidate = spec.resolveParse(candidateModule);
const cases = spec.loadSpecCases();

assert.equal(typeof candidate, "function", "parse export");
assert.equal(typeof candidateModule.parseInline, "function", "parseInline export");
assert.equal(typeof candidateModule.marked, "function", "marked export");
assert.equal(candidateModule.default, candidateModule.marked, "default export");
assert.deepEqual(Object.keys(candidateModule.getDefaults()).sort(), [
  "async",
  "breaks",
  "gfm",
  "pedantic",
  "silent",
]);

const optionSets = [{}, { breaks: true }, { pedantic: true }, { gfm: false }];
let parseChecks = 0;
let inlineChecks = 0;
const failures = [];
for (const test of cases) {
  for (const options of optionSets) {
    const label = `${test.file}#${test.example} parse ${JSON.stringify(options)}`;
    try {
      if (candidate(test.markdown, options) !== official.parse(test.markdown, options)) {
        failures.push(label);
      }
    } catch (error) {
      failures.push(`${label} threw ${String(error)}`);
    }
    parseChecks += 1;
  }
  const inline = test.markdown.replace(/\n{2,}/gu, " ").trim();
  if (inline.length > 0 && inline.length <= 400) {
    const label = `${test.file}#${test.example} parseInline`;
    try {
      if (candidateModule.parseInline(inline) !== official.parseInline(inline)) {
        failures.push(label);
      }
    } catch (error) {
      failures.push(`${label} threw ${String(error)}`);
    }
    inlineChecks += 1;
  }
}

const previous = candidateModule.getDefaults();
try {
  assert.equal(candidateModule.marked.setOptions({ breaks: true }), candidateModule.marked);
  assert.equal(
    candidate("a\nb"),
    official.parse("a\nb", { breaks: true }),
    "setOptions must update the live defaults",
  );
} finally {
  candidateModule.marked.setOptions({ ...previous, breaks: false });
}
assert.throws(() => candidateModule.marked(1), /input must be a string/u);

assert.equal(
  failures.length,
  0,
  `${failures.length} corpus mismatches: ${failures.slice(0, 12).join(", ")}`,
);

console.log(
  JSON.stringify({
    status: "passed",
    corpusCases: cases.length,
    parseChecks,
    inlineChecks,
    artifact,
  }),
);
