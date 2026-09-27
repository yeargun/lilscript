import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFile, rm, mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { verifyCli, verifyContract } from "./delivery-contract.mjs";
import { verifyPlans } from "./delivery-plan.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const compiler = process.argv[2] ?? path.join(root, "target/release/lilscript");
const outputRoot = path.join(root, "target/verification/bundles");

await rm(outputRoot, { recursive: true, force: true });
await mkdir(outputRoot, { recursive: true });

// Plan M3.3: behaviour and manifest v3 assertions, then the pinned plans.
await verifyContract(compiler, path.join(outputRoot, "contract"));
await verifyPlans(compiler, path.join(outputRoot, "plan"));
await verifyCli(compiler, path.join(outputRoot, "cli"));
await verifyAllTarget();
await verifyPackageLock();

console.log("JavaScript bundle policies passed.");

async function verifyPackageLock() {
  const source = path.join(root, "tests/packages/app/main.lil");
  const output = path.join(outputRoot, "package.js");
  const lockfile = path.join(root, "tests/packages/app/lilscript.lock");
  const before = await readFile(lockfile, "utf8");
  assert.match(before, /name = "basekit"/, "transitive package is locked");
  execFileSync(
    compiler,
    [source, "--write-lock", "--target", "js", "-o", output],
    { cwd: root },
  );
  assert.equal(await readFile(lockfile, "utf8"), before, "deterministic lockfile");
  execFileSync(compiler, [source, "--target", "js", "-o", output], {
    cwd: root,
    stdio: "inherit",
  });
  const result = execFileSync(process.execPath, [output], {
    encoding: "utf8",
  }).trim();
  assert.equal(result, "42", "locked package import output");

  const undeclared = path.join(root, "tests/packages/app/undeclared.lil");
  assert.throws(
    () =>
      execFileSync(compiler, [undeclared, "--target", "js", "-o", output], {
        cwd: root,
        encoding: "utf8",
        stdio: "pipe",
      }),
    (error) => /not declared by root package/.test(error.stderr),
  );

  const dependency = path.join(root, "tests/packages/math/lib.lil");
  const original = await readFile(dependency, "utf8");
  try {
    await writeFile(dependency, `${original}\n// stale lock probe\n`);
    assert.throws(
      () =>
        execFileSync(compiler, [source, "--target", "js", "-o", output], {
          cwd: root,
          encoding: "utf8",
          stdio: "pipe",
        }),
      (error) => /lockfile is stale/.test(error.stderr),
    );
  } finally {
    await writeFile(dependency, original);
  }
}

async function verifyAllTarget() {
  // A program with no exports: a native build has no exported ABI yet.
  const directory = path.join(outputRoot, "all");
  const source = path.join(directory, "main.lil");
  const base = path.join(directory, "app");
  await mkdir(directory, { recursive: true });
  await writeFile(path.join(directory, "package.json"), '{"type":"module"}\n');
  await writeFile(path.join(directory, "lilscript.toml"), "[javascript]\nstrip_console = false\n");
  await writeFile(
    source,
    "int state = 40;\nvoid setState(int value) { state = value; }\nsetState(41);\nprint(state + 1);\n",
  );
  // `--target all` builds a script and a native program, one file each.
  execFileSync(compiler, [source, "--target", "all", "-o", base], {
    cwd: root,
    stdio: "inherit",
  });

  const nativeResult = execFileSync(base, { encoding: "utf8" }).trim();
  assert.equal(nativeResult, "42", "all-target native output");
  const jsResult = execFileSync(process.execPath, [`${base}.js`], {
    encoding: "utf8",
  }).trim();
  assert.equal(jsResult, "42", "all-target JavaScript output");
}
