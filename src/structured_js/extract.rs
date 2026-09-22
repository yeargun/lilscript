//! Target choices borrow an unchanged semantic program. Its operators survive
//! optimization and extraction; no complete JavaScript program is cloned, and
//! no proof can outlive a mutation of the program it describes.

use super::*;
use crate::compilation_policy::WorkKind;
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError, RetainedCharge};
use analysis::{Analysis, Mode, Snapshot, Work};
use std::cell::RefCell;

/// Accounted failures retain only fixed metadata. Formatting a user diagnostic
/// belongs to the explicit inspection/reporting boundary, not target admission.
#[derive(Debug)]
pub enum OutputError {
    Invalid(&'static str),
    InvalidAt {
        reason: &'static str,
        index: usize,
    },
    Syntax {
        edition: crate::js_syntax_target::EcmaScriptEdition,
        feature: crate::js_syntax_target::JsSyntaxFeature,
    },
    Admission(AllocationError),
    ByteLimit,
}

impl From<AllocationError> for OutputError {
    fn from(error: AllocationError) -> Self {
        Self::Admission(error)
    }
}
impl From<&'static str> for OutputError {
    fn from(reason: &'static str) -> Self {
        Self::Invalid(reason)
    }
}
impl std::fmt::Display for OutputError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(reason) => formatter.write_str(reason),
            Self::InvalidAt { reason, index } => write!(formatter, "{reason} {index}"),
            Self::Syntax { feature, .. } => write!(
                formatter,
                "{} requires javascript.ecmascript {} or newer",
                feature.construct(),
                feature.min_edition().name(),
            ),
            Self::Admission(error) => write!(formatter, "output admission failed: {error:?}"),
            Self::ByteLimit => formatter.write_str("output exceeds its byte limit"),
        }
    }
}
impl std::error::Error for OutputError {}

#[derive(Clone, Copy)]
pub(super) struct JavaScriptChoices<'a> {
    pub plain_integer: &'a [bool],
    unobserved_names: &'a [bool],
}

pub(super) fn function_name<'a>(
    module: &'a Module,
    function: FunctionId,
    choices: Option<JavaScriptChoices<'_>>,
) -> Option<&'a StringValue> {
    if choices.is_some_and(|choices| choices.unobserved_names[function.index()]) {
        None
    } else {
        module.functions[function.index()].name.exact()
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NameWork {
    pub scanned_statements: usize,
    pub candidates: usize,
    pub statements: usize,
    pub expressions: usize,
}

/// One prepared output boundary shares its naming constraints, verified
/// structure and target facts across every candidate render.
pub struct Output<'a> {
    module: &'a Module,
    pub(super) basis: naming::Basis<'a>,
    choices: Option<JavaScriptChoices<'a>>,
    naming: naming::Eligibility,
    literal_alternatives: &'a [LiteralAlternative],
    has_literal_alternative: bool,
    permits_observed_literals: bool,
    pub reused_structure: bool,
    // Basis (including installed lazy caches) drops before its reservation owner.
    budget: RefCell<AllocationBudget<'a>>,
}

impl<'a> Output<'a> {
    /// Explicit policy-free inspection route, sharing the production algorithms.
    pub(super) fn from_module(module: &'a Module) -> Result<Self, String> {
        Self::from_module_in(
            module,
            naming::Eligibility::Search,
            None,
            AllocationBudget::new(None),
        )
        .map_err(|error| error.to_string())
    }

    fn from_module_in(
        module: &'a Module,
        naming: naming::Eligibility,
        edition: Option<crate::js_syntax_target::EcmaScriptEdition>,
        budget: AllocationBudget<'a>,
    ) -> Result<Self, OutputError> {
        Self::from_module_with_literals_in(module, naming, edition, &[], false, budget)
    }

