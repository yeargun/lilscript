import { spawnSync } from "node:child_process";
import { accessSync, constants } from "node:fs";
import { resolve } from "node:path";
import { compilerPath, lilscriptRoot } from "./compiler-path.mjs";

const hasCompilerOverride = Boolean(process.env.LILSCRIPT_COMPILER);
const hasCodecOverride = Boolean(process.env.LILSCRIPT_CODEC);
if (hasCompilerOverride !== hasCodecOverride) {
  throw new Error(
    "LILSCRIPT_COMPILER and LILSCRIPT_CODEC must be supplied together so measurements cannot mix unrelated builds",
  );
}

if (hasCompilerOverride) {
  accessSync(resolve(process.env.LILSCRIPT_CODEC), constants.X_OK);
  console.log(`LilScript compiler ready at ${compilerPath()}`);
  console.log(`LilScript codec scorer ready at ${process.env.LILSCRIPT_CODEC}`);
  process.exit(0);
}

const cargo = process.env.CARGO ?? "cargo";
const args = [
  "build",
  "--release",
  "--bin",
  "lilscript",
  "--bin",
  "lilscript-codec",
];
const result = spawnSync(cargo, args, {
  cwd: lilscriptRoot,
  stdio: "inherit",
  env: process.env,
});
if (result.status !== 0) process.exit(result.status ?? 1);
console.log(`LilScript compiler ready at ${compilerPath()}`);
console.log(
  `LilScript codec scorer ready at ${resolve(lilscriptRoot, "target/release/lilscript-codec")}`,
);
