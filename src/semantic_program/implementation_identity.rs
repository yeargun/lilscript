//! Canonical selected recipes over one immutable semantic snapshot.
//!
//! Compilation establishes snapshot equality separately. These descriptors use
//! local recipe positions in that snapshot's source tables, never checkpoint
//! slots, revision allocation order or proof allocation addresses. Fixed resources
//! also
//! retain exact physical contracts, actual producer bytes and permission evidence
//! through their existing shared owners. Adding another implementation parameter
//! must extend the corresponding versioned descriptor.
use super::fixed_resource::ResourceChoice;
use super::implementations::ImplementationMap;
#[path = "resource_identity.rs"]
mod resource;
pub use resource::{
    ArtifactProvenanceDescription, FrozenProducerDescription, PhysicalExportDescription,
    PhysicalParameter, ProductTransport, ResourceDescription,
};

/// Complete borrowed selected implementation. `recipe_words` is the unchanged
/// local-layout encoding; resources are a separate required identity component.
/// No retained owner or proof can escape through this read-only description.
#[derive(Debug, Clone, Copy)]
pub struct ImplementationDescription<'a> {
    recipes: &'a [u32],
    resource: ResourceDescription<'a>,
    rewrites: super::rewrite_lineage::RewriteDescription<'a>,
    snapshot: Option<RevisionId>,
    meaning: Option<RevisionId>,
}
impl<'a> ImplementationDescription<'a> {
    pub fn recipe_words(self) -> &'a [u32] {
        self.recipes
    }
    pub fn resource(self) -> ResourceDescription<'a> {
        self.resource
    }
    /// Includes inherited permission history. A step with another meaning ID
    /// preceded a SourceChange and is not a replay claim for the current root.
    pub fn rewrites(self) -> super::rewrite_lineage::RewriteDescription<'a> {
        self.rewrites
    }
    pub fn snapshot_identity(self) -> Option<RevisionId> {
        self.snapshot
    }
    pub fn meaning_identity(self) -> Option<RevisionId> {
        self.meaning
    }
    /// Only unrevised Whole has a complete identity in the legacy word view.
    pub fn whole_words(self) -> Option<&'a [u32]> {
        (matches!(self.resource, ResourceDescription::Whole) && self.rewrites.is_empty())
            .then_some(self.recipes)
    }
}

use super::string_family::StringChoice;
use super::RevisionId;
use crate::compilation_policy::{BudgetLedger, WorkKind};
use crate::output_budget::{
    AllocationBudget, AllocationClass::Retained, AllocationError, RetainedCharge,
};
use std::cmp::Ordering;
use std::mem::size_of;
#[path = "shared_implementation_identity.rs"]
mod shared;
pub(super) use shared::SharedImplementationIdentity;

const FORMAT: u32 = 3;
const HASH_OFFSET: u64 = 0xcbf29ce484222325;
const HASH_PRIME: u64 = 0x100000001b3;

/// The containing shared owner admits this inline header in its own storage.
/// Construction detaches the word backing into the same compilation's token;
/// the ledger borrow can end between frontier operations. Ordinary Drop leaves
/// the reservation charged. Only its compilation may explicitly discard it.
#[derive(Debug)]
#[must_use = "keep the admitting allocation owner alive or discard through it"]
pub(super) struct ImplementationIdentity {
    words: Vec<u32>,
    fingerprint: u64,
    resource: Option<ResourceChoice>,
    charge: RetainedCharge<RevisionId>,
}

