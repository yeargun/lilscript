//! Representation families shared by source recipes and target formation.
//!
//! A family owns stable sites, offered alternatives and permission provenance.
//! Typed proof producers still establish legality. Estimates schedule work;
//! the single artifact admission/codec path decides which complete bytes win.
#[path = "representation_authors.rs"]
mod authors;
pub use authors::RegionalChoices;

use crate::compilation_policy::{
    AdmissionError, ResolvedPolicy, RuntimeRisk, TacticId, TacticInvalidation, TacticProducer,
    TacticUse,
};
use std::sync::Arc;

/// A family of choices. Each owns the meaning of its sites' identities and
/// its alternatives' ids. M9.5–M9.9 add theirs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChoiceFamily {
    RecordLayout,
    ProductLayout,
    InlineBody,
    CallLayout,
    StringLiteral,
    SharedString,
    ExpressionInlining,
    ConstantArguments,
    HelperSharing,
    ParameterizedHelpers,
    DataEncoding,
    NameAllocation,
    PropertyNames,
    ConditionalValues,
    ExitPoints,
    LoopFusion,
    ConditionalReturns,
    LogicalBranches,
    LoopHeads,
    LogicalStatements,
    CompoundAssignments,
    QuoteDelimiter,
    DeclarationOrder,
    ReceiverAlias,
    OptionalChain,
    LogicalAssignment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    SourceRecipe,
    TargetSite,
    Naming,
    TargetHead,
}

/// Metadata shared by discovery, explicit assignments and replay admission.
/// Invalidation comes from the existing tactic registry, never a second list.
#[derive(Debug, Clone, Copy)]
pub struct FamilySpec {
    pub family: ChoiceFamily,
    pub stage: Stage,
    pub tactic: TacticId,
    pub risk: RuntimeRisk,
    pub alternatives: u8,
}
impl FamilySpec {
    pub fn invalidates(self) -> &'static [TacticInvalidation] {
        self.tactic.spec().invalidates
    }
    pub fn producer(self) -> TacticProducer {
        match (self.stage, self.family) {
            (Stage::Naming, _) => TacticProducer::Naming,
            (Stage::SourceRecipe, _) => TacticProducer::StructuralSearch,
            (_, ChoiceFamily::DataEncoding) => TacticProducer::JavaScriptFormation,
            _ => TacticProducer::OutputFamilies,
        }
    }
    pub fn enabled(self, policy: &ResolvedPolicy) -> bool {
        let producer = self.producer();
        self.tactic.spec().producer_enabled(producer, policy)
            && policy.check_tactic_permissions(&[self.usage()]).is_ok()
    }
    pub const fn usage(self) -> TacticUse {
        TacticUse {
            tactic: self.tactic,
            risk: self.risk,
        }
    }
    pub fn check(self, policy: &ResolvedPolicy) -> Result<(), AdmissionError> {
        if !self.tactic.spec().producer_enabled(self.producer(), policy) {
            return Err(AdmissionError::ForbiddenTactic(self.tactic));
        }
        policy.check_tactic_permissions(&[self.usage()])
    }
}
impl ChoiceFamily {
    pub const ALL: [Self; 26] = [
        Self::RecordLayout,
        Self::ProductLayout,
        Self::InlineBody,
        Self::CallLayout,
        Self::StringLiteral,
        Self::SharedString,
        Self::ExpressionInlining,
        Self::ConstantArguments,
        Self::HelperSharing,
        Self::ParameterizedHelpers,
        Self::DataEncoding,
        Self::NameAllocation,
        Self::PropertyNames,
        Self::ConditionalValues,
        Self::ExitPoints,
        Self::LoopFusion,
        Self::ConditionalReturns,
        Self::LogicalBranches,
        Self::LoopHeads,
        Self::LogicalStatements,
        Self::CompoundAssignments,
        Self::QuoteDelimiter,
        Self::DeclarationOrder,
        Self::ReceiverAlias,
        Self::OptionalChain,
        Self::LogicalAssignment,
    ];
    pub const fn spec(self) -> FamilySpec {
        use ChoiceFamily as F;
        use TacticId as T;
        let (stage, tactic, risk, alternatives) = match self {
            F::RecordLayout | F::ProductLayout => (
                Stage::SourceRecipe,
                T::ScalarReplacement,
                RuntimeRisk::Neutral,
                2,
            ),
            F::InlineBody => (Stage::SourceRecipe, T::Inlining, RuntimeRisk::Neutral, 2),
            F::CallLayout => (
                Stage::SourceRecipe,
                T::CallSpecialization,
                RuntimeRisk::Neutral,
                2,
            ),
            F::StringLiteral => (
                Stage::SourceRecipe,
                T::ConstantFolding,
                RuntimeRisk::Neutral,
                2,
            ),
            F::SharedString => (
                Stage::SourceRecipe,
                T::StringPooling,
                RuntimeRisk::Neutral,
                2,
            ),
            F::ExpressionInlining => (Stage::TargetSite, T::Inlining, RuntimeRisk::Neutral, 2),
            F::ConstantArguments => (
                Stage::TargetSite,
                T::CallSpecialization,
                RuntimeRisk::Neutral,
                2,
            ),
            F::HelperSharing => (Stage::TargetSite, T::HelperSharing, RuntimeRisk::Neutral, 2),
            F::ParameterizedHelpers => (
                Stage::TargetSite,
                T::HelperSharing,
                RuntimeRisk::Recurring,
                2,
            ),
            F::ConditionalValues | F::ExitPoints | F::LoopFusion | F::ConditionalReturns
            | F::LogicalBranches | F::LoopHeads | F::LogicalStatements
            | F::CompoundAssignments | F::QuoteDelimiter | F::OptionalChain | F::LogicalAssignment =>
                (Stage::TargetSite, T::StatementSpellings, RuntimeRisk::Neutral, 2),
            F::DeclarationOrder => (Stage::TargetSite, T::DeclarationOrder, RuntimeRisk::Neutral, 2),
            F::ReceiverAlias => (Stage::TargetSite, T::ReceiverAliases, RuntimeRisk::Neutral, 2),
            F::PropertyNames => (Stage::TargetHead, T::PropertyMangling, RuntimeRisk::Neutral, 2),
            F::NameAllocation => (Stage::Naming, T::NamingCompaction, RuntimeRisk::Neutral, 2),
            F::DataEncoding => (
                Stage::TargetSite,
                T::StartupReconstruction,
                RuntimeRisk::Startup,
                4,
            ),
        };
        FamilySpec {
            family: self,
            stage,
            tactic,
            risk,
            alternatives,
        }
    }
}

