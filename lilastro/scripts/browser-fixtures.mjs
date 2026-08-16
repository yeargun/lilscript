import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "vite";
import {
  canonicalCodecMeasurementsForFiles,
  canonicalCodecProvenance,
} from "../../benchmarks/codec-contract.mjs";

const scriptRoot = dirname(fileURLToPath(import.meta.url));
export const lilastroRoot = resolve(scriptRoot, "..");
export const repoRoot = resolve(lilastroRoot, "..");
export const browserBuildRoot = join(lilastroRoot, "build/browser");
export const browserManifestPath = join(browserBuildRoot, "manifest.json");
export const FIXTURES = [
  "animate-play",
  "animate-css-vars",
  "animate-stagger",
  "animate-spring",
  "animate-scroll",
  "gesture-press",
  "gesture-hover",
  "in-view",
  "resize-box",
  "motion-value",
  "perf-stagger",
  "showcase-wave",
  "showcase-spring",
  "showcase-sequence",
  "showcase-gestures",
  "showcase-carousel",
];

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const sha256File = (path) => sha256(readFileSync(path));

function run(program, args, cwd = repoRoot) {
  const result = spawnSync(program, args, {
    cwd,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(
      `${program} ${args.join(" ")}\n${result.stdout ?? ""}${result.stderr ?? ""}`,
    );
  }
  return result.stdout.trim();
}

function prepareToolchain() {
  const compilerOverride = process.env.LILSCRIPT;
  const codecOverride = process.env.LILSCRIPT_CODEC;
  if (Boolean(compilerOverride) !== Boolean(codecOverride)) {
    throw new Error(
      "LILSCRIPT and LILSCRIPT_CODEC must be supplied together for browser-fixture evidence",
    );
  }
  const compiler = compilerOverride
    ? resolve(process.cwd(), compilerOverride)
    : join(repoRoot, "target/release/lilscript");
  if (!compilerOverride) {
    run(process.env.CARGO ?? "cargo", [
      "build",
      "--release",
      "--bin",
      "lilscript",
      "--bin",
      "lilscript-codec",
    ]);
  } else if (
    !existsSync(compiler) ||
    !existsSync(resolve(process.cwd(), codecOverride))
  ) {
    throw new Error("explicit LilScript compiler/scorer pair is incomplete");
  }
  return {
    path: relative(repoRoot, compiler) || compiler,
    sha256: sha256File(compiler),
    version: run(compiler, ["--version"]),
    source: compilerOverride ? "environment-pair" : "cargo-build-release",
    executable: compiler,
  };
}

function outputFiles(root) {
  const files = [];
  const visit = (directory) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) visit(path);
      else if (entry.isFile()) files.push(path);
    }
  };
  visit(root);
  return files.sort();
}

function attestLane(root) {
  const paths = outputFiles(root);
  const javascript = paths.filter((path) => path.endsWith(".js"));
  if (javascript.length === 0) {
    throw new Error(`${root} contains no JavaScript artifact`);
  }
  const measured = canonicalCodecMeasurementsForFiles(
    javascript,
    "Lilastro browser fixture build",
  );
  const jsByPath = new Map(
    javascript.map((path, index) => [path, measured[index]]),
  );
  return paths.map((path) => {
    const bytes = readFileSync(path);
    const row = {
      path: relative(browserBuildRoot, path),
      bytes: bytes.length,
      sha256: sha256(bytes),
    };
    const size = jsByPath.get(path);
    return size
      ? {
          ...row,
          javascript: {
            raw: size.raw,
            gzip: size.gzip,
            brotli: size.brotli,
          },
        }
      : row;
  });
}

export async function buildBrowserFixtures({ rebuild = true } = {}) {
  if (!rebuild) {
    if (!existsSync(browserManifestPath)) {
      throw new Error(
        `${browserManifestPath} is missing; rebuild browser fixtures first`,
      );
    }
    return JSON.parse(readFileSync(browserManifestPath, "utf8"));
  }

  const compiler = prepareToolchain();
  const config = process.env.LILSCRIPT_CONFIG
    ? resolve(process.cwd(), process.env.LILSCRIPT_CONFIG)
    : join(lilastroRoot, "config/closed-world.toml");
  const configuredCostModel = readFileSync(config, "utf8").match(
    /^cost_model\s*=\s*["'](raw|gzip|brotli)["']\s*$/m,
  )?.[1];
  if (configuredCostModel !== "brotli") {
    throw new Error(
      `${config} must explicitly declare javascript.cost_model = "brotli" for Motion publication`,
    );
  }
  mkdirSync(browserBuildRoot, { recursive: true });
  const fixtures = [];
  for (const id of FIXTURES) {
    const lanes = {};
    for (const lane of ["npm", "lil"]) {
      const laneRoot = join(lilastroRoot, "browser", id, lane);
      if (lane === "lil") {
        run(compiler.executable, [
          join(laneRoot, "main.lil"),
          "--target",
          "js",
          "--config",
          config,
          "-o",
          join(laneRoot, "main.js"),
        ]);
      }
      const outDir = join(browserBuildRoot, `${id}-${lane}`);
      await build({
        root: laneRoot,
        base: "./",
        logLevel: "error",
        build: {
          outDir,
          emptyOutDir: true,
          minify: true,
          rollupOptions: { input: join(laneRoot, "index.html") },
        },
      });
      lanes[lane] = { files: attestLane(outDir) };
      console.log(`  ${id}-${lane}`);
    }
    fixtures.push({ id, lanes });
  }

  const packageDefinition = JSON.parse(
    readFileSync(join(lilastroRoot, "package.json"), "utf8"),
  );
  const manifest = {
    schemaVersion: 1,
    generatedAt: new Date().toISOString(),
    compiler: {
      source: compiler.source,
      path: compiler.path,
      sha256: compiler.sha256,
      version: compiler.version,
    },
    config: {
      path: relative(repoRoot, config),
      sha256: sha256File(config),
      costModel: configuredCostModel,
    },
    vite: packageDefinition.devDependencies.vite,
    packageLock: {
      path: "lilastro/package-lock.json",
      sha256: sha256File(join(lilastroRoot, "package-lock.json")),
    },
    codecs: canonicalCodecProvenance("Lilastro browser fixture manifest"),
    fixtures,
  };
  writeFileSync(browserManifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  return manifest;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  await buildBrowserFixtures();
  console.log(`wrote ${browserManifestPath}`);
}
