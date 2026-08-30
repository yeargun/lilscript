import { compile } from "../packages/vuelil/compiler-dom.js";
import * as runtimeDom from "../packages/vuelil/runtime-dom.js";

const cache = new Map();

function compileToFunction(template, options) {
  const key = `${template}\0${JSON.stringify(options ?? {})}`;
  if (!cache.has(key)) {
    const { code } = compile(template, options);
    cache.set(key, new Function("Vue", code)(runtimeDom));
  }
  return cache.get(key);
}

runtimeDom.registerRuntimeCompiler(compileToFunction);
