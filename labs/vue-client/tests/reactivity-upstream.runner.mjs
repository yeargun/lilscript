import { spawnSync } from "node:child_process";
import { resolve } from "node:path";

const projectRoot = resolve(import.meta.dirname, "..");
const upstreamRoot = resolve(projectRoot, "upstream/vue");
const vitest = resolve(projectRoot, "node_modules/vitest/vitest.mjs");
const config = resolve(projectRoot, "tests/reactivity-upstream.config.mjs");
const tests = [
  "packages/reactivity/__tests__/collections/Map.spec.ts",
  "packages/reactivity/__tests__/collections/Set.spec.ts",
  "packages/reactivity/__tests__/collections/WeakMap.spec.ts",
  "packages/reactivity/__tests__/collections/WeakSet.spec.ts",
  "packages/reactivity/__tests__/collections/shallowReadonly.spec.ts",
  "packages/reactivity/__tests__/computed.spec.ts",
  "packages/reactivity/__tests__/effectScope.spec.ts",
  "packages/reactivity/__tests__/effect.spec.ts",
  "packages/reactivity/__tests__/gc.spec.ts",
  "packages/reactivity/__tests__/reactive.spec.ts",
  "packages/reactivity/__tests__/reactiveArray.spec.ts",
  "packages/reactivity/__tests__/readonly.spec.ts",
  "packages/reactivity/__tests__/ref.spec.ts",
  "packages/reactivity/__tests__/shallowReactive.spec.ts",
  "packages/reactivity/__tests__/shallowReadonly.spec.ts",
  "packages/reactivity/__tests__/watch.spec.ts",
];

const build = spawnSync(process.execPath, [resolve(projectRoot, "scripts/build-reactivity.mjs")], {
  cwd: projectRoot,
  encoding: "utf8",
  env: process.env,
});
if (build.status !== 0) {
  process.stderr.write(`${build.stdout}${build.stderr}`);
  process.exit(build.status ?? 1);
}

const result = spawnSync(process.execPath, [vitest, "run", "--config", config, ...tests], {
  cwd: upstreamRoot,
  stdio: "inherit",
  env: { ...process.env, CI: process.env.CI ?? "1" },
});
process.exit(result.status ?? 1);
