import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const labRoot = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(labRoot, "../..");
const compiler = join(repoRoot, "target/release/lilscript");
const expected = readFileSync(join(labRoot, "apps/motion/expected.txt"), "utf8").trim();

function run(program, args, cwd = labRoot) {
  const result = spawnSync(program, args, {
    cwd,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(`${program} ${args.join(" ")}\n${result.stdout}${result.stderr}`);
  }
  return result.stdout.trim();
}

const lilOut = join(labRoot, "build/motion-verify-lilscript.js");
run(compiler, [join(labRoot, "apps/motion/lil/main.lil"), "-o", lilOut]);
const lilStdout = run(process.execPath, [lilOut]);
if (lilStdout !== expected) {
  throw new Error(`lilscript motion contract failed:\n${lilStdout}\n!=\n${expected}`);
}

const npmStdout = run(process.execPath, [join(labRoot, "apps/motion/js/main.js")]);
if (npmStdout !== expected) {
  throw new Error(`npm motion contract failed:\n${npmStdout}\n!=\n${expected}`);
}

const exportNames = Object.keys(await import("motion")).sort();
const digest = createHash("sha256").update(exportNames.join("\n")).digest("hex");
console.log(`motion-upstream:2:${exportNames.length}:${digest.slice(0, 16)}`);
