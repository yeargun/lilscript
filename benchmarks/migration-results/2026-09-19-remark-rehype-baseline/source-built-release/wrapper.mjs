import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
import { qualifyNodeLibrary } from "../../../finer/tools/qualify-node-library.mjs";

const recipe = {
  "workload": "remark-rehypelil",
  "workspace": "/home/azureuser/remark-rehypelil",
  "inventoryPath": "benchmarks/libraries/remark-rehypelil.required-tests.json",
  "discovery": {
    "directory": "benchmarks/migration-results/2026-09-19-remark-rehype-adapter/existing-dist",
    "sha256": "c71c89a9b873eab0ce53eb88be75a4d9edf449eb0d541bf9dfee9f566d98eb8a"
  },
  "caseCount": 21,
  "testFileCount": 4,
  "artifactStem": "remark-rehype",
  "esmBanner": "/*! @itslil/remark-rehype 11.1.3 | LilScript reimplementation of remark-rehype | MIT */\n",
  "kind": "source-pinned-remark-rehype-baseline",
  "armLabel": "preserved-release-original-remark-rehype",
  "scope": "Unchanged two-invocation source build, original type prerequisite and all 21 Node identities, followed by the separate original package dry-run check",
  "limitations": [
    "Compiler identity is the explicitly pinned accepted release parent; current worktree identity and semantic-backend library support are not inferred.",
    "The 21 original Node identities include 18 test nodes (including the official parent test) and 3 suites; no extra Node case IDs are invented for command prerequisites.",
    "Original runtime tests execute candidate behavior from direct generated ESM. The closed test only imports the generated artifact and checks callable-export shape; closed conversion behavior is not covered.",
    "CJS runtime/behavior, installed-package exports resolution and package import/require delivery are not covered. No exact UMD/browser runtime qualification is claimed.",
    "The original upstream mdast-util-to-hast package supplies only two numeric footnote-helper oracle functions; candidate plugin behavior uses generated ESM, not a replacement upstream plugin.",
    "The original Node site test reads existing files and imports the configured generated ESM to check its callable export. Original check:site/build:site remains unexecuted; package dry-run success is not browser or installed-package evidence.",
    "All JavaScript files are scored independently, including unexecuted raw/UMD outputs; no competitive gain, combined-stream delivery optimum, or public-observation policy change follows.",
    "Source, installed project dependencies, global npm including nested dependencies, and explicit tools are pinned; OS libraries and the whole host environment are not hermetic.",
    "Per-phase and per-prerequisite caps are individual command bounds, not additive performance evidence. Outer supervision also bounds the shared attempt; escaped sessions and supervisor SIGKILL cleanup are not covered.",
    "No compiler build, network dependency installation, source edit, altered original assertion, or retry is performed. Host load is recorded but not isolated; no speed claim."
  ],
  "defaultParent": {
    "receipt": "benchmarks/migration-results/2026-09-19-artifact-service/run-2026-09-19T19-48-31.223Z/receipt.json",
    "sha256": "8918faac11122e916d8c7decbdfe03d71f3b873c6dc990691a2709fe498e1a39",
    "compiler": "/tmp/lilscript-assignment-sink-retirement-release-20260919/lilscript",
    "codec": "/tmp/lilscript-assignment-sink-retirement-release-20260919/lilscript-codec"
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
