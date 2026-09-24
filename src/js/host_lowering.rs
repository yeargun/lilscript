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
//!   runs nothing. `var` and classes are refused.
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
use serde_json::Value;

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
    /// Lower every module `delivery` carries ahead of the root's statements,
    /// in dependency order, and point this module's imports of them at the
    /// lowered bindings. Returns whether it did: a module the tree cannot
    /// represent leaves the whole delivery as text. Nodes lowered before such
    /// a refusal stay unreachable, as edits leave them.
    pub(crate) fn lower_hosts(
        &mut self,
        delivery: &HostDelivery,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        if delivery.is_empty() {
            return Ok(false);
        }
        let root_scope = self.regions[self.root.index()].scope;
        // Each lowered module's exports: exported name and binding.
        let mut exports: Vec<Vec<(String, BindingId)>> = Vec::new();
        let mut statements = Vec::new();
        for host in &delivery.modules {
            budget.work(Analysis, host.body().len() as u64)?;
            let Ok(tree) = crate::host_modules::parse_program(host.body()) else {
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
            let lowered = match tree.get("body").and_then(Value::as_array) {
                Some(body) => lowering.block_statements(body),
                None => Err(Stop::Refused),
            };
            match lowered {
                Ok(lowered) => statements.extend(lowered),
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
        for expression in &mut self.expressions {
            if let Expr::Binding(binding) = expression {
                if let Some(lowered) = find(*binding) {
                    *binding = lowered;
                }
            }
        }
        for export in &mut self.exports {
            if let Some(lowered) = find(export.binding) {
                export.binding = lowered;
            }
        }
        self.imports.retain(|import| {
            !import
                .source
                .as_unicode()
                .is_some_and(|source| delivery.position(source).is_some())
        });
        // An imported module evaluates before its importer.
        let count = statements.len();
        let root = self.root.index();
        budget.reserve_vec(
            AllocationClass::Retained,
            &mut self.regions[root].statements,
            count,
        )?;
        self.regions[root].statements.splice(0..0, statements);
        if let Some(&first) = self.root_modules.first() {
            budget.reserve_vec(AllocationClass::Retained, &mut self.root_modules, count)?;
            self.root_modules
                .splice(0..0, std::iter::repeat_n(first, count));
        }
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

    fn block(&mut self, body: &[Value]) -> Lowered<RegionId> {
        let region = self.enter()?;
        let statements = self.block_statements(body)?;
        self.scopes.pop();
        self.fill(region, statements)?;
        Ok(region)
    }

    /// A statement list in the current scope: its names declared first, its
    /// function declarations created first, then the rest in order.
    fn block_statements(&mut self, body: &[Value]) -> Lowered<Vec<Statement>> {
        self.budget.work(Analysis, body.len() as u64)?;
        for statement in body {
            match kind(statement) {
                "FunctionDeclaration" => {
                    self.declare(identifier(statement.get("id"))?, false)?;
                }
                "VariableDeclaration" => {
                    let constant = match text(statement, "kind") {
                        Some("let") => false,
                        Some("const") => true,
                        _ => return Err(Stop::Refused),
                    };
                    for declarator in array(statement, "declarations")? {
                        let name = identifier(declarator.get("id"))?;
                        let binding = self.declare(name, constant)?;
                        if !constant
                            && numeric_literal(declarator.get("init"))
                            && numeric_writes(body, name)
                        {
                            self.numeric.push(binding);
                        }
                    }
                }
                "ClassDeclaration" => return Err(Stop::Refused),
                _ => {}
            }
        }
        let mut statements = Vec::new();
        for statement in body {
            if kind(statement) == "FunctionDeclaration" {
                let binding = self.own(identifier(statement.get("id"))?)?;
                let function = self.function(statement, false)?;
                let value = self.expression(Expr::Function(function))?;
                statements.push(Statement::Let {
                    binding,
                    value: Some(value),
                });
            }
        }
        for statement in body {
            if kind(statement) != "FunctionDeclaration" {
                self.statement(statement, &mut statements)?;
            }
        }
        Ok(statements)
    }

    /// A statement's body: its block, or a region of the one statement.
    fn nested(&mut self, node: &Value) -> Lowered<RegionId> {
        if kind(node) == "BlockStatement" {
            return self.block(array(node, "body")?);
        }
        let region = self.enter()?;
        let mut statements = Vec::new();
        self.statement(node, &mut statements)?;
        self.scopes.pop();
        self.fill(region, statements)?;
        Ok(region)
    }

    fn statement(&mut self, node: &Value, out: &mut Vec<Statement>) -> Lowered<()> {
        self.budget.work(Analysis, 1)?;
        match kind(node) {
            "EmptyStatement" => {}
            "ExpressionStatement" => {
                if present(node, "directive") {
                    return Ok(());
                }
                let expression = field(node, "expression")?;
                let value = if kind(expression) == "UpdateExpression" {
                    self.update(expression)?
                } else {
                    self.expr(expression)?
                };
                out.push(Statement::Evaluate(value));
            }
            "VariableDeclaration" => {
                for declarator in array(node, "declarations")? {
                    let binding = self.own(identifier(declarator.get("id"))?)?;
                    let value = match declarator.get("init") {
                        Some(init) if !init.is_null() => Some(self.expr(init)?),
                        _ => None,
                    };
                    out.push(Statement::Let { binding, value });
                }
            }
            "ReturnStatement" => {
                if self.functions.is_empty() {
                    return Err(Stop::Refused);
                }
                let value = match node.get("argument") {
                    Some(argument) if !argument.is_null() => Some(self.expr(argument)?),
                    _ => None,
                };
                out.push(Statement::Return(value));
            }
            "IfStatement" => {
                let condition = self.expr(field(node, "test")?)?;
                let yes = self.nested(field(node, "consequent")?)?;
                let no = match node.get("alternate") {
                    Some(alternate) if !alternate.is_null() => Some(self.nested(alternate)?),
                    _ => None,
                };
                out.push(Statement::If { condition, yes, no });
            }
            "BlockStatement" => {
                let region = self.block(array(node, "body")?)?;
                out.push(Statement::Block(region));
            }
            "ThrowStatement" => {
                let value = self.expr(field(node, "argument")?)?;
                out.push(Statement::Throw(value));
            }
            "TryStatement" => {
                let body = self.block(array(field(node, "block")?, "body")?)?;
                let catch = match node.get("handler") {
                    Some(handler) if !handler.is_null() => {
                        let region = self.enter()?;
                        let binding = match handler.get("param") {
                            Some(param) if !param.is_null() => {
                                Some(self.declare(identifier(Some(param))?, false)?)
                            }
                            _ => None,
                        };
                        let statements =
                            self.block_statements(array(field(handler, "body")?, "body")?)?;
                        self.scopes.pop();
                        self.fill(region, statements)?;
                        Some(Catch {
                            binding,
                            body: region,
                        })
                    }
                    _ => None,
                };
                let finally = match node.get("finalizer") {
                    Some(finalizer) if !finalizer.is_null() => {
                        Some(self.block(array(finalizer, "body")?)?)
                    }
                    _ => None,
                };
                out.push(Statement::Try {
                    body,
                    catch,
                    finally,
                });
            }
            "WhileStatement" => {
                let condition = self.expr(field(node, "test")?)?;
                let body = self.nested(field(node, "body")?)?;
                out.push(Statement::Loop {
                    condition: Some(condition),
                    update: None,
                    body,
                });
            }
            "ForStatement" => self.for_statement(node, out)?,
            "ForInStatement" | "ForOfStatement" => {
                if flag(node, "await") {
                    return Err(Stop::Refused);
                }
                let left = field(node, "left")?;
                if kind(left) != "VariableDeclaration" {
                    return Err(Stop::Refused);
                }
                let constant = match text(left, "kind") {
                    Some("let") => false,
                    Some("const") => true,
                    _ => return Err(Stop::Refused),
                };
                let [declarator] = array(left, "declarations")? else {
                    return Err(Stop::Refused);
                };
                if present(declarator, "init") {
                    return Err(Stop::Refused);
                }
                let name = identifier(declarator.get("id"))?;
                // The head evaluates with the loop's binding in its TDZ.
                let right = field(node, "right")?;
                if mentions(right, name) {
                    return Err(Stop::Refused);
                }
                let object = self.expr(right)?;
                let body = self.enter()?;
                let binding = self.declare(name, constant)?;
                let body_node = field(node, "body")?;
                let statements = if kind(body_node) == "BlockStatement" {
                    self.block_statements(array(body_node, "body")?)?
                } else {
                    let mut statements = Vec::new();
                    self.statement(body_node, &mut statements)?;
                    statements
                };
                self.scopes.pop();
                self.fill(body, statements)?;
                out.push(if kind(node) == "ForInStatement" {
                    Statement::ForIn {
                        binding,
                        object,
                        body,
                    }
                } else {
                    Statement::ForOf {
                        binding,
                        iterable: object,
                        body,
                    }
                });
            }
            "BreakStatement" | "ContinueStatement" => {
                if present(node, "label") {
                    return Err(Stop::Refused);
                }
                out.push(if kind(node) == "BreakStatement" {
                    Statement::Break
                } else {
                    Statement::Continue
                });
            }
            _ => return Err(Stop::Refused),
        }
        Ok(())
    }

    /// `for(init;test;update)body`. A declared `let` lives in a block around
    /// the loop, where the tree keeps one cell for every iteration: only a
    /// closure created in the loop could tell, so such a loop is refused.
    fn for_statement(&mut self, node: &Value, out: &mut Vec<Statement>) -> Lowered<()> {
        let init = node.get("init").filter(|init| !init.is_null());
        let declares = init.is_some_and(|init| kind(init) == "VariableDeclaration");
        let parts: Vec<&Value> = ["test", "update", "body"]
            .into_iter()
            .filter_map(|part| node.get(part).filter(|value| !value.is_null()))
            .collect();
        let region = if declares { Some(self.enter()?) } else { None };
        let mut statements = Vec::new();
        if let Some(init) = init {
            if declares {
                let constant = match text(init, "kind") {
                    Some("let") => false,
                    Some("const") => true,
                    _ => return Err(Stop::Refused),
                };
                for declarator in array(init, "declarations")? {
                    let name = identifier(declarator.get("id"))?;
                    if parts.iter().any(|part| captures(part, name)) {
                        return Err(Stop::Refused);
                    }
                    let binding = self.declare(name, constant)?;
                    if !constant
                        && numeric_literal(declarator.get("init"))
                        && parts
                            .iter()
                            .all(|part| numeric_writes(std::slice::from_ref(*part), name))
                    {
                        self.numeric.push(binding);
                    }
                }
                self.statement(init, &mut statements)?;
            } else {
                let value = if kind(init) == "UpdateExpression" {
                    self.update(init)?
                } else {
                    self.expr(init)?
                };
                statements.push(Statement::Evaluate(value));
            }
        }
        let condition = match node.get("test") {
            Some(test) if !test.is_null() => Some(self.expr(test)?),
            _ => None,
        };
        let update = match node.get("update") {
            Some(update) if !update.is_null() => Some(if kind(update) == "UpdateExpression" {
                self.update(update)?
            } else {
                self.expr(update)?
            }),
            _ => None,
        };
        let body = self.nested(field(node, "body")?)?;
        statements.push(Statement::Loop {
            condition,
            update,
            body,
        });
        match region {
            Some(region) => {
                self.scopes.pop();
                self.fill(region, statements)?;
                out.push(Statement::Block(region));
            }
            None => out.extend(statements),
        }
        Ok(())
    }

    fn function(&mut self, node: &Value, arrow: bool) -> Lowered<FunctionId> {
        if flag(node, "async") || flag(node, "generator") {
            return Err(Stop::Refused);
        }
        let body = self.enter()?;
        self.functions.push(arrow);
        let mut parameters = Vec::new();
        for parameter in array(node, "params")? {
            if kind(parameter) != "Identifier" {
                return Err(Stop::Refused);
            }
            parameters.push(self.declare(name(parameter)?, false)?);
        }
        let body_node = field(node, "body")?;
        let statements = if arrow && flag(node, "expression") {
            vec![Statement::Return(Some(self.expr(body_node)?))]
        } else {
            self.block_statements(array(body_node, "body")?)?
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
                // Only a function's own `arguments`; an arrow reads its creator's.
                "arguments" if self.functions.iter().any(|arrow| !arrow) => Expr::Host(name.into()),
                "arguments" | "eval" => return Err(Stop::Refused),
                _ => Expr::Host(name.into()),
            },
        };
        self.expression(expression)
    }

    fn expr(&mut self, node: &Value) -> Lowered<ExprId> {
        self.budget.work(Analysis, 1)?;
        let expression = match kind(node) {
            "Identifier" => return self.reference(name(node)?),
            "Literal" => literal(node)?,
            "ThisExpression" => {
                if !self.functions.iter().any(|arrow| !arrow) {
                    return Err(Stop::Refused);
                }
                Expr::This
            }
            "TemplateLiteral" => {
                let expressions = array(node, "expressions")?;
                let mut parts = Vec::new();
                for (index, quasi) in array(node, "quasis")?.iter().enumerate() {
                    let cooked = quasi
                        .get("value")
                        .and_then(|value| value.get("cooked"))
                        .and_then(Value::as_str)
                        .filter(|cooked| !cooked.contains('\u{FFFD}'))
                        .ok_or(Stop::Refused)?;
                    if !cooked.is_empty() {
                        parts.push(TemplatePart::String(StringValue::from(cooked)));
                    }
                    if let Some(expression) = expressions.get(index) {
                        parts.push(TemplatePart::Expression(self.expr(expression)?));
                    }
                }
                Expr::Template(parts)
            }
            "ArrayExpression" => {
                let mut values = Vec::new();
                for element in array(node, "elements")? {
                    if element.is_null() {
                        return Err(Stop::Refused);
                    }
                    let value = if kind(element) == "SpreadElement" {
                        let inner = self.expr(field(element, "argument")?)?;
                        self.expression(Expr::Spread(inner))?
                    } else {
                        self.expr(element)?
                    };
                    values.push(value);
                }
                Expr::Array(values)
            }
            "ObjectExpression" => {
                let mut entries = Vec::new();
                for property in array(node, "properties")? {
                    if kind(property) != "Property"
                        || text(property, "kind") != Some("init")
                        || flag(property, "method")
                    {
                        return Err(Stop::Refused);
                    }
                    let key = field(property, "key")?;
                    let key = if flag(property, "computed") {
                        Property::Computed(self.expr(key)?)
                    } else {
                        match (kind(key), key.get("value")) {
                            // `{__proto__}` defines a property; `__proto__:` sets
                            // the prototype, as the named key prints.
                            ("Identifier", _)
                                if flag(property, "shorthand") && name(key)? == "__proto__" =>
                            {
                                let literal = Expr::Literal(Literal::String("__proto__".into()));
                                Property::Computed(self.expression(literal)?)
                            }
                            ("Identifier", _) => Property::Named(name(key)?.to_string()),
                            ("Literal", Some(Value::String(key))) if identifier_name(key) => {
                                Property::Named(key.clone())
                            }
                            ("Literal", Some(Value::String(_) | Value::Number(_))) => {
                                let literal = literal(key)?;
                                Property::Computed(self.expression(literal)?)
                            }
                            _ => return Err(Stop::Refused),
                        }
                    };
                    let value = self.expr(field(property, "value")?)?;
                    entries.push((key, value));
                }
                Expr::Object(entries)
            }
            "FunctionExpression" => {
                if present(node, "id") {
                    return Err(Stop::Refused);
                }
                Expr::Function(self.function(node, false)?)
            }
            "ArrowFunctionExpression" => Expr::Function(self.function(node, true)?),
            "UnaryExpression" => {
                let op = match text(node, "operator") {
                    Some("-") => Unary::Negate,
                    Some("+") => Unary::Plus,
                    Some("!") => Unary::Not,
                    Some("~") => Unary::BitNot,
                    Some("typeof") => Unary::TypeOf,
                    Some("void") => Unary::Void,
                    Some("delete") => Unary::Delete,
                    _ => return Err(Stop::Refused),
                };
                let argument = field(node, "argument")?;
                if op == Unary::Delete
                    && (kind(argument) != "MemberExpression" || flag(argument, "optional"))
                {
                    return Err(Stop::Refused);
                }
                let value = self.expr(argument)?;
                Expr::Unary { op, value }
            }
            "BinaryExpression" => {
                let op = match text(node, "operator") {
                    Some("+") => Binary::Add,
                    Some("-") => Binary::Subtract,
                    Some("*") => Binary::Multiply,
                    Some("/") => Binary::Divide,
                    Some("%") => Binary::Remainder,
                    Some("<<") => Binary::ShiftLeft,
                    Some(">>") => Binary::ShiftRight,
                    Some(">>>") => Binary::UnsignedShiftRight,
                    Some("<") => Binary::Less,
                    Some("<=") => Binary::LessEqual,
                    Some(">") => Binary::Greater,
                    Some(">=") => Binary::GreaterEqual,
                    Some("===") => Binary::StrictEqual,
                    Some("!==") => Binary::StrictNotEqual,
                    Some("==") => Binary::Equal,
                    Some("!=") => Binary::NotEqual,
                    Some("in") => Binary::In,
                    Some("instanceof") => Binary::InstanceOf,
                    Some("&") => Binary::BitAnd,
                    Some("^") => Binary::BitXor,
                    Some("|") => Binary::BitOr,
                    _ => return Err(Stop::Refused),
                };
                let left = self.expr(field(node, "left")?)?;
                let right = self.expr(field(node, "right")?)?;
                Expr::Binary { op, left, right }
            }
            "LogicalExpression" => {
                let op = match text(node, "operator") {
                    Some("&&") => Binary::And,
                    Some("||") => Binary::Or,
                    Some("??") => Binary::Nullish,
                    _ => return Err(Stop::Refused),
                };
                let left = self.expr(field(node, "left")?)?;
                let right = self.expr(field(node, "right")?)?;
                Expr::Binary { op, left, right }
            }
            "ConditionalExpression" => {
                let condition = self.expr(field(node, "test")?)?;
                let yes = self.expr(field(node, "consequent")?)?;
                let no = self.expr(field(node, "alternate")?)?;
                Expr::Conditional { condition, yes, no }
            }
            "SequenceExpression" => {
                let mut values = Vec::new();
                for value in array(node, "expressions")? {
                    values.push(self.expr(value)?);
                }
                Expr::Sequence(values)
            }
            "AssignmentExpression" => {
                let left = field(node, "left")?;
                let target = self.target(left)?;
                let op = match text(node, "operator") {
                    Some("=") => None,
                    Some("+=") => Some(Binary::Add),
                    Some("-=") => Some(Binary::Subtract),
                    Some("*=") => Some(Binary::Multiply),
                    Some("/=") => Some(Binary::Divide),
                    Some("%=") => Some(Binary::Remainder),
                    Some("<<=") => Some(Binary::ShiftLeft),
                    Some(">>=") => Some(Binary::ShiftRight),
                    Some(">>>=") => Some(Binary::UnsignedShiftRight),
                    Some("&=") => Some(Binary::BitAnd),
                    Some("^=") => Some(Binary::BitXor),
                    Some("|=") => Some(Binary::BitOr),
                    _ => return Err(Stop::Refused),
                };
                let value = match op {
                    None => self.expr(field(node, "right")?)?,
                    Some(op) => {
                        let read = self.reread(left)?;
                        let right = self.expr(field(node, "right")?)?;
                        self.expression(Expr::Binary {
                            op,
                            left: read,
                            right,
                        })?
                    }
                };
                Expr::Assign { target, value }
            }
            "MemberExpression" => {
                if flag(node, "optional") {
                    return Err(Stop::Refused);
                }
                self.member(node)?
            }
            "ChainExpression" => {
                // `o?.k` for a binding `o`, which reading twice cannot observe.
                let inner = field(node, "expression")?;
                if kind(inner) != "MemberExpression" || !flag(inner, "optional") {
                    return Err(Stop::Refused);
                }
                let object = field(inner, "object")?;
                if kind(object) != "Identifier" {
                    return Err(Stop::Refused);
                }
                let binding = self
                    .lookup(name(object)?)
                    .map(|found| found.binding)
                    .ok_or(Stop::Refused)?;
                let null_test = self.absent(binding, Literal::Null)?;
                let undefined_test = self.absent(binding, Literal::Undefined)?;
                let condition = self.expression(Expr::Binary {
                    op: Binary::Or,
                    left: null_test,
                    right: undefined_test,
                })?;
                let yes = self.expression(Expr::Literal(Literal::Undefined))?;
                let object = self.expression(Expr::Binding(binding))?;
                let property = self.property(inner)?;
                let no = self.expression(Expr::Member { object, property })?;
                Expr::Conditional { condition, yes, no }
            }
            "CallExpression" => {
                if flag(node, "optional") {
                    return Err(Stop::Refused);
                }
                let callee = field(node, "callee")?;
                if kind(callee) == "Super"
                    || (kind(callee) == "Identifier"
                        && name(callee)? == "eval"
                        && self.lookup("eval").is_none())
                {
                    return Err(Stop::Refused);
                }
                let callee = self.expr(callee)?;
                let arguments = self.arguments(node)?;
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
            "NewExpression" => {
                let callee = self.expr(field(node, "callee")?)?;
                let arguments = self.arguments(node)?;
                Expr::Construct { callee, arguments }
            }
            _ => return Err(Stop::Refused),
        };
        self.expression(expression)
    }

    /// `binding === value`.
    fn absent(&mut self, binding: BindingId, value: Literal) -> Lowered<ExprId> {
        let left = self.expression(Expr::Binding(binding))?;
        let right = self.expression(Expr::Literal(value))?;
        self.expression(Expr::Binary {
            op: Binary::StrictEqual,
            left,
            right,
        })
    }

    fn arguments(&mut self, node: &Value) -> Lowered<Vec<ExprId>> {
        let mut arguments = Vec::new();
        for argument in array(node, "arguments")? {
            if kind(argument) == "SpreadElement" {
                return Err(Stop::Refused);
            }
            arguments.push(self.expr(argument)?);
        }
        Ok(arguments)
    }

    fn member(&mut self, node: &Value) -> Lowered<Expr> {
        let object = field(node, "object")?;
        if kind(object) == "Super" {
            return Err(Stop::Refused);
        }
        let object = self.expr(object)?;
        let property = self.property(node)?;
        Ok(Expr::Member { object, property })
    }

    fn property(&mut self, node: &Value) -> Lowered<Property> {
        let property = field(node, "property")?;
        if flag(node, "computed") {
            Ok(Property::Computed(self.expr(property)?))
        } else if kind(property) == "Identifier" {
            Ok(Property::Named(name(property)?.to_string()))
        } else {
            Err(Stop::Refused)
        }
    }

    /// An assignment's target: a binding it may write, or a member.
    fn target(&mut self, node: &Value) -> Lowered<ExprId> {
        match kind(node) {
            "Identifier" => {
                let found = self.lookup(name(node)?).ok_or(Stop::Refused)?;
                if found.constant {
                    return Err(Stop::Refused);
                }
                let binding = found.binding;
                self.expression(Expr::Binding(binding))
            }
            "MemberExpression" if !flag(node, "optional") => {
                let member = self.member(node)?;
                self.expression(member)
            }
            _ => Err(Stop::Refused),
        }
    }

    /// A second read of an assignment target, where it observes nothing new:
    /// a binding, or a member of a binding or `this` under a fixed key.
    fn reread(&mut self, node: &Value) -> Lowered<ExprId> {
        if kind(node) == "Identifier" {
            return self.reference(name(node)?);
        }
        let object = field(node, "object")?;
        let fixed = !flag(node, "computed") || kind(field(node, "property")?) == "Literal";
        let simple = match kind(object) {
            "Identifier" => self.lookup(name(object)?).is_some(),
            "ThisExpression" => true,
            _ => false,
        };
        if kind(node) != "MemberExpression" || flag(node, "optional") || !fixed || !simple {
            return Err(Stop::Refused);
        }
        let member = self.member(node)?;
        self.expression(member)
    }

    /// `x++` or `x--` whose value is unused: `x=x±1`, for a binding every
    /// write keeps a number (for any other value `++` converts differently).
    fn update(&mut self, node: &Value) -> Lowered<ExprId> {
        let argument = field(node, "argument")?;
        if kind(argument) != "Identifier" {
            return Err(Stop::Refused);
        }
        let found = self.lookup(name(argument)?).ok_or(Stop::Refused)?;
        let binding = found.binding;
        if found.constant || !self.numeric.contains(&binding) {
            return Err(Stop::Refused);
        }
        let op = match text(node, "operator") {
            Some("++") => Binary::Add,
            Some("--") => Binary::Subtract,
            _ => return Err(Stop::Refused),
        };
        let target = self.expression(Expr::Binding(binding))?;
        let read = self.expression(Expr::Binding(binding))?;
        let one = self.expression(Expr::Literal(Literal::Number(1.0)))?;
        let value = self.expression(Expr::Binary {
            op,
            left: read,
            right: one,
        })?;
        self.expression(Expr::Assign { target, value })
    }
}

fn literal(node: &Value) -> Lowered<Expr> {
    if present(node, "regex") {
        let raw = text(node, "raw")
            .filter(|raw| raw.starts_with('/'))
            .ok_or(Stop::Refused)?;
        return Ok(Expr::Regex(raw.to_string()));
    }
    if present(node, "bigint") {
        return Err(Stop::Refused);
    }
    Ok(Expr::Literal(match node.get("value") {
        Some(Value::Null) => Literal::Null,
        Some(Value::Bool(value)) => Literal::Bool(*value),
        Some(Value::Number(value)) => Literal::Number(value.as_f64().ok_or(Stop::Refused)?),
        // A lone surrogate reaches the tree replaced; its text would change.
        Some(Value::String(value)) if !value.contains('\u{FFFD}') => {
            Literal::String(StringValue::from(value.as_str()))
        }
        _ => return Err(Stop::Refused),
    }))
}

fn kind(node: &Value) -> &str {
    node.get("type").and_then(Value::as_str).unwrap_or("")
}

fn text<'v>(node: &'v Value, key: &str) -> Option<&'v str> {
    node.get(key).and_then(Value::as_str)
}

fn flag(node: &Value, key: &str) -> bool {
    node.get(key).and_then(Value::as_bool) == Some(true)
}

fn present(node: &Value, key: &str) -> bool {
    node.get(key).is_some_and(|value| !value.is_null())
}

fn field<'v>(node: &'v Value, key: &str) -> Lowered<&'v Value> {
    node.get(key)
        .filter(|value| !value.is_null())
        .ok_or(Stop::Refused)
}

