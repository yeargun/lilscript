import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";

const RESERVED = new Set([
  "as", "of", "match", "type", "init", "class", "struct", "object", "export",
  "import", "from", "return", "if", "else", "while", "for", "break", "continue",
  "new", "this", "true", "false", "null", "void", "bool", "int", "float", "string",
  "func", "auto",
]);

function stripComments(src) {
  let out = "";
  let i = 0;
  while (i < src.length) {
    if (src[i] === "/" && src[i + 1] === "/") {
      while (i < src.length && src[i] !== "\n") i += 1;
      continue;
    }
    if (src[i] === "/" && src[i + 1] === "*") {
      i += 2;
      while (i < src.length && !(src[i] === "*" && src[i + 1] === "/")) i += 1;
      i += 2;
      continue;
    }
    if (src[i] === "\"" || src[i] === "'" || src[i] === "`") {
      const q = src[i];
      out += src[i];
      i += 1;
      while (i < src.length && src[i] !== q) {
        if (src[i] === "\\") {
          out += src[i] + (src[i + 1] ?? "");
          i += 2;
          continue;
        }
        out += src[i];
        i += 1;
      }
      if (i < src.length) {
        out += src[i];
        i += 1;
      }
      continue;
    }
    out += src[i];
    i += 1;
  }
  return out;
}

function skipWs(src, i) {
  while (i < src.length && /\s/.test(src[i])) i += 1;
  return i;
}

function matchAt(src, i, re) {
  const m = src.slice(i).match(re);
  return m && m.index === 0 ? m : null;
}

function skipBalanced(src, i, open, close) {
  if (src[i] !== open) return i;
  let depth = 0;
  while (i < src.length) {
    const c = src[i];
    if (c === "\"" || c === "'" || c === "`") {
      const q = c;
      i += 1;
      while (i < src.length && src[i] !== q) {
        if (src[i] === "\\") i += 2;
        else i += 1;
      }
      i += 1;
      continue;
    }
    if (c === open) depth += 1;
    else if (c === close) {
      depth -= 1;
      i += 1;
      if (depth === 0) return i;
      continue;
    }
    i += 1;
  }
  return i;
}

function parseParamList(raw) {
  const params = [];
  let i = 0;
  let depthAngle = 0;
  let depthParen = 0;
  let start = 0;
  const push = (end) => {
    const piece = raw.slice(start, end).trim();
    if (piece) params.push(piece);
  };
  while (i < raw.length) {
    const c = raw[i];
    if (c === "<") depthAngle += 1;
    else if (c === ">" && depthAngle) depthAngle -= 1;
    else if (c === "(") depthParen += 1;
    else if (c === ")" && depthParen) depthParen -= 1;
    else if (c === "," && depthAngle === 0 && depthParen === 0) {
      push(i);
      i += 1;
      start = i;
      continue;
    }
    i += 1;
  }
  push(raw.length);
  return params.map((piece) => {
    const noDefault = piece.replace(/\s*=\s*[\s\S]*$/, "").trim();
    const nameMatch = noDefault.match(/([A-Za-z_][A-Za-z0-9_]*)\s*$/);
    if (!nameMatch) return null;
    const name = nameMatch[1];
    const type = noDefault.slice(0, nameMatch.index).trim();
    if (!type) return null;
    return { type, name };
  }).filter(Boolean);
}

function parseTypeParams(raw) {
  if (!raw) return [];
  return raw.split(",").map((p) => p.trim().replace(/^([A-Za-z_][A-Za-z0-9_]*).*$/, "$1")).filter(Boolean);
}

function substituteType(type, typeParams) {
  let next = type;
  for (const param of typeParams) {
    const fill = param === "K" || /Key$/.test(param) || param === "S" ? "string" : "int";
    next = next.replace(new RegExp(`\\b${param}\\b`, "g"), fill);
  }
  return next;
}

