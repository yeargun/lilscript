import { accessSync, constants } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import {
  codecPath,
  compilerPath,
  lilscriptRoot,
} from "./compiler-path.mjs";

const overriddenCompiler = Boolean(process.env.LILSCRIPT_COMPILER);
const overriddenCodec = Boolean(process.env.LILSCRIPT_CODEC);
if (overriddenCompiler !== overriddenCodec) {
  throw new Error(
    "LILSCRIPT_COMPILER and LILSCRIPT_CODEC must be supplied together",
  );
}

if (overriddenCompiler) {
  accessSync(resolve(process.env.LILSCRIPT_COMPILER), constants.X_OK);
  accessSync(resolve(process.env.LILSCRIPT_CODEC), constants.X_OK);
} else {
  const result = spawnSync(
    process.env.CARGO ?? "cargo",
    [
      "build",
      "--release",
      "--bin",
      "lilscript",
      "--bin",
      "lilscript-codec",
    ],
    { cwd: lilscriptRoot, stdio: "inherit", env: process.env },
  );
  if (result.status !== 0) process.exit(result.status ?? 1);
}

console.log(`LilScript compiler: ${compilerPath()}`);
console.log(`LilScript codec: ${codecPath()}`);