impl ImplementationIdentity {
    pub(super) fn build(
        map: Option<&ImplementationMap>,
        owner: RevisionId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        let mut phase = budget.scope();
        phase.work(WorkKind::Analysis, 1)?;
        let (records, helpers, strings, products, functions) = map.map_or((0, 0, 0, 0, 0), |map| {
            (
                map.records().len(),
                map.helpers().len(),
                map.strings().len(),
                map.products().len(),
                map.functions().len(),
            )
        });
        let mut count = 6usize
            .checked_add(records.checked_mul(4).ok_or(AllocationError::Capacity)?)
            .and_then(|n| n.checked_add(helpers.checked_mul(2)?))
            .ok_or(AllocationError::Capacity)?;
        if let Some(map) = map {
            for family in map.strings() {
                phase.work(WorkKind::Analysis, 1)?;
                count = count
                    .checked_add(3)
                    .and_then(|n| n.checked_add(family.definitions().len().checked_mul(2)?))
                    .ok_or(AllocationError::Capacity)?;
            }
        }
        if let Some(map) = map {
            for family in map.products() {
                phase.work(WorkKind::Analysis, 1)?;
                count = count
                    .checked_add(4)
                    .and_then(|n| n.checked_add(family.fields().len()))
                    .and_then(|n| n.checked_add(family.cells().len().checked_mul(4)?))
                    .ok_or(AllocationError::Capacity)?;
            }
        }
        if let Some(map) = map {
            for layout in map.functions() {
                phase.work(WorkKind::Analysis, 1)?;
                count = count
                    .checked_add(2)
                    .and_then(|n| n.checked_add(layout.parameters().len().checked_mul(3)?))
                    .ok_or(AllocationError::Capacity)?;
            }
        }
        let mut words = phase.vector(Retained, count)?;
        let mut fingerprint = HASH_OFFSET;
        append(
            &mut words,
            &mut fingerprint,
            &[FORMAT, word(records)?],
            &mut phase,
        )?;
        if let Some(map) = map {
            // Private map insertion keeps each family category in source-root
            // order. Publication order therefore has no place in this encoding.
            for family in map.records() {
                let root = family.root();
                append(
                    &mut words,
                    &mut fingerprint,
                    &[
                        word(root.state.index())?,
                        word(root.unit.index())?,
                        word(root.allocation.index())?,
                        word(family.allocation().operation.index())?,
                    ],
                    &mut phase,
                )?;
            }
        }
        append(&mut words, &mut fingerprint, &[word(helpers)?], &mut phase)?;
        if let Some(map) = map {
            for family in map.helpers() {
                let root = family.root();
                append(
                    &mut words,
                    &mut fingerprint,
                    &[word(root.cell.index())?, word(root.body.index())?],
                    &mut phase,
                )?;
            }
        }
        append(&mut words, &mut fingerprint, &[word(strings)?], &mut phase)?;
        if let Some(map) = map {
            for family in map.strings() {
                let (choice, activation) = match family.choice() {
                    StringChoice::LiteralAtDefinition => (0, 0),
                    StringChoice::SharedLiteral { activation } => (1, word(activation.index())?),
                };
                append(
                    &mut words,
                    &mut fingerprint,
                    &[choice, activation, word(family.definitions().len())?],
                    &mut phase,
                )?;
                // Group boundaries and every selected definition are necessary:
                // two equal strings can occupy one shared binding or two.
                for definition in family.definitions() {
                    append(
                        &mut words,
                        &mut fingerprint,
                        &[
                            word(definition.unit.index())?,
                            word(definition.value.index())?,
                        ],
                        &mut phase,
                    )?;
                }
            }
        }
        append(&mut words, &mut fingerprint, &[word(products)?], &mut phase)?;
        if let Some(map) = map {
            for family in map.products() {
                append(
                    &mut words,
                    &mut fingerprint,
                    &[
                        word(family.root().index())?,
                        word(family.schema().index())?,
                        word(family.fields().len())?,
                        word(family.cells().len())?,
                    ],
                    &mut phase,
                )?;
                for field in family.fields() {
                    append(
                        &mut words,
                        &mut fingerprint,
                        &[word(field.index())?],
                        &mut phase,
                    )?;
                }
                for cell in family.cells() {
                    let (kind, unit, operation) = match cell.origin {
                        super::product_family::ProductOrigin::Initialize(at) => {
                            (0, word(at.unit.index())?, word(at.operation.index())?)
                        }
                        super::product_family::ProductOrigin::Parameter => (1, 0, 0),
                    };
                    append(
                        &mut words,
                        &mut fingerprint,
                        &[word(cell.cell.index())?, kind, unit, operation],
                        &mut phase,
                    )?;
                }
            }
        }
        append(
            &mut words,
            &mut fingerprint,
            &[word(functions)?],
            &mut phase,
        )?;
        if let Some(map) = map {
            for layout in map.functions() {
                append(
                    &mut words,
                    &mut fingerprint,
                    &[
                        word(layout.body().index())?,
                        word(layout.parameters().len())?,
                    ],
                    &mut phase,
                )?;
                for parameter in layout.parameters() {
                    append(
                        &mut words,
                        &mut fingerprint,
                        &[parameter.position, word(parameter.schema.index())?, 1],
                        &mut phase,
                    )?;
                }
            }
        }
        debug_assert_eq!(words.len(), count);
        let bytes = words
            .capacity()
            .checked_mul(size_of::<u32>())
            .ok_or(AllocationError::Capacity)?;
        let bytes = u64::try_from(bytes).map_err(|_| AllocationError::Capacity)?;
        // Whole follows exactly the prior word/hash/admission path. Resource
        // visits are paid before retaining any shared producer owner.
        let selected_resource = map.and_then(ImplementationMap::resource);
        if let Some(selected) = selected_resource {
            resource::fingerprint(
                ResourceDescription::of(Some(selected)),
                &mut fingerprint,
                &mut phase,
            )?;
        }
        let resource = selected_resource
            .map(|selected| selected.share(&mut phase))
            .transpose()?;
        let charge = match phase.detach_retained(owner, bytes) {
            Ok(charge) => charge,
            Err(error) => {
                if let Some(resource) = resource {
                    resource
                        .discard(&mut phase)
                        .expect("shared identity resource retains its original owner");
                }
                return Err(error);
            }
        };
        Ok(Self {
            words,
            fingerprint,
            resource,
            charge,
        })
    }

