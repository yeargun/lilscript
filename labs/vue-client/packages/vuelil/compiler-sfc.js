export { compileStyle, compileStyleAsync, compileTemplate, parse, parseCache, rewriteDefault, rewriteDefaultAST } from "../../artifacts/compiler-sfc.generated.js";
export { parse as babelParse } from "@babel/parser";
export { default as MagicString } from "magic-string";
export { walk } from "estree-walker";
export { generateCodeFrame, walkIdentifiers, extractIdentifiers, isInDestructureAssignment, isStaticProperty } from "./compiler-core.js";
