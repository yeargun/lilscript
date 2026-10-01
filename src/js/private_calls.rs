//! Optional private call representations. The unchanged tree remains a search
//! candidate. Complete direct uses protect identity/arity; an inert root prefix
//! establishes initialization before any call, including reentrant host calls.
//! Sharing is restricted to primitive expressions: it cannot combine receiver
//! feedback sites, allocate identities, or add an indirect call.
use super::*;
use crate::compilation_policy::WorkKind::Analysis;
use crate::output_budget::AllocationClass::{Retained, Scratch};

#[derive(Clone, Copy)]
pub(super) enum Mode {
    Specialize,
    Share,
    Parameterize,
}

struct Candidate {
    binding: BindingId,
    function: FunctionId,
    expression: ExprId,
    calls: Vec<ExprId>,
    nodes: Vec<ExprId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s3_long_literal_duplication_keeps_the_smaller_retained_call_candidate() {
        use crate::build::{compile_source, ServiceOptions};
        use crate::js::selection::Objective;
        let literal = "differing-UTF16-data-abcdefghijklmnopqrstuvwxyz".repeat(8);
        let source = format!("extern string input();string pad(string x){{return x+\"{literal}\";}}print(pad(input()));print(pad(input()));");
        let compile = |permission| {
            let settings = format!("objective.codecs='raw'\neffort.level=13\n[javascript]\ncandidate_proposal_limit=0\nterminal_codec_probe_limit=128\n[policy.tactics]\nconstant-folding='off'\nstring-pooling='off'\nscalar-replacement='off'\ncall-specialization='off'\nhelper-sharing='off'\ninlining='{permission}'");
            let config = toml::from_str(&settings).unwrap();
            compile_source(&source, &config, ServiceOptions::default())
                .unwrap()
                .javascript(Objective::Raw)
                .unwrap()
                .javascript()
                .to_owned()
        };
        let retained = compile("off");
        let chosen = compile("on");
        assert!(
            chosen.len() <= retained.len(),
            "{} > {}",
            chosen.len(),
            retained.len()
        );
        assert_eq!(chosen.matches(&literal).count(), 1, "{chosen}");
        let observed = std::process::Command::new("node")
            .args(["-e", &format!("globalThis.input=()=>\"x\";{chosen}")])
            .output()
            .unwrap();
        assert!(observed.status.success());
        assert_eq!(
            String::from_utf8(observed.stdout).unwrap(),
            format!("x{literal}\nx{literal}\n")
        );
    }

    fn check(source: &str, expected: &str, mode: Mode, changes: bool) {
        use crate::build::{compile_source, ServiceOptions};
        use crate::js::selection::Objective;
        let tactic = match mode {
            Mode::Specialize => "call-specialization",
            Mode::Share | Mode::Parameterize => "helper-sharing",
        };
        let compile = |permission| {
            let settings = format!("objective.codecs='raw'\neffort.level=13\n[javascript]\ncandidate_proposal_limit=0\nterminal_codec_probe_limit=128\n[policy.tactics]\ninlining='off'\nconstant-folding='off'\nscalar-replacement='off'\nstring-pooling='off'\nnaming-search='off'\n{tactic}='{permission}'");
            let config: crate::config::ProjectConfig = toml::from_str(&settings).unwrap();
            let result = compile_source(source, &config, ServiceOptions::default()).unwrap();
            result
                .javascript(Objective::Raw)
                .unwrap()
                .javascript()
                .to_owned()
        };
        let before = compile("off");
        let after = compile("on");
        assert_eq!(before != after, changes, "before: {before}\nafter: {after}");
        for code in [before, after] {
            let observed = std::process::Command::new("node")
                .args(["--input-type=module", "-e", &code])
                .output()
                .unwrap();
            assert!(
                observed.status.success(),
                "{}\n{code}",
                String::from_utf8_lossy(&observed.stderr)
            );
            assert_eq!(
                String::from_utf8(observed.stdout).unwrap(),
                expected,
                "{code}"
            );
        }
    }

