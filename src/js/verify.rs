use super::extract::OutputError;
use super::*;
use crate::compilation_policy::WorkKind;
use crate::output_budget::{AllocationBudget, AllocationClass};

pub(crate) const MAX_NESTING: usize = 512;

/// Shape information already established by verification. Planning can use it
/// without another graph walk or an inferred printed-code nesting limit.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Structure {
    pub expression_heights: Vec<usize>,
    pub region_depths: Vec<Option<usize>>,
    pub region_postorder: Vec<RegionId>,
    pub live_expressions: Vec<bool>,
    pub live_functions: Vec<bool>,
    pub live_bindings: Vec<bool>,
}

/// Validate syntax obligations on the reachable typed target before names or
/// bytes are produced. Formation supplies older equivalent recipes; this is
/// the common backstop for new target producers, not a textual downlevel pass.
pub(super) fn verify_edition(
    module: &Module,
    structure: &Structure,
    edition: crate::js_syntax_target::EcmaScriptEdition,
) -> Result<(), String> {
    verify_edition_in(module, structure, edition, &mut AllocationBudget::new(None))
        .map_err(|error| error.to_string())
}

pub(super) fn verify_edition_in(
    module: &Module,
    structure: &Structure,
    edition: crate::js_syntax_target::EcmaScriptEdition,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), OutputError> {
    let _timing = crate::timing::TARGET_EDITION.scope(0);
    use crate::js_syntax_target::JsSyntaxFeature;
    budget.work(WorkKind::Analysis, module.expressions.len() as u64)?;
    if !edition.allows(JsSyntaxFeature::NullishCoalescing)
        && module
            .expressions
            .iter()
            .zip(&structure.live_expressions)
            .any(|(node, live)| {
                *live
                    && matches!(
                        node,
                        Expr::Binary {
                            op: Binary::Nullish,
                            ..
                        }
                    )
            })
    {
        return Err(OutputError::Syntax {
            edition,
            feature: JsSyntaxFeature::NullishCoalescing,
        });
    }
    if !edition.allows(JsSyntaxFeature::OptionalCatchBinding) {
        for (region, depth) in module.regions.iter().zip(&structure.region_depths) {
            budget.work(WorkKind::Analysis, 1 + region.statements.len() as u64)?;
            if depth.is_some()
                && region.statements.iter().any(|statement| {
                    matches!(
                        statement,
                        Statement::Try {
                            catch: Some(Catch { binding: None, .. }),
                            ..
                        }
                    )
                })
            {
                return Err(OutputError::Syntax {
                    edition,
                    feature: JsSyntaxFeature::OptionalCatchBinding,
                });
            }
        }
    }
    Ok(())
}

pub(super) fn verify(module: &Module) -> Result<Structure, String> {
    verify_in(module, &mut AllocationBudget::new(None)).map_err(|error| error.to_string())
}

