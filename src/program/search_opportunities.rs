//! Bounded semantic opportunity hints; complete family proofs own legality.
//!
//! Order is cell-table occurrence order, followed by unit/value occurrence
//! order, with body transport hints between the cell and value prefixes.
//! It is independent of emitted names, revisions and candidate handles.
//! The inventory belongs to one immutable semantic snapshot. Selected-map
//! membership and compatibility belong to the search owner, not discovery.
use super::string_family::{StringChoice, ValueRef};
use super::{CellBinding, CellId, OperationKind, Program, RevisionId, Type, UnitId, ValueId};
use crate::compilation_policy::{BudgetLedger, ResolvedPolicy, TacticId, WorkKind};
use crate::output_budget::{
    AllocationBudget, AllocationClass::Retained, AllocationError, RetainedCharge,
};
use std::mem::size_of;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OpportunityView<'a> {
    Scalar(CellId),
    Product(CellId),
    Inline(CellId),
    Function(UnitId),
    String {
        definitions: &'a [ValueRef],
        choice: StringChoice,
    },
}

#[derive(Debug, Clone, Copy)]
enum Opportunity {
    Scalar(CellId),
    Product(CellId),
    Inline(CellId),
    Function(UnitId),
    String {
        start: usize,
        length: usize,
        choice: StringChoice,
    },
}

/// The containing search owner owns the inline header; this token accounts for
/// both dynamic buffers. Ordinary Drop does not silently release their charge.
#[derive(Debug)]
#[must_use = "retain in the search owner or discard through its original ledger"]
pub(super) struct Inventory {
    opportunities: Vec<Opportunity>,
    definitions: Vec<ValueRef>,
    truncated: bool,
    needs_facts: bool,
    charge: RetainedCharge<RevisionId>,
}

