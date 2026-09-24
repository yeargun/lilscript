//! Immutable checked rewrite choices, shared by snapshots and completed output.
//! Stamps bind a rule to its original snapshot; canonical order excludes stamps.
use super::{OpId, RevisionId, UnitId};
use crate::compilation_policy::{
    AdmissionError, BudgetLedger, ResolvedPolicy, RuntimeRisk, TacticId, TacticUse, WorkKind,
};
use crate::output_budget::{
    AllocationBudget, AllocationClass::Retained, AllocationError, RetainedCharge,
};
use crate::primitive::IntBinary;
use std::cmp::Ordering;
use std::mem::size_of;
use std::sync::Arc;

pub const LITERAL_INT_FOLD_VERSION: u32 = 1;

/// One exact checked rule application. Operands name actual literal producers,
/// not inferred values of mutable cells, parameters, aliases or host objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiteralIntFold {
    pub version: u32,
    pub unit: UnitId,
    pub operation: OpId,
    pub operator: IntBinary,
    pub left_producer: OpId,
    pub left: i32,
    pub right_producer: OpId,
    pub right: i32,
    pub result: i32,
}
impl LiteralIntFold {
    pub(super) fn words(self) -> [u32; 10] {
        [
            self.version,
            0,
            self.unit.index() as u32,
            self.operation.index() as u32,
            match self.operator {
                IntBinary::Add => 0,
                IntBinary::Subtract => 1,
                IntBinary::Multiply => 2,
                IntBinary::Divide => 3,
                IntBinary::Remainder => 4,
                IntBinary::UnsignedShiftRight => 5,
            },
            self.left_producer.index() as u32,
            self.left as u32,
            self.right_producer.index() as u32,
            self.right as u32,
            self.result as u32,
        ]
    }
}

pub const DEAD_VALUE_DROP_VERSION: u32 = 1;

/// The neutral constant a retired value becomes, chosen from its declared type
/// so the verifier's type check still holds: `0`, `+0`, `false`, `null` or
/// `undefined`. Object-typed values have no neutral constant and are not
/// retired this way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NeutralConstant {
    Integer,
    Number,
    Boolean,
    Null,
    Undefined,
}
impl NeutralConstant {
    fn word(self) -> u32 {
        match self {
            Self::Integer => 0,
            Self::Number => 1,
            Self::Boolean => 2,
            Self::Null => 3,
            Self::Undefined => 4,
        }
    }
}

/// One checked dead-value retirement. Every result of `operation` has no use,
/// and the facts snapshot proves evaluating it unobservable once its result is
/// discarded (no throw, divergence, reentry, suspension, control transfer or
/// write). It becomes a neutral constant and releases its operands, which may
/// in turn become dead: cleanup cascades one checked step at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeadValueDrop {
    pub version: u32,
    pub unit: UnitId,
    pub operation: OpId,
    pub replacement: NeutralConstant,
}

