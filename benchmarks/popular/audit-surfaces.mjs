import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";

const manifestUrl = new URL("./compatibility/libraries.json", import.meta.url);
const targets = [
  { id: "acorn", packageName: "acorn", entrypoints: ["acorn"] },
  {
    id: "preact",
    packageName: "preact",
    entrypoints: ["preact", "preact/hooks"],
  },
  {
    id: "redux-toolkit",
    packageName: "@reduxjs/toolkit",
    entrypoints: ["@reduxjs/toolkit"],
  },
  { id: "immer", packageName: "immer", entrypoints: ["immer"] },
  { id: "zod", packageName: "zod", entrypoints: ["zod"] },
];

const packageUrl = (packageName) =>
  new URL(`./node_modules/${packageName}/package.json`, import.meta.url);
const digest = (names) =>
  createHash("sha256").update(names.join("\n")).digest("hex");
const runtimeExportKeys = (packageManifest) => {
  if (!packageManifest.exports) return ["."];
  if (typeof packageManifest.exports === "string") return ["."];
  return Object.keys(packageManifest.exports).filter(
    (path) => !path.endsWith("package.json"),
  );
};

const compatibility = JSON.parse(await readFile(manifestUrl, "utf8"));
for (const auditTarget of targets) {
  const target = compatibility.targets.find(
    (candidate) => candidate.id === auditTarget.id,
  );
  if (!target) throw new Error(`missing compatibility target ${auditTarget.id}`);
  const packageManifest = JSON.parse(
    await readFile(packageUrl(auditTarget.packageName), "utf8"),
  );
  if (!target.versions.includes(packageManifest.version)) {
    throw new Error(
      `${auditTarget.id} version mismatch: installed ${packageManifest.version}, manifest ${target.versions.join(", ")}`,
    );
  }

  const entrypoints = [];
  for (const specifier of auditTarget.entrypoints) {
    const runtime = await import(specifier);
    const runtimeExportNames = Object.keys(runtime).sort();
    entrypoints.push({
      specifier,
      runtimeExports: runtimeExportNames.length,
      exportNameSha256: digest(runtimeExportNames),
      runtimeExportNames,
    });
  }
  target.runtimeAudit = {
    auditedAt: "2026-08-09",
    installedVersion: packageManifest.version,
    packageRuntimeExportKeys: runtimeExportKeys(packageManifest),
    selectedSubsetPaths: target.publicRuntimeApi ?? [],
    auditedEntrypoints: entrypoints,
  };
}

await writeFile(manifestUrl, `${JSON.stringify(compatibility, null, 2)}\n`);
console.log(
  `Audited ${targets.length} incomplete package surfaces: ${targets
    .map((target) => target.id)
    .join(", ")}`,
);
