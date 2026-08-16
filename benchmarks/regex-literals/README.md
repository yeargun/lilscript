# Regex literal benchmark

This JavaScript-only gate compares the exact same typed Regex program with the
`regex-literals` compression decision enabled and disabled. Literal emission is
restricted to a conservatively proven plain-pattern subset; complex patterns
remain `new RegExp(...)` so construction errors retain their runtime position.
The report records raw/gzip/Brotli sizes and checks `g`-flag state changes,
metadata, output, runtime, and retained heap. Because both builds use the
Brotli configuration, only Brotli-11 is a size gate.

This is a same-artifact pass-isolation experiment. Raw and gzip are diagnostics
for the Brotli-selected artifacts and may lose; they are not evidence or gates
for those other objectives.

Run `node benchmarks/regex-literals/run.mjs`.
