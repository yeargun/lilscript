//! Lint rules over what the compiler checked: each module's syntax, the
//! module-graph checker's results, and the program they elaborate to. Linting
//! runs the same frontend a build runs, so it refuses what a build refuses.
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::ast::{self, ArrowBody, ClassMember, Expr, ExprKind, ExternClassMember, Item, Stmt};
use crate::build::{with_checked_program, CheckedProgram, ServiceError};
use crate::config::{BundleMode, LintConfig, LintPreset, LintSeverity, ProjectConfig};
use crate::lexer::{lex, TokenKind};
use crate::module::{ModuleError, ModuleId, ModuleSet};
use crate::primitive::{Intrinsic, ResolvedIntrinsic};
use crate::check::{CheckedModules, SymbolId};
use crate::program::{
    host_call, host_receiver, AllocationKind, CallTarget, CellBinding, Operation, OperationKind,
    Place, PlaceId, Program, RegionId, UnitData, ValueId,
};
use crate::span::Span;

pub const RULES: &[&str] = &[
    "correctness/unreachable-code",
    "correctness/constant-condition",
    "correctness/unused-import",
    "correctness/unused-private-symbol",
    "correctness/unhandled-module-task",
    "effects/pure-extern-requires-allowlist",
    "performance/allocation-in-loop",
    "performance/closure-allocation-in-loop",
    "performance/indirect-call-in-loop",
    "performance/materialized-array-chain",
    "size/eager-chunk-overhead",
    "web/eager-host-access",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    Hint,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LintEdit {
    pub span: Span,
    pub replacement: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LintFix {
    pub applicability: &'static str,
    pub edits: Vec<LintEdit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LintDiagnostic {
    pub path: PathBuf,
    pub span: Span,
    pub rule: &'static str,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub evidence: Option<String>,
    pub help: Option<String>,
    pub fix: Option<LintFix>,
}

/// What a rule provider reads: the checked module graph and its program.
/// Spans are local to a module, so a provider names the module it reports in.
pub struct LintRuleContext<'context, 'ast, 'src> {
    /// Discovery's modules by id: canonical paths, sources and edges.
    pub modules: &'context ModuleSet<&'src str>,
    /// Each module's original syntax, by module id.
    pub syntax: &'context [ast::Program<'ast, 'src>],
    /// The module-graph checker's results.
    pub semantics: &'context CheckedModules<'ast, 'src>,
    /// The checked program. A unit's operation spans are local to its module.
    pub program: &'context Program<'src>,
    pub config: &'context ProjectConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintProviderDiagnostic {
    /// The module whose source `span` and the fix's edits are in.
    pub module: ModuleId,
    pub span: Span,
    pub rule: &'static str,
    pub message: String,
    pub evidence: Option<String>,
    pub help: Option<String>,
    pub fix: Option<LintFix>,
}

pub trait LintRuleProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn rules(&self) -> &'static [&'static str];
    fn check(
        &self,
        context: &LintRuleContext<'_, '_, '_>,
        diagnostics: &mut Vec<LintProviderDiagnostic>,
    );
}

/// `web/eager-host-access`: the first read or write of a host object's member,
/// or call of its method, in the entry module's initializer (its top-level
/// code, not the functions it declares). Host objects are extern class
/// instances and `JsValue`s, as the compiler classifies host calls.
#[derive(Debug, Default)]
pub struct WebRuleProvider;

impl LintRuleProvider for WebRuleProvider {
    fn id(&self) -> &'static str {
        "web"
    }

    fn rules(&self) -> &'static [&'static str] {
        &["web/eager-host-access"]
    }

    fn check(
        &self,
        context: &LintRuleContext<'_, '_, '_>,
        diagnostics: &mut Vec<LintProviderDiagnostic>,
    ) {
        let program = context.program;
        let Some(data) = program
            .modules()
            .get(program.entry_module().index())
            .and_then(|module| program.unit(module.initializer))
        else {
            return;
        };
        let Some(operation) = data
            .operations
            .iter()
            .find(|operation| host_access(program, data, operation))
        else {
            return;
        };
        diagnostics.push(LintProviderDiagnostic {
            module: data.module.index(),
            span: operation.span,
            rule: "web/eager-host-access",
            message: "top-level host access runs before progressive enhancement starts"
                .to_string(),
            evidence: Some(
                "the entry module's initializer directly touches an extern host object"
                    .to_string(),
            ),
            help: Some(
                "move host-dependent work behind an exported start function or a capability-gated event path"
                    .to_string(),
            ),
            fix: None,
        });
    }
}

/// A read or write of a host object's member, or a call of its method.
fn host_access(program: &Program<'_>, data: &UnitData, operation: &Operation) -> bool {
    match operation.kind {
        OperationKind::Load(place) | OperationKind::Store(place) => {
            host_member(program, data, place)
        }
        OperationKind::Call(call) => matches!(
            data.calls[call.index()].target,
            CallTarget::Reference { place } if host_member(program, data, place)
        ),
        _ => false,
    }
}

fn host_member(program: &Program<'_>, data: &UnitData, place: PlaceId) -> bool {
    match data.places[place.index()] {
        Place::Member { receiver, .. } | Place::Index { receiver, .. } => {
            host_receiver(program, data, receiver)
        }
        _ => false,
    }
}

static WEB_RULE_PROVIDER: WebRuleProvider = WebRuleProvider;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintError {
    pub path: PathBuf,
    pub span: Span,
    pub message: String,
}

impl std::fmt::Display for LintError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.path.display(), self.message)
    }
}

impl std::error::Error for LintError {}

impl From<ModuleError> for LintError {
    fn from(error: ModuleError) -> Self {
        Self {
            path: error.path,
            span: error.span,
            message: error.message,
        }
    }
}

#[derive(Debug)]
struct PendingDiagnostic {
    module: ModuleId,
    span: Span,
    rule: &'static str,
    message: String,
    evidence: Option<String>,
    help: Option<String>,
    fix: Option<LintFix>,
}

pub fn lint_path(path: &Path, config: &ProjectConfig) -> Result<Vec<LintDiagnostic>, LintError> {
    lint_path_with_providers(path, config, &[&WEB_RULE_PROVIDER])
}

pub fn lint_path_with_providers(
    path: &Path,
    config: &ProjectConfig,
    providers: &[&dyn LintRuleProvider],
) -> Result<Vec<LintDiagnostic>, LintError> {
    lint_graph(path, None, config, providers)
}

/// Lint `path` with `source` as its unsaved text.
pub fn lint_path_with_source(
    path: &Path,
    source: &str,
    config: &ProjectConfig,
) -> Result<Vec<LintDiagnostic>, LintError> {
    lint_graph(path, Some(source), config, &[&WEB_RULE_PROVIDER])
}

fn lint_graph(
    path: &Path,
    source: Option<&str>,
    config: &ProjectConfig,
    providers: &[&dyn LintRuleProvider],
) -> Result<Vec<LintDiagnostic>, LintError> {
    if !config.lint.enabled || path_is_excluded(path, &config.lint.exclude) {
        return Ok(Vec::new());
    }
    with_checked_program(path, source, config, |checked| {
        lint_checked_with_providers(checked, config, providers)
    })
    .map_err(|error| checking_error(path, error))?
}

/// A build's refusal, as a lint error at the refused source.
fn checking_error(path: &Path, mut error: ServiceError) -> LintError {
    match error.diagnostic.take() {
        Some(diagnostic) => diagnostic.into(),
        None => LintError {
            path: path.to_path_buf(),
            span: Span::empty(0),
            message: error.to_string(),
        },
    }
}