    fn from_module_with_literals_in(
        module: &'a Module,
        naming: naming::Eligibility,
        edition: Option<crate::js_syntax_target::EcmaScriptEdition>,
        literal_alternatives: &'a [LiteralAlternative],
        permits_observed_literals: bool,
        mut budget: AllocationBudget<'a>,
    ) -> Result<Self, OutputError> {
        let (basis, has_literal_alternative) = {
            let mut prepare = budget.scope();
            let structure = verify::verify_in(module, &mut prepare)?;
            if let Some(edition) = edition {
                verify::verify_edition_in(module, &structure, edition, &mut prepare)?;
            }
            let has_literal_alternative =
                literal_output::validate(module, literal_alternatives, &structure, &mut prepare)?;
            let basis = naming::Basis::new_in(module, &structure, None, &mut prepare)?;
            drop(structure);
            prepare.finish_retained()?;
            (basis, has_literal_alternative)
        };
        Ok(Self {
            module,
            basis,
            choices: None,
            naming,
            literal_alternatives,
            has_literal_alternative,
            permits_observed_literals,
            reused_structure: false,
            budget: RefCell::new(budget),
        })
    }

    pub(super) fn new(
        tree: &'a lower::AnnotatedTree<'_, '_>,
        choices: Option<JavaScriptChoices<'a>>,
    ) -> Result<Self, String> {
        let (structure, reused_structure) = tree.structure()?;
        let mut budget = AllocationBudget::new(None);
        let basis = {
            let mut prepare = budget.scope();
            let basis = naming::Basis::new_in(tree.target(), structure, choices, &mut prepare)
                .map_err(|error| error.to_string())?;
            prepare
                .finish_retained()
                .map_err(|error| error.to_string())?;
            basis
        };
        Ok(Self {
            module: tree.target(),
            basis,
            choices,
            naming: naming::Eligibility::Search,
            literal_alternatives: &[],
            has_literal_alternative: false,
            permits_observed_literals: false,
            reused_structure,
            budget: RefCell::new(budget),
        })
    }

    pub(crate) fn is_accounted(&self) -> bool {
        self.budget.borrow().is_accounted()
    }

