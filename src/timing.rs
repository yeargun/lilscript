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

/// Iteration-count buckets. `calls` is the number of times the loop was
/// entered and `bytes` holds the worst single call's iteration count.
pub static SCALAR_FIXPOINT: Bucket = Bucket::new("scalar_fixpoint");
pub static INLINE_FIXPOINT: Bucket = Bucket::new("inline_fixpoint");

const BYTE_BUCKETS: [&Bucket; 14] = [
    &ADMISSION,
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
    let [(codec_hits, _), (analyze_hits, _)] = crate::artifact_memo::statistics();
    out.push_str(&format!(
        r#","memo_codec_hits":{codec_hits},"memo_analyze_hits":{analyze_hits}}}"#
    ));
    Some(out)
}