/// Lint a graph the caller already checked, with the default providers.
pub fn lint_checked(
    checked: &CheckedProgram<'_, '_, '_>,
    config: &ProjectConfig,
) -> Result<Vec<LintDiagnostic>, LintError> {
    lint_checked_with_providers(checked, config, &[&WEB_RULE_PROVIDER])
}

pub fn lint_checked_with_providers(
    checked: &CheckedProgram<'_, '_, '_>,
    config: &ProjectConfig,
    providers: &[&dyn LintRuleProvider],
) -> Result<Vec<LintDiagnostic>, LintError> {
    let root = &checked.modules.modules[checked.modules.root];
    if !config.lint.enabled || path_is_excluded(&root.path, &config.lint.exclude) {
        return Ok(Vec::new());
    }
    validate_rule_providers(providers).map_err(|message| LintError {
        path: root.path.clone(),
        span: Span::empty(0),
        message,
    })?;

    let mut pending = Vec::new();
    for (module, syntax) in checked.syntax.iter().enumerate() {
        let source = checked.modules.modules[module].source;
        lint_unused_imports(module, source, syntax, config, &mut pending);
        lint_bundle_policy(module, syntax, config, &mut pending);
        lint_items(module, syntax, &config.lint, &mut pending);
    }
    lint_unused_private_symbols(checked, &mut pending);
    lint_program(checked.program, &mut pending);

    let context = LintRuleContext {
        modules: checked.modules,
        syntax: checked.syntax,
        semantics: checked.semantics,
        program: checked.program,
        config,
    };
    for provider in providers {
        if !provider_enabled(&config.lint, provider.id()) {
            continue;
        }
        let mut diagnostics = Vec::new();
        provider.check(&context, &mut diagnostics);
        for diagnostic in &diagnostics {
            if let Some(message) = provider_misreport(*provider, diagnostic, checked.modules) {
                return Err(LintError {
                    path: root.path.clone(),
                    span: diagnostic.span,
                    message,
                });
            }
        }
        pending.extend(diagnostics.into_iter().map(|diagnostic| PendingDiagnostic {
            module: diagnostic.module,
            span: diagnostic.span,
            rule: diagnostic.rule,
            message: diagnostic.message,
            evidence: diagnostic.evidence,
            help: diagnostic.help,
            fix: diagnostic.fix,
        }));
    }

    Ok(finalize_diagnostics(checked.modules, &config.lint, pending))
}

/// A provider may report only its declared rules, in a module of the graph.
fn provider_misreport(
    provider: &dyn LintRuleProvider,
    diagnostic: &LintProviderDiagnostic,
    modules: &ModuleSet<&str>,
) -> Option<String> {
    if !provider.rules().contains(&diagnostic.rule) {
        return Some(format!(
            "lint provider `{}` emitted undeclared rule `{}`",
            provider.id(),
            diagnostic.rule
        ));
    }
    (diagnostic.module >= modules.modules.len()).then(|| {
        format!(
            "lint provider `{}` reported `{}` in unknown module {}",
            provider.id(),
            diagnostic.rule,
            diagnostic.module
        )
    })
}

fn lint_bundle_policy(
    module: ModuleId,
    syntax: &ast::Program<'_, '_>,
    config: &ProjectConfig,
    pending: &mut Vec<PendingDiagnostic>,
) {
    if matches!(config.bundle.mode, BundleMode::Single) {
        return;
    }
    for import in syntax.imports {
        pending.push(PendingDiagnostic {
            module,
            span: import.span,
            rule: "size/eager-chunk-overhead",
            message: "configured chunk boundary is loaded eagerly".to_string(),
            evidence: Some(format!(
                "bundle mode `{:?}` emits static ESM imports",
                config.bundle.mode
            )),
            help: Some(
                "use a single bundle when request and wrapper overhead outweigh cache reuse"
                    .to_string(),
            ),
            fix: None,
        });
    }
}

fn lint_unused_imports(
    module: ModuleId,
    source: &str,
    syntax: &ast::Program<'_, '_>,
    config: &ProjectConfig,
    pending: &mut Vec<PendingDiagnostic>,
) {
    let Ok(tokens) = lex(source) else {
        return;
    };
    let mut counts = HashMap::<&str, usize>::new();
    for token in &tokens {
        if let TokenKind::Ident(name) = token.kind {
            *counts.entry(name).or_default() += 1;
        }
    }
    for import in syntax.imports {
        for specifier in import.specifiers {
            if counts.get(specifier.local.name).copied().unwrap_or(0) == 1
                && severity_for(&config.lint, "correctness/unused-import").is_some()
            {
                pending.push(PendingDiagnostic {
                    module,
                    span: specifier.local.span,
                    rule: "correctness/unused-import",
                    message: format!("import `{}` is never used", specifier.local.name),
                    evidence: None,
                    help: Some("remove the unused import specifier".to_string()),
                    fix: None,
                });
            }
        }
    }
}

/// A declaration is private when its module does not export it; it is unused
/// when no identifier in any module resolves to it except its declaration.
fn lint_unused_private_symbols(
    checked: &CheckedProgram<'_, '_, '_>,
    pending: &mut Vec<PendingDiagnostic>,
) {
    let mut references = HashMap::<SymbolId, usize>::new();
    let mut exported = HashSet::<(ModuleId, &str)>::new();
    let mut type_declarations = HashSet::<(ModuleId, Span)>::new();
    for (module, syntax) in checked.syntax.iter().enumerate() {
        let Some(view) = checked.semantics.view(module) else {
            continue;
        };
        walk_program_idents(syntax, &mut |span| {
            if let Some(symbol) = view.identifier_symbol(span) {
                *references.entry(symbol).or_default() += 1;
            }
        });
        exported.extend(
            syntax
                .exports
                .iter()
                .map(|export| (module, export.local.name)),
        );
        type_declarations.extend(syntax.items.iter().filter_map(|item| match item {
            Item::Struct(declaration) => Some((module, declaration.name.span)),
            Item::Class(declaration) => Some((module, declaration.name.span)),
            Item::ExternClass(declaration) => Some((module, declaration.name.span)),
            _ => None,
        }));
    }
    for symbol in checked.semantics.symbols() {
        let Some(module) = checked.semantics.symbol_module(symbol.id) else {
            continue;
        };
        if !matches!(
            symbol.ty,
            crate::check::Type::Struct(_) | crate::check::Type::Class(_)
        ) && !type_declarations.contains(&(module, symbol.span))
            && references.get(&symbol.id).copied().unwrap_or(0) <= 1
            && !symbol.name.starts_with('_')
            && !exported.contains(&(module, symbol.name))
        {
            pending.push(PendingDiagnostic {
                module,
                span: symbol.span,
                rule: "correctness/unused-private-symbol",
                message: format!("private symbol `{}` is never used", symbol.name),
                evidence: None,
                help: Some(
                    "remove it or prefix the name with `_` when intentionally unused".to_string(),
                ),
                fix: None,
            });
        }
    }
}

fn lint_items(
    module: ModuleId,
    syntax: &ast::Program<'_, '_>,
    config: &LintConfig,
    pending: &mut Vec<PendingDiagnostic>,
) {
    for item in syntax.items {
        match item {
            Item::Function(function) => lint_statements(module, function.body, pending),
            Item::Extern(function) => lint_pure_extern_name(
                module,
                function.declared_pure,
                function.name.name,
                function.name.span,
                config,
                pending,
            ),
            Item::Class(class) => {
                for member in class.members {
                    match member {
                        ClassMember::Constructor(constructor) => {
                            lint_statements(module, constructor.body, pending)
                        }
                        ClassMember::Method(method) => {
                            lint_statements(module, method.body, pending)
                        }
                        ClassMember::Field(_) => {}
                    }
                }
            }
            Item::ExternClass(class) => {
                for member in class.members {
                    if let ExternClassMember::Method(method) = member {
                        lint_pure_extern_name(
                            module,
                            method.declared_pure,
                            method.name.name,
                            method.name.span,
                            config,
                            pending,
                        );
                    }
                }
            }
            Item::Stmt(statement) => {
                lint_statements(module, std::slice::from_ref(statement), pending)
            }
            _ => {}
        }
    }
}

