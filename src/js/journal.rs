//! The edit journal (plan M5.2; architecture §8.1, "Target edits"): what a
//! rule changed, recorded by the typed helpers every edit of the tree goes
//! through.
//!
//! A rule runs between `open_journal` and `take_journal`. Each helper records
//! the slot it edits:
//! * a region whose statement list or scope changed;
//! * a node rewritten in place;
//! * a binding or a function record changed;
//! * the module's tables (imports, exports, root rows, scopes).
//!
//! Slots a rule creates need no entry: they are reachable only through an
//! edit of a slot that already existed. `renumber` carries the node entries
//! to their new ids. The journal's edit count is the scheduler's signal
//! (M5.3a): a rule that recorded nothing changed nothing, so a round in which
//! no rule recorded an edit is the fixed point. Its entries are the dirty
//! slots incremental work re-runs on (M5.7).
//!
//! Test and debug builds check a journal against the actual difference
//! (`check_journal`): every region, binding, function and table that differs
//! from the snapshot taken when the rule started is recorded, and a rule that
//! recorded an edit changed something.
//!
//! Prior art: Oxc's `PassChanges` (`oxc@591966d
//! crates/oxc_minifier/src/state.rs:29-60`) records the references a pass
//! removed and whether it asked for another pass, and a debug walk checks
//! the removals (`compression_pass.rs:68`); Closure reports each change to its
//! enclosing function scope (`Compiler.reportChangeToEnclosingScope`) and
//! verifies the reports in its validity checks (`ChangeVerifier`).

use super::*;

/// What the rules changed since the journal was opened.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Journal {
    /// A rule is running: the helpers record only then.
    open: bool,
    /// Edits recorded, including repeats of one slot.
    edits: u64,
    regions: Vec<RegionId>,
    expressions: Vec<ExprId>,
    bindings: Vec<BindingId>,
    functions: Vec<FunctionId>,
    /// Imports, exports, root rows or scopes changed.
    tables: bool,
}

impl Journal {
    /// The number of edits recorded: zero when the rule changed nothing.
    pub(crate) fn edits(&self) -> u64 {
        self.edits
    }

    fn edit(&mut self) -> bool {
        if self.open {
            self.edits += 1;
        }
        self.open
    }

    fn region(&mut self, region: RegionId) {
        if self.edit() {
            self.regions.push(region);
        }
    }

    fn expression(&mut self, expression: ExprId) {
        if self.edit() {
            self.expressions.push(expression);
        }
    }

    fn binding(&mut self, binding: BindingId) {
        if self.edit() {
            self.bindings.push(binding);
        }
    }

    fn function(&mut self, function: FunctionId) {
        if self.edit() {
            self.functions.push(function);
        }
    }

    fn tables(&mut self) {
        if self.edit() {
            self.tables = true;
        }
    }

    /// Carry the node entries through a renumbering; a node that did not
    /// survive leaves the journal (its parent's edit is recorded).
    pub(super) fn renumber(&mut self, map: &[Option<ExprId>]) {
        self.expressions
            .retain_mut(|id| match map.get(id.index()).copied().flatten() {
                Some(new) => {
                    *id = new;
                    true
                }
                None => false,
            });
    }
}

// The typed mutation helpers. Every edit of a tree slot that existed when the
// rule started goes through one of these (or through the root-row helpers,
// which record their region).
impl Module {
    /// Start recording: a rule begins.
    pub(crate) fn open_journal(&mut self) {
        self.journal = Journal {
            open: true,
            ..Journal::default()
        };
    }

    /// Stop recording: what the rule changed.
    pub(crate) fn take_journal(&mut self) -> Journal {
        let mut journal = std::mem::take(&mut self.journal);
        journal.open = false;
        journal
    }

    /// Replace the node at `id`; returns whether it differed.
    pub(crate) fn set_expression(&mut self, id: ExprId, node: Expr) -> bool {
        if self.expressions[id.index()] == node {
            return false;
        }
        self.expressions[id.index()] = node;
        self.journal.expression(id);
        true
    }

