//! Content-addressed memoization for the two whole-artifact primitives.
//!
//! Compressor-in-loop selection scores candidates by canonically encoding the
//! *entire* artifact and by re-validating the *entire* generated program. Both
//! are pure functions of their input bytes, and the search re-visits identical
//! byte strings constantly: independent structural plans normalize to the same
//! emission, terminal families re-measure an incumbent they already scored, and
//! a rejected neighborhood step restores bytes that were measured one step
//! earlier.
//!
//! Hashing an artifact costs microseconds; a canonical Brotli quality-11 encode
//! of the same bytes costs tens of milliseconds. Keying on a SHA-256 digest is
//! therefore a pure reduction in work that cannot change a selection: a hit
//! returns exactly the value the primitive would have recomputed.

use sha2::{Digest as _, Sha256};
use std::collections::HashMap;
use std::sync::Mutex;

pub type ContentDigest = [u8; 32];

/// Entries retained per memo before it is dropped wholesale. Selection is
/// unaffected by eviction — a miss recomputes the same value — so the cheapest
/// bounded policy is the right one.
const CAPACITY: usize = 1 << 15;

/// A/B escape hatch. The memo cannot change a selection, so this exists only to
/// measure what it saves; leaving it unset is the supported configuration.
pub fn enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("LILSCRIPT_NO_MEMO").is_none())
}

pub fn content_digest(bytes: &[u8]) -> ContentDigest {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

pub struct ArtifactMemo<K, V> {
    entries: std::sync::OnceLock<Mutex<HashMap<K, V>>>,
    hits: std::sync::atomic::AtomicU64,
    misses: std::sync::atomic::AtomicU64,
}

impl<K, V> ArtifactMemo<K, V>
where
    K: std::hash::Hash + Eq,
    V: Clone,
{
    pub const fn new() -> Self {
        Self {
            entries: std::sync::OnceLock::new(),
            hits: std::sync::atomic::AtomicU64::new(0),
            misses: std::sync::atomic::AtomicU64::new(0),
        }
    }

    fn entries(&self) -> &Mutex<HashMap<K, V>> {
        self.entries.get_or_init(|| Mutex::new(HashMap::new()))
    }

    pub fn get(&self, key: &K) -> Option<V> {
        if !enabled() {
            return None;
        }
        let found = self
            .entries()
            .lock()
            .ok()
            .and_then(|entries| entries.get(key).cloned());
        let counter = if found.is_some() {
            &self.hits
        } else {
            &self.misses
        };
        counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        found
    }

    pub fn insert(&self, key: K, value: V) {
        if !enabled() {
            return;
        }
        let Ok(mut entries) = self.entries().lock() else {
            return;
        };
        if entries.len() >= CAPACITY {
            entries.clear();
        }
        entries.insert(key, value);
    }

    pub fn statistics(&self) -> (u64, u64) {
        (
            self.hits.load(std::sync::atomic::Ordering::Relaxed),
            self.misses.load(std::sync::atomic::Ordering::Relaxed),
        )
    }
}

impl<K, V> Default for ArtifactMemo<K, V>
where
    K: std::hash::Hash + Eq,
    V: Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

/// Canonical transfer size, keyed by artifact digest and cost model.
pub static COMPRESSED_SIZE: ArtifactMemo<(ContentDigest, u8), usize> = ArtifactMemo::new();

/// Generated-JavaScript syntax metrics, keyed by artifact digest. Only
/// successful analyses are retained: a rejection is fatal and never repeated
/// enough times to be worth caching, and its diagnostic borrows the source.
pub static GENERATED_ANALYSIS: ArtifactMemo<
    ContentDigest,
    crate::js_peephole::JavaScriptSyntaxMetrics,
> = ArtifactMemo::new();

/// Folds already known to leave a given artifact untouched.
///
/// Every peephole fold is a pure text transform reached through a plain
/// function item, so `(fold identity, input bytes)` fully determines its
/// answer. The pipeline runs several folds four or five times each, and 82% of
/// invocations rewrite nothing, so a fold that already declined these exact
/// bytes can be skipped without running it. Only proven no-ops are recorded:
/// an entry is written after checking the fold both reported zero rewrites and
/// returned byte-identical output.
pub static DECLINED_FOLDS: ArtifactMemo<(&'static str, ContentDigest), ()> = ArtifactMemo::new();

/// `(hits, misses)` for both memos, in report order.
pub fn statistics() -> [(u64, u64); 2] {
    [
        COMPRESSED_SIZE.statistics(),
        GENERATED_ANALYSIS.statistics(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memo_returns_the_stored_value_and_counts_hits() {
        let memo: ArtifactMemo<ContentDigest, usize> = ArtifactMemo::new();
        let key = content_digest(b"const a=1;");
        assert_eq!(memo.get(&key), None);
        memo.insert(key, 7);
        assert_eq!(memo.get(&key), Some(7));
        assert_eq!(memo.statistics(), (1, 1));
    }

    #[test]
    fn distinct_bytes_hash_to_distinct_keys() {
        assert_ne!(content_digest(b"var a=1"), content_digest(b"var a=2"));
    }

    #[test]
    fn eviction_keeps_the_memo_bounded_without_changing_answers() {
        let memo: ArtifactMemo<usize, usize> = ArtifactMemo::new();
        for index in 0..=CAPACITY {
            memo.insert(index, index);
        }
        assert!(memo.entries().lock().unwrap().len() <= CAPACITY);
        assert_eq!(memo.get(&CAPACITY), Some(CAPACITY));
    }
}