pub(super) fn verify_in(
    module: &Module,
    budget: &mut AllocationBudget<'_>,
) -> Result<Structure, OutputError> {
    let _timing = crate::timing::TARGET_VERIFY.scope(0);
    use AllocationClass::Scratch;
    budget.work(WorkKind::Analysis, 2)?;
    if module.origins.len() != module.expressions.len() {
        return Err("missing expression provenance slots".into());
    }
    if module.scopes.first() != Some(&None) {
        return Err("missing root scope".into());
    }
    for (index, parent) in module.scopes.iter().enumerate().skip(1) {
        budget.work(WorkKind::Analysis, 1)?;
        if !parent.is_some_and(|parent| (parent.index()) < index) {
            return Err("scope parent must precede its child".into());
        }
    }
    for (id, binding) in module.bindings.iter().enumerate() {
        budget.work(WorkKind::Analysis, 1 + binding.spelling.len() as u64)?;
        if binding.scope.index() >= module.scopes.len() || !identifier(&binding.spelling) {
            return Err(OutputError::InvalidAt {
                reason: "invalid binding",
                index: id,
            });
        }
    }
    let mut imported_bindings = if module.imports.is_empty() {
        Vec::new()
    } else {
        budget.filled(Scratch, module.bindings.len(), false)?
    };
    for import in &module.imports {
        let bytes = import
            .source
            .storage_bytes()
            .checked_add(import.imported.len())
            .and_then(|bytes| bytes.checked_add(1))
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(AllocationError::Capacity)?;
        budget.work(WorkKind::Analysis, bytes)?;
        if !identifier_name(&import.imported) {
            return Err("invalid imported export name".into());
        }
        let declared = imported_bindings
            .get_mut(import.binding.index())
            .ok_or("unknown import binding")?;
        if std::mem::replace(declared, true) {
            return Err("duplicate import binding declaration".into());
        }
    }
    let mut expression_heights = budget.vector::<usize>(Scratch, module.expressions.len())?;
    let mut spread_parents = budget.filled(Scratch, module.expressions.len(), false)?;
    for (index, expression) in module.expressions.iter().enumerate() {
        budget.work(WorkKind::Analysis, 1)?;
        // Postorder construction rules out dangling operands and cycles without
        // a graph walk, including in currently unreachable expression storage.
        let mut height = 1;
        expression.visit_children(|child| {
            budget.work(WorkKind::Analysis, 1)?;
            if child.index() >= index {
                Err(OutputError::InvalidAt {
                    reason: "invalid expression dependency at",
                    index,
                })
            } else {
                height = height.max(expression_heights[child.index()].saturating_add(1));
                Ok(())
            }
        })?;
        expression_heights.push(height);
        if let Expr::Array(elements) = expression {
            for element in elements {
                if matches!(module.expressions[element.index()], Expr::Spread(_)) {
                    spread_parents[element.index()] = true;
                }
            }
        }
        match expression {
            Expr::ConstructIntrinsic {
                operation,
                arguments,
            } if !native_constructor(*operation)
                .is_some_and(|constructor| arguments.len() == constructor.arity) =>
            {
                return Err("unsupported primitive construction or invalid operand count".into());
            }
            Expr::Intrinsic {
                operation,
                arguments,
                ..
            } if !intrinsic_arity(*operation)
                .is_some_and(|arity| arity.contains(&arguments.len())) =>
            {
                return Err("unsupported intrinsic or invalid operand count".into());
            }
            Expr::Host(name) => {
                budget.work(WorkKind::Analysis, name.len() as u64)?;
                if !identifier(name) {
                    return Err("invalid external identifier".into());
                }
            }
            Expr::Literal(Literal::Number(value)) if !value.is_finite() => {
                return Err("non-finite literal needs an explicit operation".into());
            }
            Expr::Sequence(values) if values.len() < 2 => {
                return Err("sequence needs two operands".into());
            }
            // Every child precedes its parent, so a spread's only legal
            // parent (an array literal) has not been visited yet; the
            // placement check runs after this pass.
            Expr::Unary {
                op: Unary::Delete,
                value,
            } if !matches!(module.expressions[value.index()], Expr::Member { .. }) => {
                return Err("delete requires a member operand".into());
            }
            Expr::Assign { target, .. }
                if !matches!(
                    module.expressions[target.index()],
                    Expr::Binding(_) | Expr::Host(_) | Expr::Member { .. }
                ) =>
            {
                return Err("assignment requires a reference".into());
            }
            Expr::Assign { target, .. }
                if matches!(module.expressions[target.index()], Expr::Binding(binding)
                    if imported_bindings.get(binding.index()) == Some(&true)) =>
            {
                return Err("assignment to readonly import binding".into());
            }
            Expr::Call {
                callee, invocation, ..
            } => {
                let callee = &module.expressions[callee.index()];
                match invocation {
                    Invocation::Reference
                        if !matches!(
                            callee,
                            Expr::Binding(_) | Expr::Host(_) | Expr::Member { .. }
                        ) =>
                    {
                        return Err("reference call requires a reference".into());
                    }
                    Invocation::Reference if matches!(callee, Expr::Host(name) if name == "eval") =>
                    {
                        return Err("eval call needs an explicit direct/value convention".into());
                    }
                    Invocation::Reference if matches!(callee, Expr::Binding(symbol) if module.bindings.get(symbol.index()).is_some_and(|binding| binding.spelling == "eval")) =>
                    {
                        return Err("eval binding needs an explicit call convention".into());
                    }
                    Invocation::DirectEval if !matches!(callee, Expr::Host(name) if name == "eval") =>
                    {
                        return Err("direct eval requires the eval reference".into());
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        let mut valid_property = |property: &Property| -> Result<(), OutputError> {
            budget.work(WorkKind::Analysis, 1)?;
            if let Property::Named(name) = property {
                budget.work(WorkKind::Analysis, name.len() as u64)?;
            }
            if matches!(property, Property::Named(name) if !identifier_name(name)) {
                Err("invalid named property; use a computed string key".into())
            } else {
                Ok(())
            }
        };
        match expression {
            Expr::Member { property, .. } => valid_property(property)?,
            Expr::Object(entries) => {
                for (key, _) in entries {
                    valid_property(key)?;
                }
            }
            _ => {}
        }
    }
    budget.work(WorkKind::Analysis, module.expressions.len() as u64)?;
    if module
        .expressions
        .iter()
        .enumerate()
        .any(|(index, expression)| matches!(expression, Expr::Spread(_)) && !spread_parents[index])
    {
        return Err("spread outside an array literal".into());
    }
    let mut walk = Walk {
        module,
        region_depths: budget.filled(Scratch, module.regions.len(), None)?,
        region_postorder: budget.vector(Scratch, module.regions.len())?,
        declarations: budget.filled(Scratch, module.bindings.len(), false)?,
        references: budget.filled(Scratch, module.bindings.len(), false)?,
        expressions: budget.filled(Scratch, module.expressions.len(), false)?,
        functions: budget.filled(Scratch, module.functions.len(), false)?,
        scopes: budget.filled(Scratch, module.scopes.len(), false)?,
        budget,
    };
    if !module.imports.is_empty() {
        let root_scope = module
            .regions
            .get(module.root.index())
            .ok_or("unknown import root region")?
            .scope;
        for import in &module.imports {
            // This is the same declaration check as let/parameter/function;
            // it rejects a nested owner or a duplicate ordinary declaration.
            walk.binding(import.binding, root_scope, true)?;
        }
    }
    walk.region(module.root, None, false, 0, 0)?;
    let root_scope = module.regions[module.root.index()].scope;
    let mut export_names = walk
        .budget
        .vector(AllocationClass::Scratch, module.exports.len())?;
    let mut longest_export = 0usize;
    for export in &module.exports {
        walk.budget
            .work(WorkKind::Analysis, 1 + export.name.len() as u64)?;
        if !identifier_name(&export.name) {
            return Err("invalid or duplicate export name".into());
        }
        longest_export = longest_export.max(export.name.len());
        export_names.push(export.name.as_str());
        walk.binding(export.binding, root_scope, false)?;
    }
    let sorting = (export_names.len() as u64)
        .checked_mul((usize::BITS - export_names.len().max(1).leading_zeros()) as u64)
        .and_then(|work| work.checked_mul(longest_export.max(1) as u64))
        .ok_or(crate::output_budget::AllocationError::Capacity)?;
    walk.budget.work(WorkKind::Analysis, sorting)?;
    export_names.sort_unstable();
    walk.budget.work(
        WorkKind::Analysis,
        (module.exports.len() as u64)
            .checked_mul(longest_export.max(1) as u64)
            .ok_or(crate::output_budget::AllocationError::Capacity)?,
    )?;
    if export_names.windows(2).any(|names| names[0] == names[1]) {
        return Err("invalid or duplicate export name".into());
    }
    walk.budget
        .work(WorkKind::Analysis, module.bindings.len() as u64)?;
    if let Some(symbol) = walk
        .references
        .iter()
        .zip(&walk.declarations)
        .position(|(referenced, declared)| *referenced && !declared)
    {
        return Err(OutputError::InvalidAt {
            reason: "binding has no declaration:",
            index: symbol,
        });
    }
    Ok(Structure {
        expression_heights,
        region_depths: walk.region_depths,
        region_postorder: walk.region_postorder,
        live_expressions: walk.expressions,
        live_functions: walk.functions,
        live_bindings: walk.declarations,
    })
}

struct Walk<'a, 'budget, 'ledger> {
    module: &'a Module,
    region_depths: Vec<Option<usize>>,
    region_postorder: Vec<RegionId>,
    declarations: Vec<bool>,
    references: Vec<bool>,
    expressions: Vec<bool>,
    functions: Vec<bool>,
    scopes: Vec<bool>,
    budget: &'budget mut AllocationBudget<'ledger>,
}

impl Walk<'_, '_, '_> {
    fn binding(
        &mut self,
        symbol: BindingId,
        scope: ScopeId,
        declaration: bool,
    ) -> Result<(), OutputError> {
        self.budget.work(WorkKind::Analysis, 1)?;
        let binding = self
            .module
            .bindings
            .get(symbol.index())
            .ok_or("unknown binding")?;
        if declaration {
            if binding.scope != scope || self.declarations[symbol.index()] {
                return Err("invalid or duplicate binding declaration".into());
            }
            self.declarations[symbol.index()] = true;
        } else {
            let mut current = Some(scope);
            while let Some(at) = current {
                self.budget.work(WorkKind::Analysis, 1)?;
                if at == binding.scope {
                    self.references[symbol.index()] = true;
                    return Ok(());
                }
                current = self.module.scopes[at.index()];
            }
            return Err("binding is outside its lexical scope".into());
        }
        Ok(())
    }

    fn expression(&mut self, id: ExprId, scope: ScopeId, depth: usize) -> Result<(), OutputError> {
        self.budget.work(WorkKind::Analysis, 1)?;
        if depth > MAX_NESTING {
            return Err("target nesting limit exceeded".into());
        }
        let module = self.module;
        let expression = module
            .expressions
            .get(id.index())
            .ok_or("unknown expression")?;
        if !matches!(expression, Expr::Literal(_)) {
            if self.expressions[id.index()] {
                return Err(
                    "expression occurrence is shared; use an explicit binding or rematerialization"
                        .into(),
                );
            }
        }
        self.expressions[id.index()] = true;
        if let Expr::Binding(symbol) = expression {
            self.binding(*symbol, scope, false)?;
        }
        if let Some(function) = expression.created_function() {
            self.function(function, scope, depth + 1)?;
        }
        expression.visit_children(|child| self.expression(child, scope, depth + 1))?;
        Ok(())
    }

    fn function(
        &mut self,
        id: FunctionId,
        parent: ScopeId,
        depth: usize,
    ) -> Result<(), OutputError> {
        self.budget.work(WorkKind::Analysis, 1)?;
        let function = self
            .module
            .functions
            .get(id.index())
            .ok_or("unknown function")?;
        self.functions[id.index()] = true;
        if let Some(length) = function.length {
            // Default syntax makes the parameter list non-simple, which
            // forbids a `"use strict"` directive in the body.
            if length >= function.parameters.len() || function.strict {
                return Err("invalid reflected function length".into());
            }
        }
        let region = self
            .module
            .regions
            .get(function.body.index())
            .ok_or("unknown function body")?;
        for parameter in &function.parameters {
            self.binding(*parameter, region.scope, true)?;
        }
        self.region(function.body, Some(parent), true, 0, depth + 1)
    }

    fn region(
        &mut self,
        id: RegionId,
        parent: Option<ScopeId>,
        in_function: bool,
        loops: usize,
        depth: usize,
    ) -> Result<(), OutputError> {
        self.budget.work(WorkKind::Analysis, 1)?;
        let visited = self
            .region_depths
            .get_mut(id.index())
            .ok_or("unknown region")?;
        if depth > MAX_NESTING || visited.is_some() {
            return Err("cyclic, shared or excessively nested region".into());
        }
        *visited = Some(depth);
        let region = self
            .module
            .regions
            .get(id.index())
            .ok_or("unknown region")?;
        if self.module.scopes.get(region.scope.index()) != Some(&parent) {
            return Err("region has an invalid lexical parent".into());
        }
        if self.scopes[region.scope.index()] {
            return Err("lexical scope is shared by different regions".into());
        }
        self.scopes[region.scope.index()] = true;
        for statement in &region.statements {
            self.budget.work(WorkKind::Analysis, 1)?;
            match statement {
                Statement::Let { binding, value } => {
                    self.binding(*binding, region.scope, true)?;
                    if let Some(value) = value {
                        self.expression(*value, region.scope, depth + 1)?;
                    }
                }
                Statement::Evaluate(value) | Statement::Throw(value) => {
                    self.expression(*value, region.scope, depth + 1)?
                }
                Statement::Return(value) => {
                    if !in_function {
                        return Err("return outside a function".into());
                    }
                    if let Some(value) = value {
                        self.expression(*value, region.scope, depth + 1)?;
                    }
                }
                Statement::If { condition, yes, no } => {
                    self.expression(*condition, region.scope, depth + 1)?;
                    self.region(*yes, Some(region.scope), in_function, loops, depth + 1)?;
                    if let Some(no) = no {
                        self.region(*no, Some(region.scope), in_function, loops, depth + 1)?;
                    }
                }
                Statement::Loop {
                    condition,
                    update,
                    body,
                } => {
                    if let Some(condition) = condition {
                        self.expression(*condition, region.scope, depth + 1)?;
                    }
                    if let Some(update) = update {
                        self.expression(*update, region.scope, depth + 1)?;
                    }
                    self.region(*body, Some(region.scope), in_function, loops + 1, depth + 1)?;
                }
                Statement::Block(body) => {
                    self.region(*body, Some(region.scope), in_function, loops, depth + 1)?
                }
                Statement::ForIn {
                    binding,
                    object,
                    body,
                }
                | Statement::ForOf {
                    binding,
                    iterable: object,
                    body,
                } => {
                    self.expression(*object, region.scope, depth + 1)?;
                    let scope = self
                        .module
                        .regions
                        .get(body.index())
                        .ok_or("unknown loop binding region")?
                        .scope;
                    self.binding(*binding, scope, true)?;
                    self.region(*body, Some(region.scope), in_function, loops + 1, depth + 1)?;
                }
                Statement::Try {
                    body,
                    catch,
                    finally,
                } => {
                    if catch.is_none() && finally.is_none() {
                        return Err("try requires a catch or finally region".into());
                    }
                    self.region(*body, Some(region.scope), in_function, loops, depth + 1)?;
                    if let Some(catch) = catch {
                        let scope = self
                            .module
                            .regions
                            .get(catch.body.index())
                            .ok_or("unknown catch region")?
                            .scope;
                        if let Some(binding) = catch.binding {
                            self.binding(binding, scope, true)?;
                        }
                        self.region(
                            catch.body,
                            Some(region.scope),
                            in_function,
                            loops,
                            depth + 1,
                        )?;
                    }
                    if let Some(finally) = finally {
                        self.region(*finally, Some(region.scope), in_function, loops, depth + 1)?;
                    }
                }
                Statement::Break | Statement::Continue if loops == 0 => {
                    return Err("loop transfer outside a loop".into());
                }
                Statement::Break | Statement::Continue => {}
                Statement::Function { binding, function } => {
                    if self
                        .module
                        .functions
                        .get(function.index())
                        .is_some_and(|function| function.arrow)
                    {
                        return Err("a declaration cannot use an arrow function body".into());
                    }
                    self.binding(*binding, region.scope, true)?;
                    if let Some(name) = self
                        .module
                        .functions
                        .get(function.index())
                        .and_then(|function| function.name.exact())
                    {
                        self.budget
                            .work(WorkKind::Analysis, name.storage_bytes() as u64)?;
                        if !name.as_unicode().is_some_and(identifier) {
                            return Err("declared function name must be an identifier".into());
                        }
                    }
                    self.function(*function, region.scope, depth + 1)?;
                }
            }
        }
        self.region_postorder.push(id);
        Ok(())
    }
}
