import fs from "node:fs";
import { brotliCompressSync, constants, gzipSync } from "node:zlib";

const rows = [];
for (let index = 2; index < process.argv.length; index += 2) {
  const label = process.argv[index];
  const path = process.argv[index + 1];
  const normalized = fs.readFileSync(path, "utf8").trimEnd();
  fs.writeFileSync(path, normalized);
  const bytes = Buffer.from(normalized);
  rows.push({
    compiler: label,
    raw: bytes.length,
    gzip: gzipSync(bytes, { level: 9, mtime: 0 }).length,
    brotli: brotliCompressSync(bytes, {
      params: { [constants.BROTLI_PARAM_QUALITY]: 11 },
    }).length,
  });
}

console.log("| Compiler | Raw | Gzip-9 | Brotli-11 |");
console.log("| --- | ---: | ---: | ---: |");
for (const row of rows) {
  console.log(`| ${row.compiler} | ${row.raw} | ${row.gzip} | ${row.brotli} |`);
}