    #[cfg(test)]
    pub(super) fn force_fingerprint_for_collision_test(&mut self, fingerprint: u64) {
        self.fingerprint = fingerprint;
    }

    /// Hash-bucket hint only. It must never order ties or establish equality.
    pub(super) fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    pub(super) fn description(&self) -> ImplementationDescription<'_> {
        ImplementationDescription {
            recipes: &self.words,
            resource: ResourceDescription::of(self.resource.as_ref()),
            rewrites: Default::default(),
            snapshot: None,
            meaning: None,
        }
    }

    #[cfg(test)]
    pub(super) fn whole_words(&self) -> Option<&[u32]> {
        self.description().whole_words()
    }

    /// Compatibility accessor for already-qualified Whole clients. Resource
    /// callers must use description(); word-only publication rejects them.
    #[cfg(test)]
    pub(super) fn words(&self) -> &[u32] {
        self.whole_words()
            .expect("resource identity requires its full description")
    }

    /// The same paid owner used by frontier identity, borrowed through a scope
    /// which releases actual storage on callback return, error or unwind.
    #[cfg(test)]
    pub(super) fn with_description<R>(
        map: Option<&ImplementationMap>,
        owner: RevisionId,
        budget: &mut AllocationBudget<'_>,
        inspect: impl FnOnce(ImplementationDescription<'_>) -> R,
    ) -> Result<R, AllocationError> {
        let identity = Self::build(map, owner, budget)?;
        let scope = IdentityScope {
            identity: Some(identity),
            owner,
            budget,
        };
        Ok(inspect(scope.identity.as_ref().unwrap().description()))
    }

    /// Selected-recipe equality only: the caller must first establish the same
    /// live semantic snapshot. Hash collisions always receive a full comparison.
    pub(super) fn equivalent(
        &self,
        other: &Self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        budget.work(WorkKind::Analysis, 1)?;
        if self.fingerprint != other.fingerprint || self.words.len() != other.words.len() {
            return Ok(false);
        }
        Ok(self.compare(other, budget)? == Ordering::Equal)
    }

    /// Stable structural tie descriptor; deliberately ignores all fingerprints
    /// and snapshot identities. Current-output naming ties belong to their
    /// separate owner; fixed producer naming/eligibility is part of a resource.
    pub(super) fn compare(
        &self,
        other: &Self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Ordering, AllocationError> {
        for (&left, &right) in self.words.iter().zip(&other.words) {
            budget.work(WorkKind::Analysis, 1)?;
            let order = left.cmp(&right);
            if order != Ordering::Equal {
                return Ok(order);
            }
        }
        budget.work(WorkKind::Analysis, 1)?;
        let order = self.words.len().cmp(&other.words.len());
        if order != Ordering::Equal || (self.resource.is_none() && other.resource.is_none()) {
            return Ok(order);
        }
        resource::compare(
            self.description().resource(),
            other.description().resource(),
            budget,
        )
    }

    /// Newly detached word backing only. Shared resource payload remains charged
    /// exactly once by its original owner, including after source map disposal.
    pub(super) fn retained_bytes(&self) -> u64 {
        self.charge.bytes()
    }

    /// A foreign owner gets the intact descriptor back. Once owner identity is
    /// established, the same-ledger charge invariant is the compilation's duty.
    pub(super) fn discard(
        self,
        owner: RevisionId,
        ledger: &mut BudgetLedger,
    ) -> Result<(), (Self, AllocationError)> {
        if !self.charge.belongs_to(&owner) {
            return Err((self, AllocationError::WrongOwner));
        }
        let Self {
            words,
            resource,
            charge,
            ..
        } = self;
        drop(words);
        if let Some(resource) = resource {
            let mut budget = AllocationBudget::new(Some((&mut *ledger, charge.domain())));
            resource
                .discard(&mut budget)
                .expect("implementation identity resource allocation owner invariant");
        }
        charge
            .discard(&owner, ledger)
            .unwrap_or_else(|_| panic!("implementation identity allocation owner invariant"));
        Ok(())
    }
}

#[cfg(test)]
struct IdentityScope<'budget, 'ledger> {
    identity: Option<ImplementationIdentity>,
    owner: RevisionId,
    budget: &'budget mut AllocationBudget<'ledger>,
}
#[cfg(test)]
impl Drop for IdentityScope<'_, '_> {
    fn drop(&mut self) {
        let identity = self.identity.take().unwrap();
        self.budget.with_ledger(|ledger| {
            identity
                .discard(
                    self.owner,
                    ledger.expect("identity construction requires admission").0,
                )
                .unwrap();
        });
    }
}

fn word(value: usize) -> Result<u32, AllocationError> {
    u32::try_from(value).map_err(|_| AllocationError::Capacity)
}

fn append(
    words: &mut Vec<u32>,
    hash: &mut u64,
    values: &[u32],
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AllocationError> {
    budget.work(
        WorkKind::Analysis,
        u64::try_from(values.len()).map_err(|_| AllocationError::Capacity)?,
    )?;
    debug_assert!(words.capacity() - words.len() >= values.len());
    for &value in values {
        for byte in value.to_le_bytes() {
            *hash = (*hash ^ u64::from(byte)).wrapping_mul(HASH_PRIME);
        }
    }
    words.extend_from_slice(values);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compilation_policy::{
        AnalysisAttempt, BudgetError, BudgetPlan, ResourceLimits, WorkDomain,
    };
    use crate::semantic_program::facts::{
        CacheLimits, FactRequest, RetainedFactsCache, LOCAL_FACTS_PLAN, LOCAL_FACTS_VERSION,
    };
    use crate::semantic_program::string_family::{self, StringFamily, ValueRef};
    use crate::semantic_program::uses::UseIndex;
    use crate::semantic_program::{helper_family, record_family, *};

    const WORK: u64 = 10_000_000;
    const MEMORY: u64 = 10_000_000;
    const SOURCE: &str = "Record<int> state=record{count:1};auto helper=(int amount)=>{state.count=(state.count??0)+amount;return state.count??0;};print(helper(2));print(\"left\"+\"right\");print(\"left\"+\"right\");print(\"left\"+\"right\");print(\"left\"+\"right\");";

    fn checked(inspect: impl FnOnce(&Program<'_>)) {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, SOURCE).unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        inspect(&from_checked_source(&syntax, &semantics).unwrap());
    }
    fn ledger() -> BudgetLedger {
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: WORK,
                optional_work: WORK,
                baseline_retained_bytes: 0,
                retained_bytes: MEMORY,
            },
        )
        .unwrap()
    }
    fn cell(program: &Program<'_>, name: &str) -> CellId {
        CellId::from_index(
            program
                .cells
                .iter()
                .position(|cell| cell.name == name)
                .unwrap(),
        )
        .unwrap()
    }
    fn cache(ledger: &mut BudgetLedger) -> RetainedFactsCache {
        RetainedFactsCache::new(
            CacheLimits {
                entries: 8,
                bytes: 1_000_000,
                result_bytes: 100_000,
            },
            ledger,
            WorkDomain::Baseline,
        )
        .unwrap()
    }
    fn request() -> FactRequest {
        FactRequest {
            attempt: AnalysisAttempt {
                plan: LOCAL_FACTS_PLAN,
                algorithm_version: LOCAL_FACTS_VERSION,
                work_quota: 100_000,
            },
            result_bytes: 100_000,
        }
    }
    fn definitions(program: &Program<'_>) -> Vec<ValueRef> {
        program
            .units
            .iter()
            .enumerate()
            .flat_map(|(unit, data)| {
                data.data().operations.iter().filter_map(move |op| {
                    matches!(op.kind, OperationKind::Binary(BinaryOp::Add)).then(|| ValueRef {
                        unit: UnitId::from_index(unit).unwrap(),
                        value: op.result.unwrap(),
                    })
                })
            })
            .collect()
    }
    fn string(
        program: &Program<'_>,
        values: &[ValueRef],
        choice: StringChoice,
        cache: &mut RetainedFactsCache,
        ledger: &mut BudgetLedger,
    ) -> StringFamily {
        let prepared = string_family::prepare(
            program,
            values,
            choice,
            string_family::FamilyRequest {
                attempt: AnalysisAttempt {
                    plan: string_family::STRING_FAMILY_PLAN,
                    algorithm_version: string_family::STRING_FAMILY_VERSION,
                    work_quota: 100_000,
                },
                scratch_bytes: 100_000,
                output_bytes: 100_000,
                local_facts: request(),
            },
            ledger,
            WorkDomain::Optional,
        )
        .unwrap();
        let mut ready = match prepared.outcome {
            string_family::PreparationOutcome::Ready(ready) => ready,
            other => panic!("string preparation {other:?}"),
        };
        {
            let mut session = cache.session(ledger, WorkDomain::Optional, 1).unwrap();
            let facts = session.query(program, ready.unit(), request()).unwrap();
            ready
                .check_facts(program, facts.facts, facts.receipt)
                .unwrap();
        }
        match ready.finish(ledger).unwrap().outcome {
            string_family::FamilyOutcome::Complete(family) => family,
            other => panic!("string proof {other:?}"),
        }
    }
    fn record(
        program: &Program<'_>,
        uses: &UseIndex,
        ledger: &mut BudgetLedger,
    ) -> record_family::RecordFamily {
        let analyzed = record_family::analyze(
            program,
            uses,
            cell(program, "state"),
            record_family::FamilyRequest {
                attempt: AnalysisAttempt {
                    plan: record_family::RECORD_FAMILY_PLAN,
                    algorithm_version: record_family::RECORD_FAMILY_VERSION,
                    work_quota: 100_000,
                },
                scratch_bytes: 100_000,
                output_bytes: 100_000,
            },
            ledger,
            WorkDomain::Optional,
        )
        .unwrap();
        match analyzed.outcome {
            record_family::FamilyOutcome::Complete(family) => family,
            other => panic!("record proof {other:?}"),
        }
    }
    fn helper(
        program: &Program<'_>,
        uses: &UseIndex,
        cache: &mut RetainedFactsCache,
        ledger: &mut BudgetLedger,
    ) -> helper_family::HelperFamily {
        let prepared = helper_family::prepare(
            program,
            uses,
            cell(program, "helper"),
            helper_family::FamilyRequest {
                execution: crate::compilation_contract::JavaScriptExecution::Module,
                attempt: AnalysisAttempt {
                    plan: helper_family::HELPER_FAMILY_PLAN,
                    algorithm_version: helper_family::HELPER_FAMILY_VERSION,
                    work_quota: 100_000,
                },
                scratch_bytes: 100_000,
                output_bytes: 100_000,
            },
            ledger,
            WorkDomain::Optional,
        )
        .unwrap();
        let mut ready = match prepared.outcome {
            helper_family::PreparationOutcome::Ready(ready) => ready,
            other => panic!("helper preparation {other:?}"),
        };
        {
            let mut session = cache.session(ledger, WorkDomain::Optional, 1).unwrap();
            let facts = session
                .query(program, ready.root().body, request())
                .unwrap();
            ready
                .check_body(program, facts.facts, facts.receipt)
                .unwrap();
        }
        match ready.finish(ledger).unwrap() {
            helper_family::FamilyOutcome::Complete(family) => family,
            other => panic!("helper proof {other:?}"),
        }
    }
    fn build(
        map: Option<&ImplementationMap>,
        owner: RevisionId,
        ledger: &mut BudgetLedger,
    ) -> ImplementationIdentity {
        let mut budget = AllocationBudget::new(Some((ledger, WorkDomain::Optional)));
        let identity = ImplementationIdentity::build(map, owner, &mut budget).unwrap();
        assert_eq!(
            budget.retained_bytes(Retained),
            0,
            "descriptor charge is detached"
        );
        identity
    }
    fn ordered_map(
        program: &Program<'_>,
        uses: &UseIndex,
        cache: &mut RetainedFactsCache,
        ledger: &mut BudgetLedger,
        order: &[char],
    ) -> ImplementationMap {
        let values = definitions(program);
        assert_eq!(values.len(), 4);
        let mut map = ImplementationMap::direct();
        for &recipe in order {
            let next = match recipe {
                'R' => map.with_scalar(record(program, uses, ledger), ledger, WorkDomain::Optional),
                'H' => map.with_inline_helper(
                    helper(program, uses, cache, ledger),
                    ledger,
                    WorkDomain::Optional,
                ),
                'S' => map.with_string(
                    string(
                        program,
                        &values,
                        StringChoice::SharedLiteral {
                            activation: values[0].unit,
                        },
                        cache,
                        ledger,
                    ),
                    ledger,
                    WorkDomain::Optional,
                ),
                _ => unreachable!(),
            }
            .unwrap();
            map.discard(ledger).unwrap();
            map = next;
        }
        map
    }

    #[test]
    fn physical_publication_orders_converge_without_proof_or_payload_identity() {
        checked(|program| {
            let owner = RevisionId::fresh();
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let mut cache = cache(&mut ledger);
            let rhs = ordered_map(program, &uses, &mut cache, &mut ledger, &['R', 'H', 'S']);
            // Independent proofs and unrelated revision issuance do not name a state.
            for _ in 0..23 {
                let _ = RevisionId::fresh();
            }
            let shr = ordered_map(program, &uses, &mut cache, &mut ledger, &['S', 'H', 'R']);
            assert!(!std::ptr::eq(
                rhs.records().next().unwrap(),
                shr.records().next().unwrap()
            ));
            let left = build(Some(&rhs), owner, &mut ledger);
            let right = build(Some(&shr), owner, &mut ledger);
            {
                let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                assert_eq!(left.fingerprint(), right.fingerprint());
                assert!(left.equivalent(&right, &mut budget).unwrap());
                assert_eq!(left.compare(&right, &mut budget).unwrap(), Ordering::Equal);
            }
            left.discard(owner, &mut ledger).unwrap();
            right.discard(owner, &mut ledger).unwrap();
            rhs.discard(&mut ledger).unwrap();
            shr.discard(&mut ledger).unwrap();
            cache.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        });
    }

    #[test]
    fn string_group_boundaries_choices_and_every_definition_are_distinct() {
        checked(|program| {
            let mut ledger = ledger();
            let owner = RevisionId::fresh();
            let mut cache = cache(&mut ledger);
            let values = definitions(program);
            let shared = StringChoice::SharedLiteral {
                activation: values[0].unit,
            };
            let groups = [
                (
                    StringChoice::LiteralAtDefinition,
                    vec![vec![values[0], values[1]]],
                ),
                (shared, vec![vec![values[0], values[1]]]),
                (shared, vec![vec![values[0]], vec![values[1]]]),
                (shared, vec![vec![values[0], values[2]]]),
                (
                    shared,
                    vec![vec![values[0], values[2]], vec![values[1], values[3]]],
                ),
                (
                    shared,
                    vec![vec![values[0], values[1]], vec![values[2], values[3]]],
                ),
            ];
            let mut identities = Vec::new(); // Test oracle storage, not the frontier.
            for (choice, groups) in groups {
                let mut map = ImplementationMap::direct();
                for definitions in groups {
                    let next = map
                        .with_string(
                            string(program, &definitions, choice, &mut cache, &mut ledger),
                            &mut ledger,
                            WorkDomain::Optional,
                        )
                        .unwrap();
                    map.discard(&mut ledger).unwrap();
                    map = next;
                }
                identities.push(build(Some(&map), owner, &mut ledger));
                map.discard(&mut ledger).unwrap();
            }
            {
                let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                for i in 0..identities.len() {
                    for j in 0..i {
                        assert!(!identities[i]
                            .equivalent(&identities[j], &mut budget)
                            .unwrap());
                        assert_ne!(
                            identities[i].compare(&identities[j], &mut budget).unwrap(),
                            Ordering::Equal
                        );
                    }
                }
                // Force a same-length descriptor collision; the full group/choice
                // comparison must still reject, and the deterministic tie is stable.
                let original_order = identities[0].compare(&identities[1], &mut budget).unwrap();
                identities[1].fingerprint = identities[0].fingerprint;
                assert!(!identities[0]
                    .equivalent(&identities[1], &mut budget)
                    .unwrap());
                assert_eq!(
                    identities[0].compare(&identities[1], &mut budget).unwrap(),
                    original_order
                );
            }
            for identity in identities {
                identity.discard(owner, &mut ledger).unwrap();
            }
            cache.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        });
    }

    #[test]
    fn detached_lifetime_releases_original_domain_and_rejects_foreign_owner() {
        let mut ledger = ledger();
        let owner = RevisionId::fresh();
        let wrong = RevisionId::fresh();
        let baseline = {
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
            ImplementationIdentity::build(None, owner, &mut budget).unwrap()
        };
        let optional = build(Some(&ImplementationMap::direct()), owner, &mut ledger);
        let bytes = baseline.retained_bytes() + optional.retained_bytes();
        assert_eq!(ledger.retained_bytes(), bytes);
        let (baseline, error) = baseline.discard(wrong, &mut ledger).unwrap_err();
        assert_eq!(error, AllocationError::WrongOwner);
        assert_eq!(ledger.retained_bytes(), bytes);
        {
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
            assert!(baseline.equivalent(&optional, &mut budget).unwrap());
        }
        optional.discard(owner, &mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), baseline.retained_bytes());
        baseline.discard(owner, &mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    }

    #[test]
    fn failed_construction_and_comparison_preserve_existing_retained_identity() {
        for memory_failure in [false, true] {
            let mut ledger = ledger();
            let owner = RevisionId::fresh();
            let incumbent = build(None, owner, &mut ledger);
            let existing = ledger.retained_bytes();
            let padding = if memory_failure {
                let padding = MEMORY - existing - incumbent.retained_bytes() + 1;
                ledger.retain(WorkDomain::Optional, padding).unwrap();
                padding
            } else {
                // The first query visit and exact Vec allocation admission pass;
                // the first descriptor-word traversal fails after allocation.
                let remaining = WORK - ledger.work_used(WorkDomain::Optional);
                ledger
                    .charge(WorkDomain::Optional, WorkKind::Analysis, remaining - 2)
                    .unwrap();
                0
            };
            {
                let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                let error = ImplementationIdentity::build(None, owner, &mut budget).unwrap_err();
                assert_eq!(budget.retained_bytes(Retained), 0);
                assert!(
                    matches!(
                        error,
                        AllocationError::Budget(BudgetError::MemoryExhausted(WorkDomain::Optional))
                    ) == memory_failure
                );
                if !memory_failure {
                    assert_eq!(
                        error,
                        AllocationError::Budget(BudgetError::WorkExhausted(WorkDomain::Optional))
                    );
                    assert!(matches!(
                        incumbent.compare(&incumbent, &mut budget),
                        Err(AllocationError::Budget(BudgetError::WorkExhausted(
                            WorkDomain::Optional
                        )))
                    ));
                }
            }
            assert_eq!(ledger.retained_bytes(), existing + padding);
            ledger.release(WorkDomain::Optional, padding).unwrap();
            incumbent.discard(owner, &mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        }
    }

    #[test]
    fn inspection_cannot_claim_detached_compilation_admission() {
        let mut budget = AllocationBudget::new(None);
        assert!(matches!(
            ImplementationIdentity::build(None, RevisionId::fresh(), &mut budget),
            Err(AllocationError::Unaccounted)
        ));
    }
}
