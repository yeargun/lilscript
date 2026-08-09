import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";

const manifestUrl = new URL("./compatibility/motion-v13.json", import.meta.url);
const packageUrl = new URL("./node_modules/motion/package.json", import.meta.url);
const reactUrl = new URL("./node_modules/react/package.json", import.meta.url);
const reactDomUrl = new URL("./node_modules/react-dom/package.json", import.meta.url);

const manifest = JSON.parse(await readFile(manifestUrl, "utf8"));
const packageManifest = JSON.parse(await readFile(packageUrl, "utf8"));
const react = JSON.parse(await readFile(reactUrl, "utf8"));
const reactDom = JSON.parse(await readFile(reactDomUrl, "utf8"));

if (packageManifest.version !== manifest.version) {
  throw new Error(
    `motion audit version mismatch: installed ${packageManifest.version}, manifest ${manifest.version}`,
  );
}

const publishedEntrypoints = Object.keys(packageManifest.exports)
  .filter((path) => path !== "./package.json")
  .map((path) => ({
    path,
    specifier: path === "." ? "motion" : `motion/${path.slice(2)}`,
  }));

const digest = (names) =>
  createHash("sha256").update(names.join("\n")).digest("hex");
const entrypoints = [];
const uniqueNames = new Set();
for (const entrypoint of publishedEntrypoints) {
  const runtime = await import(entrypoint.specifier);
  const runtimeExportNames = Object.keys(runtime).sort();
  for (const name of runtimeExportNames) uniqueNames.add(name);
  entrypoints.push({
    ...entrypoint,
    status: "not-implemented",
    runtimeExports: runtimeExportNames.length,
    implementedRuntimeExports: 0,
    exportNameSha256: digest(runtimeExportNames),
    runtimeExportNames,
  });
}

const root = entrypoints.find((entrypoint) => entrypoint.path === ".");
const output = {
  ...manifest,
  auditedAt: "2026-08-09",
  rootRuntimeExports: root.runtimeExports,
  implementedRootRuntimeExports: 0,
  publishedEntrypoints: entrypoints,
  publishedRuntimeBindings: entrypoints.reduce(
    (total, entrypoint) => total + entrypoint.runtimeExports,
    0,
  ),
  uniqueRuntimeExportNames: uniqueNames.size,
  peerEnvironment: {
    react: react.version,
    reactDom: reactDom.version,
  },
};

await writeFile(manifestUrl, `${JSON.stringify(output, null, 2)}\n`);
console.log(
  `Audited motion@${manifest.version}: ${entrypoints.length} entrypoints, ${output.publishedRuntimeBindings} entrypoint bindings, ${output.uniqueRuntimeExportNames} unique names`,
);
