//! Delivered host modules lowered into the target tree.
//!
//! A host module is JavaScript the output would otherwise carry as text,
//! evaluated whole beside the program: every function kept, whether the
//! program calls it or not, its names long, its one-line wrappers reached
//! through a call. Lowered, it is ordinary target code, so pruning, inlining,
//! forwarding and naming reach it like any other. The lowering is exact for
//! the subset it accepts and refuses the rest, which leaves every module to
//! text delivery:
//!
//! * `let`/`const` and function declarations, declared per block before any
//!   statement runs, so references resolve as hoisting does. A block's
//!   function declarations become its first statements, as `let f=function…`:
//!   strict code creates them on entry to the block, and creating a function
//!   runs nothing. `var` and class declarations are refused. Named root-class
//!   expressions with plain constructors/methods are represented directly;
//!   class self-references, fields, accessors, static and computed members,
//!   decorators and explicit `super` operations retain the ordinary fallback.
//!   An implicit derived constructor keeps JavaScript's native forwarding.
//! * The statements the tree has: returns, `if`, blocks, `throw`,
//!   `try`/`catch`/`finally`, `while`, `for` (unless a closure could capture
//!   its per-iteration `let`), `for…in`/`for…of` over one `let`/`const`,
//!   unlabelled `break`/`continue`.
//! * The expressions the tree has, `instanceof` included. `x op= y` becomes
//!   `x = x op y` where reading the target again observes nothing: a binding,
//!   or a member of a binding or `this` under a fixed key. `x++` and `x--`,
//!   their value unused, become `x = x ± 1` for a binding that every write
//!   keeps a number. `o?.k` and `o?.[k]` become `o===null||o===void 0?void
//!   0:o.k` for a binding `o`.
//! * Functions and arrows with plain parameters, neither async nor
//!   generators, and not naming themselves. Their names are not observed:
//!   the program reaches host code only through what it imports, and D2
//!   publishes none of it.
//!
//! Host modules are strict, so the caller lowers them only into a strict
//! output. They evaluate before their importer, so the lowered statements
//! lead the root, modules in dependency order.
use super::*;
use crate::compilation_policy::WorkKind::Analysis;
use crate::host_modules::HostDelivery;
use oxc_ast::{ast, AstKind};
use oxc_ast_visit::Visit;

/// Why lowering stopped: a construct the tree does not represent, or the
/// budget.
enum Stop {
    Refused,
    Budget(AllocationError),
}

impl From<AllocationError> for Stop {
    fn from(error: AllocationError) -> Self {
        Self::Budget(error)
    }
}

type Lowered<T> = Result<T, Stop>;

/// One lexical scope: its target scope and the names declared in it.
struct Scope {
    id: ScopeId,
    names: Vec<Name>,
}

struct Name {
    text: String,
    binding: BindingId,
    constant: bool,
}

struct Lowering<'m, 'b> {
    module: &'m mut Module,
    budget: &'m mut AllocationBudget<'b>,
    /// Innermost last.
    scopes: Vec<Scope>,
    /// Enclosing functions, innermost last: whether each is an arrow.
    functions: Vec<bool>,
    /// Bindings every write keeps a number, so `x++` is `x=x+1`.
    numeric: Vec<BindingId>,
}

impl Module {
    /// Lower the reachable modules `delivery` carries ahead of the root's statements,
    /// in dependency order, and point this module's imports of them at the
    /// lowered bindings. Returns whether it did: a module the tree cannot
    /// represent leaves the whole delivery as text. Nodes lowered before such
    /// a refusal stay unreachable, as edits leave them.
    pub(crate) fn lower_hosts(
        &mut self,
        delivery: &HostDelivery,
        source_modules: usize,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        if delivery.is_empty() {
            return Ok(false);
        }
        let mut needed = budget.filled(AllocationClass::Scratch, delivery.modules.len(), false)?;
        for import in &self.imports {
            budget.work(Analysis, delivery.modules.len() as u64 + 1)?;
            if let Some(index) = import.source.as_unicode().and_then(|source| delivery.position(source)) {
                needed[index] = true;
            }
        }
        // Delivery is in dependency order, so one reverse walk closes the set.
        for index in (0..needed.len()).rev() {
            if needed[index] {
                for &(dependency, _) in delivery.modules[index].imports() {
                    budget.work(Analysis, 1)?;
                    needed[dependency] = true;
                }
            }
        }
        let result = self.lower_needed_hosts(delivery, source_modules, &needed, budget);
        let bytes = crate::output_budget::vector_bytes(&needed)?;
        drop(needed);
        budget.release(AllocationClass::Scratch, bytes)?;
        if matches!(result, Ok(true)) { self.tables_mut().integrated_hosts = true; }
        result
    }

