import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { compilerPath, projectRoot } from "../tooling/compiler-path.mjs";

const directory = mkdtempSync(resolve(tmpdir(), "lilscript-cycle-value-"));
try {
  writeFileSync(
    resolve(directory, "a.lil"),
    'import { readLater } from "./b";\nexport int entry() { return readLater(); }\nexport int later = 7;\n',
  );
  writeFileSync(
    resolve(directory, "b.lil"),
    'import { later } from "./a";\nexport int readLater() { return later; }\n',
  );
  const output = resolve(directory, "out.mjs");
  const result = spawnSync(
    compilerPath(),
    [resolve(directory, "a.lil"), "--target", "js-module", "-o", output],
    { cwd: projectRoot, encoding: "utf8", env: process.env },
  );
  assert.equal(result.status, 0, `${result.stdout}${result.stderr}`);

  const runtime = spawnSync(
    process.execPath,
    [
      "--input-type=module",
      "--eval",
      `const module = await import(${JSON.stringify(pathToFileURL(output).href)}); console.log(module.entry());`,
    ],
    { cwd: projectRoot, encoding: "utf8", env: process.env },
  );
  assert.equal(runtime.status, 0, `${runtime.stdout}${runtime.stderr}`);
  assert.equal(runtime.stdout, "7\n");
} finally {
  rmSync(directory, { force: true, recursive: true });
}
