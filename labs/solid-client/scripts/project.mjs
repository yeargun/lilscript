import { readFileSync } from "node:fs";
import { resolve } from "node:path";

export const root = resolve(import.meta.dirname, "..");

export function entryBundle(target) {
  const manifest = JSON.parse(
    readFileSync(
      resolve(root, "dist", target, ".vite", "manifest.json"),
      "utf8",
    ),
  );
  const entry = Object.values(manifest).find((item) => item.isEntry);
  if (!entry) throw new Error(`No entry bundle in ${target} manifest`);
  return resolve(root, "dist", target, entry.file);
}
