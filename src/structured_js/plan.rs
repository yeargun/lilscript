//! One owner for a proposed target edit and a borrowed view of its operands.
//! The source program stays immutable until the complete plan is applied.
//! Sparse payloads have a compact index only when observation clients need
//! random access; unchanged expressions and statements have no copied payload.

use super::*;
use std::num::NonZeroU32;

struct Edits<T> {
    domain: usize,
    entries: Vec<(usize, T)>,
    positions: Option<Vec<Option<NonZeroU32>>>,
    ordered: bool,
}

impl<T> Edits<T> {
    fn new(domain: usize, mut entries: Vec<(usize, T)>) -> Self {
        entries.sort_unstable_by_key(|(index, _)| *index);
        debug_assert!(entries.iter().all(|(index, _)| *index < domain));
        debug_assert!(entries.windows(2).all(|pair| pair[0].0 != pair[1].0));
        Self {
            domain,
            entries,
            positions: None,
            ordered: true,
        }
    }

    fn position(&self, index: usize) -> Option<usize> {
        match &self.positions {
            Some(positions) => positions[index].map(|slot| slot.get() as usize - 1),
            None => self
                .entries
                .binary_search_by_key(&index, |(index, _)| *index)
                .ok(),
        }
    }

    fn get(&self, index: usize) -> Option<&T> {
        self.position(index)
            .map(|position| &self.entries[position].1)
    }

    fn set(&mut self, index: usize, value: T) {
        if let Some(position) = self.position(index) {
            self.entries[position].1 = value;
            return;
        }
        self.index();
        let positions = self.positions.as_mut().unwrap();
        positions[index] = Some(Self::slot(self.entries.len()));
        self.entries.push((index, value));
        self.ordered = false;
    }

    fn index(&mut self) {
        self.positions.get_or_insert_with(|| {
            let mut positions = vec![None; self.domain];
            for (position, (index, _)) in self.entries.iter().enumerate() {
                positions[*index] = Some(Self::slot(position));
            }
            positions
        });
    }

    fn slot(position: usize) -> NonZeroU32 {
        u32::try_from(position)
            .ok()
            .and_then(|index| index.checked_add(1))
            .and_then(NonZeroU32::new)
            .expect("planned edit capacity")
    }

    fn into_entries(mut self) -> Vec<(usize, T)> {
        if !self.ordered {
            self.entries.sort_unstable_by_key(|(index, _)| *index);
        }
        self.entries
    }
}

pub(super) type RegionEdits = Vec<(RegionId, Vec<(usize, Option<Statement>)>)>;

pub(super) struct Plan {
    pub aliases: Option<Vec<Option<ExprId>>>,
    expressions: Edits<rewrite::Replacement>,
    regions: Edits<Edits<Option<Statement>>>,
}

impl Plan {
    pub fn new(
        module: &Module,
        aliases: Option<Vec<Option<ExprId>>>,
        expressions: Vec<(usize, rewrite::Replacement)>,
        regions: RegionEdits,
    ) -> Self {
        Self {
            aliases,
            expressions: Edits::new(module.expressions.len(), expressions),
            regions: Edits::new(
                module.regions.len(),
                regions
                    .into_iter()
                    .map(|(id, edits)| {
                        (
                            id.index(),
                            Edits::new(module.regions[id.index()].statements.len(), edits),
                        )
                    })
                    .collect(),
            ),
        }
    }

    pub fn replacement(&self, id: ExprId) -> Option<&rewrite::Replacement> {
        self.expressions.get(id.index())
    }

    pub fn set_expression(&mut self, id: ExprId, replacement: rewrite::Replacement) {
        debug_assert!(self
            .aliases
            .as_ref()
            .is_none_or(|aliases| aliases[id.index()].is_none()));
        self.expressions.set(id.index(), replacement);
    }

    pub fn transfer(&mut self, module: &Module, read: ExprId, initializer: ExprId) {
        debug_assert!(initializer < read);
        debug_assert!(self.replacement(read).is_none());
        let aliases = self
            .aliases
            .get_or_insert_with(|| vec![None; module.expressions.len()]);
        debug_assert!(aliases[read.index()].is_none());
        aliases[read.index()] = Some(initializer);
    }

