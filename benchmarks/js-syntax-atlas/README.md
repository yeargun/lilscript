# JavaScript compression syntax atlas

`report.html` is a self-contained, interactive report over interchangeable or
commonly-confused minified JavaScript spellings. It covers loops, control flow,
object/property operations, prototype construction, functions, numeric and
string idioms, collections, expression spelling, async iteration, closure/scope
representations, common-subexpression sharing, invariant hoisting, non-escaping
object removal, and allocation identity.

Every comparison includes its semantic contract and failure modes. A `trap`
row is measured for illustration but is excluded from winner selection. A
`narrow` row is rankable only under the extra restriction printed beside it.
The report is about transfer size, not runtime speed or allocation cost. Pure
ECMAScript-observable behavior is still part of the contract: referential
identity, getter/proxy reads, evaluation order, direct-eval scope, live closure
bindings, RegExp state, tagged-template site objects, and promise/function/object
freshness are not dismissed merely because final printed output matches.

Each spelling has three measurement lanes:

- `single`: the exact displayed bytes as an independent stream;
- `repeated`: 32 block-scoped copies in one stream;
- `context`: marginal bytes after appending the spelling to one fixed,
  reviewable, application-shaped JavaScript corpus.

Raw bytes, gzip level 9, and Brotli quality 11 are measured by the repository's
canonical native scorer (stock zlib 1.3.1 and official Google Brotli 1.1.0).
The report embeds the scorer provenance, exact context bytes, and context hash.

Rebuild or verify the checked-in report:

```sh
node benchmarks/js-syntax-atlas/build.mjs
node benchmarks/js-syntax-atlas/build.mjs --check
node --test benchmarks/js-syntax-atlas/*.test.mjs
```

`semantics.test.mjs` contains executable positive cases and counterexamples for
the indirect transformations. In particular, it proves why adding an IIFE is
not a transparent substitute for a block around sloppy direct eval, why
per-iteration `let` closures cannot become one `var` capture, and why equal or
frozen aggregates cannot generally be shared.

Edit `catalog.mjs` to add a race. Candidate source must be already minified,
parse as a function body, omit a trailing semicolon, and state the exact
conditions under which the alternatives are interchangeable.
