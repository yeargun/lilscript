# Q2: aggregate rewrite plans

Policy algorithm 58 admits field propagation, scalar replacement, namespace
flattening, object normalization, record aliases and contiguous field-store
collection through the existing allocation owner. Plans retain their nested
backing until application completes; rejected probes and refusals release it.
Field-store tracking uses admitted sparse entries. Shared creator edges and an
explicit stack replace recursive whole-program lexical-scope scans, preserving
the requirement that every creator path reach the field bank's scope.
Primitive-class construction now waits until an aggregate consumer needs it.
Unused inspection-only behavior/read wrappers are removed.

No tactic permission or effort gate changes. Newly counted work/storage may
refuse tight limits that previously missed those costs. No measured size or CPU
improvement is claimed. Exact-value evaluation, remaining dirty dependencies,
changed-unit formation and per-module elaboration remain Q2 work.

## Qualification

All 23 selected checks pass: the 18 aggregate semantic cases, exact publication,
resource-refusal and probe cleanup, cold/reused rule views, and the native
captured-bank case with its GCC/Clang O0/O2/UBSan matrix. Native Clang is the
already installed 18.1.3 binary named in the runner invocation. The first build
caught a planner/caller error-type mismatch; the corrected build passes. There
was one focused runtime run, with no full library, release, ratchet or fleet
rerun. Logs and the explicit test list are retained here.
