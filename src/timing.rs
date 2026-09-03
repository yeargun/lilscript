//! Compile-effort telemetry.
//!
//! Compressor-in-loop selection repeats a handful of whole-artifact operations
//! hundreds of times, so on a large library the wall clock is the product of
//! "how expensive is one pass over the artifact" and "how many did the search
//! buy". Neither factor was observable. These buckets make the split visible
//! without a profiler; they are pure observation and never influence a
//! selection.
//!
//! Enabled by `LILSCRIPT_TIMING`. When unset, `Bucket::scope` returns `None`
//! before reading the clock, so instrumented call sites cost one cached bool.
use std::sync::atomic::{AtomicU64, Ordering};

pub struct Bucket {
    name: &'static str,
    nanos: AtomicU64,
    calls: AtomicU64,
    bytes: AtomicU64,
}

impl Bucket {
    const fn new(name: &'static str) -> Self {
        Self {
            name,
            nanos: AtomicU64::new(0),
            calls: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
        }
    }

    /// Start timing one pass over `len` bytes. Cost is recorded when the guard
    /// drops, so a `?` early return still accounts for the work it did.
    pub fn scope(&'static self, len: usize) -> Option<Scope> {
        enabled().then(|| Scope {
            bucket: self,
            started: std::time::Instant::now(),
            len: len as u64,
        })
    }

    /// Record a completed pass whose duration was measured by the caller.
    pub fn record_pass(&self, len: u64, elapsed_nanos: u64) {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.bytes.fetch_add(len, Ordering::Relaxed);
        self.nanos.fetch_add(elapsed_nanos, Ordering::Relaxed);
    }

    /// Record a completed iterative pass directly: `count` is the iteration
    /// count rather than a byte length, so a fixed point reports how many
    /// rounds it needed alongside how long it took.
    pub fn record(&self, count: u64, elapsed_nanos: u64) {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.bytes.fetch_max(count, Ordering::Relaxed);
        self.nanos.fetch_add(elapsed_nanos, Ordering::Relaxed);
    }

    /// Record one deterministic event; `sum` is whatever the site wants
    /// totalled (a codec delta, a count of candidates left untried). Events
    /// are counters, not clocks, so unlike the timings they are exact across
    /// thread counts and hosts and can stand as a result (objective.md §8).
    pub fn event(&self, sum: u64) {
        if !enabled() {
            return;
        }
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.bytes.fetch_add(sum, Ordering::Relaxed);
    }

    fn snapshot(&self) -> (u64, u64, u64) {
        (
            self.nanos.load(Ordering::Relaxed),
            self.calls.load(Ordering::Relaxed),
            self.bytes.load(Ordering::Relaxed),
        )
    }
}

pub struct Scope {
    bucket: &'static Bucket,
    started: std::time::Instant,
    len: u64,
}

impl Drop for Scope {
    fn drop(&mut self) {
        self.bucket
            .nanos
            .fetch_add(self.started.elapsed().as_nanos() as u64, Ordering::Relaxed);
        self.bucket.calls.fetch_add(1, Ordering::Relaxed);
        self.bucket.bytes.fetch_add(self.len, Ordering::Relaxed);
    }
}

/// Canonical gzip-9 / Brotli-11 encodes of a whole artifact.
pub static CODEC: Bucket = Bucket::new("codec");
/// Generated-JavaScript lex, validation, and syntax metrics.
pub static ANALYZE: Bucket = Bucket::new("analyze");
/// IR to JavaScript text, including the inline text folds that follow it.
pub static EMIT: Bucket = Bucket::new("emit");
/// The parsed peephole over already-emitted JavaScript.
pub static PEEPHOLE: Bucket = Bucket::new("peephole");
/// Terminal cleanup passes over already-emitted JavaScript.
pub static CLEANUP: Bucket = Bucket::new("cleanup");
/// One whole SSA optimization pipeline over the IR module.
pub static OPTIMIZE: Bucket = Bucket::new("optimize");
/// One tokenization of generated JavaScript. Every peephole fold takes `&str`
/// and re-lexes from scratch, so this counts the pipeline's re-scanning tax.
pub static LEX: Bucket = Bucket::new("lex");
/// Whole-program structures that individual folds rebuild from tokens. Each is
/// derived from the same token vector the fold just re-lexed, so they inherit
/// the pipeline's re-derivation tax.
pub static CLOSERS: Bucket = Bucket::new("closers");
pub static REGIONS: Bucket = Bucket::new("regions");
pub static SCOPES: Bucket = Bucket::new("scopes");
pub static BINDINGS: Bucket = Bucket::new("bindings");
/// Peephole folds that rewrote nothing, and the time they spent proving it.
/// A fold whose enabling syntax is absent from the artifact still pays a full
/// scan, so this bucket measures the ceiling on guard-based skipping.
pub static IDLE_FOLD: Bucket = Bucket::new("idle_fold");
/// Peephole folds that did rewrite something.
pub static ACTIVE_FOLD: Bucket = Bucket::new("active_fold");

