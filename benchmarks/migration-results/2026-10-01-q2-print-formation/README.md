# Q2: incremental formation for print-site changes

Algorithm 62 reuses a completed single-file target when assignments differ only
in nonstructural print sites. It copies structure without stale print proofs,
updates choice receipts and derives fresh print proofs. Complete family flags,
structural assignments, candidate/head/host identities and permissions still
qualify reuse. Author pins are checked again. Multi-file placement is rebuilt
because it can depend on printed costs or create cross-file setters.

The existing two-slot cache is unchanged in capacity. Both modes admit the same
complete cold stage plus a conservative overlap allowance for print-site
metadata already live while new proofs use scratch. `formation_reuse=false`
executes full formation; enabled reuse avoids structural passes. No effort or
search schedule change. Fixed target schedules and loop-proof root lists now
use stack storage instead of small unadmitted vectors.

## Qualification

All 12 distinct focused checks pass across the initial run and affected reruns.
They compare complete freshly formed/incremental trees, actual cold work/live/
peak bills, incremental storage bounds, every structural key family, admission
refusals/cleanup and raw/gzip/Brotli search parity. A small quote fixture proves
that each objective actually reaches incremental formation. The original larger
search fixture did not reach such a move in its raw schedule; its failed hit
assertion is retained. The implicit single-root path admits zero explicit
entries. Existing timing tests also exposed stale positional assertions and an
old reuse counter reporting hits as bytes; those are corrected and pass.

The [36 release checks](comparison.json) cover single-file print choices,
embedded hosts, split shared modules and delivered setters. Every file hash,
Node observation and independently measured objective score matches algorithm
61. Totals remain **899 raw / 878 gzip / 672 Brotli**. Reuse on/off resources and
search results match. The print fixture avoids another 12 raw / 6 gzip / 23
Brotli structural formations. Multi-file controls take their unchanged paths.
No full library or fleet rerun.

The release binary is pinned in [identity.json](identity.json), with the source
diff, compiler version and SHA-256. `run.py`, `measure.mjs` and `marked.mjs` retain
reproduction inputs. This completes changed-print-site formation, not arbitrary
changed-source-unit lowering or the remaining Q2 acceptance tasks.

## Markedlil

One raw/13 reuse-on/off pair preserves the exact prior 31,882-byte artifact,
two Markdown API observations, 354 judgments and identical logical resource
receipts. Full formations fall from 1,451 cold to 1,254 enabled: 22 complete-tree
hits and 175 incremental print variants. The samples use 54.59 CPU seconds cold
and 48.48 enabled. This is one pair, not a stable timing estimate; the reduction
in physical formation counts is directly observed. Inputs, compiler hashes,
phase counts and resource receipts are in `marked-comparison.json`.
