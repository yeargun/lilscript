import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { root } from "./project.mjs";

const strict = process.argv.includes("--strict");
const features = JSON.parse(
  readFileSync(resolve(root, "compatibility/lsx-features.json"), "utf8"),
);
const counts = {
  inventory: features.length,
  expected: features.filter((item) => item.lowering !== "excluded").length,
  excluded: features.filter(
    (item) => item.lowering === "excluded" || item.runtime === "excluded",
  ).length,
  loweringVerified: features.filter(
    (item) => item.lowering === "verified" && item.runtime !== "excluded",
  ).length,
  loweringImplemented: features.filter((item) =>
    ["verified", "implemented", "partial"].includes(item.lowering),
  ).length,
  runtimeVerified: features.filter(
    (item) => item.runtime === "verified" && item.lowering !== "excluded",
  ).length,
  missing: features.filter(
    (item) => item.lowering === "missing" || item.runtime === "missing",
  ).length,
};
const report = {
  generatedAt: new Date().toISOString(),
  definition:
    "Complete client parity requires lowering and integrated differential runtime evidence for every in-scope client-rendering feature family. Hydration and SSR are separately inventoried and explicitly excluded because they require a coordinated server runtime.",
  runtimeEvidence: {
    command: "npm run test:lilx",
    baseline: "tests/solid/lsx-runtime.jsx",
    candidate: "tests/lil/lsx-runtime.lilx",
    harness: "tests/lsx-runtime.test.mjs",
    comparison:
      "Both fixtures mount into independent Playwright Chromium contexts, execute the same mutations and real browser events, compare normalized DOM/state snapshots, verify keyed identity, then assert unmount cleanup.",
  },
  complete: features
    .filter((item) => item.lowering !== "excluded")
    .every(
      (item) => item.lowering === "verified" && item.runtime === "verified",
    ),
  counts,
  features,
};
const rows = features
  .map(
    (item) =>
      `| ${item.label} | ${item.lowering} | ${item.runtime} | ${item.notes} |`,
  )
  .join("\n");
const markdown = `# SolidLil LSX parity\n\n${report.definition}\n\nDifferential gate: \`${report.runtimeEvidence.command}\` compares \`${report.runtimeEvidence.candidate}\` with \`${report.runtimeEvidence.baseline}\` through \`${report.runtimeEvidence.harness}\`.\n\n| Feature family | Lowering | Runtime | Boundary |\n| --- | --- | --- | --- |\n${rows}\n\nStrict client gate: **${report.complete ? "pass" : "incomplete"}**. Excluded server-coupled families: **${counts.excluded}**.\n`;

for (const path of [
  resolve(root, "artifacts/lsx-parity.json"),
  resolve(root, "../../web/src/solid-lsx-parity.json"),
]) {
  writeFileSync(path, `${JSON.stringify(report, null, 2)}\n`);
}
writeFileSync(resolve(root, "artifacts/lsx-parity.md"), markdown);
console.log(
  `SolidLil LSX client parity: ${counts.loweringVerified}/${counts.expected} lowering families verified; ${counts.runtimeVerified}/${counts.expected} have integrated runtime evidence (${report.complete ? "complete" : "incomplete"}); ${counts.excluded} server-coupled families excluded.`,
);
if (strict && !report.complete) process.exitCode = 1;