/// Artifact-admission outcomes during terminal candidate search. `calls` counts
/// validations and `bytes` accumulates rejections, so their ratio is the share
/// of proposed candidates the admission gate discards. A high rejection rate
/// means the search is settling for artifacts it did not choose on size.
pub static ADMISSION: Bucket = Bucket::new("admission");
/// Direct-artifact validation outcomes. A failure here discards a whole plan
/// before any of its candidates reach admission, so it is invisible to the
/// `admission` bucket.
pub static DIRECT_VALIDATE: Bucket = Bucket::new("direct_validate");
/// IR probes dropped because their configured emission failed to validate or
/// to score. A dropped probe takes every emission variant it would have
/// produced with it, so this is invisible to every other counter.
pub static PROBE_DROPPED: Bucket = Bucket::new("probe_dropped");

/// Iteration-count buckets. `calls` is the number of times the loop was
/// entered and `bytes` holds the worst single call's iteration count.
pub static SCALAR_FIXPOINT: Bucket = Bucket::new("scalar_fixpoint");
pub static INLINE_FIXPOINT: Bucket = Bucket::new("inline_fixpoint");

/// Exits of the late cleanup and of its converged-local-naming candidate
/// (`converge_local_names`), so a stage that silently never runs (036, 037,
/// 041) is visible in the report. `calls` counts the exit; the sum is the
/// site's own: the ledger left at entry, candidates left untried when a
/// starved loop broke, the codec delta of each vote.
pub static CLEANUP_ENTERED: Bucket = Bucket::new("cleanup_entered");
pub static CLEANUP_UNBUDGETED: Bucket = Bucket::new("cleanup_unbudgeted");
pub static CLEANUP_SKIPPED: Bucket = Bucket::new("cleanup_skipped");
// The late cleanup's canonical whole-artifact peephole candidate, by exit (047):
// a parse error, an unchanged artifact, the function-boundary guard, an
// admission refusal, a codec probe the budget could not pay, or pushed.
pub static CLEANUP_CANONICAL_ERR: Bucket = Bucket::new("cleanup_canonical_err");
pub static CLEANUP_CANONICAL_SAME: Bucket = Bucket::new("cleanup_canonical_same");
pub static CLEANUP_CANONICAL_BOUNDARY: Bucket = Bucket::new("cleanup_canonical_boundary");
pub static CLEANUP_CANONICAL_REFUSED: Bucket = Bucket::new("cleanup_canonical_refused");
pub static CLEANUP_CANONICAL_UNPROBED: Bucket = Bucket::new("cleanup_canonical_unprobed");
pub static CLEANUP_CANONICAL_PUSHED: Bucket = Bucket::new("cleanup_canonical_pushed");
pub static CLEANUP_SHAPED_PUSHED: Bucket = Bucket::new("cleanup_shaped_pushed");
pub static CLEANUP_SHAPED_LOST: Bucket = Bucket::new("cleanup_shaped_lost");
pub static CLEANUP_SHAPED_REFUSED: Bucket = Bucket::new("cleanup_shaped_refused");
pub static RENAME_CANDIDATES: Bucket = Bucket::new("rename_candidates");
pub static IDIOM_CANDIDATES: Bucket = Bucket::new("idiom_candidates");
pub static IDIOM_WON: Bucket = Bucket::new("idiom_won");
pub static IDIOM_LOST: Bucket = Bucket::new("idiom_lost");
pub static IDIOM_IDLE: Bucket = Bucket::new("idiom_idle");
pub static RENAME_STARVED: Bucket = Bucket::new("rename_starved");
pub static RENAME_IDLE: Bucket = Bucket::new("rename_idle");
pub static RENAME_UNPARSED: Bucket = Bucket::new("rename_unparsed");
pub static RENAME_REFUSED: Bucket = Bucket::new("rename_refused");
pub static RENAME_UNPROBED: Bucket = Bucket::new("rename_unprobed");
pub static RENAME_WON: Bucket = Bucket::new("rename_won");
pub static RENAME_LOST: Bucket = Bucket::new("rename_lost");
/// The pass's own exits: a template literal (sum: how many), an unsound scope
/// (sum: how many), and a resolution that is not total only because a
/// function declares a name twice -- the case the pass proceeds through.
pub static RENAME_TEMPLATED: Bucket = Bucket::new("rename_templated");
pub static RENAME_UNSOUND: Bucket = Bucket::new("rename_unsound");
pub static RENAME_AMBIGUOUS: Bucket = Bucket::new("rename_ambiguous");