    fn lower_needed_hosts(
        &mut self,
        delivery: &HostDelivery,
        source_modules: usize,
        needed: &[bool],
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        // Namespace reflection needs the module namespace implementation in
        // opaque delivery, including its read-only exotic operations.
        if delivery.modules.iter().enumerate().any(|(index, module)| needed[index] &&
            module.imports().iter().any(|(_, bindings)| bindings.iter().any(|(name, _)| name.is_none()))) {
            return Ok(false);
        }
        let root_scope = self.regions[self.root.index()].scope;
        // Each lowered module's exports: exported name and binding.
        let mut exports: Vec<Vec<(String, BindingId)>> = Vec::new();
        let mut statements = Vec::new();
        // Per lowered statement: a host function declaration creates its
        // function and runs nothing (a definition); every other statement is
        // host code the program cannot see into (anchored).
        let mut anchors = Vec::new();
        for (host_index, host) in delivery.modules.iter().enumerate() {
            if !needed[host_index] {
                exports.push(Vec::new());
                continue;
            }
            budget.work(Analysis, host.body().len() as u64)?;
            let arena = oxc_allocator::Allocator::default();
            let Ok(tree) = crate::host_modules::parse_program(&arena, host.body()) else {
                return Ok(false);
            };
            let mut lowering = Lowering {
                module: self,
                budget,
                scopes: vec![Scope {
                    id: root_scope,
                    names: Vec::new(),
                }],
                functions: Vec::new(),
                numeric: Vec::new(),
            };
            // A module's imports name bindings of the modules before it.
            for (source, bindings) in host.imports() {
                for (imported, local) in bindings {
                    let binding = imported.as_ref().and_then(|imported| {
                        exports[*source]
                            .iter()
                            .find(|(name, _)| name == imported)
                            .map(|&(_, binding)| binding)
                    });
                    let Some(binding) = binding else {
                        return Ok(false);
                    };
                    lowering.scopes[0].names.push(Name {
                        text: local.clone(),
                        binding,
                        constant: true,
                    });
                }
            }
            let declared = tree
                .body
                .iter()
                .filter(|statement| matches!(statement, ast::Statement::FunctionDeclaration(_)))
                .count();
            let lowered = lowering.block_statements(&tree.body);
            match lowered {
                Ok(lowered) => {
                    let owner = u32::try_from(
                        source_modules
                            .checked_add(host_index)
                            .ok_or(AllocationError::Capacity)?,
                    )
                    .map_err(|_| AllocationError::Capacity)?;
                    anchors.extend((0..lowered.len()).map(|position| {
                        (
                            owner,
                            if position < declared {
                                Anchor::Definition
                            } else {
                                Anchor::Anchored
                            },
                        )
                    }));
                    statements.extend(lowered)
                }
                Err(Stop::Refused) => return Ok(false),
                Err(Stop::Budget(error)) => return Err(error),
            }
            let mut table = Vec::with_capacity(host.exports().len());
            for (exported, local) in host.exports() {
                let found = lowering.scopes[0]
                    .names
                    .iter()
                    .rev()
                    .find(|name| name.text == *local);
                let Some(found) = found else {
                    return Ok(false);
                };
                table.push((exported.clone(), found.binding));
            }
            exports.push(table);
        }
        // Every import of a delivered module must name one of its exports.
        let mut redirect = Vec::new();
        for import in &self.imports {
            budget.work(Analysis, 1)?;
            if import.imported.is_empty() { continue; }
            let Some(position) = import
                .source
                .as_unicode()
                .and_then(|source| delivery.position(source))
            else {
                continue;
            };
            let Some(&(_, binding)) = exports[position]
                .iter()
                .find(|(name, _)| *name == import.imported)
            else {
                return Ok(false);
            };
            redirect.push((import.binding, binding));
        }
        redirect.sort_unstable_by_key(|&(from, _)| from);
        let find = |binding: BindingId| {
            redirect
                .binary_search_by_key(&binding, |&(from, _)| from)
                .ok()
                .map(|index| redirect[index].1)
        };
        budget.work(Analysis, self.expressions.len() as u64)?;
        for index in 0..self.expressions.len() {
            if let Expr::Binding(binding) = self.expressions[index] {
                if let Some(lowered) = find(binding) {
                    self.set_expression(ExprId::new(index), Expr::Binding(lowered));
                }
            }
        }
        if self
            .exports
            .iter()
            .any(|export| find(export.binding).is_some())
        {
            for export in &mut self.tables_mut().exports {
                if let Some(lowered) = find(export.binding) {
                    export.binding = lowered;
                }
            }
        }
        let lowered = |import: &Import| {
            import
                .source
                .as_unicode()
                .is_some_and(|source| delivery.position(source).is_some())
        };
        if self.imports.iter().any(lowered) {
            self.tables_mut().imports.retain(|import| !lowered(import));
        }
        // An imported module evaluates before its importer.
        let count = statements.len();
        let root = self.root.index();
        budget.reserve_vec(
            AllocationClass::Retained,
            &mut self.regions[root].statements,
            count,
        )?;
        // Host code: rows of their own origin (design §7.9).
        if self.root_rows.len() == self.regions[root].statements.len() {
            budget.reserve_vec(AllocationClass::Retained, &mut self.root_rows, count)?;
        }
        self.prepend_roots(
            statements,
            anchors.into_iter().map(|(owner, anchor)| RootRow {
                completes: None,
                hoisted: false,
                module: owner,
                anchor,
                origin: RowOrigin::Host,
                point: None,
            }),
        );
        Ok(true)
    }
}

impl Lowering<'_, '_> {
    fn scope(&self) -> ScopeId {
        self.scopes.last().expect("a lowering scope").id
    }

