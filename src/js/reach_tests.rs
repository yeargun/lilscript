use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain, WorkKind::Analysis,
};
use crate::js::reach::ReachTree;

// Preserved pre-partition traversal: an independent oracle for both requests.
struct OldReach {
    expressions: Vec<(ExprId, usize)>,
    regions: Vec<RegionId>,
    captured: Vec<bool>,
    strict_regions: Vec<u8>,
    strict_expressions: Vec<u8>,
    parameters: Vec<bool>,
}

fn ledger(work: u64, bytes: u64) -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: work,
            optional_work: 0,
            baseline_retained_bytes: 0,
            retained_bytes: bytes,
        },
    )
    .unwrap()
}

fn backing<T>(values: &Vec<T>) -> u64 {
    (values.capacity() * std::mem::size_of::<T>()) as u64
}

fn tree_backing(tree: &ReachTree) -> u64 {
    backing(&tree.bindings)
        + backing(&tree.expressions)
        + backing(&tree.regions)
        + backing(&tree.strict_regions)
        + backing(&tree.strict_expressions)
}

fn compare(module: &Module) {
    let old = module
        .reach_before_partition(&mut AllocationBudget::new(None))
        .unwrap();
    let mut ledger = ledger(100_000_000, 10_000_000);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let full = module.reach(&mut budget).unwrap();
        assert_eq!(full.expressions, old.expressions);
        assert_eq!(full.regions, old.regions);
        assert_eq!(full.strict_regions, old.strict_regions);
        assert_eq!(full.strict_expressions, old.strict_expressions);
        assert_eq!(full.captured, old.captured);
        assert_eq!(full.parameters, old.parameters);
        let expected = backing(&full.bindings)
            + backing(&full.expressions)
            + backing(&full.regions)
            + backing(&full.strict_regions)
            + backing(&full.strict_expressions)
            + backing(&full.captured)
            + backing(&full.parameters);
        assert_eq!(budget.retained_bytes(AllocationClass::Retained), expected);
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        drop(full);
    }
    assert_eq!(ledger.retained_bytes(), 0);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let tree = module.reach_tree(&mut budget).unwrap();
        assert_eq!(tree.expressions, old.expressions);
        assert_eq!(tree.regions, old.regions);
        assert_eq!(tree.strict_regions, old.strict_regions);
        assert_eq!(tree.strict_expressions, old.strict_expressions);
        assert_eq!(
            budget.retained_bytes(AllocationClass::Retained),
            tree_backing(&tree)
        );
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        drop(tree);
    }
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn target_storage_facts_invalidate_aliases_and_track_nonexpression_writes() {
    let mut module = Module::default();
    let root = module.root;
    let source = binding(&mut module, root, 0, "source");
    let alias = binding(&mut module, root, 1, "alias");
    let imported = binding(&mut module, root, 2, "imported");
    let published = binding(&mut module, root, 3, "published");
    let caught = binding(&mut module, root, 4, "caught");
    let array = expr(&mut module, Expr::Array(Vec::new()));
    let read = expr(&mut module, Expr::Binding(source));
    module.regions[root.index()].statements.extend([
        Statement::Let { binding: source, value: Some(array) },
        Statement::Let { binding: alias, value: Some(read) },
    ]);
    module.imports.push(Import { source: "host".into(), imported: "value".into(), binding: imported });
    module.exports.push(Export { binding: published, name: "value".into() });
    let body = module.region(module.regions[root.index()].scope);
    module.regions[root.index()].statements.push(Statement::Try {
        body, catch: Some(Catch { binding: Some(caught), body }), finally: None,
    });
    let first = module.reach_tree(&mut AllocationBudget::new(None)).unwrap();
    assert!(first.bindings[source.index()].fixed());
    assert!(first.bindings[source.index()].literal_array());
    assert!(!first.bindings[imported.index()].fixed());
    assert!(!first.bindings[published.index()].fixed());
    assert!(first.bindings[caught.index()].implicit_writes);
    // The next pass must see the new write; it cannot forward the old alias.
    let target = expr(&mut module, Expr::Binding(source));
    let value = number(&mut module, 7.0);
    let write = expr(&mut module, Expr::Assign { target, value });
    module.regions[root.index()].statements.push(Statement::Evaluate(write));
    let next = module.reach_tree(&mut AllocationBudget::new(None)).unwrap();
    assert!(!next.bindings[source.index()].fixed());
    assert!(!next.bindings[source.index()].literal_array());
    assert_eq!(next.bindings[source.index()].assignments, 1);
    assert_eq!(module.eliminate_aliases(&mut AllocationBudget::new(None)).unwrap(), 0);
    // Unreachable arena writes do not poison the reachable storage proof.
    module.regions[root.index()].statements.pop();
    assert_eq!(module.eliminate_aliases(&mut AllocationBudget::new(None)).unwrap(), 1);
}

