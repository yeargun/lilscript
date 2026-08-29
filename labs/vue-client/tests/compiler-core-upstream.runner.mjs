import { spawnSync } from "node:child_process";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const upstream = resolve(root, "upstream/vue");
const build = spawnSync(
  process.execPath,
  [resolve(root, "scripts/build-compiler-core.mjs")],
  { cwd: root, encoding: "utf8", env: process.env },
);
if (build.status !== 0) {
  process.stderr.write(`${build.stdout}${build.stderr}`);
  process.exit(build.status ?? 1);
}

const requested = process.argv.slice(2);
const suites = requested.length
  ? requested
  : [
      "packages/compiler-core/__tests__/codegen.spec.ts",
      "packages/compiler-core/__tests__/compile.spec.ts",
      "packages/compiler-core/__tests__/parse.spec.ts",
      "packages/compiler-core/__tests__/scopeId.spec.ts",
      "packages/compiler-core/__tests__/transform.spec.ts",
      "packages/compiler-core/__tests__/utils.spec.ts",
      "packages/compiler-core/__tests__/transforms/cacheStatic.spec.ts",
      "packages/compiler-core/__tests__/transforms/noopDirectiveTransform.spec.ts",
      "packages/compiler-core/__tests__/transforms/transformElement.spec.ts",
      "packages/compiler-core/__tests__/transforms/transformExpressions.spec.ts",
      "packages/compiler-core/__tests__/transforms/transformSlotOutlet.spec.ts",
      "packages/compiler-core/__tests__/transforms/transformText.spec.ts",
      "packages/compiler-core/__tests__/transforms/vBind.spec.ts",
      "packages/compiler-core/__tests__/transforms/vFor.spec.ts",
      "packages/compiler-core/__tests__/transforms/vIf.spec.ts",
      "packages/compiler-core/__tests__/transforms/vMemo.spec.ts",
      "packages/compiler-core/__tests__/transforms/vModel.spec.ts",
      "packages/compiler-core/__tests__/transforms/vOn.spec.ts",
      "packages/compiler-core/__tests__/transforms/vOnce.spec.ts",
      "packages/compiler-core/__tests__/transforms/vSlot.spec.ts",
    ];
const result = spawnSync(
  process.execPath,
  [
    resolve(root, "node_modules/vitest/vitest.mjs"),
    "run",
    "--config",
    resolve(root, "tests/compiler-core-upstream.config.mjs"),
    ...suites,
    "--maxWorkers=1",
    "--no-file-parallelism",
    "--no-cache",
  ],
  { cwd: upstream, stdio: "inherit", env: { ...process.env, CI: "1" } },
);
process.exit(result.status ?? 1);
