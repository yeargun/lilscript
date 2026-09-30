// Isolated recipe execution. Versions and all installed runtime dependencies
// are locked by bars.mjs before this process is started.
import {readFileSync, writeFileSync} from "node:fs";
import {createRequire} from "node:module";
import {pathToFileURL} from "node:url";
import {join} from "node:path";
import {execFileSync} from "node:child_process";

const request = JSON.parse(readFileSync(process.argv[2], "utf8"));
const requireTool = createRequire(join(request.toolchain, "package.json"));
const load = name => import(pathToFileURL(requireTool.resolve(name)).href);
const source = () => readFileSync(request.input, "utf8");
const esm = request.format === "esm";
const started = process.cpuUsage();
const clock = process.hrtime.bigint();
let code;
switch (request.recipe) {
  case "terser": {
    const {minify} = await load("terser");
    ({code} = await minify(source(), {module:esm, compress:{passes:3, unsafe:false, pure_getters:false},
      mangle:true, format:{comments:false}, ecma:2020}));
    break;
  }
  case "swc": {
    const {minify} = await load("@swc/core");
    ({code} = await minify(source(), {module:esm, compress:{passes:3, unsafe:false, pure_getters:false},
      mangle:true, format:{comments:false}, ecma:2020}));
    break;
  }
  case "oxc": {
    const {minify} = await load("rolldown/experimental");
    const result = await minify(request.input, source(), {compress:true, mangle:true, module:esm});
    if (result.errors?.length) throw new Error(JSON.stringify(result.errors));
    code = result.code;
    break;
  }
  case "rolldown": {
    const {rolldown} = await load("rolldown");
    const bundle = await rolldown({input:request.entry, external:request.externals});
    const generated = await bundle.generate({format:esm ? "es" : "iife", minify:true, target:"es2020",
      comments:false, exports:esm ? "named" : "none"});
    // Do not concatenate chunks and silently change module initialization.
    if (generated.output.length !== 1 || generated.output[0].type !== "chunk") throw new Error("recipe requires one delivered chunk");
    code = generated.output[0].code;
    await bundle.close();
    break;
  }
  case "esbuild":
  case "bundle": {
    const {build} = await load("esbuild");
    const result = await build({entryPoints:[request.entry], bundle:true, write:false, format:esm ? "esm" : "iife",
      platform:"neutral", target:"es2020", external:request.externals,
      minify:request.recipe === "esbuild", legalComments:"none", logLevel:"silent", charset:"utf8"});
    if (result.outputFiles.length !== 1) throw new Error("recipe requires one delivered chunk");
    code = result.outputFiles[0].text;
    break;
  }
  case "closure-advanced": {
    const executable = join(request.toolchain,"node_modules/google-closure-compiler-linux/compiler");
    const args = ["--compilation_level","ADVANCED","--language_in","ECMASCRIPT_2020","--language_out","ECMASCRIPT_2020",
      "--warning_level","QUIET","--js",request.input,"--js_output_file",request.output];
    if (request.externs) args.push("--externs",request.externs);
    execFileSync(executable,args,{stdio:["ignore","pipe","pipe"],timeout:request.timeout_ms});
    code = readFileSync(request.output,"utf8");
    break;
  }
  case "upstream": code = source(); break;
  default: throw new Error(`unknown recipe: ${request.recipe}`);
}
if (typeof code !== "string") throw new Error("recipe returned no code");
writeFileSync(request.output, code);
const cpu = process.cpuUsage(started);
process.stdout.write(JSON.stringify({wall_ns:Number(process.hrtime.bigint()-clock),
  cpu_us:cpu.user+cpu.system, includes_children:request.recipe === "closure-advanced" ? false : null})+"\n");