const BYTE_BUCKETS: [&Bucket; 16] = [
    &ADMISSION,
    &DIRECT_VALIDATE,
    &PROBE_DROPPED,
    &CODEC,
    &ANALYZE,
    &EMIT,
    &PEEPHOLE,
    &CLEANUP,
    &OPTIMIZE,
    &LEX,
    &CLOSERS,
    &REGIONS,
    &SCOPES,
    &BINDINGS,
    &IDLE_FOLD,
    &ACTIVE_FOLD,
];
const ITERATION_BUCKETS: [&Bucket; 2] = [&SCALAR_FIXPOINT, &INLINE_FIXPOINT];
/// Deterministic event counters, reported as `<name>` (events) and
/// `<name>_sum`.
const EVENT_BUCKETS: [&Bucket; 23] = [
    &CLEANUP_ENTERED,
    &CLEANUP_UNBUDGETED,
    &CLEANUP_SKIPPED,
    &CLEANUP_CANONICAL_ERR,
    &CLEANUP_CANONICAL_SAME,
    &CLEANUP_CANONICAL_BOUNDARY,
    &CLEANUP_CANONICAL_REFUSED,
    &CLEANUP_CANONICAL_UNPROBED,
    &CLEANUP_CANONICAL_PUSHED,
    &CLEANUP_SHAPED_PUSHED,
    &CLEANUP_SHAPED_LOST,
    &CLEANUP_SHAPED_REFUSED,
    &RENAME_CANDIDATES,
    &RENAME_STARVED,
    &RENAME_IDLE,
    &RENAME_UNPARSED,
    &RENAME_REFUSED,
    &RENAME_UNPROBED,
    &RENAME_WON,
    &RENAME_LOST,
    &RENAME_TEMPLATED,
    &RENAME_UNSOUND,
    &RENAME_AMBIGUOUS,
];

/// `true` when the caller asked for a telemetry dump. Checked once; the
/// environment probe is far more expensive than the atomics it guards.
pub fn enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("LILSCRIPT_TIMING").is_some())
}

/// Per-fold accounting for the peephole pipeline, keyed by the fold's Rust type
/// name. The pipeline runs ~135 folds over the whole artifact and most of them
/// rewrite nothing on any given program, so knowing *which* ones burn the idle
/// time is what makes guarding them tractable.
static FOLD_PROFILE: std::sync::Mutex<Option<std::collections::BTreeMap<&'static str, FoldStats>>> =
    std::sync::Mutex::new(None);

#[derive(Debug, Clone, Copy, Default)]
pub struct FoldStats {
    pub calls: u64,
    pub idle_calls: u64,
    pub nanos: u64,
    pub idle_nanos: u64,
}

pub fn record_fold(name: &'static str, idle: bool, elapsed_nanos: u64) {
    if !enabled() {
        return;
    }
    let Ok(mut profile) = FOLD_PROFILE.lock() else {
        return;
    };
    let entry = profile
        .get_or_insert_with(std::collections::BTreeMap::new)
        .entry(name)
        .or_default();
    entry.calls += 1;
    entry.nanos += elapsed_nanos;
    if idle {
        entry.idle_calls += 1;
        entry.idle_nanos += elapsed_nanos;
    }
}

/// The folds that spent the most time proving they had nothing to do, worst
/// first. `None` when telemetry is off.
pub fn idle_fold_report(limit: usize) -> Option<String> {
    let profile = FOLD_PROFILE.lock().ok()?;
    let profile = profile.as_ref()?;
    let mut rows = profile.iter().collect::<Vec<_>>();
    rows.sort_by(|left, right| right.1.idle_nanos.cmp(&left.1.idle_nanos));
    let mut out = String::new();
    for (name, stats) in rows.into_iter().take(limit) {
        out.push_str(&format!(
            "{:>9.1}ms idle  {:>5}/{:<5} idle/calls  {}
",
            stats.idle_nanos as f64 / 1.0e6,
            stats.idle_calls,
            stats.calls,
            name.rsplit("::").next().unwrap_or(name),
        ));
    }
    Some(out)
}

/// Render the telemetry as one JSON object, or `None` when the caller did not
/// ask for it. CPU columns are summed across Rayon workers, so they can exceed
/// wall clock on a parallel search.
pub fn report(wall_nanos: u128) -> Option<String> {
    if !enabled() {
        return None;
    }
    let mut out = format!(r#"{{"wall_ms":{:.1}"#, wall_nanos as f64 / 1.0e6);
    for bucket in BYTE_BUCKETS {
        let (nanos, calls, bytes) = bucket.snapshot();
        out.push_str(&format!(
            r#","{name}_ms":{ms:.1},"{name}_calls":{calls},"{name}_mb":{mb:.2}"#,
            name = bucket.name,
            ms = nanos as f64 / 1.0e6,
            mb = bytes as f64 / (1024.0 * 1024.0),
        ));
    }
    for bucket in ITERATION_BUCKETS {
        let (nanos, calls, worst) = bucket.snapshot();
        out.push_str(&format!(
            r#","{name}_ms":{ms:.1},"{name}_runs":{calls},"{name}_max":{worst}"#,
            name = bucket.name,
            ms = nanos as f64 / 1.0e6,
        ));
    }
    for bucket in EVENT_BUCKETS {
        let (_, calls, sum) = bucket.snapshot();
        out.push_str(&format!(
            r#","{name}":{calls},"{name}_sum":{sum}"#,
            name = bucket.name,
        ));
    }
    let [(codec_hits, _), (analyze_hits, _)] = crate::artifact_memo::statistics();
    out.push_str(&format!(
        r#","memo_codec_hits":{codec_hits},"memo_analyze_hits":{analyze_hits}}}"#
    ));
    Some(out)
}
