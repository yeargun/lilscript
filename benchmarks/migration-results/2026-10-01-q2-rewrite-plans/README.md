# Q2: folding, signature and inline planning

Algorithm 57 connects literal folding, signature coupling and inlining to the
existing allocation owner. Fresh plans have temporary lifetimes; retained
source mutations keep their original owner. Sparse graft maps now use admitted
sorted ID entries instead of opaque hash allocations. Rejected legality probes
release their allocations; resource refusal propagates separately from a
normal declined rewrite, including during closure-subtree construction.

Signature planning counts all/owner cell reads once and groups eligible bodies
in deterministic signature order. Folding propagates required evaluation to
containing regions once per body. Inlining streams value-read checks and uses
admitted activation and evaluation-boundary arrays. Numeric and quoted-string
lengths use the shared formatter with a counting writer, avoiding temporary
literal strings and duplicate numeric spelling logic.

No permission, effort gate or heuristic threshold changes. Existing work/memory
limits include the newly covered owners. No compression or CPU improvement is
claimed here. Aggregate rewrite planning, exact-value evaluation, later dirty
dependencies, changed-unit formation and per-module elaboration remain Q2 work.

The focused runner accepts a library test binary from the completed build and
checks the complete requested test list before executing it. It covers source
publication/refusal, probe cleanup, signature coupling, literal/branch folding,
call evaluation order, nested/cross-module captures, native closure lifetime,
cold/reused behavior, and literal spelling. No full library, CLI, release,
ratchet or fleet rerun is requested.

All 38 distinct selected checks pass. The initial run passed 37 and stopped the
native case because Clang was absent from PATH. That single case passes with
the existing user-space Ubuntu Clang 18.1.3 executable; the other 37 were not
repeated. Native qualification includes the helper's GCC/Clang O0/O2/UBSan
matrix. `focused.log` retains the environment failure and `native-followup.log`
retains the successful follow-up. The runner accepts the Clang executable as an
optional second argument, or uses `LILSCRIPT_NATIVE_CLANG`/PATH.
