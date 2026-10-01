//! Program rules (plan M5.1, M6.4a, M7.3, M7.5a, M7.8a; architecture §8.2–§8.4): exact,
//! target-neutral rewrites of the owned program, run once per build after
//! conversion and before any target forms it, to their fixed point.
//! JavaScript, native and every search candidate start from the result.
//!
//! Primitive rules remove operations; inlining copies a body only where the
//! copies take no more operations than the body and calls they replace.
//! Aggregate rules monotonically remove aggregate storage, aliases or stores;
//! a scalar bank may need more initializers while eliminating its allocation.
//! Rules use shared effects, calls, initialization, values and the build's
//! boundary/storage constraints. They never read a codec, name plan or effort
//! level, so the base they leave is the same at every effort level (B5).
//! A rule phase is never truncated (§8.2): a round ceiling
//! that is reached is a compiler bug and fails the build.
//!
//! Prior art: Closure's `PhaseOptimizer` loop (`closure-compiler@0da58e1
//! src/com/google/javascript/jscomp/PhaseOptimizer.java:270-277`) and Oxc's
//! `run_in_loop` (`oxc@591966d crates/oxc_minifier/src/compressor.rs:106-140`),
//! here over the typed program instead of the syntax tree.

mod aggregates;
mod dce;
mod defaults;
mod dirty;
mod edit;
mod evaluate;
mod fold;
mod forward;
mod inline;
mod inline_clones;
mod params;
mod returns;
mod storage;
mod unreachable;
mod values;

#[cfg(test)]
mod admission_tests;
#[cfg(test)]
mod aggregate_tests;
#[cfg(test)]
mod call_tests;
#[cfg(test)]
mod tests;

use super::call_graph::Seal;
use super::effects::ProgramEffects;
use super::facts::{self, EvaluationBehavior, MemoryAccess};
use super::*;

/// What a build permits the rules: its contract's tactics and sealing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuleRequest {
    /// Physical reuse of proven stable local normalization inputs.
    pub(crate) reuse_normalization: bool,
    /// Permission for explicit source loop expansion during conversion.
    pub(crate) unroll: bool,
    /// JavaScript permission for authored sharing; native constants already share backing.
    pub(crate) pool: bool,
    pub(crate) choices: crate::representation::RegionalChoices,
    /// Literal and branch folding on exact values (M7.8a).
    pub(crate) fold: bool,
    /// Dead operations, dead stores and dead named functions (M5.1).
    pub(crate) dead_code: bool,
    /// Removal-only inlining (M7.5a). It retires what it copies, so it runs
    /// only with `dead_code`.
    pub(crate) inline: bool,
    /// Private aggregate storage and copy elision, independently permitted.
    pub(crate) scalar: bool,
    /// At least one requested artifact uses native captured-cell storage.
    pub(crate) native: bool,
    /// The target contract guarantees original builtin method behavior.
    pub(crate) pristine_builtins: bool,
    /// Modules and application scripts own private roots. An explicitly
    /// open root boundary retains structural-only storage evidence.
    pub(crate) seal: Seal,
}

impl RuleRequest {
    pub(crate) fn any(self) -> bool {
        self.fold || self.dead_code || self.scalar
    }

    fn inlining(self) -> bool {
        self.inline && self.dead_code
    }
}

