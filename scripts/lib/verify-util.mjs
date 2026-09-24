// Shared helpers for the versioned verification runners (scripts/cases.mjs and
// scripts/ports.mjs). Nothing here knows about cases or ports.
import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { chmodSync, copyFileSync, existsSync, mkdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const repository = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");

export const sha256 = (data) => createHash("sha256").update(data).digest("hex");
export const sha256File = (path) => sha256(readFileSync(path));

// A run must never read a binary that a concurrent build can overwrite: the
// compiler is copied into the run's work directory and addressed by digest.
export function pinBinary(path, directory) {
  const source = resolve(path);
  if (!existsSync(source)) throw new Error(`${source} does not exist`);
  const digest = sha256File(source);
  mkdirSync(directory, { recursive: true });
  const pinned = join(directory, `lilscript-${digest.slice(0, 16)}`);
  if (!existsSync(pinned) || sha256File(pinned) !== digest) {
    copyFileSync(source, pinned);
    chmodSync(pinned, 0o755);
  }
  const version = spawnSync(pinned, ["--version"], { encoding: "utf8" }).stdout?.trim() ?? null;
  return { source, path: pinned, sha256: digest, version };
}

export function gitIdentity(directory) {
  const git = (args) => {
    const result = spawnSync("git", ["-C", directory, ...args], { encoding: "utf8" });
    return result.status === 0 ? result.stdout.trim() : null;
  };
  const status = git(["status", "--porcelain"]);
  return { head: git(["rev-parse", "HEAD"]), dirty: status === null ? null : status !== "" };
}

// Runs a command without a shell, bounded by a timeout, capturing both
// streams. Never rejects: failures are data.
export function run(command, args, { cwd, env, timeoutMs = 120_000, input } = {}) {
  return new Promise((done) => {
    const started = performance.now();
    let stdout = "";
    let stderr = "";
    let timedOut = false;
    let child;
    try {
      child = spawn(command, args, { cwd, env, stdio: [input === undefined ? "ignore" : "pipe", "pipe", "pipe"], detached: true });
    } catch (error) {
      done({ status: null, signal: null, error: error.message, timedOut: false, stdout: "", stderr: "", ms: 0 });
      return;
    }
    const limit = 64 << 20;
    child.stdout.on("data", (chunk) => { if (stdout.length < limit) stdout += chunk; });
    child.stderr.on("data", (chunk) => { if (stderr.length < limit) stderr += chunk; });
    if (input !== undefined) child.stdin.end(input);
    const timer = setTimeout(() => {
      timedOut = true;
      // The whole process group: npm and shells leave grandchildren behind.
      try { process.kill(-child.pid, "SIGKILL"); } catch { child.kill("SIGKILL"); }
    }, timeoutMs);
    child.on("error", (error) => {
      clearTimeout(timer);
      done({ status: null, signal: null, error: error.message, timedOut, stdout, stderr, ms: performance.now() - started });
    });
    child.on("close", (status, signal) => {
      clearTimeout(timer);
      done({ status, signal, error: null, timedOut, stdout, stderr, ms: Math.round(performance.now() - started) });
    });
  });
}

// A bounded pool: at most `limit` tasks in flight, results in input order.
export async function pool(items, limit, worker) {
  const results = new Array(items.length);
  let next = 0;
  const lanes = Array.from({ length: Math.max(1, Math.min(limit, items.length)) }, async () => {
    while (next < items.length) {
      const index = next++;
      results[index] = await worker(items[index], index);
    }
  });
  await Promise.all(lanes);
  return results;
}

// First differing line between two outputs, 1-based, for failure reports.
export function firstDifference(expected, actual) {
  const left = expected.split("\n");
  const right = actual.split("\n");
  for (let index = 0; index < Math.max(left.length, right.length); index += 1) {
    const want = index < left.length ? left[index] : "<end of output>";
    const got = index < right.length ? right[index] : "<end of output>";
    if (want !== got) return { line: index + 1, expected: want, actual: got };
  }
  return null;
}

// `*` matches any run of characters; everything else is literal.
export function globMatcher(pattern) {
  const source = pattern.split("*").map((part) => part.replace(/[.+?^${}()|[\]\\/]/g, "\\$&")).join(".*");
  const expression = new RegExp(`^${source}$`);
  return (text) => expression.test(text);
}

export function headLines(text, count) {
  return text.split("\n").map((line) => line.trimEnd()).filter(Boolean).slice(0, count).join("\n");
}
