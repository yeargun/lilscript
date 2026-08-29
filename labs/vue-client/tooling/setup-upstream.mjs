import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import { projectRoot } from "./compiler-path.mjs";

const upstreamRevision = "d63616ca17de965ed32dcb449a4c5cd9982f15d2";
const upstreamTag = "v3.5.42";
const upstreamRepository = "https://github.com/vuejs/core.git";
const upstreamParent = resolve(projectRoot, "upstream");
const upstreamRoot = resolve(upstreamParent, "vue");

function run(program, args, cwd, { capture = false, env = {} } = {}) {
  const result = spawnSync(program, args, {
    cwd,
    encoding: capture ? "utf8" : undefined,
    stdio: capture ? ["ignore", "pipe", "pipe"] : "inherit",
    env: { ...process.env, ...env },
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    if (capture) process.stderr.write(result.stderr ?? result.stdout ?? "");
    process.exit(result.status ?? 1);
  }
  return capture ? result.stdout.trim() : "";
}

if (!existsSync(resolve(upstreamRoot, "package.json"))) {
  mkdirSync(upstreamParent, { recursive: true });
  if (existsSync(upstreamRoot) && readdirSync(upstreamRoot).length > 0) {
    throw new Error(`${upstreamRoot} is non-empty but is not a Vue checkout`);
  }
  run("git", ["clone", "--depth", "1", "--branch", upstreamTag, upstreamRepository, upstreamRoot], projectRoot);
}

const actualRevision = run("git", ["rev-parse", "HEAD"], upstreamRoot, {
  capture: true,
});
if (actualRevision !== upstreamRevision) {
  throw new Error(`Vue upstream must be ${upstreamRevision}; found ${actualRevision}`);
}
const changes = run(
  "git",
  ["status", "--porcelain", "--untracked-files=no"],
  upstreamRoot,
  { capture: true },
);
if (changes !== "") {
  throw new Error("Vue upstream contains tracked changes");
}

run("corepack", ["pnpm", "install", "--frozen-lockfile"], upstreamRoot, {
  env: { SKIP_INSTALL_SIMPLE_GIT_HOOKS: "1" },
});
