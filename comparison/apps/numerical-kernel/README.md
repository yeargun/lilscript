# Numerical kernel

Recursive factorial, Euclidean GCD, and iterative Fibonacci. The paired sources
preserve LilScript's ordinary binary64 multiplication followed by signed-32-bit
normalization with `*` and `|0`. `Math.imul` is reserved for an explicit LilScript
`Math.imul` call because it has different large-product semantics.
