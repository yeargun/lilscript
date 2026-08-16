import assert from "node:assert/strict";
import { Buffer } from "node:buffer";
import { mkdtempSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import vm from "node:vm";
import { compilerPath, projectRoot } from "./compiler-path.mjs";

export const coreCases = JSON.parse(
  readFileSync(resolve(projectRoot, "compatibility/core-cases.json"), "utf8"),
);

const modes = ["maximum", "none"];
// SolidLil is a browser/runtime compatibility layer. Its exact exception and
// Promise semantics intentionally use LilScript's JavaScript target; C/native
// portability is tracked separately from Solid API parity.
const backends = ["js"];

function compile(mode, backend, directory) {
  const extension = backend === "js" ? ".js" : backend === "c" ? ".c" : "";
  const output = resolve(directory, `${mode}-${backend}${extension}`);
  const result = spawnSync(
    compilerPath(),
    [
      resolve(projectRoot, "tests/lil/core-behavior.lil"),
      "--target",
      backend,
      "--config",
      resolve(projectRoot, `compatibility/config/${mode}.toml`),
      "-o",
      output,
    ],
    { cwd: projectRoot, encoding: "utf8", env: process.env },
  );
  if (result.status !== 0) {
    throw new Error(
      result.stderr || result.stdout || `compiler exited ${result.status}`,
    );
  }
  if (backend === "c") {
    const executable = resolve(directory, `${mode}-c-native`);
    const nativeResult = spawnSync(
      process.env.CC || "clang",
      ["-std=c11", "-O3", output, "-o", executable],
      { cwd: projectRoot, encoding: "utf8", env: process.env },
    );
    if (nativeResult.status !== 0) {
      throw new Error(
        nativeResult.stderr ||
          nativeResult.stdout ||
          `C compiler exited ${nativeResult.status}`,
      );
    }
    return {
      output: executable,
      generatedBytes: Buffer.byteLength(readFileSync(output)),
    };
  }
  return {
    output,
    generatedBytes:
      backend === "js"
        ? Buffer.byteLength(readFileSync(output))
        : statSync(output).size,
  };
}

function parseReports(label, lines) {
  const reports = new Map();
  for (const line of lines) {
    const match = /^case:(\d+):(true|false)$/.exec(line.trim());
    assert.notEqual(
      match,
      null,
      `${label}: malformed report ${JSON.stringify(line)}`,
    );
    const id = Number(match[1]);
    assert.equal(
      reports.has(id),
      false,
      `${label}: duplicate report for case ${id}`,
    );
    reports.set(id, match[2] === "true");
  }
  assert.deepEqual(
    [...reports.keys()].sort((left, right) => left - right),
    coreCases.map(({ id }) => id),
    `${label}: corpus reports do not match the compatibility manifest`,
  );
  return reports;
}

function execute(mode, backend, artifact) {
  const label = `${mode}/${backend}`;
  let lines;
  if (backend === "js") {
    lines = [];
    vm.runInNewContext(
      readFileSync(artifact.output, "utf8"),
      { console: { log: (value) => lines.push(String(value)) } },
      { filename: `core-behavior-${mode}.js`, timeout: 5000 },
    );
  } else {
    const result = spawnSync(artifact.output, [], {
      cwd: projectRoot,
      encoding: "utf8",
      env: process.env,
      timeout: 5000,
    });
    if (result.status !== 0) {
      throw new Error(
        result.stderr ||
          result.stdout ||
          `${label}: executable exited ${result.status}`,
      );
    }
    lines = result.stdout.trim().split("\n");
  }
  const reports = parseReports(label, lines);
  return {
    mode,
    backend,
    generatedBytes: artifact.generatedBytes,
    cases: coreCases.map(({ id, name, group, upstreamReference }) => ({
      id,
      name,
      group,
      upstreamReference,
      passed: reports.get(id) === true,
    })),
  };
}

export function runCoreCompatibility() {
  const directory = mkdtempSync(resolve(tmpdir(), "lilscript-solid-compat-"));
  try {
    const runs = modes.flatMap((mode) =>
      backends.map((backend) =>
        execute(mode, backend, compile(mode, backend, directory)),
      ),
    );
    const cases = coreCases.map((testCase) => ({
      ...testCase,
      modes: Object.fromEntries(
        modes.map((mode) => [
          mode,
          runs
            .filter((run) => run.mode === mode)
            .every(
              (run) => run.cases.find(({ id }) => id === testCase.id).passed,
            ),
        ]),
      ),
      backends: Object.fromEntries(
        backends.map((backend) => [
          backend,
          runs
            .filter((run) => run.backend === backend)
            .every(
              (run) => run.cases.find(({ id }) => id === testCase.id).passed,
            ),
        ]),
      ),
      targets: Object.fromEntries(
        runs.map((run) => [
          `${run.mode}/${run.backend}`,
          run.cases.find(({ id }) => id === testCase.id).passed,
        ]),
      ),
      passed: runs.every(
        (run) => run.cases.find(({ id }) => id === testCase.id).passed,
      ),
    }));
    return {
      suite: "LilScript Solid runtime behavior ports",
      scope: "JavaScript runtime target",
      excludedTargets: {
        c: "LilScript native targets do not yet implement JavaScript exception and Promise semantics",
        native:
          "LilScript native targets do not yet implement JavaScript exception and Promise semantics",
      },
      upstreamBaseline: 469,
      uniqueCases: cases.length,
      executions: cases.length * runs.length,
      passed: cases.filter(({ passed }) => passed).length,
      failed: cases.filter(({ passed }) => !passed).length,
      success: cases.every(({ passed }) => passed),
      runs: runs.map(({ mode, backend, generatedBytes, cases: modeCases }) => ({
        mode,
        backend,
        generatedBytes,
        passed: modeCases.filter(({ passed }) => passed).length,
        failed: modeCases.filter(({ passed }) => !passed).length,
      })),
      cases,
    };
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}