    fn expression(&mut self, expression: Expr) -> Lowered<ExprId> {
        Ok(self.module.expression_in(expression, None, self.budget)?)
    }

    fn lookup(&self, name: &str) -> Option<&Name> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.names.iter().rev().find(|found| found.text == name))
    }

    /// The binding `name` declares in the innermost scope.
    fn own(&self, name: &str) -> Lowered<BindingId> {
        self.scopes
            .last()
            .and_then(|scope| scope.names.iter().find(|found| found.text == name))
            .map(|found| found.binding)
            .ok_or(Stop::Refused)
    }

    fn declare(&mut self, name: &str, constant: bool) -> Lowered<BindingId> {
        let scope = self.scopes.last().expect("a lowering scope");
        if scope.names.iter().any(|found| found.text == name) {
            return Err(Stop::Refused);
        }
        let binding = self.module.binding_in(
            Binding {
                source_symbol: None,
                scope: scope.id,
                spelling: name.to_string(),
                pinned: false,
                class: None,
                defined: false,
            },
            self.budget,
        )?;
        self.scopes
            .last_mut()
            .expect("a lowering scope")
            .names
            .push(Name {
                text: name.to_string(),
                binding,
                constant,
            });
        Ok(binding)
    }

    /// A fresh region under the current scope, entered.
    fn enter(&mut self) -> Lowered<RegionId> {
        let region = self.module.region_in(self.scope(), self.budget)?;
        let id = self.module.regions[region.index()].scope;
        self.scopes.push(Scope {
            id,
            names: Vec::new(),
        });
        Ok(region)
    }

    fn fill(&mut self, region: RegionId, statements: Vec<Statement>) -> Lowered<()> {
        let target = &mut self.module.regions[region.index()].statements;
        self.budget
            .reserve_vec(AllocationClass::Retained, target, statements.len())?;
        target.extend(statements);
        Ok(())
    }

    fn block(&mut self, body: &[ast::Statement<'_>]) -> Lowered<RegionId> {
        let region = self.enter()?;
        let statements = self.block_statements(body)?;
        self.scopes.pop();
        self.fill(region, statements)?;
        Ok(region)
    }

    fn block_statements(&mut self, body: &[ast::Statement<'_>]) -> Lowered<Vec<Statement>> {
        self.budget.work(Analysis, body.len() as u64)?;
        for statement in body {
            match statement {
                ast::Statement::FunctionDeclaration(function) => {
                    self.declare(
                        function.id.as_ref().ok_or(Stop::Refused)?.name.as_str(),
                        false,
                    )?;
                }
                ast::Statement::VariableDeclaration(declaration) => {
                    let constant = constant(declaration.kind)?;
                    for declarator in &declaration.declarations {
                        let name = identifier(&declarator.id)?;
                        let binding = self.declare(name, constant)?;
                        let mut facts = Observations::new(name);
                        for node in body {
                            facts.visit_statement(node);
                        }
                        if !constant && numeric_literal(declarator.init.as_ref()) && facts.numeric {
                            self.numeric.push(binding);
                        }
                    }
                }
                ast::Statement::ClassDeclaration(_) => return Err(Stop::Refused),
                _ => {}
            }
        }
        let mut statements = Vec::new();
        for statement in body {
            if let ast::Statement::FunctionDeclaration(node) = statement {
                let binding = self.own(node.id.as_ref().ok_or(Stop::Refused)?.name.as_str())?;
                let function = self.function(node)?;
                let value = self.expression(Expr::Function(function))?;
                statements.push(Statement::Let {
                    binding,
                    value: Some(value),
                });
            }
        }
        for statement in body {
            if !matches!(statement, ast::Statement::FunctionDeclaration(_)) {
                self.statement(statement, &mut statements)?;
            }
        }
        Ok(statements)
    }

    fn nested(&mut self, node: &ast::Statement<'_>) -> Lowered<RegionId> {
        if let ast::Statement::BlockStatement(node) = node {
            return self.block(&node.body);
        }
        let region = self.enter()?;
        let mut statements = Vec::new();
        self.statement(node, &mut statements)?;
        self.scopes.pop();
        self.fill(region, statements)?;
        Ok(region)
    }

    fn variables(
        &mut self,
        node: &ast::VariableDeclaration<'_>,
        out: &mut Vec<Statement>,
    ) -> Lowered<()> {
        for declarator in &node.declarations {
            let binding = self.own(identifier(&declarator.id)?)?;
            let value = declarator
                .init
                .as_ref()
                .map(|value| self.expr(value))
                .transpose()?;
            out.push(Statement::Let { binding, value });
        }
        Ok(())
    }

    fn statement(&mut self, node: &ast::Statement<'_>, out: &mut Vec<Statement>) -> Lowered<()> {
        use ast::Statement as S;
        self.budget.work(Analysis, 1)?;
        match node {
            S::EmptyStatement(_) => {}
            S::ExpressionStatement(node) => {
                out.push(Statement::Evaluate(self.discarded(&node.expression)?))
            }
            S::VariableDeclaration(node) => self.variables(node, out)?,
            S::ReturnStatement(node) => {
                if self.functions.is_empty() {
                    return Err(Stop::Refused);
                }
                out.push(Statement::Return(
                    node.argument
                        .as_ref()
                        .map(|value| self.expr(value))
                        .transpose()?,
                ));
            }
            S::IfStatement(node) => {
                let condition = self.expr(&node.test)?;
                let yes = self.nested(&node.consequent)?;
                let no = node
                    .alternate
                    .as_ref()
                    .map(|value| self.nested(value))
                    .transpose()?;
                out.push(Statement::If { condition, yes, no });
            }
            S::BlockStatement(node) => out.push(Statement::Block(self.block(&node.body)?)),
            S::ThrowStatement(node) => out.push(Statement::Throw(self.expr(&node.argument)?)),
            S::TryStatement(node) => {
                let body = self.block(&node.block.body)?;
                let catch = if let Some(handler) = &node.handler {
                    let region = self.enter()?;
                    let binding = handler
                        .param
                        .as_ref()
                        .map(|param| self.declare(identifier(&param.pattern)?, false))
                        .transpose()?;
                    let statements = self.block_statements(&handler.body.body)?;
                    self.scopes.pop();
                    self.fill(region, statements)?;
                    Some(Catch {
                        binding,
                        body: region,
                    })
                } else {
                    None
                };
                let finally = node
                    .finalizer
                    .as_ref()
                    .map(|node| self.block(&node.body))
                    .transpose()?;
                out.push(Statement::Try {
                    body,
                    catch,
                    finally,
                });
            }
            S::WhileStatement(node) => {
                let condition = Some(self.expr(&node.test)?);
                let body = self.nested(&node.body)?;
                out.push(Statement::Loop {
                    condition,
                    update: None,
                    body,
                });
            }
            S::ForStatement(node) => self.for_statement(node, out)?,
            S::ForInStatement(node) => {
                self.for_each(&node.left, &node.right, &node.body, false, out)?
            }
            S::ForOfStatement(node) if !node.r#await => {
                self.for_each(&node.left, &node.right, &node.body, true, out)?
            }
            S::BreakStatement(node) if node.label.is_none() => out.push(Statement::Break),
            S::ContinueStatement(node) if node.label.is_none() => out.push(Statement::Continue),
            _ => return Err(Stop::Refused),
        }
        Ok(())
    }

    fn for_each(
        &mut self,
        left: &ast::ForStatementLeft<'_>,
        right: &ast::Expression<'_>,
        node: &ast::Statement<'_>,
        of: bool,
        out: &mut Vec<Statement>,
    ) -> Lowered<()> {
        let ast::ForStatementLeft::VariableDeclaration(left) = left else {
            return Err(Stop::Refused);
        };
        let constant = constant(left.kind)?;
        let [declarator] = left.declarations.as_slice() else {
            return Err(Stop::Refused);
        };
        if declarator.init.is_some() {
            return Err(Stop::Refused);
        }
        let name = identifier(&declarator.id)?;
        let mut facts = Observations::new(name);
        facts.visit_expression(right);
        if facts.mentioned {
            return Err(Stop::Refused);
        }
        // A target loop has one cell; a captured source binding has a fresh
        // cell on each iteration. Keep the latter in host text delivery.
        facts.visit_statement(node);
        if facts.captured {
            return Err(Stop::Refused);
        }
        let object = self.expr(right)?;
        let body = self.enter()?;
        let binding = self.declare(name, constant)?;
        let statements = if let ast::Statement::BlockStatement(node) = node {
            self.block_statements(&node.body)?
        } else {
            let mut statements = Vec::new();
            self.statement(node, &mut statements)?;
            statements
        };
        self.scopes.pop();
        self.fill(body, statements)?;
        out.push(if of {
            Statement::ForOf {
                binding,
                iterable: object,
                body,
            }
        } else {
            Statement::ForIn {
                binding,
                object,
                body,
            }
        });
        Ok(())
    }

    fn for_statement(
        &mut self,
        node: &ast::ForStatement<'_>,
        out: &mut Vec<Statement>,
    ) -> Lowered<()> {
        let declaration = match &node.init {
            Some(ast::ForStatementInit::VariableDeclaration(node)) => Some(node),
            _ => None,
        };
        let region = if declaration.is_some() {
            Some(self.enter()?)
        } else {
            None
        };
        let mut statements = Vec::new();
        if let Some(declaration) = declaration {
            let constant = constant(declaration.kind)?;
            for declarator in &declaration.declarations {
                let name = identifier(&declarator.id)?;
                let mut facts = Observations::new(name);
                if let Some(test) = &node.test {
                    facts.visit_expression(test);
                }
                if let Some(update) = &node.update {
                    facts.visit_expression(update);
                }
                facts.visit_statement(&node.body);
                if facts.captured {
                    return Err(Stop::Refused);
                }
                let binding = self.declare(name, constant)?;
                if !constant && numeric_literal(declarator.init.as_ref()) && facts.numeric {
                    self.numeric.push(binding);
                }
            }
            self.variables(declaration, &mut statements)?;
        } else if let Some(init) = &node.init {
            statements.push(Statement::Evaluate(
                self.discarded(init.as_expression().ok_or(Stop::Refused)?)?,
            ));
        }
        let condition = node
            .test
            .as_ref()
            .map(|value| self.expr(value))
            .transpose()?;
        let update = node
            .update
            .as_ref()
            .map(|value| self.discarded(value))
            .transpose()?;
        let body = self.nested(&node.body)?;
        statements.push(Statement::Loop {
            condition,
            update,
            body,
        });
        if let Some(region) = region {
            self.scopes.pop();
            self.fill(region, statements)?;
            out.push(Statement::Block(region));
        } else {
            out.extend(statements);
        }
        Ok(())
    }

    fn function(&mut self, node: &ast::Function<'_>) -> Lowered<FunctionId> {
        if node.r#async || node.generator {
            return Err(Stop::Refused);
        }
        self.callable(
            &node.params,
            Some(node.body.as_ref().ok_or(Stop::Refused)?),
            None,
            false,
        )
    }
    fn class_expression(&mut self, node: &ast::Class<'_>) -> Lowered<Expr> {
        let name = node.id.as_ref().ok_or(Stop::Refused)?.name.as_str();
        if !node.decorators.is_empty() {
            return Err(Stop::Refused);
        }
        // The target's class name is observable but not a lexical binding ID.
        // Refuse self-reference and reserve its spelling against capture by a
        // renamed outer binding in the class's inner name environment.
        let mut observations = Observations::new(name);
        observations.visit_class(node);
        if observations.mentioned {
            return Err(Stop::Refused);
        }
        self.budget.work(Analysis, node.span.size() as u64)?;
        let reserved = self.budget.string(AllocationClass::Retained, name)?;
        self.budget.push(
            AllocationClass::Retained,
            &mut self.module.reserved,
            reserved,
        )?;
        let mut constructor = None;
        let mut methods = Vec::new();
        for element in &node.body.body {
            let ast::ClassElement::MethodDefinition(method) = element else {
                return Err(Stop::Refused);
            };
            if method.computed || method.r#static || !method.decorators.is_empty() {
                return Err(Stop::Refused);
            }
            let ast::PropertyKey::StaticIdentifier(key) = &method.key else {
                return Err(Stop::Refused);
            };
            match method.kind {
                ast::MethodDefinitionKind::Constructor if constructor.is_none() => {
                    constructor = Some(self.class_method(&method.value, name)?)
                }
                ast::MethodDefinitionKind::Method => methods.push((
                    key.name.to_string(),
                    self.class_method(&method.value, key.name.as_str())?,
                )),
                _ => return Err(Stop::Refused),
            }
        }
        let base = node
            .super_class
            .as_ref()
            .map(|base| self.expr(base))
            .transpose()?;
        Ok(Expr::Class {
            name: name.into(),
            base,
            constructor,
            methods,
            members: Vec::new(),
        })
    }
    fn class_method(&mut self, node: &ast::Function<'_>, name: &str) -> Lowered<FunctionId> {
        let function = self.function(node)?;
        // A returned constructor and its prototype methods are observable
        // host values. Keep even unused formals in their public arity.
        self.module.functions[function.index()].name = FunctionName::Exact(name.into());
        Ok(function)
    }
    fn callable(
        &mut self,
        params: &ast::FormalParameters<'_>,
        block: Option<&ast::FunctionBody<'_>>,
        value: Option<&ast::Expression<'_>>,
        arrow: bool,
    ) -> Lowered<FunctionId> {
        if params.rest.is_some() {
            return Err(Stop::Refused);
        }
        let body = self.enter()?;
        self.functions.push(arrow);
        let mut parameters = Vec::new();
        for parameter in &params.items {
            if parameter.initializer.is_some() {
                return Err(Stop::Refused);
            }
            parameters.push(self.declare(identifier(&parameter.pattern)?, false)?);
        }
        let statements = if let Some(value) = value {
            vec![Statement::Return(Some(self.expr(value)?))]
        } else {
            self.block_statements(&block.ok_or(Stop::Refused)?.statements)?
        };
        self.functions.pop();
        self.scopes.pop();
        self.fill(body, statements)?;
        let id =
            FunctionId::try_new(self.module.functions.len()).ok_or(AllocationError::Capacity)?;
        self.budget.push(
            AllocationClass::Retained,
            &mut self.module.functions,
            Function {
                rest: false,
                parameters,
                body,
                arrow,
                name: FunctionName::Unobserved,
                strict: false,
                length: None,
                suspension: Suspension::None,
            },
        )?;
        Ok(id)
    }

    fn reference(&mut self, name: &str) -> Lowered<ExprId> {
        let expression = match self.lookup(name) {
            Some(found) => Expr::Binding(found.binding),
            None => match name {
                "undefined" => Expr::Literal(Literal::Undefined),
                "arguments" if self.functions.iter().any(|arrow| !arrow) => {
                    Expr::Host(Host::new(name))
                }
                "arguments" | "eval" => return Err(Stop::Refused),
                _ => Expr::Host(Host::new(name)),
            },
        };
        self.expression(expression)
    }

    fn expr(&mut self, node: &ast::Expression<'_>) -> Lowered<ExprId> {
        use ast::Expression as E;
        self.budget.work(Analysis, 1)?;
        let expression = match node {
            E::Identifier(node) => return self.reference(node.name.as_str()),
            E::NullLiteral(_) => Expr::Literal(Literal::Null),
            E::BooleanLiteral(node) => Expr::Literal(Literal::Bool(node.value)),
            E::NumericLiteral(node) => Expr::Literal(Literal::Number(node.value)),
            E::StringLiteral(node) if !node.lone_surrogates => {
                Expr::Literal(Literal::String(node.value.as_str().into()))
            }
            E::RegExpLiteral(node) => {
                Expr::Regex(node.raw.as_ref().ok_or(Stop::Refused)?.to_string())
            }
            E::ThisExpression(_) if self.functions.iter().any(|arrow| !arrow) => Expr::This,
            E::ClassExpression(node) => self.class_expression(node)?,
            E::TemplateLiteral(node) => {
                let mut parts = Vec::new();
                for (index, quasi) in node.quasis.iter().enumerate() {
                    let cooked = quasi
                        .value
                        .cooked
                        .as_ref()
                        .filter(|s| !s.contains('\u{FFFD}'))
                        .ok_or(Stop::Refused)?;
                    if !cooked.is_empty() {
                        parts.push(TemplatePart::String(cooked.as_str().into()));
                    }
                    if let Some(value) = node.expressions.get(index) {
                        parts.push(TemplatePart::Expression(self.expr(value)?));
                    }
                }
                Expr::Template(parts)
            }
            E::ArrayExpression(node) => {
                let mut values = Vec::new();
                for element in &node.elements {
                    let value = match element {
                        ast::ArrayExpressionElement::SpreadElement(node) => {
                            let value = self.expr(&node.argument)?;
                            self.expression(Expr::Spread(value))?
                        }
                        _ => self.expr(element.as_expression().ok_or(Stop::Refused)?)?,
                    };
                    values.push(value);
                }
                Expr::Array(values)
            }
            E::ObjectExpression(node) => {
                let mut entries = Vec::new();
                for property in &node.properties {
                    let ast::ObjectPropertyKind::ObjectProperty(property) = property else {
                        return Err(Stop::Refused);
                    };
                    if property.kind != ast::PropertyKind::Init || property.method {
                        return Err(Stop::Refused);
                    }
                    let key = if property.computed {
                        Property::Computed(
                            self.expr(property.key.as_expression().ok_or(Stop::Refused)?)?,
                        )
                    } else {
                        match &property.key {
                            ast::PropertyKey::StaticIdentifier(key)
                                if !(property.shorthand && key.name == "__proto__") =>
                            {
                                Property::Named(key.name.to_string())
                            }
                            ast::PropertyKey::StaticIdentifier(key) => {
                                Property::Computed(self.expression(Expr::Literal(
                                    Literal::String(key.name.as_str().into()),
                                ))?)
                            }
                            ast::PropertyKey::StringLiteral(key)
                                if !key.lone_surrogates && identifier_name(key.value.as_str()) =>
                            {
                                Property::Named(key.value.to_string())
                            }
                            ast::PropertyKey::StringLiteral(_)
                            | ast::PropertyKey::NumericLiteral(_) => Property::Computed(
                                self.expr(property.key.as_expression().ok_or(Stop::Refused)?)?,
                            ),
                            _ => return Err(Stop::Refused),
                        }
                    };
                    let value = self.expr(&property.value)?;
                    entries.push((key, value));
                }
                Expr::Object(entries)
            }
            E::FunctionExpression(node) if node.id.is_none() => {
                Expr::Function(self.function(node)?)
            }
            E::ArrowFunctionExpression(node) if !node.r#async => {
                let (block, value) = match &node.body {
                    ast::ArrowFunctionBody::FunctionBody(body) => (Some(body.as_ref()), None),
                    body => (None, body.as_expression()),
                };
                Expr::Function(self.callable(&node.params, block, value, true)?)
            }
            E::UnaryExpression(node) => {
                let op = match node.operator.as_str() {
                    "-" => Unary::Negate,
                    "+" => Unary::Plus,
                    "!" => Unary::Not,
                    "~" => Unary::BitNot,
                    "typeof" => Unary::TypeOf,
                    "void" => Unary::Void,
                    "delete" => Unary::Delete,
                    _ => return Err(Stop::Refused),
                };
                if op == Unary::Delete
                    && node
                        .argument
                        .as_member_expression()
                        .is_none_or(|member| member.optional())
                {
                    return Err(Stop::Refused);
                }
                Expr::Unary {
                    op,
                    value: self.expr(&node.argument)?,
                }
            }
            E::BinaryExpression(node) => Expr::Binary {
                op: binary(node.operator.as_str())?,
                left: self.expr(&node.left)?,
                right: self.expr(&node.right)?,
            },
            E::LogicalExpression(node) => Expr::Binary {
                op: binary(node.operator.as_str())?,
                left: self.expr(&node.left)?,
                right: self.expr(&node.right)?,
            },
            E::ConditionalExpression(node) => Expr::Conditional {
                condition: self.expr(&node.test)?,
                yes: self.expr(&node.consequent)?,
                no: self.expr(&node.alternate)?,
            },
            E::SequenceExpression(node) => {
                let mut values = Vec::new();
                for value in &node.expressions {
                    values.push(self.expr(value)?);
                }
                Expr::Sequence(values)
            }
            E::AssignmentExpression(node) => {
                let target = self.target(&node.left)?;
                let value = if node.operator.as_str() == "=" {
                    self.expr(&node.right)?
                } else {
                    let spelling = node
                        .operator
                        .as_str()
                        .strip_suffix('=')
                        .ok_or(Stop::Refused)?;
                    let op = binary(spelling)?;
                    if matches!(op, Binary::And | Binary::Or | Binary::Nullish) {
                        return Err(Stop::Refused);
                    }
                    let left = self.reread(&node.left)?;
                    let right = self.expr(&node.right)?;
                    self.expression(Expr::Binary { op, left, right })?
                };
                Expr::Assign { target, value }
            }
            E::ComputedMemberExpression(_) | E::StaticMemberExpression(_) => {
                let member = node.as_member_expression().ok_or(Stop::Refused)?;
                if member.optional() {
                    return Err(Stop::Refused);
                }
                self.member(member)?
            }
            E::ChainExpression(node) => {
                let inner = node
                    .expression
                    .as_member_expression()
                    .ok_or(Stop::Refused)?;
                if !inner.optional() {
                    return Err(Stop::Refused);
                }
                let E::Identifier(object) = inner.object() else {
                    return Err(Stop::Refused);
                };
                let binding = self
                    .lookup(object.name.as_str())
                    .map(|found| found.binding)
                    .ok_or(Stop::Refused)?;
                let left = self.absent(binding, Literal::Null)?;
                let right = self.absent(binding, Literal::Undefined)?;
                let condition = self.expression(Expr::Binary {
                    op: Binary::Or,
                    left,
                    right,
                })?;
                let yes = self.expression(Expr::Literal(Literal::Undefined))?;
                let object = self.expression(Expr::Binding(binding))?;
                let property = self.property(inner)?;
                let no = self.expression(Expr::Member { object, property })?;
                Expr::Conditional { condition, yes, no }
            }
            E::CallExpression(node) if !node.optional => {
                let callee = self.expr(&node.callee)?;
                let arguments = self.arguments(&node.arguments)?;
                let invocation =
                    if matches!(self.module.expressions[callee.index()], Expr::Member { .. }) {
                        Invocation::Reference
                    } else {
                        Invocation::Value
                    };
                Expr::Call {
                    callee,
                    arguments,
                    invocation,
                }
            }
            E::NewExpression(node) => Expr::Construct {
                callee: self.expr(&node.callee)?,
                arguments: self.arguments(&node.arguments)?,
            },
            E::ParenthesizedExpression(node) => return self.expr(&node.expression),
            _ => return Err(Stop::Refused),
        };
        self.expression(expression)
    }

    fn absent(&mut self, binding: BindingId, value: Literal) -> Lowered<ExprId> {
        let left = self.expression(Expr::Binding(binding))?;
        let right = self.expression(Expr::Literal(value))?;
        self.expression(Expr::Binary {
            op: Binary::StrictEqual,
            left,
            right,
        })
    }
    fn arguments(&mut self, nodes: &[ast::Argument<'_>]) -> Lowered<Vec<ExprId>> {
        let mut values = Vec::new();
        for node in nodes {
            values.push(self.expr(node.as_expression().ok_or(Stop::Refused)?)?);
        }
        Ok(values)
    }
    fn member(&mut self, node: &ast::MemberExpression<'_>) -> Lowered<Expr> {
        let object = self.expr(node.object())?;
        let property = self.property(node)?;
        Ok(Expr::Member { object, property })
    }
    fn property(&mut self, node: &ast::MemberExpression<'_>) -> Lowered<Property> {
        match node {
            ast::MemberExpression::StaticMemberExpression(node) => {
                Ok(Property::Named(node.property.name.to_string()))
            }
            ast::MemberExpression::ComputedMemberExpression(node) => {
                Ok(Property::Computed(self.expr(&node.expression)?))
            }
            _ => Err(Stop::Refused),
        }
    }
    fn target(&mut self, node: &ast::AssignmentTarget<'_>) -> Lowered<ExprId> {
        if let ast::AssignmentTarget::AssignmentTargetIdentifier(identifier) = node {
            let name = identifier.name.as_str();
            let found = self
                .lookup(name)
                .filter(|found| !found.constant)
                .ok_or(Stop::Refused)?;
            return self.expression(Expr::Binding(found.binding));
        }
        let member = node
            .as_member_expression()
            .filter(|member| !member.optional())
            .ok_or(Stop::Refused)?;
        let expression = self.member(member)?;
        self.expression(expression)
    }
    fn reread(&mut self, node: &ast::AssignmentTarget<'_>) -> Lowered<ExprId> {
        if let ast::AssignmentTarget::AssignmentTargetIdentifier(identifier) = node {
            return self.reference(identifier.name.as_str());
        }
        let member = node.as_member_expression().ok_or(Stop::Refused)?;
        let fixed = match member {
            ast::MemberExpression::StaticMemberExpression(_) => true,
            ast::MemberExpression::ComputedMemberExpression(node) => matches!(
                node.expression,
                ast::Expression::NumericLiteral(_)
                    | ast::Expression::StringLiteral(_)
                    | ast::Expression::BooleanLiteral(_)
                    | ast::Expression::NullLiteral(_)
            ),
            _ => false,
        };
        let simple = match member.object() {
            ast::Expression::Identifier(node) => self.lookup(node.name.as_str()).is_some(),
            ast::Expression::ThisExpression(_) => true,
            _ => false,
        };
        if member.optional() || !fixed || !simple {
            return Err(Stop::Refused);
        }
        let expression = self.member(member)?;
        self.expression(expression)
    }
    fn discarded(&mut self, node: &ast::Expression<'_>) -> Lowered<ExprId> {
        let ast::Expression::UpdateExpression(node) = node else {
            return self.expr(node);
        };
        let ast::SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) = &node.argument
        else {
            return Err(Stop::Refused);
        };
        let name = identifier.name.as_str();
        let found = self
            .lookup(name)
            .filter(|found| !found.constant && self.numeric.contains(&found.binding))
            .ok_or(Stop::Refused)?;
        let binding = found.binding;
        let target = self.expression(Expr::Binding(binding))?;
        let left = self.expression(Expr::Binding(binding))?;
        let right = self.expression(Expr::Literal(Literal::Number(1.0)))?;
        let op = if node.operator.as_str() == "++" {
            Binary::Add
        } else {
            Binary::Subtract
        };
        let value = self.expression(Expr::Binary { op, left, right })?;
        self.expression(Expr::Assign { target, value })
    }
}