function parseClassMembers(body) {
  const members = [];
  let i = 0;
  while (i < body.length) {
    i = skipWs(body, i);
    if (i >= body.length) break;
    if (body[i] === "}") break;
    if (matchAt(body, i, /^class\b/) || matchAt(body, i, /^struct\b/) || matchAt(body, i, /^object\b/)) {
      while (i < body.length && body[i] !== "{") i += 1;
      i = skipBalanced(body, i, "{", "}");
      continue;
    }
    const init = matchAt(body, i, /^init\s*\(/);
    if (init) {
      const argsStart = i + init[0].length - 1;
      const argsEnd = skipBalanced(body, argsStart, "(", ")");
      const params = body.slice(argsStart + 1, argsEnd - 1);
      i = skipWs(body, argsEnd);
      if (body[i] === "{") i = skipBalanced(body, i, "{", "}");
      members.push({ kind: "init", params: parseParamList(params) });
      continue;
    }
    let j = i;
    let angle = 0;
    let paren = 0;
    let sigEnd = -1;
    let isMethod = false;
    while (j < body.length) {
      const c = body[j];
      if (c === "<") angle += 1;
      else if (c === ">" && angle) angle -= 1;
      else if (c === "(" && angle === 0) {
        paren += 1;
        isMethod = true;
      } else if (c === ")" && angle === 0 && paren) {
        paren -= 1;
      } else if (c === "{" && angle === 0 && paren === 0) {
        sigEnd = j;
        isMethod = true;
        break;
      } else if (c === ";" && angle === 0 && paren === 0) {
        sigEnd = j;
        break;
      }
      j += 1;
    }
    if (sigEnd < 0) break;
    const sig = body.slice(i, sigEnd).trim();
    if (isMethod && /\(/.test(sig)) {
      const argsOpen = sig.lastIndexOf("(");
      const head = sig.slice(0, argsOpen).trim();
      const args = sig.slice(argsOpen + 1, sig.lastIndexOf(")")).trim();
      const nameMatch = head.match(/([A-Za-z_][A-Za-z0-9_]*)\s*$/);
      if (nameMatch && nameMatch[1] !== "init") {
        const name = nameMatch[1];
        const returnType = head.slice(0, nameMatch.index).trim() || "void";
        members.push({
          kind: "method",
          name,
          returnType,
          params: parseParamList(args),
        });
      }
      i = skipBalanced(body, sigEnd, "{", "}");
      continue;
    }
    i = sigEnd + 1;
  }
  return members;
}

export function parseLilSurface(src) {
  const text = stripComments(src);
  const classes = [];
  const functions = [];
  const constants = [];
  let i = 0;
  while (i < text.length) {
    i = skipWs(text, i);
    if (i >= text.length) break;
    if (matchAt(text, i, /^import\b/)) {
      while (i < text.length && text[i] !== ";" && text[i] !== "\n") i += 1;
      i += 1;
      continue;
    }
    const exported = matchAt(text, i, /^export\s+/);
    if (exported) i += exported[0].length;
    i = skipWs(text, i);
    const classMatch = matchAt(text, i, /^class\s+([A-Za-z_][A-Za-z0-9_]*)\s*(<[^>]*>)?/);
    if (classMatch) {
      const name = classMatch[1];
      const typeParams = parseTypeParams((classMatch[2] || "").slice(1, -1));
      i += classMatch[0].length;
      i = skipWs(text, i);
      if (text.startsWith("extends", i)) {
        while (i < text.length && text[i] !== "{") i += 1;
      }
      i = skipWs(text, i);
      if (text[i] !== "{") {
        i += 1;
        continue;
      }
      const bodyStart = i + 1;
      const bodyEnd = skipBalanced(text, i, "{", "}") - 1;
      if (exported) {
        classes.push({
          name,
          typeParams,
          members: parseClassMembers(text.slice(bodyStart, bodyEnd)),
        });
      }
      i = bodyEnd + 1;
      continue;
    }
    if (matchAt(text, i, /^(struct|object)\b/)) {
      while (i < text.length && text[i] !== "{") i += 1;
      i = skipBalanced(text, i, "{", "}");
      continue;
    }
    const braceExport = matchAt(text, i, /^\{([^}]+)\}/);
    if (exported && braceExport) {
      for (const name of braceExport[1].split(",").map((s) => s.trim()).filter(Boolean)) {
        functions.push(name);
      }
      i += braceExport[0].length;
      continue;
    }
    if (exported) {
      let j = i;
      let angle = 0;
      let paren = 0;
      while (j < text.length) {
        const c = text[j];
        if (c === "<") angle += 1;
        else if (c === ">" && angle) angle -= 1;
        else if (c === "(" && angle === 0) paren += 1;
        else if (c === ")" && angle === 0 && paren) paren -= 1;
        else if ((c === "{" || c === ";" || c === "=") && angle === 0 && paren === 0) break;
        j += 1;
      }
      const head = text.slice(i, j).trim();
      if (head.includes("(")) {
        const matches = [...head.matchAll(/\b([A-Za-z_][A-Za-z0-9_]*)\s*\(/g)];
        const nameMatch = [...matches].reverse().find((m) => m[1] !== "func");
        if (nameMatch && !RESERVED.has(nameMatch[1])) functions.push(nameMatch[1]);
        while (j < text.length && text[j] !== "{") j += 1;
        if (text[j] === "{") j = skipBalanced(text, j, "{", "}");
        i = j;
        continue;
      }
      const constMatch = head.match(/([A-Za-z_][A-Za-z0-9_]*)\s*$/);
      if (constMatch) constants.push(constMatch[1]);
      while (j < text.length && text[j] !== ";" && text[j] !== "\n") j += 1;
      i = j + 1;
      continue;
    }
    if (matchAt(text, i, /^(func|void|int|float|bool|string|auto)\b/) || /^[A-Za-z_]/.test(text[i] || "")) {
      while (i < text.length && text[i] !== "{" && text[i] !== ";") i += 1;
      if (text[i] === "{") i = skipBalanced(text, i, "{", "}");
      else i += 1;
      continue;
    }
    i += 1;
  }
  return { classes, functions: [...new Set(functions)], constants: [...new Set(constants)] };
}

function constructedType(cls) {
  if (!cls.typeParams.length) return cls.name;
  const args = cls.typeParams.map((param) => (param === "K" || /Key$/.test(param) || param === "S" ? "string" : "int"));
  return `${cls.name}<${args.join(", ")}>`;
}

function ident(name, used) {
  let next = name;
  let n = 2;
  while (RESERVED.has(next) || used.has(next)) {
    next = `${name}${n}`;
    n += 1;
  }
  used.add(next);
  return next;
}

const KNOWN_TYPES = new Set([
  "int", "float", "bool", "string", "void", "auto", "JsValue", "Map", "Set", "Record",
]);

function typeNames(type) {
  return [...type.matchAll(/\b([A-Za-z_][A-Za-z0-9_]*)\b/g)]
    .map((m) => m[1])
    .filter((name) => name !== "func");
}

export function parseImportedNames(src) {
  const names = new Set();
  const text = stripComments(src);
  const re = /import\s+\{([^}]+)\}\s+from\s+"[^"]+"/g;
  let match;
  while ((match = re.exec(text))) {
    for (const part of match[1].split(",")) {
      const piece = part.trim();
      if (!piece) continue;
      const aliased = piece.match(/as\s+([A-Za-z_][A-Za-z0-9_]*)$/);
      names.add(aliased ? aliased[1] : piece.split(/\s+/)[0]);
    }
  }
  return names;
}

