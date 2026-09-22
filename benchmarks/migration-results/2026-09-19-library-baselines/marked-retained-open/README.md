# Marked Retained Open Artifact

The [supplement receipt](receipt.json) passes without another LilScript
compilation. The [failed four-profile build](../marked-release-baseline/README.md)
remains failed. Its one complete Brotli/open output is reused only in this
separately identified supplementary run, not substituted into the failed build.

## Result

The unchanged packaging script ran in a fresh source-identical workspace whose
initial `dist` contained only the retained `marked.raw.js`. Compiler lookup was
disabled. All **29 frozen nodes (25 tests, four suites)** from the original
`compat.test.mjs`, `options.test.mjs` and `api.test.mjs` passed, with no filtering
inside those files. The original 660-example corpus, exclusions and inline-input
filters remain unchanged. The upstream package supplies assertion expectations;
the tested candidate modules are the separately observed ESM and CJS artifacts.

| Packaged Artifact | Raw | Gzip 9 | Brotli 11 |
|---|---:|---:|---:|
| ESM | 34,176 | 10,450 | 9,409 |
| CJS | 37,339 | 11,296 | 10,152 |

The preserved canonical codec measures each complete file independently. The
[test report](tests/report.json) records six production loads across the three
test processes, matching those exact artifact hashes:

- ESM: `e9a949d4b4a363c268c5b57bc844feced40bc3f55c24d6bc6755b49710d7a7de`.
- CJS: `e03e4173de323dedc8eab29b177f8d393893b97a5464955fc7c96618c171eecf`.

This is a baseline measurement, not a compression improvement or competitor win.
The whole packaging/test/measurement supplement took about 2.6 seconds on the
shared host; that does not measure compiler speed.

## Provenance And Limits

The receipt SHA-256 is
`dfc3859f181b032449dba37ebd401676000ff8d25b816b7438bf04e190009921`.
The [executed wrapper](wrapper.mjs) binds the failed parent, its successful
invocation, accepted release-compiler input manifests, retained raw bytes and
codec executable. All 104 original source entries match. The 1,109-entry
dependency tree, including nested packages, symlink destinations and executable
bits, was verified unchanged before and after. Sources, tools and packaged bytes
also remained stable. No library edit, dependency install or compiler rebuild
occurred.

The receipt retains the **46 unexecuted required identities** and all additional
coverage obligations. The original API tests exercise UMD through a VM, but the
existing observer does not bind that execution to exact scored UMD bytes; no
such qualification is claimed. Closed/gzip/raw profiles, full required-suite
qualification, browser/installed-package consumers and complete delivery-cost
acceptance remain open. The upstream-only `official-parse` file earns no
candidate credit. This older legacy-backend artifact does not qualify current
compiler sources, semantic-backend library support or D2/D5 decisions.

This README was added after the receipt was frozen and is not part of its output
inventory. Recorded evidence and the failed parent were not rewritten.