#[test]
fn target_reach_requests_match_the_original_capture_and_strictness_walk() {
    compare(&Module::default());
    compare(&single_call::sites().0);
    for kind in [
        strict_inline::Kind::Expression,
        strict_inline::Kind::Statements,
        strict_inline::Kind::Block,
    ] {
        for nested in [false, true] {
            for explicit in [false, true] {
                compare(&strict_inline::fixture(kind, nested, explicit));
            }
        }
    }
    let (mut module, _) = single_call::sites();
    let root = module.root;
    let shared = module
        .expressions
        .iter()
        .position(|node| matches!(node, Expr::Binding(_)))
        .unwrap();
    let shared = ExprId::new(shared);
    // A class base and method reach shared nodes in strict execution; those
    // nodes remain reachable in their original sloppy context too.
    let class = expr(
        &mut module,
        Expr::Class {
            name: "Example".into(),
            base: Some(shared),
            constructor: None,
            methods: vec![("method".into(), FunctionId::new(0))],
        },
    );
    module.regions[root.index()]
        .statements
        .push(Statement::Evaluate(class));
    let body = module.region(module.regions[root.index()].scope);
    let caught = binding(&mut module, body, 1000, "caught");
    let caught_read = expr(&mut module, Expr::Binding(caught));
    module.regions[body.index()]
        .statements
        .push(Statement::Evaluate(caught_read));
    let try_body = module.region(module.regions[root.index()].scope);
    module.regions[try_body.index()]
        .statements
        .push(Statement::Evaluate(shared));
    module.regions[root.index()]
        .statements
        .push(Statement::Try {
            body: try_body,
            catch: Some(Catch {
                binding: Some(caught),
                body,
            }),
            finally: None,
        });
    let loop_body = module.region(module.regions[root.index()].scope);
    let key = binding(&mut module, loop_body, 1001, "key");
    module.regions[root.index()].statements.extend([
        Statement::ForIn {
            binding: key,
            object: shared,
            body: loop_body,
        },
        Statement::ForOf {
            binding: key,
            iterable: shared,
            body: loop_body,
        },
    ]);
    let shared_binding = binding(&mut module, root, 1002, "sharedBinding");
    let zero = number(&mut module, 0.0);
    module.regions[root.index()]
        .statements
        .push(Statement::Let {
            binding: shared_binding,
            value: Some(zero),
        });
    // Distinct references from distinct functions must report capture.
    for index in [0, 1] {
        let read = expr(&mut module, Expr::Binding(shared_binding));
        let body = module.functions[index].body;
        module.regions[body.index()]
            .statements
            .push(Statement::Evaluate(read));
    }
    compare(&module);
    let full = module.reach(&mut AllocationBudget::new(None)).unwrap();
    assert!(full.captured[shared_binding.index()]);
    let reached = module.reach_tree(&mut AllocationBudget::new(None)).unwrap();
    assert_eq!(reached.strict_expressions[shared.index()], 3);
    assert_eq!(reached.strict_regions[module.functions[0].body.index()], 3);
}

#[test]
fn target_reach_tree_omits_capture_work_and_all_temporary_storage_releases() {
    let (module, _) = single_call::sites();
    let mut full_ledger = ledger(100_000_000, 10_000_000);
    let mut tree_ledger = ledger(100_000_000, 10_000_000);
    {
        let mut budget = AllocationBudget::new(Some((&mut full_ledger, WorkDomain::Baseline)));
        drop(module.reach(&mut budget).unwrap());
    }
    {
        let mut budget = AllocationBudget::new(Some((&mut tree_ledger, WorkDomain::Baseline)));
        drop(module.reach_tree(&mut budget).unwrap());
    }
    assert_eq!(full_ledger.retained_bytes(), 0);
    assert_eq!(tree_ledger.retained_bytes(), 0);
    assert!(
        tree_ledger.work_used(WorkDomain::Baseline) < full_ledger.work_used(WorkDomain::Baseline)
    );
    assert!(tree_ledger.peak_retained_bytes() < full_ledger.peak_retained_bytes());
}

#[test]
fn target_reach_refusals_drop_partial_results_without_editing_the_tree() {
    let (module, _) = single_call::sites();
    let original = module.clone();
    for full in [false, true] {
        let mut refused = 0;
        let mut completed = 0;
        for (work, bytes) in [
            (1, 1_000_000),
            (100, 1_000_000),
            (1000, 1_000_000),
            (100_000, 1),
            (100_000, 50),
            (100_000, 2000),
            (100_000, 20_000),
            (100_000, 1_000_000),
        ] {
            let mut ledger = ledger(work, bytes);
            {
                let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
                let result = if full {
                    module.reach(&mut budget).map(drop)
                } else {
                    module.reach_tree(&mut budget).map(drop)
                };
                match result {
                    Ok(()) => completed += 1,
                    Err(AllocationError::Budget(_)) => {
                        refused += 1;
                        assert_eq!(budget.retained_bytes(AllocationClass::Retained), 0);
                        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
                    }
                    Err(error) => panic!("unexpected refusal {error:?}"),
                }
            }
            assert_eq!(ledger.retained_bytes(), 0);
            assert_eq!(module, original);
        }
        assert!(
            refused >= 5 && completed >= 1,
            "{refused} refusals, {completed} successes"
        );
    }
}