export function extractImportStatements(src) {
  const text = stripComments(src);
  const out = [];
  const re = /import\s+\{[\s\S]*?\}\s+from\s+"[^"]+"\s*;?/g;
  let match;
  while ((match = re.exec(text))) {
    out.push(`${match[0].replace(/;$/, "")};`);
  }
  return out;
}

function typeIsResolvable(type, exportedClasses, importedNames) {
  return typeNames(type).every(
    (name) => KNOWN_TYPES.has(name) || exportedClasses.has(name) || importedNames.has(name),
  );
}

function rewriteImportFrom(stmt, srcDir, keepDir) {
  return stmt.replace(/from\s+"([^"]+)"/, (_, spec) => {
    if (!spec.startsWith(".")) return `from "${spec}"`;
    let next = relative(keepDir, resolve(srcDir, spec)).replace(/\\/g, "/");
    if (!next.startsWith(".")) next = `./${next}`;
    return `from "${next}"`;
  });
}

export function keepSource(src, importSpec, dirs = null) {
  const surface = parseLilSurface(src);
  const names = [
    ...surface.classes.map((cls) => cls.name),
    ...surface.functions,
    ...surface.constants,
  ].filter((name, i, all) => all.indexOf(name) === i && !RESERVED.has(name));
  if (!surface.classes.length) {
    return null;
  }
  const exportedClasses = new Set(surface.classes.map((cls) => cls.name));
  const importedNames = parseImportedNames(src);
  const used = new Set(names);
  const lines = extractImportStatements(src).map((stmt) =>
    dirs ? rewriteImportFrom(stmt, dirs.srcDir, dirs.keepDir) : stmt,
  );
  if (names.length) {
    lines.push(`import { ${names.join(", ")} } from "${importSpec}";`);
  }
  if (surface.functions.length || surface.constants.length) {
    const reexport = [...surface.functions, ...surface.constants].filter((name) => !RESERVED.has(name));
    if (reexport.length) {
      lines.push(`export { ${reexport.join(", ")} };`);
    }
  }
  for (const cls of surface.classes) {
    const ctor = constructedType(cls);
    const init = cls.members.find((m) => m.kind === "init") ?? { kind: "init", params: [] };
    const factoryName = ident(`keepNew${cls.name}`, used);
    const local = new Set(used);
    const factoryParams = init.params.map((p) => {
      const name = ident(p.name, local);
      return { ...p, name, type: substituteType(p.type, cls.typeParams) };
    });
    if (!factoryParams.some((p) => !typeIsResolvable(p.type, exportedClasses, importedNames))) {
      const paramSig = factoryParams.map((p) => `${p.type} ${p.name}`).join(", ");
      const argList = factoryParams.map((p) => p.name).join(", ");
      lines.push(`export ${ctor} ${factoryName}(${paramSig}) {`);
      lines.push(`  return new ${ctor}(${argList});`);
      lines.push(`}`);
    }
    for (const method of cls.members.filter((m) => m.kind === "method")) {
      if (RESERVED.has(method.name) && method.name !== "type") {
        continue;
      }
      const methodParams = method.params.map((p) => ({
        ...p,
        type: substituteType(p.type, cls.typeParams),
      }));
      const ret = substituteType(method.returnType, cls.typeParams);
      if (
        !typeIsResolvable(ret, exportedClasses, importedNames)
        || methodParams.some((p) => !typeIsResolvable(p.type, exportedClasses, importedNames))
      ) {
        continue;
      }
      const keepName = ident(`keep${cls.name}${method.name[0].toUpperCase()}${method.name.slice(1)}`, used);
      const methodLocal = new Set(used);
      const selfName = ident("self", methodLocal);
      const namedParams = methodParams.map((p) => ({ ...p, name: ident(p.name, methodLocal) }));
      const sig = [`${ctor} ${selfName}`, ...namedParams.map((p) => `${p.type} ${p.name}`)].join(", ");
      const callArgs = namedParams.map((p) => p.name).join(", ");
      const call = `${selfName}.${method.name}(${callArgs})`;
      if (ret === "void") {
        lines.push(`export void ${keepName}(${sig}) {`);
        lines.push(`  ${call};`);
        lines.push(`}`);
      } else {
        lines.push(`export ${ret} ${keepName}(${sig}) {`);
        lines.push(`  return ${call};`);
        lines.push(`}`);
      }
    }
  }
  return `${lines.join("\n")}\n`;
}

export function writeKeepFile(src, lilAbs, keepAbs) {
  let spec = relative(dirname(keepAbs), lilAbs).replace(/\\/g, "/");
  if (!spec.startsWith(".")) spec = `./${spec}`;
  if (!spec.endsWith(".lil")) spec = `${spec}.lil`;
  const body = keepSource(src, spec, { srcDir: dirname(lilAbs), keepDir: dirname(keepAbs) });
  if (!body) return null;
  mkdirSync(dirname(keepAbs), { recursive: true });
  writeFileSync(keepAbs, body);
  return keepAbs;
}
