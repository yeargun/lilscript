import { parse } from "../../../build/acorn-lilscript.js";

const suite = [
  "const x = 1 + 2;",
  "let a = -3; var b = !true;",
  "function f(a, b) { return a * b; }",
  "const g = (x) => x + 1;",
  "function h(x) { if (x) { return y; } }",
  "obj.prop; arr[0]; fn(1, 2);",
  'const o = { a: 1, b: "hi" }; const arr = [1, 2, 3];',
  "const z = (a, b) => { return a && b || c; };",
  "foo.bar(baz).qux;",
  "const add = (a, b) => a + b; add(1, 2);",
  "if (a) b; else c;",
  "function id(x) { return x; }",
  "const t = `a${x}b`;",
  "const a = [...xs]; f(...ys);",
  "const [a, b] = xs; const {c, d} = obj;",
  "function f([a, ...rest], {b}) { return a; }",
  "class C extends B { constructor() { super(); } m() { return 1; } }",
  "try { throw e; } catch (err) { f(err); } finally { g(); }",
  "for (let i = 0; i < n; i++) { break; } while (x) { continue; }",
  "for (const k in obj) { f(k); }",
  "switch (x) { case 1: break; default: y; }",
  "const y = new Foo(1); const z = a ? b : c; x = 1; i++;",
  "const s = (a, b, c);",
  "const v = a?.b?.(); const w = x ?? y;",
  "async function af() { return await p; }",
];

function compactLil(node) {
  if (node == null) return "#";
  const t = node.type;
  const items = node.items || [];
  const more = node.more || [];
  const list = "[" + items.map(compactLil).join(",") + "]";
  const moreList = "[" + more.map(compactLil).join(",") + "]";
  const b = (value) => (value ? "true" : "false");
  switch (t) {
    case "Program":
      return `Program(${node.name},${list})`;
    case "VariableDeclaration":
      return `VariableDeclaration(${node.name},${list})`;
    case "VariableDeclarator":
      return `VariableDeclarator(${compactLil(node.a)},${compactLil(node.b)})`;
    case "Identifier":
      return `Identifier(${node.name})`;
    case "Literal":
      return `Literal(${node.litText})`;
    case "BinaryExpression":
      return `BinaryExpression(${compactLil(node.a)},${node.name},${compactLil(node.b)})`;
    case "LogicalExpression":
      return `LogicalExpression(${compactLil(node.a)},${node.name},${compactLil(node.b)})`;
    case "UnaryExpression":
      return `UnaryExpression(${node.name},${b(node.prefix)},${compactLil(node.a)})`;
    case "CallExpression":
      return `CallExpression(${compactLil(node.a)},${list},${b(node.optional)})`;
    case "MemberExpression":
      return `MemberExpression(${compactLil(node.a)},${compactLil(node.b)},${b(node.computed)},${b(node.optional)})`;
    case "ExpressionStatement":
      return `ExpressionStatement(${compactLil(node.a)})`;
    case "BlockStatement":
      return `BlockStatement(${list})`;
    case "ReturnStatement":
      return `ReturnStatement(${compactLil(node.a)})`;
    case "IfStatement":
      return `IfStatement(${compactLil(node.a)},${compactLil(node.b)},${compactLil(node.c)})`;
    case "FunctionDeclaration":
      return `FunctionDeclaration(${compactLil(node.a)},${list},${compactLil(node.b)},${b(node.expression)},${b(node.generator)},${b(node.async)})`;
    case "ArrowFunctionExpression":
      return `ArrowFunctionExpression(${list},${compactLil(node.a)},${b(node.expression)},${b(node.generator)},${b(node.async)})`;
    case "ObjectExpression":
      return `ObjectExpression(${list})`;
    case "Property":
      return `Property(${compactLil(node.a)},${compactLil(node.b)},${node.name},${b(node.method)},${b(node.shorthand)},${b(node.computed)})`;
    case "ArrayExpression":
      return `ArrayExpression(${list})`;
    case "TemplateLiteral":
      return `TemplateLiteral(${list},${moreList})`;
    case "TemplateElement":
      return `TemplateElement(${node.litText},${node.name},${b(node.tail)})`;
    case "SpreadElement":
      return `SpreadElement(${compactLil(node.a)})`;
    case "RestElement":
      return `RestElement(${compactLil(node.a)})`;
    case "ArrayPattern":
      return `ArrayPattern(${list})`;
    case "ObjectPattern":
      return `ObjectPattern(${list})`;
    case "ClassDeclaration":
      return `ClassDeclaration(${compactLil(node.a)},${compactLil(node.b)},${compactLil(node.c)})`;
    case "ClassBody":
      return `ClassBody(${list})`;
    case "MethodDefinition":
      return `MethodDefinition(${compactLil(node.a)},${compactLil(node.b)},${node.name},${b(node.computed)},${b(node.isStatic)})`;
    case "FunctionExpression":
      return `FunctionExpression(${compactLil(node.a)},${list},${compactLil(node.b)},${b(node.expression)},${b(node.generator)},${b(node.async)})`;
    case "Super":
      return `Super()`;
    case "TryStatement":
      return `TryStatement(${compactLil(node.a)},${compactLil(node.b)},${compactLil(node.c)})`;
    case "CatchClause":
      return `CatchClause(${compactLil(node.a)},${compactLil(node.b)})`;
    case "ThrowStatement":
      return `ThrowStatement(${compactLil(node.a)})`;
    case "ForStatement":
      return `ForStatement(${compactLil(node.a)},${compactLil(node.b)},${compactLil(node.c)},${compactLil(node.d)})`;
    case "ForInStatement":
      return `ForInStatement(${compactLil(node.a)},${compactLil(node.b)},${compactLil(node.c)})`;
    case "WhileStatement":
      return `WhileStatement(${compactLil(node.a)},${compactLil(node.b)})`;
    case "BreakStatement":
      return `BreakStatement(${compactLil(node.a)})`;
    case "ContinueStatement":
      return `ContinueStatement(${compactLil(node.a)})`;
    case "SwitchStatement":
      return `SwitchStatement(${compactLil(node.a)},${list})`;
    case "SwitchCase":
      return `SwitchCase(${compactLil(node.a)},${list})`;
    case "NewExpression":
      return `NewExpression(${compactLil(node.a)},${list})`;
    case "ConditionalExpression":
      return `ConditionalExpression(${compactLil(node.a)},${compactLil(node.b)},${compactLil(node.c)})`;
    case "AssignmentExpression":
      return `AssignmentExpression(${node.name},${compactLil(node.a)},${compactLil(node.b)})`;
    case "UpdateExpression":
      return `UpdateExpression(${node.name},${b(node.prefix)},${compactLil(node.a)})`;
    case "SequenceExpression":
      return `SequenceExpression(${list})`;
    case "ChainExpression":
      return `ChainExpression(${compactLil(node.a)})`;
    case "AwaitExpression":
      return `AwaitExpression(${compactLil(node.a)})`;
    default:
      return `UNKNOWN(${t})`;
  }
}

const parts = [];
let passed = 0;
for (let i = 0; i < suite.length; i += 1) {
  const fp = compactLil(parse(suite[i]));
  parts.push(fp);
  if (fp.length > 0) passed += 1;
}

console.log(`acorn:${passed}:${parts.join("|")}`);
