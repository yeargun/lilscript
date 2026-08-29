import assert from "node:assert/strict";
import { parseArgs } from "node:util";
import { pathToFileURL } from "node:url";

const { values } = parseArgs({
  options: {
    root: { type: "string" },
    artifact: { type: "string" },
  },
});

if (!values.root || !values.artifact) {
  throw new Error("usage: motion-lane.mjs --root PATH --artifact PATH");
}

const motion = await import(`${pathToFileURL(values.artifact).href}?evidence=1`);
for (const name of ["animate", "clamp", "mix", "motionValue", "spring", "stagger"]) {
  assert.equal(typeof motion[name], "function", `missing Motion export ${name}`);
}
assert.equal(motion.clamp(0, 10, 12), 10);
assert.equal(motion.mix(0, 8, 0.5), 4);
assert.equal(motion.motionValue(3).get(), 3);

console.log(`Motion direct artifact passed: ${Object.keys(motion).length} exports`);
