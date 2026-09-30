//! Reachable target nodes and their lexical execution contexts. Consumers
//! request capture facts only when they need them; both use the same walk.
use super::*;
use crate::compilation_policy::WorkKind::Analysis;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct ReachTree {
    pub(super) expressions: Vec<(ExprId, usize)>,
    pub(super) regions: Vec<RegionId>,
    /// Relative to a sloppy root: bit 1 is sloppy, bit 2 strict. A shared
    /// node reached in both contexts cannot cross a strictness boundary.
    pub(super) strict_regions: Vec<u8>,
    pub(super) strict_expressions: Vec<u8>,
}

pub(super) struct Reach {
    pub(super) expressions: Vec<(ExprId, usize)>,
    pub(super) regions: Vec<RegionId>,
    pub(super) strict_regions: Vec<u8>,
    pub(super) strict_expressions: Vec<u8>,
    /// A reference from a different function can observe or write a binding.
    pub(super) captured: Vec<bool>,
    /// Parameters are initialized before the function's body runs.
    pub(super) parameters: Vec<bool>,
}

struct Captures {
    declared: Vec<Option<Option<FunctionId>>>,
    references: Vec<ReferenceOwners>,
    parameters: Vec<bool>,
}

#[derive(Clone, Copy)]
enum ReferenceOwners {
    None,
    One(Option<FunctionId>),
    Several,
}

impl ReferenceOwners {
    fn add(&mut self, owner: Option<FunctionId>) {
        match *self {
            Self::None => *self = Self::One(owner),
            Self::One(first) if first != owner => *self = Self::Several,
            _ => {}
        }
    }

    fn captured(self, declared: Option<Option<FunctionId>>) -> bool {
        match self {
            Self::None => false,
            Self::One(owner) => declared != Some(owner),
            Self::Several => true,
        }
    }
}

impl Captures {
    fn function(
        &mut self,
        module: &Module,
        function: FunctionId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        let parameters = &module.functions[function.index()].parameters;
        budget.work(Analysis, parameters.len() as u64)?;
        for parameter in parameters {
            self.declared[parameter.index()] = Some(Some(function));
            self.parameters[parameter.index()] = true;
        }
        Ok(())
    }
}