/// What the rules did, for the build receipt.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct RuleReceipt {
    pub(crate) local_units_visited: u64,
    pub(crate) local_units_reused: u64,
    pub(crate) effect_units_visited: u64,
    pub(crate) effect_units_reused: u64,
    pub(crate) effect_components_reused: u64,
    pub(crate) rounds: u32,
    pub(crate) folded_values: u32,
    /// Calls replaced by exact constants, including bounded evaluation.
    pub(crate) folded_calls: u32,
    pub(crate) folded_branches: u32,
    pub(crate) removed_operations: u32,
    pub(crate) removed_stores: u32,
    pub(crate) retired_functions: u32,
    /// Bodies nothing creates any more, emptied: they use nothing.
    pub(crate) emptied_units: u32,
    /// Operations no path reaches, removed.
    pub(crate) unreachable_operations: u32,
    /// Calls replaced by a copy of their body, and the bodies copied.
    pub(crate) inlined_calls: u32,
    pub(crate) inlined_bodies: u32,
    pub(crate) normalized_returns: u32,
    pub(crate) cloned_closure_units: u32,
    pub(crate) call_frequency_work: u64,
    /// Parameters that left their signatures, those because every call
    /// passed one constant, and results no call used.
    pub(crate) dropped_parameters: u32,
    pub(crate) constant_parameters: u32,
    pub(crate) unused_results: u32,
    /// Immutable cell loads replaced by their reaching initializer value.
    pub(crate) forwarded_definitions: u32,
    /// Repeated total primitive operations replaced by a dominating result.
    pub(crate) common_computations: u32,
    pub(crate) default_arguments_omitted: u32,
    pub(crate) materialized_default_arguments: u32,
    pub(crate) native_defaults: u32,
    pub(crate) set_folds: u32,
    pub(crate) path_folds: u32,
    pub(crate) evaluated_calls: u32,
    pub(crate) evaluation_refusals: [u32; 6],
    pub(crate) default_checks_removed: u32,
    pub(crate) exposed_allocations: u32,
    pub(crate) flattened_namespace_calls: u32,
    pub(crate) folded_fields: u32,
    pub(crate) removed_field_stores: u32,
    pub(crate) collected_field_stores: u32,
    pub(crate) scalarized_allocations: u32,
    pub(crate) scalar_fields: u32,
    pub(crate) elided_record_aliases: u32,
    pub(crate) aggregate_analysis_work: u64,
    pub(crate) aggregate_limits: u32,
}

impl RuleReceipt {
    fn observe_effects(&mut self, effects: &ProgramEffects, previous: &mut Option<RevisionId>) {
        let identity = effects.analysis_identity();
        if *previous != Some(identity) {
            let (visited, reused, components) = effects.reuse_stats();
            self.effect_units_visited += visited;
            self.effect_units_reused += reused;
            self.effect_components_reused += components;
            *previous = Some(identity);
        }
    }

    pub(crate) fn json(&self) -> serde_json::Value {
        let mut result = serde_json::json!({
            "rounds": self.rounds,
            "folded_values": self.folded_values,
            "folded_calls": self.folded_calls,
            "folded_branches": self.folded_branches,
            "removed_operations": self.removed_operations,
            "removed_stores": self.removed_stores,
            "retired_functions": self.retired_functions,
            "emptied_units": self.emptied_units,
            "unreachable_operations": self.unreachable_operations,
            "inlined_calls": self.inlined_calls,
            "inlined_bodies": self.inlined_bodies,
            "normalized_returns": self.normalized_returns,
            "cloned_closure_units": self.cloned_closure_units,
            "dropped_parameters": self.dropped_parameters,
            "constant_parameters": self.constant_parameters,
            "unused_results": self.unused_results,
            "forwarded_definitions": self.forwarded_definitions,
            "common_computations": self.common_computations,
            "default_arguments_omitted": self.default_arguments_omitted,
            "native_defaults": self.native_defaults,
            "set_folds": self.set_folds,
            "path_folds": self.path_folds,
            "evaluated_calls": self.evaluated_calls,
            "default_checks_removed": self.default_checks_removed,
            "exposed_allocations": self.exposed_allocations,
            "flattened_namespace_calls": self.flattened_namespace_calls,
            "folded_fields": self.folded_fields,
            "removed_field_stores": self.removed_field_stores,
            "collected_field_stores": self.collected_field_stores,
            "scalarized_allocations": self.scalarized_allocations,
            "scalar_fields": self.scalar_fields,
            "elided_record_aliases": self.elided_record_aliases,
            "aggregate_analysis_work": self.aggregate_analysis_work,
            "aggregate_limits": self.aggregate_limits,
            "evaluation_refusals": {
                "host": self.evaluation_refusals[0],
                "engine_dependent": self.evaluation_refusals[1],
                "aggregate": self.evaluation_refusals[2],
                "observable": self.evaluation_refusals[3],
                "unsupported": self.evaluation_refusals[4],
                "limit": self.evaluation_refusals[5],
            },
        });
        result["call_frequency_work"] = self.call_frequency_work.into();
        result["materialized_default_arguments"] = self.materialized_default_arguments.into();
        result["local_units_visited"] = self.local_units_visited.into();
        result["local_units_reused"] = self.local_units_reused.into();
        result["effect_units_visited"] = self.effect_units_visited.into();
        result["effect_units_reused"] = self.effect_units_reused.into();
        result["effect_components_reused"] = self.effect_components_reused.into();
        result
    }
}

