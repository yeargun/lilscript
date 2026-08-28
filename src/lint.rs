use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use bumpalo::Bump;
use serde::Serialize;

use crate::ast::{ArrowBody, ClassMember, Expr, ExternClassMember, Item, Program, Stmt};
use crate::config::{BundleMode, LintConfig, LintPreset, LintSeverity, ProjectConfig};
use crate::ir::{BlockId, ControlFlowOp, ControlShape, Intrinsic, Terminator, ValueId};
use crate::lexer::{lex, TokenKind};
use crate::lower::lower_to_control_flow;
use crate::module::{
    discover_modules_configured, discover_modules_configured_with_source, link_modules,
    locate_linked_span, parse_modules, ModuleError, ModuleSet,
};
use crate::optimizer::optimize_control_flow_with_options;
use crate::semantic::{analyze, EscapeState, SemanticModel, SymbolId};
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
    "performance/aggregate-escape",
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

pub struct LintRuleContext<'context, 'ast, 'src> {
    pub modules: &'context ModuleSet,
    pub program: &'context Program<'ast, 'src>,
    pub semantics: &'context SemanticModel<'src>,
    pub ir: &'context crate::ir::ControlFlowModule<'src>,
    pub config: &'context ProjectConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintProviderDiagnostic {
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
        let Some(entry) = context.ir.functions.get(context.ir.entry.0 as usize) else {
            return;
        };
        for instruction in entry.blocks.iter().flat_map(|block| &block.instructions) {
            if !matches!(
                instruction.op,
                ControlFlowOp::HostFieldGet { .. }
                    | ControlFlowOp::HostFieldSet { .. }
                    | ControlFlowOp::HostCall { .. }
            ) {
                continue;
            }
            diagnostics.push(LintProviderDiagnostic {
                span: instruction.span,
                rule: "web/eager-host-access",
                message: "top-level host access runs before progressive enhancement starts"
                    .to_string(),
                evidence: Some(
                    "the optimized entry function directly touches an extern host object"
                        .to_string(),
                ),
                help: Some(
                    "move host-dependent work behind an exported start function or a capability-gated event path"
                        .to_string(),
                ),
                fix: None,
            });
            break;
        }
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
    if !config.lint.enabled || path_is_excluded(path, &config.lint.exclude) {
        return Ok(Vec::new());
    }
    let modules = discover_modules_configured(path, config)?;
    lint_modules(modules, config, providers)
}

pub fn lint_path_with_source(
    path: &Path,
    source: &str,
    config: &ProjectConfig,
) -> Result<Vec<LintDiagnostic>, LintError> {
    if !config.lint.enabled || path_is_excluded(path, &config.lint.exclude) {
        return Ok(Vec::new());
    }
    let modules = discover_modules_configured_with_source(path, source, config)?;
    lint_modules(modules, config, &[&WEB_RULE_PROVIDER])
}

fn lint_modules(
    modules: ModuleSet,
    config: &ProjectConfig,
    providers: &[&dyn LintRuleProvider],
) -> Result<Vec<LintDiagnostic>, LintError> {
    let arena = Bump::new();
    let programs = parse_modules(&arena, &modules)?;
    let linked = link_modules(&arena, &modules, &programs)?;
    let semantics =
        analyze(&linked).map_err(|error| linked_error(&modules, error.span, error.message))?;

    let mut pending = Vec::new();
    lint_unused_imports(&modules, &programs, config, &mut pending);
    lint_bundle_policy(&modules, &programs, config, &mut pending);
    lint_ast(&linked, &semantics, &config.lint, &mut pending);

    let mut ir = lower_to_control_flow(&linked, &semantics)
        .map_err(|error| linked_error(&modules, error.span, error.message))?;
    optimize_control_flow_with_options(&mut ir, &config.js_optimizer_options(), true)
        .map_err(|error| linked_error(&modules, error.span, error.message))?;
    lint_ir(&ir, &mut pending);

    validate_rule_providers(providers).map_err(|message| LintError {
        path: modules.modules[modules.root].path.clone(),
        span: Span::empty(0),
        message,
    })?;
    let context = LintRuleContext {
        modules: &modules,
        program: &linked,
        semantics: &semantics,
        ir: &ir,
        config,
    };
    for provider in providers {
        if !provider_enabled(&config.lint, provider.id()) {
            continue;
        }
        let mut diagnostics = Vec::new();
        provider.check(&context, &mut diagnostics);
        if let Some(diagnostic) = diagnostics
            .iter()
            .find(|diagnostic| !provider.rules().contains(&diagnostic.rule))
        {
            return Err(LintError {
                path: modules.modules[modules.root].path.clone(),
                span: diagnostic.span,
                message: format!(
                    "lint provider `{}` emitted undeclared rule `{}`",
                    provider.id(),
                    diagnostic.rule
                ),
            });
        }
        pending.extend(diagnostics.into_iter().map(|diagnostic| PendingDiagnostic {
            span: diagnostic.span,
            rule: diagnostic.rule,
            message: diagnostic.message,
            evidence: diagnostic.evidence,
            help: diagnostic.help,
            fix: diagnostic.fix,
        }));
    }

    Ok(finalize_diagnostics(&modules, &config.lint, pending))
}

