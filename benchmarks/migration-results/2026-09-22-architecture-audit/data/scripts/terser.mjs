import { minify } from "/home/azureuser/katexlil/node_modules/terser/main.js";
import { readFileSync, writeFileSync } from "node:fs";
// terser.mjs <in> <out> <module|script> <compress|mangle|full>
const [input, output, kind, mode] = process.argv.slice(2);
const module = kind === "module";
const compress = mode === "mangle" ? false : { passes: 2, toplevel: true, module };
const mangle = mode === "compress" ? false : { toplevel: true, module };
const result = await minify(readFileSync(input, "utf8"), { module, compress, mangle, format: { comments: false } });
writeFileSync(output, result.code);
