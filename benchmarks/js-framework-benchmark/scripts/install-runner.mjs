import { resolve } from "node:path";
import { upstreamRoot } from "./paths.mjs";
import { run } from "./process.mjs";

const env = { ...process.env, PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD: "1" };
run("npm", ["ci", "--ignore-scripts", "--legacy-peer-deps"], {
  cwd: upstreamRoot,
  env,
});
run("npm", ["ci", "--ignore-scripts"], {
  cwd: resolve(upstreamRoot, "server"),
  env,
});
run("npm", ["ci", "--ignore-scripts"], {
  cwd: resolve(upstreamRoot, "webdriver-ts"),
  env,
});
run("npm", ["run", "compile"], {
  cwd: resolve(upstreamRoot, "webdriver-ts"),
  env,
});