    pub fn statement<'a>(
        &'a self,
        module: &'a Module,
        site: analysis::Site,
    ) -> Option<&'a Statement> {
        match self
            .regions
            .get(site.region.index())
            .and_then(|edits| edits.get(site.statement))
        {
            Some(statement) => statement.as_ref(),
            None => Some(&module.regions[site.region.index()].statements[site.statement]),
        }
    }

    pub fn set_statement(
        &mut self,
        module: &Module,
        site: analysis::Site,
        statement: Option<Statement>,
    ) {
        if let Some(position) = self.regions.position(site.region.index()) {
            self.regions.entries[position]
                .1
                .set(site.statement, statement);
        } else {
            self.regions.set(
                site.region.index(),
                Edits::new(
                    module.regions[site.region.index()].statements.len(),
                    vec![(site.statement, statement)],
                ),
            );
        }
    }

    /// Forwarding changes transfer an existing occurrence. They neither load
    /// a new binding nor grant permission to duplicate the referenced value.
    pub fn resolve(&self, mut id: ExprId) -> ExprId {
        loop {
            let next = self
                .aliases
                .as_ref()
                .and_then(|aliases| aliases[id.index()])
                .or_else(|| match self.replacement(id) {
                    Some(rewrite::Replacement::Value(value)) => Some(*value),
                    _ => None,
                });
            let Some(next) = next else {
                return id;
            };
            debug_assert!(next < id, "planned inputs retain postorder ownership");
            id = next;
        }
    }

    pub fn visit_inputs(&self, module: &Module, id: ExprId, mut visit: impl FnMut(ExprId)) {
        let id = self.resolve(id);
        let original = &module.expressions[id.index()];
        if let Some(replacement) = self.replacement(id) {
            replacement.visit_inputs(original, visit);
        } else {
            original
                .visit_children::<()>(|value| {
                    visit(value);
                    Ok(())
                })
                .unwrap();
        }
    }

    pub fn is_literal(&self, module: &Module, id: ExprId) -> bool {
        let id = self.resolve(id);
        match self.replacement(id) {
            Some(rewrite::Replacement::Constant(_) | rewrite::Replacement::Literal(_)) => true,
            Some(_) => false,
            None => matches!(module.expressions[id.index()], Expr::Literal(_)),
        }
    }

    pub fn into_parts(
        self,
    ) -> (
        Option<Vec<Option<ExprId>>>,
        Vec<(usize, rewrite::Replacement)>,
        RegionEdits,
    ) {
        (
            self.aliases,
            self.expressions.into_entries(),
            self.regions
                .into_entries()
                .into_iter()
                .map(|(index, edits)| (RegionId::new(index), edits.into_entries()))
                .collect(),
        )
    }
}