fn lint_pure_extern_name(
    module: ModuleId,
    declared_pure: bool,
    name: &str,
    span: Span,
    config: &LintConfig,
    pending: &mut Vec<PendingDiagnostic>,
) {
    if declared_pure
        && !config
            .pure_extern_allowlist
            .iter()
            .any(|allowed| allowed == name)
    {
        pending.push(PendingDiagnostic {
            module,
            span,
            rule: "effects/pure-extern-requires-allowlist",
            message: format!("trusted pure extern `{name}` is not allowlisted"),
            evidence: Some("extern purity cannot be verified from LilScript code".to_string()),
            help: Some(
                "audit the host implementation, then add its name to `lint.pure_extern_allowlist`"
                    .to_string(),
            ),
            fix: None,
        });
    }
}

fn lint_statements(
    module: ModuleId,
    statements: &[Stmt<'_, '_>],
    pending: &mut Vec<PendingDiagnostic>,
) {
    let mut terminated = false;
    for statement in statements {
        if terminated {
            pending.push(PendingDiagnostic {
                module,
                span: statement.span(),
                rule: "correctness/unreachable-code",
                message: "statement is unreachable".to_string(),
                evidence: Some(
                    "a preceding statement always exits this control-flow path".to_string(),
                ),
                help: Some("remove the statement or change the preceding control flow".to_string()),
                fix: removable_statement_span(statement).map(|span| LintFix {
                    applicability: "machine-applicable",
                    edits: vec![LintEdit {
                        span,
                        replacement: String::new(),
                    }],
                }),
            });
        }
        lint_statement(module, statement, pending);
        terminated |= statement_terminates(statement);
    }
}

fn removable_statement_span(statement: &Stmt<'_, '_>) -> Option<Span> {
    match statement {
        // Expression spans stop before their mandatory statement terminator.
        Stmt::Expr(_) => {
            let span = statement.span();
            Some(Span::new(span.start, span.end + 1))
        }
        _ => None,
    }
}

fn lint_statement(
    module: ModuleId,
    statement: &Stmt<'_, '_>,
    pending: &mut Vec<PendingDiagnostic>,
) {
    match statement {
        Stmt::Expr(expression)
            if contains_dynamic_import(expression) && !task_chain_has_catch(expression) =>
        {
            pending.push(PendingDiagnostic {
                module,
                span: expression.span(),
                rule: "correctness/unhandled-module-task",
                message: "dynamic module task has no failure handler".to_string(),
                evidence: Some("the expression starts a runtime chunk request".to_string()),
                help: Some(
                    "terminate the task chain with `.catch((auto error) => ...)`".to_string(),
                ),
                fix: None,
            });
        }
        Stmt::Block { body, .. } => lint_statements(module, body, pending),
        Stmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            lint_constant_condition(module, condition, pending);
            lint_statement(module, then_branch, pending);
            if let Some(branch) = else_branch {
                lint_statement(module, branch, pending);
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            lint_constant_condition(module, condition, pending);
            lint_statement(module, body, pending);
        }
        Stmt::For {
            condition, body, ..
        } => {
            if let Some(condition) = condition {
                lint_constant_condition(module, condition, pending);
            }
            lint_statement(module, body, pending);
        }
        _ => {}
    }
}

fn task_chain_has_catch(expression: &Expr<'_, '_>) -> bool {
    match expression {
        Expr {
            kind: ExprKind::Call { callee, .. },
            ..
        } => match callee {
            Expr {
                kind: ExprKind::Member {
                    object, property, ..
                },
                ..
            } => property.name == "catch" || task_chain_has_catch(object),
            _ => task_chain_has_catch(callee),
        },
        Expr {
            kind: ExprKind::Member { object, .. },
            ..
        }
        | Expr {
            kind: ExprKind::OptionalMember { object, .. },
            ..
        } => task_chain_has_catch(object),
        _ => false,
    }
}

fn contains_dynamic_import(expression: &Expr<'_, '_>) -> bool {
    match expression {
        Expr {
            kind: ExprKind::DynamicImport { .. },
            ..
        } => true,
        Expr {
            kind: ExprKind::ArrayLiteral { elements, .. },
            ..
        } => elements
            .iter()
            .any(|element| contains_dynamic_import(element.value())),
        Expr {
            kind: ExprKind::RecordLiteral { entries, .. },
            ..
        }
        | Expr {
            kind: ExprKind::ObjectLiteral { entries, .. },
            ..
        } => entries
            .iter()
            .any(|entry| contains_dynamic_import(entry.value())),
        Expr {
            kind: ExprKind::StructLiteral { values, .. },
            ..
        } => values.iter().any(contains_dynamic_import),
        Expr {
            kind: ExprKind::New { args, .. },
            ..
        } => args
            .iter()
            .any(|argument| contains_dynamic_import(&argument.expression)),
        Expr {
            kind: ExprKind::Member { object, .. },
            ..
        }
        | Expr {
            kind: ExprKind::OptionalMember { object, .. },
            ..
        }
        | Expr {
            kind: ExprKind::Unary { expr: object, .. },
            ..
        }
        | Expr {
            kind: ExprKind::Await { task: object, .. },
            ..
        }
        | Expr {
            kind: ExprKind::TypeCheck { value: object, .. },
            ..
        }
        | Expr {
            kind: ExprKind::Update { target: object, .. },
            ..
        } => contains_dynamic_import(object),
        Expr {
            kind: ExprKind::Call { callee, args, .. },
            ..
        } => {
            contains_dynamic_import(callee)
                || args
                    .iter()
                    .any(|argument| contains_dynamic_import(&argument.expression))
        }
        Expr {
            kind: ExprKind::ArrowFunction { body, .. },
            ..
        } => match body {
            ArrowBody::Expr(expression) => contains_dynamic_import(expression),
            ArrowBody::Block(_) => false,
        },
        Expr {
            kind: ExprKind::Binary { lhs, rhs, .. },
            ..
        }
        | Expr {
            kind:
                ExprKind::Index {
                    object: lhs,
                    index: rhs,
                    ..
                },
            ..
        }
        | Expr {
            kind:
                ExprKind::OptionalIndex {
                    object: lhs,
                    index: rhs,
                    ..
                },
            ..
        }
        | Expr {
            kind:
                ExprKind::Assignment {
                    target: lhs,
                    value: rhs,
                    ..
                },
            ..
        } => contains_dynamic_import(lhs) || contains_dynamic_import(rhs),
        Expr {
            kind: ExprKind::Template { parts, .. },
            ..
        } => parts.iter().any(|part| match part {
            crate::ast::TemplatePart::Expr(expression) => contains_dynamic_import(expression),
            crate::ast::TemplatePart::String(..) => false,
        }),
        Expr {
            kind: ExprKind::Match { value, arms, .. },
            ..
        } => {
            contains_dynamic_import(value)
                || arms.iter().any(|arm| contains_dynamic_import(&arm.value))
        }
        Expr {
            kind:
                ExprKind::If {
                    condition,
                    then_value,
                    else_value,
                    ..
                },
            ..
        } => {
            contains_dynamic_import(condition)
                || contains_dynamic_import(then_value)
                || contains_dynamic_import(else_value)
        }
        Expr {
            kind: ExprKind::Int(..),
            ..
        }
        | Expr {
            kind: ExprKind::Float(..),
            ..
        }
        | Expr {
            kind: ExprKind::String(..),
            ..
        }
        | Expr {
            kind: ExprKind::Bool(..),
            ..
        }
        | Expr {
            kind: ExprKind::Null(..),
            ..
        }
        | Expr {
            kind: ExprKind::Ident(..),
            ..
        } => false,
    }
}