/// One site of one family, by an identity formation keeps through every edit
/// of the tree and across every formation of one candidate: for data, the
/// root binding the table initializes (never an expression id, which
/// formation renumbers).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChoiceKey {
    pub family: ChoiceFamily,
    pub site: SiteId,
}

/// A site's identity (M4.4): the source symbol its binding declares, or, for
/// a binding formation creates, the binding's ordinal in the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SiteId {
    Symbol(u32),
    Formed(u32),
    Cell(u32),
    Unit(u32),
    Value { unit: u32, value: u32 },
    /// Target identity scoped to a formed head. Expressions carry this id
    /// through arena renumbering; regions/functions retain their arena ids.
    Target { head: u8, kind: u8, ordinal: u32 },
}

/// One alternative of a site, named by its family. `AltId(0)` is the
/// family's canonical form (for data, the literal as written).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AltId(pub u8);

/// An immutable assignment of alternatives to sites, sorted by key and shared
/// between the artifacts that hold it. A site it does not name takes its seed.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChoiceMap {
    entries: Option<Arc<[(ChoiceKey, AltId)]>>,
    joint: bool,
}

impl ChoiceMap {
    /// Every site at its seed.
    pub const SEEDS: Self = Self {
        entries: None,
        joint: false,
    };

    pub fn get(&self, key: ChoiceKey) -> Option<AltId> {
        let entries = self.entries.as_deref()?;
        entries
            .binary_search_by_key(&key, |(key, _)| *key)
            .ok()
            .map(|index| entries[index].1)
    }

