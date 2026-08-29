import assert from "node:assert/strict";
import { copyFile, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { parseArgs } from "node:util";
import { pathToFileURL } from "node:url";

const { values } = parseArgs({
  options: {
    root: { type: "string" },
    artifact: { type: "string" },
  },
});

if (!values.root || !values.artifact) {
  throw new Error("usage: mobx-lane.mjs --root PATH --artifact PATH");
}

const temporary = await mkdtemp(join(tmpdir(), "lilscript-mobx-lane-"));
const modulePath = join(temporary, "mobx.mjs");
try {
  await copyFile(values.artifact, modulePath);
  const mobx = await import(`${pathToFileURL(modulePath).href}?evidence=1`);
  for (const name of ["ObservableMap", "ObservableSet", "autorun", "computed", "observable"] ) {
    assert.equal(typeof mobx[name], "function", `missing MobX export ${name}`);
  }
  const value = mobx.observable.box(1);
  assert.equal(value.get(), 1);
  value.set(2);
  assert.equal(value.get(), 2);
  const map = new mobx.ObservableMap([["key", 3]]);
  assert.equal(map.get("key"), 3);
  console.log(`MobX production-min artifact passed: ${Object.keys(mobx).length} exports`);
} finally {
  await rm(temporary, { recursive: true, force: true });
}
