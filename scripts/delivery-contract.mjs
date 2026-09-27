// Delivery contract assertions (plan M3.3; design §3 DL1–DL10, §14.3): each
// `tests/bundles` fixture's behaviour, and what its manifest v3 states about
// the delivered files. The expected outputs come from the sources' ES module
// semantics, never from the compiler under test.
//
//   node scripts/delivery-contract.mjs [compiler]
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { cp, mkdir, readdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

/// Each fixture: its delivery mode and what running `main.js` prints.
export const fixtures = [
  { name: "preserve", mode: "preserve-modules", expected: "42" },
  { name: "split", mode: "split", expected: "7" },
  { name: "lazy", mode: "split", expected: "42" },
  { name: "lazy-cycle", mode: "split", expected: "42" },
];

/// Build a fixture into `directory` (its `[delivery]` table decides the
/// files); returns its manifest's one output.
export async function build(compiler, name, directory) {
  await rm(directory, { recursive: true, force: true });
  await mkdir(directory, { recursive: true });
  const source = path.join(root, `tests/bundles/${name}/main.lil`);
  execFileSync(compiler, [source, "--target", "js-module", "--out-dir", directory], {
    cwd: root,
    stdio: "inherit",
  });
  await writeFile(path.join(directory, "package.json"), '{"type":"module"}\n');
  const manifest = JSON.parse(
    await readFile(path.join(directory, "lilscript.manifest.json"), "utf8"),
  );
  assert.equal(manifest.version, 3, `${name}: manifest version`);
  assert.equal(manifest.outputs.length, 1, `${name}: one output`);
  return manifest.outputs[0];
}

function run(file) {
  return execFileSync(process.execPath, [file], { encoding: "utf8" }).trim();
}

/// What a manifest output must state about its files (design §7.11, §10).
export async function assertManifest(name, directory, output) {
  const files = new Map(output.files.map((file) => [file.file, file]));
  for (const file of output.files) {
    const code = await readFile(path.join(directory, file.file));
    // DL9: the scored and listed bytes are the delivered bytes.
    assert.equal(code.length, file.bytes, `${name}: ${file.file} bytes`);
    assert.equal(
      createHash("sha256").update(code).digest("hex"),
      file.sha256,
      `${name}: ${file.file} sha256`,
    );
    assert.ok(Number.isInteger(file.codec_bytes) && file.codec_bytes > 0, `${name}: ${file.file} codec bytes`);
    // P7: no file imports an entry file, and a lazy file is only loaded.
    for (const target of file.imports) {
      const imported = files.get(target);
      assert.ok(imported, `${name}: ${file.file} imports a listed file`);
      assert.ok(
        imported.role !== "entry" && imported.role !== "lazy",
        `${name}: ${file.file} imports ${target}, a ${imported.role} file`,
      );
    }
  }
  // `side_effects`: exactly the files whose loading runs code.
  assert.deepEqual(
    [...output.side_effects].sort(),
    output.files.filter((file) => file.anchored).map((file) => file.file).sort(),
    `${name}: side_effects`,
  );
  for (const entry of output.entries) {
    // DL3: an entry runs only code of modules it reaches, so every file
    // with effects in its closure carries its name.
    for (const member of entry.closure) {
      const file = files.get(member);
      assert.ok(file, `${name}: ${entry.name} loads a listed file`);
      if (file.anchored) {
        assert.ok(
          file.label.includes(entry.name),
          `${name}: ${entry.name} runs ${member}, whose label is ${file.label}`,
        );
      }
    }
    // Rows (design §10): the objective's bytes of what the entry loads.
    const row = entry.closure.reduce((sum, member) => sum + files.get(member).codec_bytes, 0);
    assert.equal(entry.row, row, `${name}: ${entry.name} row`);
  }
  if (output.mode === "preserve-modules") {
    // DL8: a module's statements stay in its file.
    for (const file of output.files) {
      if (file.role === "module") {
        assert.equal(file.modules.length, 1, `${name}: ${file.file} is one module`);
      }
    }
  }
}

export async function verifyContract(compiler, outputRoot) {
  for (const fixture of fixtures) {
    const directory = path.join(outputRoot, fixture.name);
    const output = await build(compiler, fixture.name, directory);
    assert.equal(output.mode, fixture.mode, `${fixture.name}: mode`);
    assert.equal(run(path.join(directory, "main.js")), fixture.expected, `${fixture.name}: output`);
    await assertManifest(fixture.name, directory, output);
    if (fixture.name === "lazy") {
      // DL5: the lazy file serves only the members read, and `preload =
      // "entry"` preloads it; a missing lazy file rejects the task.
      const lazy = output.files.find((file) => file.role === "lazy");
      assert.ok(lazy, "lazy: a lazy file");
      const lazyCode = await readFile(path.join(directory, lazy.file), "utf8");
      assert.doesNotMatch(lazyCode, /99|unused/);
      assert.match(await readFile(path.join(directory, "main.js"), "utf8"), /modulepreload/);
      const lazyPath = path.join(directory, lazy.file);
      await rename(lazyPath, `${lazyPath}.missing`);
      assert.match(run(path.join(directory, "main.js")), /Cannot find module|ERR_MODULE_NOT_FOUND/);
      await rename(`${lazyPath}.missing`, lazyPath);
    }
    if (fixture.name === "split") {
      // DL1 and DL6: both entries share one instance of `left`.
      const probe = path.join(directory, "probe.mjs");
      await writeFile(
        probe,
        `const main=await import(${JSON.stringify(pathToFileURL(path.join(directory, "left.js")).href)});console.log(main.left());`,
      );
      assert.equal(run(probe), "3", "split: the left entry");
    }
    if (fixture.name === "preserve") {
      // A rebuild removes the files the previous manifest listed and this
      // build no longer writes, and nothing else.
      const manifestPath = path.join(directory, "lilscript.manifest.json");
      const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
      await writeFile(path.join(directory, "stale.js"), "export{};\n");
      await writeFile(path.join(directory, "kept.js"), "export{};\n");
      manifest.outputs[0].files.push({ file: "stale.js" });
      await writeFile(manifestPath, `${JSON.stringify(manifest)}\n`);
      execFileSync(compiler, [path.join(root, "tests/bundles/preserve/main.lil"), "--target", "js-module", "--out-dir", directory], { cwd: root });
      await assert.rejects(readFile(path.join(directory, "stale.js")), { code: "ENOENT" });
      assert.equal(await readFile(path.join(directory, "kept.js"), "utf8"), "export{};\n");
    }
  }
}

/// Every file under `directory`, by relative name, with its bytes.
async function tree(directory) {
  const files = new Map();
  for (const entry of await readdir(directory, { recursive: true, withFileTypes: true })) {
    if (entry.isFile()) {
      const full = path.join(entry.parentPath ?? entry.path, entry.name);
      files.set(path.relative(directory, full), await readFile(full));
    }
  }
  return files;
}

/// DL10 and the command line (design §4, §11): bytes, names and manifest do
/// not depend on `-j`, the working directory, the sources' absolute path or
/// the order of `--entry` flags; `-o FILE` names the entry file FILE.
export async function verifyCli(compiler, outputRoot) {
  await rm(outputRoot, { recursive: true, force: true });
  await mkdir(outputRoot, { recursive: true });
  const fixture = path.join(root, "tests/bundles/split");
  const copy = path.join(outputRoot, "elsewhere/deeper/split");
  await cp(fixture, copy, { recursive: true });
  // The copy declares its entries on the command line, in another order.
  await writeFile(
    path.join(copy, "lilscript.toml"),
    "[optimization]\ninlining = false\n\n[javascript]\nstrip_console = false\n\n[delivery]\nmode = \"split\"\n",
  );
  const first = path.join(outputRoot, "first");
  const second = path.join(outputRoot, "second");
  execFileSync(compiler, [path.join(fixture, "main.lil"), "--target", "js-module", "--out-dir", first, "-j", "1"], { cwd: root });
  execFileSync(
    compiler,
    ["--entry", "main=main.lil", "--entry", "left=left.lil", "--target", "js-module", "--out-dir", second, "-j", "4"],
    { cwd: copy },
  );
  const [a, b] = [await tree(first), await tree(second)];
  assert.deepEqual([...a.keys()].sort(), [...b.keys()].sort(), "cli: file names");
  for (const [name, bytes] of a) {
    assert.ok(bytes.equals(b.get(name)), `cli: ${name} differs between -j 1 here and -j 4 elsewhere`);
  }
  // `-o FILE`: FILE is the entry's file, named so in the manifest; a
  // rebuild keeps a file the compiler never wrote.
  const single = path.join(outputRoot, "o");
  await mkdir(single, { recursive: true });
  const preserve = path.join(root, "tests/bundles/preserve/main.lil");
  const file = path.join(single, "app.mjs");
  execFileSync(compiler, [preserve, "--target", "js-module", "-o", file], { cwd: root });
  const manifest = JSON.parse(await readFile(path.join(single, "app.manifest.json"), "utf8"));
  assert.equal(manifest.outputs[0].entries[0].file, "app.mjs", "cli: -o names the entry file");
  assert.deepEqual(manifest.outputs[0].files.map((file) => file.file).sort(), ["app.mjs", "state.mjs"]);
  await writeFile(path.join(single, "package.json"), '{"type":"module"}\n');
  assert.equal(run(file), "42", "cli: -o output");
  await writeFile(path.join(single, "main.mjs"), "export{};\n");
  execFileSync(compiler, [preserve, "--target", "js-module", "-o", file], { cwd: root });
  assert.equal(await readFile(path.join(single, "main.mjs"), "utf8"), "export{};\n", "cli: a file the compiler never wrote stays");
  // Refusals before any compile, and the removed flag that only warns.
  const refused = (args) => {
    assert.throws(() => execFileSync(compiler, args, { cwd: root, stdio: "pipe" }), undefined, args.join(" "));
  };
  refused([preserve, "--target", "js-module", "-o", file, "--out-dir", single]);
  refused([preserve, "--target", "all", "--out-dir", single]);
  refused(["--entry", `a=${preserve}`, "--entry", `b=${path.join(fixture, "left.lil")}`, "--target", "c"]);
  refused(["--entry", `a=${preserve}`, "--entry", `b=${path.join(fixture, "left.lil")}`, "--target", "js", "--print-policy"]);
  refused([preserve, "--target", "js-module", "--format", "cjs", "--out-dir", single]);
  const warned = execFileSync(compiler, [preserve, "--target", "js-module", "--codec-jobs", "2", "--print-policy"], {
    cwd: root,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  assert.equal(JSON.parse(warned).execution.codec_workers, 2, "cli: --codec-jobs is reported, with no effect");
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const compiler = process.argv[2] ?? path.join(root, "target/release/lilscript");
  await verifyContract(compiler, path.join(root, "target/verification/delivery-contract"));
  await verifyCli(compiler, path.join(root, "target/verification/delivery-cli"));
  console.log("Delivery contract passed.");
}