fn lint_constant_condition(
    module: ModuleId,
    condition: &Expr<'_, '_>,
    pending: &mut Vec<PendingDiagnostic>,
) {
    if let Expr {
        kind: ExprKind::Bool(value, span),
        ..
    } = condition
    {
        pending.push(PendingDiagnostic {
            module,
            span: *span,
            rule: "correctness/constant-condition",
            message: format!("condition is always `{value}`"),
            evidence: Some("the condition is a boolean literal".to_string()),
            help: Some("remove the dead branch or make the condition data-dependent".to_string()),
            fix: None,
        });
    }
}

fn statement_terminates(statement: &Stmt<'_, '_>) -> bool {
    match statement {
        Stmt::Return { .. } | Stmt::Throw { .. } | Stmt::Break(_) | Stmt::Continue(_) => true,
        Stmt::Block { body, .. } => body.last().is_some_and(statement_terminates),
        Stmt::If {
            then_branch,
            else_branch: Some(else_branch),
            ..
        } => statement_terminates(then_branch) && statement_terminates(else_branch),
        Stmt::Try {
            body,
            catch,
            finally,
            ..
        } => {
            finally.is_some_and(|body| body.last().is_some_and(statement_terminates))
                || (body.last().is_some_and(statement_terminates)
                    && catch
                        .as_ref()
                        .is_none_or(|clause| clause.body.last().is_some_and(statement_terminates)))
        }
        _ => false,
    }
}

/// Rules over the checked program: every unit, every region, with loop
/// nesting taken from the program's own loop operations (the test, body and
/// update of a counted loop; the body of `for...in` and `for...of`).
///
/// These rules see the program as checked, before any program rule runs; the
/// old rules saw an optimized IR. They move after the program rules when
/// those land (plan M5). Until then each rule reports what the source asks
/// for, not what survives optimization:
/// - `performance/allocation-in-loop`: every array, struct, class instance,
///   `Map`, `Set`, buffer, typed-array view, `Symbol` or map/filter/concat
///   result allocated inside a loop.
/// - `performance/closure-allocation-in-loop`: every closure created inside a
///   loop.
/// - `performance/indirect-call-in-loop`: every call inside a loop through a
///   value that is neither a closure created in place nor a function
///   declaration's own binding, or through a member of a program-owned object.
///   Calls of host functions and host methods are not indirect.
/// - `performance/materialized-array-chain`: every map or filter whose
///   receiver is a map or filter result, inside a loop or not.
fn lint_program(program: &Program<'_>, pending: &mut Vec<PendingDiagnostic>) {
    for unit in program.units() {
        let data = unit.data();
        lint_region(program, data, data.entry, false, pending);
    }
}

fn lint_region(
    program: &Program<'_>,
    data: &UnitData,
    region: RegionId,
    in_loop: bool,
    pending: &mut Vec<PendingDiagnostic>,
) {
    let Some(region) = data.regions.get(region.index()) else {
        return;
    };
    for &operation in &region.operations {
        let operation = &data.operations[operation.index()];
        if in_loop {
            lint_loop_operation(program, data, operation, pending);
        }
        lint_array_chain(data, operation, pending);
        let iterates = matches!(
            operation.kind,
            OperationKind::Loop { .. } | OperationKind::ForIn { .. } | OperationKind::ForOf { .. }
        );
        for child in operation.kind.child_regions() {
            lint_region(program, data, child, in_loop || iterates, pending);
        }
    }
}

fn lint_loop_operation(
    program: &Program<'_>,
    data: &UnitData,
    operation: &Operation,
    pending: &mut Vec<PendingDiagnostic>,
) {
    let module = data.module.index();
    let allocation = |kind: &str, rule| allocation_diagnostic(module, operation.span, kind, rule);
    match &operation.kind {
        OperationKind::Allocate {
            kind: AllocationKind::Array | AllocationKind::SpreadArray(_),
            ..
        } => pending.push(allocation("array", "performance/allocation-in-loop")),
        OperationKind::Allocate {
            kind: AllocationKind::Struct(_),
            ..
        }
        | OperationKind::ConstructClass => {
            pending.push(allocation("aggregate", "performance/allocation-in-loop"))
        }
        OperationKind::Allocate {
            kind: AllocationKind::Object(_),
            ..
        } if operation
            .result
            .is_some_and(|result| class_instance(program, data, result)) =>
        {
            pending.push(allocation("aggregate", "performance/allocation-in-loop"))
        }
        OperationKind::Intrinsic(operation_kind) => {
            if let Some(kind) = intrinsic_allocation_kind(*operation_kind) {
                pending.push(allocation(kind, "performance/allocation-in-loop"));
            }
        }
        OperationKind::Closure(_) => pending.push(allocation(
            "closure",
            "performance/closure-allocation-in-loop",
        )),
        OperationKind::Call(call) => {
            let target = &data.calls[call.index()].target;
            if let CallTarget::Intrinsic {
                operation: kind, ..
            } = target
            {
                if let Some(kind) = intrinsic_allocation_kind(*kind) {
                    pending.push(allocation(kind, "performance/allocation-in-loop"));
                }
            } else if indirect_call(program, data, target) {
                pending.push(PendingDiagnostic {
                    module,
                    span: operation.span,
                    rule: "performance/indirect-call-in-loop",
                    message: "indirect function call remains inside a loop".to_string(),
                    evidence: Some(
                        "the checked program does not resolve this call to a known function"
                            .to_string(),
                    ),
                    help: Some(
                        "keep the call site monomorphic or pass a statically known function when this loop is hot"
                            .to_string(),
                    ),
                    fix: None,
                });
            }
        }
        _ => {}
    }
}

/// A map or filter whose receiver is itself a map or filter result.
fn lint_array_chain(data: &UnitData, operation: &Operation, pending: &mut Vec<PendingDiagnostic>) {
    let OperationKind::Call(call) = operation.kind else {
        return;
    };
    let CallTarget::Intrinsic {
        operation: kind,
        receiver: Some(receiver),
    } = data.calls[call.index()].target
    else {
        return;
    };
    if !array_pipeline_stage(kind) || !produced_by_pipeline_stage(data, receiver) {
        return;
    }
    pending.push(PendingDiagnostic {
        module: data.module.index(),
        span: operation.span,
        rule: "performance/materialized-array-chain",
        message: "array pipeline remains materialized after optimization".to_string(),
        evidence: Some(
            "a map/filter result is the receiver of another map/filter, so the intermediate array is built"
                .to_string(),
        ),
        help: Some("inspect callback side effects or escaping intermediate values".to_string()),
        fix: None,
    });
}

fn array_pipeline_stage(operation: ResolvedIntrinsic) -> bool {
    matches!(
        operation,
        ResolvedIntrinsic::Method(Intrinsic::ArrayMap | Intrinsic::ArrayFilter)
    )
}

fn produced_by_pipeline_stage(data: &UnitData, value: ValueId) -> bool {
    let definition = &data.operations[data.values[value.index()].definition.index()];
    matches!(
        definition.kind,
        OperationKind::Call(call) if matches!(
            data.calls[call.index()].target,
            CallTarget::Intrinsic { operation, .. } if array_pipeline_stage(operation)
        )
    )
}

/// A class construction's instance, as opposed to an object literal.
fn class_instance(program: &Program<'_>, data: &UnitData, value: ValueId) -> bool {
    matches!(
        program.ty(data.values[value.index()].ty),
        Some(crate::check::Type::Class(_) | crate::check::Type::ClassInstance { .. })
    )
}