/// Types whose values own no storage a scope releases: numbers, booleans,
/// enums, and host values (`JsValue`), which only JavaScript's collected
/// heap carries (a native build refuses them).
fn scalar(program: &Program<'_>, ty: TypeId) -> bool {
    matches!(
        program.ty(ty),
        Some(
            crate::check::Type::Int
                | crate::check::Type::Float
                | crate::check::Type::Bool
                | crate::check::Type::Enum(_)
                | crate::check::Type::Dynamic
        )
    )
}

/// The program rules, in their structural order (architecture §8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProgramRule {
    Defaults,
    Returns,
    Aggregates,
    Forward,
    Fold,
    Unreachable,
    Inline,
    Parameters,
    DeadCode,
}

/// Default preparation fills a missing argument once; the other rules remove
/// operations, calls or owned aggregate storage. This one-way progress makes rounds
/// are bounded by the program's size; inlining a chain of calls takes a
/// round per independent set, logarithmic in its depth. This ceiling only
/// catches a rule that does not converge.
const ROUND_CEILING: u32 = 128;

#[cfg(test)]
thread_local! {
    /// Audit only: force the former cold behavior without changing decisions.
    static COLD_RULE_VIEWS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Runs the permitted rules to their fixed point. The result is verified: a
/// rule that leaves an invalid program is a compiler bug, reported as an
/// error rather than delivered.
pub(crate) fn optimize<'src>(
    program: Program<'src>,
    request: RuleRequest,
) -> Result<(Program<'src>, RuleReceipt), String> {
    optimize_admitted(
        program,
        request,
        &mut crate::output_budget::AllocationBudget::new(None),
    )
    .map_err(|error| error.to_string())
}

#[derive(Debug)]
pub(crate) enum RuleError {
    Invalid(String),
    Allocation(crate::output_budget::AllocationError),
}
impl From<String> for RuleError {
    fn from(value: String) -> Self {
        Self::Invalid(value)
    }
}
impl From<&str> for RuleError {
    fn from(value: &str) -> Self {
        Self::Invalid(value.into())
    }
}
impl From<crate::output_budget::AllocationError> for RuleError {
    fn from(value: crate::output_budget::AllocationError) -> Self {
        Self::Allocation(value)
    }
}
impl std::fmt::Display for RuleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
            Self::Allocation(error) => error.fmt(formatter),
        }
    }
}

