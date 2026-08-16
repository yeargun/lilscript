import { existsSync } from "node:fs";
import { join } from "node:path";
import { defineConfig } from "vite";

function rewritePublicIndex() {
  return {
    name: "public-directory-index",
    configureServer(server) {
      server.middlewares.use((req, _res, next) => {
        const [path, query] = (req.url ?? "").split("?");
        if (path && path.length > 1 && path.endsWith("/")) {
          const file = join(server.config.publicDir, path.slice(1), "index.html");
          if (existsSync(file)) {
            req.url = `${path}index.html${query ? `?${query}` : ""}`;
          }
        }
        next();
      });
    },
    configurePreviewServer(server) {
      server.middlewares.use((req, _res, next) => {
        const [path, query] = (req.url ?? "").split("?");
        if (path && path.length > 1 && path.endsWith("/")) {
          const file = join(server.config.publicDir, path.slice(1), "index.html");
          if (existsSync(file)) {
            req.url = `${path}index.html${query ? `?${query}` : ""}`;
          }
        }
        next();
      });
    },
  };
}

export default defineConfig({
  appType: "mpa",
  plugins: [rewritePublicIndex()],
  server: {
    proxy: {
      "/api": "http://127.0.0.1:4173",
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    rolldownOptions: {
      input: {
        home: resolveHtml("index.html"),
        demos: resolveHtml("demos.html"),
        playground: resolveHtml("playground.html"),
        about: resolveHtml("about.html"),
        benchmarks: resolveHtml("benchmarks.html"),
        explorer: resolveHtml("explorer.html"),
        delivery: resolveHtml("delivery.html"),
        benchmarkDetail: resolveHtml("benchmark-detail.html"),
        libraries: resolveHtml("libraries.html"),
        roadmap: resolveHtml("roadmap.html"),
        docs: resolveHtml("docs.html"),
        lastro: resolveHtml("lastro.html"),
        lilastro: resolveHtml("lilastro.html"),
        solidlil: resolveHtml("solidlil.html"),
        marketplace: resolveHtml("marketplace.html"),
      },
    },
  },
});

function resolveHtml(file) {
  return join(import.meta.dirname, file);
}