/// Check the applied reachable program independently of the edit/delta walk.
/// Arena storage can still contain orphans here; compaction has not run yet.
pub(super) fn check_observations(module: &Module, expected: &[usize]) {
    #[cfg(debug_assertions)]
    {
        enum Task {
            Region(RegionId),
            Expression(ExprId, bool),
        }
        let mut counts = vec![0; module.bindings.len()];
        let mut tasks = vec![Task::Region(module.root)];
        while let Some(task) = tasks.pop() {
            match task {
                Task::Region(id) => {
                    for statement in &module.regions[id.index()].statements {
                        statement.visit_expressions(|id| tasks.push(Task::Expression(id, false)));
                        match statement {
                            Statement::If { yes, no, .. } => {
                                tasks.push(Task::Region(*yes));
                                if let Some(no) = no {
                                    tasks.push(Task::Region(*no));
                                }
                            }
                            Statement::Block(body)
                            | Statement::Loop { body, .. }
                            | Statement::ForIn { body, .. }
                            | Statement::ForOf { body, .. } => {
                                tasks.push(Task::Region(*body));
                            }
                            Statement::Try {
                                body,
                                catch,
                                finally,
                            } => {
                                tasks.push(Task::Region(*body));
                                if let Some(catch) = catch {
                                    tasks.push(Task::Region(catch.body));
                                }
                                if let Some(finally) = finally {
                                    tasks.push(Task::Region(*finally));
                                }
                            }
                            Statement::Function { function, .. } => {
                                tasks.push(Task::Region(module.functions[function.index()].body));
                            }
                            _ => {}
                        }
                    }
                }
                Task::Expression(id, place) => match &module.expressions[id.index()] {
                    Expr::Binding(binding) if !place => counts[binding.index()] += 1,
                    Expr::Assign { target, value } => {
                        tasks.push(Task::Expression(*target, true));
                        tasks.push(Task::Expression(*value, false));
                    }
                    Expr::Function(function) => {
                        tasks.push(Task::Region(module.functions[function.index()].body));
                    }
                    Expr::Class {
                        base, constructor, ..
                    } => {
                        tasks.push(Task::Region(module.functions[constructor.index()].body));
                        tasks.push(Task::Expression(*base, false));
                    }
                    expression => expression
                        .visit_children::<()>(|id| {
                            tasks.push(Task::Expression(id, false));
                            Ok(())
                        })
                        .unwrap(),
                },
            }
        }
        assert_eq!(
            counts, expected,
            "planned observations differ from applied reads"
        );
    }
    #[cfg(not(debug_assertions))]
    let _ = (module, expected);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Input {
    Expression(ExprId),
    Place(ExprId),
    Region(RegionId),
}

fn expression_inputs(module: &Module, id: ExprId, mut visit: impl FnMut(Input)) {
    match &module.expressions[id.index()] {
        Expr::Assign { target, value } => {
            visit(Input::Place(*target));
            visit(Input::Expression(*value));
        }
        Expr::Function(function) => visit(Input::Region(module.functions[function.index()].body)),
        Expr::Class {
            base, constructor, ..
        } => {
            visit(Input::Expression(*base));
            visit(Input::Region(module.functions[constructor.index()].body));
        }
        expression => expression
            .visit_children::<()>(|id| {
                visit(Input::Expression(id));
                Ok(())
            })
            .unwrap(),
    }
}

fn statement_inputs(module: &Module, statement: &Statement, mut visit: impl FnMut(Input)) {
    statement.visit_expressions(|value| visit(Input::Expression(value)));
    match statement {
        Statement::If { yes, no, .. } => {
            visit(Input::Region(*yes));
            if let Some(no) = no {
                visit(Input::Region(*no));
            }
        }
        Statement::Loop { body, .. }
        | Statement::Block(body)
        | Statement::ForIn { body, .. }
        | Statement::ForOf { body, .. } => {
            visit(Input::Region(*body))
        }
        Statement::Try {
            body,
            catch,
            finally,
        } => {
            visit(Input::Region(*body));
            if let Some(catch) = catch {
                visit(Input::Region(catch.body));
            }
            if let Some(finally) = finally {
                visit(Input::Region(*finally));
            }
        }
        Statement::Function { function, .. } => {
            visit(Input::Region(module.functions[function.index()].body))
        }
        _ => {}
    }
}

/// Counts refer to syntax occurrences in the proposed program. Existing
/// unique-ownership proofs make deletions and transfers explicit; this is not
/// a new semantic/dataflow analysis or complete recursive-SCC liveness.
struct Observations<'a> {
    module: &'a Module,
    counts: Vec<usize>,
    changed: Vec<BindingId>,
    queued: Vec<bool>,
    removed_expressions: Vec<bool>,
    removed_regions: Vec<bool>,
    moved: Vec<bool>,
    stack: Vec<Input>,
    retained: Vec<Input>,
}

impl<'a> Observations<'a> {
    fn new(
        module: &'a Module,
        analysis: &analysis::Analysis<'_, '_, '_>,
        structure: &verify::Structure,
    ) -> Self {
        let mut counts = vec![0; module.bindings.len()];
        for (index, count) in counts.iter_mut().enumerate() {
            let binding = BindingId::new(index);
            for read in &analysis.binding_uses(binding).reads {
                if structure.live_expressions[read.expression.index()] {
                    *count += 1;
                }
            }
        }
        Self {
            module,
            counts,
            changed: vec![],
            queued: vec![false; module.bindings.len()],
            removed_expressions: structure
                .live_expressions
                .iter()
                .map(|live| !live)
                .collect(),
            removed_regions: vec![false; module.regions.len()],
            moved: vec![false; module.expressions.len()],
            stack: vec![],
            retained: vec![],
        }
    }

    fn forget_read(&mut self, id: ExprId) -> Option<BindingId> {
        if !std::mem::replace(&mut self.removed_expressions[id.index()], true) {
            let Expr::Binding(binding) = self.module.expressions[id.index()] else {
                unreachable!("an original read retains its binding identity")
            };
            self.counts[binding.index()] -= 1;
            return Some(binding);
        }
        None
    }

