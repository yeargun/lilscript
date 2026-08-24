#!/usr/bin/env node
/* Loads the page's engine (data + src/00–69) into a fresh object, the same
   concatenation the page ships, so tests and research harnesses run exactly
   the code the page runs. UI modules (70+) are skipped: they need a document. */
import { readFileSync, readdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));

export function loadEngine() {
  const files = [
    "data/tables.js",
    ...readdirSync(join(here, "src"))
      .filter((f) => /^[0-6]\d-.*\.js$/.test(f))
      .sort()
      .map((f) => "src/" + f),
  ];
  const source = files.map((f) => readFileSync(join(here, f), "utf8")).join("\n");
  const g = {};
  new Function("globalThis", source).call(g, g);
  return g.BM;
}
