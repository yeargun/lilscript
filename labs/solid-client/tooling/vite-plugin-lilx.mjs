import { readFileSync, unlinkSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { compileLilx } from "./lilx/compile.mjs";
import { compilerPath } from "./compiler-path.mjs";

export function lilx({
  prelude,
  config,
  target = "js-module",
  reactiveImport,
  domImport,
  hostImport,
} = {}) {
  return {
    name: "lilx",
    enforce: "pre",
    transform(source, id) {
      const path = id.split("?", 1)[0];
      if (!path.endsWith(".lilx")) return null;

      let lilscript;
      try {
        lilscript = compileLilx(source, {
          filename: path,
          reactiveImport,
          domImport,
          hostImport,
        });
      } catch (error) {
        this.error(error.message || String(error));
        return null;
      }

      const generated = resolve(
        dirname(path),
        `.lilx-${process.pid}-${Date.now()}.generated.lil`,
      );
      writeFileSync(generated, lilscript);
      const output = resolve(
        dirname(path),
        `.lilx-${process.pid}-${Date.now()}.js`,
      );
      const args = [generated, "--target", target];
      if (config) args.push("--config", config);
      args.push("-o", output);
      const result = spawnSync(compilerPath(), args, {
        encoding: "utf8",
        env: process.env,
        cwd: dirname(path),
      });
      if (result.status !== 0) {
        try {
          unlinkSync(generated);
        } catch {
          // Preserve the compiler diagnostic when temporary cleanup fails.
        }
        this.error(result.stderr.trim() || `LilScript failed for ${generated}`);
        return null;
      }
      const code = readFileSync(output, "utf8");
      for (const temporary of [generated, output]) {
        try {
          unlinkSync(temporary);
        } catch {
          // A failed cleanup should not obscure a successful compilation.
        }
      }
      const setup = prelude ? `import ${JSON.stringify(prelude)};\n` : "";
      return { code: `${setup}${code}\nexport default null;\n`, map: null };
    },
    handleHotUpdate(context) {
      if (context.file.endsWith(".lilx") || context.file.endsWith(".lil")) {
        context.server.ws.send({ type: "full-reload" });
        return [];
      }
      return undefined;
    },
  };
}
