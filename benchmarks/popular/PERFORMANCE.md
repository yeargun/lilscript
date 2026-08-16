# Selected-entrypoint performance and retained-memory checks

Median of 1 isolated Node v22.21.1 processes. LilScript runtime uses the Brotli-objective modules emitted by the preceding size build with explicit `lilscript.toml`; raw and gzip diagnostics do not select a runtime artifact. Time workloads use identical inputs and checksums; retained memory is the unclamped heap-used delta after forced GC while keeping equivalent results or emitter state alive. Nano ID compares the same published browser entrypoint used by the size lane, not its distinct pooled Node entrypoint. Ratios are LilScript / npm. Eligible exact ports must remain at or below 1.05 for both median time and retained memory.

| Project | npm ms | LilScript ms | Time ratio | npm retained B | LilScript retained B | Memory ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| nanoid | 6.809 | 7.027 | 1.032 | 619432 | 618208 | 0.998 |
| mitt | 25.051 | 19.248 | 0.768 | 2088760 | 2088832 | 1.000 |
| clsx | 26.717 | 27.746 | 1.039 | 1785952 | 1786784 | 1.000 |
| gl-matrix | 2.853 | 2.317 | 0.812 | 1813368 | 1823264 | 1.005 |
