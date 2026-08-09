import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import test from "node:test";

const compatibility = JSON.parse(
  await readFile(new URL("../compatibility/libraries.json", import.meta.url), "utf8"),
);

test("incomplete package inventories match their pinned runtime entrypoints", async () => {
  const audited = compatibility.targets.filter((target) => target.runtimeAudit);
  assert.deepEqual(
    audited.map((target) => target.id),
    ["acorn", "preact", "redux-toolkit", "immer", "zod"],
  );

  for (const target of audited) {
    assert.doesNotMatch(target.status, /^exact-/);
    for (const entrypoint of target.runtimeAudit.auditedEntrypoints) {
      const runtime = await import(entrypoint.specifier);
      const names = Object.keys(runtime).sort();
      const sha256 = createHash("sha256").update(names.join("\n")).digest("hex");
      assert.deepEqual(names, entrypoint.runtimeExportNames, entrypoint.specifier);
      assert.equal(sha256, entrypoint.exportNameSha256, entrypoint.specifier);
    }
  }
});
