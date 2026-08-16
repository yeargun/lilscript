import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(here, "../..");
const buildApp = join(here, "build-app.sh");
const fixtureApp = join(
  repositoryRoot,
  "comparison/apps/aggregate-model",
);

function sanitizedEnvironment(overrides = {}) {
  const environment = { ...process.env };
  delete environment.LILSCRIPT;
  delete environment.LILSCRIPT_CODEC;
  return { ...environment, ...overrides };
}

function makeFakeCargo(directory, marker) {
  const path = join(directory, "fake-cargo.cjs");
  writeFileSync(
    path,
    `#!/usr/bin/env node
const { writeFileSync } = require("node:fs");
writeFileSync(process.env.BUILD_APP_TEST_MARKER, JSON.stringify({
  argv: process.argv.slice(2),
  cargoTargetDir: process.env.CARGO_TARGET_DIR,
  hasCargoBuildTarget: Object.hasOwn(process.env, "CARGO_BUILD_TARGET"),
}));
process.exit(73);
`,
  );
  chmodSync(path, 0o755);
  return path;
}

function invoke(environment) {
  return spawnSync(buildApp, [fixtureApp], {
    cwd: repositoryRoot,
    encoding: "utf8",
    env: environment,
  });
}

test(
  "comparison app builds reject incomplete or empty compiler/scorer overrides before side effects",
  { skip: process.platform === "win32" },
  () => {
    const directory = mkdtempSync(join(tmpdir(), "lilscript-build-app-pair-"));
    const marker = join(directory, "cargo-invoked.json");
    const cargo = makeFakeCargo(directory, marker);
    try {
      const cases = [
        {
          overrides: { LILSCRIPT: process.execPath },
          diagnostic: /overrides must be supplied together/u,
        },
        {
          overrides: { LILSCRIPT_CODEC: process.execPath },
          diagnostic: /overrides must be supplied together/u,
        },
        {
          overrides: { LILSCRIPT: "", LILSCRIPT_CODEC: "" },
          diagnostic: /overrides must both be non-empty/u,
        },
        {
          overrides: {
            LILSCRIPT: process.execPath,
            LILSCRIPT_CODEC: "",
          },
          diagnostic: /overrides must both be non-empty/u,
        },
        {
          overrides: {
            LILSCRIPT: "",
            LILSCRIPT_CODEC: process.execPath,
          },
          diagnostic: /overrides must both be non-empty/u,
        },
      ];
      for (const { overrides, diagnostic } of cases) {
        const result = invoke(
          sanitizedEnvironment({
            ...overrides,
            BUILD_APP_TEST_MARKER: marker,
            CARGO: cargo,
          }),
        );
        assert.notEqual(result.status, 0);
        assert.match(result.stderr, diagnostic);
        assert.equal(
          existsSync(marker),
          false,
          "invalid overrides must fail before Cargo or Closure installation",
        );
      }
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  },
);

test(
  "comparison app default builds jointly select compiler and scorer in the repository target directory",
  { skip: process.platform === "win32" },
  () => {
    const directory = mkdtempSync(
      join(tmpdir(), "lilscript-build-app-default-"),
    );
    const marker = join(directory, "cargo-invoked.json");
    const cargo = makeFakeCargo(directory, marker);
    try {
      const result = invoke(
        sanitizedEnvironment({
          BUILD_APP_TEST_MARKER: marker,
          CARGO: cargo,
          CARGO_BUILD_TARGET: "must-not-reach-cargo",
          CARGO_TARGET_DIR: join(directory, "must-not-reach-cargo"),
        }),
      );
      assert.equal(result.status, 73);
      const invocation = JSON.parse(readFileSync(marker, "utf8"));
      assert.deepEqual(invocation, {
        argv: [
          "build",
          "--manifest-path",
          join(repositoryRoot, "Cargo.toml"),
          "--release",
          "--bin",
          "lilscript",
          "--bin",
          "lilscript-codec",
        ],
        cargoTargetDir: join(repositoryRoot, "target"),
        hasCargoBuildTarget: false,
      });
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  },
);
