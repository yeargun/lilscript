import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const root = process.argv[2];
if (!root) throw new Error("comparison root is required");

const failures = [];
let count = 0;
for (const entry of readdirSync(join(root, "apps"), { withFileTypes: true })) {
  if (!entry.isDirectory()) continue;
  const report = JSON.parse(
    readFileSync(
      join(root, "apps", entry.name, "build", "report.json"),
      "utf8",
    ),
  );
  if (report.schemaVersion !== 4 || !report.objectiveContract) {
    failures.push(
      `${entry.name}: stale report does not identify objective-specific artifacts`,
    );
    continue;
  }
  if (
    !/^[0-9a-f]{64}$/u.test(report.toolVersions?.lilscript?.digest ?? "") ||
    !/^[0-9a-f]{64}$/u.test(report.toolVersions?.closure?.digest ?? "") ||
    !report.provenance?.configs?.raw?.digest ||
    !report.provenance?.configs?.gzip?.digest ||
    !report.provenance?.configs?.brotli?.digest
  ) {
    failures.push(`${entry.name}: report lacks compiler/config digests`);
    continue;
  }
  if (
    report.codecs?.implementation !== "lilscript-codec" ||
    report.codecs?.schemaVersion !== 1 ||
    report.codecs?.gzip9?.libraryVersion !== "1.3.1" ||
    report.codecs?.brotli11?.libraryVersion !== "1.1.0" ||
    !/^[0-9a-f]{64}$/u.test(report.codecs?.scorer?.sha256 ?? "")
  ) {
    failures.push(
      `${entry.name}: report is not backed by the pinned canonical codec scorer`,
    );
    continue;
  }
  count += 1;
  for (const metric of ["raw", "gzip9", "brotli11"]) {
    const objectiveArtifact = report.lilscriptArtifacts?.[metric];
    if (
      !objectiveArtifact ||
      objectiveArtifact.sizes?.[metric] !== report.lilscript?.[metric]
    ) {
      failures.push(
        `${entry.name}/${metric}: report value is not backed by its objective-specific artifact`,
      );
      continue;
    }
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
console.log(
  `Closure parity gate passed for ${count} maintained application pairs.`,
);