    /// Coordinate the compilation's private artifact owner without exposing the
    /// output budget or its underlying ledger to an external caller.
    pub(crate) fn with_allocation_budget<R>(
        &self,
        inspect: impl FnOnce(&mut AllocationBudget<'_>) -> R,
    ) -> R {
        inspect(&mut self.budget.borrow_mut())
    }

    /// Inspection returns owned bytes at its explicit, unaccounted boundary.
    /// Production must instead transfer admission with the bytes to its store.
    pub fn render(&self, plan: &naming::Plan) -> Result<String, String> {
        self.render_bounded(plan, usize::MAX)
    }

    pub(super) fn render_bounded(
        &self,
        plan: &naming::Plan,
        limit: usize,
    ) -> Result<String, String> {
        if self.is_accounted() {
            return Err("accounted output requires the compilation artifact owner".into());
        }
        self.render_scoped(plan, LiteralOutput::Original, limit)
            .map_err(|error| error.to_string())
    }

    fn render_scoped(
        &self,
        plan: &naming::Plan,
        literals: LiteralOutput,
        limit: usize,
    ) -> Result<String, OutputError> {
        self.naming.check_in(plan)?;
        let mut budget = self.budget.borrow_mut();
        let mut render = budget.scope();
        let result = (|| {
            let names = self.basis.names_in(plan, &mut render)?;
            print::render_with_literals_admitted(
                self.module,
                &names,
                self.choices,
                self.literal_alternatives,
                literals,
                limit,
                &mut render,
            )
            .map_err(|error| match error {
                print::PrintError::Admission(error) => OutputError::Admission(error),
                print::PrintError::ByteLimit => OutputError::ByteLimit,
            })
        })();
        // Lazy Basis caches already installed by this render stay live even if
        // naming or printing failed. Printer rolls back its own partial bytes;
        // Names and other scratch have dropped before this commit.
        render.finish_retained()?;
        result
    }

    pub(crate) fn render_admitted<Owner: Eq>(
        &self,
        plan: &naming::Plan,
        limit: usize,
        owner: Owner,
    ) -> Result<(String, RetainedCharge<Owner>), OutputError> {
        self.render_with_literals_admitted(plan, LiteralOutput::Original, limit, owner)
            .map(|(text, charge, _)| (text, charge))
    }

    pub(crate) fn has_literal_alternative_admitted(&self) -> Result<bool, OutputError> {
        self.budget.borrow_mut().work(WorkKind::Analysis, 1)?;
        Ok(self.has_literal_alternative && self.permits_observed_literals)
    }

    pub(crate) fn render_with_literals_admitted<Owner: Eq>(
        &self,
        plan: &naming::Plan,
        literals: LiteralOutput,
        limit: usize,
        owner: Owner,
    ) -> Result<(String, RetainedCharge<Owner>, LiteralOutput), OutputError> {
        if !self.is_accounted() {
            return Err(AllocationError::Unaccounted.into());
        }
        let literals = {
            let mut budget = self.budget.borrow_mut();
            budget.work(WorkKind::Analysis, 1)?;
            if literals == LiteralOutput::Observed && !self.permits_observed_literals {
                return Err(OutputError::Invalid(
                    "observed literal output requires target-compaction permission",
                ));
            }
            if self.has_literal_alternative {
                literals
            } else {
                LiteralOutput::Original
            }
        };
        let text = self.render_scoped(plan, literals, limit)?;
        let bytes = u64::try_from(text.capacity()).map_err(|_| AllocationError::Capacity)?;
        let mut budget = self.budget.borrow_mut();
        // The last print/copy segment may finish after the deadline. Reject
        // before publishing bytes, while keeping already installed Basis caches
        // owned. Cleanup and successful ownership commits do not check time.
        if let Err(error) = budget.work(WorkKind::Render, 0) {
            drop(text);
            budget.release(AllocationClass::Retained, bytes)?;
            return Err(error.into());
        }
        let charge = budget.detach_retained(owner, bytes)?;
        Ok((text, charge, literals))
    }

    pub(crate) fn source_candidates_admitted(&self) -> Result<&[BindingId], OutputError> {
        self.basis
            .source_candidates_in(&mut self.budget.borrow_mut())
    }

    pub(super) fn naming_seeds(&self) -> &'static [naming::Style] {
        self.naming.seeds()
    }
    pub(super) fn permits_naming_search(&self) -> bool {
        self.naming.permits_search()
    }
}

impl Module {
    fn check_import_execution(
        &self,
        policy: &crate::compilation_policy::ResolvedPolicy,
    ) -> Result<(), OutputError> {
        if !self.imports.is_empty()
            && policy.javascript_contract().is_some_and(|contract| {
                contract.execution != crate::compilation_contract::JavaScriptExecution::Module
            })
        {
            return Err("static imports require ECMAScript module execution".into());
        }
        Ok(())
    }

    /// Explicit inspection wrapper; permissions are copied into prepared output.
    pub fn prepare_output_with_policy(
        &self,
        policy: &crate::compilation_policy::ResolvedPolicy,
    ) -> Result<Output<'_>, String> {
        self.check_import_execution(policy)
            .map_err(|error| error.to_string())?;
        let naming =
            naming::Eligibility::from_policy_in(policy).map_err(|error| error.to_string())?;
        let edition = policy
            .javascript_contract()
            .expect("naming checked the target")
            .ecmascript;
        Output::from_module_in(self, naming, Some(edition), AllocationBudget::new(None))
            .map_err(|error| error.to_string())
    }

    /// Production retains verifier/naming storage under the compilation's one
    /// allocation owner. No bare owned artifact can escape this prepared view.
    pub(crate) fn prepare_output_admitted<'a>(
        &'a self,
        policy: &crate::compilation_policy::ResolvedPolicy,
        parent: &'a mut AllocationBudget<'_>,
    ) -> Result<Output<'a>, OutputError> {
        self.prepare_output_with_literals_admitted(policy, &[], parent)
    }

    pub(crate) fn prepare_output_with_literals_admitted<'a>(
        &'a self,
        policy: &crate::compilation_policy::ResolvedPolicy,
        rows: &'a [LiteralAlternative],
        parent: &'a mut AllocationBudget<'_>,
    ) -> Result<Output<'a>, OutputError> {
        self.check_import_execution(policy)?;
        let naming = naming::Eligibility::from_policy_in(policy)?;
        let edition = policy
            .javascript_contract()
            .expect("naming checked the target")
            .ecmascript;
        Output::from_module_with_literals_in(
            self,
            naming,
            Some(edition),
            rows,
            policy
                .tactic(crate::compilation_policy::TacticId::TargetCompaction)
                .enabled,
            parent.scope(),
        )
    }
}

