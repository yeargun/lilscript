# G2: first measurement, before preservation correction

Source `4447524b`, compiler SHA-256 `32437612ef6925474aa325e782dc5af5a01b07226d31900197cf2c097a3a16f0`, codec `006eb31ae4d3764b859083b0d5a6d1e388f2c09b610fcc79aba51f67c815042f`.

These measurements are retained as an initial experiment. This compiler was **not accepted as a baseline**: the new property producer did not yet consume explicit `mangle.preserve_properties`. Source `7b97a107` corrects preservation, excludes reserved names from generated keys and covers direct/cached formation. Final qualification belongs in [the corrected release](../2026-09-30-g2-private-fields/README.md).

The [generic run](ratchet.json) passes all 642 programs under each objective, with −410 raw, −176 gzip and −123 Brotli bytes and no individual growth. These configurations do not set preserved properties. The [reference suites](ports.json) pass 29/29, 1,353/1,353 and 21/21 tests.

[Five alternating CPU pairs](compile-cost.json), frozen at effort 13, give median ratios 1.387 markedlil, 2.510 zodlil and 1.004 posthoglil. Their Brotli sizes are 9,158 → 8,851, 26,642 → 26,573 and 5,224 → 5,224 bytes; exact terminal judgments are 49 → 68, 28 → 67 and 25 → 25. The cost is real: sampled ranges are separated for the first two workloads. Warm-up telemetry places zodlil's additional work in target rules and canonical encoding. Q2 owns repeated formation; this experiment establishes neither runtime parity nor an optimal search policy.
