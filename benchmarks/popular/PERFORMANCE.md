# Selected-entrypoint performance and retained-memory checks

Median of 9 isolated Node v22.21.1 processes. LilScript runtime uses the Brotli-objective modules emitted by the preceding size build with explicit `lilscript.toml`; raw and gzip diagnostics do not select a runtime artifact. Time workloads use identical inputs and checksums; retained memory is the unclamped heap-used delta after forced GC while keeping equivalent results or emitter state alive. Nano ID compares the same published browser entrypoint used by the size lane, not its distinct pooled Node entrypoint. Ratios are LilScript / npm. Eligible exact ports must remain at or below 1.05 for both median time and retained memory.

| Project | npm ms | LilScript ms | Time ratio | npm retained B | LilScript retained B | Memory ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| nanoid | 7.852 | 7.788 | 0.992 | 618280 | 618208 | 1.000 |
| mitt | 8.021 | 8.079 | 1.007 | 2088760 | 2093416 | 1.002 |
| clsx | 10.987 | 10.977 | 0.999 | 1785952 | 1786824 | 1.000 |
| gl-matrix | 1.112 | 1.088 | 0.978 | 1813368 | 1821768 | 1.005 |
