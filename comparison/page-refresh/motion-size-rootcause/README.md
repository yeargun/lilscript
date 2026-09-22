# Motion size diagnosis

These are isolated diagnostic builds of publication `d4e7a9c`, using the same
pinned compiler and source-built original as the live comparison. They do not
replace the published comparison or performance inputs.

`size-attribution.json` records exact bundle hashes, matching 50-export size
experiments, emitted-syntax counts and source-map attribution (unmapped bytes
remain explicit). `optimization-levels.json` records levels 3 and 9. Level 13
fails compilation; its error is in `level13.log`. The existing 37 browser checks
pass with the full entry built at level 9 and default entries left unchanged.

To reproduce, use an isolated Motionlil checkout, install its locked dependencies
and build with `MOTIONLIL_KEEP_COMPILER_OUTPUT=1` and the recorded release compiler.
Run `node analyze.mjs CHECKOUT OUTPUT_DIRECTORY`. For each optimization-level
config, compile `src/full.lil` to `src/.__compiler-levelN.mjs`, then run
`node measure-levels.mjs CHECKOUT OUTPUT_DIRECTORY` for levels 3 and 9.

The copied configs and logs are evidence, not a production configuration change.
Gzip measurements use Python gzip level 9 with mtime 0 to match the publication;
Brotli uses quality 11. Compression deltas cannot be added as source attribution.
