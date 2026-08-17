import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const defsRoot = join(here, "../node_modules/monaco-editor/esm/vs/languages/definitions");
const popular = new Set(["javascript", "typescript", "json", "html", "css", "python", "markdown"]);

function extractArray(src, name) {
  const match = src.match(new RegExp(`${name}:\\s*\\[([\\s\\S]*?)\\]`, "m"));
  if (!match) {
    return [];
  }
  return [...match[1].matchAll(/"((?:\\.|[^"\\])*)"/g)].map((row) => row[1]);
}

function extractLineComment(src) {
  const match = src.match(/lineComment:\s*"((?:\\.|[^"\\])*)"/);
  return match ? match[1] : "";
}

function lilString(value) {
  return JSON.stringify(value);
}

const langs = [];
for (const dir of readdirSync(defsRoot, { withFileTypes: true })) {
  if (!dir.isDirectory() || popular.has(dir.name)) {
    continue;
  }
  let src = "";
  try {
    src = readFileSync(join(defsRoot, dir.name, `${dir.name}.js`), "utf8");
  } catch {
    continue;
  }
  langs.push({
    id: dir.name,
    keywords: extractArray(src, "keywords").slice(0, 64),
    lineComment: extractLineComment(src),
  });
}

const lines = [];
lines.push(`import { Lexer, createLexer, addRule } from "../editor/monarch";`);
lines.push(`import { Language, registerLanguage } from "./registry";`);
lines.push(``);
lines.push(`Lexer keywordLexer(string id, string postfix, string[] keywords, string lineComment) {`);
lines.push(`  Lexer lex = createLexer(id, postfix, "", keywords);`);
lines.push(`  addRule(lex, "root", "[ \\\\t\\\\r\\\\n]+", 0, "", "", false);`);
lines.push(`  if (lineComment.length > 0) {`);
lines.push(`    string escaped = "";`);
lines.push(`    int c = 0;`);
lines.push(`    while (c < lineComment.length) {`);
lines.push(`      string ch = lineComment.charAt(c);`);
lines.push(`      if (ch == "/" || ch == "*" || ch == "-" || ch == "+" || ch == "?" || ch == "." || ch == "(" || ch == ")" || ch == "[" || ch == "]" || ch == "{" || ch == "}" || ch == "^" || ch == "$" || ch == "|" || ch == "\\\\") {`);
lines.push(`        escaped = escaped + "\\\\" + ch;`);
lines.push(`      } else {`);
lines.push(`        escaped = escaped + ch;`);
lines.push(`      }`);
lines.push(`      c++;`);
lines.push(`    }`);
lines.push(`    addRule(lex, "root", escaped + ".*", 0, "comment", "", false);`);
lines.push(`  }`);
lines.push(`  addRule(lex, "root", "/\\\\*", 1, "comment", "comment", false);`);
lines.push(`  addRule(lex, "root", "\\\"", 1, "string", "string", false);`);
lines.push(`  addRule(lex, "root", "'", 1, "string", "stringS", false);`);
lines.push(`  addRule(lex, "root", "\\\\d+", 0, "number", "", false);`);
lines.push(`  addRule(lex, "root", "[a-zA-Z_][\\\\w]*", 4, "identifier", "", false);`);
lines.push(`  addRule(lex, "root", ".", 0, "", "", false);`);
lines.push(`  addRule(lex, "comment", "\\\\*/", 2, "comment", "", false);`);
lines.push(`  addRule(lex, "comment", "[^*]+", 0, "comment", "", false);`);
lines.push(`  addRule(lex, "comment", "\\\\*", 0, "comment", "", false);`);
lines.push(`  addRule(lex, "string", "\\\"", 2, "string", "", false);`);
lines.push(`  addRule(lex, "string", "[^\\\\\"]+", 0, "string", "", false);`);
lines.push(`  addRule(lex, "stringS", "'", 2, "string", "", false);`);
lines.push(`  addRule(lex, "stringS", "[^']+", 0, "string", "", false);`);
lines.push(`  return lex;`);
lines.push(`}`);
lines.push(``);
lines.push(`export string[] remainingLanguageIds() {`);
lines.push(`  return [`);
for (const lang of langs) {
  lines.push(`    ${lilString(lang.id)},`);
}
lines.push(`  ];`);
lines.push(`}`);
lines.push(``);
lines.push(`bool remainingRegistered = false;`);
lines.push(``);
lines.push(`export void registerRemainingLanguages() {`);
lines.push(`  if (remainingRegistered) {`);
lines.push(`    return;`);
lines.push(`  }`);
lines.push(`  remainingRegistered = true;`);
for (const lang of langs) {
  const kw = lang.keywords.map((k) => lilString(k)).join(", ");
  lines.push(
    `  registerLanguage(new Language(${lilString(lang.id)}, keywordLexer(${lilString(lang.id)}, ${lilString("." + lang.id)}, [${kw}], ${lilString(lang.lineComment)}), ${lilString(lang.lineComment)}, "/*", "*/"));`,
  );
}
lines.push(`}`);
lines.push(``);

writeFileSync(join(here, "../ports/monaco/languages/remaining.lil"), lines.join("\n"));
console.log(`wrote ${langs.length} remaining languages`);
