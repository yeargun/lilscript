import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFile, rm, mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const compiler = process.argv[2] ?? path.join(root, "target/release/lilscript");
const outputRoot = path.join(root, "target/verification/bundles");

await rm(outputRoot, { recursive: true, force: true });
await mkdir(outputRoot, { recursive: true });

await verifyBundle("preserve", "42", "preserve-modules", ["state.lil"]);
await verifyBundle("split", "7", "split", ["shared.lil"]);
await verifyAllTarget();

console.log("JavaScript bundle policies passed.");

async function verifyBundle(name, expected, mode, modules) {
  const source = path.join(root, `tests/bundles/${name}/main.lil`);
  const directory = path.join(outputRoot, name);
  const entry = path.join(directory, "entry.mjs");
  await mkdir(directory, { recursive: true });
  execFileSync(compiler, [source, "--target", "js-module", "-o", entry], {
    cwd: root,
    stdio: "inherit",
  });

  const result = execFileSync(process.execPath, [entry], {
    encoding: "utf8",
  }).trim();
  assert.equal(result, expected, `${name} bundle output`);

  const manifestPath = path.join(directory, "entry.manifest.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  assert.equal(manifest.version, 1);
  assert.equal(manifest.mode, mode);
  assert.equal(manifest.entry, "entry.mjs");
  assert.deepEqual(
    manifest.chunks.flatMap((chunk) => chunk.modules),
    modules,
  );
  for (const chunk of manifest.chunks) {
    const code = await readFile(path.join(directory, chunk.file), "utf8");
    assert.equal(Buffer.byteLength(code), chunk.bytes);
  }
  if (name === "preserve") {
    const stale = "chunk-stale.mjs";
    await writeFile(path.join(directory, stale), "export{};\n");
    manifest.chunks.push({ file: stale, modules: [], bytes: 10 });
    await writeFile(manifestPath, `${JSON.stringify(manifest)}\n`);
    execFileSync(compiler, [source, "--target", "js-module", "-o", entry]);
    await assert.rejects(readFile(path.join(directory, stale)), {
      code: "ENOENT",
    });
  }
}

async function verifyAllTarget() {
  const source = path.join(root, "tests/bundles/preserve/main.lil");
  const directory = path.join(outputRoot, "all");
  const base = path.join(directory, "app");
  await mkdir(directory, { recursive: true });
  await writeFile(path.join(directory, "package.json"), '{"type":"module"}\n');
  execFileSync(compiler, [source, "--target", "all", "-o", base], {
    cwd: root,
    stdio: "inherit",
  });

  const nativeResult = execFileSync(base, { encoding: "utf8" }).trim();
  assert.equal(nativeResult, "42", "all-target native output");
  const jsResult = execFileSync(process.execPath, [`${base}.js`], {
    encoding: "utf8",
  }).trim();
  assert.equal(jsResult, "42", "all-target chunked JavaScript output");
  const manifest = JSON.parse(await readFile(`${base}.manifest.json`, "utf8"));
  assert.equal(manifest.mode, "preserve-modules");
}