/// A call through a value the program does not name as one function, or
/// through a member of a program-owned object. Host functions and host
/// methods are the host's calls, not indirect ones.
fn indirect_call(program: &Program<'_>, data: &UnitData, target: &CallTarget) -> bool {
    match *target {
        CallTarget::Value { callee, .. } => {
            !host_call(program, data, target) && !known_callee(program, data, callee)
        }
        CallTarget::Reference { place } => !host_member(program, data, place),
        CallTarget::Builtin(_) | CallTarget::Intrinsic { .. } => false,
    }
}

/// A closure created in place, or a function declaration's own binding.
fn known_callee(program: &Program<'_>, data: &UnitData, callee: ValueId) -> bool {
    let definition = &data.operations[data.values[callee.index()].definition.index()];
    match definition.kind {
        OperationKind::Closure(_) => true,
        OperationKind::Load(place) => match data.places[place.index()] {
            Place::Cell(cell) => program.cells().get(cell.index()).is_some_and(|cell| {
                matches!(cell.binding, CellBinding::Function(_)) && !cell.assigned
            }),
            _ => false,
        },
        _ => false,
    }
}

fn intrinsic_allocation_kind(operation: ResolvedIntrinsic) -> Option<&'static str> {
    let (ResolvedIntrinsic::Property(intrinsic)
    | ResolvedIntrinsic::Method(intrinsic)
    | ResolvedIntrinsic::Constructor(intrinsic)) = operation;
    match intrinsic {
        Intrinsic::ArrayMap | Intrinsic::ArrayFilter | Intrinsic::ArrayConcat => {
            Some("array result")
        }
        Intrinsic::MapNew => Some("map"),
        Intrinsic::SetNew => Some("set"),
        Intrinsic::ArrayBufferNew | Intrinsic::SharedArrayBufferNew | Intrinsic::BufferSlice => {
            Some("buffer")
        }
        other
            if matches!(
                crate::typed_array::classify_typed_array_intrinsic(other),
                Some((
                    _,
                    crate::typed_array::TypedArrayIntrinsic::New
                        | crate::typed_array::TypedArrayIntrinsic::Slice
                        | crate::typed_array::TypedArrayIntrinsic::Subarray
                ))
            ) =>
        {
            Some("typed array view")
        }
        Intrinsic::SymbolNew => Some("symbol"),
        _ => None,
    }
}

fn allocation_diagnostic(
    module: ModuleId,
    span: Span,
    kind: &str,
    rule: &'static str,
) -> PendingDiagnostic {
    PendingDiagnostic {
        module,
        span,
        rule,
        message: format!("surviving {kind} allocation executes inside a loop"),
        evidence: Some("the checked program allocates here on every iteration".to_string()),
        help: Some(
            "hoist, reuse, or prevent the value from escaping when identity is not required"
                .to_string(),
        ),
        fix: None,
    }
}

fn finalize_diagnostics(
    modules: &ModuleSet<&str>,
    config: &LintConfig,
    pending: Vec<PendingDiagnostic>,
) -> Vec<LintDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen = HashSet::new();
    for item in pending {
        let provider = item
            .rule
            .split_once('/')
            .map_or(item.rule, |(provider, _)| provider);
        if !provider_enabled(config, provider) {
            continue;
        }
        let Some(severity) = severity_for(config, item.rule) else {
            continue;
        };
        let Some(module) = modules.modules.get(item.module) else {
            continue;
        };
        if path_is_excluded(&module.path, &config.exclude)
            || is_suppressed(module.source, item.span, item.rule)
            || !seen.insert((item.module, item.span, item.rule))
        {
            continue;
        }
        diagnostics.push(LintDiagnostic {
            path: module.path.clone(),
            span: item.span,
            rule: item.rule,
            severity,
            message: item.message,
            evidence: item.evidence,
            help: item.help,
            fix: item.fix,
        });
    }
    diagnostics.sort_by(|left, right| {
        (&left.path, left.span.start, left.rule).cmp(&(&right.path, right.span.start, right.rule))
    });
    diagnostics
}

fn severity_for(config: &LintConfig, rule: &str) -> Option<DiagnosticSeverity> {
    let configured = config
        .rules
        .get(rule)
        .copied()
        .unwrap_or_else(|| match config.preset {
            LintPreset::Minimal => {
                if rule.starts_with("correctness/") {
                    LintSeverity::Error
                } else {
                    LintSeverity::Off
                }
            }
            LintPreset::Recommended => {
                if rule.starts_with("correctness/") {
                    LintSeverity::Error
                } else if rule.starts_with("size/") || rule.starts_with("web/") {
                    LintSeverity::Hint
                } else {
                    LintSeverity::Warn
                }
            }
            LintPreset::Strict => {
                if rule.starts_with("correctness/") || rule.starts_with("effects/") {
                    LintSeverity::Error
                } else {
                    LintSeverity::Warn
                }
            }
        });
    match configured {
        LintSeverity::Off => None,
        LintSeverity::Hint => Some(DiagnosticSeverity::Hint),
        LintSeverity::Warn => Some(if config.deny_warnings {
            DiagnosticSeverity::Error
        } else {
            DiagnosticSeverity::Warning
        }),
        LintSeverity::Error => Some(DiagnosticSeverity::Error),
    }
}

fn provider_enabled(config: &LintConfig, provider: &str) -> bool {
    config
        .providers
        .as_ref()
        .is_none_or(|providers| providers.iter().any(|enabled| enabled == provider))
}

fn validate_rule_providers(providers: &[&dyn LintRuleProvider]) -> Result<(), String> {
    let mut provider_ids = HashSet::new();
    let mut rules = HashSet::new();
    for provider in providers {
        let id = provider.id();
        if id.is_empty() || id.contains('/') {
            return Err(format!(
                "lint provider `{id}` must use a nonempty namespace identifier without `/`"
            ));
        }
        if !provider_ids.insert(id) {
            return Err(format!("duplicate lint provider `{id}`"));
        }
        for rule in provider.rules() {
            if !rule.starts_with(&format!("{id}/")) {
                return Err(format!(
                    "lint provider `{id}` owns rule `{rule}` outside its namespace"
                ));
            }
            if !rules.insert(*rule) {
                return Err(format!("duplicate pluggable lint rule `{rule}`"));
            }
        }
    }
    Ok(())
}

fn is_suppressed(source: &str, span: Span, rule: &str) -> bool {
    let line_index = source[..span.start.min(source.len())]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count();
    let lines = source.lines().collect::<Vec<_>>();
    let file_marker = format!("lilscript-lint-disable {rule}");
    if lines
        .iter()
        .take(line_index + 1)
        .any(|line| line.contains(&file_marker))
    {
        return true;
    }
    let next_marker = format!("lilscript-lint-disable-next-line {rule}");
    line_index > 0
        && lines
            .get(line_index - 1)
            .is_some_and(|line| line.contains(&next_marker))
}

fn path_is_excluded(path: &Path, patterns: &[String]) -> bool {
    let value = path.to_string_lossy().replace('\\', "/");
    patterns
        .iter()
        .any(|pattern| wildcard_match(pattern, &value))
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    let parts = pattern.split('*').collect::<Vec<_>>();
    if parts.len() == 1 {
        return value.ends_with(pattern) || value == pattern;
    }
    let mut cursor = 0;
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        let Some(found) = value[cursor..].find(part) else {
            return false;
        };
        if index == 0 && !pattern.starts_with('*') && found != 0 {
            return false;
        }
        cursor += found + part.len();
    }
    pattern.ends_with('*') || parts.last().is_some_and(|part| value.ends_with(part))
}

