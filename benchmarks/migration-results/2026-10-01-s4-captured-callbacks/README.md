# S4 captured public callback transport

1. Replace the one-unit callback-use proof with an admitted alias graph over
   shared cells and all nested bodies. Permit capture when every observable use
   still invokes the private callback adapter. Keep identity/property/return,
   host storage, argument and reference escapes refused. Follow aliases across
   units independently of source declaration order; do not change purity or
   exception behavior.
2. Preserve one incoming adapter per call, default/reassigned callbacks, nested
   closures and escaped closures that only invoke the callback. The callback
   itself remains private, so no wrapper identity becomes observable.
3. Check all three objectives at low/default effort, development checks,
   original host getter order/count and exceptions, negative identity/escape
   cases and resource cleanup. Then build one release pin and compare existing
   supported control artifacts plus new capability output. No full-suite/fleet.

Prediction: existing supported programs retain bytes. More public callback
signatures become representable without copying host reference collections or
inventing a runtime generic type witness. The proof uses linear admitted
storage, sorting and linear validation rather than repeated alias scans.
Wider opaque generic/ambiguous-union/collection ABI remains separately open.

## Result

The admitted graph follows initializer aliases across all units, then validates
all uses with constant-time cell membership. A callback may remain captured in
returned/nested closures; direct return/storage, opaque conversion, property
observation and argument/ref escape remain refused. Existing callback codecs
are reused without an extra runtime representation or optimization control.

23 focused checks pass (public callbacks, unions, product boundaries and the
new success/work-refusal/memory-refusal ownership test). The release CLI passes
18 independent artifact oracles with exact scored/delivered sizes, including
development checks and delayed throws. The first measurement configuration
incorrectly placed `checks` at the root; correcting it to `javascript.checks`
required no compiler change. No full suite, fleet or timing run.

Pin `/home/azureuser/lilscript-work/bin/s4-captured-callbacks-1/lilscript`, SHA-256
`d79d17c8da61774c0a361ba9e6b9d6f551464a22d85ff2b309bfb07ea792f3eb`;
algorithm 41 / search 32 / walk 10 / local facts 14.
[identity.json](identity.json) retains exact source identities;
[comparison.json](comparison.json) retains artifacts and search counts.

| Program | Raw | Gzip | Brotli |
|---|---:|---:|---:|
| Existing product control, before = after | 175 | 134 | 112 |
| Existing callback control, before = after | 162 | 112 | 85 |
| New captured callbacks | 203 | 138 | 116 |
| New captured callbacks with development checks | 562 | 287 | 237 |

Both matched controls are byte-identical. New capabilities are not scored as
wins against an old compiler that refused them. S4 wider public erased ABI
remains open; no baseline or port artifact changed.
