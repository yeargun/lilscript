import { existsSync, mkdirSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { monacoEditorVersion, monacoEditorCommitId, vscodeCommitId } from "./catalog.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const vendorRoot = join(here, "../vendor");

function run(cwd, program, args) {
  const result = spawnSync(program, args, { cwd, encoding: "utf8", stdio: "inherit" });
  if (result.status !== 0) {
    throw new Error(`${program} ${args.join(" ")} failed`);
  }
}

function gitOut(dir, args) {
  const result = spawnSync("git", args, { cwd: dir, encoding: "utf8" });
  return result.status === 0 ? result.stdout.trim() : "";
}

function alreadyPinned(dir, pin) {
  const head = gitOut(dir, ["rev-parse", "HEAD"]);
  if (head === pin || (pin.length >= 7 && head.startsWith(pin))) {
    return true;
  }
  return gitOut(dir, ["describe", "--tags", "--exact-match", "HEAD"]) === pin;
}

function ensureSparse(url, dir, commit, sparsePaths) {
  mkdirSync(vendorRoot, { recursive: true });
  if (!existsSync(join(dir, ".git"))) {
    run(vendorRoot, "git", ["clone", "--filter=blob:none", "--sparse", "--no-checkout", url, dir]);
    run(dir, "git", ["sparse-checkout", "set", ...sparsePaths]);
  }
  if (alreadyPinned(dir, commit)) {
    console.log(`already at ${commit} in ${dir}`);
    return;
  }
  run(dir, "git", ["fetch", "--depth", "1", "origin", commit]);
  run(dir, "git", ["checkout", "--detach", commit]);
}

ensureSparse(
  "https://github.com/microsoft/vscode.git",
  join(vendorRoot, "vscode"),
  vscodeCommitId,
  ["src/vs/base", "src/vs/editor", "src/vs/platform", "src/vs/nls"],
);

ensureSparse(
  "https://github.com/microsoft/monaco-editor.git",
  join(vendorRoot, "monaco-editor"),
  monacoEditorCommitId,
  ["src/features", "src/languages"],
);

console.log(`vendor pins: vscode@${vscodeCommitId} monaco-editor@${monacoEditorVersion} (${monacoEditorCommitId})`);
