import { resolve } from "node:path";
import { defineConfig } from "vite";

export default defineConfig({
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
        playground: resolve(import.meta.dirname, "index.html"),
        about: resolve(import.meta.dirname, "about.html"),
        benchmarks: resolve(import.meta.dirname, "benchmarks.html"),
        explorer: resolve(import.meta.dirname, "explorer.html"),
        delivery: resolve(import.meta.dirname, "delivery.html"),
        benchmarkDetail: resolve(import.meta.dirname, "benchmark-detail.html"),
        libraries: resolve(import.meta.dirname, "libraries.html"),
        roadmap: resolve(import.meta.dirname, "roadmap.html"),
        docs: resolve(import.meta.dirname, "docs.html"),
      },
    },
  },
});