fn lint_bundle_policy(
    modules: &ModuleSet,
    programs: &[Program<'_, '_>],
    config: &ProjectConfig,
    pending: &mut Vec<PendingDiagnostic>,
) {
    if matches!(config.bundle.mode, BundleMode::Single) {
        return;
    }
    for (module_id, program) in programs.iter().enumerate() {
        let module = &modules.modules[module_id];
        for import in program.imports {
            pending.push(PendingDiagnostic {
                span: Span::new(
                    module.offset + import.span.start,
                    module.offset + import.span.end,
                ),
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
}

fn linked_error(modules: &ModuleSet, span: Span, message: String) -> LintError {
    let (module, local) = locate_linked_span(modules, span);
    LintError {
        path: module.path.clone(),
        span: local,
        message,
    }
}

fn lint_unused_imports(
    modules: &ModuleSet,
    programs: &[Program<'_, '_>],
    config: &ProjectConfig,
    pending: &mut Vec<PendingDiagnostic>,
) {
    for (module_id, program) in programs.iter().enumerate() {
        let module = &modules.modules[module_id];
        let Ok(tokens) = lex(&module.source) else {
            continue;
        };
        let mut counts = HashMap::<&str, usize>::new();
        for token in tokens {
            if let TokenKind::Ident(name) = token.kind {
                *counts.entry(name).or_default() += 1;
            }
        }
        for import in program.imports {
            for specifier in import.specifiers {
                if counts.get(specifier.local.name).copied().unwrap_or(0) == 1 {
                    let global_span = Span::new(
                        module.offset + specifier.local.span.start,
                        module.offset + specifier.local.span.end,
                    );
                    if severity_for(&config.lint, "correctness/unused-import").is_some() {
                        pending.push(PendingDiagnostic {
                            span: global_span,
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
    }
}

fn lint_ast(
    program: &Program<'_, '_>,
    semantics: &SemanticModel<'_>,
    config: &LintConfig,
    pending: &mut Vec<PendingDiagnostic>,
) {
    let mut references = HashMap::<SymbolId, usize>::new();
    walk_program_idents(program, &mut |span| {
        if let Some(symbol) = semantics.identifier_symbol(span) {
            *references.entry(symbol).or_default() += 1;
        }
    });
    let exported = program
        .exports
        .iter()
        .map(|export| export.local.name)
        .collect::<HashSet<_>>();
    let type_declarations = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Struct(declaration) => Some(declaration.name.span),
            Item::Class(declaration) => Some(declaration.name.span),
            Item::ExternClass(declaration) => Some(declaration.name.span),
            _ => None,
        })
        .collect::<HashSet<_>>();
    for symbol in semantics.symbols() {
        if !matches!(
            symbol.ty,
            crate::semantic::Type::Struct(_) | crate::semantic::Type::Class(_)
        ) && !type_declarations.contains(&symbol.span)
            && references.get(&symbol.id).copied().unwrap_or(0) <= 1
            && !symbol.name.starts_with('_')
            && !exported.contains(symbol.name)
        {
            pending.push(PendingDiagnostic {
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

    for item in program.items {
        match item {
            Item::Function(function) => {
                lint_statements(function.body, pending);
                lint_pure_extern_name(false, "", Span::empty(0), config, pending);
            }
            Item::Extern(function) => lint_pure_extern_name(
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
                            lint_statements(constructor.body, pending)
                        }
                        ClassMember::Method(method) => lint_statements(method.body, pending),
                        ClassMember::Field(_) => {}
                    }
                }
            }
            Item::ExternClass(class) => {
                for member in class.members {
                    if let ExternClassMember::Method(method) = member {
                        lint_pure_extern_name(
                            method.declared_pure,
                            method.name.name,
                            method.name.span,
                            config,
                            pending,
                        );
                    }
                }
            }
            Item::Stmt(statement) => lint_statements(std::slice::from_ref(statement), pending),
            _ => {}
        }
    }
}

fn lint_pure_extern_name(
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

fn lint_statements(statements: &[Stmt<'_, '_>], pending: &mut Vec<PendingDiagnostic>) {
    let mut terminated = false;
    for statement in statements {
        if terminated {
            pending.push(PendingDiagnostic {
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
        lint_statement(statement, pending);
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

fn lint_statement(statement: &Stmt<'_, '_>, pending: &mut Vec<PendingDiagnostic>) {
    match statement {
        Stmt::Expr(expression)
            if contains_dynamic_import(expression) && !task_chain_has_catch(expression) =>
        {
            pending.push(PendingDiagnostic {
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
        Stmt::Block { body, .. } => lint_statements(body, pending),
        Stmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            lint_constant_condition(condition, pending);
            lint_statement(then_branch, pending);
            if let Some(branch) = else_branch {
                lint_statement(branch, pending);
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            lint_constant_condition(condition, pending);
            lint_statement(body, pending);
        }
        Stmt::For {
            condition, body, ..
        } => {
            if let Some(condition) = condition {
                lint_constant_condition(condition, pending);
            }
            lint_statement(body, pending);
        }
        _ => {}
    }
}

fn task_chain_has_catch(expression: &Expr<'_, '_>) -> bool {
    match expression {
        Expr::Call { callee, .. } => match callee {
            Expr::Member {
                object, property, ..
            } => property.name == "catch" || task_chain_has_catch(object),
            _ => task_chain_has_catch(callee),
        },
        Expr::Member { object, .. } | Expr::OptionalMember { object, .. } => {
            task_chain_has_catch(object)
        }
        _ => false,
    }
}

fn contains_dynamic_import(expression: &Expr<'_, '_>) -> bool {
    match expression {
        Expr::DynamicImport { .. } => true,
        Expr::ArrayLiteral { elements, .. } => elements
            .iter()
            .any(|element| contains_dynamic_import(element.value())),
        Expr::RecordLiteral { entries, .. } | Expr::ObjectLiteral { entries, .. } => entries
            .iter()
            .any(|entry| contains_dynamic_import(entry.value())),
        Expr::StructLiteral { values, .. } | Expr::New { args: values, .. } => {
            values.iter().any(contains_dynamic_import)
        }
        Expr::Member { object, .. }
        | Expr::OptionalMember { object, .. }
        | Expr::Unary { expr: object, .. }
        | Expr::Await { task: object, .. }
        | Expr::TypeCheck { value: object, .. }
        | Expr::Update { target: object, .. } => contains_dynamic_import(object),
        Expr::Call { callee, args, .. } => {
            contains_dynamic_import(callee) || args.iter().any(contains_dynamic_import)
        }
        Expr::ArrowFunction { body, .. } => match body {
            ArrowBody::Expr(expression) => contains_dynamic_import(expression),
            ArrowBody::Block(_) => false,
        },
        Expr::Binary { lhs, rhs, .. }
        | Expr::Index {
            object: lhs,
            index: rhs,
            ..
        }
        | Expr::OptionalIndex {
            object: lhs,
            index: rhs,
            ..
        }
        | Expr::Assignment {
            target: lhs,
            value: rhs,
            ..
        } => contains_dynamic_import(lhs) || contains_dynamic_import(rhs),
        Expr::Template { parts, .. } => parts.iter().any(|part| match part {
            crate::ast::TemplatePart::Expr(expression) => contains_dynamic_import(expression),
            crate::ast::TemplatePart::String(..) => false,
        }),
        Expr::Match { value, arms, .. } => {
            contains_dynamic_import(value)
                || arms.iter().any(|arm| contains_dynamic_import(&arm.value))
        }
        Expr::If {
            condition,
            then_value,
            else_value,
            ..
        } => {
            contains_dynamic_import(condition)
                || contains_dynamic_import(then_value)
                || contains_dynamic_import(else_value)
        }
        Expr::Int(..)
        | Expr::Float(..)
        | Expr::String(..)
        | Expr::Bool(..)
        | Expr::Null(..)
        | Expr::Ident(..) => false,
    }
}

fn lint_constant_condition(condition: &Expr<'_, '_>, pending: &mut Vec<PendingDiagnostic>) {
    if let Expr::Bool(value, span) = condition {
        pending.push(PendingDiagnostic {
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

fn lint_ir(module: &crate::ir::ControlFlowModule<'_>, pending: &mut Vec<PendingDiagnostic>) {
    for function in &module.functions {
        if !function.live {
            continue;
        }
        let loop_blocks = function
            .shapes
            .iter()
            .filter_map(|shape| match shape {
                ControlShape::Loop { body, exit, .. } => {
                    Some(blocks_until_exit(function, *body, *exit))
                }
                _ => None,
            })
            .fold(HashSet::new(), |mut all, blocks| {
                all.extend(blocks);
                all
            });
        let mut definitions = HashMap::<ValueId, &ControlFlowOp<'_>>::new();
        for block in &function.blocks {
            for instruction in &block.instructions {
                if let Some(out) = instruction.out {
                    definitions.insert(out, &instruction.op);
                }
                let in_loop = loop_blocks.contains(&block.id);
                if in_loop {
                    match &instruction.op {
                        ControlFlowOp::Array(_) => pending.push(allocation_diagnostic(
                            instruction.span,
                            "array",
                            "performance/allocation-in-loop",
                        )),
                        ControlFlowOp::Struct { .. } | ControlFlowOp::NewClass { .. } => {
                            pending.push(allocation_diagnostic(
                                instruction.span,
                                "aggregate",
                                "performance/allocation-in-loop",
                            ));
                        }
                        ControlFlowOp::Intrinsic { intrinsic, .. } => {
                            if let Some(kind) = intrinsic_allocation_kind(*intrinsic) {
                                pending.push(allocation_diagnostic(
                                    instruction.span,
                                    kind,
                                    "performance/allocation-in-loop",
                                ));
                            }
                        }
                        ControlFlowOp::Closure { .. } => pending.push(allocation_diagnostic(
                            instruction.span,
                            "closure",
                            "performance/closure-allocation-in-loop",
                        )),
                        ControlFlowOp::CallValue { .. } => pending.push(PendingDiagnostic {
                            span: instruction.span,
                            rule: "performance/indirect-call-in-loop",
                            message: "indirect function call remains inside a loop".to_string(),
                            evidence: Some(
                                "the optimized IR could not resolve this call to a direct target"
                                    .to_string(),
                            ),
                            help: Some(
                                "keep the call site monomorphic or pass a statically known function when this loop is hot"
                                    .to_string(),
                            ),
                            fix: None,
                        }),
                        _ => {}
                    }
                }
                match &instruction.op {
                    ControlFlowOp::Struct { .. } | ControlFlowOp::NewClass { .. } => {
                        if let Some(out) = instruction.out {
                            if function.value_escapes.get(out.0 as usize).copied()
                                != Some(EscapeState::LocalOnly)
                            {
                                pending.push(PendingDiagnostic {
                                    span: instruction.span,
                                    rule: "performance/aggregate-escape",
                                    message: "aggregate escapes and cannot be scalar-replaced".to_string(),
                                    evidence: Some(format!(
                                        "escape analysis classified value {:?}",
                                        function.value_escapes.get(out.0 as usize)
                                    )),
                                    help: Some("keep the value inside typed LilScript boundaries when allocation removal matters".to_string()),
                                    fix: None,
                                });
                            }
                        }
                    }
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::ArrayMap | Intrinsic::ArrayFilter,
                        receiver: Some(receiver),
                        ..
                    } if definitions.get(receiver).is_some_and(|operation| {
                        matches!(
                            operation,
                            ControlFlowOp::Intrinsic {
                                intrinsic: Intrinsic::ArrayMap | Intrinsic::ArrayFilter,
                                ..
                            }
                        )
                    }) =>
                    {
                        pending.push(PendingDiagnostic {
                            span: instruction.span,
                            rule: "performance/materialized-array-chain",
                            message: "array pipeline remains materialized after optimization"
                                .to_string(),
                            evidence: Some(
                                "two adjacent map/filter operations survived optimizer fusion"
                                    .to_string(),
                            ),
                            help: Some(
                                "inspect callback side effects or escaping intermediate values"
                                    .to_string(),
                            ),
                            fix: None,
                        })
                    }
                    _ => {}
                }
            }
        }
    }
}

fn intrinsic_allocation_kind(intrinsic: Intrinsic) -> Option<&'static str> {
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

fn allocation_diagnostic(span: Span, kind: &str, rule: &'static str) -> PendingDiagnostic {
    PendingDiagnostic {
        span,
        rule,
        message: format!("surviving {kind} allocation executes inside a loop"),
        evidence: Some("the allocation remains in optimized IR".to_string()),
        help: Some(
            "hoist, reuse, or prevent the value from escaping when identity is not required"
                .to_string(),
        ),
        fix: None,
    }
}

fn blocks_until_exit(
    function: &crate::ir::ControlFlowFunction<'_>,
    start: BlockId,
    exit: BlockId,
) -> HashSet<BlockId> {
    let mut blocks = HashSet::new();
    let mut queue = VecDeque::from([start]);
    while let Some(block) = queue.pop_front() {
        if block == exit || !blocks.insert(block) {
            continue;
        }
        let Some(block_data) = function.blocks.get(block.0 as usize) else {
            continue;
        };
        match block_data.terminator.as_ref() {
            Some(Terminator::Jump(target)) => queue.push_back(*target),
            Some(Terminator::Branch {
                then_block,
                else_block,
                ..
            }) => {
                queue.push_back(*then_block);
                queue.push_back(*else_block);
            }
            _ => {}
        }
    }
    blocks
}

fn finalize_diagnostics(
    modules: &ModuleSet,
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
        let (module, span) = locate_linked_span(modules, item.span);
        if path_is_excluded(&module.path, &config.exclude)
            || is_suppressed(&module.source, span, item.rule)
            || !seen.insert((module.path.clone(), span, item.rule))
        {
            continue;
        }
        let fix = item.fix.map(|fix| LintFix {
            applicability: fix.applicability,
            edits: fix
                .edits
                .into_iter()
                .filter_map(|edit| {
                    let (edit_module, edit_span) = locate_linked_span(modules, edit.span);
                    (edit_module.path == module.path).then_some(LintEdit {
                        span: edit_span,
                        replacement: edit.replacement,
                    })
                })
                .collect(),
        });
        diagnostics.push(LintDiagnostic {
            path: module.path.clone(),
            span,
            rule: item.rule,
            severity,
            message: item.message,
            evidence: item.evidence,
            help: item.help,
            fix,
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

fn walk_program_idents(program: &Program<'_, '_>, visitor: &mut impl FnMut(Span)) {
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
                walk_expr_idents(argument, visitor);
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
        Expr::Ident(identifier) => visitor(identifier.span),
        Expr::ArrayLiteral { elements, .. } => {
            for element in *elements {
                walk_expr_idents(element.value(), visitor);
            }
        }
        Expr::RecordLiteral { entries, .. } | Expr::ObjectLiteral { entries, .. } => {
            for entry in *entries {
                if let crate::ast::RecordElement::Entry(entry) = entry {
                    visitor(entry.key.span);
                }
                walk_expr_idents(entry.value(), visitor);
            }
        }
        Expr::StructLiteral { name, values, .. } => {
            visitor(name.span);
            for value in *values {
                walk_expr_idents(value, visitor);
            }
        }
        Expr::New { class, args, .. } => {
            visitor(class.span);
            for argument in *args {
                walk_expr_idents(argument, visitor);
            }
        }
        Expr::Member { object, .. } | Expr::OptionalMember { object, .. } => {
            walk_expr_idents(object, visitor)
        }
        Expr::Call { callee, args, .. } => {
            walk_expr_idents(callee, visitor);
            for argument in *args {
                walk_expr_idents(argument, visitor);
            }
        }
        Expr::ArrowFunction { params, body, .. } => {
            for parameter in *params {
                visitor(parameter.name.span);
            }
            match body {
                ArrowBody::Expr(expression) => walk_expr_idents(expression, visitor),
                ArrowBody::Block(body) => walk_statements_idents(body, visitor),
            }
        }
        Expr::Unary { expr, .. } => walk_expr_idents(expr, visitor),
        Expr::Await { task, .. } => walk_expr_idents(task, visitor),
        Expr::Binary { lhs, rhs, .. } => {
            walk_expr_idents(lhs, visitor);
            walk_expr_idents(rhs, visitor);
        }
        Expr::TypeCheck { value, .. } => walk_expr_idents(value, visitor),
        Expr::Index { object, index, .. } | Expr::OptionalIndex { object, index, .. } => {
            walk_expr_idents(object, visitor);
            walk_expr_idents(index, visitor);
        }
        Expr::Assignment { target, value, .. } => {
            walk_expr_idents(target, visitor);
            walk_expr_idents(value, visitor);
        }
        Expr::Update { target, .. } => walk_expr_idents(target, visitor),
        Expr::Template { parts, .. } => {
            for part in *parts {
                if let crate::ast::TemplatePart::Expr(expression) = part {
                    walk_expr_idents(expression, visitor);
                }
            }
        }
        Expr::Match { value, arms, .. } => {
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
        Expr::If {
            condition,
            then_value,
            else_value,
            ..
        } => {
            walk_expr_idents(condition, visitor);
            walk_expr_idents(then_value, visitor);
            walk_expr_idents(else_value, visitor);
        }
        Expr::Int(..)
        | Expr::Float(..)
        | Expr::String(..)
        | Expr::Bool(..)
        | Expr::Null(..)
        | Expr::DynamicImport { .. } => {}
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
            let entry = &context.ir.functions[context.ir.entry.0 as usize];
            diagnostics.push(LintProviderDiagnostic {
                span: entry.span,
                rule: "agent/entry-budget",
                message: "agent policy checked the optimized entry".to_string(),
                evidence: Some(format!("{} optimized blocks", entry.blocks.len())),
                help: None,
                fix: None,
            });
        }
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
    }

    #[test]
    fn unreachable_code_has_a_machine_applicable_fix() {
        let directory =
            std::env::temp_dir().join(format!("lilscript-lint-fix-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("main.lil");
        std::fs::write(&path, "int example(){return 1;print(2);}print(example());").unwrap();

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
        std::fs::remove_dir_all(directory).unwrap();
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

        let directory =
            std::env::temp_dir().join(format!("lilscript-lint-bundle-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let dependency = directory.join("dependency.lil");
        let main = directory.join("main.lil");
        std::fs::write(&dependency, "export int value=1;").unwrap();
        std::fs::write(&main, "import {value} from \"./dependency\";print(value);").unwrap();
        config.bundle.mode = BundleMode::PreserveModules;

        let diagnostics = lint_path(&main, &config).unwrap();
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule == "size/eager-chunk-overhead"));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn reports_surviving_collection_allocations_and_indirect_calls_in_loops() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-lint-loop-cost-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("main.lil");
        std::fs::write(
            &path,
            "extern void consume(Map<string,int> values);extern func(int)->int choose();func(int)->int operation=choose();for(int index=0;index<3;index++){Map<string,int> values=new Map();consume(values);print(operation(index));}",
        )
        .unwrap();

        let diagnostics = lint_path(&path, &ProjectConfig::default()).unwrap();
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.rule == "performance/allocation-in-loop"
                && diagnostic.message.contains("map")
        }));
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule == "performance/indirect-call-in-loop"));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn reports_unhandled_dynamic_module_tasks() {
        let arena = Bump::new();
        let program = parse_source(&arena, "import(\"./feature\");").unwrap();
        let Item::Stmt(statement) = &program.items[0] else {
            panic!("expected expression statement");
        };
        let mut diagnostics = Vec::new();
        lint_statements(std::slice::from_ref(statement), &mut diagnostics);
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule == "correctness/unhandled-module-task"));
    }

    #[test]
    fn pluggable_rule_providers_use_exact_namespaces_and_configured_severity() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-lint-provider-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("main.lil");
        std::fs::write(&path, "print(42);").unwrap();
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
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn web_provider_reports_eager_host_access_and_can_be_disabled() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-lint-web-provider-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("main.lil");
        std::fs::write(
            &path,
            "extern class Document{string title;}extern Document document;print(document.title);",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.lint.providers = Some(vec!["web".to_string()]);
        let diagnostics = lint_path(&path, &config).unwrap();
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule == "web/eager-host-access"));

        config.lint.providers = Some(vec!["correctness".to_string()]);
        assert!(lint_path(&path, &config)
            .unwrap()
            .iter()
            .all(|diagnostic| diagnostic.rule != "web/eager-host-access"));
        std::fs::remove_dir_all(directory).unwrap();
    }
}
