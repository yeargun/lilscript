//! A sparse physical output choice supplied by the trusted Formation owner.
//! The target verifies locations and syntax; it does not infer weak source uses.
use super::{extract::OutputError, verify::Structure, Expr, ExprId, Literal, Module};
use crate::compilation_policy::WorkKind;
use crate::output_budget::AllocationBudget;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum LiteralOutput {
    #[default]
    Original,
    Observed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WeakLiteralObservation {
    Truthy,
    Nullish,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LiteralAlternative {
    expression: ExprId,
    observation: WeakLiteralObservation,
}
impl LiteralAlternative {
    /// Only trusted Formation supplies the discharged source observation. A
    /// public target Module cannot attach this certificate through inspection.
    pub(crate) fn new(expression: ExprId, observation: WeakLiteralObservation) -> Self {
        Self {
            expression,
            observation,
        }
    }

    pub(crate) fn expression(&self) -> ExprId {
        self.expression
    }

    /// Follow a renumbered arena; false when the literal no longer exists.
    pub(crate) fn remap(&mut self, map: &[Option<ExprId>]) -> bool {
        match map.get(self.expression.index()).copied().flatten() {
            Some(expression) => {
                self.expression = expression;
                true
            }
            None => false,
        }
    }
}

/// Borrow the verifier's existing liveness before its temporary Structure drops.
/// Keep dead rows in their admitted owner; no filtered vector or graph is built.
pub(super) fn validate(
    module: &Module,
    rows: &[LiteralAlternative],
    structure: &Structure,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, OutputError> {
    let mut previous = None;
    let mut live = false;
    for row in rows {
        budget.work(WorkKind::Analysis, 1)?;
        if previous.is_some_and(|old| old >= row.expression)
            || !matches!(
                module.expressions.get(row.expression.index()),
                Some(Expr::Literal(Literal::String(_)))
            )
        {
            return Err(OutputError::Invalid("invalid observed literal occurrence"));
        }
        previous = Some(row.expression);
        live |= structure.live_expressions[row.expression.index()];
    }
    Ok(live)
}

/// One comparison per charged step; the borrowed output payload remains sorted.
/// Error type belongs to the actual renderer/inspection admission client.
pub(super) fn lookup<E>(
    rows: &[LiteralAlternative],
    expression: ExprId,
    mut work: impl FnMut(usize) -> Result<(), E>,
) -> Result<Option<WeakLiteralObservation>, E> {
    let (mut low, mut high) = (0, rows.len());
    while low < high {
        work(1)?;
        let middle = low + (high - low) / 2;
        match rows[middle].expression.cmp(&expression) {
            std::cmp::Ordering::Less => low = middle + 1,
            std::cmp::Ordering::Greater => high = middle,
            std::cmp::Ordering::Equal => return Ok(Some(rows[middle].observation)),
        }
    }
    Ok(None)
}