    fn remove_read(&mut self, id: ExprId) {
        if let Some(binding) = self.forget_read(id) {
            if !self.queued[binding.index()] {
                self.changed.push(binding);
                self.queued[binding.index()] = true;
            }
        }
    }

    fn resolve(plan: &Plan, input: Input) -> Input {
        match input {
            Input::Expression(value) => Input::Expression(plan.resolve(value)),
            input => input,
        }
    }

    fn drain_removed(&mut self, plan: &Plan, keep: &[Input]) {
        let module = self.module;
        while let Some(input) = self.stack.pop() {
            let input = Self::resolve(plan, input);
            if keep.contains(&input) {
                continue;
            }
            match input {
                Input::Expression(id) => {
                    if self.removed_expressions[id.index()] {
                        continue;
                    }
                    if matches!(self.module.expressions[id.index()], Expr::Binding(_)) {
                        debug_assert!(plan.replacement(id).is_none());
                        self.remove_read(id);
                        continue;
                    }
                    self.removed_expressions[id.index()] = true;
                    if let Some(replacement) = plan.replacement(id) {
                        replacement.visit_inputs(&module.expressions[id.index()], |value| {
                            self.stack.push(Input::Expression(value))
                        });
                    } else {
                        expression_inputs(module, id, |input| self.stack.push(input));
                    }
                }
                Input::Place(id) => {
                    // The place itself does not load a lexical cell. A member
                    // place still evaluates its base and computed property.
                    if !std::mem::replace(&mut self.removed_expressions[id.index()], true) {
                        expression_inputs(module, id, |input| self.stack.push(input));
                    }
                }
                Input::Region(id) => {
                    if std::mem::replace(&mut self.removed_regions[id.index()], true) {
                        continue;
                    }
                    for statement in 0..module.regions[id.index()].statements.len() {
                        let site = analysis::Site {
                            region: id,
                            statement,
                        };
                        if let Some(statement) = plan.statement(module, site) {
                            statement_inputs(module, statement, |input| self.stack.push(input));
                        }
                    }
                }
            }
        }
    }

    fn replace_expression(&mut self, plan: &Plan, id: ExprId, replacement: &rewrite::Replacement) {
        let original = &self.module.expressions[id.index()];
        let mut keep = std::mem::take(&mut self.retained);
        debug_assert!(keep.is_empty());
        replacement.visit_inputs(original, |value| {
            keep.push(Input::Expression(plan.resolve(value)));
        });
        expression_inputs(self.module, id, |input| self.stack.push(input));
        self.drain_removed(plan, &keep);
        keep.clear();
        self.retained = keep;
    }

    fn replace_statement(
        &mut self,
        plan: &Plan,
        original: &Statement,
        replacement: Option<&Statement>,
    ) {
        let mut keep = std::mem::take(&mut self.retained);
        debug_assert!(keep.is_empty());
        if let Some(statement) = replacement {
            statement_inputs(self.module, statement, |input| {
                keep.push(Self::resolve(plan, input))
            });
        }
        let moved_initializer = match original {
            Statement::Let {
                value: Some(value), ..
            } if self.moved[value.index()] => Some(*value),
            _ => None,
        };
        statement_inputs(self.module, original, |input| {
            if !matches!(input, Input::Expression(value) if Some(value) == moved_initializer) {
                self.stack.push(input);
            }
        });
        self.drain_removed(plan, &keep);
        keep.clear();
        self.retained = keep;
    }
}

/// A fast original proof remains valid because these rewrites introduce no
/// observable effects. Otherwise inspect retained operands and the operation's
/// own contribution. In particular a parent's WRITE/THROW cannot be subtracted
/// merely because a removed child carried the same bit.
fn discardable(
    module: &Module,
    plan: &Plan,
    analysis: &mut analysis::Analysis<'_, '_, '_>,
    root: ExprId,
    stack: &mut Vec<ExprId>,
) -> bool {
    debug_assert!(stack.is_empty());
    stack.push(root);
    while let Some(id) = stack.pop() {
        let id = plan.resolve(id);
        if plan.is_literal(module, id) {
            continue;
        }
        let original = analysis.facts(id);
        if original.effects.discardable() {
            continue;
        }
        let own = match plan.replacement(id) {
            Some(rewrite::Replacement::Constant(_) | rewrite::Replacement::Literal(_)) => continue,
            Some(
                rewrite::Replacement::Value(_)
                | rewrite::Replacement::IntegerOffset { .. }
                | rewrite::Replacement::Effects(_),
            ) => analysis::Effects::default(),
            _ => original.own_effects,
        };
        if !own.discardable() {
            stack.clear();
            return false;
        }
        plan.visit_inputs(module, id, |value| stack.push(value));
    }
    true
}

