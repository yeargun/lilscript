//! Naming constraints are derived once from a verified immutable program.
//! Candidate plans contain choices, not cloned semantic programs or facts.
use super::extract::OutputError;
use super::*;
use crate::compilation_policy::WorkKind;
use crate::compilation_policy::{ResolvedPolicy, RuntimeRisk, TacticId, TacticUse};
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Style {
    Global,
    Scoped,
    Source,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Plan {
    pub style: Style,
    /// Lexical names retained as a compression choice, not a semantic pin.
    pub source_names: Vec<BindingId>,
    /// Spell for raw bytes. Each function that is not an arrow and has an
    /// exact name prints as `function name(){…}`, so the binding holding it
    /// takes a short name, and statements take their shortest exact forms
    /// (`x+=y`, `return c?a:b`, `c&&e`). Always fewer raw bytes, but a
    /// repeated long name or statement shape is nearly free under a codec, so
    /// it is a choice rather than a rule.
    pub raw_spelling: bool,
}

impl Plan {
    pub fn new(style: Style) -> Self {
        Self::spelled(style, false)
    }

    pub fn spelled(style: Style, raw_spelling: bool) -> Self {
        Self {
            style,
            source_names: vec![],
            raw_spelling,
        }
    }

    /// The same permission boundary used by prepared-output rendering. This
    /// validates choices, not binding existence or naming constraints.
    pub fn check_policy(&self, policy: &ResolvedPolicy) -> Result<(), OutputError> {
        Eligibility::from_policy_in(policy)?.check_in(self)
    }

    pub fn seeds_for_policy(policy: &ResolvedPolicy) -> Result<&'static [Style], OutputError> {
        Ok(Eligibility::from_policy_in(policy)?.seeds())
    }

    pub(crate) fn provenance_for_policy(
        &self,
        policy: &ResolvedPolicy,
    ) -> Result<NamingProvenance, OutputError> {
        let eligibility = Eligibility::from_policy_in(policy)?;
        eligibility.check_in(self)?;
        Ok(NamingProvenance {
            mangling: self.style != Style::Source,
            search: self.style == Style::Scoped
                || !self.source_names.is_empty()
                || (self.style == Style::Source && eligibility.permits_search()),
        })
    }
}

/// How a valid plan was chosen remains separate from its spelling. In
/// particular, Source can be the required baseline or an explored alternative.
/// Cached equal bytes must not erase the latter's search permission dependency.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NamingProvenance {
    mangling: bool,
    search: bool,
}

impl NamingProvenance {
    pub(crate) fn check_policy(
        self,
        plan: &Plan,
        policy: &ResolvedPolicy,
    ) -> Result<(), OutputError> {
        let eligibility = Eligibility::from_policy_in(policy)?;
        eligibility.check_in(plan)?;
        if self.search && !eligibility.permits_search() {
            return Err(
                "retained naming search requires effective naming-search permission".into(),
            );
        }
        Ok(())
    }

    pub(crate) fn tactics(self) -> &'static [TacticUse] {
        const MANGLE: TacticUse = TacticUse {
            tactic: TacticId::IdentifierMangling,
            risk: RuntimeRisk::Neutral,
        };
        const SEARCH: TacticUse = TacticUse {
            tactic: TacticId::NamingSearch,
            risk: RuntimeRisk::Neutral,
        };
        match (self.mangling, self.search) {
            (false, false) => &[],
            (true, false) => &[MANGLE],
            (false, true) => &[SEARCH],
            (true, true) => &[MANGLE, SEARCH],
        }
    }
}

/// Permissions belong to a prepared output, independently of reusable naming
/// constraints. Public rendering and exploration both consult this same value.
#[derive(Debug, Clone, Copy)]
pub(super) enum Eligibility {
    SourceOnly,
    GlobalOnly,
    Search,
}