#[test]
fn target_reach_callback_releases_facts_and_preserves_the_callers_allocations() {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let (mut module, _) = single_call::sites();
    let mut ledger = ledger(100_000_000, 10_000_000);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let marker = budget
            .string(AllocationClass::Retained, "existing caller storage")
            .unwrap();
        let retained = budget.retained_bytes(AllocationClass::Retained);
        let output = module
            .with_reach(&mut budget, |_, reach, budget| {
                assert!(!reach.captured.is_empty());
                budget.string(AllocationClass::Retained, "new caller storage")
            })
            .unwrap()
            .unwrap();
        assert_eq!(
            budget.retained_bytes(AllocationClass::Retained),
            retained + output.capacity() as u64
        );
        let refused = module
            .with_reach_tree(&mut budget, |_, tree, budget| {
                assert!(!tree.expressions.is_empty());
                budget.work(Analysis, u64::MAX)
            })
            .unwrap();
        assert!(matches!(refused, Err(AllocationError::Budget(_))));
        let before_panic = budget.retained_bytes(AllocationClass::Retained);
        let unwound = catch_unwind(AssertUnwindSafe(|| {
            module
                .with_reach(&mut budget, |_, _, _| panic!("analysis callback"))
                .unwrap();
        }));
        assert!(unwound.is_err());
        assert_eq!(
            budget.retained_bytes(AllocationClass::Retained),
            before_panic
        );
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        drop((marker, output));
    }
    assert_eq!(ledger.retained_bytes(), 0);
}

impl Module {
    /// Every expression and region reachable from the root, each expression
    /// with its depth as the verifier counts it.
    fn reach_before_partition(
        &self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<OldReach, AllocationError> {
        let mut reach = OldReach {
            expressions: Vec::new(),
            regions: Vec::new(),
            captured: vec![false; self.bindings.len()],
            strict_regions: vec![0; self.regions.len()],
            strict_expressions: vec![0; self.expressions.len()],
            parameters: vec![false; self.bindings.len()],
        };
        // The function each binding is declared in, and each reference's.
        let mut declared: Vec<Option<Option<FunctionId>>> = vec![None; self.bindings.len()];
        let mut references: Vec<(BindingId, Option<FunctionId>)> = Vec::new();
        let mut regions = vec![(self.root, 0usize, None::<FunctionId>, false)];
        let mut pending: Vec<(ExprId, usize, bool)> = Vec::new();
        while let Some((region, depth, owner, strict)) = regions.pop() {
            budget.work(Analysis, 1)?;
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
                budget.work(Analysis, 1)?;
                match statement {
                    Statement::Let { binding, .. }
                    | Statement::ForIn { binding, .. }
                    | Statement::ForOf { binding, .. }
                    | Statement::Function { binding, .. } => {
                        declared[binding.index()] = Some(owner);
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
                statement.visit_expressions(|root| pending.push((root, depth + 1, strict)));
                statement.visit_regions(|child| regions.push((child, depth + 1, owner, strict)));
                if let Statement::Function { function, .. } = statement {
                    for parameter in &self.functions[function.index()].parameters {
                        declared[parameter.index()] = Some(Some(*function));
                        reach.parameters[parameter.index()] = true;
                    }
                    regions.push((
                        self.functions[function.index()].body,
                        depth + 2,
                        Some(*function),
                        strict || self.functions[function.index()].strict,
                    ));
                }
                while let Some((id, at, strict)) = pending.pop() {
                    budget.work(Analysis, 1)?;
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
                    if let Expr::Binding(binding) = expression {
                        references.push((*binding, owner));
                    }
                    for function in expression.created_functions() {
                        for parameter in &self.functions[function.index()].parameters {
                            declared[parameter.index()] = Some(Some(function));
                            reach.parameters[parameter.index()] = true;
                        }
                        regions.push((
                            self.functions[function.index()].body,
                            at + 2,
                            Some(function),
                            strict || self.functions[function.index()].strict,
                        ));
                    }
                    let _ = expression.visit_children(|child| {
                        pending.push((child, at + 1, strict));
                        Ok::<_, ()>(())
                    });
                }
            }
        }
        budget.work(Analysis, references.len() as u64)?;
        for (binding, owner) in references {
            // An undeclared binding (an import or host) counts as captured.
            if declared[binding.index()] != Some(owner) {
                reach.captured[binding.index()] = true;
            }
        }
        Ok(reach)
    }
}
