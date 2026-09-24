//! Closed semantic rules. A caller selects a site, never a replacement or an
//! unchecked equivalence claim. Publication uses the existing patch transaction.
use super::*;
use crate::check::Type;
use crate::compilation_policy::AdmissionError;
use crate::program::facts::{Legality, ObservationDemand};
use crate::program::uses::{CellUse, CellUseSite, UseIndex, ValueUse};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RewriteError {
    Publication(PublicationError),
    ForbiddenTactic(TacticId),
    StaleProof,
    PolicyMismatch,
    InvalidProof,
    /// A rule that needs local facts was asked for while none are enabled.
    FactsUnavailable,
}
impl From<PublicationError> for RewriteError {
    fn from(error: PublicationError) -> Self {
        Self::Publication(error)
    }
}

/// One prepared rule application, bound to the exact snapshot, meaning, table
/// and unit revisions and the policy it was checked under. Commit re-derives it
/// and refuses anything that moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CheckedRewrite {
    base: SemanticId,
    snapshot: RevisionId,
    meaning: RevisionId,
    tables: RevisionId,
    unit_revision: RevisionId,
    policy_fingerprint: [u8; 32],
    rule: RewriteRule,
}
impl CheckedRewrite {
    pub(super) fn base(self) -> SemanticId {
        self.base
    }
    pub(super) fn unit_revision(self) -> RevisionId {
        self.unit_revision
    }
    pub(super) fn rule(self) -> RewriteRule {
        self.rule
    }
    /// The operation kind the rule publishes at its site.
    pub(super) fn replacement(self) -> OperationKind {
        OperationKind::Constant(match self.rule {
            RewriteRule::LiteralIntFold(fold) => Constant::Integer(fold.result),
            RewriteRule::DeadValueDrop(drop) => match drop.replacement {
                NeutralConstant::Integer => Constant::Integer(0),
                NeutralConstant::Number => Constant::Number(0f64.to_bits()),
                NeutralConstant::Boolean => Constant::Boolean(false),
                NeutralConstant::Null => Constant::Null,
                NeutralConstant::Undefined => Constant::Undefined,
            },
        })
    }
    pub(super) fn step(self, after: RevisionId) -> CheckedRewriteStep {
        CheckedRewriteStep {
            meaning: self.meaning,
            before: self.snapshot,
            after,
            rule: self.rule,
        }
    }
}

/// The neutral constant a retired value of this type becomes, or `None` when
/// its type has no constant that preserves the verifier's type check. Enums are
/// excluded: an integer only satisfies an enum type as one of its variants.
fn neutral_constant(ty: &Type<'_>) -> Option<NeutralConstant> {
    match ty {
        Type::Int => Some(NeutralConstant::Integer),
        Type::Float => Some(NeutralConstant::Number),
        Type::Bool => Some(NeutralConstant::Boolean),
        Type::Null | Type::Nullable(_) => Some(NeutralConstant::Null),
        Type::TypeParameter("$js") => Some(NeutralConstant::Undefined),
        _ => None,
    }
}

/// A local cell that is initialized or written but never read, referenced,
/// captured, bound as a parameter or catch variable, or exported. Storing a
/// different value of the same type into it cannot be observed.
fn cell_is_write_only(uses: &UseIndex, program: &Program<'_>, cell: CellId) -> bool {
    if program
        .cells
        .get(cell.index())
        .is_none_or(|entry| entry.binding != CellBinding::Local)
    {
        return false;
    }
    let Some(users) = uses.cell(cell) else {
        return false;
    };
    !users.reference_exposed()
        && users.sites().iter().all(|site| {
            matches!(
                site,
                CellUseSite::Unit {
                    usage: CellUse::Initialize(_) | CellUse::Write { .. },
                    ..
                }
            )
        })
}

/// Value-producing operations whose only obligation, once their result is
/// unused, is whatever the facts snapshot reports. Structural operations,
/// calls, stores, initialization, allocation and control transfer are never
/// retired by this rule, whatever their effects summary says.
fn retirable_kind(kind: &OperationKind) -> bool {
    matches!(
        kind,
        OperationKind::IntBinary(_)
            | OperationKind::Binary(_)
            | OperationKind::Unary { .. }
            | OperationKind::CopyValue
            | OperationKind::Load(_)
            | OperationKind::Intrinsic(_)
            | OperationKind::Select { .. }
    )
}

