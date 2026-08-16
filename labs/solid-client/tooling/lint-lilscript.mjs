import { existsSync, readdirSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { compilerPath, projectRoot } from "./compiler-path.mjs";

function collect(directory) {
  if (!existsSync(directory)) return [];
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return collect(path);
    return entry.isFile() && entry.name.endsWith(".lil") ? [path] : [];
  });
}

const entries = collect(resolve(projectRoot, "apps"))
  .concat(collect(resolve(projectRoot, "benchmarks")))
  .filter((path) => basename(path) === "main.lil");

for (const entry of entries) {
  if (!statSync(entry).isFile()) continue;
  const output = join(
    tmpdir(),
    `lilscript-lint-${process.pid}-${basename(entry)}.js`,
  );
  const result = spawnSync(
    compilerPath(),
    [entry, "--target", "js", "-o", output],
    {
      encoding: "utf8",
      env: process.env,
    },
  );
  if (result.status !== 0) {
    console.error(result.stderr.trim());
    process.exit(result.status ?? 1);
  }
}

console.log(
  `${entries.length} LilScript entry modules passed compiler-backed linting.`,
);
