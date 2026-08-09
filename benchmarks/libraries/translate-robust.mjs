import { readFile } from "node:fs/promises";
import { basename } from "node:path";

const sourcePath = process.argv[2];
if (!sourcePath) throw new Error("usage: node translate-robust.mjs <upstream-esm-file>");

const floats = (...names) => names.map((name) => `float ${name}`).join(", ");
const signatures = {
  orient3d: {
    finadd: `int finadd(int finlen, int alen, Float64Array a)`,
    tailinit: `int tailinit(${floats("xtail", "ytail", "ax", "ay", "bx", "by")}, Float64Array a, Float64Array b)`,
    tailadd: `int tailadd(int finlen, ${floats("a", "b", "k", "z")})`,
    orient3dadapt: `float orient3dadapt(${floats("ax", "ay", "az", "bx", "by", "bz", "cx", "cy", "cz", "dx", "dy", "dz", "permanent")})`,
    orient3d: `float orient3d(${floats("ax", "ay", "az", "bx", "by", "bz", "cx", "cy", "cz", "dx", "dy", "dz")})`,
    orient3dfast: `float orient3dfast(${floats("ax", "ay", "az", "bx", "by", "bz", "cx", "cy", "cz", "dx", "dy", "dz")})`,
  },
  incircle: {
    finadd: `int finadd(int finlen, int a, Float64Array alen)`,
    incircleadapt: `float incircleadapt(${floats("ax", "ay", "bx", "by", "cx", "cy", "dx", "dy", "permanent")})`,
    incircle: `float incircle(${floats("ax", "ay", "bx", "by", "cx", "cy", "dx", "dy")})`,
    incirclefast: `float incirclefast(${floats("ax", "ay", "bx", "by", "cx", "cy", "dx", "dy")})`,
  },
  insphere: {
    sum_three_scale: `int sum_three_scale(Float64Array a, Float64Array b, Float64Array c, ${floats("az", "bz", "cz")}, Float64Array out)`,
    liftexact: `int liftexact(int alen, Float64Array a, int blen, Float64Array b, int clen, Float64Array c, int dlen, Float64Array d, ${floats("x", "y", "z")}, Float64Array out)`,
    insphereexact: `float insphereexact(${floats("ax", "ay", "az", "bx", "by", "bz", "cx", "cy", "cz", "dx", "dy", "dz", "ex", "ey", "ez")})`,
    liftadapt: `int liftadapt(Float64Array a, Float64Array b, Float64Array c, ${floats("az", "bz", "cz", "x", "y", "z")}, Float64Array out)`,
    insphereadapt: `float insphereadapt(${floats("ax", "ay", "az", "bx", "by", "bz", "cx", "cy", "cz", "dx", "dy", "dz", "ex", "ey", "ez", "permanent")})`,
    insphere: `float insphere(${floats("ax", "ay", "az", "bx", "by", "bz", "cx", "cy", "cz", "dx", "dy", "dz", "ex", "ey", "ez")})`,
    inspherefast: `float inspherefast(${floats("ax", "ay", "az", "bx", "by", "bz", "cx", "cy", "cz", "dx", "dy", "dz", "ex", "ey", "ez")})`,
  },
};

const id = basename(sourcePath, ".js");
if (!signatures[id]) throw new Error(`unsupported robust-predicates module ${id}`);

let source = await readFile(sourcePath, "utf8");
source = source.replace(/^import .*?;\n\n/s, `import {
  epsilon,
  expansionEstimate as estimate,
  expansionNegate as negate,
  expansionScale as scale,
  expansionSum as sum,
  expansionSumThree as sum_three,
  expansionVector as vec,
  resulterrbound,
  splitter
} from "./util";

`);
source = source.replace(/^const ([A-Za-z0-9_]+) = vec\((\d+)\);$/gm, "Float64Array $1 = vec($2);");
source = source.replace(/^let ([A-Za-z0-9_]+) = vec\((\d+)\);$/gm, "Float64Array $1 = vec($2);");
source = source.replace(/^const ([A-Za-z0-9_]+) = /gm, "float $1 = ");

for (const [name, signature] of Object.entries(signatures[id])) {
  const pattern = new RegExp(`^(export )?function ${name}\\([^)]*\\) \\{`, "m");
  source = source.replace(pattern, (match, exported) => `${exported || ""}${signature} {`);
}

source = source.replace(/^(\s+)const ([A-Za-z0-9_]+) =/gm, "$1auto $2 =");
source = source.replace(/^(\s+)let ([A-Za-z0-9_]+) =/gm, "$1auto $2 =");
source = source.replace(/^(\s+)let ([^;=]+);$/gm, (_match, indent, names) =>
  names
    .split(",")
    .map((rawName) => rawName.trim())
    .map((name) => `${indent}${/len$/i.test(name) ? "int" : "float"} ${name} = ${/len$/i.test(name) ? "0" : "0.0"};`)
    .join("\n")
);
source = source.replace(/for \(let ([A-Za-z0-9_]+) = /g, "for (int $1 = ");
source = source.replaceAll("===", "==").replaceAll("!==", "!=");
source = source.replace(/Math\.abs\((-?[A-Za-z0-9_]+)\)/g, "$1.abs()");
source = source.replaceAll("//# sourceMappingURL=", "// sourceMappingURL=");

const target = `benchmarks/libraries/ports/robust-predicates/${id}.lil`;
const patchLines = ["*** Begin Patch", `*** Add File: ${target}`];
for (const line of source.split("\n")) patchLines.push(`+${line}`);
patchLines.push("*** End Patch");
process.stdout.write(patchLines.join("\n"));
