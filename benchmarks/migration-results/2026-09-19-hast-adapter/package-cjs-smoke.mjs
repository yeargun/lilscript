import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join, resolve } from "node:path";
import { observeNodeArtifacts } from "../../../finer/tools/observe-node-artifacts.mjs";

assert.equal(process.argv.length, 5, "usage: package-cjs-smoke.mjs consumer-directory load-directory result-file");
const [consumer, loadDirectory, resultFile] = process.argv.slice(2).map(path => resolve(path));
const required = join(consumer, "node_modules/@itslil/hast-util-to-html/dist/to-html.cjs");
const requirePackage = createRequire(join(consumer, "consumer.cjs"));
const observer = observeNodeArtifacts({ paths: [required], directory: loadDirectory });
const cases = [];
function check(id, fn) {
  try {
    fn();
    cases.push({ id, status: "pass" });
  } catch (error) {
    cases.push({ id, status: "fail", message: error.message, expected: error.expected, actual: error.actual });
  }
}
let library;
check("package-resolution", () => {
  assert.equal(requirePackage.resolve("@itslil/hast-util-to-html"), required);
  library = requirePackage("@itslil/hast-util-to-html");
});
check("named-exports", () => {
  assert.equal(typeof library.toHtml, "function");
  assert.deepEqual(Object.keys(library).sort(), ["toHtml"]);
});
check("public-name", () => assert.equal(library.toHtml.name, "toHtml"));
check("public-arity", () => assert.equal(library.toHtml.length, 2));
check("text", () => assert.deepEqual(library.toHtml({ type: "text", value: "alpha" }), "alpha"));
check("escaping", () => assert.deepEqual(library.toHtml({ type: "text", value: "3 < 5 & 7" }), "3 &#x3C; 5 &#x26; 7"));
check("invalid-argument", () => assert.throws(() => library.toHtml(true), /Expected node, not `true`/));
check("array-input", () => {
  // Explicit equivalents of the original core test's h('b') and h('i').
  const nodes = ["b", "i"].map(tagName => ({ type: "element", tagName, properties: {}, children: [] }));
  assert.equal(library.toHtml(nodes), "<b></b><i></i>");
});
observer.deregister();
const loaded = Object.keys(requirePackage.cache).sort();
const dependencyClosure = loaded.length === 1 && loaded[0] === required;
const report = {
  schema: 1, cases, passed: cases.every(row => row.status === "pass") && dependencyClosure,
  observed: { exports: Object.keys(library ?? {}), name: library?.toHtml?.name, arity: library?.toHtml?.length },
  loadedCommonJsFiles: loaded, dependencyClosure,
  installedManifest: JSON.parse(readFileSync(join(consumer, "node_modules/@itslil/hast-util-to-html/package.json"), "utf8")),
};
writeFileSync(resultFile, JSON.stringify(report, null, 2) + "\n");
console.log(JSON.stringify(report));
if (!report.passed) process.exitCode = 1;
