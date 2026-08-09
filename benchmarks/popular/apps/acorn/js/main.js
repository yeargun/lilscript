import * as acorn from "acorn";

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

function compact(node) {
  if (node == null) return "#";
  if (Array.isArray(node)) return "[" + node.map(compact).join(",") + "]";
  const t = node.type;
  const b = (v) => (v ? "true" : "false");
  switch (t) {
    case "Program":
      return `Program(${node.sourceType},${compact(node.body)})`;
    case "VariableDeclaration":
      return `VariableDeclaration(${node.kind},${compact(node.declarations)})`;
    case "VariableDeclarator":
      return `VariableDeclarator(${compact(node.id)},${compact(node.init)})`;
    case "Identifier":
      return `Identifier(${node.name})`;
    case "Literal":
      return `Literal(${JSON.stringify(node.value)})`;
    case "BinaryExpression":
      return `BinaryExpression(${compact(node.left)},${node.operator},${compact(node.right)})`;
    case "LogicalExpression":
      return `LogicalExpression(${compact(node.left)},${node.operator},${compact(node.right)})`;
    case "UnaryExpression":
      return `UnaryExpression(${node.operator},${node.prefix},${compact(node.argument)})`;
    case "CallExpression":
      return `CallExpression(${compact(node.callee)},${compact(node.arguments)},${b(!!node.optional)})`;
    case "MemberExpression":
      return `MemberExpression(${compact(node.object)},${compact(node.property)},${b(node.computed)},${b(!!node.optional)})`;
    case "ExpressionStatement":
      return `ExpressionStatement(${compact(node.expression)})`;
    case "BlockStatement":
      return `BlockStatement(${compact(node.body)})`;
    case "ReturnStatement":
      return `ReturnStatement(${compact(node.argument)})`;
    case "IfStatement":
      return `IfStatement(${compact(node.test)},${compact(node.consequent)},${compact(node.alternate)})`;
    case "FunctionDeclaration":
      return `FunctionDeclaration(${compact(node.id)},${compact(node.params)},${compact(node.body)},${b(node.expression)},${b(node.generator)},${b(node.async)})`;
    case "ArrowFunctionExpression":
      return `ArrowFunctionExpression(${compact(node.params)},${compact(node.body)},${b(node.expression)},${b(node.generator)},${b(node.async)})`;
    case "ObjectExpression":
      return `ObjectExpression(${compact(node.properties)})`;
    case "Property":
      return `Property(${compact(node.key)},${compact(node.value)},${node.kind},${b(node.method)},${b(node.shorthand)},${b(node.computed)})`;
    case "ArrayExpression":
      return `ArrayExpression(${compact(node.elements)})`;
    case "TemplateLiteral":
      return `TemplateLiteral(${compact(node.quasis)},${compact(node.expressions)})`;
    case "TemplateElement":
      return `TemplateElement(${JSON.stringify(node.value.cooked)},${JSON.stringify(node.value.raw)},${b(node.tail)})`;
    case "SpreadElement":
      return `SpreadElement(${compact(node.argument)})`;
    case "RestElement":
      return `RestElement(${compact(node.argument)})`;
    case "ArrayPattern":
      return `ArrayPattern(${compact(node.elements)})`;
    case "ObjectPattern":
      return `ObjectPattern(${compact(node.properties)})`;
    case "ClassDeclaration":
      return `ClassDeclaration(${compact(node.id)},${compact(node.superClass)},${compact(node.body)})`;
    case "ClassBody":
      return `ClassBody(${compact(node.body)})`;
    case "MethodDefinition":
      return `MethodDefinition(${compact(node.key)},${compact(node.value)},${node.kind},${b(node.computed)},${b(node.static)})`;
    case "FunctionExpression":
      return `FunctionExpression(${compact(node.id)},${compact(node.params)},${compact(node.body)},${b(node.expression)},${b(node.generator)},${b(node.async)})`;
    case "Super":
      return `Super()`;
    case "TryStatement":
      return `TryStatement(${compact(node.block)},${compact(node.handler)},${compact(node.finalizer)})`;
    case "CatchClause":
      return `CatchClause(${compact(node.param)},${compact(node.body)})`;
    case "ThrowStatement":
      return `ThrowStatement(${compact(node.argument)})`;
    case "ForStatement":
      return `ForStatement(${compact(node.init)},${compact(node.test)},${compact(node.update)},${compact(node.body)})`;
    case "ForInStatement":
      return `ForInStatement(${compact(node.left)},${compact(node.right)},${compact(node.body)})`;
    case "WhileStatement":
      return `WhileStatement(${compact(node.test)},${compact(node.body)})`;
    case "BreakStatement":
      return `BreakStatement(${compact(node.label)})`;
    case "ContinueStatement":
      return `ContinueStatement(${compact(node.label)})`;
    case "SwitchStatement":
      return `SwitchStatement(${compact(node.discriminant)},${compact(node.cases)})`;
    case "SwitchCase":
      return `SwitchCase(${compact(node.test)},${compact(node.consequent)})`;
    case "NewExpression":
      return `NewExpression(${compact(node.callee)},${compact(node.arguments)})`;
    case "ConditionalExpression":
      return `ConditionalExpression(${compact(node.test)},${compact(node.consequent)},${compact(node.alternate)})`;
    case "AssignmentExpression":
      return `AssignmentExpression(${node.operator},${compact(node.left)},${compact(node.right)})`;
    case "UpdateExpression":
      return `UpdateExpression(${node.operator},${b(node.prefix)},${compact(node.argument)})`;
    case "SequenceExpression":
      return `SequenceExpression(${compact(node.expressions)})`;
    case "ChainExpression":
      return `ChainExpression(${compact(node.expression)})`;
    case "AwaitExpression":
      return `AwaitExpression(${compact(node.argument)})`;
    default:
      return `UNKNOWN(${t})`;
  }
}

const parts = [];
let passed = 0;
for (let i = 0; i < suite.length; i += 1) {
  const ast = acorn.parse(suite[i], { ecmaVersion: 2020 });
  const fp = compact(ast);
  parts.push(fp);
  if (fp.length > 0) passed += 1;
}

console.log(`acorn:${passed}:${parts.join("|")}`);
