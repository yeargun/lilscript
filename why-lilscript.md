# Why LilScript

LilScript is a compression-first web language: typed, closed-world, compiling to highly optimized JavaScript (primary) and native/`exec` (secondary). It is not a TypeScript glue layer. Types, `extern` boundaries, effects, and modules exist so the compiler can delete, dissolve, mangle, and **score complete artifacts under gzip or Brotli** — the bytes that are actually served.

Local “this spelling is shorter” is not the objective. The compiler performs
proof-gated, deterministic bounded search and exactly scores complete artifacts
under the configured codec. It searches toward global optima without claiming
that an NP-hard, non-additive space was exhausted. Tradeoffs among transfer size,
compile time, and runtime are explicit. Bundling, lazy `import()`, and progressive
enhancement are language concerns, not afterthoughts.

The maintained reasoning tree starts with user intent, then contracts, durable
decisions, current architecture, planned architecture, verification, evidence,
and migration:

**[docs/knowledge/README.md](docs/knowledge/README.md)**

Canonical syntax and schema: [docs/language-v0.1.md](docs/language-v0.1.md),
[docs/configuration.md](docs/configuration.md). Current truth:
[docs/current-status.md](docs/current-status.md).