/// Every checked equivalent-rewrite rule. Each names the tactic it exercises,
/// so lineage reports exactly the permissions an artifact depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RewriteRule {
    LiteralIntFold(LiteralIntFold),
    DeadValueDrop(DeadValueDrop),
}
impl RewriteRule {
    /// Canonical words; word 1 is the rule tag, so two rules never collide.
    pub(super) fn words(self) -> [u32; 10] {
        match self {
            Self::LiteralIntFold(fold) => fold.words(),
            Self::DeadValueDrop(drop) => [
                drop.version,
                1,
                drop.unit.index() as u32,
                drop.operation.index() as u32,
                drop.replacement.word(),
                0,
                0,
                0,
                0,
                0,
            ],
        }
    }
    pub fn unit(self) -> UnitId {
        match self {
            Self::LiteralIntFold(fold) => fold.unit,
            Self::DeadValueDrop(drop) => drop.unit,
        }
    }
    pub fn operation(self) -> OpId {
        match self {
            Self::LiteralIntFold(fold) => fold.operation,
            Self::DeadValueDrop(drop) => drop.operation,
        }
    }
    pub fn tactic(self) -> TacticId {
        match self {
            Self::LiteralIntFold(_) => TacticId::ConstantFolding,
            Self::DeadValueDrop(_) => TacticId::DeadCodeElimination,
        }
    }
    pub fn literal_fold(self) -> Option<LiteralIntFold> {
        match self {
            Self::LiteralIntFold(fold) => Some(fold),
            Self::DeadValueDrop(_) => None,
        }
    }
    pub fn dead_value_drop(self) -> Option<DeadValueDrop> {
        match self {
            Self::DeadValueDrop(drop) => Some(drop),
            Self::LiteralIntFold(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedRewriteStep {
    /// Meaning under which this rule was checked. A later SourceChange keeps
    /// this as inherited permission history, not a replay claim for its new root.
    pub meaning: RevisionId,
    pub before: RevisionId,
    pub after: RevisionId,
    pub rule: RewriteRule,
}

#[derive(Debug)]
struct Node {
    owner: RevisionId,
    step: CheckedRewriteStep,
    previous: RewriteLineage,
    /// Every tactic this history exercised, in `TacticId` order, so policy
    /// checks see exactly the permissions the snapshot depends on.
    tactics: Vec<TacticUse>,
    count: usize,
    fingerprint: u64,
    retained_bytes: u64,
    charge: RetainedCharge<RevisionId>,
}

#[derive(Debug, Default)]
#[must_use = "discard through the admitting compilation"]
pub(super) struct RewriteLineage(Option<Arc<Node>>);

impl Drop for RewriteLineage {
    fn drop(&mut self) {
        // Abandoned compilations also drop arbitrarily long histories without
        // recursion. Explicit discard below additionally releases ledger charges.
        while let Some(node) = self.0.take() {
            let Ok(mut node) = Arc::try_unwrap(node) else {
                break;
            };
            self.0 = node.previous.0.take();
        }
    }
}

/// A borrowed newest-first history. Descriptions cannot clone retained owners.
#[derive(Debug, Clone, Copy, Default)]
pub struct RewriteDescription<'a>(Option<&'a Node>);
impl<'a> RewriteDescription<'a> {
    pub fn len(self) -> usize {
        self.0.map_or(0, |node| node.count)
    }
    pub fn is_empty(self) -> bool {
        self.0.is_none()
    }
    pub fn steps(self) -> impl Iterator<Item = CheckedRewriteStep> + 'a {
        std::iter::successors(self.0, |node| node.previous.0.as_deref()).map(|node| node.step)
    }
    pub(super) fn fingerprint(self) -> u64 {
        self.0.map_or(0, |node| node.fingerprint)
    }
    pub(super) fn compare(
        self,
        other: Self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Ordering, AllocationError> {
        budget.work(WorkKind::Analysis, 1)?;
        let order = self.len().cmp(&other.len());
        if order != Ordering::Equal {
            return Ok(order);
        }
        for (left, right) in self.steps().zip(other.steps()) {
            for (left, right) in left.rule.words().into_iter().zip(right.rule.words()) {
                budget.work(WorkKind::Analysis, 1)?;
                let order = left.cmp(&right);
                if order != Ordering::Equal {
                    return Ok(order);
                }
            }
        }
        Ok(Ordering::Equal)
    }
}

impl RewriteLineage {
    pub(super) fn description(&self) -> RewriteDescription<'_> {
        RewriteDescription(self.0.as_deref())
    }
    pub(super) fn retained_bytes(&self) -> u64 {
        self.0.as_ref().map_or(0, |node| node.retained_bytes)
    }
    pub(super) fn share(&self, budget: &mut AllocationBudget<'_>) -> Result<Self, AllocationError> {
        if self.0.is_some() {
            budget.work(WorkKind::Analysis, 1)?;
        }
        Ok(Self(self.0.as_ref().map(Arc::clone)))
    }
    pub(super) fn append(
        &self,
        owner: RevisionId,
        step: CheckedRewriteStep,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        let mut phase = budget.scope();
        phase.work(WorkKind::Analysis, 11)?;
        let count = self
            .description()
            .len()
            .checked_add(1)
            .ok_or(AllocationError::Capacity)?;
        let mut tactics = self.tactics().to_vec();
        let usage = TacticUse {
            tactic: step.rule.tactic(),
            risk: RuntimeRisk::Neutral,
        };
        if !tactics
            .iter()
            .any(|existing| existing.tactic == usage.tactic)
        {
            tactics.push(usage);
            tactics.sort_by_key(|usage| usage.tactic);
        }
        let bytes = (size_of::<Node>()
            + 2 * size_of::<usize>()
            + tactics.len() * size_of::<TacticUse>()) as u64;
        let retained_bytes = self
            .retained_bytes()
            .checked_add(bytes)
            .ok_or(AllocationError::Capacity)?;
        phase.retain(Retained, bytes)?;
        let mut fingerprint = self
            .0
            .as_ref()
            .map_or(0xcbf29ce484222325, |node| node.fingerprint);
        for word in step.rule.words() {
            for byte in word.to_le_bytes() {
                fingerprint = (fingerprint ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        let previous = self.share(&mut phase)?;
        let charge = phase
            .detach_retained(owner, bytes)
            .expect("admitted rewrite node");
        Ok(Self(Some(Arc::new(Node {
            owner,
            step,
            previous,
            tactics,
            count,
            fingerprint,
            retained_bytes,
            charge,
        }))))
    }
    /// The tactics this history exercised. Each rule contributes its own; an
    /// artifact that only dropped dead values does not depend on folding.
    pub(super) fn tactics(&self) -> &[TacticUse] {
        self.0.as_ref().map_or(&[], |node| node.tactics.as_slice())
    }
    pub(super) fn check_policy(&self, policy: &ResolvedPolicy) -> Result<(), AdmissionError> {
        for usage in self.tactics() {
            if !policy.tactic(usage.tactic).enabled {
                return Err(AdmissionError::ForbiddenTactic(usage.tactic));
            }
        }
        Ok(())
    }
    pub(super) fn discard(mut self, ledger: &mut BudgetLedger) {
        let owner = self.0.as_ref().map(|node| node.owner);
        // Iterative release also bounds stack depth for long accepted histories.
        while let Some(node) = self.0.take() {
            assert_eq!(Some(node.owner), owner, "rewrite lineage owner");
            let Ok(node) = Arc::try_unwrap(node) else {
                break;
            };
            self = node.previous;
            node.charge
                .discard(&node.owner, ledger)
                .expect("owned rewrite lineage charge");
        }
    }
}
