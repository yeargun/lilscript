// regex-lit.mjs <in> <out> <module|script>: rewrite new RegExp("lit"[, "flags"]) to /lit/flags where valid
import * as acorn from "/home/azureuser/katexlil/node_modules/acorn/dist/acorn.mjs";
import * as walk from "/home/azureuser/katexlil/node_modules/acorn-walk/dist/walk.mjs";
import { readFileSync, writeFileSync } from "node:fs";
const [inp, out, kind] = process.argv.slice(2);
const src = readFileSync(inp, "utf8");
const ast = acorn.parse(src, { ecmaVersion: "latest", sourceType: kind === "module" ? "module" : "script" });
const edits = [];
walk.simple(ast, { NewExpression(n) {
  if (n.callee.type !== "Identifier" || n.callee.name !== "RegExp") return;
  const a = n.arguments; if (a.length < 1 || a.length > 2) return;
  if (!a.every(x => x.type === "Literal" && typeof x.value === "string")) return;
  const pattern = a[0].value, flags = a[1] ? a[1].value : "";
  let body = new RegExp(pattern, flags).source; // canonical source with / escaped
  if (body.includes("\n")) return;
  const lit = "/" + body + "/" + flags;
  try { acorn.parse("(" + lit + ")", { ecmaVersion: "latest" }); } catch { return; }
  if (new RegExp(body, flags).source !== new RegExp(pattern, flags).source) return;
  edits.push([n.start, n.end, lit]);
}});
edits.sort((x, y) => y[0] - x[0]);
let s = src; for (const [a, b, t] of edits) s = s.slice(0, a) + t + s.slice(b);
writeFileSync(out, s); console.error(`${edits.length} rewrites`);