pub(crate) fn optimize_admitted<'src>(
    mut program: Program<'src>,
    request: RuleRequest,
    budget: &mut crate::output_budget::AllocationBudget<'_>,
) -> Result<(Program<'src>, RuleReceipt), RuleError> {
    let mut dirty =
        dirty::DirtyUnits::new(request.reuse_normalization && crate::schedule::reuses_stability());
    let mut receipt = RuleReceipt::default();
    if !request.any() {
        return Ok((program, receipt));
    }
    program.views.discard_in(budget)?;
    program.views = views::ProgramViews::normalization(
        request.reuse_normalization && crate::schedule::reuses_stability(),
    );
    let mut last_effects = None;
    let mut editor = edit::Editor::new_in(program, budget)?;
    // These are target-boundary permissions, not changed source semantics.
    // Recompute them after structural edits, so a second optimization cannot
    // remove a retained guard on the strength of its own omitted arguments.
    for unit in 0..editor.program().units.len() {
        let id = UnitId::from_index(unit).expect("a unit index");
        let data = editor.program().unit(id).expect("a program unit");
        if data.native_default_length.is_some()
            || data.calls.iter().any(|site| site.omit_trailing != 0)
        {
            let data = editor.unit_mut_in(id, budget)?;
            data.native_default_length = None;
            for site in &mut data.calls {
                site.omit_trailing = 0;
            }
        }
    }
    editor.commit_in(budget)?;
    let mut rules = [ProgramRule::Defaults; 9];
    let mut count = 0;
    let mut push = |rule| {
        rules[count] = rule;
        count += 1;
    };
    if request.fold {
        push(ProgramRule::Defaults);
        push(ProgramRule::Forward);
    }
    if request.fold {
        push(ProgramRule::Fold);
    }
    // Operations no path reaches go before inlining judges a body's exits.
    if request.dead_code {
        push(ProgramRule::Unreachable);
    }
    if request.inlining() {
        push(ProgramRule::Returns);
        push(ProgramRule::Inline);
    }
    if request.fold || request.dead_code || request.scalar {
        push(ProgramRule::Aggregates);
    }
    if request.dead_code {
        push(ProgramRule::Parameters);
        push(ProgramRule::DeadCode);
    }
    receipt.rounds = crate::schedule::fixed_point(
        &mut editor,
        &rules[..count],
        ROUND_CEILING,
        |editor, rule| {
            // These normalizers read only UnitData. Building a whole-program
            // effect graph first would defeat their local dependency contract.
            if matches!(rule, ProgramRule::Returns | ProgramRule::Unreachable) {
                let created = if matches!(rule, ProgramRule::Unreachable) {
                    Some(created_units_in(editor.program(), budget)?)
                } else {
                    None
                };
                let changed = match rule {
                    ProgramRule::Returns => {
                        returns::apply(editor, &mut dirty, &mut receipt, budget)?
                    }
                    _ => unreachable::apply(
                        editor,
                        &mut dirty,
                        created.as_deref().unwrap(),
                        &mut receipt,
                        budget,
                    )?,
                };
                if let Some(created) = created {
                    storage::release_vec(
                        created,
                        crate::output_budget::AllocationClass::Retained,
                        budget,
                    )?;
                }
                editor.commit_in(budget)?;
                return Ok(changed);
            }
            let effects = editor.program().effects_in(request.seal, budget)?;
            receipt.observe_effects(&effects, &mut last_effects);
            if matches!(
                rule,
                ProgramRule::Forward
                    | ProgramRule::Fold
                    | ProgramRule::Inline
                    | ProgramRule::DeadCode
            ) {
                // These transfers consume the immutable class view throughout
                // their planning phase; mutation commits invalidate it below.
                drop(
                    editor
                        .program()
                        .primitive_classes_in(request.seal, budget)?,
                );
            }
            let changed = match rule {
                ProgramRule::Defaults => {
                    defaults::apply(editor, effects.graph(), &mut receipt, budget)?
                }
                ProgramRule::Returns | ProgramRule::Unreachable => unreachable!(),
                ProgramRule::Aggregates => {
                    aggregates::apply(editor, &effects, request, &mut receipt, budget)?
                }
                ProgramRule::Forward => forward::apply(editor, &effects, &mut receipt, budget)?,
                ProgramRule::Fold => budget.with_temporary_context(
                    editor,
                    |editor, budget| {
                        Ok::<_, RuleError>(values::ProgramValues::compute_in(
                            editor.program(),
                            &effects,
                            request.seal,
                            request.pristine_builtins,
                            budget,
                        )?)
                    },
                    |values, editor, budget| {
                        receipt.evaluated_calls =
                            receipt.evaluated_calls.saturating_add(values.evaluated);
                        for (total, count) in
                            receipt.evaluation_refusals.iter_mut().zip(values.refused)
                        {
                            *total = total.saturating_add(count);
                        }
                        fold::apply(editor, values, &effects, &mut receipt, budget)
                    },
                )?,
                ProgramRule::Inline => {
                    inline::apply(editor, &effects, &mut receipt, request.native, budget)?
                }
                // Unread parameters and unused results are dead code;
                // constant parameters are folding.
                ProgramRule::Parameters => budget.with_temporary_context(
                    editor,
                    |editor, budget| {
                        Ok::<_, RuleError>(values::ProgramValues::compute_in(
                            editor.program(),
                            &effects,
                            request.seal,
                            request.pristine_builtins,
                            budget,
                        )?)
                    },
                    |values, editor, budget| {
                        params::apply(editor, &effects, values, request.fold, &mut receipt, budget)
                    },
                )?,
                ProgramRule::DeadCode => {
                    dce::apply(editor, &effects, request.seal, &mut receipt, budget)?
                }
            };
            drop(effects);
            editor.commit_in(budget)?;
            Ok(changed)
        },
        |editor, round| {
            if cfg!(debug_assertions) {
                editor
                    .program()
                    .verify()
                    .map_err(|error| format!("program rules, round {round}: {error}"))?;
            }
            Ok(())
        },
        |_| RuleError::Invalid("program rules did not reach a fixed point".into()),
    )?;
    // Choose target default transport after structural folding has removed
    // redundant guards. The remaining explicit guards must survive for the
    // target calls that omit their literal arguments.
    if request.fold {
        let effects = editor.program().effects_in(request.seal, budget)?;
        receipt.observe_effects(&effects, &mut last_effects);
        budget.with_temporary_context(
            &mut editor,
            |editor, budget| {
                Ok::<_, RuleError>(super::defaults::plan_in(
                    editor.program(),
                    effects.graph(),
                    budget,
                )?)
            },
            |plan, editor, budget| {
                for &(unit, call, omitted) in &plan.calls {
                    editor.unit_mut_in(unit, budget)?.calls[call.index()].omit_trailing = omitted;
                    receipt.default_arguments_omitted += omitted;
                }
                for &(unit, length) in &plan.lengths {
                    editor.unit_mut_in(unit, budget)?.native_default_length = length;
                    receipt.native_defaults += u32::from(length.is_some());
                }
                Ok(())
            },
        )?;
        drop(effects);
        editor.commit_in(budget)?;
    }
    let program = editor.finish_in(budget)?;
    dirty.release(budget)?;
    program
        .verify()
        .map_err(|error| format!("program rules left an invalid program: {error}"))?;
    Ok((program, receipt))
}

