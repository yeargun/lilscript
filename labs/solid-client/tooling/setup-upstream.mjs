import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import { projectRoot } from "./compiler-path.mjs";

const upstreamRevision = "3be495cec52bf78d7cc61f054af00320ecf4058c";
const upstreamTag = "v1.9.13";
const upstreamRepository = "https://github.com/solidjs/solid.git";
const upstreamParent = resolve(projectRoot, "upstream");
const upstreamRoot = resolve(upstreamParent, "solid");

function run(program, args, cwd, { capture = false, environment = {} } = {}) {
  const result = spawnSync(program, args, {
    cwd,
    encoding: capture ? "utf8" : undefined,
    stdio: capture ? ["ignore", "pipe", "pipe"] : "inherit",
    env: { ...process.env, ...environment },
    maxBuffer: 32 * 1024 * 1024,
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
    throw new Error(
      `${upstreamRoot} is a partial non-empty checkout; remove or repair it before setup`,
    );
  }
  run("git", ["init", upstreamRoot], projectRoot);
  run(
    "git",
    ["-C", upstreamRoot, "remote", "add", "origin", upstreamRepository],
    projectRoot,
  );
  run(
    "git",
    ["-C", upstreamRoot, "fetch", "--depth", "1", "origin", "tag", upstreamTag],
    projectRoot,
  );
  run(
    "git",
    ["-C", upstreamRoot, "checkout", "--detach", "FETCH_HEAD"],
    projectRoot,
  );
}

const actualRevision = run(
  "git",
  ["-C", upstreamRoot, "rev-parse", "HEAD"],
  projectRoot,
  { capture: true },
);
if (actualRevision !== upstreamRevision) {
  throw new Error(
    `Solid upstream must be pinned at ${upstreamRevision}; found ${actualRevision}`,
  );
}
const trackedChanges = run(
  "git",
  ["-C", upstreamRoot, "status", "--porcelain", "--untracked-files=no"],
  projectRoot,
  { capture: true },
);
if (trackedChanges !== "") {
  throw new Error(
    "Solid upstream contains tracked changes; evidence requires an unmodified checkout",
  );
}

run("corepack", ["pnpm", "install", "--frozen-lockfile"], upstreamRoot, {
  environment: { SKIP_INSTALL_SIMPLE_GIT_HOOKS: "1" },
});
