// Delivery plan assertions (plan M3.3, design §14.3): the exact files, roles
// and labels each `tests/bundles` fixture delivers. A change here is legal
// only with an explanation in the commit that makes it; behaviour is
// `delivery-contract.mjs`'s to check.
//
//   node scripts/delivery-plan.mjs [compiler]
import assert from "node:assert/strict";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "./delivery-contract.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

/// Per fixture: each file as [name, role, label].
export const plans = {
  // Every module a file; the write to `state` goes through a setter.
  preserve: [
    ["main.js", "entry", ["main"]],
    ["state.js", "module", ["main"]],
  ],
  // Two entries: `left` is a facade over the code both load.
  split: [
    ["left.js", "entry", ["left"]],
    ["main.js", "entry", ["main"]],
    ["0.js", "definitions", ["left", "main"]],
  ],
  // The lazy file holds only the member read.
  lazy: [
    ["main.js", "entry", ["main"]],
    ["0.js", "lazy", ["feature"]],
  ],
  // What the lazy file and the entry both need stays out of the facade.
  "lazy-cycle": [
    ["main.js", "entry", ["main"]],
    ["0.js", "lazy", ["feature"]],
    ["1.js", "shared", ["main"]],
  ],
};

export async function verifyPlans(compiler, outputRoot) {
  for (const [name, expected] of Object.entries(plans)) {
    const output = await build(compiler, name, path.join(outputRoot, name));
    assert.deepEqual(
      output.files.map((file) => [file.file, file.role, file.label]),
      expected,
      `${name}: delivered files`,
    );
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const compiler = process.argv[2] ?? path.join(root, "target/release/lilscript");
  await verifyPlans(compiler, path.join(root, "target/verification/delivery-plan"));
  console.log("Delivery plans passed.");
}
