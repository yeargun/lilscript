# Why LilScript

LilScript is a compression-first web language: typed, closed-world, compiling to highly optimized JavaScript (primary) and native/`exec` (secondary). It is not a TypeScript glue layer. Types, `extern` boundaries, effects, and modules exist so the compiler can delete, dissolve, mangle, and **score complete artifacts under gzip or Brotli** — the bytes that are actually served.

Local “this spelling is shorter” is not the objective. The compiler searches for global codec optima, with explicit tradeoffs among transfer size, compile time, and runtime. Bundling, lazy `import()`, and progressive enhancement are language concerns, not afterthoughts.

The full reasoning tree (language → compilation → every TOML knob → jQuery evidence):

**[docs/knowledge/README.md](docs/knowledge/README.md)**

Canonical syntax and schema: [docs/language-v0.1.md](docs/language-v0.1.md), [docs/configuration.md](docs/configuration.md).
