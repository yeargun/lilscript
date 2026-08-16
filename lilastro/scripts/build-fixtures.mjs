import { spawnSync } from "node:child_process";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "vite";

const labRoot = resolve(fileURLToPath(new URL(".", import.meta.url)));
const lilastroRoot = resolve(labRoot, "..");
const repoRoot = resolve(lilastroRoot, "..");
const buildRoot = join(lilastroRoot, "build/browser");
const compiler = join(repoRoot, "target/release/lilscript");

const fixtureIds = (
  process.env.FIXTURES ?? "animate-play,animate-css-vars,perf-stagger"
).split(",");

const compilerConfig =
  process.env.LILSCRIPT_CONFIG ?? join(lilastroRoot, "config/closed-world.toml");

function compileLil(fixtureId) {
  const lilDir = join(lilastroRoot, "browser", fixtureId, "lil");
  const args = [join(lilDir, "main.lil"), "--target", "js", "-o", join(lilDir, "main.js")];
  if (compilerConfig) {
    args.push("--config", compilerConfig);
  }
  const result = spawnSync(compiler, args, {
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(`lilscript ${fixtureId}\n${result.stdout}\n${result.stderr}`);
  }
  return lilDir;
}

for (const fixtureId of fixtureIds) {
  for (const lane of ["npm", "lil"]) {
    const root =
      lane === "lil"
        ? compileLil(fixtureId)
        : join(lilastroRoot, "browser", fixtureId, "npm");
    await build({
      root,
      base: "./",
      logLevel: "error",
      build: {
        outDir: join(buildRoot, `${fixtureId}-${lane}`),
        emptyOutDir: true,
        minify: true,
        rollupOptions: { input: join(root, "index.html") },
      },
    });
    console.log(`built ${fixtureId}-${lane}`);
  }
}
