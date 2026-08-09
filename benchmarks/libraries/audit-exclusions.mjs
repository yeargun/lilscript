import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";

const manifestUrl = new URL("./compatibility/libraries.json", import.meta.url);
const exclusions = [
  {
    packageName: "nanoid",
    entrypoints: ["nanoid", "nanoid/non-secure"],
  },
  { packageName: "yocto-queue", entrypoints: ["yocto-queue"] },
];
const digest = (names) =>
  createHash("sha256").update(names.join("\n")).digest("hex");
const compatibility = JSON.parse(await readFile(manifestUrl, "utf8"));

for (const exclusion of exclusions) {
  const record = compatibility.auditedButIneligible.find(
    (candidate) => candidate.package === exclusion.packageName,
  );
  if (!record) throw new Error(`missing exclusion ${exclusion.packageName}`);
  const packageManifest = JSON.parse(
    await readFile(
      new URL(
        `./node_modules/${exclusion.packageName}/package.json`,
        import.meta.url,
      ),
      "utf8",
    ),
  );
  if (packageManifest.version !== record.version) {
    throw new Error(
      `${exclusion.packageName} version mismatch: installed ${packageManifest.version}, manifest ${record.version}`,
    );
  }
  const entrypoints = [];
  for (const specifier of exclusion.entrypoints) {
    const runtime = await import(specifier);
    const runtimeExportNames = Object.keys(runtime).sort();
    entrypoints.push({
      specifier,
      runtimeExports: runtimeExportNames.length,
      implementedRuntimeExports: 0,
      exportNameSha256: digest(runtimeExportNames),
      runtimeExportNames,
    });
  }
  record.runtimeAudit = {
    auditedAt: "2026-08-09",
    installedVersion: packageManifest.version,
    packageRuntimeExportKeys:
      typeof packageManifest.exports === "object"
        ? Object.keys(packageManifest.exports).filter(
            (path) => !path.endsWith("package.json"),
          )
        : ["."],
    auditedEntrypoints: entrypoints,
  };
}

await writeFile(manifestUrl, `${JSON.stringify(compatibility, null, 2)}\n`);
console.log("Audited excluded nanoid and yocto-queue package surfaces");
