import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { metadataPath, upstreamRoot } from "./paths.mjs";
import { run } from "./process.mjs";

const metadata = JSON.parse(readFileSync(metadataPath, "utf8"));
for (const framework of metadata.frameworks) {
  const directory = resolve(upstreamRoot, "frameworks", framework.path);
  console.log(`\nBuilding ${framework.path}`);
  run("npm", ["ci", "--ignore-scripts"], { cwd: directory });
  run("npm", ["run", "build-prod"], { cwd: directory });
}
