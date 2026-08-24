#!/usr/bin/env node
/* A small, conservative scope analyser over acorn's AST, used by the naming
   experiments in this folder.

   It exists so a rename is a *legal* rewrite rather than a token substitution:
   every binding knows its declaring scope and all of its references, and any
   construct this file does not fully understand marks its bindings
   unrenamable instead of guessing. Renames are applied by splicing the
   original text, so the only difference between two scored artifacts is the
   spelling of names. */
import { createRequire } from "node:module";
const require = createRequire("/Users/yeargun/lilscript/benchmarks/popular/package.json");
const acorn = require("acorn");

const FUNCTION_NODES = new Set(["FunctionDeclaration", "FunctionExpression", "ArrowFunctionExpression"]);
const BLOCK_NODES = new Set([
  "BlockStatement", "ForStatement", "ForInStatement", "ForOfStatement",
  "SwitchStatement", "StaticBlock",
]);

let uid = 0;

class Scope {
  constructor(kind, parent, node) {
    this.id = uid++;
    this.kind = kind; /* module | script | function | block | class | catch */
    this.parent = parent;
    this.node = node;
    this.children = [];
    this.bindings = new Map(); /* name -> Binding */
    this.references = [];      /* Reference nodes resolved here or passed up */
    if (parent) parent.children.push(this);
  }
  get functionScope() {
    let s = this;
    while (s && s.kind === "block") s = s.parent;
    return s;
  }
  lookup(name) {
    for (let s = this; s; s = s.parent) {
      const found = s.bindings.get(name);
      if (found) return found;
    }
    return null;
  }
  /* Every name visible from this scope: shadowing one of these is illegal. */
  visibleNames(into = new Set()) {
    for (let s = this; s; s = s.parent) for (const name of s.bindings.keys()) into.add(name);
    return into;
  }
}

class Binding {
  constructor(name, scope, kind) {
    this.name = name;
    this.scope = scope;
    this.kind = kind; /* var | let | const | param | function | class | catch | import */
    this.declarations = []; /* identifier nodes */
    this.references = [];   /* identifier nodes, declarations included */
    this.renamable = true;
    this.reason = null;
    this.shorthandNodes = new Set(); /* Property / ImportSpecifier needing "a: b" */
  }
  block(reason) { this.renamable = false; this.reason = this.reason || reason; }
  get count() { return this.references.length; }
}

/* `renameModuleTopLevel` is for complete, self-contained artifacts: in a module
   only the export list is reachable from outside, so every other top-level
   binding is private and may be renamed. Leave it off for a fragment that
   something else might import by name. */