impl Inventory {
    pub(super) fn build(
        program: &Program<'_>,
        policy: &ResolvedPolicy,
        max_opportunities: usize,
        owner: RevisionId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        let mut phase = budget.scope();
        phase.work(WorkKind::Analysis, 1)?;
        let scalar = policy.tactic(TacticId::ScalarReplacement).enabled;
        let inline = policy.tactic(TacticId::Inlining).enabled;
        let functions = policy.tactic(TacticId::CallSpecialization).enabled;
        let literal = policy.tactic(TacticId::ConstantFolding).enabled;
        let shared = literal && policy.tactic(TacticId::StringPooling).enabled;
        let mut opportunities = Vec::new();
        let mut definitions = Vec::new();
        let mut truncated = false;
        let mut needs_facts = false;
        'discover: {
            if scalar || inline {
                for (index, cell) in program.cells.iter().enumerate() {
                    phase.work(WorkKind::Analysis, 1)?;
                    let ty = program
                        .types
                        .get(cell.ty.index())
                        .ok_or(AllocationError::Capacity)?;
                    let id = CellId::from_index(index).ok_or(AllocationError::Capacity)?;
                    let hint = if scalar && matches!(ty, Type::Record(_)) {
                        Some(Opportunity::Scalar(id))
                    } else if scalar && matches!(ty, Type::Struct(_) | Type::StructInstance { .. })
                    {
                        Some(Opportunity::Product(id))
                    } else if inline
                        && !matches!(cell.binding, CellBinding::Foreign)
                        && matches!(ty, Type::Function(_) | Type::GenericFunction(_))
                    {
                        Some(Opportunity::Inline(id))
                    } else {
                        None
                    };
                    if let Some(hint) = hint {
                        if opportunities.len() == max_opportunities {
                            truncated = true;
                            break 'discover;
                        }
                        needs_facts |= matches!(hint, Opportunity::Inline(_));
                        phase.push(Retained, &mut opportunities, hint)?;
                    }
                }
            }
            if functions {
                // One body-wide hint changes every eligible private creator's
                // shared signature. A function-valued cell hint would repeat
                // the same all-call proof for each creator or alias.
                for (unit_index, unit) in program.units.iter().enumerate() {
                    phase.work(WorkKind::Analysis, 1)?;
                    let mut product = false;
                    for &parameter in &unit.data().parameters {
                        phase.work(WorkKind::Analysis, 1)?;
                        let cell = program
                            .cells
                            .get(parameter.index())
                            .ok_or(AllocationError::Capacity)?;
                        if matches!(
                            program.types.get(cell.ty.index()),
                            Some(Type::Struct(_) | Type::StructInstance { .. })
                        ) && !program.is_reference_parameter(parameter)
                        {
                            product = true;
                            break;
                        }
                    }
                    if product {
                        if opportunities.len() == max_opportunities {
                            truncated = true;
                            break 'discover;
                        }
                        phase.push(
                            Retained,
                            &mut opportunities,
                            Opportunity::Function(
                                UnitId::from_index(unit_index).ok_or(AllocationError::Capacity)?,
                            ),
                        )?;
                    }
                }
            }
            if literal {
                for (unit_index, unit) in program.units.iter().enumerate() {
                    phase.work(WorkKind::Analysis, 1)?;
                    let unit_id =
                        UnitId::from_index(unit_index).ok_or(AllocationError::Capacity)?;
                    for (value_index, value) in unit.data().values.iter().enumerate() {
                        phase.work(WorkKind::Analysis, 1)?;
                        if !matches!(program.types.get(value.ty.index()), Some(Type::String)) {
                            continue;
                        }
                        let operation = unit
                            .data()
                            .operations
                            .get(value.definition.index())
                            .ok_or(AllocationError::Capacity)?;
                        // Already-literal producers need no optional materialization.
                        // Loads/calls/other string producers are only hints: no exact
                        // value, effect or initialization fact is inferred here.
                        if matches!(operation.kind, OperationKind::Constant(_)) {
                            continue;
                        }
                        if opportunities.len() == max_opportunities {
                            truncated = true;
                            break 'discover;
                        }
                        let start = definitions.len();
                        needs_facts = true;
                        phase.push(
                            Retained,
                            &mut definitions,
                            ValueRef {
                                unit: unit_id,
                                value: ValueId::from_index(value_index)
                                    .ok_or(AllocationError::Capacity)?,
                            },
                        )?;
                        phase.push(
                            Retained,
                            &mut opportunities,
                            Opportunity::String {
                                start,
                                length: 1,
                                choice: StringChoice::LiteralAtDefinition,
                            },
                        )?;
                        if shared {
                            if opportunities.len() == max_opportunities {
                                truncated = true;
                                break 'discover;
                            }
                            // Both alternatives borrow the same definition range.
                            // Future proved equal-value groups can widen the range
                            // without changing the scheduler-facing interface.
                            phase.push(
                                Retained,
                                &mut opportunities,
                                Opportunity::String {
                                    start,
                                    length: 1,
                                    choice: StringChoice::SharedLiteral {
                                        activation: unit_id,
                                    },
                                },
                            )?;
                        }
                    }
                }
            }
        }
        let bytes = opportunities
            .capacity()
            .checked_mul(size_of::<Opportunity>())
            .and_then(|n| n.checked_add(definitions.capacity().checked_mul(size_of::<ValueRef>())?))
            .ok_or(AllocationError::Capacity)?;
        let charge = phase.detach_retained(
            owner,
            u64::try_from(bytes).map_err(|_| AllocationError::Capacity)?,
        )?;
        Ok(Self {
            opportunities,
            definitions,
            truncated,
            needs_facts,
            charge,
        })
    }

    pub(super) fn len(&self) -> usize {
        self.opportunities.len()
    }

    pub(super) fn truncated(&self) -> bool {
        self.truncated
    }

    pub(super) fn needs_facts(&self) -> bool {
        self.needs_facts
    }

    pub(super) fn get(&self, index: usize) -> Option<OpportunityView<'_>> {
        Some(match *self.opportunities.get(index)? {
            Opportunity::Scalar(cell) => OpportunityView::Scalar(cell),
            Opportunity::Product(cell) => OpportunityView::Product(cell),
            Opportunity::Inline(cell) => OpportunityView::Inline(cell),
            Opportunity::Function(body) => OpportunityView::Function(body),
            Opportunity::String {
                start,
                length,
                choice,
            } => OpportunityView::String {
                definitions: &self.definitions[start..start + length],
                choice,
            },
        })
    }

    /// Cell hints form one sorted prefix. The caller admits logarithmic work;
    /// no cell-to-hint array is retained merely to share a component proof.
    pub(super) fn product_index(&self, cell: CellId) -> Option<usize> {
        let index = self
            .opportunities
            .binary_search_by(|hint| match hint {
                Opportunity::Scalar(found)
                | Opportunity::Product(found)
                | Opportunity::Inline(found) => found.cmp(&cell),
                Opportunity::Function(_) | Opportunity::String { .. } => {
                    std::cmp::Ordering::Greater
                }
            })
            .ok()?;
        matches!(self.opportunities[index], Opportunity::Product(_)).then_some(index)
    }

    pub(super) fn discard(
        self,
        owner: RevisionId,
        ledger: &mut BudgetLedger,
    ) -> Result<(), (Self, AllocationError)> {
        if !self.charge.belongs_to(&owner) {
            return Err((self, AllocationError::WrongOwner));
        }
        let Self {
            opportunities,
            definitions,
            charge,
            ..
        } = self;
        drop(opportunities);
        drop(definitions);
        charge
            .discard(&owner, ledger)
            .unwrap_or_else(|_| panic!("opportunity inventory allocation owner invariant"));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compilation_policy::{
        BudgetError, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
    };
    use crate::program::{from_checked_source, BinaryOp};

    const SOURCE: &str = "extern string host();Record<int> state=record{count:1};auto helper=(int x)=>x+1;string known=\"left\"+\"right\";string dynamic=host();";
    const WORK: u64 = 100_000;
    const MEMORY: u64 = 1_000_000;

    fn checked(inspect: impl FnOnce(&Program<'_>)) {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, SOURCE).unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        inspect(&from_checked_source(&syntax, &semantics).unwrap());
    }

    fn policy(settings: &str) -> ResolvedPolicy {
        toml::from_str::<crate::config::ProjectConfig>(settings)
            .unwrap()
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            })
            .unwrap()
    }

    fn ledger(optional_work: u64, memory: u64) -> BudgetLedger {
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: WORK,
                optional_work,
                baseline_retained_bytes: 0,
                retained_bytes: memory,
            },
        )
        .unwrap()
    }

    fn inventory(
        program: &Program<'_>,
        policy: &ResolvedPolicy,
        limit: usize,
        owner: RevisionId,
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
    ) -> Result<Inventory, AllocationError> {
        Inventory::build(
            program,
            policy,
            limit,
            owner,
            &mut AllocationBudget::new(Some((ledger, domain))),
        )
    }

    #[test]
    fn automatic_hints_use_semantic_occurrences_and_keep_unknown_strings_for_proof() {
        checked(|program| {
            let mut ledger = ledger(WORK, MEMORY);
            let owner = RevisionId::fresh();
            let inventory = inventory(
                program,
                &policy(""),
                100,
                owner,
                &mut ledger,
                WorkDomain::Optional,
            )
            .unwrap();
            assert!(!inventory.truncated());
            assert!(inventory.needs_facts());
            let cell = |name| {
                CellId::from_index(
                    program
                        .cells
                        .iter()
                        .position(|cell| cell.name == name)
                        .unwrap(),
                )
                .unwrap()
            };
            assert_eq!(
                inventory.get(0),
                Some(OpportunityView::Scalar(cell("state")))
            );
            assert_eq!(
                inventory.get(1),
                Some(OpportunityView::Inline(cell("helper")))
            );
            let mut add = false;
            let mut call = false;
            let mut previous = None;
            for index in (2..inventory.len()).step_by(2) {
                let Some(OpportunityView::String {
                    definitions,
                    choice: StringChoice::LiteralAtDefinition,
                }) = inventory.get(index)
                else {
                    panic!("literal hint must lead its optional shared alternative")
                };
                assert_eq!(definitions.len(), 1);
                let value = definitions[0];
                let key = (value.unit.index(), value.value.index());
                assert!(previous.is_none_or(|previous| previous < key));
                previous = Some(key);
                let unit = program.unit(value.unit).unwrap();
                let definition = &unit.values[value.value.index()];
                assert!(matches!(program.types[definition.ty.index()], Type::String));
                let operation = &unit.operations[definition.definition.index()];
                assert!(!matches!(operation.kind, OperationKind::Constant(_)));
                add |= matches!(operation.kind, OperationKind::Binary(BinaryOp::Add));
                call |= matches!(operation.kind, OperationKind::Call(_));
                let Some(OpportunityView::String {
                    definitions: shared,
                    choice,
                }) = inventory.get(index + 1)
                else {
                    panic!("missing shared hint")
                };
                assert_eq!(
                    choice,
                    StringChoice::SharedLiteral {
                        activation: value.unit
                    }
                );
                assert!(
                    std::ptr::eq(definitions, shared),
                    "one backing range serves both choices"
                );
            }
            assert!(
                add && call,
                "known construction and unknown foreign result are both only hints"
            );
            assert_eq!(inventory.get(inventory.len()), None);
            assert!(ledger.work_by_kind(WorkKind::Analysis) > 0);
            assert!(ledger.retained_bytes() > 0);
            inventory.discard(owner, &mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        });
    }

    #[test]
    fn product_hints_share_scalar_policy_and_need_no_local_facts() {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(
            &arena,
            "struct Point{int x;int y;}Point a=Point{1,2};Point b=a;b.x=9;print(a.x);print(b.x);",
        )
        .unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        let program = from_checked_source(&syntax, &semantics).unwrap();
        for enabled in [false, true] {
            let policy = policy(&format!(
                "[policy.tactics]\nscalar-replacement='{}'\ninlining='off'\nconstant-folding='off'",
                if enabled { "on" } else { "off" },
            ));
            let mut ledger = ledger(WORK, MEMORY);
            let owner = RevisionId::fresh();
            let hints = inventory(
                &program,
                &policy,
                100,
                owner,
                &mut ledger,
                WorkDomain::Optional,
            )
            .unwrap();
            assert!(!hints.needs_facts());
            assert!(!hints.truncated());
            assert_eq!(hints.len(), if enabled { 2 } else { 0 });
            if enabled {
                for (index, name) in ["a", "b"].into_iter().enumerate() {
                    let Some(OpportunityView::Product(cell)) = hints.get(index) else {
                        panic!("ordinary value products must reach their complete family proof")
                    };
                    assert_eq!(program.cells[cell.index()].name, name);
                }
            }
            hints.discard(owner, &mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        }
    }

    #[test]
    fn private_function_hints_are_body_scoped_and_independent_of_storage_and_inlining() {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(
            &arena,
            "struct Point{int x;int y;}int step(Point p){return p.x;}void mutate(ref Point p){p.x=4;}Point state=Point{1,2};print(step(state));mutate(ref state);",
        ).unwrap();
        let checked = crate::analyze(&syntax).unwrap();
        let program = from_checked_source(&syntax, &checked).unwrap();
        for enabled in [false, true] {
            let policy = policy(&format!(
                "[policy.tactics]\ncall-specialization='{}'\nscalar-replacement='off'\ninlining='off'\nconstant-folding='off'",
                if enabled { "on" } else { "off" },
            ));
            let mut ledger = ledger(WORK, MEMORY);
            let owner = RevisionId::fresh();
            let hints = inventory(
                &program,
                &policy,
                100,
                owner,
                &mut ledger,
                WorkDomain::Optional,
            )
            .unwrap();
            assert!(!hints.needs_facts());
            assert_eq!(hints.len(), usize::from(enabled));
            if enabled {
                let Some(OpportunityView::Function(body)) = hints.get(0) else {
                    panic!("missing function transport hint")
                };
                let cell = program.unit(body).unwrap().parameters[0];
                assert_eq!(program.cells[cell.index()].name, "p");
                assert!(!program.is_reference_parameter(cell));
                assert_eq!(hints.product_index(cell), None);
            }
            hints.discard(owner, &mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        }
    }

    #[test]
    fn bounded_inventory_is_an_exact_prefix_independent_of_its_owner() {
        checked(|program| {
            let mut ledger = ledger(WORK, MEMORY);
            let policy = policy("");
            let owner = RevisionId::fresh();
            let full = inventory(
                program,
                &policy,
                100,
                owner,
                &mut ledger,
                WorkDomain::Baseline,
            )
            .unwrap();
            let original_bytes = ledger.retained_bytes();
            for limit in [0, 1, 2, full.len() - 1, full.len(), full.len() + 1] {
                let other_owner = RevisionId::fresh();
                let limited = inventory(
                    program,
                    &policy,
                    limit,
                    other_owner,
                    &mut ledger,
                    WorkDomain::Optional,
                )
                .unwrap();
                assert_eq!(limited.len(), limit.min(full.len()));
                assert_eq!(limited.truncated(), limit < full.len());
                for index in 0..limited.len() {
                    assert_eq!(limited.get(index), full.get(index));
                }
                assert_eq!(limited.needs_facts(), limited.len() > 1);
                limited.discard(other_owner, &mut ledger).unwrap();
                assert_eq!(ledger.retained_bytes(), original_bytes);
            }
            full.discard(owner, &mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        });
    }

    #[test]
    fn tactic_filters_avoid_unneeded_facts_and_definition_storage() {
        checked(|program| {
            let mut ledger = ledger(WORK, MEMORY);
            let owner = RevisionId::fresh();
            let scalar_only = policy("[policy.tactics]\ninlining='off'\nconstant-folding='off'\n");
            let scalar = inventory(
                program,
                &scalar_only,
                100,
                owner,
                &mut ledger,
                WorkDomain::Optional,
            )
            .unwrap();
            assert_eq!(scalar.len(), 1);
            assert!(!scalar.needs_facts());
            assert!(scalar.definitions.is_empty());
            scalar.discard(owner, &mut ledger).unwrap();
            let all_off = policy("[policy.tactics]\nscalar-replacement='off'\ninlining='off'\nconstant-folding='off'\n");
            let empty = inventory(
                program,
                &all_off,
                0,
                owner,
                &mut ledger,
                WorkDomain::Optional,
            )
            .unwrap();
            assert_eq!(empty.len(), 0);
            assert!(!empty.truncated());
            assert!(!empty.needs_facts());
            assert_eq!(ledger.retained_bytes(), 0);
            empty.discard(owner, &mut ledger).unwrap();
            let no_pool = policy("[policy.tactics]\nstring-pooling='off'\n");
            let unpooled = inventory(
                program,
                &no_pool,
                100,
                owner,
                &mut ledger,
                WorkDomain::Optional,
            )
            .unwrap();
            for index in 0..unpooled.len() {
                assert!(!matches!(
                    unpooled.get(index),
                    Some(OpportunityView::String {
                        choice: StringChoice::SharedLiteral { .. },
                        ..
                    })
                ));
            }
            unpooled.discard(owner, &mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        });
    }

    #[test]
    fn partial_construction_denial_releases_new_buffers_and_preserves_baseline() {
        checked(|program| {
            let policy = policy("");
            let owner = RevisionId::fresh();
            let mut pilot = ledger(WORK, MEMORY);
            let prefix =
                inventory(program, &policy, 1, owner, &mut pilot, WorkDomain::Optional).unwrap();
            let prefix_work = pilot.work_used(WorkDomain::Optional);
            prefix.discard(owner, &mut pilot).unwrap();
            let one_buffer_bytes =
                (4 * size_of::<Opportunity>() + 4 * size_of::<ValueRef>()) as u64;
            for (work, memory, expected) in [
                (
                    prefix_work,
                    MEMORY,
                    BudgetError::WorkExhausted(WorkDomain::Optional),
                ),
                (
                    WORK,
                    64 + one_buffer_bytes,
                    BudgetError::MemoryExhausted(WorkDomain::Optional),
                ),
            ] {
                let mut ledger = ledger(work, memory);
                ledger.retain(WorkDomain::Baseline, 64).unwrap();
                assert!(
                    matches!(inventory(program, &policy, 100, owner, &mut ledger, WorkDomain::Optional), Err(AllocationError::Budget(error)) if error == expected)
                );
                assert!(
                    ledger.peak_retained_bytes() > 64,
                    "exercise failure after admitted private allocation"
                );
                assert_eq!(ledger.retained_bytes(), 64);
                ledger.release(WorkDomain::Baseline, 64).unwrap();
                assert_eq!(ledger.retained_bytes(), 0);
            }
        });
    }

    #[test]
    fn wrong_owner_returns_intact_inventory_until_original_domain_discard() {
        checked(|program| {
            for domain in [WorkDomain::Baseline, WorkDomain::Optional] {
                let mut ledger = ledger(WORK, MEMORY);
                let owner = RevisionId::fresh();
                let inventory =
                    inventory(program, &policy(""), 100, owner, &mut ledger, domain).unwrap();
                let length = inventory.len();
                let bytes = ledger.retained_bytes();
                let (inventory, error) = inventory
                    .discard(RevisionId::fresh(), &mut ledger)
                    .unwrap_err();
                assert_eq!(error, AllocationError::WrongOwner);
                assert_eq!(inventory.len(), length);
                assert_eq!(ledger.retained_bytes(), bytes);
                inventory.discard(owner, &mut ledger).unwrap();
                assert_eq!(ledger.retained_bytes(), 0);
            }
        });
    }
}