    /// This map with `key` assigned `alternative`: a new map; this one and
    /// every artifact holding it are unchanged.
    pub fn with(&self, key: ChoiceKey, alternative: AltId) -> Self {
        self.with_all([(key, alternative)])
    }

    /// Apply one joint assignment with one allocation/sort instead of cloning
    /// a growing map once per site. Later assignments override earlier ones.
    pub fn with_all(&self, changes: impl IntoIterator<Item = (ChoiceKey, AltId)>) -> Self {
        let mut entries: Vec<_> = self.iter().chain(changes).collect();
        entries.sort_by_key(|(key, _)| *key);
        entries.dedup_by(|later, earlier| {
            if later.0 == earlier.0 {
                earlier.1 = later.1;
                true
            } else {
                false
            }
        });
        Self {
            entries: (!entries.is_empty()).then(|| entries.into()),
            joint: self.joint,
        }
    }

    pub fn retaining(&self, keep: impl Fn(ChoiceKey) -> bool) -> Self {
        let mut result = Self::SEEDS.with_all(self.iter().filter(|(key, _)| keep(*key)));
        result.joint = self.joint;
        result
    }

    pub fn iter(&self) -> impl Iterator<Item = (ChoiceKey, AltId)> + '_ {
        self.entries.as_deref().unwrap_or(&[]).iter().copied()
    }

    /// Validate every explicit assignment independently of default flags.
    /// Site existence/legality is checked by formation under this candidate's
    /// semantic identity; an inactive site is harmless, never a proof.
    pub fn from_joint(mut self) -> Self {
        self.joint = true;
        self
    }

    pub fn uses(&self) -> impl Iterator<Item = TacticUse> + '_ {
        self.iter()
            .filter(|(_, alt)| *alt != AltId(0))
            .map(|(key, _)| key.family.spec().usage())
            .chain(self.joint.then_some(TacticUse {
                tactic: TacticId::RepresentationJoints,
                risk: RuntimeRisk::Neutral,
            }))
    }

    pub fn check_target_policy(&self, policy: &ResolvedPolicy) -> Result<(), AdmissionError> {
        if self.joint {
            policy.check_tactic_permissions(&[TacticUse {
                tactic: TacticId::RepresentationJoints,
                risk: RuntimeRisk::Neutral,
            }])?;
        }
        for (key, alternative) in self.iter() {
            let spec = key.family.spec();
            if spec.stage != Stage::TargetSite
                || !matches!(key.site, SiteId::Symbol(_) | SiteId::Formed(_) | SiteId::Target { .. })
                || alternative.0 >= spec.alternatives
            {
                return Err(AdmissionError::Constraint(
                    "invalid target representation assignment",
                ));
            }
            if alternative != AltId(0) {
                spec.check(policy)?;
            }
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        !self.joint && self.entries.as_deref().is_none_or(<[_]>::is_empty)
    }

    pub fn len(&self) -> usize {
        self.entries.as_deref().map_or(0, <[_]>::len)
    }

    /// Heap bytes an artifact holding this map is charged for.
    pub fn retained_bytes(&self) -> usize {
        self.len() * std::mem::size_of::<(ChoiceKey, AltId)>()
    }
}

/// One alternative of a discovered site: its id, its name in reports, and
/// the estimator's raw bytes saved against the canonical form (positive is smaller).
/// The estimate orders the codec's work; it never decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// Authored sites are fixed under every search move.
    pub pinned: bool,
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

