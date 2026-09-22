#!/usr/bin/env node
// Executed through an arm-local wrapper. Settings are explicit, hashed run
// inputs, not an alternative compiler-optimization configuration surface.
import { spawnSync } from "node:child_process"
import { readFileSync } from "node:fs"
import { fileIdentity, invokeCompiler } from "./artifact-evidence.mjs"

export function runCompilerReceipt(settingsPath, args) {
  const settings = JSON.parse(readFileSync(settingsPath, "utf8"))
  if (fileIdentity(settings.compiler).sha256 !== settings.compilerSha256) throw new Error("pinned compiler identity changed")
  if (args.length === 1 && ["--version", "-V", "--help", "-h"].includes(args[0])) {
    return spawnSync(settings.compiler, args, { stdio: "inherit" }).status ?? 1
  }
  return invokeCompiler({ ...settings, args, cwd: process.cwd() }).exitCode
}
