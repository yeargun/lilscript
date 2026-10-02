//! Eligibility evidence attached to an evaluated artifact, independently of its
//! reusable byte scores. The immutable artifact owns this header and canonical
//! naming-plan backing. Structural recipe identity has its own immutable shared
//! owner; aggregate tactic/risk uses here are admission evidence, not replay data.
//! This evidence neither certifies semantic legality nor invents runtime cost
//! estimates for a neutral-risk transformation.
use super::RevisionId;
use crate::compilation_policy::{
    AdmissionError, BudgetLedger, CandidateCostEvidence, ResolvedPolicy, RuntimeRisk, TacticId,
    TacticUse, WorkKind,
};
use crate::js::extract::OutputError;
use crate::js::selection::Plan;
pub use crate::js::LiteralOutput;
use crate::js::{BindingId, NamingProvenance};
use crate::output_budget::{
    AllocationBudget, AllocationClass::Retained, AllocationError, RetainedCharge,
};
use std::cmp::Ordering;
use std::mem::size_of;

/// The passes actually selected for output formation. Enabled permissions are
/// defaults, not obligations: an admitted candidate may choose a proper subset.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputTactics {
    pub dead_code_elimination: bool,
    pub target_compaction: bool,
    pub rules: crate::js::TargetRules,
    pub literals: LiteralOutput,
    /// The output families formation applied (M9.2). The objective supplies
    /// only their seed; the terminal stage keeps another assignment when the
    /// exact codec says the whole artifact shrank.
    pub families: crate::js::OutputFamilies,
    /// The alternative each choice site takes where it is not its seed
    /// (M9.1): an immutable map the terminal stage extends when the exact
    /// codec says the whole artifact shrank.
    pub choices: crate::js::ChoiceMap,
}

impl OutputTactics {
    pub fn from_policy(policy: &ResolvedPolicy) -> Self {
        let dead_code_elimination = policy.tactic(TacticId::DeadCodeElimination).enabled;
        let target_compaction = policy.tactic(TacticId::TargetCompaction).enabled;
        Self {
            dead_code_elimination,
            target_compaction,
            rules: crate::js::TargetRules::from_policy(policy),
            literals: if dead_code_elimination && target_compaction {
                LiteralOutput::Observed
            } else {
                LiteralOutput::Original
            },
            families: match policy.objective() {
                Some(objective) if target_compaction => {
                    crate::js::OutputFamilies::seed(objective.codec).permitted(policy)
                }
                _ => crate::js::OutputFamilies::NONE,
            },
            choices: crate::js::ChoiceMap::SEEDS,
        }
    }

    /// Permission only; artifact admission separately checks cost evidence.
    pub fn check_policy(&self, policy: &ResolvedPolicy) -> Result<(), AdmissionError> {
        if (self.literals == LiteralOutput::Observed
            || self.families != crate::js::OutputFamilies::NONE
            || !self.choices.is_empty())
            && !self.target_compaction
        {
            return Err(AdmissionError::ForbiddenTactic(TacticId::TargetCompaction));
        }
        for (selected, tactic) in [
            (self.dead_code_elimination, TacticId::DeadCodeElimination),
            (self.target_compaction, TacticId::TargetCompaction),
        ]
        .into_iter()
        .chain(self.families.tactics())
        .chain(
            self.rules
                .tactics()
                .map(|(on, tactic)| (on && self.target_compaction, tactic)),
        ) {
            if selected && !policy.tactic(tactic).enabled {
                return Err(AdmissionError::ForbiddenTactic(tactic));
            }
        }
        if self.target_compaction {
            if self.families.string_array_packing
                && self.rules.array_packing == crate::js::ArrayPacking::Disabled
            {
                return Err(AdmissionError::RuntimePermission {
                    tactic: TacticId::StringArrayPacking,
                    risk: RuntimeRisk::Startup,
                });
            }
            self.choices.check_target_policy(policy)?;
            // A policy permission alone cannot turn on a producer explicitly
            // disabled in this formation request.
            for (key, alternative) in self.choices.iter() {
                if alternative == crate::js::AltId(0) { continue; }
                use crate::js::ChoiceFamily as F;
                let enabled = match key.family {
                    F::ExpressionInlining => self.rules.inlining,
                    F::ConstantArguments => self.rules.call_specialization,
                    F::HelperSharing => self.rules.helper_sharing,
                    F::ParameterizedHelpers => self.rules.parameterized_helpers,
                    F::DataEncoding => self.rules.data_encoding,
                    F::ConditionalValues | F::ExitPoints | F::LoopFusion | F::ConditionalReturns
                    | F::LogicalBranches | F::LoopHeads | F::LogicalStatements
                    | F::CompoundAssignments | F::QuoteDelimiter | F::OptionalChain | F::LogicalAssignment => self.rules.statement_spellings,
                    F::DeclarationOrder => self.rules.declaration_order,
                    F::ReceiverAlias => self.rules.receiver_aliases,
                    _ => false,
                };
                if !enabled {
                    return Err(AdmissionError::ForbiddenTactic(key.family.spec().tactic));
                }
            }
            for usage in self.rules.runtime_uses(self.families) {
                policy.check_tactic_permissions(std::slice::from_ref(&usage))?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(super) enum ProvenanceError {
    ContractMismatch,
    Allocation(AllocationError),
    Naming(OutputError),
    Admission(AdmissionError),
}

impl From<AllocationError> for ProvenanceError {
    fn from(error: AllocationError) -> Self {
        Self::Allocation(error)
    }
}

/// This owner is not clonable. Clients borrowing the public Plan must admit any
/// separate copy; this instance's backing leaves only through consuming discard.
/// Dropping without discard stays charged.
#[derive(Debug)]
#[must_use = "retain with the admitting compilation or explicitly discard through it"]
pub(super) struct ArtifactProvenance {
    contract: crate::compilation_policy::CompilationContract,
    naming: Plan,
    naming_origin: NamingProvenance,
    output: OutputTactics,
    uses: [TacticUse; TacticId::ALL.len()],
    use_count: usize,
    charge: RetainedCharge<RevisionId>,
}

/// Actual output and permission evidence retained with an artifact's bytes.
/// Naming provenance is explicit: the same Source plan can be a mandatory
/// baseline or an optional explored plan with different policy eligibility.
#[derive(Debug, Clone, Copy)]
pub struct ArtifactProvenanceDescription<'a> {
    naming: &'a Plan,
    output: &'a OutputTactics,
    naming_tactics: &'a [TacticUse],
    tactics: &'a [TacticUse],
}
impl<'a> ArtifactProvenanceDescription<'a> {
    pub fn naming(self) -> &'a Plan {
        self.naming
    }
    pub fn output(self) -> &'a OutputTactics {
        self.output
    }
    pub fn naming_tactics(self) -> &'a [TacticUse] {
        self.naming_tactics
    }
    pub fn tactics(self) -> &'a [TacticUse] {
        self.tactics
    }
}

