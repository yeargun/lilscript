import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFile, rename, rm, mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const compiler = process.argv[2] ?? path.join(root, "target/release/lilscript");
const outputRoot = path.join(root, "target/verification/bundles");

await rm(outputRoot, { recursive: true, force: true });
await mkdir(outputRoot, { recursive: true });

await verifyBundle("preserve", "42", "preserve-modules", ["state.lil"]);
await verifyBundle("split", "7", "split", ["shared.lil"]);
await verifyBundle("lazy", "42", "split", ["feature.lil"]);
await verifyBundle("lazy-cycle", "42", "split", ["feature.lil"]);
await verifyAllTarget();
await verifyPackageLock();

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
  assert.equal(manifest.version, 2);
  assert.match(manifest.build_id, /^[0-9a-f]{64}$/);
  assert.equal(typeof manifest.deploy_cost, "number");
  assert.equal(manifest.mode, mode);
  assert.equal(manifest.entry, "entry.mjs");
  assert.deepEqual(
    manifest.chunks.flatMap((chunk) => chunk.modules),
    modules,
  );
  for (const chunk of manifest.chunks) {
    const code = await readFile(path.join(directory, chunk.file), "utf8");
    assert.equal(Buffer.byteLength(code), chunk.bytes);
    assert.ok(chunk.gzip_bytes > 0);
    assert.ok(chunk.brotli_bytes > 0);
    assert.match(chunk.cache_key, /^[0-9a-f]{64}$/);
    assert.ok(Array.isArray(chunk.dependencies));
    assert.ok(Array.isArray(chunk.dynamic_dependencies));
  }
  if (name === "lazy") {
    const [chunk] = manifest.chunks;
    assert.equal(chunk.kind, "lazy");
    assert.deepEqual(manifest.preload, [chunk.file]);
    assert.deepEqual(chunk.dependencies, []);
    const lazyCode = await readFile(path.join(directory, chunk.file), "utf8");
    assert.doesNotMatch(lazyCode, /99|unused/);
    const entryCode = await readFile(entry, "utf8");
    assert.match(entryCode, /modulepreload/);
    const chunkPath = path.join(directory, chunk.file);
    const hiddenPath = `${chunkPath}.missing`;
    await rename(chunkPath, hiddenPath);
    const failure = execFileSync(process.execPath, [entry], {
      encoding: "utf8",
    }).trim();
    assert.match(failure, /Cannot find module|ERR_MODULE_NOT_FOUND/);
    await rename(hiddenPath, chunkPath);
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
