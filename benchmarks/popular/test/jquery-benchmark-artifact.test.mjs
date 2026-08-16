import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, realpathSync } from "node:fs";
import { dirname } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  JQUERY_LILSCRIPT_ARTIFACT_ENV,
  JQUERY_LILSCRIPT_ARTIFACT_SHA256_ENV,
  resolveJqueryLilscriptArtifact,
} from "../jquery-benchmark-artifact.mjs";

const testRoot = dirname(fileURLToPath(import.meta.url));
const labRoot = dirname(testRoot);
const fixturePath = fileURLToPath(
  new URL("../benchmark-jquery-worker.mjs", import.meta.url),
);
const fixtureSha256 = createHash("sha256")
  .update(readFileSync(fixturePath))
  .digest("hex");

test("selects the compatible default and records its identity", () => {
  assert.deepEqual(
    resolveJqueryLilscriptArtifact({
      environment: {},
      workingDirectory: labRoot,
      defaultArtifactPath: "benchmark-jquery-worker.mjs",
    }),
    {
      path: realpathSync(fixturePath),
      sha256: fixtureSha256,
      selectedBy: "default",
    },
  );
});

test("an explicit relative artifact overrides the default", () => {
  const selected = resolveJqueryLilscriptArtifact({
    environment: {
      [JQUERY_LILSCRIPT_ARTIFACT_ENV]: "benchmark-jquery-worker.mjs",
      [JQUERY_LILSCRIPT_ARTIFACT_SHA256_ENV]: fixtureSha256.toUpperCase(),
    },
    workingDirectory: labRoot,
    defaultArtifactPath: "missing-default.js",
  });
  assert.equal(selected.path, realpathSync(fixturePath));
  assert.equal(selected.sha256, fixtureSha256);
  assert.equal(selected.selectedBy, JQUERY_LILSCRIPT_ARTIFACT_ENV);
});

test("rejects missing, non-file, and digest-mismatched selections", () => {
  const select = (artifact, digest) => resolveJqueryLilscriptArtifact({
    environment: {
      [JQUERY_LILSCRIPT_ARTIFACT_ENV]: artifact,
      ...(digest === undefined
        ? {}
        : { [JQUERY_LILSCRIPT_ARTIFACT_SHA256_ENV]: digest }),
    },
    workingDirectory: labRoot,
    defaultArtifactPath: fixturePath,
  });

  assert.throws(() => select(""), /must name a JavaScript artifact file/u);
  assert.throws(() => select("does-not-exist.js"), /readable regular file/u);
  assert.throws(() => select("."), /readable regular file/u);
  assert.throws(
    () => select("benchmark-jquery-worker.mjs", "0".repeat(64)),
    /changed after selection/u,
  );
});
