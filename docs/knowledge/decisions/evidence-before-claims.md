# Evidence before compression claims

Status: accepted. Parent: [design decisions](README.md).

## Intent

Make “smaller” reproducible and scoped rather than a copied number that changes
meaning across pages.

## Decision

Source/tests define behavior. Tracked generated reports define numbers and name
the source, compiler, config, artifact, scorer, harness, boundary, and semantic
result. Prose links to those records instead of becoming a second numerical
database.

Direct LilScript compiler output and downstream deployment pipelines are
separate evidence lanes. Only direct output versus an independently authored,
eligible JS baseline supports a language/compiler compression claim. A
post-minified LilScript artifact is diagnostic.

## Tradeoff

Some appealing claims remain unpublished until the evidence contract can rerun
them. That is preferable to proving the wrong boundary or comparing stale files.

## Refusal

- No inherited `dist/` artifact after a failed build.
- No cross-metric offset: a gzip gain does not pass a Brotli regression.
- No broad “all JS libraries” claim from a partial or ineligible corpus.

Contracts: [verification](../verification/README.md). Results:
[evidence](../evidence/README.md).
