import { accessSync, constants } from "node:fs";
import { resolve } from "node:path";

export const projectRoot = resolve(import.meta.dirname, "..");
export const repositoryRoot = resolve(projectRoot, "..", "..");
export const lilscriptRoot = process.env.LILSCRIPT_ROOT ?? repositoryRoot;

function firstExecutable(candidates, label) {
  for (const candidate of candidates.filter(Boolean)) {
    try {
      accessSync(candidate, constants.X_OK);
      return candidate;
    } catch {
      // Try the next deterministic location.
    }
  }
  throw new Error(`${label} not found. Run npm run setup or set its environment override.`);
}

export function compilerPath() {
  return firstExecutable(
    [
      process.env.LILSCRIPT_COMPILER,
      resolve(lilscriptRoot, "target", "release", "lilscript"),
      resolve(lilscriptRoot, "target", "debug", "lilscript"),
    ],
    "LilScript compiler",
  );
}

export function codecPath() {
  return firstExecutable(
    [
      process.env.LILSCRIPT_CODEC,
      resolve(lilscriptRoot, "target", "release", "lilscript-codec"),
      resolve(lilscriptRoot, "target", "debug", "lilscript-codec"),
    ],
    "LilScript codec scorer",
  );
}