pub struct JavaScriptView<'tree, 'sem, 'src> {
    tree: &'tree lower::AnnotatedTree<'sem, 'src>,
    plain_integer: Vec<bool>,
    unobserved_names: Vec<bool>,
    pub omitted_normalizations: usize,
    pub omitted_function_names: usize,
    pub name_work: NameWork,
    pub reused_facts: bool,
    pub work: Work,
}

impl<'tree, 'sem, 'src> JavaScriptView<'tree, 'sem, 'src> {
    pub fn prepare(tree: &'tree lower::AnnotatedTree<'sem, 'src>, mode: Mode) -> Self {
        Self::prepare_in_world(
            tree,
            mode,
            crate::compilation_contract::JavaScriptWorld::ReusableLibrary,
        )
    }

    pub fn prepare_in_world(
        tree: &'tree lower::AnnotatedTree<'sem, 'src>,
        mode: Mode,
        world: crate::compilation_contract::JavaScriptWorld,
    ) -> Self {
        Self::prepare_in_execution(
            tree,
            mode,
            world,
            crate::compilation_contract::JavaScriptExecution::Script,
        )
    }

    pub fn prepare_in_execution(
        tree: &'tree lower::AnnotatedTree<'sem, 'src>,
        mode: Mode,
        world: crate::compilation_contract::JavaScriptWorld,
        execution: crate::compilation_contract::JavaScriptExecution,
    ) -> Self {
        Self::from_analysis(
            Analysis::new_in_execution(tree, mode, world, execution),
            false,
        )
    }