impl Eligibility {
    pub(super) fn from_policy_in(policy: &ResolvedPolicy) -> Result<Self, OutputError> {
        if policy.javascript_contract().is_none() {
            return Err("JavaScript output requires a JavaScript compilation policy".into());
        }
        Ok(if !policy.tactic(TacticId::IdentifierMangling).enabled {
            Self::SourceOnly
        } else if !policy.tactic(TacticId::NamingSearch).enabled {
            // Global is the existing selector's first useful baseline. It also
            // avoids preparing the extra scoped-name interference structure.
            Self::GlobalOnly
        } else {
            Self::Search
        })
    }

    pub(super) fn check_in(self, plan: &Plan) -> Result<(), OutputError> {
        let permitted = match self {
            Self::SourceOnly => plan.style == Style::Source && plan.source_names.is_empty(),
            Self::GlobalOnly => plan.style == Style::Global && plan.source_names.is_empty(),
            Self::Search => true,
        };
        if permitted {
            Ok(())
        } else {
            Err(match self {
                Self::SourceOnly => "identifier mangling is disabled; only the source naming plan is eligible",
                Self::GlobalOnly => "naming search is disabled; only the global baseline without naming overrides is eligible",
                Self::Search => unreachable!(),
            }.into())
        }
    }

    pub(super) fn seeds(self) -> &'static [Style] {
        match self {
            Self::SourceOnly => &[Style::Source],
            Self::GlobalOnly => &[Style::Global],
            Self::Search => &[Style::Global, Style::Scoped, Style::Source],
        }
    }

    pub(super) fn permits_search(self) -> bool {
        matches!(self, Self::Search)
    }
}

pub(super) struct Basis<'a> {
    module: &'a Module,
    choices: Option<extract::JavaScriptChoices<'a>>,
    required: Vec<Option<&'a str>>,
    preferred: Vec<Option<&'a str>>,
    hosts: Vec<&'a str>,
    references: Vec<(ExprId, ScopeId)>,
    /// Functions that can print as named expressions, `function name(){…}`:
    /// their exact name owns itself, so under a self-named plan the binding
    /// holding one takes a short name instead of that spelling. Indexed by
    /// function.
    self_named: Vec<bool>,
    /// `preferred` without the self-named functions' bindings, and `hosts`
    /// with their names reserved: what a self-named plan uses.
    preferred_self: Vec<Option<&'a str>>,
    hosts_self: Vec<&'a str>,
    /// `let`/assignment initializers holding a function with an exact name,
    /// resolved into `preferred` once `self_named` is known.
    preferences: Vec<(BindingId, FunctionId, &'a str)>,
    direct_eval: bool,
    scoped: OnceLock<Scoped>,
    source_candidates: OnceLock<Vec<BindingId>>,
}

fn sort_work(count: usize, longest: usize) -> Result<u64, AllocationError> {
    (count as u64)
        .checked_mul((usize::BITS - count.max(1).leading_zeros()) as u64)
        .and_then(|work| work.checked_mul(longest.max(1) as u64))
        .ok_or(AllocationError::Capacity)
}

