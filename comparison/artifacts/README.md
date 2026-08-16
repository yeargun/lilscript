# Checked-in comparison artifacts

Each directory is a reproducible snapshot of one comparison application:

- `lilscript.js`: default JavaScript emitted with the app's discovered config,
  retained as the JS/native/C behavior-parity snapshot;
- `lilscript-raw.js`, `lilscript-gzip.js`, and `lilscript-brotli.js`: independent
  size-first builds for the named objective. Each is gated only on that metric;
  its other two compressed-size measurements are diagnostic;
- `lilscript.c`: portable C emitted from the same optimized SSA module;
- `lilscript-native-macos-arm64`: native executable produced on macOS ARM64;
- `closure-advanced.js`: equivalent JavaScript compiled by Closure ADVANCED;
- `expected.stdout`: behavior expected from every executable form;
- `report.json` and `report.md`: objective-specific raw, gzip-9, and Brotli-11
  measurements plus hashes and diagnostic cross-metrics for all three LilScript
  artifacts;
- `toolchain.txt`: pinned compiler versions.

The editable LilScript and Closure input programs remain in the corresponding
`comparison/apps/<name>/lilscript` and `comparison/apps/<name>/closure`
directories. Rebuild and refresh all snapshots with:

```sh
comparison/run-all.sh
comparison/export-artifacts.sh
comparison/test-artifacts.sh
```

The checked-in native files run only on macOS ARM64. On another platform,
compile the portable `lilscript.c` or run the application `build.sh` to produce
a native executable for that host.
