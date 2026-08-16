import { join, resolve } from "node:path";

import {
  requireExistingLilscriptToolchain,
  requirePairedLilscriptOverrides,
} from "../codec-contract.mjs";

export const scenarioToolchainContext = "scenario publication";

export function resolveScenarioToolchain({
  repoRoot,
  env = process.env,
  cwd = process.cwd(),
  platform = process.platform,
}) {
  const { compilerOverride, codecOverride } =
    requirePairedLilscriptOverrides(scenarioToolchainContext, env);
  const executableSuffix = platform === "win32" ? ".exe" : "";
  const compiler = compilerOverride
    ? resolve(cwd, compilerOverride)
    : join(repoRoot, `target/release/lilscript${executableSuffix}`);
  const codecScorer = codecOverride
    ? resolve(cwd, codecOverride)
    : join(repoRoot, `target/release/lilscript-codec${executableSuffix}`);
  return {
    compiler,
    codecScorer,
    explicitOverrides: Boolean(compilerOverride),
  };
}

export function prepareScenarioToolchain({
  repoRoot,
  cargo,
  command,
  env = process.env,
  cwd = process.cwd(),
  platform = process.platform,
  requireToolchain = requireExistingLilscriptToolchain,
}) {
  const resolved = resolveScenarioToolchain({ repoRoot, env, cwd, platform });
  if (resolved.explicitOverrides) {
    requireToolchain(
      scenarioToolchainContext,
      resolved.compiler,
      resolved.codecScorer,
    );
    return { ...resolved, toolchainSource: "explicit-overrides" };
  }

  const buildEnv = {
    ...env,
    CARGO_TARGET_DIR: join(repoRoot, "target"),
  };
  delete buildEnv.CARGO_BUILD_TARGET;
  command(
    cargo,
    ["build", "--release", "--bin", "lilscript", "--bin", "lilscript-codec"],
    { cwd: repoRoot, env: buildEnv },
  );
  requireToolchain(
    scenarioToolchainContext,
    resolved.compiler,
    resolved.codecScorer,
  );
  return { ...resolved, toolchainSource: "current-checkout-build" };
}
