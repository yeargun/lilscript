import { spawnSync } from "node:child_process";
import { statSync } from "node:fs";
import { resolve } from "node:path";
import { compilerPath, projectRoot } from "../tooling/compiler-path.mjs";

const source = resolve(projectRoot, "src", "shared", "index.lil");
const output = resolve(
  projectRoot,
  "artifacts",
  "shared-runtime.generated.js",
);
const config = resolve(projectRoot, "config", "open-world.toml");

const result = spawnSync(
  compilerPath(),
  [source, "--target", "js-module", "--config", config, "-o", output],
  { cwd: projectRoot, stdio: "inherit", env: process.env },
);

if (result.status !== 0) process.exit(result.status ?? 1);

const runtime = await import(`../packages/vuelil/shared.js?build=${Date.now()}`);
if (Object.keys(runtime).length !== 73) {
  throw new Error(
    `shared facade exports ${Object.keys(runtime).length} names instead of 73`,
  );
}

console.log(
  `Built ${output} (${statSync(output).size} bytes, ${Object.keys(runtime).length} exports)`,
);
