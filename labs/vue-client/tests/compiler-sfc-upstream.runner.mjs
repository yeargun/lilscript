import { spawnSync } from "node:child_process";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const upstream = resolve(root, "upstream/vue");
const build = spawnSync(
  process.execPath,
  [resolve(root, "scripts/build-compiler-sfc.mjs")],
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
      "packages/compiler-sfc/__tests__/compileScript.spec.ts",
      "packages/compiler-sfc/__tests__/compileScript/defineEmits.spec.ts",
      "packages/compiler-sfc/__tests__/compileScript/defineExpose.spec.ts",
      "packages/compiler-sfc/__tests__/compileScript/defineModel.spec.ts",
      "packages/compiler-sfc/__tests__/compileScript/defineOptions.spec.ts",
      "packages/compiler-sfc/__tests__/compileScript/defineProps.spec.ts",
      "packages/compiler-sfc/__tests__/compileScript/definePropsDestructure.spec.ts",
      "packages/compiler-sfc/__tests__/compileScript/defineSlots.spec.ts",
      "packages/compiler-sfc/__tests__/compileScript/hoistStatic.spec.ts",
      "packages/compiler-sfc/__tests__/compileScript/importUsageCheck.spec.ts",
      "packages/compiler-sfc/__tests__/compileScript/resolveType.spec.ts",
      "packages/compiler-sfc/__tests__/compileStyle.spec.ts",
      "packages/compiler-sfc/__tests__/compileTemplate.spec.ts",
      "packages/compiler-sfc/__tests__/cssVars.spec.ts",
      "packages/compiler-sfc/__tests__/parse.spec.ts",
      "packages/compiler-sfc/__tests__/rewriteDefault.spec.ts",
      "packages/compiler-sfc/__tests__/templateTransformAssetUrl.spec.ts",
      "packages/compiler-sfc/__tests__/templateTransformSrcset.spec.ts",
      "packages/compiler-sfc/__tests__/templateUtils.spec.ts",
    ];
const result = spawnSync(
  process.execPath,
  [
    resolve(root, "node_modules/vitest/vitest.mjs"),
    "run",
    "--config",
    resolve(root, "tests/compiler-sfc-upstream.config.mjs"),
    ...suites,
    "--maxWorkers=1",
    "--no-file-parallelism",
    "--no-cache",
  ],
  { cwd: upstream, stdio: "inherit", env: { ...process.env, CI: "1" } },
);
process.exit(result.status ?? 1);