    /// The node at `id`, for an edit in place.
    pub(crate) fn expression_mut(&mut self, id: ExprId) -> &mut Expr {
        self.journal.expression(id);
        &mut self.expressions[id.index()]
    }

    /// The statements of `region`, for an edit.
    pub(crate) fn statements_mut(&mut self, region: usize) -> &mut Vec<Statement> {
        self.journal
            .region(RegionId::try_new(region).expect("a region index"));
        &mut self.regions[region].statements
    }

    /// Replace statement `index` of `region`; returns whether it differed.
    pub(crate) fn set_statement(
        &mut self,
        region: usize,
        index: usize,
        statement: Statement,
    ) -> bool {
        if self.regions[region].statements[index] == statement {
            return false;
        }
        self.statements_mut(region)[index] = statement;
        true
    }

    /// Give `region` the scope `scope`.
    pub(crate) fn set_region_scope(&mut self, region: RegionId, scope: ScopeId) {
        if self.regions[region.index()].scope != scope {
            self.regions[region.index()].scope = scope;
            self.journal.region(region);
        }
    }

    /// The record of `binding`, for an edit.
    pub(crate) fn binding_mut(&mut self, binding: BindingId) -> &mut Binding {
        self.journal.binding(binding);
        &mut self.bindings[binding.index()]
    }

    /// The record of `function`, for an edit.
    pub(crate) fn function_mut(&mut self, function: FunctionId) -> &mut Function {
        self.journal.function(function);
        &mut self.functions[function.index()]
    }

    /// Imports, exports or scopes are about to change.
    pub(crate) fn tables_mut(&mut self) -> &mut Self {
        self.journal.tables();
        self
    }

    /// Record an edit of `region`'s statements made through a helper that
    /// borrows them directly (the root-row helpers).
    pub(super) fn journal_region(&mut self, region: usize) {
        self.journal
            .region(RegionId::try_new(region).expect("a region index"));
        if self.rows_of(region) {
            self.journal.tables();
        }
    }
}

#[cfg(any(test, debug_assertions))]
mod check {
    use super::*;
    use std::hash::{Hash, Hasher};

    fn hasher() -> std::collections::hash_map::DefaultHasher {
        std::collections::hash_map::DefaultHasher::new()
    }

    /// Each node's digest: its own shape and its children's digests, so a
    /// renumbering changes none.
    fn expression_digests(expressions: &[Expr]) -> Vec<u64> {
        let mut digests: Vec<Option<u64>> = vec![None; expressions.len()];
        let mut stack = Vec::new();
        for start in 0..expressions.len() {
            stack.push((start, false));
            while let Some((index, expanded)) = stack.pop() {
                if digests[index].is_some() {
                    continue;
                }
                let node = &expressions[index];
                if !expanded {
                    stack.push((index, true));
                    let _ = node.visit_children(|child| {
                        if child.index() < expressions.len() && digests[child.index()].is_none() {
                            stack.push((child.index(), false));
                        }
                        Ok::<(), ()>(())
                    });
                    continue;
                }
                let mut state = hasher();
                let mut shape = node.clone();
                shape.remap_children(|_| ExprId::new(0));
                format!("{shape:?}").hash(&mut state);
                let _ = node.visit_children(|child| {
                    digests
                        .get(child.index())
                        .copied()
                        .flatten()
                        .unwrap_or(u64::MAX)
                        .hash(&mut state);
                    Ok::<(), ()>(())
                });
                digests[index] = Some(state.finish());
            }
        }
        digests
            .into_iter()
            .map(|digest| digest.unwrap_or(0))
            .collect()
    }

    /// A region's digest: its scope and its statements, each with its nodes'
    /// digests in place of their ids.
    fn region_digest(region: &Region, digests: &[u64]) -> u64 {
        let mut state = hasher();
        region.scope.hash(&mut state);
        for statement in &region.statements {
            let mut shape = statement.clone();
            shape.remap_expressions(|_| ExprId::new(0));
            format!("{shape:?}").hash(&mut state);
            statement.visit_expressions(|root| {
                digests
                    .get(root.index())
                    .copied()
                    .unwrap_or(u64::MAX)
                    .hash(&mut state)
            });
        }
        state.finish()
    }