fn array<'v>(node: &'v Value, key: &str) -> Lowered<&'v [Value]> {
    node.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or(Stop::Refused)
}

fn name(node: &Value) -> Lowered<&str> {
    text(node, "name").ok_or(Stop::Refused)
}

/// A declaration's plain identifier; a pattern is refused.
fn identifier(node: Option<&Value>) -> Lowered<&str> {
    match node {
        Some(node) if kind(node) == "Identifier" => name(node),
        _ => Err(Stop::Refused),
    }
}

fn numeric_literal(node: Option<&Value>) -> bool {
    node.is_some_and(|node| {
        kind(node) == "Literal" && node.get("value").is_some_and(Value::is_number)
    })
}

/// Every node under `node`, depth first, while `visit` keeps returning true.
fn each(node: &Value, visit: &mut dyn FnMut(&Value) -> bool) -> bool {
    match node {
        Value::Object(fields) => {
            if fields.contains_key("type") && !visit(node) {
                return false;
            }
            fields.values().all(|value| each(value, visit))
        }
        Value::Array(items) => items.iter().all(|item| each(item, visit)),
        _ => true,
    }
}

/// Whether any identifier under `node` spells `name`.
fn mentions(node: &Value, target: &str) -> bool {
    !each(node, &mut |node| {
        !(kind(node) == "Identifier" && text(node, "name") == Some(target))
    })
}

