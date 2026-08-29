import { spawnSync } from "node:child_process";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const upstream = resolve(root, "upstream/vue");
const build = spawnSync(
  process.execPath,
  [resolve(root, "scripts/build-compiler-dom.mjs")],
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
      "packages/compiler-dom/__tests__/decoderHtmlBrowser.spec.ts",
      "packages/compiler-dom/__tests__/parse.spec.ts",
      "packages/compiler-dom/__tests__/index.spec.ts",
      "packages/compiler-dom/__tests__/transforms/Transition.spec.ts",
      "packages/compiler-dom/__tests__/transforms/ignoreSideEffectTags.spec.ts",
      "packages/compiler-dom/__tests__/transforms/stringifyStatic.spec.ts",
      "packages/compiler-dom/__tests__/transforms/transformStyle.spec.ts",
      "packages/compiler-dom/__tests__/transforms/vHtml.spec.ts",
      "packages/compiler-dom/__tests__/transforms/vModel.spec.ts",
      "packages/compiler-dom/__tests__/transforms/vOn.spec.ts",
      "packages/compiler-dom/__tests__/transforms/vShow.spec.ts",
      "packages/compiler-dom/__tests__/transforms/vText.spec.ts",
      "packages/compiler-dom/__tests__/transforms/validateHtmlNesting.spec.ts",
    ];
const result = spawnSync(
  process.execPath,
  [
    resolve(root, "node_modules/vitest/vitest.mjs"),
    "run",
    "--config",
    resolve(root, "tests/compiler-dom-upstream.config.mjs"),
    ...suites,
    "--maxWorkers=1",
    "--no-file-parallelism",
    "--no-cache",
  ],
  { cwd: upstream, stdio: "inherit", env: { ...process.env, CI: "1" } },
);
process.exit(result.status ?? 1);