impl ArtifactProvenance {
    pub(super) fn description(&self) -> ArtifactProvenanceDescription<'_> {
        ArtifactProvenanceDescription {
            naming: &self.naming,
            output: &self.output,
            naming_tactics: self.naming_origin.tactics(),
            tactics: self.tactics(),
        }
    }
    pub(super) fn build<'a>(
        structural: impl IntoIterator<Item = &'a TacticUse>,
        output: OutputTactics,
        naming: &Plan,
        policy: &ResolvedPolicy,
        owner: RevisionId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, ProvenanceError> {
        let mut phase = budget.scope();
        phase.work(WorkKind::Analysis, 1)?;
        output
            .check_policy(policy)
            .map_err(ProvenanceError::Admission)?;
        let naming_origin = naming
            .provenance_for_policy(policy)
            .map_err(ProvenanceError::Naming)?;
        let mut risks = [None; TacticId::ALL.len()];
        for &usage in structural {
            phase.work(WorkKind::Analysis, 1)?;
            merge_risk(&mut risks[usage.tactic as usize], usage.risk);
        }
        for (selected, tactic) in [
            (output.dead_code_elimination, TacticId::DeadCodeElimination),
            (output.target_compaction, TacticId::TargetCompaction),
        ]
        .into_iter()
        .chain(output.families.tactics())
        .chain(
            output
                .rules
                .tactics()
                .map(|(on, tactic)| (on && output.target_compaction, tactic)),
        ) {
            phase.work(WorkKind::Analysis, 1)?;
            if selected {
                merge_risk(&mut risks[tactic as usize], RuntimeRisk::Neutral);
            }
        }
        for usage in naming_origin.tactics() {
            phase.work(WorkKind::Analysis, 1)?;
            merge_risk(&mut risks[usage.tactic as usize], usage.risk);
        }
        if output.target_compaction {
            for usage in output.rules.runtime_uses(output.families).chain(output.choices.uses()) {
                phase.work(WorkKind::Analysis, 1)?;
                merge_risk(&mut risks[usage.tactic as usize], usage.risk);
            }
        }
        let mut uses = [TacticUse {
            tactic: TacticId::DeadCodeElimination,
            risk: RuntimeRisk::Neutral,
        }; TacticId::ALL.len()];
        let mut use_count = 0;
        for tactic in TacticId::ALL {
            phase.work(WorkKind::Analysis, 1)?;
            if let Some(risk) = risks[tactic as usize] {
                uses[use_count] = TacticUse { tactic, risk };
                use_count += 1;
            }
        }
        // Overrides impose a set of source-spelling requirements. Their input
        // order and duplicate mentions do not affect Basis::names_in. Preserve
        // every distinct binding while canonicalizing the retained descriptor.
        let mut source_names = phase.copy_slice(Retained, &naming.source_names)?;
        let count = u64::try_from(source_names.len()).map_err(|_| AllocationError::Capacity)?;
        let levels = u64::from(usize::BITS - source_names.len().max(1).leading_zeros());
        phase.work(
            WorkKind::Analysis,
            count.checked_mul(levels).ok_or(AllocationError::Capacity)?,
        )?;
        source_names.sort_unstable();
        phase.work(WorkKind::Analysis, count)?;
        source_names.dedup();
        // The choice map is shared with the artifacts holding the same
        // one; each holder is charged for it, as if it were its own.
        let choices = u64::try_from(output.choices.retained_bytes())
            .map_err(|_| AllocationError::Capacity)?;
        phase.retain(Retained, choices)?;
        let bytes = source_names
            .capacity()
            .checked_mul(size_of::<BindingId>())
            .and_then(|n| n.checked_add(output.choices.retained_bytes()))
            .and_then(|n| u64::try_from(n).ok())
            .ok_or(AllocationError::Capacity)?;
        let before = phase.retained_bytes(Retained);
        let contract = policy.contract().copy_admitted(&mut phase)?;
        let bytes = bytes.checked_add(phase.retained_bytes(Retained) - before)
            .ok_or(AllocationError::Capacity)?;
        let charge = phase.detach_retained(owner, bytes)?;
        Ok(Self {
            contract,
            naming: Plan {
                style: naming.style,
                alphabet: naming.alphabet,
                source_names,
                self_named: naming.self_named,
                read_order: naming.read_order,
                local_read_order: naming.local_read_order,
            compact_order: naming.compact_order,
            },
            naming_origin,
            output,
            uses,
            use_count,
            charge,
        })
    }

    pub(super) fn naming(&self) -> &Plan {
        &self.naming
    }
    pub(super) fn tactics(&self) -> &[TacticUse] {
        &self.uses[..self.use_count]
    }
    #[cfg(test)]
    pub(super) fn output(&self) -> OutputTactics {
        self.output.clone()
    }

    /// Compare complete output choices, not bytes, handles or a hash. The
    /// frontier orders structural identity before these naming and literal ties.
    pub(super) fn compare_output(
        &self,
        other: &Self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Ordering, AllocationError> {
        budget.work(WorkKind::Analysis, 1)?;
        let style = self.naming.style.cmp(&other.naming.style);
        if style != Ordering::Equal {
            return Ok(style);
        }
        for (left, right) in self
            .naming
            .source_names
            .iter()
            .zip(&other.naming.source_names)
        {
            budget.work(WorkKind::Analysis, 1)?;
            let order = left.cmp(right);
            if order != Ordering::Equal {
                return Ok(order);
            }
        }
        budget.work(WorkKind::Analysis, 1)?;
        let length = self
            .naming
            .source_names
            .len()
            .cmp(&other.naming.source_names.len());
        if length != Ordering::Equal {
            return Ok(length);
        }
        budget.work(WorkKind::Analysis, 58)?;
        let spelling = (
            self.naming.self_named,
            self.naming.read_order,
            self.naming.local_read_order,
            self.naming.compact_order,
            self.naming.alphabet,
        )
            .cmp(&(
                other.naming.self_named,
                other.naming.read_order,
                other.naming.local_read_order,
                other.naming.compact_order,
                other.naming.alphabet,
            ));
        if spelling != Ordering::Equal {
            return Ok(spelling);
        }
        budget.work(WorkKind::Analysis, 1)?;
        Ok(self.output.cmp(&other.output))
    }

    pub(super) fn admit(
        &self,
        policy: &ResolvedPolicy,
        cost: CandidateCostEvidence,
        baseline: CandidateCostEvidence,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), ProvenanceError> {
        if self.contract != *policy.contract() {
            return Err(ProvenanceError::ContractMismatch);
        }
        self.check_output_permissions(policy, budget)?;
        policy
            .admit_evidence(self.tactics(), cost, baseline)
            .map_err(ProvenanceError::Admission)
    }

    fn check_output_permissions(
        &self,
        policy: &ResolvedPolicy,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), ProvenanceError> {
        budget.work(WorkKind::Analysis, 1 + self.use_count as u64)?;
        self.output
            .check_policy(policy)
            .map_err(ProvenanceError::Admission)?;
        self.naming_origin
            .check_policy(&self.naming, policy)
            .map_err(ProvenanceError::Naming)
    }

    #[cfg(test)]
    pub(super) fn retained_bytes(&self) -> u64 {
        self.charge.bytes()
    }

    pub(super) fn discard(
        self,
        owner: RevisionId,
        ledger: &mut BudgetLedger,
    ) -> Result<(), (Self, AllocationError)> {
        if !self.charge.belongs_to(&owner) {
            return Err((self, AllocationError::WrongOwner));
        }
        let Self { naming, contract, output, charge, .. } = self;
        drop((naming, contract, output));
        charge
            .discard(&owner, ledger)
            .unwrap_or_else(|_| panic!("artifact provenance allocation owner invariant"));
        Ok(())
    }
}