/// What evaluating each operation of `unit` may do, in arena order, with the
/// program's summaries. A cell load the initialization owner proves past the
/// cell's initialization cannot fail: the temporal dead zone is a passive
/// load's only failure (as `facts.rs` refines it). A value with exact
/// knowledge is a primitive, so it runs no conversion hook.
fn behaviors_in(
    program: &Program<'_>,
    effects: &ProgramEffects,
    unit: UnitId,
    values: Option<&values::ProgramValues>,
    budget: &mut crate::output_budget::AllocationBudget<'_>,
) -> Result<Vec<EvaluationBehavior>, crate::output_budget::AllocationError> {
    use crate::output_budget::AllocationClass::{Retained, Scratch};
    budget.retained_phase(|budget| {
        let data = program.unit(unit).expect("a program unit");
        let initialization = effects.initialization();
        let classes = program.primitive_classes(effects.graph().seal());
        let mut domains = storage::collect(
            (0..data.values.len()).map(|index| {
                classes
                    .value(unit, ValueId::from_index(index).unwrap())
                    .primitive()
            }),
            Scratch,
            budget,
        )?;
        let mut behaviors = budget.vector(Retained, data.operations.len())?;
        for (index, operation) in data.operations.iter().enumerate() {
            budget.work(
                crate::compilation_policy::WorkKind::Analysis,
                data.operands(operation.operands)
                    .map_or(0, |values| values.len()) as u64
                    + 1,
            )?;
            let mut behavior = facts::operation_evaluation_behavior(
                program,
                Some(effects),
                unit,
                data,
                operation,
                &domains,
            );
            if let OperationKind::Load(place) = operation.kind {
                if let Some(&Place::Cell(cell)) = data.places.get(place.index()) {
                    let passive = EvaluationBehavior {
                        reads: behavior.reads,
                        may_throw: true,
                        ..EvaluationBehavior::TOTAL
                    };
                    if behavior == passive
                        && matches!(behavior.reads, MemoryAccess::Cell(_))
                        && initialization.initialized(
                            program,
                            unit,
                            OpId::from_index(index).unwrap(),
                            cell,
                        )
                    {
                        behavior.may_throw = false;
                    }
                }
            }
            if let Some(result) = operation.result {
                domains[result.index()] |=
                    facts::primitive_result_domain(program, data, operation, &domains)
                        || values.is_some_and(|values| values.exact(unit, result).is_some());
            }
            behaviors.push(behavior);
        }
        Ok(behaviors)
    })
}