fn walk_program_idents(program: &ast::Program<'_, '_>, visitor: &mut impl FnMut(Span)) {
    for import in program.imports {
        for specifier in import.specifiers {
            visitor(specifier.imported.span);
            visitor(specifier.local.span);
        }
    }
    for import in program.foreign_imports {
        for specifier in import.specifiers {
            visitor(specifier.imported.span);
            visitor(specifier.local.span);
        }
    }
    for export in program.exports {
        visitor(export.local.span);
        visitor(export.exported.span);
    }
    for item in program.items {
        walk_item_idents(item, visitor);
    }
}

fn walk_item_idents(item: &Item<'_, '_>, visitor: &mut impl FnMut(Span)) {
    match item {
        Item::Enum(declaration) => {
            visitor(declaration.name.span);
            for variant in declaration.variants {
                visitor(variant.span);
            }
        }
        Item::Function(function) => {
            visitor(function.name.span);
            for parameter in function.params {
                visitor(parameter.name.span);
                if let Some(default) = &parameter.default {
                    walk_expr_idents(default, visitor);
                }
            }
            walk_statements_idents(function.body, visitor);
        }
        Item::Extern(function) => {
            visitor(function.name.span);
            for parameter in function.params {
                visitor(parameter.name.span);
            }
        }
        Item::ExternGlobal(global) => visitor(global.name.span),
        Item::Struct(structure) => visitor(structure.name.span),
        Item::Class(class) => {
            visitor(class.name.span);
            for member in class.members {
                match member {
                    ClassMember::Field(field) => visitor(field.name.span),
                    ClassMember::Constructor(constructor) => {
                        for parameter in constructor.params {
                            visitor(parameter.name.span);
                        }
                        walk_statements_idents(constructor.body, visitor);
                    }
                    ClassMember::Method(method) => {
                        visitor(method.name.span);
                        for parameter in method.params {
                            visitor(parameter.name.span);
                        }
                        walk_statements_idents(method.body, visitor);
                    }
                }
            }
        }
        Item::ExternClass(class) => visitor(class.name.span),
        Item::Stmt(statement) => walk_statement_idents(statement, visitor),
    }
}

fn walk_statements_idents(statements: &[Stmt<'_, '_>], visitor: &mut impl FnMut(Span)) {
    for statement in statements {
        walk_statement_idents(statement, visitor);
    }
}

fn walk_statement_idents(statement: &Stmt<'_, '_>, visitor: &mut impl FnMut(Span)) {
    match statement {
        Stmt::VarDecl(declaration) => {
            visitor(declaration.name.span);
            if let Some(initializer) = &declaration.initializer {
                walk_expr_idents(initializer, visitor);
            }
        }
        Stmt::ArrayDestructure {
            bindings, value, ..
        } => {
            for binding in *bindings {
                match binding {
                    crate::ast::ArrayBinding::Hole(_) => {}
                    crate::ast::ArrayBinding::Name(name) | crate::ast::ArrayBinding::Rest(name) => {
                        visitor(name.span)
                    }
                }
            }
            walk_expr_idents(value, visitor);
        }
        Stmt::RecordDestructure {
            bindings,
            rest,
            value,
            ..
        } => {
            for binding in *bindings {
                visitor(binding.name.span);
            }
            if let Some(rest) = rest {
                visitor(rest.span);
            }
            walk_expr_idents(value, visitor);
        }
        Stmt::Expr(expression) => walk_expr_idents(expression, visitor),
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                walk_expr_idents(value, visitor);
            }
        }
        Stmt::Throw { value, .. } => walk_expr_idents(value, visitor),
        Stmt::SuperCall { args, .. } => {
            for argument in *args {
                walk_expr_idents(&argument.expression, visitor);
            }
        }
        Stmt::Yield { value, .. } => walk_expr_idents(value, visitor),
        Stmt::Try {
            body,
            catch,
            finally,
            ..
        } => {
            walk_statements_idents(body, visitor);
            if let Some(clause) = catch {
                if let Some(binding) = clause.binding {
                    visitor(binding.name.span);
                }
                walk_statements_idents(clause.body, visitor);
            }
            if let Some(body) = finally {
                walk_statements_idents(body, visitor);
            }
        }
        Stmt::Block { body, .. } => walk_statements_idents(body, visitor),
        Stmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            walk_expr_idents(condition, visitor);
            walk_statement_idents(then_branch, visitor);
            if let Some(branch) = else_branch {
                walk_statement_idents(branch, visitor);
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            walk_expr_idents(condition, visitor);
            walk_statement_idents(body, visitor);
        }
        Stmt::For {
            initializer,
            condition,
            update,
            body,
            ..
        } => {
            if let Some(initializer) = initializer {
                match initializer {
                    crate::ast::ForInitializer::VarDecl(declaration) => {
                        visitor(declaration.name.span);
                        if let Some(value) = &declaration.initializer {
                            walk_expr_idents(value, visitor);
                        }
                    }
                    crate::ast::ForInitializer::Expr(expression) => {
                        walk_expr_idents(expression, visitor)
                    }
                }
            }
            if let Some(condition) = condition {
                walk_expr_idents(condition, visitor);
            }
            if let Some(update) = update {
                walk_expr_idents(update, visitor);
            }
            walk_statement_idents(body, visitor);
        }
        Stmt::ForIn {
            key, object, body, ..
        } => {
            visitor(key.span);
            walk_expr_idents(object, visitor);
            walk_statement_idents(body, visitor);
        }
        Stmt::ForOf {
            element,
            iterable,
            body,
            ..
        } => {
            visitor(element.span);
            walk_expr_idents(iterable, visitor);
            walk_statement_idents(body, visitor);
        }
        Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}

