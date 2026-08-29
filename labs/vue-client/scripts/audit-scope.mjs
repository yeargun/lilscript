import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const projectRoot = resolve(import.meta.dirname, "..");
export const scopePath = resolve(projectRoot, "compatibility/scope.json");
export const inventoryPath = resolve(projectRoot, "compatibility/inventory.json");
export const upstreamRoot = resolve(projectRoot, "upstream/vue");

const SOURCE_EXTENSION = /\.(?:[cm]?[jt]sx?|d\.ts)$/u;
const TEST_FILE = /\.(?:spec|test)\.(?:[cm]?[jt]sx?)$/u;
const DECLARATION_TEST_FILE = /\.test-d\.tsx?$/u;

function compareText(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

function runGit(args, cwd = upstreamRoot) {
  const result = spawnSync("git", args, {
    cwd,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.error) {
    throw new Error(`Unable to run git ${args.join(" ")}: ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(
      `git ${args.join(" ")} failed: ${(result.stderr || result.stdout || "no diagnostic").trim()}`,
    );
  }
  return result.stdout;
}

function readJson(path, label) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    throw new Error(`Unable to read ${label} at ${path}: ${error.message}`);
  }
}

// This tokenizer intentionally retains only syntax needed to identify exports.
// Ignoring comments and literals prevents generated-code examples from becoming
// false API entries while keeping the audit independent of installed packages.
function tokens(source) {
  const result = [];
  let index = 0;
  while (index < source.length) {
    const character = source[index];
    if (/\s/u.test(character)) {
      index += 1;
      continue;
    }
    if (character === "/" && source[index + 1] === "/") {
      index = source.indexOf("\n", index + 2);
      if (index === -1) break;
      continue;
    }
    if (character === "/" && source[index + 1] === "*") {
      const end = source.indexOf("*/", index + 2);
      if (end === -1) throw new Error("Unterminated block comment in TypeScript source");
      index = end + 2;
      continue;
    }
    if (character === "/") {
      const previous = result.at(-1)?.value;
      const regexContext =
        previous === undefined ||
        ["(", "[", "{", "=", ",", ":", ";", "!", "?", ">", "return", "case"].includes(
          previous,
        );
      if (regexContext) {
        let end = index + 1;
        let inCharacterClass = false;
        let regexEnd = null;
        while (end < source.length) {
          if (source[end] === "\\") end += 2;
          else if (source[end] === "[") {
            inCharacterClass = true;
            end += 1;
          } else if (source[end] === "]") {
            inCharacterClass = false;
            end += 1;
          } else if (source[end] === "/" && !inCharacterClass) {
            end += 1;
            while (/[A-Za-z]/u.test(source[end] ?? "")) end += 1;
            regexEnd = end;
            break;
          } else if (source[end] === "\n") break;
          else end += 1;
        }
        if (regexEnd !== null) {
          index = regexEnd;
          continue;
        }
      }
    }
    if (character === "'" || character === '"') {
      const quote = character;
      let value = "";
      index += 1;
      while (index < source.length && source[index] !== quote) {
        if (source[index] === "\\") {
          value += source[index + 1] ?? "";
          index += 2;
        } else {
          value += source[index];
          index += 1;
        }
      }
      if (source[index] !== quote) throw new Error("Unterminated string in TypeScript source");
      index += 1;
      result.push({ type: "string", value });
      continue;
    }
    if (character === "`") {
      index += 1;
      while (index < source.length) {
        if (source[index] === "\\") index += 2;
        else if (source[index] === "`") {
          index += 1;
          break;
        } else index += 1;
      }
      continue;
    }
    if (/[A-Za-z_$]/u.test(character)) {
      let end = index + 1;
      while (end < source.length && /[\w$]/u.test(source[end])) end += 1;
      result.push({ type: "identifier", value: source.slice(index, end) });
      index = end;
      continue;
    }
    result.push({ type: "punctuation", value: character });
    index += 1;
  }
  return result;
}

function exportedNamesInFile(path, resolveModule, seen = new Set()) {
  if (seen.has(path)) return new Set();
  seen.add(path);
  const sourceTokens = tokens(readFileSync(path, "utf8"));
  const names = new Set();
  const wildcardModules = [];
  let braceDepth = 0;

  for (let index = 0; index < sourceTokens.length; index += 1) {
    const token = sourceTokens[index];
    if (token.value === "{") {
      braceDepth += 1;
      continue;
    }
    if (token.value === "}") {
      braceDepth -= 1;
      continue;
    }
    if (braceDepth !== 0 || token.value !== "export") continue;

    let cursor = index + 1;
    if (sourceTokens[cursor]?.value === "default") {
      names.add("default");
      continue;
    }
    if (sourceTokens[cursor]?.value === "declare") cursor += 1;
    if (sourceTokens[cursor]?.value === "async") cursor += 1;
    if (sourceTokens[cursor]?.value === "abstract") cursor += 1;

    if (sourceTokens[cursor]?.value === "*") {
      while (
        cursor < sourceTokens.length &&
        sourceTokens[cursor]?.value !== ";" &&
        sourceTokens[cursor]?.type !== "string"
      ) {
        cursor += 1;
      }
      if (sourceTokens[cursor]?.type !== "string") {
        throw new Error(`Unable to parse wildcard export in ${path}`);
      }
      wildcardModules.push(sourceTokens[cursor].value);
      continue;
    }

    let declarationType = sourceTokens[cursor]?.value;
    if (declarationType === "type" && sourceTokens[cursor + 1]?.value === "{") {
      cursor += 1;
      declarationType = "{";
    }
    if (declarationType === "{") {
      let end = cursor + 1;
      let segment = [];
      while (end < sourceTokens.length && sourceTokens[end].value !== "}") {
        if (sourceTokens[end].value === ",") {
          addNamedExport(segment, names);
          segment = [];
        } else {
          segment.push(sourceTokens[end]);
        }
        end += 1;
      }
      if (end === sourceTokens.length) {
        throw new Error(`Unable to parse named export in ${path}`);
      }
      addNamedExport(segment, names);
      index = end;
      continue;
    }

    if (
      [
        "class",
        "const",
        "enum",
        "function",
        "interface",
        "let",
        "module",
        "namespace",
        "type",
        "var",
      ].includes(declarationType)
    ) {
      const name = sourceTokens[cursor + 1];
      if (name?.type !== "identifier") {
        throw new Error(`Unable to identify exported declaration in ${path}`);
      }
      names.add(name.value);
    }
  }

  for (const specifier of wildcardModules) {
    const target = resolveModule(path, specifier);
    for (const name of exportedNamesInFile(target, resolveModule, seen)) {
      if (name !== "default") names.add(name);
    }
  }
  return names;
}

function addNamedExport(segment, names) {
  const values = segment
    .filter((token) => token.type === "identifier")
    .map((token) => token.value)
    .filter((value) => value !== "type");
  if (values.length === 0) return;
  const asIndex = values.lastIndexOf("as");
  names.add(asIndex >= 0 ? values[asIndex + 1] : values[0]);
}

function sourceModuleResolver(packageDirectories) {
  return (fromPath, specifier) => {
    let base;
    if (specifier.startsWith(".")) {
      base = resolve(dirname(fromPath), specifier);
    } else if (packageDirectories.has(specifier)) {
      base = resolve(upstreamRoot, packageDirectories.get(specifier), "src/index");
    } else {
      throw new Error(`Cannot follow wildcard export ${JSON.stringify(specifier)} from ${fromPath}`);
    }
    const candidates = [
      base,
      `${base}.ts`,
      `${base}.tsx`,
      `${base}.d.ts`,
      resolve(base, "index.ts"),
      resolve(base, "index.tsx"),
      resolve(base, "index.d.ts"),
    ];
    for (const candidate of candidates) {
      try {
        readFileSync(candidate);
        return candidate;
      } catch (error) {
        if (error.code !== "ENOENT" && error.code !== "EISDIR") throw error;
      }
    }
    throw new Error(`Cannot resolve wildcard export ${JSON.stringify(specifier)} from ${fromPath}`);
  };
}

function publicEntrypoints(manifest) {
  if (manifest.exports && typeof manifest.exports === "object") {
    return Object.keys(manifest.exports).sort(compareText);
  }
  return manifest.main || manifest.module || manifest.types ? ["."] : [];
}

export function serializeInventory(inventory) {
  return `${JSON.stringify(inventory, null, 2)}\n`;
}

export function buildInventory({ verifyClean = true } = {}) {
  const scope = readJson(scopePath, "scope manifest");
  const revision = runGit(["rev-parse", "HEAD"]).trim();
  if (revision !== scope.upstream?.revision) {
    throw new Error(
      `Vue checkout revision mismatch: expected ${scope.upstream?.revision}, found ${revision}`,
    );
  }
  const tags = runGit(["tag", "--points-at", "HEAD"])
    .trim()
    .split("\n")
    .filter(Boolean);
  if (!tags.includes(scope.upstream?.tag)) {
    throw new Error(`Vue checkout HEAD is not tagged ${scope.upstream?.tag}`);
  }
  const changes = runGit(["status", "--porcelain"]).trim();
  if (verifyClean && changes !== "") {
    throw new Error(`Vue checkout is not clean:\n${changes}`);
  }

  const rootManifest = readJson(resolve(upstreamRoot, "package.json"), "Vue manifest");
  if (rootManifest.version !== scope.upstream?.version) {
    throw new Error(
      `Vue checkout version mismatch: expected ${scope.upstream?.version}, found ${rootManifest.version}`,
    );
  }
  const trackedFiles = runGit(["ls-files", "-z"])
    .split("\0")
    .filter(Boolean)
    .sort(compareText);
  const packageManifestPaths = trackedFiles.filter(
    (path) => /^packages\/[^/]+\/package\.json$/u.test(path),
  );
  const packageDirectories = new Map();
  const manifests = new Map();
  for (const path of packageManifestPaths) {
    const manifest = readJson(resolve(upstreamRoot, path), `package manifest ${path}`);
    const directory = dirname(path);
    packageDirectories.set(manifest.name, directory);
    manifests.set(manifest.name, manifest);
  }
  const resolveModule = sourceModuleResolver(packageDirectories);

  const packages = scope.packages.map(({ name }) => {
    const directory = packageDirectories.get(name);
    const manifest = manifests.get(name);
    if (!directory || !manifest) {
      throw new Error(`Scoped package ${name} is absent from the pinned checkout`);
    }
    if (name !== "@vue/runtime-test" && manifest.version !== scope.upstream.version) {
      throw new Error(`${name} version ${manifest.version} does not match ${scope.upstream.version}`);
    }
    const sourcePrefix = `${directory}/src/`;
    const testPrefix = `${directory}/__tests__/`;
    const sourceFiles = trackedFiles.filter(
      (path) => path.startsWith(sourcePrefix) && SOURCE_EXTENSION.test(path),
    );
    const testFiles = trackedFiles.filter(
      (path) => path.startsWith(testPrefix) && TEST_FILE.test(path),
    );
    const sourceEntrypoint = `${directory}/src/index.ts`;
    if (!sourceFiles.includes(sourceEntrypoint)) {
      throw new Error(`${name} has no tracked source entrypoint at ${sourceEntrypoint}`);
    }
    const publicExports = [...exportedNamesInFile(
      resolve(upstreamRoot, sourceEntrypoint),
      resolveModule,
    )].sort(compareText);
    if (publicExports.length === 0) {
      throw new Error(`${name} has no statically enumerable public exports`);
    }
    return {
      name,
      directory,
      private: manifest.private === true,
      sourceEntrypoint,
      packageEntrypoints: publicEntrypoints(manifest),
      formats: [...(manifest.buildOptions?.formats ?? [])].sort(compareText),
      sourceFiles,
      testFiles,
      publicExports,
    };
  });

  const declarationTestFiles = trackedFiles.filter(
    (path) =>
      path.startsWith("packages-private/dts-test/") &&
      DECLARATION_TEST_FILE.test(path),
  );
  const upstreamTestFiles = packages.flatMap(({ testFiles }) => testFiles);
  if (upstreamTestFiles.length !== scope.gates?.upstreamTestFiles) {
    throw new Error(
      `Scope records ${scope.gates?.upstreamTestFiles} upstream tests, but the pinned checkout contains ${upstreamTestFiles.length}`,
    );
  }
  if (declarationTestFiles.length !== scope.gates?.declarationTestFiles) {
    throw new Error(
      `Scope records ${scope.gates?.declarationTestFiles} declaration tests, but the pinned checkout contains ${declarationTestFiles.length}`,
    );
  }

  return {
    schemaVersion: 1,
    upstream: {
      package: scope.upstream.package,
      version: rootManifest.version,
      tag: scope.upstream.tag,
      revision,
      tree: runGit(["rev-parse", "HEAD^{tree}"]).trim(),
      clean: changes === "",
    },
    totals: {
      packages: packages.length,
      sourceFiles: packages.reduce((sum, entry) => sum + entry.sourceFiles.length, 0),
      upstreamTestFiles: upstreamTestFiles.length,
      declarationTestFiles: declarationTestFiles.length,
      publicExports: packages.reduce(
        (sum, entry) => sum + entry.publicExports.length,
        0,
      ),
    },
    declarationTestFiles,
    packages,
  };
}

export function auditScope() {
  const inventory = buildInventory();
  writeFileSync(inventoryPath, serializeInventory(inventory));
  return inventory;
}

function isMain() {
  return process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}

if (isMain()) {
  const inventory = auditScope();
  console.log(
    `Audited Vue ${inventory.upstream.version} at ${inventory.upstream.revision}: ` +
      `${inventory.totals.sourceFiles} source files, ${inventory.totals.upstreamTestFiles} upstream tests, ` +
      `${inventory.totals.declarationTestFiles} declaration tests, ${inventory.totals.publicExports} public export names.`,
  );
}