/// The operations of a module initializer's instantiation prefix: the named
/// function pairs, which only the dead-code rule may retire, as pairs.
fn prefix(data: &UnitData) -> &[OpId] {
    let operations = &data.regions[data.entry.index()].operations;
    &operations[..(data.instantiation_prefix as usize).min(operations.len())]
}

/// Units something creates: every module initializer, and every body a
/// `Closure` of a created unit names (a named function's is its prefix pair).
/// A body nothing creates never runs; its operations are not evidence.
#[cfg(test)]
pub(super) fn created_units(program: &Program<'_>) -> Vec<bool> {
    created_units_in(
        program,
        &mut crate::output_budget::AllocationBudget::new(None),
    )
    .expect("inspection created bodies")
}

pub(super) fn created_units_in(
    program: &Program<'_>,
    budget: &mut crate::output_budget::AllocationBudget<'_>,
) -> Result<Vec<bool>, crate::output_budget::AllocationError> {
    budget.retained_phase(|budget| {
        created_units_in_class(
            program,
            crate::output_budget::AllocationClass::Retained,
            budget,
        )
    })
}
pub(super) fn created_units_in_class(
    program: &Program<'_>,
    class: crate::output_budget::AllocationClass,
    budget: &mut crate::output_budget::AllocationBudget<'_>,
) -> Result<Vec<bool>, crate::output_budget::AllocationError> {
    use crate::output_budget::AllocationClass::Scratch;
    let mut created = budget.filled(class, program.units.len(), false)?;
    let mut pending = budget.vector(Scratch, program.units.len())?;
    for unit in &program.units {
        budget.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
        if unit.data().kind == UnitKind::ModuleInitialization {
            created[unit.id().index()] = true;
            pending.push(unit.id());
        }
    }
    while let Some(unit) = pending.pop() {
        let data = program.unit(unit).expect("a program unit");
        for operation in &data.operations {
            budget.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
            if let OperationKind::Closure(body) = operation.kind {
                if !std::mem::replace(&mut created[body.index()], true) {
                    pending.push(body);
                }
            }
        }
    }
    storage::release_vec(pending, Scratch, budget)?;
    Ok(created)
}
