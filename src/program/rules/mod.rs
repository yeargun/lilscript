//! Program rules (plan M5.1, M6.4a, M7.3, M7.5a, M7.8a; architecture §8.2–§8.4): exact,
//! target-neutral rewrites of the owned program, run once per build after
//! conversion and before any target forms it, to their fixed point.
//! JavaScript, native and every search candidate start from the result.
//!
//! A rule never grows the program (L3): it removes operations, and inlining
//! copies a body only where the copies take no more operations than the body
//! and the calls they replace. Rules read the
//! program's facts (effects and the call graph, initialization order) and the
//! exact values of `values.rs`. They never read a codec, a name plan, a
//! target or the effort level, so the base they leave is the same at every
//! level (B5), and a rule phase is never truncated (§8.2): a round ceiling
//! that is reached is a compiler bug and fails the build.
//!
//! Prior art: Closure's `PhaseOptimizer` loop (`closure-compiler@0da58e1
//! src/com/google/javascript/jscomp/PhaseOptimizer.java:270-277`) and Oxc's
//! `run_in_loop` (`oxc@591966d crates/oxc_minifier/src/compressor.rs:106-140`),
//! here over the typed program instead of the syntax tree.

mod dce;
mod edit;
mod fold;
mod inline;
mod params;
mod unreachable;
mod values;

#[cfg(test)]
mod tests;

use super::call_graph::Seal;
use super::effects::ProgramEffects;
use super::facts::{self, EvaluationBehavior, MemoryAccess};
use super::*;

/// What a build permits the rules: its contract's tactics and sealing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuleRequest {
    /// Literal and branch folding on exact values (M7.8a).
    pub(crate) fold: bool,
    /// Dead operations, dead stores and dead named functions (M5.1).
    pub(crate) dead_code: bool,
    /// Removal-only inlining (M7.5a). It retires what it copies, so it runs
    /// only with `dead_code`.
    pub(crate) inline: bool,
    /// Root storage is sealed only in module execution: a script's root
    /// bindings are globals other scripts may read and write.
    pub(crate) seal: Seal,
}

impl RuleRequest {
    pub(crate) fn any(self) -> bool {
        self.fold || self.dead_code
    }

    fn inlining(self) -> bool {
        self.inline && self.dead_code
    }
}

/// What the rules did, for the build receipt.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct RuleReceipt {
    pub(crate) rounds: u32,
    pub(crate) folded_values: u32,
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
    /// Parameters that left their signatures, those because every call
    /// passed one constant, and results no call used.
    pub(crate) dropped_parameters: u32,
    pub(crate) constant_parameters: u32,
    pub(crate) unused_results: u32,
}

impl RuleReceipt {
    pub(crate) fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "rounds": self.rounds,
            "folded_values": self.folded_values,
            "folded_branches": self.folded_branches,
            "removed_operations": self.removed_operations,
            "removed_stores": self.removed_stores,
            "retired_functions": self.retired_functions,
            "emptied_units": self.emptied_units,
            "unreachable_operations": self.unreachable_operations,
            "inlined_calls": self.inlined_calls,
            "inlined_bodies": self.inlined_bodies,
            "dropped_parameters": self.dropped_parameters,
            "constant_parameters": self.constant_parameters,
            "unused_results": self.unused_results,
        })
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
    Fold,
    Unreachable,
    Inline,
    Parameters,
    DeadCode,
}

/// Every round removes operations or calls and adds no operation, so rounds
/// are bounded by the program's size; inlining a chain of calls takes a
/// round per independent set, logarithmic in its depth. This ceiling only
/// catches a rule that does not converge.
const ROUND_CEILING: u32 = 128;

/// Runs the permitted rules to their fixed point. The result is verified: a
/// rule that leaves an invalid program is a compiler bug, reported as an
/// error rather than delivered.
pub(crate) fn optimize<'src>(
    program: Program<'src>,
    request: RuleRequest,
) -> Result<(Program<'src>, RuleReceipt), String> {
    let mut receipt = RuleReceipt::default();
    if !request.any() {
        return Ok((program, receipt));
    }
    let mut editor = edit::Editor::new(program);
    let mut rules = Vec::with_capacity(4);
    if request.fold {
        rules.push(ProgramRule::Fold);
    }
    // Operations no path reaches go before inlining judges a body's exits.
    if request.dead_code {
        rules.push(ProgramRule::Unreachable);
    }
    if request.inlining() {
        rules.push(ProgramRule::Inline);
    }
    if request.dead_code {
        rules.extend([ProgramRule::Parameters, ProgramRule::DeadCode]);
    }
    receipt.rounds = crate::schedule::fixed_point(
        &mut editor,
        &rules,
        ROUND_CEILING,
        |editor, rule| {
            let effects = editor.program().effects(request.seal);
            let changed = match rule {
                ProgramRule::Fold => {
                    let values =
                        values::ProgramValues::compute(editor.program(), &effects, request.seal);
                    fold::apply(editor, &values, &effects, &mut receipt)
                }
                ProgramRule::Unreachable => unreachable::apply(editor, &mut receipt),
                ProgramRule::Inline => inline::apply(editor, &effects, &mut receipt)
                    .map_err(|error| format!("program rules, inlining: {error}"))?,
                // Unread parameters and unused results are dead code;
                // constant parameters are folding.
                ProgramRule::Parameters => {
                    let values =
                        values::ProgramValues::compute(editor.program(), &effects, request.seal);
                    params::apply(editor, &effects, &values, request.fold, &mut receipt)
                        .map_err(|error| format!("program rules, parameters: {error}"))?
                }
                ProgramRule::DeadCode => dce::apply(editor, &effects, request.seal, &mut receipt),
            };
            editor.commit()?;
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
        |_| "program rules did not reach a fixed point".to_string(),
    )?;
    let program = editor.finish()?;
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
fn behaviors(
    program: &Program<'_>,
    effects: &ProgramEffects,
    unit: UnitId,
    values: Option<&values::ProgramValues>,
) -> Vec<EvaluationBehavior> {
    let data = program.unit(unit).expect("a program unit");
    let initialization = effects.initialization();
    let mut domains = vec![false; data.values.len()];
    let mut behaviors = Vec::with_capacity(data.operations.len());
    for (index, operation) in data.operations.iter().enumerate() {
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
            domains[result.index()] =
                facts::primitive_result_domain(program, data, operation, &domains)
                    || values.is_some_and(|values| values.exact(unit, result).is_some());
        }
        behaviors.push(behavior);
    }
    behaviors
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
pub(super) fn created_units(program: &Program<'_>) -> Vec<bool> {
    let mut created = vec![false; program.units.len()];
    let mut pending: Vec<UnitId> = program
        .units
        .iter()
        .filter(|unit| unit.data().kind == UnitKind::ModuleInitialization)
        .map(FrozenUnit::id)
        .collect();
    for unit in &pending {
        created[unit.index()] = true;
    }
    while let Some(unit) = pending.pop() {
        let data = program.unit(unit).expect("a program unit");
        for operation in &data.operations {
            if let OperationKind::Closure(body) = operation.kind {
                if !std::mem::replace(&mut created[body.index()], true) {
                    pending.push(body);
                }
            }
        }
    }
    created
}
