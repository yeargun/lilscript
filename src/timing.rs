//! Compile-effort telemetry.
//!
//! The compiler's phases and its canonical encodes repeat whole-artifact work,
//! so on a large library the wall clock is the product of "how expensive is one
//! pass" and "how many did the search buy". These buckets make the split visible
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

/// The one compiler's JavaScript and target phases. These scopes count
/// attempts, including refusal and unwind. Times are accumulated elapsed
/// durations, not process CPU. Naming includes lazy scoped-name preparation;
/// byte totals are not defined.
pub static JS_DEMAND: Bucket = Bucket::new("js_demand");
pub static JS_FORMATION: Bucket = Bucket::new("js_formation");
pub static TARGET_VERIFY: Bucket = Bucket::new("target_verify");
pub static TARGET_EDITION: Bucket = Bucket::new("target_edition");
pub static TARGET_BASIS: Bucket = Bucket::new("target_basis");
pub static TARGET_NAMES: Bucket = Bucket::new("target_names");
pub static TARGET_PRINT: Bucket = Bucket::new("target_print");
/// Admitted canonical encoder attempts; raw measurement is excluded.
pub static CANONICAL_GZIP: Bucket = Bucket::new("canonical_gzip");
pub static CANONICAL_BROTLI: Bucket = Bucket::new("canonical_brotli");

const PHASE_BUCKETS: [&Bucket; 9] = [
    &JS_DEMAND,
    &JS_FORMATION,
    &TARGET_VERIFY,
    &TARGET_EDITION,
    &TARGET_BASIS,
    &TARGET_NAMES,
    &TARGET_PRINT,
    &CANONICAL_GZIP,
    &CANONICAL_BROTLI,
];

/// `true` when the caller asked for a telemetry dump. Checked once; the
/// environment probe is far more expensive than the atomics it guards.
pub fn enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("LILSCRIPT_TIMING").is_some())
}

/// Render the telemetry as one JSON object, or `None` when the caller did not
/// ask for it. Durations accumulate elapsed scopes, so they are neither
/// additive wall time nor process CPU.
pub fn report(wall_nanos: u128) -> Option<String> {
    if !enabled() {
        return None;
    }
    let mut out = format!(r#"{{"wall_ms":{:.1}"#, wall_nanos as f64 / 1.0e6);
    for bucket in PHASE_BUCKETS {
        let (nanos, calls, _) = bucket.snapshot();
        out.push_str(&format!(
            r#","{name}_ms":{ms:.3},"{name}_calls":{calls}"#,
            name = bucket.name,
            ms = nanos as f64 / 1.0e6,
        ));
    }
    out.push('}');
    Some(out)
}

#[cfg(test)]
#[path = "timing_tests.rs"]
mod tests;
