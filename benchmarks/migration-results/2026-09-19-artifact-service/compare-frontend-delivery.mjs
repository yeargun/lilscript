import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const directory = dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
assert(args.length === 0 || args.length === 3, "expected before-run after-run output-directory");
const runs = args.length ? args.slice(0, 2) : ["run-2026-09-19T13-59-26.405Z", "run-2026-09-19T14-47-06.382Z"];
const hash = value => createHash("sha256").update(value).digest("hex");
const identity = path => ({ path: relative(directory, path), sha256: hash(readFileSync(path)) });
const read = path => JSON.parse(readFileSync(path));
const receipts = runs.map(run => join(directory, run, "receipt.json"));
for (const path of receipts) {
  assert.equal(read(path).passed, true);
  assert.equal(read(path).inputsStable, true);
}
const rows = [];
for (const mode of ["direct", "search", "paired", "native"]) {
  const paths = runs.map(run => join(directory, run, `example-${mode}/summary.json`));
  const summaries = paths.map(read);
  for (const summary of summaries) assert.equal(summary.qualified, true);
  assert.equal(summaries[0].runs.length, summaries[1].runs.length);
  for (let index = 0; index < summaries[0].runs.length; index++) {
    const pair = summaries.map(summary => summary.runs[index]);
    assert.equal(pair[0].mode, pair[1].mode);
    assert.deepEqual(pair[0].artifacts, pair[1].artifacts);
    const names = mode === "native" ? ["native.c"] : pair[0].artifacts.map(([name]) => `${name}.mjs`);
    assert(names.length > 0);
    for (const name of names) {
      const files = pair.map(run => join(run.directory, name));
      assert(readFileSync(files[0]).equals(readFileSync(files[1])), `${mode}/${name}`);
      rows.push({ mode: pair[0].mode, name, files: files.map(identity) });
    }
  }
}
const output = join(directory, args[2] ?? "frontend-delivery-comparison");
mkdirSync(output, { recursive: false });
writeFileSync(join(output, "receipt.json"), JSON.stringify({
  schema: 1, scope: "delivered files from accepted original-consumer checkpoints; not a fleet or speed claim",
  checker: identity(fileURLToPath(import.meta.url)), receipts: receipts.map(identity),
  equal: true, filesCompared: rows.length, rows,
}, null, 2) + "\n");
console.log(JSON.stringify({ equal: true, filesCompared: rows.length, output }));
