# Selected-entrypoint performance and retained-memory checks

Median of 5 isolated Node v24.11.1 processes. Time workloads use identical inputs and checksums; retained memory is the unclamped heap-used delta after forced GC while keeping equivalent results or emitter state alive. Nano ID compares the same published browser entrypoint used by the size lane, not its distinct pooled Node entrypoint. Ratios are LilScript / npm. Eligible exact ports must remain at or below 1.05 for both median time and retained memory.

| Project | npm ms | LilScript ms | Time ratio | npm retained B | LilScript retained B | Memory ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| nanoid | 7.886 | 7.958 | 1.009 | 664296 | 669472 | 1.008 |
| mitt | 7.631 | 7.744 | 1.015 | 2093120 | 2094168 | 1.001 |
| clsx | 9.691 | 9.631 | 0.994 | 1789928 | 1791000 | 1.001 |
| gl-matrix | 1.218 | 1.243 | 1.020 | 1942368 | 1947192 | 1.002 |
