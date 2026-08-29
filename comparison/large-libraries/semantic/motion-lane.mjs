import assert from "node:assert/strict";
import { parseArgs } from "node:util";
import { pathToFileURL } from "node:url";
import { basename } from "node:path";

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
const boundary = basename(values.artifact).replace(/^\.__compiled-/u, "").replace(/\.mjs$/u, "");
const expected = {
  "animate-mini": ["animateMini"],
  animate: ["animate"],
  "animate-stagger": ["animate", "stagger"],
  lab: ["animate", "animateMini", "hover", "inView", "motionValue", "press", "scroll", "stagger"],
  export: ["animate", "animateMini", "hover", "inView", "motionValue", "press", "resize", "scroll", "stagger"],
  mini: ["animate", "animateSequence"],
  full: ["animate", "animateMini", "motionValue", "scroll", "stagger"],
  "animate-direct": ["animate", "clamp", "mix", "motionValue", "spring", "stagger"],
}[boundary];
if (!expected) throw new Error(`unknown Motion evidence boundary ${boundary}`);
for (const name of expected) {
  assert.equal(typeof motion[name], "function", `missing Motion export ${name}`);
}
if (motion.clamp) assert.equal(motion.clamp(0, 10, 12), 10);
if (motion.mix) assert.equal(motion.mix(0, 8, 0.5), 4);
if (motion.motionValue) assert.equal(motion.motionValue(3).get(), 3);

console.log(`Motion direct artifact passed: ${Object.keys(motion).length} exports`);
