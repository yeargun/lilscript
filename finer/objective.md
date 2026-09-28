# LilScript objective

The objective is [§2 of the architecture](../docs/future-architecture.md#2-what-the-compiler-is-for).
For each maintained library, each export condition and each selected objective
(Brotli first, then gzip and raw bytes, each a codec with its parameters),
deliver a program that is, in this order: correct; not slower than the original
(runtime parity as the owner ruled, defined in §16.4); the smallest under that
objective; and built within the work budget the effort level sets (§13). It must
beat the strongest pinned competitor in every cell of the benchmark contract
(§18), in both the open and the closed world, and compile to directly executable
native code with identical results. The rest of
[future-architecture.md](../docs/future-architecture.md) (version 1) is the
design that serves it, [language.md](../docs/language.md) is the target language
contract, and the single [migration plan](../docs/migration/plan.md) holds every
implementation step and its gates; [its to-do list](../docs/migration/index.md)
holds its progress. What stands today is in
[current status](../docs/current-status.md).

This page is a pointer, not another objective or plan. Earlier objectives are in
git history (`git show b79efb19^:finer/objective.md` is the last full contract,
2026-09-02); the owner's briefs are in [intent/](intent/). Their implementation
sequences and numeric policy proposals are retired.