    #[test]
    fn s3_share_private_primitive_bodies_without_observing_names() {
        check(
            "int a(int x){return x*x+3;}int b(int n){return n*n+3;}print(a(2));print(b(4));",
            "7\n19\n",
            Mode::Share,
            true,
        );
    }
    #[test]
    fn s3_parameterize_helper_literals_preserves_argument_positions() {
        check(
            "int a(int x){return x*x+3;}int b(int n){return n*n+7;}print(a(2));print(b(4));",
            "7\n23\n",
            Mode::Parameterize,
            true,
        );
    }
    #[test]
    fn s3_specialize_uniform_arguments_without_reordering_other_effects() {
        check("int scale(int n,int factor){return n*factor+factor;}print(scale(3,7));print(scale(4,7));", "28\n35\n", Mode::Specialize, true);
    }
    #[test]
    fn s3_sharing_keeps_distinct_captures_and_observed_identity() {
        check("int x=2;int y=9;int a(int n){return n+x;}int b(int n){return n+y;}print(a(1));print(b(1));", "3\n10\n", Mode::Share, false);
        check("int a(int n){return n+2;}int b(int n){return n+2;}print(a==b);print(a(1));print(b(1));", "false\n3\n3\n", Mode::Share, false);
    }
    #[test]
    fn s3_specialization_distinguishes_positive_and_negative_zero() {
        check(
            "float inverse(float n){return 1.0/n;}print(inverse(0.0));print(inverse(-0.0));",
            "Infinity\n-Infinity\n",
            Mode::Specialize,
            false,
        );
    }
    #[test]
    fn s3_sharing_rejects_receiver_feedback_and_reentry() {
        check("class Box{int value=3;}int a(Box x){return x.value;}int b(Box x){return x.value;}Box x=new Box();print(a(x));print(b(x));", "3\n3\n", Mode::Share, false);
    }
}

fn same_literal(left: &Literal, right: &Literal) -> bool {
    match (left, right) {
        (Literal::Number(a), Literal::Number(b)) => a.to_bits() == b.to_bits(),
        _ => left == right,
    }
}

fn copy_literal(
    value: &Literal,
    budget: &mut AllocationBudget<'_>,
) -> Result<Literal, AllocationError> {
    Ok(match value {
        Literal::String(text) => Literal::String(budget.string_value(Retained, text)?),
        value => value.clone(),
    })
}

