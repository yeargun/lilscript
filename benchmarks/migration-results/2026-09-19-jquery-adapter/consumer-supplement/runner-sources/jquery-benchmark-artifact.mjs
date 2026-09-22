import { createHash } from "node:crypto";
import { readFileSync, realpathSync, statSync } from "node:fs";
import { resolve } from "node:path";

export const JQUERY_LILSCRIPT_ARTIFACT_ENV = "JQUERY_LILSCRIPT_ARTIFACT";
export const JQUERY_LILSCRIPT_ARTIFACT_SHA256_ENV =
  "JQUERY_LILSCRIPT_ARTIFACT_SHA256";

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

export function resolveJqueryLilscriptArtifact({
  environment = process.env,
  workingDirectory = process.cwd(),
  defaultArtifactPath,
} = {}) {
  const hasExplicitPath = Object.hasOwn(
    environment,
    JQUERY_LILSCRIPT_ARTIFACT_ENV,
  );
  const configuredPath = hasExplicitPath
    ? environment[JQUERY_LILSCRIPT_ARTIFACT_ENV]
    : defaultArtifactPath;

  if (typeof configuredPath !== "string" || configuredPath.trim() === "") {
    const source = hasExplicitPath
      ? JQUERY_LILSCRIPT_ARTIFACT_ENV
      : "default jQuery LilScript artifact";
    throw new Error(`${source} must name a JavaScript artifact file`);
  }

  const requestedPath = resolve(workingDirectory, configuredPath);
  let artifactPath;
  let artifactBytes;
  try {
    artifactPath = realpathSync(requestedPath);
    if (!statSync(artifactPath).isFile()) {
      throw new Error("path is not a regular file");
    }
    artifactBytes = readFileSync(artifactPath);
  } catch (error) {
    throw new Error(
      `${JQUERY_LILSCRIPT_ARTIFACT_ENV} does not resolve to a readable regular file: ${requestedPath}`,
      { cause: error },
    );
  }

  const artifactSha256 = sha256(artifactBytes);
  const expectedSha256 = environment[JQUERY_LILSCRIPT_ARTIFACT_SHA256_ENV];
  if (expectedSha256 !== undefined) {
    if (!/^[a-f0-9]{64}$/iu.test(expectedSha256)) {
      throw new Error(
        `${JQUERY_LILSCRIPT_ARTIFACT_SHA256_ENV} must be a SHA-256 hex digest`,
      );
    }
    if (artifactSha256 !== expectedSha256.toLowerCase()) {
      throw new Error(
        `${JQUERY_LILSCRIPT_ARTIFACT_ENV} changed after selection: expected ${expectedSha256.toLowerCase()}, got ${artifactSha256}`,
      );
    }
  }

  return {
    path: artifactPath,
    sha256: artifactSha256,
    selectedBy: hasExplicitPath ? JQUERY_LILSCRIPT_ARTIFACT_ENV : "default",
  };
}
