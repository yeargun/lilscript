//! Choices (architecture §9; plan M9.1, first slice): representation
//! alternatives at one *site* of the program, whose value depends on the
//! objective or on the rest of the artifact, never on meaning.
//!
//! A family discovers its sites while formation runs, names each by a stable
//! identity (`ChoiceKey`), lists its alternatives with an estimate of what each
//! saves, and applies the alternative the artifact's `ChoiceMap` names, or the
//! site's seed when the map names none. The map is part of the artifact's
//! output assignment (next to `OutputFamilies`), so a decision is data on the
//! tree the printer renders (live-16), and the terminal stage
//! (`program::search_terminal`) offers other assignments as declared
//! challengers, each kept only when the exact codec says the whole artifact
//! shrank (L7). Estimators order that work; codecs decide (L8).
//!
//! Prior art: Terser picks between two spellings of one node by printed size
//! (`best_of_expression`, `terser@8fa44c8 lib/compress/common.js:170`), a
//! local judge without a codec; the old route's `DecisionRegistry`
//! (`d362338f:src/decision_registry.rs`) scored whole-program flags, never a
//! site. Neither Closure, esbuild nor Oxc has per-site alternatives.
use std::sync::Arc;

/// A family of choices. Each owns the meaning of its sites' identities and
/// its alternatives' ids. M9.5–M9.9 add theirs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChoiceFamily {
    /// How a constant data literal is spelled (M9.8, `tables.rs`).
    DataEncoding,
}

/// One site of one family, by an identity formation keeps through every edit
/// of the tree and across every formation of one candidate: for data, the
/// root binding the table initializes (never an expression id, which
/// formation renumbers).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChoiceKey {
    pub family: ChoiceFamily,
    pub site: u32,
}

/// One alternative of a site, named by its family. `AltId(0)` is the
/// family's canonical form (for data, the literal as written).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AltId(pub u8);

/// An immutable assignment of alternatives to sites, sorted by key and shared
/// between the artifacts that hold it. A site it does not name takes its seed.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChoiceMap(Option<Arc<[(ChoiceKey, AltId)]>>);

impl ChoiceMap {
    /// Every site at its seed.
    pub const SEEDS: Self = Self(None);

    pub fn get(&self, key: ChoiceKey) -> Option<AltId> {
        let entries = self.0.as_deref()?;
        entries
            .binary_search_by_key(&key, |(key, _)| *key)
            .ok()
            .map(|index| entries[index].1)
    }

    /// This map with `key` assigned `alternative`: a new map; this one and
    /// every artifact holding it are unchanged.
    pub fn with(&self, key: ChoiceKey, alternative: AltId) -> Self {
        let mut entries: Vec<(ChoiceKey, AltId)> = self.iter().collect();
        match entries.binary_search_by_key(&key, |(key, _)| *key) {
            Ok(index) => entries[index].1 = alternative,
            Err(index) => entries.insert(index, (key, alternative)),
        }
        Self(Some(entries.into()))
    }

    pub fn iter(&self) -> impl Iterator<Item = (ChoiceKey, AltId)> + '_ {
        self.0.as_deref().unwrap_or(&[]).iter().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.0.as_deref().is_none_or(<[_]>::is_empty)
    }

    pub fn len(&self) -> usize {
        self.0.as_deref().map_or(0, <[_]>::len)
    }

    /// Heap bytes an artifact holding this map is charged for.
    pub fn retained_bytes(&self) -> usize {
        self.len() * std::mem::size_of::<(ChoiceKey, AltId)>()
    }
}

/// One alternative of a discovered site: its id, its name in reports, and
/// the estimator's raw bytes saved against the literal (positive is smaller).
/// The estimate orders the codec's work; it never decides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChoiceAlternative {
    pub alternative: AltId,
    pub name: &'static str,
    pub saving: i64,
}

/// A site formation found, as it found it: what it offers, what it seeds and
/// what the artifact's map made it apply. Formation records every site on
/// the tree it formed, so the terminal stage can ask what one artifact
/// offers without re-deriving it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChoiceSite {
    pub key: ChoiceKey,
    /// The site's source name, for reports.
    pub name: String,
    /// Every alternative, the canonical one first.
    pub alternatives: Vec<ChoiceAlternative>,
    /// The alternative tried first: the estimator's best when it saves
    /// anything, the canonical form otherwise.
    pub seed: AltId,
    /// The alternative formation applied.
    pub applied: AltId,
}

impl ChoiceSite {
    pub fn offers(&self, alternative: AltId) -> bool {
        self.alternatives
            .iter()
            .any(|offered| offered.alternative == alternative)
    }

    pub fn name_of(&self, alternative: AltId) -> &'static str {
        self.alternatives
            .iter()
            .find(|offered| offered.alternative == alternative)
            .map_or("unknown", |offered| offered.name)
    }

    /// The largest saving any alternative estimates: how much the site is
    /// worth judging.
    pub fn stake(&self) -> i64 {
        self.alternatives
            .iter()
            .map(|offered| offered.saving.abs())
            .max()
            .unwrap_or(0)
    }
}

/// The seed among `alternatives`: the largest estimated saving above zero,
/// the lowest id on a tie, else the canonical form.
pub fn seed(alternatives: &[ChoiceAlternative]) -> AltId {
    alternatives
        .iter()
        .filter(|offered| offered.saving > 0)
        .max_by(|a, b| {
            a.saving
                .cmp(&b.saving)
                .then(b.alternative.cmp(&a.alternative))
        })
        .map_or(AltId(0), |best| best.alternative)
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn key(site: u32) -> ChoiceKey {
        ChoiceKey {
            family: ChoiceFamily::DataEncoding,
            site,
        }
    }

    #[test]
    fn a_map_is_immutable_and_sorted() {
        let empty = ChoiceMap::SEEDS;
        assert!(empty.is_empty());
        assert_eq!(empty.get(key(3)), None);
        let one = empty.with(key(3), AltId(2));
        let two = one.with(key(1), AltId(1));
        let changed = two.with(key(3), AltId(0));
        assert_eq!(empty, ChoiceMap::SEEDS);
        assert_eq!(one.get(key(3)), Some(AltId(2)));
        assert_eq!(one.get(key(1)), None);
        assert_eq!(
            two.iter().collect::<Vec<_>>(),
            [(key(1), AltId(1)), (key(3), AltId(2))]
        );
        assert_eq!(changed.get(key(3)), Some(AltId(0)));
        assert_eq!(two.get(key(3)), Some(AltId(2)));
        assert_eq!(changed.len(), 2);
    }

    #[test]
    fn the_seed_is_the_best_positive_estimate_or_the_canonical_form() {
        let offered = |alternative, saving| ChoiceAlternative {
            alternative: AltId(alternative),
            name: "",
            saving,
        };
        assert_eq!(seed(&[offered(0, 0), offered(1, -5)]), AltId(0));
        assert_eq!(
            seed(&[offered(0, 0), offered(1, 40), offered(2, 90)]),
            AltId(2)
        );
        assert_eq!(
            seed(&[offered(0, 0), offered(1, 90), offered(2, 90)]),
            AltId(1)
        );
    }
}