export function analyze(source, { sourceType = "module", ecmaVersion = 2022, renameModuleTopLevel = false } = {}) {
  uid = 0;
  let ast;
  try {
    ast = acorn.parse(source, { ecmaVersion, sourceType, allowHashBang: true });
  } catch (e) {
    if (sourceType === "module") return analyze(source, { sourceType: "script", ecmaVersion, renameModuleTopLevel });
    throw e;
  }
  const top = new Scope(sourceType === "module" ? "module" : "script", null, ast);
  const allScopes = [top];
  const unresolved = new Map();

  const newScope = (kind, parent, node) => {
    const s = new Scope(kind, parent, node);
    allScopes.push(s);
    return s;
  };

  /* --- declaration ---------------------------------------------------- */
  function declare(scope, node, kind, extra) {
    /* A node may be reached twice: once while hoisting the scope's lexical
       declarations, once while visiting the statement. Declare it only once. */
    if (node.__binding) return node.__binding;
    const target = kind === "var" || kind === "function-var" ? scope.functionScope : scope;
    let binding = target.bindings.get(node.name);
    if (!binding) {
      binding = new Binding(node.name, target, kind);
      target.bindings.set(node.name, binding);
    }
    binding.declarations.push(node);
    binding.references.push(node);
    node.__binding = binding;
    if (extra && extra.shorthand) binding.shorthandNodes.add(node);
    if (extra && extra.block) binding.block(extra.block);
    return binding;
  }

  function declarePattern(scope, pattern, kind, declareOnly) {
    if (!pattern) return;
    switch (pattern.type) {
      case "Identifier": declare(scope, pattern, kind); break;
      case "ObjectPattern":
        for (const prop of pattern.properties) {
          if (prop.type === "RestElement") { declarePattern(scope, prop.argument, kind, declareOnly); continue; }
          if (prop.computed && !declareOnly) visitExpression(scope, prop.key);
          const value = prop.value;
          if (prop.shorthand && value.type === "Identifier") {
            declare(scope, value, kind, { shorthand: true });
          } else if (prop.shorthand && value.type === "AssignmentPattern") {
            declare(scope, value.left, kind, { shorthand: true });
            if (!declareOnly) visitExpression(scope, value.right);
          } else {
            declarePattern(scope, value, kind, declareOnly);
          }
        }
        break;
      case "ArrayPattern":
        for (const element of pattern.elements) declarePattern(scope, element, kind, declareOnly);
        break;
      case "AssignmentPattern":
        declarePattern(scope, pattern.left, kind, declareOnly);
        if (!declareOnly) visitExpression(scope, pattern.right);
        break;
      case "RestElement": declarePattern(scope, pattern.argument, kind, declareOnly); break;
      default: /* MemberExpression target in for-in: a reference, not a binding */
        if (!declareOnly) visitExpression(scope, pattern);
    }
  }

  /* --- hoisting ------------------------------------------------------- */
  /* A function body can reference a `let` declared later in an enclosing
     scope, so every scope's lexical declarations must exist before any of its
     statements are walked. Missing this makes such references look free, and a
     renamer that believes it will move the declaration out from under them. */
  function hoistLexical(scope, body) {
    for (const stmt of body) {
      if (!stmt) continue;
      switch (stmt.type) {
        case "VariableDeclaration":
          if (stmt.kind !== "var") {
            for (const d of stmt.declarations) declarePattern(scope, d.id, stmt.kind, true);
          }
          break;
        case "FunctionDeclaration":
          if (stmt.id) declare(scope, stmt.id, scope.kind === "block" ? "function" : "function-var");
          break;
        case "ClassDeclaration":
          if (stmt.id) declare(scope, stmt.id, "class");
          break;
        case "ExportNamedDeclaration":
        case "ExportDefaultDeclaration":
          if (stmt.declaration) hoistLexical(scope, [stmt.declaration]);
          break;
        case "LabeledStatement":
          hoistLexical(scope, [stmt.body]);
          break;
        default: break;
      }
    }
  }

  function hoist(scope, body) {
    for (const stmt of body) hoistStatement(scope, stmt);
    hoistLexical(scope, body);
  }
  function hoistStatement(scope, node) {
    if (!node) return;
    switch (node.type) {
      case "VariableDeclaration":
        if (node.kind === "var") for (const d of node.declarations) declarePattern(scope, d.id, "var");
        break;
      case "FunctionDeclaration":
        if (node.id) declare(scope, node.id, scope.kind === "block" ? "function" : "function-var");
        break;
      case "IfStatement": hoistStatement(scope, node.consequent); hoistStatement(scope, node.alternate); break;
      case "ForStatement": hoistStatement(scope, node.init); hoistStatement(scope, node.body); break;
      case "ForInStatement": case "ForOfStatement": hoistStatement(scope, node.left); hoistStatement(scope, node.body); break;
      case "WhileStatement": case "DoWhileStatement": case "LabeledStatement":
        hoistStatement(scope, node.body); break;
      case "BlockStatement": for (const s of node.body) if (s.type === "VariableDeclaration" || !FUNCTION_NODES.has(s.type)) hoistVarOnly(scope, s); break;
      case "TryStatement":
        hoistStatement(scope, node.block);
        if (node.handler) hoistStatement(scope, node.handler.body);
        hoistStatement(scope, node.finalizer);
        break;
      case "SwitchStatement": for (const c of node.cases) for (const s of c.consequent) hoistVarOnly(scope, s); break;
      case "ExportNamedDeclaration": case "ExportDefaultDeclaration":
        hoistStatement(scope, node.declaration); break;
      default: break;
    }
  }
  /* Inside nested blocks only `var` climbs to the function scope. */
  function hoistVarOnly(scope, node) {
    if (!node) return;
    if (node.type === "VariableDeclaration") {
      if (node.kind === "var") for (const d of node.declarations) declarePattern(scope, d.id, "var");
      return;
    }
    if (FUNCTION_NODES.has(node.type)) return;
    switch (node.type) {
      case "IfStatement": hoistVarOnly(scope, node.consequent); hoistVarOnly(scope, node.alternate); break;
      case "ForStatement": hoistVarOnly(scope, node.init); hoistVarOnly(scope, node.body); break;
      case "ForInStatement": case "ForOfStatement": hoistVarOnly(scope, node.left); hoistVarOnly(scope, node.body); break;
      case "WhileStatement": case "DoWhileStatement": case "LabeledStatement": hoistVarOnly(scope, node.body); break;
      case "BlockStatement": for (const s of node.body) hoistVarOnly(scope, s); break;
      case "TryStatement":
        hoistVarOnly(scope, node.block);
        if (node.handler) hoistVarOnly(scope, node.handler.body);
        hoistVarOnly(scope, node.finalizer);
        break;
      case "SwitchStatement": for (const c of node.cases) for (const s of c.consequent) hoistVarOnly(scope, s); break;
      default: break;
    }
  }

  /* --- references ------------------------------------------------------ */
  function reference(scope, node) {
    node.__scope = scope;
    const binding = scope.lookup(node.name);
    if (binding) {
      binding.references.push(node);
      node.__binding = binding;
    } else {
      if (!unresolved.has(node.name)) unresolved.set(node.name, []);
      unresolved.get(node.name).push(node);
      node.__free = true;
    }
  }

  /* --- the walk -------------------------------------------------------- */
  function visitFunction(parent, node) {
    const scope = newScope("function", parent, node);
    scope.isArrow = node.type === "ArrowFunctionExpression";
    if (node.type === "FunctionExpression" && node.id) declare(scope, node.id, "function-name");
    for (const param of node.params) declarePattern(scope, param, "param");
    if (node.body.type === "BlockStatement") {
      hoist(scope, node.body.body);
      for (const stmt of node.body.body) visit(scope, stmt);
    } else {
      visitExpression(scope, node.body);
    }
    /* `arguments`, `eval` and `with` make renaming unsafe in this scope. */
    return scope;
  }

  function visit(scope, node) {
    if (!node || typeof node.type !== "string") return;
    switch (node.type) {
      case "VariableDeclaration":
        for (const d of node.declarations) {
          if (node.kind !== "var") declarePattern(scope, d.id, node.kind);
          else markVarPattern(scope, d.id);
          visitExpression(scope, d.init);
        }
        return;
      case "FunctionDeclaration": {
        if (node.id && !node.id.__binding) declare(scope, node.id, "function-var");
        visitFunction(scope, node);
        return;
      }
      case "ClassDeclaration": {
        if (node.id) declare(scope, node.id, "class");
        visitClassBody(scope, node);
        return;
      }
      case "BlockStatement": {
        const inner = newScope("block", scope, node);
        hoistBlockDeclarations(inner, node.body);
        for (const s of node.body) visit(inner, s);
        return;
      }
      case "ForStatement": {
        const inner = newScope("block", scope, node);
        if (node.init) hoistLexical(inner, [node.init]);
        if (node.init) visit(inner, node.init);
        visitExpression(inner, node.test);
        visitExpression(inner, node.update);
        visit(inner, node.body);
        return;
      }
      case "ForInStatement": case "ForOfStatement": {
        const inner = newScope("block", scope, node);
        if (node.left.type === "VariableDeclaration") hoistLexical(inner, [node.left]);
        if (node.left.type === "VariableDeclaration") visit(inner, node.left);
        else visitExpression(inner, node.left);
        visitExpression(inner, node.right);
        visit(inner, node.body);
        return;
      }
      case "SwitchStatement": {
        visitExpression(scope, node.discriminant);
        const inner = newScope("block", scope, node);
        for (const c of node.cases) hoistBlockDeclarations(inner, c.consequent);
        for (const c of node.cases) {
          visitExpression(inner, c.test);
          for (const s of c.consequent) visit(inner, s);
        }
        return;
      }
      case "TryStatement": {
        visit(scope, node.block);
        if (node.handler) {
          const inner = newScope("catch", scope, node.handler);
          if (node.handler.param) declarePattern(inner, node.handler.param, "catch");
          visit(inner, node.handler.body);
        }
        visit(scope, node.finalizer);
        return;
      }
      case "LabeledStatement": visit(scope, node.body); return;
      case "ImportDeclaration":
        for (const spec of node.specifiers) {
          const local = spec.local;
          const binding = declare(scope, local, "import");
          if (spec.type === "ImportSpecifier" && spec.imported && spec.imported.start === local.start) {
            binding.shorthandNodes.add(local);
          }
        }
        return;
      case "ExportNamedDeclaration":
        if (node.declaration) {
          visit(scope, node.declaration);
          for (const b of declaredBindings(node.declaration, scope)) b.block("exported by name");
        }
        for (const spec of node.specifiers || []) {
          if (spec.local) {
            reference(scope, spec.local);
            const b = spec.local.__binding;
            if (b) b.block("in an export list");
          }
        }
        return;
      case "ExportDefaultDeclaration": visit(scope, node.declaration); return;
      case "ExportAllDeclaration": return;
      case "ExpressionStatement": visitExpression(scope, node.expression); return;
      default:
        visitExpression(scope, node);
    }
  }

  function hoistBlockDeclarations(scope, body) {
    hoistLexical(scope, body);
  }

  function declaredBindings(decl, scope) {
    const out = [];
    if (!decl) return out;
    if (decl.type === "VariableDeclaration") {
      for (const d of decl.declarations) collectPatternBindings(d.id, out);
    } else if (decl.id) {
      if (decl.id.__binding) out.push(decl.id.__binding);
    }
    return out;
  }
  function collectPatternBindings(pattern, out) {
    if (!pattern) return;
    if (pattern.type === "Identifier") { if (pattern.__binding) out.push(pattern.__binding); return; }
    if (pattern.type === "ObjectPattern") for (const p of pattern.properties) collectPatternBindings(p.value || p.argument, out);
    if (pattern.type === "ArrayPattern") for (const e of pattern.elements) collectPatternBindings(e, out);
    if (pattern.type === "AssignmentPattern") collectPatternBindings(pattern.left, out);
    if (pattern.type === "RestElement") collectPatternBindings(pattern.argument, out);
  }

  /* A `var` pattern was declared during hoisting; here we only visit its
     computed keys and defaults. */
  function markVarPattern(scope, pattern) {
    if (!pattern) return;
    if (pattern.type === "Identifier") return;
    if (pattern.type === "ObjectPattern") {
      for (const p of pattern.properties) {
        if (p.type === "RestElement") { markVarPattern(scope, p.argument); continue; }
        if (p.computed) visitExpression(scope, p.key);
        markVarPattern(scope, p.value);
      }
      return;
    }
    if (pattern.type === "ArrayPattern") { for (const e of pattern.elements) markVarPattern(scope, e); return; }
    if (pattern.type === "AssignmentPattern") { markVarPattern(scope, pattern.left); visitExpression(scope, pattern.right); return; }
    if (pattern.type === "RestElement") { markVarPattern(scope, pattern.argument); return; }
    visitExpression(scope, pattern);
  }

  function visitClassBody(scope, node) {
    if (node.superClass) visitExpression(scope, node.superClass);
    for (const element of node.body.body) {
      if (element.computed) visitExpression(scope, element.key);
      if (element.value) visitExpression(scope, element.value);
      if (element.type === "StaticBlock") { const inner = newScope("block", scope, element); for (const s of element.body) visit(inner, s); }
    }
  }

  function visitExpression(scope, node) {
    if (!node || typeof node.type !== "string") return;
    switch (node.type) {
      case "Identifier": reference(scope, node); return;
      case "MemberExpression":
        visitExpression(scope, node.object);
        if (node.computed) visitExpression(scope, node.property);
        return;
      case "Property":
        if (node.computed) visitExpression(scope, node.key);
        visitExpression(scope, node.value);
        return;
      case "ObjectExpression": for (const p of node.properties) visitExpression(scope, p); return;
      case "SpreadElement": case "RestElement": visitExpression(scope, node.argument); return;
      case "FunctionExpression": case "ArrowFunctionExpression": visitFunction(scope, node); return;
      case "FunctionDeclaration": visit(scope, node); return;
      case "ClassExpression": {
        const inner = newScope("class", scope, node);
        if (node.id) declare(inner, node.id, "class-name");
        visitClassBody(inner, node);
        return;
      }
      case "ClassDeclaration": visit(scope, node); return;
      case "MetaProperty": case "Super": case "ThisExpression": case "Literal":
      case "TemplateElement": case "PrivateIdentifier": case "DebuggerStatement":
      case "EmptyStatement": return;
      case "LabeledStatement": visit(scope, node.body); return;
      case "BreakStatement": case "ContinueStatement": return; /* labels are not bindings */
      case "MemberChain": return;
      default: {
        /* Generic walk over child nodes; statements route back through visit. */
        for (const key of Object.keys(node)) {
          if (key === "type" || key === "start" || key === "end" || key.startsWith("__")) continue;
          const value = node[key];
          if (Array.isArray(value)) {
            for (const child of value) {
              if (child && typeof child.type === "string") routeChild(scope, child);
            }
          } else if (value && typeof value.type === "string") {
            routeChild(scope, value);
          }
        }
      }
    }
  }
  const STATEMENTS = /Statement$|Declaration$/;
  function routeChild(scope, node) {
    if (STATEMENTS.test(node.type)) visit(scope, node);
    else visitExpression(scope, node);
  }

  /* --- run -------------------------------------------------------------- */
  hoist(top, ast.body);
  for (const stmt of ast.body) visit(top, stmt);

  /* Anything that can see a dynamic scope must not be renamed. */
  const dynamic = /\beval\s*\(|\bwith\s*\(/.test(source);
  const bindings = [];
  for (const scope of allScopes) {
    for (const b of scope.bindings.values()) {
      if (dynamic) b.block("file contains eval or with");
      if (b.kind === "import") b.block("import binding");
      if (scope.kind === "module" || scope.kind === "script") {
        /* Top-level names may be reached from outside the file. A script's
           top level *is* the global object, so those always stay. */
        if (sourceType !== "module") b.block("script top level");
        else if (!renameModuleTopLevel) b.block("module top level");
      }
      bindings.push(b);
    }
  }
  return { ast, source, top, scopes: allScopes, bindings, unresolved, sourceType, renameModuleTopLevel };
}

/* Apply {binding -> newName} by splicing the original text. */
export function rename(analysis, mapping) {
  const edits = [];
  for (const [binding, newName] of mapping) {
    if (!newName || newName === binding.name) continue;
    for (const node of binding.references) {
      if (binding.shorthandNodes.has(node)) {
        edits.push({ start: node.start, end: node.end, text: `${node.name}: ${newName}` });
      } else {
        edits.push({ start: node.start, end: node.end, text: newName });
      }
    }
  }
  edits.sort((a, b) => a.start - b.start);
  let out = "", cursor = 0;
  for (const edit of edits) {
    if (edit.start < cursor) throw new Error("overlapping edits");
    out += analysis.source.slice(cursor, edit.start) + edit.text;
    cursor = edit.end;
  }
  return out + analysis.source.slice(cursor);
}

/* A rename is legal when every reference still resolves to the same binding.
   Comparing binding-graph *shapes* is too weak: two bindings in one scope with
   the same reference count can swap without changing the shape. So compare the
   resolution sequence instead — walk every identifier occurrence in source
   order and record which binding it resolves to. Renaming does not add or
   remove occurrences, so the two sequences must correspond element by element
   under the mapping. */
function resolutionSequence(analysis) {
  const index = new Map();
  const ordered = [...analysis.bindings].sort((a, b) =>
    (a.declarations[0]?.start ?? 0) - (b.declarations[0]?.start ?? 0) || a.scope.id - b.scope.id);
  ordered.forEach((binding, i) => index.set(binding, i));
  const items = [];
  for (const binding of analysis.bindings) {
    for (const ref of binding.references) items.push({ start: ref.start, key: index.get(binding) });
  }
  for (const [name, nodes] of analysis.unresolved) {
    for (const node of nodes) items.push({ start: node.start, key: `free:${name}` });
  }
  items.sort((a, b) => a.start - b.start);
  return items.map((item) => item.key);
}

export function verify(before, after, mapping) {
  const options = { sourceType: before.sourceType, renameModuleTopLevel: before.renameModuleTopLevel };
  const a = analyze(before.source, options);
  const b = analyze(after, options);
  if (a.bindings.length !== b.bindings.length) {
    return { ok: false, why: `binding count ${a.bindings.length} -> ${b.bindings.length}` };
  }
  const freeA = [...a.unresolved.keys()].sort().join(",");
  const freeB = [...b.unresolved.keys()].sort().join(",");
  if (freeA !== freeB) {
    const added = [...b.unresolved.keys()].filter((n) => !a.unresolved.has(n));
    return { ok: false, why: `free names changed; new: ${added.slice(0, 6).join(", ")}` };
  }
  const seqA = resolutionSequence(a);
  const seqB = resolutionSequence(b);
  if (seqA.length !== seqB.length) {
    return { ok: false, why: `reference count ${seqA.length} -> ${seqB.length}` };
  }
  for (let i = 0; i < seqA.length; i++) {
    if (String(seqA[i]) !== String(seqB[i])) {
      return { ok: false, why: `reference ${i} resolves differently (${seqA[i]} -> ${seqB[i]})`, at: i };
    }
  }
  return { ok: true };
}

export const RESERVED = new Set([
  "break", "case", "catch", "class", "const", "continue", "debugger", "default", "delete",
  "do", "else", "export", "extends", "finally", "for", "function", "if", "import", "in",
  "instanceof", "new", "return", "super", "switch", "this", "throw", "try", "typeof", "var",
  "void", "while", "with", "yield", "let", "static", "await", "enum", "implements", "package",
  "protected", "interface", "private", "public", "null", "true", "false", "arguments", "eval",
  "NaN", "Infinity", "undefined",
]);