fn identifier<'a>(node: &'a ast::BindingPattern<'_>) -> Lowered<&'a str> {
    match node {
        ast::BindingPattern::BindingIdentifier(node) => Ok(node.name.as_str()),
        _ => Err(Stop::Refused),
    }
}
fn constant(kind: ast::VariableDeclarationKind) -> Lowered<bool> {
    match kind {
        ast::VariableDeclarationKind::Let => Ok(false),
        ast::VariableDeclarationKind::Const => Ok(true),
        _ => Err(Stop::Refused),
    }
}
fn numeric_literal(node: Option<&ast::Expression<'_>>) -> bool {
    matches!(node, Some(ast::Expression::NumericLiteral(_)))
}
fn binary(op: &str) -> Lowered<Binary> {
    Ok(match op {
        "+" => Binary::Add,
        "-" => Binary::Subtract,
        "*" => Binary::Multiply,
        "/" => Binary::Divide,
        "%" => Binary::Remainder,
        "<<" => Binary::ShiftLeft,
        ">>" => Binary::ShiftRight,
        ">>>" => Binary::UnsignedShiftRight,
        "<" => Binary::Less,
        "<=" => Binary::LessEqual,
        ">" => Binary::Greater,
        ">=" => Binary::GreaterEqual,
        "===" => Binary::StrictEqual,
        "!==" => Binary::StrictNotEqual,
        "==" => Binary::Equal,
        "!=" => Binary::NotEqual,
        "in" => Binary::In,
        "instanceof" => Binary::InstanceOf,
        "&" => Binary::BitAnd,
        "^" => Binary::BitXor,
        "|" => Binary::BitOr,
        "&&" => Binary::And,
        "||" => Binary::Or,
        "??" => Binary::Nullish,
        _ => return Err(Stop::Refused),
    })
}

