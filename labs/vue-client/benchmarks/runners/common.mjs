import { createHash } from "node:crypto";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

export function artifactPath() {
  const value = process.env.VUE_LAB_ARTIFACT;
  if (!value) throw new Error("VUE_LAB_ARTIFACT is required");
  return resolve(value);
}

export function artifactUrl() {
  return pathToFileURL(artifactPath()).href;
}

export function checksum(value) {
  return createHash("sha256").update(value).digest("hex");
}

export function emitResult(result) {
  process.stdout.write(`${JSON.stringify(result)}\n`);
}

export function nodeEnvironment() {
  return { engine: "Node.js", version: process.version, architecture: process.arch };
}
