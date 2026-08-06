import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const root = process.argv[2];
if (!root) throw new Error("comparison root is required");

const failures = [];
let count = 0;
for (const entry of readdirSync(join(root, "apps"), { withFileTypes: true })) {
  if (!entry.isDirectory()) continue;
  const report = JSON.parse(
    readFileSync(join(root, "apps", entry.name, "build", "report.json"), "utf8"),
  );
  count += 1;
  for (const metric of ["raw", "gzip9", "brotli11"]) {
    if (report.lilscript[metric] > report.closure[metric]) {
      failures.push(
        `${entry.name}/${metric}: LilScript ${report.lilscript[metric]} > Closure ${report.closure[metric]}`,
      );
    }
  }
}

if (failures.length > 0) {
  throw new Error(`Closure parity gate failed:\n${failures.join("\n")}`);
}
console.log(`Closure parity gate passed for ${count} maintained application pairs.`);
