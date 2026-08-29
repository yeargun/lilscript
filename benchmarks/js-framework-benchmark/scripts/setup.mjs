import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
} from "node:fs";
import { basename, resolve } from "node:path";
import {
  adapterRoot,
  benchmarkRoot,
  metadataPath,
  repositoryRoot,
  upstreamRoot,
} from "./paths.mjs";
import { run } from "./process.mjs";

const metadata = JSON.parse(readFileSync(metadataPath, "utf8"));

mkdirSync(benchmarkRoot, { recursive: true });
if (!existsSync(resolve(upstreamRoot, ".git"))) {
  run(
    "git",
    ["clone", "--filter=blob:none", "--no-checkout", metadata.repository, upstreamRoot],
    { cwd: benchmarkRoot },
  );
}

const currentCommit = run("git", ["rev-parse", "HEAD"], {
  cwd: upstreamRoot,
  capture: true,
});
if (currentCommit !== metadata.commit) {
  run("git", ["fetch", "--depth=1", "origin", metadata.commit], {
    cwd: upstreamRoot,
  });
  run("git", ["checkout", "--detach", metadata.commit], { cwd: upstreamRoot });
}

const destination = resolve(upstreamRoot, "frameworks", "keyed", "solidlil");
rmSync(destination, { recursive: true, force: true });
cpSync(adapterRoot, destination, {
  recursive: true,
  filter: (source) => !["dist", "node_modules"].includes(basename(source)),
});

const solidlilRoot = resolve(
  process.env.SOLIDLIL_ROOT ?? resolve(repositoryRoot, "..", "solidlil"),
);
const solidlilInstaller = resolve(
  solidlilRoot,
  "benchmarks",
  "js-framework-benchmark",
  "scripts",
  "install-into-upstream.mjs",
);
if (!existsSync(solidlilInstaller)) {
  throw new Error(
    `SolidLil v2 adapters not found at ${solidlilInstaller}; set SOLIDLIL_ROOT`,
  );
}
run(process.execPath, [solidlilInstaller], {
  cwd: solidlilRoot,
  env: { ...process.env, JSFB_UPSTREAM: upstreamRoot },
});

console.log(`Prepared ${destination}`);
console.log(`Pinned upstream commit ${metadata.commit}`);