fn merge_risk(slot: &mut Option<RuntimeRisk>, incoming: RuntimeRisk) {
    let rank = |risk| match risk {
        RuntimeRisk::Neutral => 0,
        RuntimeRisk::Startup => 1,
        RuntimeRisk::Recurring => 2,
    };
    if slot.is_none_or(|old| rank(incoming) > rank(old)) {
        *slot = Some(incoming);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compilation_policy::{
        BudgetError, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
    };
    use crate::js::selection::Style;

    const NO_OUTPUT: OutputTactics = OutputTactics {
        literals: LiteralOutput::Original,
        dead_code_elimination: false,
        target_compaction: false,
        rules: crate::js::TargetRules::NONE,
        families: crate::js::OutputFamilies::NONE,
        choices: crate::js::ChoiceMap::SEEDS,
    };
    const WORK: u64 = 100_000;
    const MEMORY: u64 = 100_000;

    fn policy(text: &str) -> ResolvedPolicy {
        let config: crate::config::ProjectConfig = toml::from_str(text).unwrap();
        config
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            })
            .unwrap()
    }
    fn enabled() -> ResolvedPolicy {
        policy(
            "[policy.tactics]\nidentifier-mangling='on'\nnaming-search='on'\nnaming-compaction='on'\nscalar-replacement='on'\ndead-code-elimination='on'\ntarget-compaction='on'",
        )
    }

    #[test]
    fn retained_alphabet_keeps_its_own_veto_and_parent_permissions() {
        let resolved = enabled();
        let owner = RevisionId::fresh();
        let mut ledger = ledger(WORK, MEMORY);
        let mut plan = Plan::new(Style::Scoped);
        plan.alphabet = crate::js::selection::Alphabet::observed(
            [b"zzzzz".as_slice()],
            &mut AllocationBudget::new(None),
        )
        .unwrap();
        let evidence = build(
            owner,
            &mut ledger,
            WorkDomain::Optional,
            &resolved,
            &plan,
            &[],
            NO_OUTPUT,
        );
        assert!(evidence
            .tactics()
            .iter()
            .any(|usage| usage.tactic == TacticId::NamingAlphabet));
        assert_eq!(evidence.naming().alphabet, plan.alphabet);
        for tactic in [
            TacticId::NamingAlphabet,
            TacticId::NamingSearch,
            TacticId::IdentifierMangling,
        ] {
            let off = policy(&format!("[policy.tactics]\n{}='off'", tactic.spec().name));
            assert!(admit(&evidence, &off, &mut ledger).is_err());
        }
        evidence.discard(owner, &mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    }

    #[test]
    fn retained_local_read_order_keeps_search_permission_and_distinct_provenance() {
        let resolved = enabled();
        let owner = RevisionId::fresh();
        let mut ledger = ledger(WORK, MEMORY);
        let mut plan = Plan::new(Style::Scoped);
        let plain = build(owner, &mut ledger, WorkDomain::Optional, &resolved, &plan, &[], NO_OUTPUT);
        plan.local_read_order = true;
        let local = build(owner, &mut ledger, WorkDomain::Optional, &resolved, &plan, &[], NO_OUTPUT);
        assert!(local.naming().local_read_order);
        assert!(local.tactics().iter().any(|usage| usage.tactic == TacticId::NamingSearch));
        assert_eq!(plain.compare_output(&local, &mut AllocationBudget::new(None)).unwrap(), Ordering::Less);
        for setting in ["identifier-mangling", "naming-search"] {
            let disabled = policy(&format!("[policy.tactics]\n{setting}='off'"));
            assert!(admit(&local, &disabled, &mut ledger).is_err());
        }
        for evidence in [plain, local] {
            evidence.discard(owner, &mut ledger).unwrap();
        }
        assert_eq!(ledger.retained_bytes(), 0);
    }
    #[test]
    fn g1_retained_compact_order_keeps_search_permission_and_distinct_provenance() {
        let resolved = enabled();
        let owner = RevisionId::fresh();
        let mut ledger = ledger(WORK, MEMORY);
        let mut plan = Plan::new(Style::Scoped);
        let plain = build(owner, &mut ledger, WorkDomain::Optional, &resolved, &plan, &[], NO_OUTPUT);
        plan.compact_order = true;
        let local = build(owner, &mut ledger, WorkDomain::Optional, &resolved, &plan, &[], NO_OUTPUT);
        assert!(local.naming().compact_order);
        assert!(local.tactics().iter().any(|usage| usage.tactic == TacticId::NamingSearch));
        assert_eq!(plain.compare_output(&local, &mut AllocationBudget::new(None)).unwrap(), Ordering::Less);
        for setting in ["identifier-mangling", "naming-search", "naming-compaction"] {
            let disabled = policy(&format!("[policy.tactics]\n{setting}='off'"));
            assert!(admit(&local, &disabled, &mut ledger).is_err());
        }
        for evidence in [plain, local] {
            evidence.discard(owner, &mut ledger).unwrap();
        }
        assert_eq!(ledger.retained_bytes(), 0);
    }
    fn ledger(work: u64, memory: u64) -> BudgetLedger {
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: work,
                optional_work: work,
                baseline_retained_bytes: 0,
                retained_bytes: memory,
            },
        )
        .unwrap()
    }
    fn build(
        owner: RevisionId,
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
        policy: &ResolvedPolicy,
        plan: &Plan,
        uses: &[TacticUse],
        output: OutputTactics,
    ) -> ArtifactProvenance {
        ArtifactProvenance::build(
            uses,
            output,
            plan,
            policy,
            owner,
            &mut AllocationBudget::new(Some((ledger, domain))),
        )
        .unwrap()
    }
    fn admit(
        evidence: &ArtifactProvenance,
        policy: &ResolvedPolicy,
        ledger: &mut BudgetLedger,
    ) -> Result<(), ProvenanceError> {
        evidence.admit(
            policy,
            CandidateCostEvidence::size_only(10),
            CandidateCostEvidence::size_only(10),
            &mut AllocationBudget::new(Some((ledger, WorkDomain::Optional))),
        )
    }

    #[test]
    fn naming_records_actual_choices_and_revalidates_equal_cached_bytes() {
        use crate::js::{Binding, Expr, Literal, Module, ScopeId, Statement};
        let resolved = enabled();
        let baseline = policy("[policy.tactics]\nidentifier-mangling='on'\nnaming-search='off'");
        // The allocator's seed needs no search permission; Global does.
        assert_eq!(Plan::seeds_for_policy(&baseline).unwrap(), &[Style::Scoped]);
        let mut module = Module::default();
        let binding = module.binding(Binding {
            source_symbol: None,
            scope: ScopeId::new(0),
            spelling: "longLocalName".into(),
            pinned: false,
            class: None,
            defined: false,
        });
        let value = module.expression(Expr::Literal(Literal::Number(7.0)), None);
        module.regions[0].statements.push(Statement::Let {
            binding,
            value: Some(value),
        });
        let output = module.prepare_output_with_policy(&resolved).unwrap();
        assert_eq!(
            output.render(&Plan::new(Style::Global)).unwrap(),
            output.render(&Plan::new(Style::Scoped)).unwrap()
        );

        let owner = RevisionId::fresh();
        let mut ledger = ledger(WORK, MEMORY);
        let global = build(
            owner,
            &mut ledger,
            WorkDomain::Optional,
            &resolved,
            &Plan::new(Style::Global),
            &[],
            NO_OUTPUT,
        );
        let scoped = build(
            owner,
            &mut ledger,
            WorkDomain::Optional,
            &resolved,
            &Plan::new(Style::Scoped),
            &[],
            NO_OUTPUT,
        );
        assert_eq!(
            scoped.tactics(),
            &[TacticUse {
                tactic: TacticId::IdentifierMangling,
                risk: RuntimeRisk::Neutral
            }]
        );
        assert_eq!(global.tactics().len(), 2);
        assert!(admit(&scoped, &baseline, &mut ledger).is_ok());
        assert!(matches!(
            admit(&global, &baseline, &mut ledger),
            Err(ProvenanceError::Naming(_))
        ));
        global.discard(owner, &mut ledger).unwrap();
        scoped.discard(owner, &mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    }

    #[test]
    fn source_baseline_and_searched_source_keep_different_permission_dependencies() {
        let resolved = enabled();
        // Raw naming-search permission remains on, but mangling off makes the
        // effective naming plan space Source-only through the common owner.
        let source_only = policy("[policy.tactics]\nidentifier-mangling='off'\nnaming-search='on'");
        assert_eq!(
            Plan::seeds_for_policy(&source_only).unwrap(),
            &[Style::Source]
        );
        let owner = RevisionId::fresh();
        let mut ledger = ledger(WORK, MEMORY);
        let plan = Plan::new(Style::Source);
        let searched = build(
            owner,
            &mut ledger,
            WorkDomain::Optional,
            &resolved,
            &plan,
            &[],
            NO_OUTPUT,
        );
        let required = build(
            owner,
            &mut ledger,
            WorkDomain::Optional,
            &source_only,
            &plan,
            &[],
            NO_OUTPUT,
        );
        assert_eq!(
            searched.tactics(),
            &[TacticUse {
                tactic: TacticId::NamingSearch,
                risk: RuntimeRisk::Neutral
            }]
        );
        assert!(required.tactics().is_empty());
        assert_eq!(searched.naming(), required.naming());
        assert!(admit(&required, &source_only, &mut ledger).is_ok());
        assert!(admit(&required, &resolved, &mut ledger).is_ok());
        assert!(matches!(
            admit(&searched, &source_only, &mut ledger),
            Err(ProvenanceError::Naming(_))
        ));
        searched.discard(owner, &mut ledger).unwrap();
        required.discard(owner, &mut ledger).unwrap();
    }

    #[test]
    fn full_plan_is_canonical_but_overrides_still_require_search() {
        let resolved = enabled();
        let owner = RevisionId::fresh();
        let mut ledger = ledger(WORK, MEMORY);
        let mut plan = Plan {
            style: Style::Global,
            alphabet: Default::default(),
            source_names: vec![BindingId::new(8), BindingId::new(2), BindingId::new(8)],
            self_named: false,
            read_order: false,
            local_read_order: false,
            compact_order: false,
        };
        let first = build(
            owner,
            &mut ledger,
            WorkDomain::Optional,
            &resolved,
            &plan,
            &[],
            NO_OUTPUT,
        );
        plan.source_names = vec![BindingId::new(2), BindingId::new(8)];
        let second = build(
            owner,
            &mut ledger,
            WorkDomain::Optional,
            &resolved,
            &plan,
            &[],
            NO_OUTPUT,
        );
        plan.source_names[1] = BindingId::new(9);
        let different = build(
            owner,
            &mut ledger,
            WorkDomain::Optional,
            &resolved,
            &plan,
            &[],
            NO_OUTPUT,
        );
        assert_eq!(
            first.naming().source_names,
            vec![BindingId::new(2), BindingId::new(8)]
        );
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        assert_eq!(
            first.compare_output(&second, &mut budget).unwrap(),
            Ordering::Equal
        );
        assert_eq!(
            first.compare_output(&different, &mut budget).unwrap(),
            Ordering::Less
        );
        drop(budget);
        assert!(first
            .tactics()
            .iter()
            .any(|usage| usage.tactic == TacticId::NamingSearch));
        let baseline = policy("[policy.tactics]\nidentifier-mangling='on'\nnaming-search='off'");
        assert!(admit(&first, &baseline, &mut ledger).is_err());
        for value in [first, second, different] {
            value.discard(owner, &mut ledger).unwrap();
        }
        assert_eq!(ledger.retained_bytes(), 0);
    }

    #[test]
    fn structural_and_output_tactics_are_actual_and_not_lost_to_cost_reuse() {
        let resolved = enabled();
        let owner = RevisionId::fresh();
        let mut ledger = ledger(WORK, MEMORY);
        let usage = TacticUse {
            tactic: TacticId::ScalarReplacement,
            risk: RuntimeRisk::Neutral,
        };
        let scalar = build(
            owner,
            &mut ledger,
            WorkDomain::Optional,
            &resolved,
            &Plan::new(Style::Global),
            &[usage, usage],
            NO_OUTPUT,
        );
        assert_eq!(
            scalar
                .tactics()
                .iter()
                .filter(|entry| **entry == usage)
                .count(),
            1
        );
        assert_eq!(scalar.output(), NO_OUTPUT);
        let scalar_off =
            policy("[policy.tactics]\nidentifier-mangling='on'\nscalar-replacement='off'");
        assert!(matches!(
            admit(&scalar, &scalar_off, &mut ledger),
            Err(ProvenanceError::Admission(AdmissionError::ForbiddenTactic(
                TacticId::ScalarReplacement
            )))
        ));
        let direct = build(
            owner,
            &mut ledger,
            WorkDomain::Optional,
            &resolved,
            &Plan::new(Style::Global),
            &[],
            NO_OUTPUT,
        );
        let optimized = build(
            owner,
            &mut ledger,
            WorkDomain::Optional,
            &resolved,
            &Plan::new(Style::Global),
            &[],
            OutputTactics::from_policy(&resolved),
        );
        for (setting, tactic) in [
            ("dead-code-elimination", TacticId::DeadCodeElimination),
            ("target-compaction", TacticId::TargetCompaction),
        ] {
            let off = policy(&format!(
                "[policy.tactics]\nidentifier-mangling='on'\n{setting}='off'"
            ));
            assert!(admit(&direct, &off, &mut ledger).is_ok());
            assert!(matches!(admit(&optimized, &off, &mut ledger),
                Err(ProvenanceError::Admission(AdmissionError::ForbiddenTactic(found))) if found == tactic));
            let before = ledger.retained_bytes();
            let denied = ArtifactProvenance::build(
                &[],
                optimized.output(),
                &Plan::new(Style::Global),
                &off,
                owner,
                &mut AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional))),
            );
            assert!(denied.is_err());
            assert_eq!(ledger.retained_bytes(), before);
        }
        for value in [scalar, direct, optimized] {
            value.discard(owner, &mut ledger).unwrap();
        }
    }

    #[test]
    fn retained_target_rule_permissions_survive_score_reuse() {
        let resolved = policy("effort.level=16");
        let owner = RevisionId::fresh();
        let mut ledger = ledger(WORK, MEMORY);
        let evidence = build(
            owner,
            &mut ledger,
            WorkDomain::Optional,
            &resolved,
            &Plan::new(Style::Global),
            &[],
            OutputTactics {
                families: crate::js::OutputFamilies::seed(crate::js::selection::Objective::Raw),
                ..OutputTactics::from_policy(&resolved)
            },
        );
        for tactic in [
            TacticId::StringPooling,
            TacticId::StringArrayPacking,
            TacticId::Inlining,
            TacticId::ConstantFolding,
            TacticId::ScalarReplacement,
        ] {
            assert_eq!(
                evidence
                    .tactics()
                    .iter()
                    .filter(|usage| usage.tactic == tactic)
                    .count(),
                1
            );
            let off = policy(&format!(
                "effort.level=16\n[policy.tactics]\n{}='off'",
                tactic.spec().name
            ));
            assert!(matches!(admit(&evidence, &off, &mut ledger),
                Err(ProvenanceError::Admission(AdmissionError::ForbiddenTactic(found))) if found == tactic));
        }
        evidence.discard(owner, &mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    }

    #[test]
    fn unknown_runtime_cost_remains_unknown_under_policy_constraints() {
        let resolved = enabled();
        let owner = RevisionId::fresh();
        let mut ledger = ledger(WORK, MEMORY);
        let evidence = build(
            owner,
            &mut ledger,
            WorkDomain::Optional,
            &resolved,
            &Plan::new(Style::Global),
            &[],
            NO_OUTPUT,
        );
        assert!(admit(&evidence, &resolved, &mut ledger).is_ok());
        for (text, missing) in [
            ("[policy.constraints]\nmax_startup_work=0", "startup work"),
            (
                "[policy.constraints]\nmax_recurring_work=0",
                "recurring work",
            ),
            (
                "[policy.constraints]\nmax_runtime_memory_bytes=0",
                "runtime memory",
            ),

        ] {
            assert!(matches!(admit(&evidence, &policy(text), &mut ledger),
                Err(ProvenanceError::Admission(AdmissionError::MissingCostEvidence(found))) if found == missing));
        }
        evidence.discard(owner, &mut ledger).unwrap();
        let balanced = policy("[javascript]\npriority='balanced'");
        let evidence = build(owner, &mut ledger, WorkDomain::Optional, &balanced,
            &Plan::new(Style::Global), &[], NO_OUTPUT);
        assert!(matches!(admit(&evidence, &balanced, &mut ledger),
            Err(ProvenanceError::Admission(AdmissionError::MissingCostEvidence("performance estimate")))));
        evidence.discard(owner, &mut ledger).unwrap();
    }

    #[test]
    fn reconstruction_risks_survive_retention_and_cannot_replay_under_weaker_permissions() {
        for (tactic, risk) in [
            (TacticId::StartupReconstruction, RuntimeRisk::Startup),
            (TacticId::StringArrayPacking, RuntimeRisk::Recurring),
        ] {
            let resolved = policy(&format!("[policy.tactics]\n{}='on'", tactic.spec().name));
            let owner = RevisionId::fresh();
            let mut ledger = ledger(WORK, MEMORY);
            let mut output = OutputTactics::from_policy(&resolved);
            output.families.string_array_packing = tactic == TacticId::StringArrayPacking;
            let evidence = build(
                owner,
                &mut ledger,
                WorkDomain::Optional,
                &resolved,
                &Plan::new(Style::Scoped),
                &[],
                output,
            );
            assert!(evidence.tactics().contains(&TacticUse { tactic, risk }));
            for level in [0, 13, 16] {
                let denied = policy(&format!(
                    "effort.level={level}\n[policy.tactics]\n{}='off'",
                    tactic.spec().name
                ));
                assert!(admit(&evidence, &denied, &mut ledger).is_err());
            }
            if risk == RuntimeRisk::Recurring {
                assert!(
                    admit(&evidence, &policy("effort.level=16"), &mut ledger).is_err(),
                    "a startup grant cannot admit recurring work"
                );
            }
            // A retained legacy assignment cannot smuggle an effort-granted
            // startup permission into an explicit-risk request.
            let modern = policy("effort.level=16\n[policy]\nversion=3");
            assert!(admit(&evidence, &modern, &mut ledger).is_err());
            let modern_on = policy(&format!("effort.level=16\n[policy]\nversion=3\n[policy.tactics]\n{}='on'", tactic.spec().name));
            assert!(admit(&evidence, &modern_on, &mut ledger).is_ok());
            evidence.discard(owner, &mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        }
    }

    #[test]
    fn q1_retained_site_risk_and_joint_permission_are_revalidated() {
        use crate::representation::{AltId, ChoiceFamily as F, ChoiceKey, ChoiceMap, SiteId};
        let resolved = policy("policy.tactics.helper-sharing='on'\npolicy.tactics.representation-joints='on'");
        let owner = RevisionId::fresh();
        let mut ledger = ledger(WORK, MEMORY);
        let mut output = OutputTactics::from_policy(&resolved);
        output.families.parameterized_helpers = false;
        output.choices = ChoiceMap::SEEDS.with(ChoiceKey { family: F::ParameterizedHelpers,
            site: SiteId::Symbol(0) }, AltId(1)).from_joint();
        let evidence = build(owner, &mut ledger, WorkDomain::Optional, &resolved,
            &Plan::new(Style::Scoped), &[], output);
        assert!(evidence.tactics().contains(&TacticUse { tactic: TacticId::HelperSharing, risk: RuntimeRisk::Recurring }));
        assert!(evidence.tactics().contains(&TacticUse { tactic: TacticId::RepresentationJoints, risk: RuntimeRisk::Neutral }));
        for denied in ["policy.tactics.helper-sharing='auto'\npolicy.tactics.representation-joints='on'",
            "policy.tactics.helper-sharing='on'\npolicy.tactics.representation-joints='off'"] {
            assert!(admit(&evidence, &policy(denied), &mut ledger).is_err());
        }
        evidence.discard(owner, &mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    }

    #[test]
    fn explicit_encoded_sites_cannot_bypass_a_disabled_encoding_rule() {
        let resolved = policy("[policy.tactics]\nstartup-reconstruction='off'");
        let mut output = OutputTactics::from_policy(&resolved);
        output.choices = output.choices.with(
            crate::js::ChoiceKey {
                family: crate::js::ChoiceFamily::DataEncoding,
                site: crate::js::SiteId::Symbol(0),
            },
            crate::js::AltId(1),
        );
        assert_eq!(
            output.check_policy(&resolved),
            Err(AdmissionError::ForbiddenTactic(
                TacticId::StartupReconstruction
            ))
        );
    }

    #[test]
    fn level_16_grants_declared_startup_risk_and_a_tactic_override_still_wins() {
        // D5: level 16 is level 15 plus startup-risk permission for the
        // tactics that declare it. The compiler has no grant of its
        // own; it inherits this one from the resolved policy, and a
        // `[policy.tactics]` row overrides it in either direction.
        let owner = RevisionId::fresh();
        let mut ledger = ledger(WORK, MEMORY);
        let packing = TacticId::StringArrayPacking;
        let risky = [TacticUse {
            tactic: packing,
            risk: RuntimeRisk::Startup,
        }];
        let at = |level: u8, tactics: &str| {
            policy(&format!(
                "effort.level={level}\n[policy.tactics]\n{tactics}"
            ))
        };
        for (resolved, expected) in [
            (
                at(15, ""),
                Some(AdmissionError::RuntimePermission {
                    tactic: packing,
                    risk: RuntimeRisk::Startup,
                }),
            ),
            (at(16, ""), None),
            (at(15, "string-array-packing='on'"), None),
            (
                at(16, "string-array-packing='off'"),
                Some(AdmissionError::ForbiddenTactic(packing)),
            ),
        ] {
            let evidence = build(
                owner,
                &mut ledger,
                WorkDomain::Optional,
                &resolved,
                &Plan::new(Style::Source),
                &risky,
                NO_OUTPUT,
            );
            match (admit(&evidence, &resolved, &mut ledger), expected) {
                (Ok(()), None) => {}
                (Err(ProvenanceError::Admission(found)), Some(expected)) => {
                    assert_eq!(found, expected)
                }
                (found, expected) => panic!("admitted {found:?}, expected {expected:?}"),
            }
            evidence.discard(owner, &mut ledger).unwrap();
        }
    }

    #[test]
    fn detached_plan_owns_original_domain_and_rejected_admission_releases_storage() {
        let resolved = enabled();
        let owner = RevisionId::fresh();
        let plan = Plan {
            style: Style::Global,
            alphabet: Default::default(),
            source_names: vec![BindingId::new(1), BindingId::new(2)],
            self_named: false,
            read_order: false,
            local_read_order: false,
            compact_order: false,
        };
        for domain in [WorkDomain::Baseline, WorkDomain::Optional] {
            let mut ledger = ledger(WORK, MEMORY);
            let evidence = build(owner, &mut ledger, domain, &resolved, &plan, &[], NO_OUTPUT);
            let bytes = evidence.retained_bytes();
            assert_eq!(bytes, (2 * size_of::<BindingId>()) as u64);
            assert_eq!(ledger.retained_bytes_in(domain), bytes);
            let (evidence, error) = evidence
                .discard(RevisionId::fresh(), &mut ledger)
                .unwrap_err();
            assert_eq!(error, AllocationError::WrongOwner);
            assert_eq!(ledger.retained_bytes_in(domain), bytes);
            assert!(admit(&evidence, &resolved, &mut ledger).is_ok());
            assert_eq!(ledger.retained_bytes_in(domain), bytes);
            evidence.discard(owner, &mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        }
        let mut limited = ledger(WORK, 0);
        assert!(matches!(
            ArtifactProvenance::build(
                &[],
                NO_OUTPUT,
                &plan,
                &resolved,
                owner,
                &mut AllocationBudget::new(Some((&mut limited, WorkDomain::Optional)))
            ),
            Err(ProvenanceError::Allocation(AllocationError::Budget(
                BudgetError::MemoryExhausted(WorkDomain::Optional)
            )))
        ));
        assert_eq!(limited.peak_retained_bytes(), 0);
        let mut observed_allocated_rejection = false;
        for work in 0..64 {
            let mut limited = ledger(work, MEMORY);
            let result = ArtifactProvenance::build(
                &[],
                NO_OUTPUT,
                &plan,
                &resolved,
                owner,
                &mut AllocationBudget::new(Some((&mut limited, WorkDomain::Optional))),
            );
            match result {
                Ok(evidence) => {
                    evidence.discard(owner, &mut limited).unwrap();
                }
                Err(ProvenanceError::Allocation(AllocationError::Budget(
                    BudgetError::WorkExhausted(_),
                ))) => {
                    observed_allocated_rejection |= limited.peak_retained_bytes() > 0;
                }
                error => panic!("unexpected admission: {error:?}"),
            }
            assert_eq!(limited.retained_bytes(), 0);
        }
        assert!(observed_allocated_rejection);
        assert!(matches!(
            ArtifactProvenance::build(
                &[],
                NO_OUTPUT,
                &plan,
                &resolved,
                owner,
                &mut AllocationBudget::new(None)
            ),
            Err(ProvenanceError::Allocation(AllocationError::Unaccounted))
        ));
    }
}