    pub fn prepare_reusing(
        tree: &'tree lower::AnnotatedTree<'sem, 'src>,
        snapshot: Snapshot,
    ) -> Result<Self, &'static str> {
        Self::prepare_reusing_in_execution(
            tree,
            snapshot,
            crate::compilation_contract::JavaScriptExecution::Script,
        )
    }

    pub fn prepare_reusing_in_execution(
        tree: &'tree lower::AnnotatedTree<'sem, 'src>,
        snapshot: Snapshot,
        execution: crate::compilation_contract::JavaScriptExecution,
    ) -> Result<Self, &'static str> {
        Ok(Self::from_analysis(
            Analysis::resume_in_execution(tree, snapshot, execution)?,
            true,
        ))
    }

    fn from_analysis(mut analysis: Analysis<'tree, 'sem, 'src>, reused_facts: bool) -> Self {
        let tree = analysis.tree();
        let module = tree.target();
        let mut plain_integer = vec![false; module.expressions.len()];
        let mut unobserved_names = vec![false; module.functions.len()];
        let mut name_work = NameWork::default();
        let mut omitted_normalizations = 0;
        // Direct eval can invalidate lexical value facts as well as names.
        // Until an eval contract constrains those writes, extraction must not
        // use an initializer's old range to remove semantic normalization.
        if analysis.has_direct_eval() {
            return Self {
                tree,
                plain_integer,
                unobserved_names,
                omitted_normalizations,
                omitted_function_names: 0,
                name_work,
                reused_facts,
                work: analysis.work(),
            };
        }
        for (index, expression) in module.expressions.iter().enumerate() {
            if matches!(
                expression,
                Expr::IntBinary { .. }
                    | Expr::IntNegate(_)
                    | Expr::ToInt32(_)
                    | Expr::Intrinsic { .. }
            ) && analysis.facts(ExprId::new(index)).normalization_redundant
            {
                plain_integer[index] = true;
                omitted_normalizations += 1;
            }
        }
        if !module.functions.is_empty() {
            let root_scope = module.regions[module.root.index()].scope;
            let mut pending_regions = Vec::new();
            let mut pending_expressions = Vec::new();
            for region in &module.regions {
                if analysis.world() == crate::compilation_contract::JavaScriptWorld::ReusableLibrary
                    && region.scope == root_scope
                {
                    continue;
                }
                for statement in &region.statements {
                    name_work.scanned_statements += 1;
                    let (binding, function) = match statement {
                        Statement::Function { binding, function } => (*binding, *function),
                        Statement::Let {
                            binding,
                            value: Some(value),
                        } => {
                            let Expr::Function(function) = module.expressions[value.index()] else {
                                continue;
                            };
                            (*binding, function)
                        }
                        _ => continue,
                    };
                    if module.functions[function.index()].name.exact().is_none() {
                        continue;
                    }
                    let uses = analysis.binding_uses(binding);
                    if uses.exported
                        || uses.reads.is_empty()
                        || !uses.writes.is_empty()
                        || !uses
                            .reads
                            .iter()
                            .all(|read| matches!(read.observation, analysis::Observation::Call(_)))
                    {
                        continue;
                    }
                    name_work.candidates += 1;
                    unobserved_names[function.index()] = frame_unobservable(
                        &mut analysis,
                        function,
                        &mut pending_regions,
                        &mut pending_expressions,
                        &mut name_work,
                    );
                }
            }
        }
        let omitted_function_names = unobserved_names
            .iter()
            .filter(|unobserved| **unobserved)
            .count();
        Self {
            tree,
            plain_integer,
            unobserved_names,
            omitted_normalizations,
            omitted_function_names,
            name_work,
            reused_facts,
            work: analysis.work(),
        }
    }

    pub fn render(&self, policy: PrintPolicy) -> Result<String, String> {
        self.output()?
            .render(&naming::Plan::new(if policy.mangle_bindings {
                naming::Style::Global
            } else {
                naming::Style::Source
            }))
    }

    pub fn output(&self) -> Result<Output<'_>, String> {
        Output::new(
            self.tree,
            Some(JavaScriptChoices {
                plain_integer: &self.plain_integer,
                unobserved_names: &self.unobserved_names,
            }),
        )
    }
}

/// Value effects alone do not describe observation of the executing frame.
/// In particular, a typed primitive may emit a mutable prototype-method call.
/// Refuse those calls without a target contract that proves them non-reentrant.
fn frame_unobservable(
    analysis: &mut Analysis<'_, '_, '_>,
    function: FunctionId,
    regions: &mut Vec<RegionId>,
    expressions: &mut Vec<ExprId>,
    work: &mut NameWork,
) -> bool {
    let module = analysis.tree().target();
    regions.clear();
    expressions.clear();
    regions.push(module.functions[function.index()].body);
    while let Some(region) = regions.pop() {
        for statement in &module.regions[region.index()].statements {
            work.statements += 1;
            match statement {
                Statement::If { yes, no, .. } => {
                    regions.push(*yes);
                    regions.extend(*no);
                }
                Statement::Loop { body, .. } | Statement::ForIn { body, .. } | Statement::ForOf { body, .. } => {
                    regions.push(*body);
                }
                Statement::Block(body) => {
                    regions.push(*body);
                }
                Statement::Throw(_) | Statement::Function { .. } | Statement::Try { .. } => {
                    return false
                }
                _ => {}
            }
            let mut unobservable = true;
            statement.visit_expressions(|root| {
                unobservable &= analysis.facts(root).effects.stable_scalar();
                expressions.push(root);
            });
            if !unobservable {
                return false;
            }
            while let Some(expression) = expressions.pop() {
                work.expressions += 1;
                let node = &module.expressions[expression.index()];
                if matches!(node, Expr::Intrinsic { operation, .. } if matches!(intrinsic_form(*operation), IntrinsicForm::Method(_)))
                {
                    return false;
                }
                node.visit_children(|child| {
                    expressions.push(child);
                    Ok::<_, ()>(())
                })
                .unwrap();
            }
        }
    }
    true
}