/// Deterministic finite moves over any registered family's offered sites.
/// Raw estimates rank but never suppress a legal alternative. Canonical joint
/// rollback is retained from the old data scheduler. Additional family/pair
/// moves have a fixed bound and require the representation-joints permission.
pub fn schedule(sites: &[ChoiceSite], extra_joints: bool) -> Vec<Vec<(usize, AltId)>> {
    let canonical = AltId(0);
    let mut order: Vec<usize> = (0..sites.len()).filter(|&i| !sites[i].pinned).collect();
    order.sort_by(|&a, &b| {
        sites[b]
            .stake()
            .cmp(&sites[a].stake())
            .then(sites[a].key.cmp(&sites[b].key))
    });
    let mut moves = Vec::new();
    let rollback: Vec<_> = order
        .iter()
        .copied()
        .filter(|&site| sites[site].applied != canonical)
        .map(|site| (site, canonical))
        .collect();
    if rollback.len() > 1 {
        moves.push(rollback);
    }
    let mut best = Vec::new();
    for &site in &order {
        let mut alternatives: Vec<_> = sites[site]
            .alternatives
            .iter()
            .filter(|offered| offered.alternative != sites[site].applied)
            .collect();
        alternatives.sort_by(|a, b| {
            b.saving
                .cmp(&a.saving)
                .then(a.alternative.cmp(&b.alternative))
        });
        if let Some(offered) = alternatives.first() {
            best.push((site, offered.alternative));
        }
        moves.extend(
            alternatives
                .into_iter()
                .map(|offered| vec![(site, offered.alternative)]),
        );
    }
    if extra_joints {
        // Shared decoder/body overhead can make all sites of one family pay
        // together. A bounded set of leading cross-family pairs exposes
        // specialization/inlining/sharing interactions without a powerset.
        for family in ChoiceFamily::ALL {
            let group: Vec<_> = best
                .iter()
                .copied()
                .filter(|&(site, _)| sites[site].key.family == family)
                .collect();
            if group.len() > 1 && !moves.contains(&group) {
                moves.push(group);
            }
        }
        let limit = best.len().min(4);
        for a in 0..limit {
            for b in a + 1..limit {
                let pair = vec![best[a], best[b]];
                if !moves.contains(&pair) {
                    moves.push(pair);
                }
            }
        }
        if limit > 2 {
            let group = best[..limit].to_vec();
            if !moves.contains(&group) {
                moves.push(group);
            }
        }
    }
    moves
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn q1_joint_map_is_atomic_stable_and_keeps_search_provenance() {
        let prior = ChoiceMap::SEEDS.with(key(3), AltId(1));
        let joined = prior
            .with_all([(key(1), AltId(1)), (key(3), AltId(0)), (key(1), AltId(0))])
            .from_joint();
        assert_eq!(
            joined.iter().collect::<Vec<_>>(),
            [(key(1), AltId(0)), (key(3), AltId(0))]
        );
        assert_eq!(prior.get(key(3)), Some(AltId(1)));
        assert!(joined
            .uses()
            .any(|usage| usage.tactic == TacticId::RepresentationJoints));
        assert!(joined
            .retaining(|_| false)
            .uses()
            .any(|usage| usage.tactic == TacticId::RepresentationJoints));
    }

    #[test]
    fn q1_schedule_keeps_negative_estimates_and_bounded_cross_family_moves() {
        let site = |index, family| ChoiceSite {
            pinned: false,
            key: ChoiceKey {
                family,
                site: SiteId::Symbol(index),
            },
            name: format!("f{index}"),
            seed: AltId(0),
            applied: AltId(0),
            alternatives: vec![
                ChoiceAlternative {
                    alternative: AltId(0),
                    name: "retained",
                    saving: 0,
                },
                ChoiceAlternative {
                    alternative: AltId(1),
                    name: "changed",
                    saving: -10,
                },
            ],
        };
        let sites = [
            site(0, ChoiceFamily::ConstantArguments),
            site(1, ChoiceFamily::ExpressionInlining),
            site(2, ChoiceFamily::HelperSharing),
        ];
        let single = schedule(&sites, false);
        let joint = schedule(&sites, true);
        assert_eq!(single.len(), 3);
        assert_eq!(&joint[..single.len()], &single);
        assert!(joint.iter().any(|moves| moves.len() == 2
            && moves.contains(&(0, AltId(1)))
            && moves.contains(&(1, AltId(1)))));
        assert!(joint.iter().any(
            |moves| moves.len() == 3 && (0..3).all(|index| moves.contains(&(index, AltId(1))))
        ));
        assert!(joint.len() <= single.len() + ChoiceFamily::ALL.len() + 7);
    }

    const fn key(site: u32) -> ChoiceKey {
        ChoiceKey {
            family: ChoiceFamily::DataEncoding,
            site: SiteId::Symbol(site),
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
