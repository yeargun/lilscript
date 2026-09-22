import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
import { qualifyNodeLibrary } from "../../../finer/tools/qualify-node-library.mjs";

const recipe = {
  "workload": "mdast-util-to-hastlil",
  "workspace": "/home/azureuser/mdast-util-to-hastlil",
  "inventoryPath": "benchmarks/libraries/mdast-util-to-hastlil.required-tests.json",
  "discovery": {
    "directory": "benchmarks/migration-results/2026-09-19-mdast-to-hast-adapter/existing-dist",
    "sha256": "b49158603b840d7c97dbebc2d8d338310dcf5189494491e67f740ffbc11925c0"
  },
  "caseCount": 152,
  "testFileCount": 3,
  "artifactStem": "to-hast",
  "esmBanner": "/*! @itslil/mdast-util-to-hast 13.2.1 | LilScript reimplementation of mdast-util-to-hast | MIT */\n",
  "kind": "source-pinned-mdast-to-hast-baseline",
  "armLabel": "preserved-release-original-to-hast",
  "scope": "Unchanged two-invocation source build, original type prerequisite and all 152 Node identities, followed by the separate original package dry-run check",
  "limitations": [
    "Compiler identity is the explicitly pinned accepted release parent; current worktree identity and semantic-backend library support are not inferred.",
    "The 152 original Node identities include 149 test nodes (including parent tests) and 3 suites; no extra Node case IDs are invented for command prerequisites.",
    "Original runtime tests execute direct generated ESM and closed paths. CJS runtime/behavior, installed-package exports resolution and package import/require delivery are not covered.",
    "No newly packed/installed consumer or exact UMD/browser runtime qualification is claimed.",
    "The original upstream mdast-util-to-hast@13.2.1 package supplies only two numeric footnote-helper oracle functions; candidate conversion always uses generated ESM/closed artifacts.",
    "The original Node site test reads existing files and imports the configured generated ESM to check its callable export. Original check:site/build:site remains unexecuted; package dry-run success is not browser or installed-package evidence.",
    "All JavaScript files are scored independently, including unexecuted raw/UMD outputs; no competitive gain, combined-stream delivery optimum, or public-observation policy change follows.",
    "Source, installed project dependencies, global npm including nested dependencies, and explicit tools are pinned; OS libraries and the whole host environment are not hermetic.",
    "Per-phase and per-prerequisite caps are individual command bounds, not additive performance evidence. Outer supervision also bounds the shared attempt; escaped sessions and supervisor SIGKILL cleanup are not covered.",
    "No compiler build, network dependency installation, source edit, altered original assertion, or retry is performed. Host load is recorded but not isolated; no speed claim."
  ],
  "defaultParent": {
    "receipt": "benchmarks/migration-results/2026-09-19-artifact-service/run-2026-09-19T16-36-23.430Z/receipt.json",
    "sha256": "020c288f5b0e6bafc016e12cbb6d5a7a7ab53b9b28a32444c584c9781abbf533",
    "compiler": "/tmp/lilscript-public-integration-release-baseline-20260919/lilscript",
    "codec": "/tmp/lilscript-public-integration-release-baseline-20260919/lilscript-codec"
  }
};
const receipt = await qualifyNodeLibrary(recipe, process.argv.slice(2), fileURLToPath(import.meta.url));
console.log(JSON.stringify({
  output: resolve(process.argv[2]), passed: receipt.passed,
  originalTestBoundaryPassed: receipt.originalTestBoundaryPassed,
  packageDryRunPassed: receipt.packageDryRunPassed, qualification: receipt.qualification,
  inputsStable: receipt.inputsStable, failure: receipt.failure?.message,
  validationFailure: receipt.validationFailure?.message,
}));
if (!receipt.passed) process.exitCode = 1;
