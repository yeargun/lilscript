import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  prepareScenarioToolchain,
  resolveScenarioToolchain,
  scenarioToolchainContext,
} from "./toolchain.mjs";

const scenariosRoot = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scenariosRoot, "../..");

test("stdout-gated scenario configs preserve terminal print effects", () => {
  for (const name of ["unmangled", "public-safe", "closed-world"]) {
    const source = readFileSync(join(scenariosRoot, `config/${name}.toml`), "utf8");
    const sectionStart = source.indexOf("[javascript]");
    assert.notEqual(
      sectionStart,
      -1,
      `${name} must declare a [javascript] section`,
    );
    const sectionEnd = source.indexOf("\n[", sectionStart + 1);
    const javascript = source.slice(
      sectionStart,
      sectionEnd === -1 ? source.length : sectionEnd,
    );
    assert.match(
      javascript,
      /^strip_console\s*=\s*false\s*$/mu,
      `${name} must retain print because stdout is a hard scenario gate`,
    );
  }
});

test("scenario toolchain rejects unpaired or empty overrides before preparation", () => {
  assert.throws(
    () =>
      resolveScenarioToolchain({
        repoRoot,
        env: { LILSCRIPT: "compiler" },
        cwd: "/work",
        platform: "linux",
      }),
    new RegExp(
      `${scenarioToolchainContext} requires LILSCRIPT and LILSCRIPT_CODEC overrides to be supplied together`,
      "u",
    ),
  );
  for (const env of [
    { LILSCRIPT: "", LILSCRIPT_CODEC: "" },
    { LILSCRIPT: "compiler", LILSCRIPT_CODEC: "" },
    { LILSCRIPT: "", LILSCRIPT_CODEC: "codec" },
  ]) {
    assert.throws(
      () =>
        resolveScenarioToolchain({
          repoRoot,
          env,
          cwd: "/work",
          platform: "linux",
        }),
      new RegExp(
        `${scenarioToolchainContext} requires LILSCRIPT and LILSCRIPT_CODEC overrides to both be non-empty`,
        "u",
      ),
    );
  }
});

test("explicit scenario toolchain pair is resolved and never rebuilt", () => {
  const commands = [];
  const required = [];
  const result = prepareScenarioToolchain({
    repoRoot: "/repo",
    cargo: "/cargo",
    env: {
      LILSCRIPT: "tools/lilscript",
      LILSCRIPT_CODEC: "tools/lilscript-codec",
    },
    cwd: "/work",
    platform: "linux",
    command: (...args) => commands.push(args),
    requireToolchain: (...args) => required.push(args),
  });

  assert.equal(result.compiler, "/work/tools/lilscript");
  assert.equal(result.codecScorer, "/work/tools/lilscript-codec");
  assert.equal(result.toolchainSource, "explicit-overrides");
  assert.deepEqual(commands, []);
  assert.deepEqual(required, [
    [
      scenarioToolchainContext,
      "/work/tools/lilscript",
      "/work/tools/lilscript-codec",
    ],
  ]);
});

test("default scenario publication fresh-builds a joint repository toolchain", () => {
  const commands = [];
  const required = [];
  const result = prepareScenarioToolchain({
    repoRoot: "/repo",
    cargo: "/cargo",
    env: {
      CARGO_BUILD_TARGET: "stale-cross-target",
      CARGO_TARGET_DIR: "/elsewhere",
      UNRELATED: "retained",
    },
    cwd: "/work",
    platform: "linux",
    command: (...args) => commands.push(args),
    requireToolchain: (...args) => required.push(args),
  });

  assert.equal(result.compiler, "/repo/target/release/lilscript");
  assert.equal(result.codecScorer, "/repo/target/release/lilscript-codec");
  assert.equal(result.toolchainSource, "current-checkout-build");
  assert.equal(commands.length, 1);
  assert.deepEqual(commands[0].slice(0, 2), [
    "/cargo",
    ["build", "--release", "--bin", "lilscript", "--bin", "lilscript-codec"],
  ]);
  assert.equal(commands[0][2].cwd, "/repo");
  assert.equal(commands[0][2].env.CARGO_TARGET_DIR, "/repo/target");
  assert.equal(commands[0][2].env.CARGO_BUILD_TARGET, undefined);
  assert.equal(commands[0][2].env.UNRELATED, "retained");
  assert.deepEqual(required, [
    [
      scenarioToolchainContext,
      "/repo/target/release/lilscript",
      "/repo/target/release/lilscript-codec",
    ],
  ]);
});