impl Module {
    pub(super) fn private_calls(
        &mut self,
        mode: Mode,
        frames_hidden: bool,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        if !frames_hidden {
            return Ok(());
        }
        self.with_reach(budget, |module, reach, budget| {
            let mut reads = budget.filled(Scratch, module.bindings.len(), 0usize)?;
            let mut calls: Vec<Vec<ExprId>> = budget.vector(Scratch, module.bindings.len())?;
            calls.resize_with(module.bindings.len(), Vec::new);
            let mut references = if matches!(mode, Mode::Parameterize) {
                budget.filled(Scratch, module.expressions.len(), 0u32)?
            } else {
                Vec::new()
            };
            for &(id, _) in &reach.expressions {
                budget.work(Analysis, 1)?;
                match &module.expressions[id.index()] {
                    Expr::Binding(binding) => reads[binding.index()] += 1,
                    Expr::Call { callee, .. } => {
                        if let Expr::Binding(binding) = module.expressions[callee.index()] {
                            budget.push(Scratch, &mut calls[binding.index()], id)?;
                        }
                    }
                    // Direct eval could observe otherwise unreferenced bindings.
                    Expr::Host(host) if host.kind == crate::catalog::HostKind::Eval => {
                        return Ok(())
                    }
                    _ => {}
                }
                if !references.is_empty() {
                    module.expressions[id.index()].visit_children(|child| {
                        budget.work(Analysis, 1)?;
                        references[child.index()] = references[child.index()].saturating_add(1);
                        Ok::<_, AllocationError>(())
                    })?;
                }
            }
            if !references.is_empty() {
                for &region in &reach.regions {
                    for statement in &module.regions[region.index()].statements {
                        budget.work(Analysis, 1)?;
                        statement.visit_expressions(|id| {
                            references[id.index()] = references[id.index()].saturating_add(1)
                        });
                    }
                }
            }
            let mut candidates = budget.vector(Scratch, 0)?;
            for statement in &module.regions[module.root.index()].statements {
                budget.work(Analysis, 1)?;
                let Statement::Let { binding, value } = *statement else {
                    break;
                };
                let Some(value) = value else { continue };
                let function = match module.expressions[value.index()] {
                    Expr::Literal(_) => continue,
                    Expr::Function(function) => function,
                    _ => break,
                };
                let data = &module.functions[function.index()];
                if module.bindings[binding.index()].pinned
                    || calls[binding.index()].is_empty()
                    || reads[binding.index()] != calls[binding.index()].len()
                    || module
                        .exports
                        .iter()
                        .any(|export| export.binding == binding)
                    || data.suspension != Suspension::None
                    || data.length.is_some()
                    || data.parameters.len() > 32
                {
                    continue;
                }
                let [Statement::Return(Some(expression))] =
                    module.regions[data.body.index()].statements[..]
                else {
                    continue;
                };
                let Some(nodes) = module.primitive_body(expression, budget)? else {
                    continue;
                };
                let sites = budget.copy_slice(Scratch, &calls[binding.index()])?;
                let mut complete = true;
                for &id in &sites {
                    budget.work(Analysis, 1)?;
                    let Expr::Call {
                        callee,
                        arguments,
                        invocation,
                    } = &module.expressions[id.index()]
                    else {
                        continue;
                    };
                    if module.expressions[callee.index()] != Expr::Binding(binding) {
                        continue;
                    }
                    complete &= matches!(invocation, Invocation::Value | Invocation::Reference)
                        && arguments.len() == data.parameters.len()
                        && arguments
                            .iter()
                            .all(|arg| !matches!(module.expressions[arg.index()], Expr::Spread(_)));
                }
                if complete {
                    budget.push(
                        Scratch,
                        &mut candidates,
                        Candidate {
                            binding,
                            function,
                            expression,
                            calls: sites,
                            nodes,
                        },
                    )?;
                }
            }
            match mode {
                Mode::Specialize => {
                    for candidate in &candidates {
                        module.specialize_constants(candidate, budget)?;
                    }
                }
                Mode::Share | Mode::Parameterize => {
                    let mut retired = budget.filled(Scratch, candidates.len(), false)?;
                    // Quadratic matching has a finite work allowance; larger
                    // populations retain their original implementations.
                    let mut comparisons = 0usize;
                    'matching: for left in 0..candidates.len() {
                        if retired[left] {
                            continue;
                        }
                        for right in left + 1..candidates.len() {
                            if retired[right] {
                                continue;
                            }
                            comparisons += 1;
                            if comparisons > 4096 {
                                break 'matching;
                            }
                            let a = &candidates[left];
                            let b = &candidates[right];
                            if let Some(differences) = module.match_helpers(
                                a,
                                b,
                                matches!(mode, Mode::Parameterize),
                                &references,
                                budget,
                            )? {
                                module.share_helpers(a, b, &differences, budget)?;
                                retired[right] = true;
                                // Added parameters change the representative's
                                // signature. Rediscover before any further merge.
                                break;
                            }
                        }
                    }
                    if retired.iter().any(|retired| *retired) {
                        // Parameterized calls gained newly allocated operands.
                        // Restore the target arena's postorder before admission.
                        let _ = module.renumber(budget)?;
                    }
                }
            }
            Ok(())
        })?
    }

    fn primitive_body(
        &self,
        root: ExprId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Option<Vec<ExprId>>, AllocationError> {
        let mut pending = budget.vector(Scratch, 256)?;
        let mut nodes = budget.vector(Scratch, 256)?;
        pending.push(root);
        while let Some(id) = pending.pop() {
            budget.work(Analysis, 1)?;
            if nodes.len() + pending.len() >= 256 {
                return Ok(None);
            }
            let node = &self.expressions[id.index()];
            match node {
                Expr::Literal(_) => {}
                Expr::Binding(binding)
                    if matches!(
                        self.bindings[binding.index()].class,
                        Some(
                            ValueClass::Int
                                | ValueClass::Number
                                | ValueClass::String
                                | ValueClass::Boolean
                        )
                    ) => {}
                Expr::Unary {
                    op:
                        Unary::Negate
                        | Unary::Not
                        | Unary::BitNot
                        | Unary::Plus
                        | Unary::TypeOf
                        | Unary::Void,
                    ..
                }
                | Expr::Binary { .. }
                | Expr::IntBinary { .. }
                | Expr::IntNegate(_)
                | Expr::ToInt32(_)
                | Expr::Conditional { .. } => {}
                _ => return Ok(None),
            }
            node.visit_children(|child| budget.push(Scratch, &mut pending, child))?;
            nodes.push(id);
        }
        Ok(Some(nodes))
    }

    fn specialize_constants(
        &mut self,
        candidate: &Candidate,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        let parameters = self.functions[candidate.function.index()]
            .parameters
            .clone();
        let mut removed = budget.filled(Scratch, parameters.len(), false)?;
        for (position, &parameter) in parameters.iter().enumerate() {
            budget.work(Analysis, 1)?;
            let mut constant: Option<Literal> = None;
            let mut uniform = true;
            for &call in &candidate.calls {
                budget.work(Analysis, 1)?;
                let Expr::Call { arguments, .. } = &self.expressions[call.index()] else {
                    unreachable!()
                };
                let Expr::Literal(value) = &self.expressions[arguments[position].index()] else {
                    uniform = false;
                    break;
                };
                if let Some(prior) = &constant {
                    uniform &= same_literal(prior, value);
                } else {
                    constant = Some(value.clone());
                }
            }
            if !uniform {
                continue;
            }
            let Some(constant) = constant else { continue };
            for &id in &candidate.nodes {
                budget.work(Analysis, 1)?;
                if self.expressions[id.index()] == Expr::Binding(parameter) {
                    let value = copy_literal(&constant, budget)?;
                    self.set_expression(id, Expr::Literal(value));
                }
            }
            removed[position] = true;
        }
        if !removed.iter().any(|removed| *removed) {
            return Ok(());
        }
        let mut position = 0;
        self.function_mut(candidate.function)
            .parameters
            .retain(|_| {
                let keep = !removed[position];
                position += 1;
                keep
            });
        for &call in &candidate.calls {
            let Expr::Call { arguments, .. } = self.expression_mut(call) else {
                unreachable!()
            };
            let mut position = 0;
            arguments.retain(|_| {
                let keep = !removed[position];
                position += 1;
                keep
            });
        }
        Ok(())
    }

    fn match_helpers(
        &self,
        a: &Candidate,
        b: &Candidate,
        parameterize: bool,
        references: &[u32],
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Option<Vec<(ExprId, ExprId)>>, AllocationError> {
        let left = &self.functions[a.function.index()];
        let right = &self.functions[b.function.index()];
        if left.parameters.len() != right.parameters.len()
            || left.strict != right.strict
            || left
                .parameters
                .iter()
                .zip(&right.parameters)
                .any(|(a, b)| self.bindings[a.index()].class != self.bindings[b.index()].class)
        {
            return Ok(None);
        }
        let mut pending = budget.vector(Scratch, 256)?;
        let mut differences = budget.vector(Scratch, 4)?;
        pending.push((a.expression, b.expression));
        while let Some((aid, bid)) = pending.pop() {
            budget.work(Analysis, 1)?;
            let a = &self.expressions[aid.index()];
            let b = &self.expressions[bid.index()];
            match (a, b) {
                (Expr::Binding(a), Expr::Binding(b)) => {
                    match (
                        left.parameters.iter().position(|p| p == a),
                        right.parameters.iter().position(|p| p == b),
                    ) {
                        (Some(a), Some(b)) if a == b => {}
                        (None, None) if a == b => {}
                        _ => return Ok(None),
                    }
                }
                (Expr::Literal(a), Expr::Literal(b)) => {
                    if !same_literal(a, b) {
                        if !parameterize
                            || references.get(aid.index()) != Some(&1)
                            || differences.len() == 4
                            || std::mem::discriminant(a) != std::mem::discriminant(b)
                        {
                            return Ok(None);
                        }
                        differences.push((aid, bid));
                    }
                }
                _ => {
                    let (mut a, mut b) = (a.clone(), b.clone());
                    let mut children = [None; 3];
                    let mut count = 0;
                    a.remap_children(|child| {
                        children[count] = Some(child);
                        count += 1;
                        ExprId::new(0)
                    });
                    let mut index = 0;
                    let mut mismatch = false;
                    b.remap_children(|child| {
                        if let Some(Some(other)) = children.get(index) {
                            pending.push((*other, child));
                        } else {
                            mismatch = true;
                        }
                        index += 1;
                        ExprId::new(0)
                    });
                    if mismatch || count != index || a != b {
                        return Ok(None);
                    }
                }
            }
        }
        Ok(Some(differences))
    }

    fn share_helpers(
        &mut self,
        a: &Candidate,
        b: &Candidate,
        differences: &[(ExprId, ExprId)],
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        for &(left, right) in differences {
            let (Expr::Literal(left_value), Expr::Literal(right_value)) = (
                &self.expressions[left.index()],
                &self.expressions[right.index()],
            ) else {
                unreachable!()
            };
            let (left_value, right_value) = (left_value.clone(), right_value.clone());
            let scope = self.regions[self.functions[a.function.index()].body.index()].scope;
            let class = match left_value {
                Literal::Number(_) => Some(ValueClass::Number),
                Literal::String(_) => Some(ValueClass::String),
                Literal::Bool(_) => Some(ValueClass::Boolean),
                _ => None,
            };
            let parameter = self.binding_in(
                Binding {
                    source_symbol: None,
                    scope,
                    spelling: "shared_constant".into(),
                    pinned: false,
                    class,
                    defined: false,
                },
                budget,
            )?;
            budget.reserve_vec(Retained, &mut self.function_mut(a.function).parameters, 1)?;
            self.function_mut(a.function).parameters.push(parameter);
            for (candidate, literal) in [(a, &left_value), (b, &right_value)] {
                for &call in &candidate.calls {
                    let value = copy_literal(literal, budget)?;
                    let argument = self.expression_in(Expr::Literal(value), None, budget)?;
                    let Expr::Call { arguments, .. } = self.expression_mut(call) else {
                        unreachable!()
                    };
                    budget.push(Retained, arguments, argument)?;
                }
            }
            self.set_expression(left, Expr::Binding(parameter));
        }
        for &call in &b.calls {
            let Expr::Call { callee, .. } = self.expressions[call.index()] else {
                unreachable!()
            };
            self.set_expression(callee, Expr::Binding(a.binding));
        }
        let position = self.regions[self.root.index()].statements.iter().position(|statement|
            matches!(statement, Statement::Let { binding, .. } if *binding == b.binding)).expect("private helper declaration");
        self.remove_statement(self.root.index(), position);
        Ok(())
    }
}
