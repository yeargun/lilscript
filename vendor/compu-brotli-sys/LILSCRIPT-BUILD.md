# Pinned Brotli build integration

This directory copies the exact `compu-brotli-sys` 1.1.0 package already pinned by LilScript. `UPSTREAM-SHA256.json` records every original package file and, when available, the registry archive SHA256. Original licenses and the bundled Google Brotli 1.1.0 sources are retained.

The sole upstream file change is in `build.rs`: define the encoder’s official `BROTLI_ENCODER_CLEANUP_ON_OOM` macro. The upstream default otherwise calls `exit(EXIT_FAILURE)` when a later custom allocation returns null, which would make compiler budget rejection terminate the process. Cleanup mode reports failure and frees admitted allocations. No encoder algorithm source or quality/window setting changes.

`Cargo.toml` uses a local crates.io patch for this exact package. A global CFLAGS setting is deliberately unnecessary. This build setting belongs in codec provenance along with package/library versions; equivalence tests compare canonical scores with the unmodified convenience encoding API, and a child-process test exercises late allocation denial.