fn walk_expr_idents(expression: &Expr<'_, '_>, visitor: &mut impl FnMut(Span)) {
    match expression {
        Expr {
            kind: ExprKind::Ident(identifier),
            ..
        } => visitor(identifier.span),
        Expr {
            kind: ExprKind::ArrayLiteral { elements, .. },
            ..
        } => {
            for element in *elements {
                walk_expr_idents(element.value(), visitor);
            }
        }
        Expr {
            kind: ExprKind::RecordLiteral { entries, .. },
            ..
        }
        | Expr {
            kind: ExprKind::ObjectLiteral { entries, .. },
            ..
        } => {
            for entry in *entries {
                if let crate::ast::RecordElement::Entry(entry) = entry {
                    visitor(entry.key.span);
                }
                walk_expr_idents(entry.value(), visitor);
            }
        }
        Expr {
            kind: ExprKind::StructLiteral { name, values, .. },
            ..
        } => {
            visitor(name.span);
            for value in *values {
                walk_expr_idents(value, visitor);
            }
        }
        Expr {
            kind: ExprKind::New { class, args, .. },
            ..
        } => {
            visitor(class.span);
            for argument in *args {
                walk_expr_idents(&argument.expression, visitor);
            }
        }
        Expr {
            kind: ExprKind::Member { object, .. },
            ..
        }
        | Expr {
            kind: ExprKind::OptionalMember { object, .. },
            ..
        } => walk_expr_idents(object, visitor),
        Expr {
            kind: ExprKind::Call { callee, args, .. },
            ..
        } => {
            walk_expr_idents(callee, visitor);
            for argument in *args {
                walk_expr_idents(&argument.expression, visitor);
            }
        }
        Expr {
            kind: ExprKind::ArrowFunction { params, body, .. },
            ..
        } => {
            for parameter in *params {
                visitor(parameter.name.span);
            }
            match body {
                ArrowBody::Expr(expression) => walk_expr_idents(expression, visitor),
                ArrowBody::Block(body) => walk_statements_idents(body, visitor),
            }
        }
        Expr {
            kind: ExprKind::Unary { expr, .. },
            ..
        } => walk_expr_idents(expr, visitor),
        Expr {
            kind: ExprKind::Await { task, .. },
            ..
        } => walk_expr_idents(task, visitor),
        Expr {
            kind: ExprKind::Binary { lhs, rhs, .. },
            ..
        } => {
            walk_expr_idents(lhs, visitor);
            walk_expr_idents(rhs, visitor);
        }
        Expr {
            kind: ExprKind::TypeCheck { value, .. },
            ..
        } => walk_expr_idents(value, visitor),
        Expr {
            kind: ExprKind::Index { object, index, .. },
            ..
        }
        | Expr {
            kind: ExprKind::OptionalIndex { object, index, .. },
            ..
        } => {
            walk_expr_idents(object, visitor);
            walk_expr_idents(index, visitor);
        }
        Expr {
            kind: ExprKind::Assignment { target, value, .. },
            ..
        } => {
            walk_expr_idents(target, visitor);
            walk_expr_idents(value, visitor);
        }
        Expr {
            kind: ExprKind::Update { target, .. },
            ..
        } => walk_expr_idents(target, visitor),
        Expr {
            kind: ExprKind::Template { parts, .. },
            ..
        } => {
            for part in *parts {
                if let crate::ast::TemplatePart::Expr(expression) = part {
                    walk_expr_idents(expression, visitor);
                }
            }
        }
        Expr {
            kind: ExprKind::Match { value, arms, .. },
            ..
        } => {
            walk_expr_idents(value, visitor);
            for arm in *arms {
                if let crate::ast::MatchPattern::EnumVariant {
                    enum_name, variant, ..
                } = arm.pattern
                {
                    visitor(enum_name.span);
                    visitor(variant.span);
                }
                walk_expr_idents(&arm.value, visitor);
            }
        }
        Expr {
            kind:
                ExprKind::If {
                    condition,
                    then_value,
                    else_value,
                    ..
                },
            ..
        } => {
            walk_expr_idents(condition, visitor);
            walk_expr_idents(then_value, visitor);
            walk_expr_idents(else_value, visitor);
        }
        Expr {
            kind: ExprKind::Int(..),
            ..
        }
        | Expr {
            kind: ExprKind::Float(..),
            ..
        }
        | Expr {
            kind: ExprKind::String(..),
            ..
        }
        | Expr {
            kind: ExprKind::Bool(..),
            ..
        }
        | Expr {
            kind: ExprKind::Null(..),
            ..
        }
        | Expr {
            kind: ExprKind::DynamicImport { .. },
            ..
        } => {}
    }
}

