# LilScript objective

The objective is [§2 of the architecture](../docs/future-architecture.md#2-what-the-compiler-is-for):
for each maintained library and each selected objective (Brotli first, then gzip
and raw bytes), deliver the smallest correct program, and beat the strongest
pinned competitor in both the open and the closed world. The rest of
[future-architecture.md](../docs/future-architecture.md) is the architecture that
serves it, and the single [migration plan](../docs/migration/index.md) holds every
implementation step, its gates and its progress. What stands today is in
[current status](../docs/current-status.md).

This page is a pointer, not another objective or plan. Earlier objectives are in
git history (`git show b79efb19^:finer/objective.md` is the last full contract,
2026-09-02); the owner's briefs are in [intent/](intent/). Their implementation
sequences and numeric policy proposals are retired.