#[derive(Clone, Copy)]
struct PlacementSite {
    height: usize,
    forwarded: Option<usize>,
}

/// A moved initializer transfers its whole statement's expression ownership.
/// Original reads follow that owner instead of updating every descendant site.
struct Placement {
    regions: Vec<Option<Vec<PlacementSite>>>,
}
impl Placement {
    fn region<'a>(
        &'a mut self,
        module: &Module,
        structure: &verify::Structure,
        id: RegionId,
    ) -> &'a mut [PlacementSite] {
        self.regions[id.index()].get_or_insert_with(|| {
            module.regions[id.index()]
                .statements
                .iter()
                .map(|statement| {
                    let mut height = 0;
                    statement.visit_expressions(|id| {
                        height = height.max(structure.expression_heights[id.index()])
                    });
                    PlacementSite {
                        height,
                        forwarded: None,
                    }
                })
                .collect()
        })
    }
    fn resolve(&mut self, mut site: analysis::Site) -> analysis::Site {
        let Some(region) = &mut self.regions[site.region.index()] else {
            return site;
        };
        let original = site.statement;
        while let Some(next) = region[site.statement].forwarded {
            debug_assert!(next > site.statement);
            site.statement = next;
        }
        let mut current = original;
        while let Some(next) = region[current].forwarded {
            region[current].forwarded = Some(site.statement);
            current = next;
        }
        site
    }
    fn transfer(
        &mut self,
        module: &Module,
        structure: &verify::Structure,
        from: analysis::Site,
        to: analysis::Site,
        literal: bool,
    ) -> bool {
        debug_assert_eq!(from.region, to.region);
        debug_assert!(from.statement < to.statement);
        let region = self.region(module, structure, from.region);
        let height = if literal {
            1
        } else {
            region[from.statement].height
        };
        let proposed = region[to.statement]
            .height
            .saturating_add(height.saturating_sub(1));
        if !structure.region_depths[from.region.index()]
            .is_some_and(|depth| proposed <= verify::MAX_NESTING - depth)
        {
            return false;
        }
        debug_assert!(region[from.statement].forwarded.is_none());
        region[from.statement].forwarded = Some(to.statement);
        region[to.statement].height = proposed;
        true
    }
}

pub(super) struct Planner<'tree, 'sem, 'src> {
    module: &'tree Module,
    structure: &'tree verify::Structure,
    analysis: analysis::Analysis<'tree, 'sem, 'src>,
    world: crate::compilation_contract::JavaScriptWorld,
    owned_data: optimize::OwnedData,
    plan: Plan,
    observations: Observations<'tree>,
    placement: Placement,
    effect_stack: Vec<ExprId>,
    touched: Vec<analysis::Site>,
}

impl<'tree, 'sem, 'src> Planner<'tree, 'sem, 'src> {
    pub fn new(
        module: &'tree Module,
        structure: &'tree verify::Structure,
        analysis: analysis::Analysis<'tree, 'sem, 'src>,
        world: crate::compilation_contract::JavaScriptWorld,
        owned_data: optimize::OwnedData,
    ) -> Self {
        let observations = Observations::new(module, &analysis, structure);
        Self {
            module,
            structure,
            analysis,
            world,
            owned_data,
            plan: Plan::new(module, None, vec![], vec![]),
            observations,
            placement: Placement {
                regions: vec![None; module.regions.len()],
            },
            effect_stack: vec![],
            touched: vec![],
        }
    }

    fn effectless(&mut self, value: ExprId) -> bool {
        discardable(
            self.module,
            &self.plan,
            &mut self.analysis,
            value,
            &mut self.effect_stack,
        )
    }