pub fn rule_defaults() -> BTreeMap<&'static str, DiagnosticSeverity> {
    RULES
        .iter()
        .map(|rule| {
            let severity = if rule.starts_with("correctness/") {
                DiagnosticSeverity::Error
            } else if rule.starts_with("size/") {
                DiagnosticSeverity::Hint
            } else {
                DiagnosticSeverity::Warning
            };
            (*rule, severity)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_source;
    use bumpalo::Bump;

    struct AgentRuleProvider;

    impl LintRuleProvider for AgentRuleProvider {
        fn id(&self) -> &'static str {
            "agent"
        }

        fn rules(&self) -> &'static [&'static str] {
            &["agent/entry-budget"]
        }

        fn check(
            &self,
            context: &LintRuleContext<'_, '_, '_>,
            diagnostics: &mut Vec<LintProviderDiagnostic>,
        ) {
            let program = context.program;
            let initializer = program.modules()[program.entry_module().index()].initializer;
            let data = program.unit(initializer).unwrap();
            diagnostics.push(LintProviderDiagnostic {
                module: data.module.index(),
                span: context.syntax[context.modules.root].span,
                rule: "agent/entry-budget",
                message: "agent policy checked the entry initializer".to_string(),
                evidence: Some(format!("{} operations", data.operations.len())),
                help: None,
                fix: None,
            });
        }
    }

    /// One scratch directory per test, removed when the test ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let directory =
                std::env::temp_dir().join(format!("lilscript-lint-{name}-{}", std::process::id()));
            std::fs::create_dir_all(&directory).unwrap();
            Self(directory)
        }

        fn file(&self, name: &str, source: &str) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, source).unwrap();
            path
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn lint(name: &str, source: &str) -> Vec<LintDiagnostic> {
        let scratch = Scratch::new(name);
        let path = scratch.file("main.lil", source);
        lint_path(&path, &ProjectConfig::default()).unwrap()
    }

    fn rule_messages<'a>(diagnostics: &'a [LintDiagnostic], rule: &str) -> Vec<&'a str> {
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.rule == rule)
            .map(|diagnostic| diagnostic.message.as_str())
            .collect()
    }

    #[test]
    fn wildcard_exclusions_and_suppressions_are_deterministic() {
        assert!(wildcard_match(
            "**/generated/*.lil",
            "/tmp/generated/app.lil"
        ));
        let source =
            "// lilscript-lint-disable-next-line correctness/constant-condition\nif(true){}";
        assert!(is_suppressed(
            source,
            Span::new(
                source.find("true").unwrap(),
                source.find("true").unwrap() + 4
            ),
            "correctness/constant-condition"
        ));
    }

    #[test]
    fn rule_inventory_has_stable_unique_ids() {
        let unique = RULES.iter().copied().collect::<HashSet<_>>();
        assert_eq!(unique.len(), RULES.len());
        assert!(RULES.iter().all(|rule| rule.contains('/')));
        assert_eq!(rule_defaults().len(), RULES.len());
    }

    #[test]
    fn aggregate_escape_waits_for_the_escape_fact() {
        assert!(!RULES.contains(&"performance/aggregate-escape"));
        assert!(!rule_defaults().contains_key("performance/aggregate-escape"));
    }

    #[test]
    fn unreachable_code_has_a_machine_applicable_fix() {
        let scratch = Scratch::new("fix");
        let path = scratch.file(
            "main.lil",
            "int example(){return 1;print(2);}print(example());",
        );

        let diagnostics = lint_path(&path, &ProjectConfig::default()).unwrap();
        let unreachable = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.rule == "correctness/unreachable-code")
            .unwrap();
        let fix = unreachable.fix.as_ref().unwrap();
        assert_eq!(fix.applicability, "machine-applicable");
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(
            &std::fs::read_to_string(&path).unwrap()
                [fix.edits[0].span.start..fix.edits[0].span.end],
            "print(2);"
        );
    }

    #[test]
    fn lint_presets_and_static_chunk_policy_are_effective() {
        let mut config = ProjectConfig::default();
        config.lint.preset = LintPreset::Minimal;
        assert_eq!(
            severity_for(&config.lint, "performance/allocation-in-loop"),
            None
        );
        config.lint.preset = LintPreset::Strict;
        assert_eq!(
            severity_for(&config.lint, "effects/pure-extern-requires-allowlist"),
            Some(DiagnosticSeverity::Error)
        );

        let scratch = Scratch::new("bundle");
        scratch.file("dependency.lil", "export int value=1;");
        let main = scratch.file(
            "main.lil",
            "import {value} from \"./dependency\";print(value);",
        );
        config.bundle.mode = BundleMode::PreserveModules;

        let diagnostics = lint_path(&main, &config).unwrap();
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule == "size/eager-chunk-overhead"));
    }

    #[test]
    fn findings_in_imported_modules_carry_their_own_path_and_span() {
        let scratch = Scratch::new("modules");
        let dependency = scratch.file(
            "dependency.lil",
            "export int value(){return 1;print(2);}int hidden=3;",
        );
        let main = scratch.file(
            "main.lil",
            "import {value} from \"./dependency\";print(value());",
        );

        let diagnostics = lint_path(&main, &ProjectConfig::default()).unwrap();
        let source = std::fs::read_to_string(&dependency).unwrap();
        let canonical = dependency.canonicalize().unwrap();
        let unreachable = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.rule == "correctness/unreachable-code")
            .unwrap();
        assert_eq!(unreachable.path, canonical);
        assert_eq!(
            &source[unreachable.span.start..unreachable.span.end],
            "print(2)"
        );
        let unused = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.rule == "correctness/unused-private-symbol")
            .unwrap();
        assert_eq!(unused.path, canonical);
        assert_eq!(&source[unused.span.start..unused.span.end], "hidden");
        // An exported declaration is part of its module's interface.
        assert!(!diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("`value`")));
    }

    #[test]
    fn reports_surviving_collection_allocations_and_indirect_calls_in_loops() {
        let diagnostics = lint(
            "loop-cost",
            "extern void consume(Map<string,int> values);extern func(int)->int choose();func(int)->int operation=choose();for(int index=0;index<3;index++){Map<string,int> values=new Map();consume(values);print(operation(index));}",
        );
        assert!(
            rule_messages(&diagnostics, "performance/allocation-in-loop")
                .iter()
                .any(|message| message.contains("map"))
        );
        assert_eq!(
            rule_messages(&diagnostics, "performance/indirect-call-in-loop").len(),
            1
        );
    }

    #[test]
    fn reports_array_aggregate_and_closure_allocations_only_inside_loops() {
        let diagnostics = lint(
            "allocations",
            "struct Point{int x;int y;}class Box{int value;init(int value){this.value=value;}}int total=0;int[] outside=[1];for(int index=0;index<3;index++){int[] pair=[index,index];Point point=Point{index,index};Box box=new Box(index);func(int)->int twice=(int value)=>value*2;total=total+pair[0]+point.x+box.value+twice(index);}print(total+outside[0]);",
        );
        let allocations = rule_messages(&diagnostics, "performance/allocation-in-loop");
        assert_eq!(
            allocations
                .iter()
                .filter(|message| message.contains("surviving array allocation"))
                .count(),
            1,
            "{allocations:?}"
        );
        assert_eq!(
            allocations
                .iter()
                .filter(|message| message.contains("surviving aggregate allocation"))
                .count(),
            2,
            "{allocations:?}"
        );
        assert_eq!(
            rule_messages(&diagnostics, "performance/closure-allocation-in-loop"),
            ["surviving closure allocation executes inside a loop"]
        );
        // `twice` is a value the program cannot name as one function.
        assert_eq!(
            rule_messages(&diagnostics, "performance/indirect-call-in-loop").len(),
            1
        );
    }

    #[test]
    fn direct_calls_in_loops_are_not_indirect() {
        let diagnostics = lint(
            "direct-calls",
            "class Counter{int count;init(){this.count=0;}void bump(){this.count=this.count+1;}}int square(int value){return value*value;}Counter counter=new Counter();int total=0;for(int index=0;index<3;index++){counter.bump();total=total+square(index)+((int value)=>value)(index);}print(total+counter.count);",
        );
        assert!(
            rule_messages(&diagnostics, "performance/indirect-call-in-loop").is_empty(),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn reports_materialized_array_chains() {
        let diagnostics = lint(
            "chain",
            "int[] values=[1,2,3];int[] kept=values.map((int value)=>value*2).filter((int value)=>value>2);int[] single=values.map((int value)=>value+1);print(kept.length+single.length);",
        );
        assert_eq!(
            rule_messages(&diagnostics, "performance/materialized-array-chain"),
            ["array pipeline remains materialized after optimization"]
        );
    }

    #[test]
    fn reports_unhandled_dynamic_module_tasks() {
        let arena = Bump::new();
        let program = parse_source(&arena, "import(\"./feature\");").unwrap();
        let Item::Stmt(statement) = &program.items[0] else {
            panic!("expected expression statement");
        };
        let mut diagnostics = Vec::new();
        lint_statements(0, std::slice::from_ref(statement), &mut diagnostics);
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule == "correctness/unhandled-module-task"));
    }

    #[test]
    fn pluggable_rule_providers_use_exact_namespaces_and_configured_severity() {
        let scratch = Scratch::new("provider");
        let path = scratch.file("main.lil", "print(42);");
        let mut config = ProjectConfig::default();
        config.lint.providers = Some(vec!["agent".to_string()]);
        config
            .lint
            .rules
            .insert("agent/entry-budget".to_string(), LintSeverity::Error);

        let diagnostics = lint_path_with_providers(&path, &config, &[&AgentRuleProvider]).unwrap();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule, "agent/entry-budget");
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);
        assert_eq!(diagnostics[0].path, path.canonicalize().unwrap());
    }

    struct StrayProvider;

    impl LintRuleProvider for StrayProvider {
        fn id(&self) -> &'static str {
            "stray"
        }

        fn rules(&self) -> &'static [&'static str] {
            &["stray/finding"]
        }

        fn check(
            &self,
            _context: &LintRuleContext<'_, '_, '_>,
            diagnostics: &mut Vec<LintProviderDiagnostic>,
        ) {
            diagnostics.push(LintProviderDiagnostic {
                module: 7,
                span: Span::empty(0),
                rule: "stray/finding",
                message: "a finding in no module".to_string(),
                evidence: None,
                help: None,
                fix: None,
            });
        }
    }

    #[test]
    fn providers_report_only_in_modules_of_the_graph() {
        let scratch = Scratch::new("stray-provider");
        let path = scratch.file("main.lil", "print(42);");
        let error = lint_path_with_providers(&path, &ProjectConfig::default(), &[&StrayProvider])
            .unwrap_err();
        assert!(error.message.contains("unknown module 7"), "{error}");
    }

    #[test]
    fn web_provider_reports_eager_host_access_and_can_be_disabled() {
        let scratch = Scratch::new("web-provider");
        let path = scratch.file(
            "main.lil",
            "extern class Document{string title;}extern Document document;print(document.title);",
        );
        let mut config = ProjectConfig::default();
        config.lint.providers = Some(vec!["web".to_string()]);
        let diagnostics = lint_path(&path, &config).unwrap();
        let eager = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.rule == "web/eager-host-access")
            .unwrap();
        let source = std::fs::read_to_string(&path).unwrap();
        assert_eq!(&source[eager.span.start..eager.span.end], "document.title");

        config.lint.providers = Some(vec!["correctness".to_string()]);
        assert!(lint_path(&path, &config)
            .unwrap()
            .iter()
            .all(|diagnostic| diagnostic.rule != "web/eager-host-access"));
    }

    #[test]
    fn host_access_behind_a_function_is_not_eager() {
        let scratch = Scratch::new("web-deferred");
        let path = scratch.file(
            "main.lil",
            "extern class Document{string title;}extern Document document;export void start(){print(document.title);}",
        );
        let diagnostics = lint_path(&path, &ProjectConfig::default()).unwrap();
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule != "web/eager-host-access"));
    }

    #[test]
    fn a_refused_program_is_a_lint_error_at_the_compiler_diagnostic() {
        let scratch = Scratch::new("refused");
        let path = scratch.file("main.lil", "int value=\"wrong\";print(value);");
        let error = lint_path(&path, &ProjectConfig::default()).unwrap_err();
        assert_eq!(error.path, path.canonicalize().unwrap());
        assert!(error.message.contains("expected `int`"), "{error}");
        assert_eq!(
            &std::fs::read_to_string(&path).unwrap()[error.span.start..error.span.end],
            "\"wrong\""
        );
    }

    #[test]
    fn unsaved_source_replaces_the_entry_file() {
        let scratch = Scratch::new("unsaved");
        let path = scratch.file("main.lil", "int value=\"wrong\";");
        let diagnostics =
            lint_path_with_source(&path, "if(true){print(1);}", &ProjectConfig::default()).unwrap();
        assert_eq!(
            rule_messages(&diagnostics, "correctness/constant-condition"),
            ["condition is always `true`"]
        );
    }
}
