//! Proved spelling decisions. Formation discovers sites and records immutable
//! print forms; rendering never discovers a structural rewrite.
use super::*;
use crate::compilation_policy::WorkKind::Analysis;
use crate::output_budget::AllocationClass::{Retained, Scratch};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LoopHead {
    pub binding: BindingId,
    pub value: ExprId,
    pub condition: Option<ExprId>,
    pub update: Option<ExprId>,
    pub body: RegionId,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Logical {
    pub condition: ExprId,
    pub op: Binary,
    pub left: ExprId,
    pub right: ExprId,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Defaults {
    pub values: Vec<Option<ExprId>>,
    pub absorbed: usize,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PrintForms {
    pub loops: Vec<Option<LoopHead>>,
    pub logical: Vec<Option<Logical>>,
    pub compound: Vec<Option<(Binary, ExprId)>>,
    pub increment: Vec<Option<&'static str>>,
    pub quotes: Vec<bool>,
    pub defaults: Vec<Defaults>,
    pub optional: Vec<Option<ExprId>>,
    pub logical_assignments: Vec<Option<Logical>>,
}
impl PrintForms {
    pub(super) fn clone_in(&self, budget: &mut AllocationBudget<'_>) -> Result<Self, AllocationError> {
        Ok(Self {
            loops: budget.copy_slice(Retained, &self.loops)?,
            logical: budget.copy_slice(Retained, &self.logical)?,
            compound: budget.copy_slice(Retained, &self.compound)?,
            increment: budget.copy_slice(Retained, &self.increment)?,
            quotes: budget.copy_slice(Retained, &self.quotes)?,
            defaults: super::cloning::map(&self.defaults, budget, |defaults, budget| Ok(Defaults {
                values: budget.copy_slice(Retained, &defaults.values)?, absorbed: defaults.absorbed,
            }))?,
            optional: budget.copy_slice(Retained, &self.optional)?,
            logical_assignments: budget.copy_slice(Retained, &self.logical_assignments)?,
        })
    }

    pub(crate) fn bytes(&self) -> u64 {
        (self.loops.len() * std::mem::size_of::<Option<LoopHead>>()
            + self.logical.len() * std::mem::size_of::<Option<Logical>>()
            + self.compound.len() * std::mem::size_of::<Option<(Binary, ExprId)>>()
            + self.increment.len() * std::mem::size_of::<Option<&'static str>>()
            + self.optional.len() * std::mem::size_of::<Option<ExprId>>()
            + self.logical_assignments.len() * std::mem::size_of::<Option<Logical>>()
            + self.quotes.len()
            + self.defaults.len() * std::mem::size_of::<Defaults>()
            + self
                .defaults
                .iter()
                .map(|d| d.values.len() * std::mem::size_of::<Option<ExprId>>())
                .sum::<usize>()) as u64
    }

    /// Inspection uses the same proofs with module defaults. Production asks
    /// for candidates, then applies each site's explicit/default assignment.
    pub(crate) fn new(
        module: &Module,
        discover: bool,
        class: AllocationClass,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        let mut result = Self {
            loops: budget.filled(class, module.regions.len(), None)?,
            logical: budget.filled(class, module.regions.len(), None)?,
            compound: budget.filled(class, module.expressions.len(), None)?,
            increment: budget.filled(class, module.expressions.len(), None)?,
            quotes: budget.filled(class, module.expressions.len(), false)?,
            defaults: budget.vector(class, module.functions.len())?,
            optional: budget.filled(class, module.expressions.len(), None)?,
            logical_assignments: budget.filled(class, module.regions.len(), None)?,
        };
        result
            .defaults
            .resize_with(module.functions.len(), Defaults::default);
        for (index, region) in module.regions.iter().enumerate() {
            budget.work(Analysis, region.statements.len() as u64 + 1)?;
            if discover || module.loop_head_declarations {
                if let [Statement::Let {
                    binding,
                    value: Some(value),
                }, Statement::Loop {
                    condition,
                    update,
                    body,
                }] = region.statements.as_slice()
                {
                    let roots: Vec<_> = condition.iter().chain(update.iter()).copied().collect();
                    if !module.mentions(&[*body], &roots, *binding, true)
                        && !contains_in(module, *value, budget)?
                    {
                        result.loops[index] = Some(LoopHead {
                            binding: *binding,
                            value: *value,
                            condition: *condition,
                            update: *update,
                            body: *body,
                        });
                    }
                }
            }
            if discover || module.logical_statements {
                for statement in &region.statements {
                    if let Statement::If {
                        condition,
                        yes,
                        no: None,
                    } = statement
                    {
                        if let [Statement::Evaluate(value)] =
                            module.regions[yes.index()].statements.as_slice()
                        {
                            let (op, left) = match module.expressions[condition.index()] {
                                Expr::Unary {
                                    op: Unary::Not,
                                    value,
                                } => (Binary::Or, value),
                                _ => (Binary::And, *condition),
                            };
                            let level = if op == Binary::Or { 4 } else { 5 };
                            if super::print::precedence(&module.expressions[left.index()]) >= level
                                && super::print::precedence(&module.expressions[value.index()])
                                    > level
                            {
                                result.logical[yes.index()] = Some(Logical {
                                    condition: *condition,
                                    op,
                                    left,
                                    right: *value,
                                });
                            }
                        }
                    }
                }
            }
        }
        for (index, expression) in module.expressions.iter().enumerate() {
            budget.work(Analysis, 1)?;
            if let Expr::Assign { target, value } = *expression {
                result.increment[index] = increment(module, target, value);
                if discover || module.compound_assignments {
                    result.compound[index] = compound(module, target, value);
                }
            }
            if discover || module.quotes {
                if let Expr::Literal(Literal::String(value)) = expression {
                    budget.work(Analysis, value.storage_bytes() as u64)?;
                    result.quotes[index] = value
                        .as_unicode()
                        .is_some_and(|text| text.matches('"').count() > text.matches('\'').count());
                }
            }
        }
        for (index, function) in module.functions.iter().enumerate() {
            budget.work(Analysis, 1)?;
            let Some(length) = function.length else {
                continue;
            };
            if function.strict || !module.arguments_free(FunctionId::new(index)) {
                continue;
            }
            let defaults = &mut result.defaults[index];
            defaults.values = budget.filled(class, function.parameters.len(), None)?;
            let mut last = None;
            for statement in &module.regions[function.body.index()].statements {
                budget.work(Analysis, function.parameters.len() as u64 + 1)?;
                let Some((parameter, default)) = module.default_check(statement) else {
                    break;
                };
                let Some(position) = function.parameters.iter().position(|&p| p == parameter)
                else {
                    break;
                };
                if position < length || last.is_some_and(|last| position <= last) {
                    break;
                }
                defaults.values[position] = Some(default);
                last = Some(position);
                defaults.absorbed += 1;
            }
        }
        Ok(result)
    }
}
fn contains_in(
    module: &Module,
    root: ExprId,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, AllocationError> {
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        budget.work(Analysis, 1)?;
        let expression = &module.expressions[id.index()];
        if matches!(expression, Expr::Binary { op: Binary::In, .. }) {
            return Ok(true);
        }
        if !expression.creates_function() {
            expression.visit_children(|child| {
                pending.push(child);
                Ok::<_, AllocationError>(())
            })?;
        }
    }
    Ok(false)
}
fn increment(module: &Module, target: ExprId, value: ExprId) -> Option<&'static str> {
    let Expr::Binding(binding) = module.expressions[target.index()] else {
        return None;
    };
    if !matches!(
        module.bindings[binding.index()].class,
        Some(ValueClass::Int | ValueClass::Number)
    ) {
        return None;
    }
    let Expr::Binary { op, left, right } = module.expressions[value.index()] else {
        return None;
    };
    let one =
        matches!(module.expressions[right.index()],Expr::Literal(Literal::Number(n)) if n==1.0);
    let same = matches!(module.expressions[left.index()],Expr::Binding(found) if found==binding);
    match op {
        Binary::Add if one && same => Some("++"),
        Binary::Subtract if one && same => Some("--"),
        _ => None,
    }
}
fn compound(module: &Module, target: ExprId, value: ExprId) -> Option<(Binary, ExprId)> {
    let Expr::Binary { op, left, right } = module.expressions[value.index()] else {
        return None;
    };
    if !matches!(
        op,
        Binary::Add
            | Binary::Subtract
            | Binary::Multiply
            | Binary::Divide
            | Binary::Remainder
            | Binary::ShiftLeft
            | Binary::ShiftRight
            | Binary::UnsignedShiftRight
            | Binary::BitAnd
            | Binary::BitOr
            | Binary::BitXor
    ) {
        return None;
    }
    let place = |id: ExprId| match &module.expressions[id.index()] {
        Expr::Binding(binding) => Some((Some(*binding), None)),
        Expr::Member {
            object,
            property: Property::Named(name),
        } => match module.expressions[object.index()] {
            Expr::Binding(binding) => Some((Some(binding), Some(name.as_str()))),
            Expr::This => Some((None, Some(name.as_str()))),
            _ => None,
        },
        _ => None,
    };
    (place(target)? == place(left)?).then_some((op, right))
}

impl Module {
    pub(crate) fn identify_spelling_sites(
        &mut self,
        head: u8,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        self.authored_sites = budget.copy_slice(Retained, &self.authored_expressions)?;
        self.spelling_head = head;
        self.spelling_regions = self.regions.len();
        self.spelling_functions = self.functions.len();
        self.spelling_node_count = self.expressions.len();
        self.spelling_nodes = budget.vector(Retained, self.expressions.len())?;
        for index in 0..self.expressions.len() {
            budget.work(Analysis, 1)?;
            self.spelling_nodes.push(Some(
                u32::try_from(index).map_err(|_| AllocationError::Capacity)?,
            ));
        }
        Ok(())
    }
    pub(super) fn region_site(&self, region: RegionId) -> Option<SiteId> {
        (region.index() < self.spelling_regions).then_some(SiteId::Target {
            head: self.spelling_head,
            kind: 1,
            ordinal: region.index() as u32,
        })
    }
    pub(super) fn expression_site(&self, id: ExprId) -> Option<SiteId> {
        self.spelling_nodes
            .get(id.index())
            .copied()
            .flatten()
            .map(|ordinal| SiteId::Target {
                head: self.spelling_head,
                kind: 0,
                ordinal,
            })
    }
    pub(super) fn site_choice(
        &mut self,
        site: Option<SiteId>,
        family: ChoiceFamily,
        seed: bool,
        name: &str,
        saving: i64,
        choices: Option<&ChoiceMap>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        let (Some(site), Some(choices)) = (site, choices) else {
            return Ok(seed);
        };
        use crate::representation::ChoiceAlternative;
        let key = ChoiceKey { family, site };
        let seed = AltId(u8::from(seed));
        let pin = self.authored_site(site).get(family);
        let applied = pin.or_else(|| choices.get(key).filter(|alt| alt.0 < 2)).unwrap_or(seed);
        budget.work(Analysis, self.choice_sites.len() as u64 + 1)?;
        if !self.choice_sites.iter().any(|s| s.key == key) {
            let name = budget.string(Retained, name)?;
            let alternatives = budget.copy_slice(
                Retained,
                &[
                    ChoiceAlternative {
                        alternative: AltId(0),
                        name: "canonical",
                        saving: 0,
                    },
                    ChoiceAlternative {
                        alternative: AltId(1),
                        name: "compact",
                        saving,
                    },
                ],
            )?;
            budget.push(
                Retained,
                &mut self.choice_sites,
                ChoiceSite {
                    key,
                    name,
                    alternatives,
                    seed,
                    applied,
                    pinned: pin.is_some(),
                },
            )?;
        }
        Ok(applied == AltId(1))
    }

    pub(crate) fn form_spelling_choices(
        &mut self,
        families: OutputFamilies,
        rules: TargetRules,
        choices: &ChoiceMap,
        frames_hidden: bool,
        year: u16,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        if rules.receiver_aliases && frames_hidden {
            self.form_receiver_aliases(choices, budget)?;
        }
        if rules.declaration_order {
            self.form_declaration_order(choices, budget)?;
        }
        self.refresh_print_choices(families, rules, choices, year, budget)
    }

    /// Delivery may append setters and replace writes with calls. Re-prove the
    /// print forms under the same assignments after those edits, without
    /// moving declarations or creating receiver aliases a second time.
    pub(crate) fn refresh_print_choices(
        &mut self,
        families: OutputFamilies,
        rules: TargetRules,
        choices: &ChoiceMap,
        year: u16,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        let mut forms = PrintForms::new(self, rules.statement_spellings, Retained, budget)?;
        if rules.statement_spellings {
            for index in 0..forms.loops.len() {
                let site = self.region_site(RegionId::new(index));
                if forms.loops[index].is_some()
                    && !self.site_choice(
                        site,
                        ChoiceFamily::LoopHeads,
                        families.loop_heads,
                        "loop-head",
                        3,
                        Some(choices),
                        budget,
                    )?
                {
                    forms.loops[index] = None;
                }
                if forms.logical[index].is_some()
                    && !self.site_choice(
                        site,
                        ChoiceFamily::LogicalStatements,
                        families.logical_statements,
                        "logical-statement",
                        3,
                        Some(choices),
                        budget,
                    )?
                {
                    forms.logical[index] = None;
                }
            }
            for index in 0..forms.compound.len() {
                let site = self.expression_site(ExprId::new(index));
                if forms.compound[index].is_some()
                    && forms.increment[index].is_none()
                    && !self.site_choice(
                        site,
                        ChoiceFamily::CompoundAssignments,
                        families.compound_assignments,
                        "compound-assignment",
                        2,
                        Some(choices),
                        budget,
                    )?
                {
                    forms.compound[index] = None;
                }
                if self.authored_site(site.unwrap_or(SiteId::Formed(u32::MAX))).get(ChoiceFamily::QuoteDelimiter) == Some(AltId(1))
                    && matches!(self.expressions[index], Expr::Literal(Literal::String(_))) {
                    forms.quotes[index] = true;
                }
                if forms.quotes[index]
                    && !self.site_choice(
                        site,
                        ChoiceFamily::QuoteDelimiter,
                        families.quotes,
                        "quote-delimiter",
                        1,
                        Some(choices),
                        budget,
                    )?
                {
                    forms.quotes[index] = false;
                }
            }
        }
        if rules.statement_spellings {
            self.form_modern_spellings(&mut forms, year, choices, budget)?;
        }
        if let Some(previous) = self.print_forms.replace(forms) {
            let bytes = previous.bytes();
            drop(previous);
            budget.release(Retained, bytes)?;
        }
        Ok(())
    }
}

impl Module {
    pub(super) fn statement_site_spellings(
        &mut self,
        region: RegionId,
        index: usize,
        statement: &Statement,
        seeds: StatementSpellings,
        choices: Option<&ChoiceMap>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<StatementSpellings, AllocationError> {
        let mut result = seeds;
        if choices.is_none() {
            return Ok(result);
        }
        match *statement {
            Statement::Loop {
                update: None, body, ..
            } if matches!(
                self.regions[body.index()].statements.last(),
                Some(Statement::Evaluate(_))
            ) =>
            {
                result.loop_fusion = self.site_choice(
                    self.region_site(body),
                    ChoiceFamily::LoopFusion,
                    seeds.loop_fusion,
                    "loop-update",
                    2,
                    choices,
                    budget,
                )?;
            }
            Statement::If { yes, no, .. } => {
                let site = self.region_site(yes);
                let yes_statements = &self.regions[yes.index()].statements;
                let no_statements = no.map(|no| self.regions[no.index()].statements.as_slice());
                let returns = matches!(yes_statements.as_slice(), [Statement::Return(_)])
                    && (no_statements
                        .is_some_and(|statements| matches!(statements, [Statement::Return(_)]))
                        || no.is_none()
                            && self.regions[region.index()].statements[index + 1..]
                                .iter()
                                .find(|statement| !matches!(statement, Statement::Evaluate(_)))
                                .is_some_and(|statement| {
                                    matches!(statement, Statement::Return(_))
                                }));
                let exits = no.is_none()
                    && index + 1 < self.regions[region.index()].statements.len()
                    && matches!(
                        yes_statements.last(),
                        Some(Statement::Return(None) | Statement::Continue)
                    );
                let assignments = |statements: &[Statement]| matches!(statements,[Statement::Evaluate(id)] if matches!(self.expressions[id.index()],Expr::Assign{..}));
                let values = assignments(yes_statements) && no_statements.is_some_and(assignments);
                let evaluations = |statements: &[Statement]| {
                    statements
                        .iter()
                        .all(|s| matches!(s, Statement::Evaluate(_)))
                };
                let logical = (evaluations(yes_statements)
                    && no_statements.is_none_or(evaluations))
                    || no.is_none()
                        && matches!(yes_statements.as_slice(), [Statement::If { no: None, .. }]);
                if returns {
                    result.conditional_returns = self.site_choice(
                        site,
                        ChoiceFamily::ConditionalReturns,
                        seeds.conditional_returns,
                        "conditional-return",
                        3,
                        choices,
                        budget,
                    )?;
                }
                if exits {
                    result.exit_points = self.site_choice(
                        site,
                        ChoiceFamily::ExitPoints,
                        seeds.exit_points,
                        "exit-point",
                        3,
                        choices,
                        budget,
                    )?;
                }
                if values {
                    result.conditional_values = self.site_choice(
                        site,
                        ChoiceFamily::ConditionalValues,
                        seeds.conditional_values,
                        "conditional-value",
                        3,
                        choices,
                        budget,
                    )?;
                }
                if logical {
                    result.logical_branches = self.site_choice(
                        site,
                        ChoiceFamily::LogicalBranches,
                        seeds.logical_branches,
                        "logical-branch",
                        3,
                        choices,
                        budget,
                    )?;
                }
            }
            _ => {}
        }
        Ok(result)
    }
}

impl Module {
    fn receiver_reads_in(
        &self,
        body: RegionId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(Vec<ExprId>, bool), AllocationError> {
        let mut regions = vec![body];
        let mut expressions: Vec<ExprId> = Vec::new();
        let mut reads = Vec::new();
        let mut unsafe_entry = false;
        while !regions.is_empty() || !expressions.is_empty() {
            if let Some(id) = expressions.pop() {
                budget.work(Analysis, 1)?;
                let expression = &self.expressions[id.index()];
                match expression {
                    Expr::This => reads.push(id),
                    Expr::SuperCall { .. } => unsafe_entry = true,
                    Expr::Host(host) if host.kind == crate::catalog::HostKind::Eval => {
                        unsafe_entry = true
                    }
                    _ => {}
                }
                // Function bodies have separate owners; class heritage is an
                // ordinary child evaluated here, unlike its methods.
                expression.visit_children(|child| {
                    expressions.push(child);
                    Ok::<_, AllocationError>(())
                })?;
            } else if let Some(region) = regions.pop() {
                for statement in &self.regions[region.index()].statements {
                    budget.work(Analysis, 1)?;
                    if !matches!(statement, Statement::Function { .. }) {
                        statement.visit_expressions(|root| expressions.push(root));
                        statement.visit_regions(|child| regions.push(child));
                    }
                }
            }
        }
        Ok((reads, unsafe_entry))
    }
    fn form_receiver_aliases(
        &mut self,
        choices: &ChoiceMap,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        let mut derived = budget.filled(Scratch, self.functions.len(), false)?;
        for expression in &self.expressions {
            budget.work(Analysis, 1)?;
            if let Expr::Class {
                base: Some(_),
                constructor: Some(function),
                ..
            } = expression
            {
                derived[function.index()] = true;
            }
        }
        let mut ownership = budget.filled(Scratch, self.expressions.len(), None::<usize>)?;
        let mut ambiguous = budget.filled(Scratch, self.expressions.len(), false)?;
        let (root_reads, _) = self.receiver_reads_in(self.root, budget)?;
        for id in root_reads {
            ownership[id.index()] = Some(usize::MAX);
        }
        // A shared expression node must not acquire an alias owned by a
        // different function, including a nested arrow's lexical-this frame.
        for index in 0..self.functions.len() {
            let (reads, _) = self.receiver_reads_in(self.functions[index].body, budget)?;
            for id in reads {
                if ownership[id.index()].is_some_and(|owner| owner != index) {
                    ambiguous[id.index()] = true;
                }
                ownership[id.index()] = Some(index);
            }
        }
        for index in 0..self.spelling_functions.min(self.functions.len()) {
            let function = &self.functions[index];
            // A derived constructor may return before super, or run it in a
            // nested arrow. Only ordinary initialized receiver frames qualify.
            if function.arrow || derived[index] {
                continue;
            }
            let body = function.body;
            let (reads, unsafe_entry) =
                self.receiver_reads_in(self.functions[index].body, budget)?;
            if unsafe_entry || reads.len() < 4 || reads.iter().any(|id| ambiguous[id.index()]) {
                continue;
            }
            let site = Some(SiteId::Target {
                head: self.spelling_head,
                kind: 2,
                ordinal: index as u32,
            });
            if !self.site_choice(
                site,
                ChoiceFamily::ReceiverAlias,
                false,
                "receiver-alias",
                reads.len() as i64 * 3 - 12,
                Some(choices),
                budget,
            )? {
                continue;
            }
            let spelling = budget.string(Retained, "receiver")?;
            let binding = self.binding_in(
                Binding {
                    source_symbol: None,
                    scope: self.regions[body.index()].scope,
                    spelling,
                    pinned: false,
                    class: None,
                    defined: false,
                },
                budget,
            )?;
            let receiver = self.expression_in(Expr::This, None, budget)?;
            for id in reads {
                self.set_expression(id, Expr::Binding(binding));
            }
            budget.reserve_vec(Retained, &mut self.regions[body.index()].statements, 1)?;
            self.statements_mut(body.index()).insert(
                0,
                Statement::Let {
                    binding,
                    value: Some(receiver),
                },
            );
        }
        let scratch = (ownership.capacity() * std::mem::size_of::<Option<usize>>()
            + ambiguous.capacity()
            + derived.capacity()) as u64;
        drop(ownership);
        drop(ambiguous);
        drop(derived);
        budget.release(Scratch, scratch)?;
        Ok(())
    }

    fn form_declaration_order(
        &mut self,
        choices: &ChoiceMap,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        for region in 0..self.regions.len() {
            let root = region == self.root.index();
            if root && self.root_rows.len() != self.regions[region].statements.len() {
                continue;
            }
            let mut begin = 0;
            while begin < self.regions[region].statements.len() {
                budget.work(Analysis, 1)?;
                if !self.movable_declaration(&self.regions[region].statements[begin])
                    || root && self.root_rows[begin].anchor != Anchor::Definition
                {
                    begin += 1;
                    continue;
                }
                let mut end = begin + 1;
                while end < self.regions[region].statements.len()
                    && self.movable_declaration(&self.regions[region].statements[end])
                    && (!root
                        || self.root_rows[end].module == self.root_rows[begin].module
                            && self.root_rows[end].anchor == self.root_rows[begin].anchor)
                {
                    end += 1;
                }
                if end - begin >= 2 {
                    let mut order = Vec::new();
                    for index in begin..end {
                        order.push((
                            self.declaration_key(&self.regions[region].statements[index], budget)?,
                            index,
                        ));
                    }
                    budget.work(
                        Analysis,
                        (order.len() * (order.len().ilog2() as usize + 1)) as u64,
                    )?;
                    order.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
                    if order
                        .iter()
                        .enumerate()
                        .any(|(offset, (_, index))| *index != begin + offset)
                    {
                        let binding = match self.regions[region].statements[begin] {
                            Statement::Let { binding, .. }
                            | Statement::Function { binding, .. } => binding,
                            _ => unreachable!(),
                        };
                        if self.site_choice(
                            Some(SiteId::Target {
                                head: self.spelling_head,
                                kind: 3,
                                ordinal: binding.index() as u32,
                            }),
                            ChoiceFamily::DeclarationOrder,
                            false,
                            "locality-order",
                            0,
                            Some(choices),
                            budget,
                        )? {
                            let statements: Vec<_> = order
                                .iter()
                                .map(|(_, index)| self.regions[region].statements[*index].clone())
                                .collect();
                            let rows: Vec<_> = if root {
                                order
                                    .iter()
                                    .map(|(_, index)| self.root_rows[*index])
                                    .collect()
                            } else {
                                Vec::new()
                            };
                            for (offset, statement) in statements.into_iter().enumerate() {
                                self.statements_mut(region)[begin + offset] = statement;
                            }
                            if root {
                                self.root_rows[begin..end].copy_from_slice(&rows);
                            }
                        }
                    }
                }
                begin = end;
            }
        }
        Ok(())
    }
    fn movable_declaration(&self, statement: &Statement) -> bool {
        match statement {
            Statement::Function { .. } => true,
            Statement::Let {
                value: Some(value), ..
            } => matches!(
                self.expressions[value.index()],
                Expr::Literal(_) | Expr::Function(_)
            ),
            Statement::Let { value: None, .. } => true,
            _ => false,
        }
    }
    fn declaration_key(
        &self,
        statement: &Statement,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Vec<u8>, AllocationError> {
        let mut key = Vec::new();
        let mut expressions: Vec<ExprId> = Vec::new();
        let mut regions = Vec::new();
        match statement {
            Statement::Function { function, .. } => {
                key.push(0);
                regions.push(self.functions[function.index()].body);
            }
            Statement::Let {
                value: Some(value), ..
            } => {
                if let Expr::Function(function) = self.expressions[value.index()] {
                    key.push(0);
                    regions.push(self.functions[function.index()].body);
                } else {
                    key.push(1);
                    expressions.push(*value);
                }
            }
            _ => key.push(2),
        }
        // A bounded shape prefix groups similar bodies and literal prefixes.
        // It only schedules a legal permutation; exact full bytes decide.
        for _ in 0..256 {
            budget.work(Analysis, 1)?;
            if let Some(id) = expressions.pop() {
                let expression = &self.expressions[id.index()];
                match expression {
                    Expr::Literal(Literal::String(value)) => {
                        key.push(1);
                        if let Some(text) = value.as_unicode() {
                            key.extend(text.as_bytes().iter().take(64));
                        }
                    }
                    Expr::Literal(Literal::Number(_)) => key.push(2),
                    Expr::Literal(_) => key.push(3),
                    Expr::Binding(_) => key.push(4),
                    Expr::Binary { op, .. } => {
                        key.push(5);
                        key.push(*op as u8);
                    }
                    Expr::Call { .. } => key.push(6),
                    Expr::Member { .. } => key.push(7),
                    Expr::Function(_) => key.push(8),
                    Expr::Conditional { .. } => key.push(9),
                    Expr::Assign { .. } => key.push(10),
                    _ => key.push(11),
                }
                expression.visit_children(|child| {
                    expressions.push(child);
                    Ok::<_, AllocationError>(())
                })?;
            } else if let Some(region) = regions.pop() {
                for statement in self.regions[region.index()].statements.iter().rev() {
                    statement.visit_expressions(|root| expressions.push(root));
                    statement.visit_regions(|child| regions.push(child));
                }
            } else {
                break;
            }
        }
        Ok(key)
    }
}

impl Module {
    pub(super) fn inherit_region_spelling(
        &mut self,
        value: ExprId,
        region: RegionId,
    ) -> Result<(), AllocationError> {
        if region.index() < self.spelling_regions && !self.spelling_nodes.is_empty() {
            let ordinal = self
                .spelling_node_count
                .checked_add(region.index())
                .and_then(|id| u32::try_from(id).ok())
                .ok_or(AllocationError::Capacity)?;
            self.spelling_nodes[value.index()] = Some(ordinal);
            if !self.authored_expressions.is_empty() { self.authored_expressions[value.index()] = self.region_choices(region); }
        }
        Ok(())
    }
    /// Exactly null OR undefined, with no HTMLDDA widening and no repeated
    /// getter. Typed loose tests remain a separate canonical normalization.
    fn exact_nullish_binding(&self, condition: ExprId) -> Option<BindingId> {
        let Expr::Binary {
            op: Binary::Or,
            left,
            right,
        } = self.expressions[condition.index()]
        else {
            return None;
        };
        let part = |id: ExprId| {
            let Expr::Binary {
                op: Binary::StrictEqual,
                left,
                right,
            } = self.expressions[id.index()]
            else {
                return None;
            };
            for (value, nothing) in [(left, right), (right, left)] {
                if let Expr::Binding(binding) = self.expressions[value.index()] {
                    match self.expressions[nothing.index()] {
                        Expr::Literal(Literal::Null) => return Some((binding, false)),
                        Expr::Literal(Literal::Undefined) => return Some((binding, true)),
                        _ => {}
                    }
                }
            }
            None
        };
        let (a, ak) = part(left)?;
        let (b, bk) = part(right)?;
        (a == b && ak != bk).then_some(a)
    }
    fn form_modern_spellings(
        &mut self,
        forms: &mut PrintForms,
        year: u16,
        choices: &ChoiceMap,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        if year >= 2020 {
            for index in 0..self.expressions.len() {
                budget.work(Analysis, 1)?;
                let Expr::Conditional { condition, yes, no } = self.expressions[index] else {
                    continue;
                };
                if !matches!(
                    self.expressions[yes.index()],
                    Expr::Literal(Literal::Undefined)
                ) {
                    continue;
                }
                let Some(binding) = self.exact_nullish_binding(condition) else {
                    continue;
                };
                let Expr::Member { object, .. } = self.expressions[no.index()] else {
                    continue;
                };
                if !matches!(self.expressions[object.index()],Expr::Binding(found) if found==binding)
                {
                    continue;
                }
                if self.site_choice(
                    self.expression_site(ExprId::new(index)),
                    ChoiceFamily::OptionalChain,
                    false,
                    "optional-member",
                    8,
                    Some(choices),
                    budget,
                )? {
                    forms.optional[index] = Some(no);
                }
            }
        }
        if year >= 2021 {
            for region in 0..self.regions.len() {
                for index in 0..self.regions[region].statements.len() {
                    budget.work(Analysis, 1)?;
                    let Statement::If {
                        condition,
                        yes,
                        no: None,
                    } = self.regions[region].statements[index]
                    else {
                        continue;
                    };
                    let [Statement::Evaluate(assignment)] =
                        self.regions[yes.index()].statements.as_slice()
                    else {
                        continue;
                    };
                    let Expr::Assign { target, value } = self.expressions[assignment.index()]
                    else {
                        continue;
                    };
                    let Expr::Binding(binding) = self.expressions[target.index()] else {
                        continue;
                    };
                    let op = match self.expressions[condition.index()] {
                        Expr::Binding(found) if found == binding => Some(Binary::And),
                        Expr::Unary {
                            op: Unary::Not,
                            value,
                        } if matches!(self.expressions[value.index()],Expr::Binding(found) if found==binding) => {
                            Some(Binary::Or)
                        }
                        _ if self.exact_nullish_binding(condition) == Some(binding) => {
                            Some(Binary::Nullish)
                        }
                        _ => None,
                    };
                    let Some(op) = op else {
                        continue;
                    };
                    if self.site_choice(
                        self.region_site(yes),
                        ChoiceFamily::LogicalAssignment,
                        false,
                        "logical-assignment",
                        4,
                        Some(choices),
                        budget,
                    )? {
                        forms.logical_assignments[yes.index()] = Some(Logical {
                            condition,
                            op,
                            left: target,
                            right: value,
                        });
                    }
                }
            }
        }
        Ok(())
    }
}
