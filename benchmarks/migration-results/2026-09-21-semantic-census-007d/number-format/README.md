# Native number formatting against Node

`number-format.lil` prints 5,232 floats: every power of two from 2^0 up to
overflow and down to zero through the subnormals, thirds of them, a
compounding series in templates, integers near 2^53, `NaN`, the infinities
and negative zero.

`node.out` is the semantic JavaScript route (`--mode development`, Node 24).
`native.out` is the semantic C route (`--target c`, `cc -O1 -fno-fast-math
-ffp-contract=off`). The two files are byte-identical.

Replay:

    lilscript number-format.lil --config tests/config/no-optimization.toml \
      --backend semantic --mode development --target c --output n.c
    cc -std=c11 -O1 -fno-fast-math -ffp-contract=off n.c -lm -o n && ./n > native.out