impl Module {
    /// The facts cannot escape their callback. Mutations retain the caller's
    /// allocation owner, while these immutable analysis buffers release on
    /// success, refusal or unwind before the next analysis starts.
    fn with_reach_analysis<Facts, R>(
        &mut self,
        budget: &mut AllocationBudget<'_>,
        prepare: impl FnOnce(&Self, &mut AllocationBudget<'_>) -> Result<Facts, AllocationError>,
        inspect: impl FnOnce(&mut Self, &Facts, &mut AllocationBudget<'_>) -> R,
    ) -> Result<R, AllocationError> {
        use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
        let before = budget.retained_bytes(AllocationClass::Retained);
        let facts = prepare(self, budget)?;
        let bytes = budget.retained_bytes(AllocationClass::Retained) - before;
        let outcome = catch_unwind(AssertUnwindSafe(|| inspect(self, &facts, budget)));
        drop(facts);
        budget.release(AllocationClass::Retained, bytes)?;
        match outcome {
            Ok(result) => Ok(result),
            Err(payload) => resume_unwind(payload),
        }
    }

    pub(super) fn with_reach<R>(
        &mut self,
        budget: &mut AllocationBudget<'_>,
        inspect: impl FnOnce(&mut Self, &Reach, &mut AllocationBudget<'_>) -> R,
    ) -> Result<R, AllocationError> {
        self.with_reach_analysis(budget, Self::reach, inspect)
    }

    pub(super) fn with_reach_tree<R>(
        &mut self,
        budget: &mut AllocationBudget<'_>,
        inspect: impl FnOnce(&mut Self, &ReachTree, &mut AllocationBudget<'_>) -> R,
    ) -> Result<R, AllocationError> {
        self.with_reach_analysis(budget, Self::reach_tree, inspect)
    }

    pub(super) fn reach(
        &self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Reach, AllocationError> {
        let mut phase = budget.scope();
        let captures = Captures {
            declared: phase.filled(AllocationClass::Scratch, self.bindings.len(), None)?,
            references: phase.filled(
                AllocationClass::Scratch,
                self.bindings.len(),
                ReferenceOwners::None,
            )?,
            parameters: phase.filled(AllocationClass::Retained, self.bindings.len(), false)?,
        };
        let (tree, captures) = self.walk_reach::<true>(Some(captures), &mut phase)?;
        let captures = captures.unwrap();
        let mut captured = phase.vector(AllocationClass::Retained, self.bindings.len())?;
        phase.work(Analysis, captures.references.len() as u64)?;
        for (&owners, &declared) in captures.references.iter().zip(&captures.declared) {
            // An undeclared binding (an import or host) counts as captured.
            captured.push(owners.captured(declared));
        }
        let Captures {
            declared,
            references,
            parameters,
        } = captures;
        drop((declared, references));
        phase.finish_retained()?;
        let ReachTree {
            expressions,
            regions,
            strict_regions,
            strict_expressions,
        } = tree;
        Ok(Reach {
            expressions,
            regions,
            strict_regions,
            strict_expressions,
            captured,
            parameters,
        })
    }

    pub(super) fn reach_tree(
        &self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<ReachTree, AllocationError> {
        self.walk_reach::<false>(None, budget).map(|(tree, _)| tree)
    }

    fn walk_reach<const CAPTURES: bool>(
        &self,
        captures: Option<Captures>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(ReachTree, Option<Captures>), AllocationError> {
        let mut phase = budget.scope();
        // Capture buffers keep their outer owner; this walk never grows them.
        let mut captures = captures;
        let mut reach = ReachTree {
            // Each node enters its result once, even if reached in both
            // strictness contexts. Admit those bounds once, before the walk.
            expressions: phase.vector(AllocationClass::Retained, self.expressions.len())?,
            regions: phase.vector(AllocationClass::Retained, self.regions.len())?,
            strict_regions: phase.filled(AllocationClass::Retained, self.regions.len(), 0)?,
            strict_expressions: phase.filled(
                AllocationClass::Retained,
                self.expressions.len(),
                0,
            )?,
        };
        let mut regions = Vec::new();
        phase.push(
            AllocationClass::Scratch,
            &mut regions,
            (self.root, 0usize, None::<FunctionId>, false),
        )?;
        let mut pending: Vec<(ExprId, usize, bool)> = Vec::new();
        while let Some((region, depth, owner, strict)) = regions.pop() {
            phase.work(Analysis, 1)?;
            let mask = if strict { 2 } else { 1 };
            let seen = reach.strict_regions[region.index()];
            if seen & mask != 0 {
                continue;
            }
            reach.strict_regions[region.index()] |= mask;
            if seen == 0 {
                reach.regions.push(region);
            }
            for statement in &self.regions[region.index()].statements {
                phase.work(Analysis, 1)?;
                if CAPTURES {
                    let declared = &mut captures.as_mut().unwrap().declared;
                    match statement {
                        Statement::Let { binding, .. }
                        | Statement::ForIn { binding, .. }
                        | Statement::ForOf { binding, .. }
                        | Statement::Function { binding, .. } => {
                            declared[binding.index()] = Some(owner)
                        }
                        Statement::Try {
                            catch:
                                Some(Catch {
                                    binding: Some(binding),
                                    ..
                                }),
                            ..
                        } => declared[binding.index()] = Some(owner),
                        _ => {}
                    }
                }
                // Statement visitors are infallible. Admit their exact push
                // counts before visiting, without an uncharged staging list.
                let mut roots = 0;
                statement.visit_expressions(|_| roots += 1);
                phase.work(Analysis, roots as u64)?;
                phase.reserve_vec(AllocationClass::Scratch, &mut pending, roots)?;
                statement.visit_expressions(|root| pending.push((root, depth + 1, strict)));
                let mut children = 0;
                statement.visit_regions(|_| children += 1);
                phase.work(Analysis, children as u64)?;
                phase.reserve_vec(AllocationClass::Scratch, &mut regions, children)?;
                statement.visit_regions(|child| regions.push((child, depth + 1, owner, strict)));
                if let Statement::Function { function, .. } = statement {
                    if CAPTURES {
                        captures
                            .as_mut()
                            .unwrap()
                            .function(self, *function, &mut phase)?;
                    }
                    phase.push(
                        AllocationClass::Scratch,
                        &mut regions,
                        (
                            self.functions[function.index()].body,
                            depth + 2,
                            Some(*function),
                            strict || self.functions[function.index()].strict,
                        ),
                    )?;
                }
                while let Some((id, at, strict)) = pending.pop() {
                    phase.work(Analysis, 1)?;
                    let expression = &self.expressions[id.index()];
                    let strict = strict || matches!(expression, Expr::Class { .. });
                    let mask = if strict { 2 } else { 1 };
                    let seen = reach.strict_expressions[id.index()];
                    if seen & mask != 0 {
                        continue;
                    }
                    reach.strict_expressions[id.index()] |= mask;
                    if seen == 0 {
                        reach.expressions.push((id, at));
                    }
                    if CAPTURES {
                        if let Expr::Binding(binding) = expression {
                            captures.as_mut().unwrap().references[binding.index()].add(owner);
                        }
                    }
                    for function in expression.created_functions() {
                        if CAPTURES {
                            captures
                                .as_mut()
                                .unwrap()
                                .function(self, function, &mut phase)?;
                        }
                        phase.push(
                            AllocationClass::Scratch,
                            &mut regions,
                            (
                                self.functions[function.index()].body,
                                at + 2,
                                Some(function),
                                strict || self.functions[function.index()].strict,
                            ),
                        )?;
                    }
                    expression.visit_children(|child| {
                        phase.push(
                            AllocationClass::Scratch,
                            &mut pending,
                            (child, at + 1, strict),
                        )
                    })?;
                }
            }
        }
        drop((regions, pending));
        phase.finish_retained()?;
        Ok((reach, captures))
    }
}
