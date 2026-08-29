import assert from "node:assert/strict";
import { existsSync, mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import test from "node:test";
import {
  createVueTestAliases,
  labRoot,
  loadTestMap,
  resolveTestSuite,
  upstreamRoot,
} from "../tooling/vue-test-aliases.mjs";
import { auditErrors } from "../scripts/test-upstream-candidate.mjs";
import { loadSuite, summarizeVitest } from "../scripts/test-upstream-reference.mjs";

test("shared suite pins all six Vue 3.5.42 shared specs", () => {
  const map = loadTestMap();
  const suite = resolveTestSuite("shared", map);
  assert.equal(map.upstream.version, "3.5.42");
  assert.equal(map.upstream.revision, "d63616ca17de965ed32dcb449a4c5cd9982f15d2");
  assert.deepEqual(suite.targetPackages, ["@vue/shared"]);
  assert.equal(suite.testFiles.length, 6);
  assert.equal(
    suite.testFiles.reduce((total, file) => total + file.tests, 0),
    62,
  );
  for (const file of suite.testFiles) {
    assert.equal(existsSync(resolve(upstreamRoot, file.path)), true, file.path);
  }
});

test("candidate aliases cover package and relative source imports", () => {
  const plugin = createVueTestAliases();
  const facade = resolve(labRoot, "packages/vuelil/shared.js");
  const testFile = resolve(
    upstreamRoot,
    "packages/shared/__tests__/codeframe.spec.ts",
  );
  assert.equal(plugin.resolveId("@vue/shared", testFile), facade);
  assert.equal(plugin.resolveId("@vue/shared/codeframe", testFile), facade);
  assert.equal(plugin.resolveId("../src", testFile), facade);
  assert.equal(plugin.resolveId("../src/codeframe", testFile), facade);
  assert.equal(plugin.resolveId("@vue/reactivity", testFile), null);
});

test("source audit fails closed when targeted upstream code reaches load", () => {
  const directory = mkdtempSync(resolve(tmpdir(), "vuelil-alias-test-"));
  const auditPath = resolve(directory, "audit.json");
  const plugin = createVueTestAliases({ auditPath });
  plugin.configResolved();
  assert.throws(
    () => plugin.load(resolve(upstreamRoot, "packages/shared/src/general.ts")),
    /Blocked targeted upstream implementation module/,
  );
  plugin.api.finish();
  const audit = JSON.parse(readFileSync(auditPath, "utf8"));
  assert.equal(audit.noTargetedUpstreamImplementationImports, false);
  assert.deepEqual(audit.blockedTargetedModules, [
    {
      package: "@vue/shared",
      path: "packages/shared/src/general.ts",
      hook: "load",
    },
  ]);
});

test("source audit requires redirects and an observed candidate load", () => {
  const plugin = createVueTestAliases();
  const facade = plugin.resolveId(
    "../src",
    resolve(upstreamRoot, "packages/shared/__tests__/cssVars.spec.ts"),
  );
  plugin.load(facade);
  plugin.api.finish();
  const audit = plugin.api.snapshot();
  const { suite } = loadSuite("shared");
  assert.deepEqual(auditErrors(audit, suite), []);
  assert.equal(audit.targets[0].redirects, 1);
  assert.equal(audit.targets[0].candidateLoaded, true);
});

test("per-file reporting preserves every assertion result", () => {
  const suite = {
    testFiles: [{ path: "packages/shared/__tests__/sample.spec.ts", tests: 2 }],
  };
  const path = resolve(upstreamRoot, suite.testFiles[0].path);
  const summary = summarizeVitest(
    {
      testResults: [
        {
          name: path,
          status: "failed",
          startTime: 10,
          endTime: 14,
          message: "one failed",
          assertionResults: [
            {
              fullName: "sample passes",
              status: "passed",
              duration: 1,
              failureMessages: [],
            },
            {
              fullName: "sample fails",
              status: "failed",
              duration: 2,
              failureMessages: ["failure"],
            },
          ],
        },
      ],
    },
    suite,
  );
  assert.equal(summary.files[0].status, "failed");
  assert.deepEqual(summary.files[0].counts, {
    total: 2,
    passed: 1,
    failed: 1,
    pending: 0,
  });
  assert.equal(summary.files[0].tests[1].failureMessages[0], "failure");
});
