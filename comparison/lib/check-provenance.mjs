import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const root = process.argv[2];
if (!root) throw new Error("comparison root is required");

const readJson = (path) => JSON.parse(readFileSync(path, "utf8"));
const appReports = readdirSync(join(root, "apps"), { withFileTypes: true })
  .filter((entry) => entry.isDirectory())
  .map((entry) => [
    `app:${entry.name}`,
    readJson(join(root, "apps", entry.name, "build", "report.json")),
  ]);
const reports = [
  ...appReports,
  ["cases", readJson(join(root, "cases", "summary.json"))],
  ["algorithms", readJson(join(root, "algorithms", "summary.json"))],
];

const failures = [];
const expectedSchemas = new Map([
  ["cases", 5],
  ["algorithms", 2],
]);
for (const [label, report] of reports) {
  const expectedSchema = label.startsWith("app:")
    ? 4
    : expectedSchemas.get(label);
  if (report.schemaVersion !== expectedSchema) {
    failures.push(
      `${label}: expected schema ${expectedSchema}, received ${report.schemaVersion}`,
    );
  }
  if (!label.startsWith("app:") && report.selectedBy !== "all") {
    failures.push(
      `${label}: focused report ${JSON.stringify(report.selectedBy)} cannot satisfy release provenance`,
    );
  }
}

const identity = ([label, report]) => ({
  label,
  compiler: report.toolVersions?.lilscript?.digest,
  scorer: report.codecs?.scorer?.sha256,
  codecs: JSON.stringify({
    schemaVersion: report.codecs?.schemaVersion,
    gzip9: report.codecs?.gzip9,
    brotli11: report.codecs?.brotli11,
  }),
  configs: JSON.stringify(
    Object.fromEntries(
      ["raw", "gzip", "brotli"].map((metric) => [
        metric,
        report.provenance?.configs?.[metric]?.digest,
      ]),
    ),
  ),
});
const identities = reports.map(identity);
for (const field of ["compiler", "scorer", "codecs", "configs"]) {
  const expected = identities[0][field];
  for (const candidate of identities) {
    if (!candidate[field] || candidate[field] !== expected) {
      failures.push(
        `${candidate.label}: ${field} provenance differs from ${identities[0].label}`,
      );
    }
  }
}

const closureRelease = (version) => String(version ?? "").match(/\d{8}/u)?.[0];
const appClosureIdentities = appReports.map(([label, report]) => ({
  label,
  release: closureRelease(report.toolVersions?.closure?.version),
  digest: report.toolVersions?.closure?.digest,
}));
const expectedAppClosure = appClosureIdentities[0];
for (const candidate of appClosureIdentities) {
  if (
    !candidate.release ||
    candidate.release !== expectedAppClosure.release ||
    !candidate.digest ||
    candidate.digest !== expectedAppClosure.digest
  ) {
    failures.push(
      `${candidate.label}: Closure compiler provenance differs across app lanes`,
    );
  }
}
const algorithmReport = reports.find(([label]) => label === "algorithms")?.[1];
if (
  closureRelease(algorithmReport?.toolVersions?.googleClosureCompiler) !==
  expectedAppClosure.release
) {
  failures.push(
    "algorithms: Closure release differs from the Closure-app lanes",
  );
}

if (failures.length > 0) {
  throw new Error(`comparison provenance gate failed:\n${failures.join("\n")}`);
}
console.log(
  `Comparison provenance gate passed for ${appReports.length} apps, cases, and algorithms with one compiler/scorer/config set.`,
);
