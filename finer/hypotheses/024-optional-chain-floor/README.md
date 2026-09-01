# 024 — The ES syntax floor never actually rejected optional chaining

**Status: DEAD CHECK FOUND AND FIXED. Latent, not shipped — but it is the guard that is supposed to
stop us shipping syntax a target cannot parse.**

## How it surfaced

Running the full suite after [023](../023-unparseable-class-expressions/README.md) turned up a
failure that **is not mine** — it reproduces at `HEAD` with my change stashed:

```
test js_peephole::tests::rejects_generated_syntax_above_the_configured_floor ... FAILED
panicked at src/js_peephole/tests.rs:1233:10   // .unwrap_err()
```

The test asserts `validate_generated_javascript_syntax_floor("let a=o?.x", Es2019)` is an error.
It returned `Ok`.

Worth stating plainly: I reported "1631 tests passing" at the end of the previous hypothesis. That
was wrong — the suite had this failure in it, and I had not run it.

## The defect

`src/js_peephole/mod.rs`, scanning tokens for features the target edition forbids:

```rust
let feature = match token.text {
    "?." => Some(JsSyntaxFeature::OptionalChain),
```

**The lexer never produces a `"?."` token.** `src/js_peephole/token.rs:739` says so itself, and
relies on it:

```rust
"?" => {
    // `?.` reaches the lexer as `?` then `.`; optional chaining is
    // not a conditional and has no `:` to pair with.
    let optional_chain = tokens.get(index + 1)
        .is_some_and(|next| next.text == "." && next.start == tokens[index].end);
```

So the match arm was unreachable and the floor admitted `?.` at any edition. `??`, by contrast, *is*
a single token, so nullish coalescing was checked correctly — which is why the gap survived: the
neighbouring feature in the same `match` worked.

## The fix

Test the pair by adjacency, exactly as `token.rs` already does:

```rust
"?" if tokens
    .get(index + 1)
    .is_some_and(|next| next.text == "." && next.start == token.end) =>
{
    Some(JsSyntaxFeature::OptionalChain)
}
```

Adjacency is what makes this correct rather than merely working: `?` and `.` are only optional
chaining when they touch. A conditional like `a?.5:b` is not a chain — and the lexer hands `.5`
back as one number token, so it cannot collide either way.

## Blast radius: none shipped

The default target is `EcmaScriptEdition::Es2022` (`src/config.rs:1263`), and **no port sets
`ecmascript` or `browsers`** in any `lilscript*.toml` across the tree. Every port is compiled at a
target where `?.` is legal, so no artifact was mis-emitted. This was a guard that would have failed
open the first time somebody lowered a target — not damage already done.

Independently confirmed by parsing every artifact of every sibling port: **211/211 parse.**

## What this says about the checks themselves

Both this and [023](../023-unparseable-class-expressions/README.md) are the same shape: a condition
written against a spelling the rest of the pipeline does not use. 023 keyed a terminator off the
declaration *keyword* when the emitted *shape* is what matters; 024 matched a token text the lexer
never emits. Neither is caught by size measurement, and 023 shipped two ports that could not build
at all.

`comparison/markdown-stack/parse-check.mjs` is the cheap standing answer for the emit side: run a
real parser over every declared artifact, and unparseable output is named instead of showing up as
an esbuild stack trace from a size run. It takes about a second.
