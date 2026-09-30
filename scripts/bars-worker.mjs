// The exact same observer is run in a fresh process for every delivered
// artifact. The observer installs the declared host before loading the entry.
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {pathToFileURL} from "node:url";
import vm from "node:vm";

const request = JSON.parse(readFileSync(process.argv[2], "utf8"));
const oracle = await import(pathToFileURL(request.oracle).href);
if (oracle.install) await oracle.install();
let api;
if (request.format === "esm") api = await import(pathToFileURL(request.artifact).href);
else {
  vm.runInThisContext(readFileSync(request.artifact, "utf8"), {filename: request.artifact});
  api = globalThis;
}
const actual = await oracle.observe(api);
assert.deepStrictEqual(actual, request.expected, "common behavior oracle differs");
process.stdout.write(JSON.stringify({schema:1, passed:true, actual}) + "\n");
