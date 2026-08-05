import {brotliCompressSync, constants, gzipSync} from "node:zlib";
import {readFileSync, writeFileSync} from "node:fs";
import {basename, join} from "node:path";

const [app, lilscriptPath, closurePath, lilscriptVersion, closureVersion] = process.argv.slice(2);
if (!closureVersion) throw new Error("missing measurement arguments");

function sizes(path) {
  const source = readFileSync(path);
  return {
    raw: source.length,
    gzip9: gzipSync(source, {level: 9, mtime: 0}).length,
    brotli11: brotliCompressSync(source, {
      params: {[constants.BROTLI_PARAM_QUALITY]: 11},
    }).length,
  };
}

const result = {
  app: basename(app),
  lilscriptVersion,
  closureVersion,
  lilscript: sizes(lilscriptPath),
  closure: sizes(closurePath),
};
const build = join(app, "build");
writeFileSync(join(build, "report.json"), `${JSON.stringify(result, null, 2)}\n`);

const winner = (metric) => {
  const lilscript = result.lilscript[metric];
  const closure = result.closure[metric];
  if (lilscript === closure) return "Tie";
  return lilscript < closure ? "LilScript" : "Closure";
};
const markdown = `# ${result.app}\n\n` +
  `LilScript ${lilscriptVersion} vs Closure Compiler ${closureVersion} ADVANCED.\n\n` +
  `| Compiler | Raw | Gzip-9 | Brotli-11 |\n` +
  `| --- | ---: | ---: | ---: |\n` +
  `| LilScript | ${result.lilscript.raw} | ${result.lilscript.gzip9} | ${result.lilscript.brotli11} |\n` +
  `| Closure | ${result.closure.raw} | ${result.closure.gzip9} | ${result.closure.brotli11} |\n\n` +
  `Winners: raw **${winner("raw")}**, gzip **${winner("gzip9")}**, ` +
  `Brotli **${winner("brotli11")}**.\n`;
writeFileSync(join(build, "report.md"), markdown);
process.stdout.write(markdown);