    fn change_expression(&mut self, id: ExprId, replacement: rewrite::Replacement) {
        debug_assert!(self.plan.replacement(id).is_none());
        self.observations
            .replace_expression(&self.plan, id, &replacement);
        self.plan.set_expression(id, replacement);
    }

    fn change_statement(&mut self, site: analysis::Site, replacement: Option<Statement>) {
        let original = self
            .plan
            .statement(self.module, site)
            .cloned()
            .expect("one current statement owner");
        self.observations
            .replace_statement(&self.plan, &original, replacement.as_ref());
        self.plan.set_statement(self.module, site, replacement);
    }

    pub fn run(&mut self, report: &mut optimize::Report) {
        // Seed declarations in the verifier's child-first region order and
        // declaration order. Subsequent changes use the same worklist/policy.
        for &id in self.structure.region_postorder.iter().rev() {
            for (statement, node) in self.module.regions[id.index()]
                .statements
                .iter()
                .enumerate()
                .rev()
            {
                match node {
                    Statement::Let { binding, .. } | Statement::Function { binding, .. } => {
                        self.observations.changed.push(*binding);
                        self.observations.queued[binding.index()] = true;
                    }
                    Statement::Evaluate(_) => self.touched.push(analysis::Site {
                        region: id,
                        statement,
                    }),
                    _ => {}
                }
            }
        }
        self.storage(report);
        let replacements = optimize::plan_expressions(
            self.module,
            self.structure,
            &mut self.analysis,
            &self.plan,
            report,
        );
        for (index, replacement) in replacements {
            self.change_expression(ExprId::new(index), replacement);
        }
        self.storage(report);
        self.control(report);
        self.storage(report);
    }

    pub fn finish(
        self,
        report: &mut optimize::Report,
    ) -> (analysis::Snapshot, Plan, Option<Vec<usize>>) {
        report.work = self.analysis.work();
        (
            self.analysis.detach(),
            self.plan,
            Some(self.observations.counts),
        )
    }

    fn storage(&mut self, report: &mut optimize::Report) {
        loop {
            while let Some(binding) = self.observations.changed.pop() {
                self.observations.queued[binding.index()] = false;
                self.cell(binding, report);
            }
            if self.touched.is_empty() {
                break;
            }
            self.touched
                .sort_unstable_by_key(|site| (site.region, site.statement));
            self.touched.dedup();
            while let Some(site) = self.touched.pop() {
                let Some(Statement::Evaluate(value)) = self.plan.statement(self.module, site)
                else {
                    continue;
                };
                if self.effectless(*value) {
                    self.change_statement(site, None);
                    report.discarded_expressions += 1;
                }
            }
        }
    }