/// Conservative observations: shadowing can only refuse a lowering. AST
/// binding/reference nodes avoid mistaking fixed property names for captures.
struct Observations<'n> {
    target: &'n str,
    depth: usize,
    mentioned: bool,
    captured: bool,
    numeric: bool,
}
impl<'n> Observations<'n> {
    fn new(target: &'n str) -> Self {
        Self {
            target,
            depth: 0,
            mentioned: false,
            captured: false,
            numeric: true,
        }
    }
}
impl<'a> Visit<'a> for Observations<'_> {
    fn enter_node(&mut self, node: AstKind<'a>) {
        match node {
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => self.depth += 1,
            AstKind::IdentifierReference(node) if node.name == self.target => {
                self.mentioned = true;
                self.captured |= self.depth != 0;
            }
            AstKind::AssignmentExpression(node) => {
                if matches!(&node.left, ast::AssignmentTarget::AssignmentTargetIdentifier(identifier) if identifier.name == self.target)
                {
                    self.numeric &= matches!(node.operator.as_str(), "=" | "+=" | "-=")
                        && numeric_literal(Some(&node.right));
                } else {
                    let mut writes = Observations::new(self.target);
                    writes.visit_assignment_target(&node.left);
                    self.numeric &= !writes.mentioned;
                }
            }
            AstKind::ForInStatement(node) => self.loop_write(&node.left),
            AstKind::ForOfStatement(node) => self.loop_write(&node.left),
            _ => {}
        }
    }
    fn leave_node(&mut self, node: AstKind<'a>) {
        if matches!(
            node,
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
        ) {
            self.depth -= 1;
        }
    }
}
impl Observations<'_> {
    fn loop_write(&mut self, left: &ast::ForStatementLeft<'_>) {
        let mut facts = Observations::new(self.target);
        facts.visit_for_statement_left(left);
        self.numeric &= !facts.mentioned;
    }
}
