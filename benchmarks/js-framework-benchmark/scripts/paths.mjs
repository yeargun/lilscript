import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const benchmarkRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const repositoryRoot = resolve(benchmarkRoot, "..", "..");
export const upstreamRoot = resolve(benchmarkRoot, "upstream");
export const adapterRoot = resolve(benchmarkRoot, "adapter");
export const metadataPath = resolve(benchmarkRoot, "upstream.json");