    fn cell(&mut self, binding: BindingId, report: &mut optimize::Report) {
        let module = self.module;
        if !self.analysis.unobserved_cell_removable(binding, self.world) {
            return;
        }
        let Some(site) = self.analysis.binding_uses(binding).declaration else {
            return;
        };
        if self.observations.removed_regions[site.region.index()] {
            return;
        }
        let Some(statement) = self.plan.statement(module, site).cloned() else {
            return;
        };
        let count = self.observations.counts[binding.index()];
        if count == 0 {
            match statement {
                Statement::Let { value, .. } => {
                    let replacement = value
                        .filter(|value| !self.effectless(*value))
                        .map(Statement::Evaluate);
                    self.change_statement(site, replacement);
                    self.touched.push(site);
                }
                Statement::Function { .. } => self.change_statement(site, None),
                _ => return,
            }
            report.removed_bindings += 1;
            for index in 0..self.analysis.binding_uses(binding).writes.len() {
                let write = self.analysis.binding_uses(binding).writes[index];
                if self.observations.removed_expressions[write.operation.index()]
                    || self.plan.replacement(write.operation).is_some()
                {
                    continue;
                }
                let Expr::Assign { value, .. } = module.expressions[write.operation.index()] else {
                    unreachable!("assignment owner")
                };
                self.change_expression(write.operation, rewrite::Replacement::Value(value));
                report.removed_stores += 1;
                self.touched.push(write.site);
            }
            return;
        }
        if self.owned_data != optimize::OwnedData::Scalars
            || module.bindings[binding.index()].pinned
            || !self.analysis.binding_uses(binding).writes.is_empty()
        {
            return;
        }
        let Statement::Let {
            value: Some(initializer),
            ..
        } = statement
        else {
            return;
        };
        if let Expr::Array(elements) = &module.expressions[initializer.index()] {
            // A spread element has no length known here.
            if elements
                .iter()
                .any(|element| matches!(module.expressions[element.index()], Expr::Spread(_)))
            {
                return;
            }
            let length = i32::try_from(elements.len()).ok();
            let mut lengths = Vec::new();
            for read in &self.analysis.binding_uses(binding).reads {
                if self.observations.removed_expressions[read.expression.index()] {
                    continue;
                }
                let at = self.placement.resolve(read.site);
                if at.region != site.region || at.statement <= site.statement {
                    return;
                }
                let analysis::Observation::ArrayLength(length) = read.observation else {
                    return;
                };
                lengths.push(length);
            }
            if let Some(length) = length {
                let effects: Vec<_> = elements
                    .iter()
                    .copied()
                    .filter(|value| !self.effectless(*value))
                    .collect();
                let retained = !effects.is_empty();
                self.change_expression(initializer, rewrite::Replacement::Effects(effects));
                for value in lengths {
                    self.change_expression(value, rewrite::Replacement::Constant(length));
                }
                self.change_statement(site, retained.then_some(Statement::Evaluate(initializer)));
                if retained {
                    self.touched.push(site);
                }
                report.removed_bindings += 1;
                report.removed_allocations += 1;
            }
            return;
        }
        if count != 1 {
            return;
        }
        let read = self
            .analysis
            .binding_uses(binding)
            .reads
            .iter()
            .find(|read| !self.observations.removed_expressions[read.expression.index()])
            .copied()
            .expect("one retained read");
        let at = self.placement.resolve(read.site);
        if at.region != site.region
            || site.statement >= at.statement
            || initializer >= read.expression
        {
            return;
        }
        let literal = self.plan.is_literal(module, initializer);
        if !literal {
            if matches!(
                self.plan.statement(module, at),
                Some(Statement::Loop { .. } | Statement::ForIn { .. } | Statement::ForOf { .. })
            ) {
                return;
            }
            let facts = self.analysis.facts(initializer);
            if !facts.effects.stable_scalar()
                || matches!(
                    facts.value,
                    analysis::ValueKind::Unknown | analysis::ValueKind::Object
                )
            {
                return;
            }
        }
        if !self
            .placement
            .transfer(module, self.structure, site, at, literal)
        {
            report.retained_for_depth += 1;
            return;
        }
        self.observations.forget_read(read.expression);
        self.observations.moved[initializer.index()] = true;
        self.plan.transfer(module, read.expression, initializer);
        self.change_statement(site, None);
        report.inlined_bindings += 1;
    }

    fn control(&mut self, report: &mut optimize::Report) {
        let module = self.module;
        let mut empty = vec![false; module.regions.len()];
        for &id in &self.structure.region_postorder {
            if self.observations.removed_regions[id.index()] {
                empty[id.index()] = true;
                continue;
            }
            for position in 0..module.regions[id.index()].statements.len() {
                let site = analysis::Site {
                    region: id,
                    statement: position,
                };
                let Some(statement) = self.plan.statement(module, site).cloned() else {
                    continue;
                };
                let replacement = match statement {
                    Statement::If { condition, yes, no } => {
                        if let Some(value) =
                            optimize::literal_truth(&module.expressions[condition.index()])
                        {
                            let branch =
                                if value { Some(yes) } else { no }.filter(|id| !empty[id.index()]);
                            branch.map(Statement::Block)
                        } else if empty[yes.index()] && no.is_none_or(|no| empty[no.index()]) {
                            (!self.effectless(condition)).then_some(Statement::Evaluate(condition))
                        } else {
                            continue;
                        }
                    }
                    Statement::Block(body) if empty[body.index()] => None,
                    Statement::Loop {
                        condition: Some(condition),
                        ..
                    } if optimize::literal_truth(&module.expressions[condition.index()])
                        == Some(false) =>
                    {
                        None
                    }
                    _ => continue,
                };
                self.change_statement(site, replacement);
                report.simplified_control += 1;
            }
            empty[id.index()] = (0..module.regions[id.index()].statements.len()).all(|statement| {
                self.plan
                    .statement(
                        module,
                        analysis::Site {
                            region: id,
                            statement,
                        },
                    )
                    .is_none()
            });
        }
    }
}
