import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { brotliCompressSync, constants, gzipSync } from "node:zlib";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const compiler = process.env.LILSCRIPT ?? join(root, "target/release/lilscript");
const cargo = process.env.CARGO ?? join(process.env.HOME ?? "", ".cargo/bin/cargo");

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

export function runPassAblation({
  id,
  source,
  expected,
  variants,
  strictMetrics = ["raw", "gzip", "brotli"],
  nonRegressionMetrics = [],
}) {
  const build = join(root, "target/pass-ablation", id);
  const expectedOutput = readFileSync(join(root, expected), "utf8").trimEnd();
  mkdirSync(build, { recursive: true });
  if (!existsSync(compiler)) {
    command(cargo, ["build", "--release", "--bin", "lilscript"]);
  }

  const results = [];
  for (const [label, config, file] of variants) {
    const output = join(build, file);
    command(compiler, [
      join(root, source),
      "--config",
      join(root, config),
      "-o",
      output,
    ]);
    const actual = command(process.execPath, [output], true).trimEnd();
    if (actual !== expectedOutput) {
      throw new Error(
        `${label} output mismatch\nexpected:\n${expectedOutput}\nactual:\n${actual}`,
      );
    }
    results.push({ label, ...sizes(output) });
  }

  const [enabled, disabled] = results;
  for (const metric of strictMetrics) {
    if (enabled[metric] >= disabled[metric]) {
      throw new Error(
        `${id} did not reduce ${metric}: ${enabled[metric]} >= ${disabled[metric]}`,
      );
    }
  }
  for (const metric of nonRegressionMetrics) {
    if (enabled[metric] > disabled[metric]) {
      throw new Error(
        `${id} regressed ${metric}: ${enabled[metric]} > ${disabled[metric]}`,
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
}
