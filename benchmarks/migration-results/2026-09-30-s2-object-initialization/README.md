# S2: preserve object initialization during scalar replacement

Source `5e7c9ad2`. Compiler `~/lilscript-work/bin/s2-object-initialization-1/lilscript` SHA-256 `25c34a8f187280562ec87f3bbda302c99b9ee1d654d027cc35df401a30db2f30`; codec SHA-256 `ac595bed002d727aa10a5bf026c6103f3119975bf7f6b6de81e941a4e5a9fd22`.

Replacing an object literal with separate local bindings previously exposed partially initialized fields. Reentrant callbacks, an initializer that throws and a direct self-read could observe a value where the original program throws `ReferenceError`. Captured objects now require initialization that cannot call out or throw: inert creation or binding reads proved initialized at that point. Uncaptured initializers must preserve direct self-read TDZ behavior. Argument order and the scalar-replacement veto remain unchanged.

The first full test run exposed a useful optimization blocked by an overly conservative first guard. The proof was extended to initialized locals and parameters, including nested creation; the existing captured-counter test remains intact. Five independent execution tests pass, including public source under every objective with TOML scalar replacement on/off. All 1,733 active library tests pass, with four existing ignores and the previously qualified expensive table case excluded.

The [generic gate](ratchet.md), with [full receipt](ratchet.json), passes all 642 programs under each objective. Every recorded size and artifact hash matches qualified S3 strict inlining. All 20 [port artifact records](ports.json) are identical too, with 29/29, 1,353/1,353 and 21/21 reference tests passing. No size baseline update is needed.

Compilation cost is measured together with the subsequent explicit proxy controls in the [C1 paired report](../2026-09-30-c1-proxy-policy/README.md), against qualified S3. Those pairs keep the same artifacts, exact judgments and counted work; all CPU sample ranges overlap. This does not isolate a per-fix CPU effect. Broader alias, escape and flattening opportunities remain S2 work.
