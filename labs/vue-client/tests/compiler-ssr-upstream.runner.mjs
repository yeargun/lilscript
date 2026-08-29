import { spawnSync } from "node:child_process";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const upstream = resolve(root, "upstream/vue");
const build = spawnSync(
  process.execPath,
  [resolve(root, "scripts/build-compiler-ssr.mjs")],
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
      "packages/compiler-ssr/__tests__/ssrComponent.spec.ts",
      "packages/compiler-ssr/__tests__/ssrElement.spec.ts",
      "packages/compiler-ssr/__tests__/ssrFallthroughAttrs.spec.ts",
      "packages/compiler-ssr/__tests__/ssrInjectCssVars.spec.ts",
      "packages/compiler-ssr/__tests__/ssrPortal.spec.ts",
      "packages/compiler-ssr/__tests__/ssrScopeId.spec.ts",
      "packages/compiler-ssr/__tests__/ssrSlotOutlet.spec.ts",
      "packages/compiler-ssr/__tests__/ssrSuspense.spec.ts",
      "packages/compiler-ssr/__tests__/ssrText.spec.ts",
      "packages/compiler-ssr/__tests__/ssrTransition.spec.ts",
      "packages/compiler-ssr/__tests__/ssrTransitionGroup.spec.ts",
      "packages/compiler-ssr/__tests__/ssrVFor.spec.ts",
      "packages/compiler-ssr/__tests__/ssrVIf.spec.ts",
      "packages/compiler-ssr/__tests__/ssrVModel.spec.ts",
      "packages/compiler-ssr/__tests__/ssrVShow.spec.ts",
    ];
const result = spawnSync(
  process.execPath,
  [
    resolve(root, "node_modules/vitest/vitest.mjs"),
    "run",
    "--config",
    resolve(root, "tests/compiler-ssr-upstream.config.mjs"),
    ...suites,
    "--maxWorkers=1",
    "--no-file-parallelism",
    "--no-cache",
  ],
  { cwd: upstream, stdio: "inherit", env: { ...process.env, CI: "1" } },
);
process.exit(result.status ?? 1);
