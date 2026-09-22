# probelil

The LilScript migration's inner loop. Not a port of anything: one small library
whose source deliberately exercises every language feature, so a compiler change
that breaks one shows up in seconds rather than in a 130-second port build.

    LILSCRIPT_COMPILER=../lilscript/target/release/lilscript node scripts/build.mjs --compile

It is a differential build. The same source is emitted at `preset = "none"` and
at the shipped config, both are run, and both are compared to `expected.out`. A
wrong program at either level fails here.

`--bless` rewrites `expected.out` from the optimized lane. Only bless after
reading the diff and believing it.

`extern int read()` is the opacity valve: constant folding would otherwise
evaluate most of the probe at compile time and it would test the folder instead
of the emitter. The runner defines `read()` as a constant so the output stays
deterministic.