    impl Module {
        /// Check `journal`, recorded by a rule that started from `before`,
        /// against what actually differs now: every changed region still
        /// reachable, binding, function and table is recorded, and a rule
        /// that recorded an edit changed something.
        pub(crate) fn check_journal(
            &self,
            before: &Module,
            journal: &Journal,
        ) -> Result<(), String> {
            let old = expression_digests(&before.expressions);
            let new = expression_digests(&self.expressions);
            // The regions reachable now, and the ones holding each node the
            // journal names.
            let named: std::collections::HashSet<usize> =
                journal.expressions.iter().map(|id| id.index()).collect();
            let mut claimed = vec![false; self.regions.len()];
            for region in &journal.regions {
                if let Some(slot) = claimed.get_mut(region.index()) {
                    *slot = true;
                }
            }
            let mut reached = vec![false; self.regions.len()];
            let mut regions = vec![self.root];
            let mut nodes = Vec::new();
            while let Some(region) = regions.pop() {
                if std::mem::replace(&mut reached[region.index()], true) {
                    continue;
                }
                for statement in &self.regions[region.index()].statements {
                    statement.visit_regions(|child| regions.push(child));
                    if let Statement::Function { function, .. } = statement {
                        regions.push(self.functions[function.index()].body);
                    }
                    statement.visit_expressions(|root| nodes.push(root));
                    while let Some(id) = nodes.pop() {
                        if named.contains(&id.index()) {
                            claimed[region.index()] = true;
                        }
                        let node = &self.expressions[id.index()];
                        for function in node.created_functions() {
                            regions.push(self.functions[function.index()].body);
                        }
                        let _ = node.visit_children(|child| {
                            nodes.push(child);
                            Ok::<(), ()>(())
                        });
                    }
                }
            }
            let mut missing = Vec::new();
            let mut changed = false;
            for (index, region) in self.regions.iter().enumerate() {
                let Some(previous) = before.regions.get(index) else {
                    continue;
                };
                let differs = region_digest(region, &new) != region_digest(previous, &old);
                changed |= differs;
                if differs && reached[index] && !claimed[index] {
                    missing.push(format!("region {index}"));
                }
            }
            // A node rewritten outside every region (a dead one) changed the
            // arena, though no region shows it.
            changed |= self
                .expressions
                .iter()
                .zip(&before.expressions)
                .any(|(node, previous)| node != previous);
            let bindings: std::collections::HashSet<usize> =
                journal.bindings.iter().map(|id| id.index()).collect();
            for (index, binding) in self.bindings.iter().enumerate() {
                if before
                    .bindings
                    .get(index)
                    .is_some_and(|previous| previous != binding)
                {
                    changed = true;
                    if !bindings.contains(&index) {
                        missing.push(format!("binding {index}"));
                    }
                }
            }
            let functions: std::collections::HashSet<usize> =
                journal.functions.iter().map(|id| id.index()).collect();
            for (index, function) in self.functions.iter().enumerate() {
                if before
                    .functions
                    .get(index)
                    .is_some_and(|previous| previous != function)
                {
                    changed = true;
                    if !functions.contains(&index) {
                        missing.push(format!("function {index}"));
                    }
                }
            }
            let tables = self.imports != before.imports
                || self.exports != before.exports
                || self.root_rows != before.root_rows
                || self.scopes.get(..before.scopes.len()) != Some(&before.scopes[..]);
            changed |= tables;
            if tables && !journal.tables {
                missing.push("the tables".to_string());
            }
            if !missing.is_empty() {
                return Err(format!("unrecorded edits: {}", missing.join(", ")));
            }
            if journal.edits > 0 && !changed {
                return Err(format!("{} edits recorded, nothing changed", journal.edits));
            }
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "journal_tests.rs"]
mod tests;