impl Compilation<'_> {
    /// Fold exactly one i32 binary operation whose two producers are integer
    /// literals. Not-applicable is not a proof of any alternative replacement.
    /// Success forks a source checkpoint; the original and all artifacts survive.
    pub fn fold_literal_int_binary(
        &mut self,
        base: SemanticId,
        unit: UnitId,
        operation: OpId,
        policy: &ResolvedPolicy,
        domain: WorkDomain,
    ) -> Result<Option<SemanticId>, RewriteError> {
        let started = self.ledger.work_used(domain);
        let Some(proof) = self.prepare_literal_int_fold(base, unit, operation, policy, domain)?
        else {
            return Ok(None);
        };
        self.commit_checked_rewrite(proof, policy, domain, started, None)
            .map(Some)
    }

    /// Retire one operation whose every result is unused and whose evaluation
    /// the local facts prove unobservable once discarded. It becomes a neutral
    /// constant of its result type and releases its operands, so their
    /// producers may be retired by later calls: cleanup cascades one checked,
    /// individually published step at a time. Not-applicable is `Ok(None)`.
    ///
    /// Legality is the facts snapshot's `can_drop` — no throw, divergence,
    /// reentry, suspension, control transfer or write — never the absence of
    /// writes alone. Coercions and user getters are summarized as possibly
    /// throwing and reentrant, so an unused value that could run one survives.
    pub fn drop_dead_value(
        &mut self,
        base: SemanticId,
        unit: UnitId,
        operation: OpId,
        policy: &ResolvedPolicy,
        domain: WorkDomain,
        facts: LocalFactsRequest,
    ) -> Result<Option<SemanticId>, RewriteError> {
        let started = self.ledger.work_used(domain);
        let Some(proof) =
            self.prepare_dead_value_drop(base, unit, operation, policy, domain, facts)?
        else {
            return Ok(None);
        };
        self.commit_checked_rewrite(proof, policy, domain, started, Some(facts))
            .map(Some)
    }

    fn checked_site(
        &mut self,
        base: SemanticId,
        policy: &ResolvedPolicy,
        tactic: TacticId,
        domain: WorkDomain,
    ) -> Result<usize, RewriteError> {
        let index = self.lookup(base)?;
        if !policy.tactic(tactic).enabled {
            return Err(RewriteError::ForbiddenTactic(tactic));
        }
        work(&mut self.ledger, domain, 8)?;
        // Earlier semantic choices remain permission dependencies, including
        // inherited history after an explicit source-meaning change.
        self.slots[index]
            .checkpoint
            .as_ref()
            .unwrap()
            .semantic
            .lineage
            .check_policy(policy)
            .map_err(|error| match error {
                AdmissionError::ForbiddenTactic(tactic) => RewriteError::ForbiddenTactic(tactic),
                _ => RewriteError::ForbiddenTactic(tactic),
            })?;
        Ok(index)
    }

    fn prepare_literal_int_fold(
        &mut self,
        base: SemanticId,
        unit: UnitId,
        operation: OpId,
        policy: &ResolvedPolicy,
        domain: WorkDomain,
    ) -> Result<Option<CheckedRewrite>, RewriteError> {
        let index = self.checked_site(base, policy, TacticId::ConstantFolding, domain)?;
        let semantic = &self.slots[index].checkpoint.as_ref().unwrap().semantic;
        let frozen = semantic
            .program
            .units
            .get(unit.index())
            .ok_or(RewriteError::InvalidProof)?;
        let data = frozen.data();
        let Some(target) = data.operations.get(operation.index()) else {
            return Err(RewriteError::InvalidProof);
        };
        let OperationKind::IntBinary(operator) = target.kind else {
            return Ok(None);
        };
        let operands = data
            .operands(target.operands)
            .ok_or(RewriteError::InvalidProof)?;
        if operands.len() != 2 {
            return Err(RewriteError::InvalidProof);
        }
        let literal = |value: ValueId| {
            let definition = data.values.get(value.index())?.definition;
            let producer = data.operations.get(definition.index())?;
            match producer.kind {
                OperationKind::Constant(Constant::Integer(value)) => Some((definition, value)),
                _ => None,
            }
        };
        let (Some((left_producer, left)), Some((right_producer, right))) =
            (literal(operands[0]), literal(operands[1]))
        else {
            return Ok(None);
        };
        Ok(Some(CheckedRewrite {
            base,
            snapshot: semantic.identity,
            meaning: semantic.meaning,
            tables: semantic.program.tables_revision,
            unit_revision: frozen.revision(),
            policy_fingerprint: policy.fingerprint(),
            rule: RewriteRule::LiteralIntFold(LiteralIntFold {
                version: LITERAL_INT_FOLD_VERSION,
                unit,
                operation,
                operator,
                left_producer,
                left,
                right_producer,
                right,
                result: operator.evaluate(left, right),
            }),
        }))
    }

    fn prepare_dead_value_drop(
        &mut self,
        base: SemanticId,
        unit: UnitId,
        operation: OpId,
        policy: &ResolvedPolicy,
        domain: WorkDomain,
        facts: LocalFactsRequest,
    ) -> Result<Option<CheckedRewrite>, RewriteError> {
        if self.local_facts.is_none() {
            return Err(RewriteError::FactsUnavailable);
        }
        let index = self.checked_site(base, policy, TacticId::DeadCodeElimination, domain)?;
        let (snapshot, meaning, tables, unit_revision, replacement) = {
            let semantic = &self.slots[index].checkpoint.as_ref().unwrap().semantic;
            let frozen = semantic
                .program
                .units
                .get(unit.index())
                .ok_or(RewriteError::InvalidProof)?;
            let data = frozen.data();
            let Some(target) = data.operations.get(operation.index()) else {
                return Err(RewriteError::InvalidProof);
            };
            if !retirable_kind(&target.kind) {
                return Ok(None);
            }
            // Exactly one result, and it must have no use anywhere the index
            // records — operands, region results, captures and exports alike.
            let mut results = data
                .values
                .iter()
                .enumerate()
                .filter(|(_, value)| value.definition == operation)
                .map(|(position, _)| ValueId::from_index(position));
            let Some(Some(result)) = results.next() else {
                return Ok(None);
            };
            if results.next().is_some() {
                return Ok(None);
            }
            let Some(unit_uses) = semantic.uses.unit(unit) else {
                return Err(RewriteError::InvalidProof);
            };
            let Some(uses) = unit_uses.value_uses(result) else {
                return Err(RewriteError::InvalidProof);
            };
            // Locals live in cells, so a dead local's value is "used" by the
            // `Initialize` of a cell nothing ever reads. Such a use observes
            // nothing: the cell keeps a neutral constant of the same type
            // instead, and its computation — and whatever fed only that —
            // can be retired.
            if !uses.iter().all(|value_use| match *value_use {
                ValueUse::Operand {
                    operation: initializer,
                    position: 0,
                } => {
                    match data
                        .operations
                        .get(initializer.index())
                        .map(|operation| &operation.kind)
                    {
                        Some(OperationKind::Initialize(cell)) => {
                            cell_is_write_only(&semantic.uses, &semantic.program, *cell)
                        }
                        _ => false,
                    }
                }
                _ => false,
            }) {
                return Ok(None);
            }
            let result_type = &semantic.program.types[data.values[result.index()].ty.index()];
            let Some(replacement) = neutral_constant(result_type) else {
                return Ok(None);
            };
            (
                semantic.identity,
                semantic.meaning,
                semantic.program.tables_revision,
                frozen.revision(),
                replacement,
            )
        };
        let permitted = self
            .with_local_facts(domain, 1, |group| {
                group.query(base, unit, facts).map(|view| {
                    view.facts.can_drop(operation, ObservationDemand::Discarded)
                        == Legality::PermittedUnderContext
                })
            })
            .map_err(|_| RewriteError::FactsUnavailable)?
            .map_err(|_| RewriteError::FactsUnavailable)?;
        if !permitted {
            return Ok(None);
        }
        Ok(Some(CheckedRewrite {
            base,
            snapshot,
            meaning,
            tables,
            unit_revision,
            policy_fingerprint: policy.fingerprint(),
            rule: RewriteRule::DeadValueDrop(DeadValueDrop {
                version: DEAD_VALUE_DROP_VERSION,
                unit,
                operation,
                replacement,
            }),
        }))
    }

    fn commit_checked_rewrite(
        &mut self,
        proof: CheckedRewrite,
        policy: &ResolvedPolicy,
        domain: WorkDomain,
        started: u64,
        facts: Option<LocalFactsRequest>,
    ) -> Result<SemanticId, RewriteError> {
        let index = self.lookup(proof.base)?;
        let semantic = &self.slots[index].checkpoint.as_ref().unwrap().semantic;
        let unit = proof.rule.unit();
        if semantic.identity != proof.snapshot
            || semantic.meaning != proof.meaning
            || semantic.program.tables_revision != proof.tables
            || semantic
                .program
                .units
                .get(unit.index())
                .map(FrozenUnit::revision)
                != Some(proof.unit_revision)
        {
            return Err(RewriteError::StaleProof);
        }
        let tactic = proof.rule.tactic();
        if !policy.tactic(tactic).enabled {
            return Err(RewriteError::ForbiddenTactic(tactic));
        }
        if policy.fingerprint() != proof.policy_fingerprint {
            return Err(RewriteError::PolicyMismatch);
        }
        let operation = proof.rule.operation();
        let expected = match (proof.rule, facts) {
            (RewriteRule::LiteralIntFold(_), _) => {
                self.prepare_literal_int_fold(proof.base, unit, operation, policy, domain)?
            }
            (RewriteRule::DeadValueDrop(_), Some(facts)) => {
                self.prepare_dead_value_drop(proof.base, unit, operation, policy, domain, facts)?
            }
            (RewriteRule::DeadValueDrop(_), None) => return Err(RewriteError::FactsUnavailable),
        };
        if expected != Some(proof) {
            return Err(RewriteError::InvalidProof);
        }
        Ok(self.edit_checked_rewrite_transaction(proof, domain, started)?)
    }
}

#[cfg(test)]
#[path = "rewrite_tests.rs"]
mod tests;