impl<'a> Basis<'a> {
    pub(super) fn new_in(
        module: &'a Module,
        structure: &verify::Structure,
        choices: Option<extract::JavaScriptChoices<'a>>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, OutputError> {
        let _timing = crate::timing::TARGET_BASIS.scope(0);
        use AllocationClass::{Retained, Scratch};
        let mut basis = Self {
            module,
            choices,
            required: budget.filled(Retained, module.bindings.len(), None)?,
            preferred: budget.filled(Retained, module.bindings.len(), None)?,
            hosts: budget.vector(Retained, 2)?,
            references: Vec::new(),
            self_named: budget.filled(Retained, module.functions.len(), false)?,
            preferred_self: Vec::new(),
            hosts_self: Vec::new(),
            preferences: Vec::new(),
            direct_eval: false,
            scoped: OnceLock::new(),
            source_candidates: OnceLock::new(),
        };
        basis.hosts.extend_from_slice(&["eval", "arguments"]);
        for name in &module.reserved {
            budget.work(WorkKind::Analysis, name.len() as u64)?;
            budget.push(Retained, &mut basis.hosts, name.as_str())?;
        }
        for (symbol, binding) in module.bindings.iter().enumerate() {
            budget.work(WorkKind::Analysis, 1)?;
            if binding.pinned {
                basis.required[symbol] = Some(&binding.spelling);
            }
        }
        let mut pending = Vec::new();
        for (index, region) in module.regions.iter().enumerate() {
            budget.work(WorkKind::Analysis, 1)?;
            if structure.region_depths[index].is_none() {
                continue;
            }
            for statement in &region.statements {
                budget.work(WorkKind::Analysis, 1)?;
                match statement {
                    Statement::Function { binding, function } => {
                        if let Some(name) = extract::function_name(module, *function, choices) {
                            let name = name
                                .as_unicode()
                                .ok_or("declared function name is not an identifier")?;
                            budget.work(WorkKind::Analysis, name.len() as u64)?;
                            if basis.required[binding.index()]
                                .replace(name)
                                .is_some_and(|old| old != name)
                            {
                                return Err(
                                    "lexical spelling conflicts with declared function name".into(),
                                );
                            }
                        }
                    }
                    Statement::Let {
                        binding,
                        value: Some(value),
                    } => basis.prefer(*binding, *value),
                    _ => {}
                }
                // `for (let k in object)` evaluates `object` while `k` is
                // already in its TDZ, so its references share the key's scope:
                // an outer binding it reads can never be named `k`.
                let scope = match statement {
                    Statement::ForIn { binding, .. } | Statement::ForOf { binding, .. } => {
                        module.bindings[binding.index()].scope
                    }
                    _ => region.scope,
                };
                let mut roots = Ok(());
                statement.visit_expressions(|root| {
                    if roots.is_ok() {
                        roots = budget.push(Scratch, &mut pending, root);
                    }
                });
                roots?;
                while let Some(id) = pending.pop() {
                    budget.work(WorkKind::Analysis, 1)?;
                    let expression = &module.expressions[id.index()];
                    match expression {
                        Expr::ConstructIntrinsic { operation, .. } => {
                            budget.push(
                                Retained,
                                &mut basis.hosts,
                                native_constructor(*operation)
                                    .expect("verified construction")
                                    .name,
                            )?;
                        }
                        Expr::Binding(_) => {
                            budget.push(Retained, &mut basis.references, (id, scope))?
                        }
                        Expr::Host(name) => {
                            budget.push(Retained, &mut basis.hosts, name.as_str())?;
                            budget.push(Retained, &mut basis.references, (id, scope))?;
                        }
                        Expr::Assign { target, value } => {
                            if let Expr::Binding(binding) = module.expressions[target.index()] {
                                basis.prefer(binding, *value);
                            }
                        }
                        Expr::Call {
                            invocation: Invocation::DirectEval,
                            ..
                        } => basis.direct_eval = true,
                        _ => {}
                    }
                    expression.visit_children(|child| budget.push(Scratch, &mut pending, child))?;
                }
            }
        }
        budget.work(WorkKind::Analysis, basis.hosts.len() as u64)?;
        let longest = basis.hosts.iter().map(|name| name.len()).max().unwrap_or(0);
        budget.work(WorkKind::Analysis, sort_work(basis.hosts.len(), longest)?)?;
        basis.hosts.sort_unstable();
        budget.work(
            WorkKind::Analysis,
            (basis.hosts.len() as u64)
                .checked_mul(longest.max(1) as u64)
                .ok_or(AllocationError::Capacity)?,
        )?;
        basis.hosts.dedup();
        basis.settle_function_names(budget)?;
        Ok(basis)
    }

