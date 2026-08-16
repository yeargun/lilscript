import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { accessSync, constants, readFileSync, realpathSync } from "node:fs";
import { relative, resolve, sep } from "node:path";

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function portablePath(repositoryRoot, path) {
  const repositoryRelative = relative(repositoryRoot, path);
  if (
    repositoryRelative !== "" &&
    repositoryRelative !== ".." &&
    !repositoryRelative.startsWith(`..${sep}`)
  ) {
    return repositoryRelative.split(sep).join("/");
  }
  return path;
}

function command(program, args, cwd, context) {
  const result = spawnSync(program, args, {
    cwd,
    encoding: "utf8",
    env: process.env,
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.error) {
    throw new Error(
      `${context} could not execute ${program}: ${result.error.message}`,
    );
  }
  if (result.status !== 0) {
    throw new Error(
      `${context}: ${program} ${args.join(" ")} exited ${result.status}\n` +
        `${result.stdout ?? ""}${result.stderr ?? ""}`,
    );
  }
  return result.stdout.trim();
}

export function resolveCompilerToolchain(repositoryRoot, context) {
  const compilerOverride = process.env.LILSCRIPT;
  const scorerOverride = process.env.LILSCRIPT_CODEC;
  if (Boolean(compilerOverride) !== Boolean(scorerOverride)) {
    throw new Error(
      `${context} requires LILSCRIPT and LILSCRIPT_CODEC to be supplied together`,
    );
  }
  const suffix = process.platform === "win32" ? ".exe" : "";
  const requested = compilerOverride
    ? resolve(process.cwd(), compilerOverride)
    : resolve(repositoryRoot, `target/release/lilscript${suffix}`);
  accessSync(requested, constants.R_OK | constants.X_OK);
  const executable = realpathSync(requested);
  const bytes = readFileSync(executable);
  return {
    executable,
    evidence: {
      path: portablePath(repositoryRoot, executable),
      absolutePath: executable,
      version: command(executable, ["--version"], repositoryRoot, context),
      sha256: sha256(bytes),
      bytes: bytes.length,
      source: compilerOverride ? "environment-pair" : "repository-release",
    },
  };
}

function javascriptCostModels(source) {
  let table = null;
  const values = [];
  for (const sourceLine of source.split(/\r?\n/u)) {
    const line = sourceLine.replace(/\s+#.*$/u, "").trim();
    if (line === "") continue;
    const tableMatch = line.match(/^\[([^\]]+)\]$/u);
    if (tableMatch) {
      table = tableMatch[1].trim();
      continue;
    }
    if (table !== "javascript") continue;
    const costModel = line.match(/^cost_model\s*=\s*["']([^"']+)["']\s*$/u);
    if (costModel) values.push(costModel[1]);
  }
  return values;
}

export function resolveBrotliConfig(configPath, repositoryRoot, context) {
  const requested = resolve(configPath);
  const path = realpathSync(requested);
  const bytes = readFileSync(path);
  const costModels = javascriptCostModels(bytes.toString("utf8"));
  if (costModels.length !== 1 || costModels[0] !== "brotli") {
    throw new Error(
      `${context} requires exactly one explicit [javascript] cost_model = "brotli" in ${path}; ` +
        `found ${JSON.stringify(costModels)}`,
    );
  }
  return {
    resolvedPath: path,
    evidence: {
      path: portablePath(repositoryRoot, path),
      absolutePath: path,
      sha256: sha256(bytes),
      bytes: bytes.length,
      costModel: "brotli",
    },
  };
}
