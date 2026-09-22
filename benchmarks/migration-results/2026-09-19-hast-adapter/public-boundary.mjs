import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const directory = dirname(fileURLToPath(import.meta.url));
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const buildRecord = readFileSync(join(directory, "source-built/hast-util-to-htmllil.json"));
const built = JSON.parse(buildRecord);
const inputs = [
  ["upstream", "/home/azureuser/hast-util-to-htmllil/node_modules/hast-util-to-html/index.js"],
  ["source-built-esm", "/tmp/lilscript-source-built-hast-20260919/workspaces/hast-util-to-htmllil/dist/to-html.esm.js"],
  ["source-built-closed", "/tmp/lilscript-source-built-hast-20260919/workspaces/hast-util-to-htmllil/dist/to-html.closed.js"],
];
const records = [];
for (const [label, path] of inputs) {
  const before = readFileSync(path);
  if (label !== "upstream" && !built.artifacts.some(artifact => artifact.sha256 === hash(before))) {
    throw new Error(`artifact is not part of the preserved source build: ${path}`);
  }
  const namespace = await import(pathToFileURL(path).href);
  const exports = Object.entries(namespace).map(([name, value]) => {
    const observation = { export: name, type: typeof value };
    if (typeof value === "function") {
      let constructible = true;
      try { Reflect.construct(Object, [], value); } catch { constructible = false; }
      Object.assign(observation, {
        name: value.name, length: value.length, constructible,
        ownProperties: Object.getOwnPropertyNames(value),
        nameDescriptor: Object.getOwnPropertyDescriptor(value, "name"),
        lengthDescriptor: Object.getOwnPropertyDescriptor(value, "length"),
      });
    }
    return observation;
  });
  if (hash(readFileSync(path)) !== hash(before)) throw new Error(`changed artifact: ${path}`);
  records.push({ label, path, sha256: hash(before), bytes: before.length, exports });
}
const receipt = {
  schema: 1, status: "unverified", node: process.version,
  harnessSha256: hash(readFileSync(fileURLToPath(import.meta.url))), records,
  sourceBuildRecordSha256: hash(buildRecord),
  sourceBuiltConfigurations: ["lilscript.toml", "lilscript.closed.toml"].map(name => {
    const path = join(built.workspace, name);
    const content = readFileSync(path);
    return { path, sha256: hash(content), bytes: content.length, content: content.toString("utf8") };
  }),
  limitations: [
    "Supplementary public-binding observation, not a replacement for original tests.",
    "The original suite passes despite differing function names on the legacy compiler's delivered artifacts.",
    "The ESM profile explicitly requests arrow spelling; constructibility differences therefore need boundary-contract review, not silent equivalence credit.",
    "Upstream dependency closure is not pinned by this diagnostic; no competitive eligibility or size win follows.",
    "CJS, UMD, installed-package, declaration and browser consumers remain required work."
  ]
};
writeFileSync(join(directory, "public-boundary.json"), JSON.stringify(receipt, null, 2) + "\n");
console.log(JSON.stringify(records.map(({ label, exports }) => ({ label, exports })), null, 2));
