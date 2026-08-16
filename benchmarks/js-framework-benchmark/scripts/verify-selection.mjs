import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { metadataPath, upstreamRoot } from "./paths.mjs";
import { run } from "./process.mjs";

const metadata = JSON.parse(readFileSync(metadataPath, "utf8"));
const commit = run("git", ["rev-parse", "HEAD"], {
  cwd: upstreamRoot,
  capture: true,
});
if (commit !== metadata.commit) {
  throw new Error(`Expected upstream ${metadata.commit}, found ${commit}`);
}

for (const expected of metadata.frameworks) {
  const directory = resolve(upstreamRoot, "frameworks", expected.path);
  const packageJson = JSON.parse(readFileSync(resolve(directory, "package.json"), "utf8"));
  const packageLock = JSON.parse(readFileSync(resolve(directory, "package-lock.json"), "utf8"));
  const benchmark = packageJson["js-framework-benchmark"];
  let version;
  if (benchmark.frameworkVersionFromPackage) {
    version = benchmark.frameworkVersionFromPackage
      .split(":")
      .map((name) => {
        const locked =
          packageLock.dependencies?.[name]?.version ??
          packageLock.packages?.[`node_modules/${name}`]?.version;
        if (!locked) throw new Error(`${expected.path}: ${name} is missing from the lockfile`);
        return locked;
      })
      .join(" + ");
  } else {
    version = benchmark.frameworkVersion;
  }
  if (version !== expected.version) {
    throw new Error(`${expected.path}: expected ${expected.version}, found ${version}`);
  }
  console.log(`${expected.path}-v${version}`);
}

console.log(`Verified ${metadata.frameworks.length} pinned keyed implementations.`);
