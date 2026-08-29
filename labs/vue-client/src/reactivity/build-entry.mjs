import { init, initSync, parse } from "es-module-lexer";

const identifier = /^[A-Za-z_$][A-Za-z0-9_$]*$/u;

function skipTrivia(source, start) {
  let index = start;
  while (index < source.length) {
    if (/\s/u.test(source[index])) {
      index++;
      continue;
    }
    if (source.startsWith("//", index)) {
      const end = source.indexOf("\n", index + 2);
      return end < 0 ? source.length : skipTrivia(source, end + 1);
    }
    if (source.startsWith("/*", index)) {
      const end = source.indexOf("*/", index + 2);
      if (end < 0) throw new Error("unterminated comment in static import");
      index = end + 2;
      continue;
    }
    break;
  }
  return index;
}

function readIdentifier(source, start) {
  const index = skipTrivia(source, start);
  const match = /^[A-Za-z_$][A-Za-z0-9_$]*/u.exec(source.slice(index));
  if (!match) throw new Error("production integration requires identifier-named imports");
  return { name: match[0], end: index + match[0].length };
}

function namedImports(statement) {
  let index = skipTrivia(statement, "import".length);
  if (statement[index] !== "{") {
    throw new Error(
      "production integration supports static named imports, not default or namespace imports",
    );
  }
  index++;
  const names = [];
  for (;;) {
    index = skipTrivia(statement, index);
    if (statement[index] === "}") return names;
    const imported = readIdentifier(statement, index);
    index = skipTrivia(statement, imported.end);
    if (statement.slice(index, index + 2) === "as" &&
        !/[A-Za-z0-9_$]/u.test(statement[index + 2] ?? "")) {
      const local = readIdentifier(statement, index + 2);
      index = local.end;
    }
    names.push(imported.name);
    index = skipTrivia(statement, index);
    if (statement[index] === "}") return names;
    if (statement[index] !== ",") throw new Error("invalid static named import list");
    index++;
  }
}

function collectStaticImportedNames(source, specifier) {
  const [imports] = parse(source);
  const names = new Set();
  let matched = false;
  for (const entry of imports) {
    if (entry.n !== specifier) continue;
    if (entry.t !== 1) {
      throw new Error(`production integration requires static imports from ${JSON.stringify(specifier)}`);
    }
    const statement = source.slice(entry.ss, entry.se);
    if (!/^import\b/u.test(statement)) {
      throw new Error(`production integration does not support re-exports from ${JSON.stringify(specifier)}`);
    }
    matched = true;
    for (const name of namedImports(statement)) names.add(name);
  }
  if (!matched) throw new Error(`no static import from ${JSON.stringify(specifier)}`);
  if (names.size === 0) {
    throw new Error(`static import from ${JSON.stringify(specifier)} has no named bindings`);
  }
  return [...names].sort();
}

export async function staticImportedNames(source, specifier) {
  await init;
  return collectStaticImportedNames(source, specifier);
}

export function staticImportedNamesSync(source, specifier) {
  // es-module-lexer exposes a synchronous initializer for build-report validation.
  // Repeated initialization is supported and keeps this helper deterministic.
  initSync();
  return collectStaticImportedNames(source, specifier);
}

export function renderReactivityBuildEntry(exportNames) {
  const names = [...new Set(exportNames)].sort();
  if (names.length === 0 || names.some((name) => !identifier.test(name))) {
    throw new Error("reactivity build entry requires identifier-named exports");
  }
  const list = names.join(", ");
  return `import { ${list} } from "./production";\nexport { ${list} };\n`;
}

export function reactivityExportOwners(barrelSource) {
  const owners = new Map();
  const pattern = /import\s*\{([\s\S]*?)\}\s*from\s*"([^"]+)"\s*;/gu;
  for (const match of barrelSource.matchAll(pattern)) {
    for (const entry of match[1].split(",")) {
      const parts = entry.trim().split(/\s+as\s+/u);
      const exported = parts.at(-1);
      if (!identifier.test(exported)) {
        throw new Error(`invalid LilScript barrel import ${JSON.stringify(entry.trim())}`);
      }
      if (owners.has(exported)) {
        throw new Error(`duplicate LilScript barrel binding ${exported}`);
      }
      owners.set(exported, match[2]);
    }
  }
  return owners;
}

export function renderClosedReactivityBuildEntry(exportNames, owners = null) {
  const names = [...new Set(exportNames)].sort();
  if (names.length === 0 || names.some((name) => !identifier.test(name))) {
    throw new Error("reactivity build entry requires identifier-named exports");
  }
  let imports = `import { ${names.join(", ")} } from "./production";`;
  if (owners) {
    const byModule = new Map();
    for (const name of names) {
      const specifier = owners.get(name);
      if (!specifier) throw new Error(`no mirrored owner module for ${name}`);
      const bindings = byModule.get(specifier) ?? [];
      bindings.push(name);
      byModule.set(specifier, bindings);
    }
    imports = [...byModule]
      .sort(([left], [right]) => left.localeCompare(right))
      .map(
        ([specifier, bindings]) =>
          `import { ${bindings.sort().join(", ")} } from ${JSON.stringify(specifier)};`,
      )
      .join("\n");
  }
  const properties = names.map((name) => `${JSON.stringify(name)}, ${name}`).join(", ");
  return [
    imports,
    "extern void hostInstallSelected(JsValue value);",
    `hostInstallSelected(JS.object(${properties}));`,
    "",
  ].join("\n");
}
