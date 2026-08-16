import { accessSync, constants } from "node:fs";
import { resolve } from "node:path";

export const projectRoot = resolve(import.meta.dirname, "..");
export const repositoryRoot = resolve(projectRoot, "..", "..");
export const lilscriptRoot = process.env.LILSCRIPT_ROOT ?? repositoryRoot;

export function compilerPath() {
  const candidates = [
    process.env.LILSCRIPT_COMPILER,
    resolve(lilscriptRoot, "target", "release", "lilscript"),
    resolve(lilscriptRoot, "target", "debug", "lilscript"),
  ].filter(Boolean);
  for (const candidate of candidates) {
    try {
      accessSync(candidate, constants.X_OK);
      return candidate;
    } catch {
      // Continue to the next deterministic location.
    }
  }
  throw new Error(
    "LilScript compiler not found. Run `npm run setup` or set LILSCRIPT_COMPILER.",
  );
}
