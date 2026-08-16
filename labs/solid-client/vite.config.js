import { resolve } from "node:path";
import { defineConfig } from "vite";
import solid from "vite-plugin-solid";
import { lilx } from "./tooling/vite-plugin-lilx.mjs";
import { lilscript } from "./tooling/vite-plugin-lilscript.mjs";

export default defineConfig(({ mode }) => {
  const lsx = mode === "lsx-solid" || mode === "lsx-lilscript";
  const solidTarget = mode === "solid" || mode === "lsx-solid";
  const target = solidTarget ? "solid" : "lilscript";
  const app = lsx ? `lsx-${target}` : target;
  const root = resolve(import.meta.dirname, "apps", app);
  return {
    root,
    plugins: [
      solidTarget && solid(),
      !solidTarget &&
        !lsx &&
        lilscript({
          prelude: resolve(root, "src", "host.js"),
          config: resolve(import.meta.dirname, "config", "closed-world.toml"),
          target: "js",
        }),
      mode === "lsx-lilscript" &&
        lilx({
          config: resolve(import.meta.dirname, "config", "closed-world.toml"),
          target: "js",
          reactiveImport: "../../apps/lilscript/src/reactive",
          domImport: "../../apps/lilscript/src/web",
        }),
    ].filter(Boolean),
    server: {
      port: target === "solid" ? 5181 : 5180,
      strictPort: true,
    },
    build: {
      outDir: resolve(import.meta.dirname, "dist", app),
      emptyOutDir: true,
      manifest: true,
      minify: "oxc",
      target: "es2022",
      rollupOptions: {
        input: resolve(root, "index.html"),
      },
    },
  };
});
