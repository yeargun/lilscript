import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { brotliCompressSync, constants, gzipSync } from "node:zlib";

const directory = dirname(fileURLToPath(import.meta.url));
const root = resolve(directory, "../..");
const compiler = process.env.LILSCRIPT ?? join(root, "target/release/lilscript");
const cargo = process.env.CARGO ?? join(process.env.HOME ?? "", ".cargo/bin/cargo");
const build = join(root, "target/finite-value-benchmark");
const source = join(root, "tests/cases/interprocedural_finite_values.lil");
const expected = readFileSync(
  join(root, "tests/cases/interprocedural_finite_values.out"),
  "utf8",
).trimEnd();

function command(executable, args, capture = false) {
  return execFileSync(executable, args, {
    cwd: root,
    encoding: "utf8",
    stdio: capture ? ["ignore", "pipe", "pipe"] : "inherit",
  });
}

function sizes(path) {
  const bytes = readFileSync(path);
  return {
    raw: bytes.length,
    gzip: gzipSync(bytes, { level: 9, mtime: 0 }).length,
    brotli: brotliCompressSync(bytes, {
      params: { [constants.BROTLI_PARAM_QUALITY]: 11 },
    }).length,
  };
}

mkdirSync(build, { recursive: true });
if (!existsSync(compiler)) {
  command(cargo, ["build", "--release", "--bin", "lilscript"]);
}
const variants = [
  ["finite values enabled", "no-inlining.toml", "enabled.js"],
  ["finite values disabled", "no-finite-values.toml", "disabled.js"],
];
const results = [];
for (const [label, config, file] of variants) {
  const output = join(build, file);
  command(compiler, [
    source,
    "--config",
    join(root, "tests/config", config),
    "-o",
    output,
  ]);
  const actual = command(process.execPath, [output], true).trimEnd();
  if (actual !== expected) {
    throw new Error(`${label} output mismatch\nexpected:\n${expected}\nactual:\n${actual}`);
  }
  results.push({ label, ...sizes(output) });
}

const [enabled, disabled] = results;
for (const metric of ["raw", "gzip", "brotli"]) {
  if (enabled[metric] >= disabled[metric]) {
    throw new Error(
      `finite value propagation did not reduce ${metric}: ${enabled[metric]} >= ${disabled[metric]}`,
    );
  }
}

console.log("| Variant | Raw | Gzip-9 | Brotli-11 |");
console.log("| --- | ---: | ---: | ---: |");
for (const result of results) {
  console.log(
    `| ${result.label} | ${result.raw} | ${result.gzip} | ${result.brotli} |`,
  );
}
