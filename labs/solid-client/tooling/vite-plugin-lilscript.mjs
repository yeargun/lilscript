import { spawnSync } from "node:child_process";
import { compilerPath } from "./compiler-path.mjs";

export function lilscript({ prelude, config, target = "js" } = {}) {
  return {
    name: "lilscript",
    enforce: "pre",
    transform(_source, id) {
      const path = id.split("?", 1)[0];
      if (!path.endsWith(".lil")) return null;
      const args = [path, "--target", target];
      if (config) args.push("--config", config);
      const result = spawnSync(compilerPath(), args, {
        encoding: "utf8",
        env: process.env,
      });
      if (result.status !== 0) {
        this.error(result.stderr.trim() || `LilScript failed for ${path}`);
      }
      const setup = prelude ? `import ${JSON.stringify(prelude)};` : "";
      return {
        code: `${setup}${result.stdout};export default null;`,
        map: null,
      };
    },
    handleHotUpdate(context) {
      if (context.file.endsWith(".lil")) {
        context.server.ws.send({ type: "full-reload" });
        return [];
      }
      return undefined;
    },
  };
}