    pub(super) fn source_candidates_in(
        &self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<&[BindingId], OutputError> {
        if let Some(candidates) = self.source_candidates.get() {
            return Ok(candidates);
        }
        let mut phase = budget.scope();
        let mut occurrences =
            phase.filled(AllocationClass::Scratch, self.module.bindings.len(), 0usize)?;
        for &(expression, _) in &self.references {
            phase.work(WorkKind::Analysis, 1)?;
            if let Expr::Binding(binding) = self.module.expressions[expression.index()] {
                occurrences[binding.index()] = occurrences[binding.index()]
                    .checked_add(1)
                    .ok_or(AllocationError::Capacity)?;
            }
        }
        let mut candidates = phase.vector(AllocationClass::Retained, self.module.bindings.len())?;
        for (id, name) in self.required.iter().enumerate() {
            phase.work(WorkKind::Analysis, 1)?;
            if name.is_none() && self.module.bindings[id].source_symbol.is_some() {
                candidates.push(BindingId::new(id));
            }
        }
        phase.work(WorkKind::Analysis, sort_work(candidates.len(), 1)?)?;
        candidates.sort_unstable_by_key(|symbol| {
            (std::cmp::Reverse(occurrences[symbol.index()]), *symbol)
        });
        drop(occurrences);
        self.source_candidates
            .set(candidates)
            .map_err(|_| OutputError::Invalid("naming candidate cache raced"))?;
        phase.finish_retained()?;
        Ok(self.source_candidates.get().unwrap())
    }

    fn prefer(&mut self, binding: BindingId, value: ExprId) {
        if let Expr::Function(function) = self.module.expressions[value.index()] {
            if let Some(name) = extract::function_name(self.module, function, self.choices)
                .and_then(StringValue::as_unicode)
            {
                self.preferences.push((binding, function, name));
            }
        }
    }

    /// A function that is not an arrow can carry its exact name itself as a
    /// named expression, which costs the name once rather than at every
    /// reference to the binding holding it. Its name then binds inside its own
    /// body, so it must not be a spelling any reference there could print:
    /// not a host name, not a required spelling, and never allocated (it joins
    /// the reserved hosts). The remaining functions keep the old preference:
    /// the binding takes the exact name when it is free.
    fn settle_function_names(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), OutputError> {
        let module = self.module;
        let mut required: Vec<&str> = self.required.iter().flatten().copied().collect();
        budget.work(WorkKind::Analysis, required.len() as u64 + 1)?;
        required.sort_unstable();
        let mut named = Vec::new();
        for (index, function) in module.functions.iter().enumerate() {
            budget.work(WorkKind::Analysis, 1)?;
            if function.arrow {
                continue;
            }
            let Some(name) = extract::function_name(module, FunctionId::new(index), self.choices)
                .and_then(StringValue::as_unicode)
            else {
                continue;
            };
            budget.work(WorkKind::Analysis, name.len() as u64)?;
            if name.is_empty()
                || !identifier(name)
                || matches!(name, "eval" | "arguments" | "undefined")
                || self.hosts.binary_search(&name).is_ok()
                || required.binary_search(&name).is_ok()
            {
                continue;
            }
            self.self_named[index] = true;
            named.push(name);
        }
        self.preferred_self = budget.filled(AllocationClass::Retained, module.bindings.len(), None)?;
        for &(binding, function, name) in &self.preferences {
            budget.work(WorkKind::Analysis, 1)?;
            self.preferred[binding.index()].get_or_insert(name);
            if !self.self_named[function.index()] {
                self.preferred_self[binding.index()].get_or_insert(name);
            }
        }
        budget.extend_copy(AllocationClass::Retained, &mut self.hosts_self, &self.hosts)?;
        if !named.is_empty() {
            budget.extend_copy(AllocationClass::Retained, &mut self.hosts_self, &named)?;
            budget.work(WorkKind::Analysis, sort_work(self.hosts_self.len(), 16)?)?;
            self.hosts_self.sort_unstable();
            self.hosts_self.dedup();
        }
        Ok(())
    }

    fn scoped_in(&self, budget: &mut AllocationBudget<'_>) -> Result<&Scoped, OutputError> {
        if let Some(scoped) = self.scoped.get() {
            return Ok(scoped);
        }
        let mut phase = budget.scope();
        let module = self.module;
        let mut free = phase.vector(AllocationClass::Retained, module.scopes.len())?;
        for _ in &module.scopes {
            phase.work(WorkKind::Analysis, 1)?;
            free.push(Vec::new());
        }
        for &(expression, scope) in &self.references {
            phase.work(WorkKind::Analysis, 1)?;
            if let Expr::Binding(binding) = module.expressions[expression.index()] {
                if module.bindings[binding.index()].scope != scope {
                    phase.push(AllocationClass::Retained, &mut free[scope.index()], binding)?;
                }
            }
        }
        for index in (0..module.scopes.len()).rev() {
            phase.work(WorkKind::Analysis, 1 + sort_work(free[index].len(), 1)?)?;
            free[index].sort_unstable();
            phase.work(WorkKind::Analysis, free[index].len() as u64)?;
            free[index].dedup();
            if let Some(parent) = module.scopes[index] {
                let (ancestors, current) = free.split_at_mut(index);
                for &binding in &current[0] {
                    phase.work(WorkKind::Analysis, 1)?;
                    if module.bindings[binding.index()].scope != parent {
                        phase.push(
                            AllocationClass::Retained,
                            &mut ancestors[parent.index()],
                            binding,
                        )?;
                    }
                }
            }
        }
        let mut order = phase.vector(AllocationClass::Retained, module.bindings.len())?;
        for id in 0..module.bindings.len() {
            phase.work(WorkKind::Analysis, 1)?;
            order.push(BindingId::new(id));
        }
        phase.work(WorkKind::Analysis, sort_work(order.len(), 1)?)?;
        order.sort_unstable_by_key(|binding| (module.bindings[binding.index()].scope, *binding));
        // For raw bytes, the root's most read bindings take its shortest
        // names, as a frequency renamer gives them (esbuild's top-level
        // slots). Nested scopes keep declaration order either way, so the
        // same position in every function spells the same name. A codec
        // prefers the declaration order: measured +647 Brotli on zodlil.
        let mut reads = phase.filled(AllocationClass::Scratch, module.bindings.len(), 0u32)?;
        for &(expression, _) in &self.references {
            phase.work(WorkKind::Analysis, 1)?;
            if let Expr::Binding(binding) = module.expressions[expression.index()] {
                reads[binding.index()] = reads[binding.index()].saturating_add(1);
            }
        }
        let root = module.regions[module.root.index()].scope;
        let mut by_reads = phase.vector(AllocationClass::Retained, order.len())?;
        phase.extend_copy(AllocationClass::Retained, &mut by_reads, &order)?;
        phase.work(WorkKind::Analysis, sort_work(by_reads.len(), 1)?)?;
        by_reads.sort_by_key(|binding| {
            let scope = module.bindings[binding.index()].scope;
            (scope, if scope == root { u32::MAX - reads[binding.index()] } else { 0 })
        });
        self.scoped
            .set(Scoped {
                order,
                by_reads,
                free,
            })
            .map_err(|_| OutputError::Invalid("scoped naming cache raced"))?;
        phase.finish_retained()?;
        Ok(self.scoped.get().unwrap())
    }

    #[cfg(test)]
    pub fn names(&self, plan: &Plan) -> Result<Names, String> {
        self.names_in(plan, &mut AllocationBudget::new(None))
            .map_err(|error| error.to_string())
    }

    pub(super) fn names_in(
        &self,
        plan: &Plan,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Names, OutputError> {
        let _timing = crate::timing::TARGET_NAMES.scope(0);
        use AllocationClass::Scratch;
        let module = self.module;
        let scoped = if plan.style != Style::Global {
            Some(self.scoped_in(budget)?)
        } else {
            None
        };
        let mut required = budget.vector(Scratch, self.required.len())?;
        budget.extend_copy(Scratch, &mut required, &self.required)?;
        if self.direct_eval {
            for (id, binding) in module.bindings.iter().enumerate() {
                budget.work(WorkKind::Render, 1 + binding.spelling.len() as u64)?;
                if required[id]
                    .replace(&binding.spelling)
                    .is_some_and(|old| old != binding.spelling)
                {
                    return Err("source spelling conflicts with required function name".into());
                }
            }
        }
        for id in &plan.source_names {
            budget.work(WorkKind::Render, 1)?;
            let binding = module
                .bindings
                .get(id.index())
                .ok_or("unknown naming choice binding")?;
            budget.work(WorkKind::Render, binding.spelling.len() as u64)?;
            if required[id.index()]
                .replace(&binding.spelling)
                .is_some_and(|old| old != binding.spelling)
            {
                return Err("naming choice conflicts with required spelling".into());
            }
        }
        let (hosts, preferred_names) = if plan.raw_spelling {
            (&self.hosts_self, &self.preferred_self)
        } else {
            (&self.hosts, &self.preferred)
        };
        let count = hosts
            .len()
            .checked_add(required.len())
            .ok_or(AllocationError::Capacity)?;
        let mut reserved = budget.vector(Scratch, count)?;
        budget.extend_copy(Scratch, &mut reserved, hosts)?;
        for name in required.iter().flatten() {
            budget.work(WorkKind::Render, 1)?;
            reserved.push(*name);
        }
        budget.work(WorkKind::Render, reserved.len() as u64)?;
        let longest = reserved.iter().map(|name| name.len()).max().unwrap_or(0);
        budget.work(WorkKind::Render, sort_work(reserved.len(), longest)?)?;
        reserved.sort_unstable();
        budget.work(
            WorkKind::Render,
            (reserved.len() as u64)
                .checked_mul(longest.max(1) as u64)
                .ok_or(AllocationError::Capacity)?,
        )?;
        reserved.dedup();
        let mut bindings = budget.vector(Scratch, module.bindings.len())?;
        for _ in &module.bindings {
            budget.work(WorkKind::Render, 1)?;
            bindings.push(String::new());
        }
        let mut self_named = budget.vector(Scratch, self.self_named.len())?;
        if plan.raw_spelling {
            budget.extend_copy(Scratch, &mut self_named, &self.self_named)?;
        }
        let mut names = Names {
            bindings,
            by_scope: NameIndex::new(module.bindings.len(), budget)?,
            self_named,
            raw: plan.raw_spelling,
        };
        let mut global = if scoped.is_none() {
            Some(NameIndex::new(module.bindings.len(), budget)?)
        } else {
            None
        };
        let mut next = 0usize;
        let mut scope = None;
        let mut allocate = |symbol: BindingId| -> Result<(), OutputError> {
            budget.work(WorkKind::Render, 1)?;
            let binding = &module.bindings[symbol.index()];
            if scoped.is_some() && scope != Some(binding.scope) {
                next = 0;
                scope = Some(binding.scope);
            }
            let preferred = if plan.style == Style::Source && binding.source_symbol.is_some() {
                Some(binding.spelling.as_str())
            } else {
                preferred_names[symbol.index()]
            };
            let name = if let Some(name) = required[symbol.index()] {
                // Required names may intentionally repeat in separate scopes.
                name
            } else if let Some(name) = preferred {
                budget.work(WorkKind::Render, name.len() as u64)?;
                if identifier(name)
                    && name_available(
                        module,
                        &names,
                        global.as_ref(),
                        scoped,
                        &reserved,
                        binding.scope,
                        name,
                        budget,
                    )?
                {
                    name
                } else {
                    ""
                }
            } else {
                ""
            };
            let name = if !name.is_empty() {
                budget.work(WorkKind::Render, name.len() as u64)?;
                budget.string(Scratch, name)?
            } else {
                loop {
                    // Fixed stack storage covers every digit of a usize even
                    // in base two; this encoder uses the existing base54 order.
                    let mut bytes = [0u8; usize::BITS as usize];
                    let mut index = next;
                    next = next.checked_add(1).ok_or(AllocationError::Capacity)?;
                    const ALPHABET: &[u8] =
                        b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ$_";
                    let mut length = 0;
                    loop {
                        bytes[length] = ALPHABET[index % ALPHABET.len()];
                        length += 1;
                        index /= ALPHABET.len();
                        if index == 0 {
                            break;
                        }
                        index -= 1;
                    }
                    let candidate =
                        std::str::from_utf8(&bytes[..length]).expect("ASCII name alphabet");
                    budget.work(WorkKind::Render, length as u64)?;
                    if identifier(candidate)
                        && name_available(
                            module,
                            &names,
                            global.as_ref(),
                            scoped,
                            &reserved,
                            binding.scope,
                            candidate,
                            budget,
                        )?
                    {
                        break budget.string(Scratch, candidate)?;
                    }
                }
            };
            budget.work(WorkKind::Render, name.len() as u64)?;
            if !identifier(&name) {
                return Err("invalid chosen identifier".into());
            }
            let slot = names
                .by_scope
                .find(module, &names.bindings, Some(binding.scope), &name, budget)?
                .err()
                .ok_or(OutputError::InvalidAt {
                    reason: "binding spelling collision at",
                    index: symbol.index(),
                })?;
            names.bindings[symbol.index()] = name;
            names.by_scope.slots[slot] = Some(symbol);
            if let Some(global) = &mut global {
                if let Err(slot) =
                    global.find(module, &names.bindings, None, names.get(symbol), budget)?
                {
                    global.slots[slot] = Some(symbol);
                }
            }
            Ok(())
        };
        if let Some(scoped) = scoped {
            let order = if plan.raw_spelling {
                &scoped.by_reads
            } else {
                &scoped.order
            };
            for &symbol in order {
                allocate(symbol)?;
            }
        } else {
            for id in 0..module.bindings.len() {
                allocate(BindingId::new(id))?;
            }
        }
        for &(expression, scope) in &self.references {
            budget.work(WorkKind::Render, 1)?;
            match &module.expressions[expression.index()] {
                Expr::Binding(binding)
                    if names.resolve_in(module, scope, names.get(*binding), budget)?
                        != Some(*binding) =>
                {
                    return Err("printed name would capture a different binding".into());
                }
                Expr::Host(name) if names.resolve_in(module, scope, name, budget)?.is_some() => {
                    return Err("printed local would capture an external identifier".into());
                }
                _ => {}
            }
        }
        Ok(names)
    }
}

fn name_available(
    module: &Module,
    names: &Names,
    global: Option<&NameIndex>,
    scoped: Option<&Scoped>,
    reserved: &[&str],
    scope: ScopeId,
    name: &str,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, OutputError> {
    let mut low = 0;
    let mut high = reserved.len();
    while low < high {
        let middle = low + (high - low) / 2;
        budget.work(
            WorkKind::Render,
            1 + name.len().min(reserved[middle].len()) as u64,
        )?;
        match reserved[middle].cmp(name) {
            std::cmp::Ordering::Less => low = middle + 1,
            std::cmp::Ordering::Greater => high = middle,
            std::cmp::Ordering::Equal => return Ok(false),
        }
    }
    if let Some(global) = global {
        if global
            .find(module, &names.bindings, None, name, budget)?
            .is_ok()
        {
            return Ok(false);
        }
    }
    if let Some(scoped) = scoped {
        if names
            .by_scope
            .find(module, &names.bindings, Some(scope), name, budget)?
            .is_ok()
        {
            return Ok(false);
        }
        for outer in &scoped.free[scope.index()] {
            let other = names.get(*outer);
            budget.work(WorkKind::Render, 1 + name.len().min(other.len()) as u64)?;
            if other == name {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

struct Scoped {
    order: Vec<BindingId>,
    /// `order` with the root's bindings by descending reads, for raw plans.
    by_reads: Vec<BindingId>,
    free: Vec<Vec<BindingId>>,
}

/// One fixed-capacity index, at most half full. It borrows names by binding ID,
/// so neither scope lookup nor global collision checks duplicate their bytes.
/// Probe order is lookup-only and cannot choose naming order.
struct NameIndex {
    slots: Vec<Option<BindingId>>,
}
impl NameIndex {
    fn new(count: usize, budget: &mut AllocationBudget<'_>) -> Result<Self, OutputError> {
        let capacity = if count == 0 {
            0
        } else {
            count
                .checked_mul(2)
                .and_then(usize::checked_next_power_of_two)
                .ok_or(AllocationError::Capacity)?
        };
        Ok(Self {
            slots: budget.filled(AllocationClass::Scratch, capacity, None)?,
        })
    }
    fn find(
        &self,
        module: &Module,
        names: &[String],
        scope: Option<ScopeId>,
        name: &str,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Result<BindingId, usize>, OutputError> {
        use std::hash::{Hash, Hasher};
        if self.slots.is_empty() {
            return Ok(Err(0));
        }
        budget.work(WorkKind::Render, 1 + name.len() as u64)?;
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        scope.hash(&mut hash);
        name.hash(&mut hash);
        let mut slot = hash.finish() as usize & (self.slots.len() - 1);
        for _ in 0..self.slots.len() {
            budget.work(WorkKind::Render, 1)?;
            let Some(symbol) = self.slots[slot] else {
                return Ok(Err(slot));
            };
            if scope.is_none_or(|scope| module.bindings[symbol.index()].scope == scope) {
                let previous = &names[symbol.index()];
                budget.work(WorkKind::Render, 1 + name.len().min(previous.len()) as u64)?;
                if previous == name {
                    return Ok(Ok(symbol));
                }
            }
            slot = (slot + 1) & (self.slots.len() - 1);
        }
        Err("naming index exceeded its admitted half-load capacity".into())
    }
}

pub(super) struct Names {
    bindings: Vec<String>,
    by_scope: NameIndex,
    self_named: Vec<bool>,
    raw: bool,
}
impl Names {
    pub fn new(module: &Module, policy: PrintPolicy) -> Result<Self, String> {
        let mut budget = AllocationBudget::new(None);
        let structure =
            verify::verify_in(module, &mut budget).map_err(|error| error.to_string())?;
        Basis::new_in(module, &structure, None, &mut budget)
            .map_err(|error| error.to_string())?
            .names_in(
                &Plan::new(if policy.mangle_bindings {
                    Style::Global
                } else {
                    Style::Source
                }),
                &mut budget,
            )
            .map_err(|error| error.to_string())
    }
    pub fn get(&self, id: BindingId) -> &str {
        &self.bindings[id.index()]
    }
    /// Whether the plan spells for raw bytes.
    pub fn raw(&self) -> bool {
        self.raw
    }
    /// Whether this function prints as `function name(){…}`.
    pub fn self_named(&self, function: FunctionId) -> bool {
        self.self_named.get(function.index()).copied().unwrap_or(false)
    }
    fn resolve_in(
        &self,
        module: &Module,
        mut scope: ScopeId,
        name: &str,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Option<BindingId>, OutputError> {
        loop {
            if let Ok(symbol) =
                self.by_scope
                    .find(module, &self.bindings, Some(scope), name, budget)?
            {
                return Ok(Some(symbol));
            }
            let Some(parent) = module.scopes[scope.index()] else {
                return Ok(None);
            };
            scope = parent;
        }
    }
}

#[cfg(test)]
#[path = "naming_allocation_tests.rs"]
mod allocation_tests;