/// Whether a function under `node` mentions `name`: it could capture a
/// per-iteration binding.
fn captures(node: &Value, target: &str) -> bool {
    !each(node, &mut |node| {
        !(matches!(
            kind(node),
            "FunctionExpression" | "ArrowFunctionExpression" | "FunctionDeclaration"
        ) && mentions(node, target))
    })
}

/// Whether every write to `name` under `nodes` keeps a number: `++`, `--`,
/// or `=`, `+=`, `-=` of a number literal. Writes to a shadowing binding of
/// the same name count too, which only refuses more.
fn numeric_writes(nodes: &[Value], target: &str) -> bool {
    let is_target = |node: &Value| kind(node) == "Identifier" && text(node, "name") == Some(target);
    nodes.iter().all(|node| {
        each(node, &mut |node| match kind(node) {
            "AssignmentExpression" => {
                let left = node.get("left");
                match left {
                    Some(left) if is_target(left) => {
                        matches!(text(node, "operator"), Some("=" | "+=" | "-="))
                            && numeric_literal(node.get("right"))
                    }
                    Some(left) => !mentions(left, target),
                    None => false,
                }
            }
            "ForInStatement" | "ForOfStatement" => {
                node.get("left").is_none_or(|left| !is_target(left))
            }
            _ => true,
        })
    })
}
