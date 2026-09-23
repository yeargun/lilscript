//! The annotated-tree contender for the architecture experiment. Unsupported
//! source semantics fail explicitly; this is not a production fallback backend.

use super::*;
use crate::ast::{self, ArrayElement, ArrowBody, AssignmentOp, Item, RecordElement, Stmt};
use crate::primitive::ResolvedIntrinsic;
pub use crate::semantic::ExpressionResolution as Resolution;
use crate::semantic::{BuiltinCall, DefaultValue, FunctionType, NominalId, SemanticModel, Type};
use crate::span::Span;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

mod nominal;
use nominal::NominalCallable;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsupported {
    pub span: Span,
    pub feature: &'static str,
}

/// A borrowed projection of checker-owned facts, never a retained second table.
/// Target provenance selects the source occurrence; spans are diagnostic only.
#[derive(Debug, Clone, Copy)]
pub struct SourceFacts<'sem, 'src> {
    pub ty: Option<&'sem Type<'src>>,
    pub resolution: Resolution,
    pub span: Span,
}

impl<'sem, 'src> SourceFacts<'sem, 'src> {
    fn get(semantics: &'sem SemanticModel<'_, 'src>, id: ast::SourceNodeId) -> Option<Self> {
        let source = semantics.source_expression(id)?;
        let resolution = match (&source.kind, semantics.expression_resolution(id)) {
            // A receiver call uses the method identity checked on its callee.
            // Keep that one authority; the call owns its separate result type.
            (ast::ExprKind::Call { callee, .. }, Resolution::None) => {
                match semantics.expression_resolution(callee.id) {
                    resolution @ (Resolution::Primitive(_) | Resolution::NominalMember(_)) => {
                        resolution
                    }
                    _ => Resolution::None,
                }
            }
            (_, resolution) => resolution,
        };
        Some(Self {
            ty: semantics.expression_type(id),
            resolution,
            span: source.span(),
        })
    }
}

/// Identity for one immutable program state. Identical clones can share facts;
/// either branch's first mutable access acquires a distinct identity. An
/// incrementing per-program counter would collide after independent edits.
#[derive(Debug, Clone)]
pub(super) struct Revision(Arc<()>);
impl Revision {
    fn new() -> Self {
        Self(Arc::new(()))
    }
    pub(super) fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// The program owns semantic facts. Derived snapshots refer to its revision;
/// only a checked immutable borrow can expose those snapshots as proofs.
#[derive(Debug, Clone)]
pub struct AnnotatedTree<'sem, 'src> {
    target: Module,
    semantics: &'sem SemanticModel<'sem, 'src>,
    pub(super) revision: Revision,
    // A proof belongs to its immutable owner. Clones share the completed
    // proof; mutation detaches it with the same boundary as semantic facts.
    structure: OnceLock<Result<Arc<verify::Structure>, String>>,
}

impl AnnotatedTree<'_, '_> {
    pub fn target(&self) -> &Module {
        &self.target
    }
    pub(super) fn target_mut(&mut self) -> &mut Module {
        self.revision = Revision::new();
        self.structure.take();
        &mut self.target
    }
    pub(super) fn structure(&self) -> Result<(&verify::Structure, bool), String> {
        let reused = self.structure.get().is_some();
        self.structure
            .get_or_init(|| verify::verify(&self.target).map(Arc::new))
            .as_deref()
            .map(|structure| (structure, reused))
            .map_err(Clone::clone)
    }

    /// Verify an owned edit once. Reachability compaction preserves the
    /// verified graph and remaps its metadata with the same target handles.
    /// A failed edit leaves no previously valid proof attached to this state.
    pub(super) fn edit(
        &mut self,
        edit: impl FnOnce(&mut Module),
    ) -> Result<compact::Removed, String> {
        edit(self.target_mut());
        let mut structure = verify::verify(&self.target)?;
        let removed = compact::compact(&mut self.target, &mut structure);
        self.structure = OnceLock::from(Ok(Arc::new(structure)));
        Ok(removed)
    }
    pub fn source_type(&self, expression: ExprId) -> Option<&Type<'_>> {
        let origin = (*self.target.origins.get(expression.index())?)?;
        self.semantics.expression_type(origin)
    }
    pub fn source_facts(&self, expression: ExprId) -> Option<SourceFacts<'_, '_>> {
        let origin = (*self.target.origins.get(expression.index())?)?;
        SourceFacts::get(self.semantics, origin)
    }
    pub fn render(&self, policy: PrintPolicy) -> Result<String, String> {
        self.output()?
            .render(&naming::Plan::new(if policy.mangle_bindings {
                naming::Style::Global
            } else {
                naming::Style::Source
            }))
    }
    pub fn output(&self) -> Result<extract::Output<'_>, String> {
        extract::Output::new(self, None)
    }
}

pub fn lower_slice<'ast, 'sem, 'src>(
    program: &ast::Program<'ast, 'src>,
    semantics: &'sem SemanticModel<'sem, 'src>,
) -> Result<AnnotatedTree<'sem, 'src>, Unsupported> {
    if !semantics.belongs_to(program.source_identity()) {
        return Err(Unsupported {
            span: program.span,
            feature: "semantic facts belong to a different source program",
        });
    }
    if let Some(span) = semantics.reference_parameter_span() {
        return Err(Unsupported {
            span,
            feature: "reference parameter calling convention",
        });
    }
    if !program.imports.is_empty()
        || !program.foreign_imports.is_empty()
        || !program.dynamic_imports.is_empty()
        || !program.exports.is_empty()
        || !program.module_bindings.is_empty()
        || !program.constructor_values.is_empty()
    {
        return Err(Unsupported {
            span: program.span,
            feature: "module and public-boundary lowering",
        });
    }
    let mut lower = Lower {
        module: Module::default(),
        semantics,
        externs: BTreeMap::new(),
        bindings: vec![None; semantics.symbols().len()],
        bound_symbols: Vec::new(),
        nominal_functions: HashMap::new(),
        constructor: None,
    };
    // An extern's SymbolId is the authority. Shadowed local names stay local.
    for item in program.items {
        match item {
            Item::Function(function) => {
                lower.declare(function.name, RegionId::new(0))?;
            }
            Item::Stmt(Stmt::VarDecl(declaration)) => {
                lower.declare(declaration.name, RegionId::new(0))?;
            }
            Item::Class(class) => lower.declare_class(class)?,
            Item::Extern(declaration) => {
                if declaration.name.name == "eval" {
                    return Err(Unsupported {
                        span: declaration.span,
                        feature: "source-level eval binding contract",
                    });
                }
                lower.externs.insert(
                    lower.symbol(declaration.name)?.0,
                    declaration.name.name.into(),
                );
            }
            Item::ExternGlobal(declaration) => {
                if declaration.name.name == "eval" {
                    return Err(Unsupported {
                        span: declaration.span,
                        feature: "source-level eval binding contract",
                    });
                }
                lower.externs.insert(
                    lower.symbol(declaration.name)?.0,
                    declaration.name.name.into(),
                );
            }
            _ => {}
        }
    }
    for item in program.items {
        match item {
            Item::Function(function) => {
                if function.is_async || function.is_generator {
                    return Err(Unsupported {
                        span: function.span,
                        feature: "async or generator function",
                    });
                }
                // Type parameters erase for supported value-level operations.
                // Their semantic types remain in SourceFacts; an unresolved T
                // supplies no primitive, range or effect proof. Unsupported
                // representation-dependent operations still fail in expression
                // lowering instead of guessing an instantiation's layout.
                let body = lower.module.region(ScopeId::new(0));
                let checkpoint = lower.bound_symbols.len();
                let parameters = lower.parameters(function.params, body)?;
                lower.statements(function.body, body)?;
                lower.restore_bindings(checkpoint);
                let id = FunctionId::new(lower.module.functions.len());
                lower.module.functions.push(Function {
                    parameters,
                    body,
                    arrow: false,
                    name: FunctionName::Exact(function.name.name.into()),
                    strict: false,
                    length: None,
                    suspension: crate::structured_js::Suspension::None,
                });
                let binding = lower.binding(function.name)?;
                lower.push(
                    RegionId::new(0),
                    Statement::Function {
                        binding,
                        function: id,
                    },
                );
            }
            Item::Stmt(statement) => lower.statement(statement, RegionId::new(0))?,
            Item::Class(class) => lower.class(class)?,
            Item::Extern(_) | Item::ExternGlobal(_) => {}
            _ => {
                return Err(Unsupported {
                    span: item.span(),
                    feature: "aggregate declaration",
                });
            }
        }
    }
    Ok(AnnotatedTree {
        revision: Revision::new(),
        target: lower.module,
        semantics,
        structure: OnceLock::new(),
    })
}

struct Lower<'sem, 'src> {
    module: Module,
    semantics: &'sem SemanticModel<'sem, 'src>,
    externs: BTreeMap<u32, String>,
    // This translation belongs only to source construction, not target analyses.
    bindings: Vec<Option<BindingId>>,
    // A source callable may be instantiated at several default call sites.
    // Each construction has its own local target cells; outer translations
    // survive and only the bindings introduced by this body are rolled back.
    bound_symbols: Vec<SymbolId>,
    // Source callable declarations translate to ordinary target functions once.
    // This construction map is discarded with the lowerer, not retained as facts.
    nominal_functions: HashMap<NominalCallable, BindingId>,
    constructor: Option<(NominalId, BindingId)>,
}

/// Construction-only place capture. The persistent program contains ordinary
/// lexical cells and ordered expressions, with one owner for each occurrence.
struct CapturedPlace {
    before: Vec<ExprId>,
    target: ExprId,
    old: ExprId,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Use {
    Value,
    Discarded,
    CallTarget,
    Place,
}

impl<'sem, 'src> Lower<'sem, 'src> {
    fn symbol(&self, ident: ast::Ident<'src>) -> Result<SymbolId, Unsupported> {
        self.semantics
            .identifier_symbol(ident.span)
            .ok_or(Unsupported {
                span: ident.span,
                feature: "unresolved semantic binding",
            })
    }

    fn binding(&self, ident: ast::Ident<'src>) -> Result<BindingId, Unsupported> {
        self.bindings[self.symbol(ident)?.0 as usize].ok_or(Unsupported {
            span: ident.span,
            feature: "target cell missing from its declared lexical scope",
        })
    }

    fn declare(
        &mut self,
        ident: ast::Ident<'src>,
        region: RegionId,
    ) -> Result<BindingId, Unsupported> {
        let symbol = self.symbol(ident)?;
        let scope = self.module.regions[region.index()].scope;
        if let Some(binding) = self.bindings[symbol.0 as usize] {
            if self.module.bindings[binding.index()].scope != scope {
                return Err(Unsupported {
                    span: ident.span,
                    feature: "source declaration has conflicting lexical owners",
                });
            }
            return Ok(binding);
        }
        let binding = self.module.binding(Binding {
            source_symbol: Some(symbol),
            scope,
            spelling: ident.name.into(),
            pinned: false,
        });
        self.bindings[symbol.0 as usize] = Some(binding);
        self.bound_symbols.push(symbol);
        Ok(binding)
    }

    fn restore_bindings(&mut self, checkpoint: usize) {
        for symbol in self.bound_symbols.drain(checkpoint..) {
            self.bindings[symbol.0 as usize] = None;
        }
    }

    fn parameters(
        &mut self,
        parameters: &[ast::Param<'_, 'src>],
        body: RegionId,
    ) -> Result<Vec<BindingId>, Unsupported> {
        // Defaults belong to the checked callable contract. Typed calls create
        // omitted values; the target function keeps its complete parameter list.
        parameters
            .iter()
            .map(|parameter| {
                if parameter.parameter.passing != crate::primitive::ParameterPassing::Value {
                    return Err(Unsupported {
                        span: parameter.span,
                        feature: "reference parameter storage",
                    });
                }
                self.declare(parameter.name, body)
            })
            .collect()
    }

    fn call_arguments(
        &mut self,
        callee: &ast::Expr<'_, 'src>,
        provided: &[ast::Argument<'_, 'src>],
        region: RegionId,
    ) -> Result<Vec<ExprId>, Unsupported> {
        let signature = match self.semantics.expression_type(callee.id) {
            Some(Type::Function(signature)) => Some(signature),
            Some(Type::GenericFunction(function)) => Some(&function.signature),
            _ => None,
        };
        self.checked_arguments(provided, signature, region, callee.span())
    }

    fn checked_arguments(
        &mut self,
        provided: &[ast::Argument<'_, 'src>],
        signature: Option<&FunctionType<'src>>,
        region: RegionId,
        span: Span,
    ) -> Result<Vec<ExprId>, Unsupported> {
        if let Some(argument) = provided
            .iter()
            .find(|argument| argument.passing != crate::primitive::ParameterPassing::Value)
        {
            return Err(Unsupported {
                span: argument.span,
                feature: "reference argument transport",
            });
        }
        if signature.is_some_and(|signature| {
            signature
                .params
                .iter()
                .any(|parameter| parameter.passing != crate::primitive::ParameterPassing::Value)
        }) {
            return Err(Unsupported {
                span,
                feature: "reference parameter calling convention",
            });
        }
        let Some(signature) = signature.filter(|signature| provided.len() < signature.params.len())
        else {
            return provided
                .iter()
                .map(|argument| self.expression(&argument.expression, region))
                .collect();
        };
        // A later default observes an earlier argument's already evaluated
        // value, including defaults that another omitted parameter references.
        let mut referenced = vec![false; signature.params.len()];
        for (index, parameter) in signature.params.iter().enumerate().skip(provided.len()) {
            if let Some(DefaultValue::Parameter(earlier)) = &parameter.default {
                if *earlier >= index {
                    return Err(Unsupported {
                        span,
                        feature: "invalid parameter-default dependency",
                    });
                }
                referenced[*earlier] = true;
            }
        }
        self.arguments_with_defaults(provided, signature, &referenced, region, span)
    }

    fn arguments_with_defaults(
        &mut self,
        provided: &[ast::Argument<'_, 'src>],
        signature: &FunctionType<'src>,
        referenced: &[bool],
        region: RegionId,
        span: Span,
    ) -> Result<Vec<ExprId>, Unsupported> {
        let mut arguments = Vec::with_capacity(signature.params.len());
        let mut snapshots = vec![None; signature.params.len()];
        for index in 0..signature.params.len() {
            let value = if let Some(argument) = provided.get(index) {
                self.expression(&argument.expression, region)?
            } else {
                let default = signature
                    .params
                    .get(index)
                    .and_then(|parameter| parameter.default.as_ref())
                    .ok_or(Unsupported {
                        span,
                        feature: "missing checked parameter default",
                    })?;
                self.default_argument(default, &snapshots, region, span)?
            };
            if referenced[index] {
                let (binding, assignment) = self.snapshot(value, region);
                snapshots[index] = Some(binding);
                // The assignment is this argument's evaluation. It must not
                // move ahead of callee lookup or another supplied argument.
                arguments.push(assignment);
            } else {
                arguments.push(value);
            }
        }
        Ok(arguments)
    }

    fn default_argument(
        &mut self,
        default: &DefaultValue<'src>,
        snapshots: &[Option<BindingId>],
        region: RegionId,
        span: Span,
    ) -> Result<ExprId, Unsupported> {
        let node = match default {
            DefaultValue::Int(value) => Expr::Literal(Literal::Number(*value as f64)),
            DefaultValue::Float(bits) => Expr::Literal(Literal::Number(f64::from_bits(*bits))),
            DefaultValue::String(value) => Expr::Literal(Literal::String(
                StringValue::decode_source(value).map_err(|_| Unsupported {
                    span,
                    feature: "invalid checked string default",
                })?,
            )),
            DefaultValue::Bool(value) => Expr::Literal(Literal::Bool(*value)),
            DefaultValue::Null => Expr::Literal(Literal::Null),
            DefaultValue::Undefined => Expr::Literal(Literal::Undefined),
            DefaultValue::Symbol(symbol) => self.identifier_node(*symbol, span)?,
            DefaultValue::Parameter(index) => Expr::Binding(
                snapshots
                    .get(*index)
                    .copied()
                    .flatten()
                    .ok_or(Unsupported {
                        span,
                        feature: "missing evaluated default argument",
                    })?,
            ),
            DefaultValue::Array(values) => Expr::Array(
                values
                    .iter()
                    .map(|value| self.default_argument(value, snapshots, region, span))
                    .collect::<Result<_, _>>()?,
            ),
            DefaultValue::Arrow(source) => {
                let expression = self
                    .semantics
                    .source_expression(*source)
                    .ok_or(Unsupported {
                        span,
                        feature: "missing checked default callable source",
                    })?;
                return self.expression(expression, region);
            }
            DefaultValue::Struct { .. } | DefaultValue::NewClass { .. } => {
                return Err(Unsupported {
                    span,
                    feature: "nominal default outside the supported source slice",
                });
            }
            DefaultValue::PendingIdentifier { .. } | DefaultValue::PendingUndefined { .. } => {
                return Err(Unsupported {
                    span,
                    feature: "unresolved checked parameter default",
                });
            }
        };
        // This is a new evaluation at the call site, not another occurrence of
        // a declaration initializer or of a previously evaluated argument.
        Ok(self.module.expression(node, None))
    }

    fn push(&mut self, region: RegionId, statement: Statement) {
        self.module.regions[region.index()]
            .statements
            .push(statement);
    }

    fn statements(
        &mut self,
        statements: &[Stmt<'_, 'src>],
        region: RegionId,
    ) -> Result<(), Unsupported> {
        // A closure may capture a later declaration. Allocate the scope's
        // lexical cells before lowering values, without initializing any cell.
        for statement in statements {
            if let Stmt::VarDecl(declaration) = statement {
                self.declare(declaration.name, region)?;
            }
        }
        for statement in statements {
            self.statement(statement, region)?;
        }
        Ok(())
    }

    fn branch(
        &mut self,
        statement: &Stmt<'_, 'src>,
        parent: RegionId,
    ) -> Result<RegionId, Unsupported> {
        let region = self
            .module
            .region(self.module.regions[parent.index()].scope);
        if let Stmt::Block { body, .. } = statement {
            self.statements(body, region)?;
        } else {
            self.statement(statement, region)?;
        }
        Ok(region)
    }

    fn statement(
        &mut self,
        statement: &Stmt<'_, 'src>,
        region: RegionId,
    ) -> Result<(), Unsupported> {
        let target = match statement {
            Stmt::VarDecl(declaration) => self.variable_declaration(declaration, region)?,
            Stmt::Expr(value) => {
                Statement::Evaluate(self.expression_in(value, region, Use::Discarded)?)
            }
            Stmt::Return { value, .. } => Statement::Return(match value {
                Some(value) => Some(self.expression(value, region)?),
                None => self
                    .constructor
                    .map(|(_, receiver)| self.module.expression(Expr::Binding(receiver), None)),
            }),
            Stmt::SuperCall { args, span } => {
                Statement::Evaluate(self.super_call(args, region, *span)?)
            }
            Stmt::Throw { value, .. } => Statement::Throw(self.expression(value, region)?),
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => Statement::If {
                condition: self.expression(condition, region)?,
                yes: self.branch(then_branch, region)?,
                no: else_branch
                    .map(|branch| self.branch(branch, region))
                    .transpose()?,
            },
            Stmt::While {
                condition, body, ..
            } => Statement::Loop {
                condition: Some(self.expression(condition, region)?),
                update: None,
                body: self.branch(body, region)?,
            },
            Stmt::For {
                initializer,
                condition,
                update,
                body,
                ..
            } => {
                // The source initializer executes once and closures share its
                // cell. JS for(let ...) would create different per-iteration
                // cells. Retain the source region and explicit loop control.
                let header = self
                    .module
                    .region(self.module.regions[region.index()].scope);
                if let Some(initializer) = initializer {
                    let initializer = match initializer {
                        ast::ForInitializer::VarDecl(declaration) => {
                            self.variable_declaration(declaration, header)?
                        }
                        ast::ForInitializer::Expr(value) => Statement::Evaluate(
                            self.expression_in(value, header, Use::Discarded)?,
                        ),
                    };
                    self.push(header, initializer);
                }
                let condition = condition
                    .as_ref()
                    .map(|value| self.expression(value, header))
                    .transpose()?;
                let update = update
                    .as_ref()
                    .map(|value| self.expression_in(value, header, Use::Discarded))
                    .transpose()?;
                let body = self.branch(body, header)?;
                self.push(
                    header,
                    Statement::Loop {
                        condition,
                        update,
                        body,
                    },
                );
                Statement::Block(header)
            }
            Stmt::Block { .. } => Statement::Block(self.branch(statement, region)?),
            Stmt::Try {
                body,
                catch,
                finally,
                ..
            } => {
                let parent = self.module.regions[region.index()].scope;
                let body_region = self.module.region(parent);
                self.statements(body, body_region)?;
                let catch = catch
                    .as_ref()
                    .map(|clause| -> Result<_, Unsupported> {
                        let body = self.module.region(parent);
                        let binding = clause
                            .binding
                            .map(|binding| self.declare(binding.name, body))
                            .transpose()?;
                        self.statements(clause.body, body)?;
                        Ok(Catch { binding, body })
                    })
                    .transpose()?;
                let finally = finally
                    .map(|statements| -> Result<_, Unsupported> {
                        let body = self.module.region(parent);
                        self.statements(statements, body)?;
                        Ok(body)
                    })
                    .transpose()?;
                Statement::Try {
                    body: body_region,
                    catch,
                    finally,
                }
            }
            Stmt::Break(_) => Statement::Break,
            Stmt::Continue(_) => Statement::Continue,
            _ => {
                return Err(Unsupported {
                    span: statement.span(),
                    feature: "statement outside the initial source slice",
                });
            }
        };
        self.push(region, target);
        Ok(())
    }

    fn variable_declaration(
        &mut self,
        declaration: &ast::VarDecl<'_, 'src>,
        region: RegionId,
    ) -> Result<Statement, Unsupported> {
        let binding = self.declare(declaration.name, region)?;
        let Some(initializer) = &declaration.initializer else {
            return Err(Unsupported {
                span: declaration.span,
                feature: "implicit default initialization",
            });
        };
        Ok(Statement::Let {
            binding,
            value: Some(self.named_expression(initializer, region, declaration.name.name)?),
        })
    }

    fn expression(
        &mut self,
        source: &ast::Expr<'_, 'src>,
        region: RegionId,
    ) -> Result<ExprId, Unsupported> {
        self.expression_in(source, region, Use::Value)
    }

    fn arithmetic(op: Binary, ty: Option<&Type<'_>>, left: ExprId, right: ExprId) -> Expr {
        // Signed bitwise operators already normalize; unsigned shift may
        // produce 2^32-1. The same source operation owns this obligation in a
        // value expression, compound store or prefix/postfix update.
        if let Some(op) = IntBinary::from_javascript(op).filter(|_| ty == Some(&Type::Int)) {
            Expr::IntBinary { op, left, right }
        } else {
            Expr::Binary { op, left, right }
        }
    }

    fn binary_operator(op: ast::BinaryOp) -> Binary {
        match op {
            ast::BinaryOp::Add => Binary::Add,
            ast::BinaryOp::Sub => Binary::Subtract,
            ast::BinaryOp::Mul => Binary::Multiply,
            ast::BinaryOp::Div => Binary::Divide,
            ast::BinaryOp::Mod => Binary::Remainder,
            ast::BinaryOp::BitAnd => Binary::BitAnd,
            ast::BinaryOp::BitOr => Binary::BitOr,
            ast::BinaryOp::Xor => Binary::BitXor,
            ast::BinaryOp::ShiftLeft => Binary::ShiftLeft,
            ast::BinaryOp::ShiftRight => Binary::ShiftRight,
            ast::BinaryOp::UnsignedShiftRight => Binary::UnsignedShiftRight,
            ast::BinaryOp::Eq => Binary::StrictEqual,
            ast::BinaryOp::NotEq => Binary::StrictNotEqual,
            ast::BinaryOp::Less => Binary::Less,
            ast::BinaryOp::LessEq => Binary::LessEqual,
            ast::BinaryOp::Greater => Binary::Greater,
            ast::BinaryOp::GreaterEq => Binary::GreaterEqual,
            ast::BinaryOp::And => Binary::And,
            ast::BinaryOp::Or => Binary::Or,
            ast::BinaryOp::Nullish => Binary::Nullish,
        }
    }

    fn is_eager_binary(source: &ast::Expr<'_, '_>) -> bool {
        matches!(source.kind, ast::ExprKind::Binary { op, .. }
            if !matches!(op, ast::BinaryOp::And | ast::BinaryOp::Or | ast::BinaryOp::Nullish))
    }

    /// Construct eager operators in left-to-right postorder. Each completed
    /// source occurrence passes through the same materialization boundary as
    /// other expressions, including signed normalization and source provenance.
    fn eager_binary_expression<'ast>(
        &mut self,
        source: &'ast ast::Expr<'ast, 'src>,
        region: RegionId,
        use_: Use,
    ) -> Result<ExprId, Unsupported> {
        enum Step<'ast, 'sem, 'src> {
            Visit(&'ast ast::Expr<'ast, 'src>, Use),
            Finish {
                source: &'ast ast::Expr<'ast, 'src>,
                facts: SourceFacts<'sem, 'src>,
                use_: Use,
            },
        }
        let mut pending = vec![Step::Visit(source, use_)];
        let mut values = Vec::new();
        while let Some(step) = pending.pop() {
            match step {
                Step::Visit(source, use_) if Self::is_eager_binary(source) => {
                    let facts = SourceFacts::get(self.semantics, source.id).ok_or(Unsupported {
                        span: source.span(),
                        feature: "missing checked source occurrence",
                    })?;
                    let ast::ExprKind::Binary { lhs, rhs, .. } = &source.kind else {
                        unreachable!("eager binary visit owns a binary expression")
                    };
                    pending.push(Step::Finish {
                        source,
                        facts,
                        use_,
                    });
                    pending.push(Step::Visit(rhs, Use::Value));
                    pending.push(Step::Visit(lhs, Use::Value));
                }
                Step::Visit(source, use_) => {
                    values.push(self.expression_in(source, region, use_)?);
                }
                Step::Finish {
                    source,
                    facts,
                    use_,
                } => {
                    let right = values.pop().expect("lowered right binary operand");
                    let left = values.pop().expect("lowered left binary operand");
                    let ast::ExprKind::Binary { op, .. } = source.kind else {
                        unreachable!("eager binary finish owns a binary expression")
                    };
                    let node = Self::arithmetic(Self::binary_operator(op), facts.ty, left, right);
                    values.push(self.materialize(source, node, facts, use_)?);
                }
            }
        }
        debug_assert_eq!(values.len(), 1);
        Ok(values.pop().expect("lowered binary expression"))
    }

    fn identifier_node(&self, symbol: SymbolId, span: Span) -> Result<Expr, Unsupported> {
        if let Some(name) = self.externs.get(&symbol.0) {
            Ok(Expr::Host(name.clone()))
        } else {
            Ok(Expr::Binding(self.bindings[symbol.0 as usize].ok_or(
                Unsupported {
                    span,
                    feature: "target cell missing from its declared lexical scope",
                },
            )?))
        }
    }

    fn index_reference(
        &mut self,
        object: &ast::Expr<'_, 'src>,
        index: &ast::Expr<'_, 'src>,
        region: RegionId,
        span: Span,
    ) -> Result<Expr, Unsupported> {
        let receiver = self.semantics.expression_type(object.id);
        if !matches!(
            receiver,
            Some(Type::Array(_) | Type::Record(_) | Type::String)
        ) && !receiver.is_some_and(crate::typed_array::is_typed_array_type)
        {
            return Err(Unsupported {
                span,
                feature: "indexed reference outside the supported source types",
            });
        }
        Ok(Expr::Member {
            object: self.expression(object, region)?,
            property: Property::Computed(self.expression(index, region)?),
        })
    }

    fn snapshot(&mut self, value: ExprId, region: RegionId) -> (BindingId, ExprId) {
        let temporary = self.module.binding(Binding {
            source_symbol: None,
            scope: self.module.regions[region.index()].scope,
            spelling: "temporary".into(),
            pinned: false,
        });
        self.push(
            region,
            Statement::Let {
                binding: temporary,
                value: None,
            },
        );
        let cell = self.module.expression(Expr::Binding(temporary), None);
        let assignment = self.module.expression(
            Expr::Assign {
                target: cell,
                value,
            },
            None,
        );
        (temporary, assignment)
    }

    fn sequence(&mut self, mut before: Vec<ExprId>, result: Expr) -> Expr {
        if before.is_empty() {
            result
        } else {
            before.push(self.module.expression(result, None));
            Expr::Sequence(before)
        }
    }

    fn capture_place(
        &mut self,
        source: &ast::Expr<'_, 'src>,
        region: RegionId,
    ) -> Result<CapturedPlace, Unsupported> {
        // Only references need this construction path. Ordinary arithmetic,
        // calls and literals go directly from their builder into the arena.
        let facts = SourceFacts::get(self.semantics, source.id).ok_or(Unsupported {
            span: source.span(),
            feature: "missing checked source occurrence",
        })?;
        let node = match source {
            ast::Expr {
                kind: ast::ExprKind::Ident(ident),
                ..
            } => {
                let Resolution::Binding(symbol) = facts.resolution else {
                    return Err(Unsupported {
                        span: ident.span,
                        feature: "unresolved semantic binding",
                    });
                };
                self.identifier_node(symbol, ident.span)?
            }
            ast::Expr {
                kind: ast::ExprKind::Index { object, index, .. },
                ..
            } => self.index_reference(object, index, region, source.span())?,
            ast::Expr {
                kind: ast::ExprKind::Member { object, .. },
                ..
            } => self.nominal_field(source, object, region)?,
            _ => {
                return Err(Unsupported {
                    span: source.span(),
                    feature: "update requires a supported mutable reference",
                });
            }
        };
        let mut before = Vec::new();
        let (write, read) = match node {
            Expr::Binding(binding) => (Expr::Binding(binding), Expr::Binding(binding)),
            Expr::Member { object, property } => {
                // The receiver can be reassigned by evaluation of the key or
                // RHS. Capture values, never reevaluate the original syntax.
                let (object, assignment) = self.snapshot(object, region);
                before.push(assignment);
                let (write_key, read_key) = match property {
                    Property::Named(name) => (Property::Named(name.clone()), Property::Named(name)),
                    Property::Computed(key) => {
                        // Supported source indexed places have primitive int
                        // or string keys. Get and Put cannot invoke an object
                        // key's coercion twice under this checked contract.
                        let ast::Expr {
                            kind: ast::ExprKind::Index { index, .. },
                            ..
                        } = source
                        else {
                            unreachable!()
                        };
                        if !matches!(
                            self.semantics.expression_type(index.id),
                            Some(Type::Int | Type::String)
                        ) {
                            return Err(Unsupported {
                                span: index.span(),
                                feature: "compound place requires a primitive source key",
                            });
                        }
                        let (key, assignment) = self.snapshot(key, region);
                        before.push(assignment);
                        (
                            Property::Computed(self.module.expression(Expr::Binding(key), None)),
                            Property::Computed(self.module.expression(Expr::Binding(key), None)),
                        )
                    }
                };
                let write_object = self.module.expression(Expr::Binding(object), None);
                let read_object = self.module.expression(Expr::Binding(object), None);
                (
                    Expr::Member {
                        object: write_object,
                        property: write_key,
                    },
                    Expr::Member {
                        object: read_object,
                        property: read_key,
                    },
                )
            }
            _ => {
                return Err(Unsupported {
                    span: source.span(),
                    feature: "update requires a supported mutable reference",
                });
            }
        };
        let target = self.materialize(source, write, facts, Use::Place)?;
        // A typed-array or array int getter still needs its source conversion;
        // the raw member itself does not inherit the normalized result proof.
        let old = self.materialize(source, read, facts, Use::Value)?;
        Ok(CapturedPlace {
            before,
            target,
            old,
        })
    }

    fn update_place(
        &mut self,
        place: CapturedPlace,
        op: Binary,
        ty: Option<&Type<'_>>,
        keep_old: bool,
        region: RegionId,
    ) -> Expr {
        let CapturedPlace {
            mut before,
            target,
            old,
        } = place;
        let one = self
            .module
            .expression(Expr::Literal(Literal::Number(1.0)), None);
        if !keep_old {
            let value = self
                .module
                .expression(Self::arithmetic(op, ty, old, one), None);
            return self.sequence(before, Expr::Assign { target, value });
        }
        // Store coercion (for example Uint8 truncation) does not redefine the
        // source update's result. A postfix value is the normalized old read.
        let (temporary, assignment) = self.snapshot(old, region);
        before.push(assignment);
        let current = self.module.expression(Expr::Binding(temporary), None);
        let value = self
            .module
            .expression(Self::arithmetic(op, ty, current, one), None);
        before.push(self.module.expression(Expr::Assign { target, value }, None));
        self.sequence(before, Expr::Binding(temporary))
    }

    /// Source named evaluation applies only to a directly created anonymous
    /// function. Assigning an existing value, including another assignment's
    /// result, does not rename that value.
    fn named_expression(
        &mut self,
        source: &ast::Expr<'_, 'src>,
        region: RegionId,
        name: &str,
    ) -> Result<ExprId, Unsupported> {
        let value = self.expression(source, region)?;
        if let Expr::Function(function) = self.module.expressions[value.index()] {
            self.module.functions[function.index()].name = FunctionName::Exact(name.into());
        }
        Ok(value)
    }

    fn expression_in(
        &mut self,
        source: &ast::Expr<'_, 'src>,
        region: RegionId,
        use_: Use,
    ) -> Result<ExprId, Unsupported> {
        if Self::is_eager_binary(source) {
            return self.eager_binary_expression(source, region, use_);
        }
        let semantics = self.semantics;
        let span = source.span();
        let facts = SourceFacts::get(semantics, source.id).ok_or(Unsupported {
            span,
            feature: "missing checked source occurrence",
        })?;
        let ty = facts.ty;
        let builtin = match facts.resolution {
            Resolution::Builtin(builtin) => Some(builtin),
            _ => None,
        };
        let symbol = match facts.resolution {
            Resolution::Binding(symbol) => Some(symbol),
            _ => None,
        };
        let member = match facts.resolution {
            Resolution::Primitive(operation) => Some(operation),
            _ => None,
        };
        let unsupported = |feature| Unsupported { span, feature };
        let node = match source {
            ast::Expr {
                kind: ast::ExprKind::Int(value, _),
                ..
            } if i32::try_from(*value).is_ok() => Expr::Literal(Literal::Number(*value as f64)),
            ast::Expr {
                kind: ast::ExprKind::Float(value, _),
                ..
            } => Expr::Literal(Literal::Number(*value)),
            ast::Expr {
                kind: ast::ExprKind::String(value, _),
                ..
            } => Expr::Literal(Literal::String(
                StringValue::decode_source(value)
                    .map_err(|_| unsupported("invalid or unsupported source string escape"))?,
            )),
            ast::Expr {
                kind: ast::ExprKind::Template { parts, .. },
                ..
            } => {
                let mut lowered = Vec::with_capacity(parts.len());
                for part in *parts {
                    lowered.push(match part {
                        ast::TemplatePart::String(value, span) => TemplatePart::String(
                            StringValue::decode_template(value).map_err(|_| Unsupported {
                                span: *span,
                                feature: "invalid or unsupported template escape",
                            })?,
                        ),
                        ast::TemplatePart::Expr(value) => {
                            TemplatePart::Expression(self.expression(value, region)?)
                        }
                    });
                }
                match lowered.as_slice() {
                    [] => Expr::Literal(Literal::String("".into())),
                    [TemplatePart::String(_)] => {
                        let TemplatePart::String(value) = lowered.pop().unwrap() else {
                            unreachable!()
                        };
                        Expr::Literal(Literal::String(value))
                    }
                    _ => Expr::Template(lowered),
                }
            }
            ast::Expr {
                kind: ast::ExprKind::Bool(value, _),
                ..
            } => Expr::Literal(Literal::Bool(*value)),
            ast::Expr {
                kind: ast::ExprKind::Null(_),
                ..
            } => Expr::Literal(Literal::Null),
            ast::Expr {
                kind: ast::ExprKind::Ident(_),
                ..
            } => self.identifier_node(
                symbol.expect("identifier resolved before construction"),
                span,
            )?,
            ast::Expr {
                kind: ast::ExprKind::Unary { op, expr, .. },
                ..
            } => {
                let value = self.expression(expr, region)?;
                if *op == ast::UnaryOp::Neg && ty == Some(&Type::Int) {
                    Expr::IntNegate(value)
                } else {
                    Expr::Unary {
                        op: match op {
                            ast::UnaryOp::Neg => Unary::Negate,
                            ast::UnaryOp::Not => Unary::Not,
                        },
                        value,
                    }
                }
            }
            ast::Expr {
                kind: ast::ExprKind::Binary { op, lhs, rhs, .. },
                ..
            } => {
                let op = Self::binary_operator(*op);
                let left = self.expression(lhs, region)?;
                let right = self.expression(rhs, region)?;
                Self::arithmetic(op, ty, left, right)
            }
            ast::Expr {
                kind:
                    ast::ExprKind::If {
                        condition,
                        then_value,
                        else_value,
                        ..
                    },
                ..
            } => Expr::Conditional {
                condition: self.expression(condition, region)?,
                yes: self.expression(then_value, region)?,
                no: self.expression(else_value, region)?,
            },
            ast::Expr {
                kind:
                    ast::ExprKind::Assignment {
                        op: AssignmentOp::Assign,
                        target,
                        value,
                        ..
                    },
                ..
            } => Expr::Assign {
                target: self.expression_in(target, region, Use::Place)?,
                value: if let ast::Expr {
                    kind: ast::ExprKind::Ident(target),
                    ..
                } = target
                {
                    self.named_expression(value, region, target.name)?
                } else {
                    self.expression(value, region)?
                },
            },
            ast::Expr {
                kind:
                    ast::ExprKind::Assignment {
                        op, target, value, ..
                    },
                ..
            } => {
                let CapturedPlace {
                    before,
                    target: reference,
                    old,
                } = self.capture_place(target, region)?;
                let result = if *op == AssignmentOp::Nullish {
                    let value = if let ast::Expr {
                        kind: ast::ExprKind::Ident(ident),
                        ..
                    } = target
                    {
                        self.named_expression(value, region, ident.name)?
                    } else {
                        self.expression(value, region)?
                    };
                    let store = self.module.expression(
                        Expr::Assign {
                            target: reference,
                            value,
                        },
                        None,
                    );
                    Expr::Binary {
                        op: Binary::Nullish,
                        left: old,
                        right: store,
                    }
                } else {
                    let op = match op {
                        AssignmentOp::Add => Binary::Add,
                        AssignmentOp::Sub => Binary::Subtract,
                        AssignmentOp::Mul => Binary::Multiply,
                        AssignmentOp::Div => Binary::Divide,
                        AssignmentOp::Mod => Binary::Remainder,
                        AssignmentOp::BitAnd => Binary::BitAnd,
                        AssignmentOp::BitOr => Binary::BitOr,
                        AssignmentOp::Xor => Binary::BitXor,
                        AssignmentOp::ShiftLeft => Binary::ShiftLeft,
                        AssignmentOp::ShiftRight => Binary::ShiftRight,
                        AssignmentOp::UnsignedShiftRight => Binary::UnsignedShiftRight,
                        AssignmentOp::Assign | AssignmentOp::Nullish => unreachable!(),
                    };
                    let right = self.expression(value, region)?;
                    let value = self
                        .module
                        .expression(Self::arithmetic(op, ty, old, right), None);
                    Expr::Assign {
                        target: reference,
                        value,
                    }
                };
                self.sequence(before, result)
            }
            ast::Expr {
                kind:
                    ast::ExprKind::Update {
                        target, op, prefix, ..
                    },
                ..
            } => {
                let place = self.capture_place(target, region)?;
                self.update_place(
                    place,
                    match op {
                        ast::UpdateOp::Increment => Binary::Add,
                        ast::UpdateOp::Decrement => Binary::Subtract,
                    },
                    ty,
                    !prefix && use_ != Use::Discarded,
                    region,
                )
            }
            ast::Expr {
                kind:
                    ast::ExprKind::Member {
                        object, property, ..
                    },
                ..
            } if matches!(semantics.expression_type(object.id), Some(Type::Record(_))) => {
                Expr::Member {
                    object: self.expression(object, region)?,
                    property: Property::Named(property.name.into()),
                }
            }
            ast::Expr {
                kind: ast::ExprKind::Index { object, index, .. },
                ..
            } => self.index_reference(object, index, region, span)?,
            ast::Expr {
                kind: ast::ExprKind::Member { object, .. },
                ..
            } if matches!(facts.resolution, Resolution::NominalMember(_)) => {
                self.nominal_field(source, object, region)?
            }
            ast::Expr {
                kind:
                    ast::ExprKind::Member {
                        object, property, ..
                    },
                ..
            } if matches!(
                self.semantics.expression_type(object.id),
                Some(
                    Type::String
                        | Type::Array(_)
                        | Type::Map(_, _)
                        | Type::Set(_)
                        | Type::ArrayBuffer
                        | Type::SharedArrayBuffer
                )
            ) || semantics
                .expression_type(object.id)
                .is_some_and(crate::typed_array::is_typed_array_type) =>
            {
                if matches!(ty, Some(Type::Function(_))) && use_ != Use::CallTarget {
                    return Err(unsupported("primitive methods require a receiver call"));
                }
                let receiver = self.expression(object, region)?;
                match member {
                    Some(ResolvedIntrinsic::Property(operation))
                        if intrinsic_arity(operation).is_some() =>
                    {
                        Expr::Intrinsic {
                            operation,
                            receiver,
                            arguments: Vec::new(),
                        }
                    }
                    _ => Expr::Member {
                        object: receiver,
                        property: Property::Named(property.name.into()),
                    },
                }
            }
            ast::Expr {
                kind:
                    ast::ExprKind::Call {
                        callee:
                            ast::Expr {
                                kind: ast::ExprKind::Member { object, .. },
                                ..
                            },
                        args,
                        ..
                    },
                ..
            } if matches!(member, Some(ResolvedIntrinsic::Method(operation)) if intrinsic_arity(operation).is_some()) =>
            {
                let Some(ResolvedIntrinsic::Method(operation)) = member else {
                    unreachable!()
                };
                Expr::Intrinsic {
                    operation,
                    receiver: self.expression(object, region)?,
                    arguments: args
                        .iter()
                        .map(|argument| self.expression(&argument.expression, region))
                        .collect::<Result<_, _>>()?,
                }
            }
            ast::Expr {
                kind: ast::ExprKind::Call { .. },
                ..
            } if builtin == Some(BuiltinCall::JsUndefined) => Expr::Literal(Literal::Undefined),
            ast::Expr {
                kind: ast::ExprKind::Call { callee, args, .. },
                ..
            } if matches!(facts.resolution, Resolution::NominalMember(member)
                if matches!(semantics.nominal_member(member),
                    Some(crate::semantic::NominalMember::Method { .. }))) =>
            {
                let Resolution::NominalMember(member) = facts.resolution else {
                    unreachable!()
                };
                self.nominal_call(callee, member, args, region)?
            }
            ast::Expr {
                kind: ast::ExprKind::Call { callee, args, .. },
                ..
            } => {
                let source_callee = *callee;
                let (callee, invocation) = match builtin {
                    Some(builtin @ (BuiltinCall::Print | BuiltinCall::MathImul)) => {
                        let (namespace, method) = if builtin == BuiltinCall::Print {
                            ("console", "log")
                        } else {
                            ("Math", "imul")
                        };
                        let object = self.module.expression(Expr::Host(namespace.into()), None);
                        (
                            self.module.expression(
                                Expr::Member {
                                    object,
                                    property: Property::Named(method.into()),
                                },
                                None,
                            ),
                            Invocation::Reference,
                        )
                    }
                    Some(_) => {
                        return Err(unsupported(
                            "builtin operation outside the initial source slice",
                        ));
                    }
                    None => {
                        let invocation = if matches!(
                            callee,
                            ast::Expr {
                                kind: ast::ExprKind::Member { .. },
                                ..
                            } | ast::Expr {
                                kind: ast::ExprKind::Index { .. },
                                ..
                            }
                        ) {
                            Invocation::Reference
                        } else {
                            Invocation::Value
                        };
                        (
                            self.expression_in(callee, region, Use::CallTarget)?,
                            invocation,
                        )
                    }
                };
                let arguments = self.call_arguments(source_callee, args, region)?;
                Expr::Call {
                    callee,
                    arguments,
                    invocation,
                }
            }
            ast::Expr {
                kind: ast::ExprKind::New { args, .. },
                ..
            } if matches!(facts.resolution, Resolution::NominalConstruction(_)) => {
                let Resolution::NominalConstruction(owner) = facts.resolution else {
                    unreachable!()
                };
                self.construct_class(source, owner, args, region)?
            }
            ast::Expr {
                kind: ast::ExprKind::New { args, .. },
                ..
            } => {
                let Some(ResolvedIntrinsic::Constructor(operation)) = member else {
                    return Err(unsupported(
                        "class construction outside the supported source slice",
                    ));
                };
                if native_constructor(operation).is_none() {
                    return Err(unsupported(
                        "primitive construction outside the supported source slice",
                    ));
                }
                Expr::ConstructIntrinsic {
                    operation,
                    arguments: args
                        .iter()
                        .map(|value| self.expression(&value.expression, region))
                        .collect::<Result<_, _>>()?,
                }
            }
            ast::Expr {
                kind: ast::ExprKind::ArrayLiteral { elements, .. },
                ..
            } => Expr::Array(
                elements
                    .iter()
                    .map(|element| {
                        if let ArrayElement::Value(value) = element {
                            self.expression(value, region)
                        } else {
                            Err(unsupported("array spread"))
                        }
                    })
                    .collect::<Result<_, _>>()?,
            ),
            ast::Expr {
                kind: ast::ExprKind::RecordLiteral { entries, .. },
                ..
            } => {
                // A materialized record has no inherited keys. An actual
                // __proto__ entry is a computed data key, separate from the
                // prototype-setting syntax used for the representation.
                let null = self.module.expression(Expr::Literal(Literal::Null), None);
                let mut properties = Vec::with_capacity(entries.len() + 1);
                properties.push((Property::Named("__proto__".into()), null));
                for entry in *entries {
                    let RecordElement::Entry(entry) = entry else {
                        return Err(unsupported("record spread"));
                    };
                    let name = StringValue::decode_source(entry.key.name)
                        .map_err(|_| unsupported("invalid record key escape"))?;
                    let key = match name.as_unicode() {
                        Some(name) if name != "__proto__" && identifier_name(name) => {
                            Property::Named(name.into())
                        }
                        _ => Property::Computed(
                            self.module
                                .expression(Expr::Literal(Literal::String(name.clone())), None),
                        ),
                    };
                    let value = self.expression(&entry.value, region)?;
                    if let Expr::Function(function) = self.module.expressions[value.index()] {
                        self.module.functions[function.index()].name = FunctionName::Exact(name);
                    }
                    properties.push((key, value));
                }
                Expr::Object(properties)
            }
            ast::Expr {
                kind: ast::ExprKind::ArrowFunction { params, body, .. },
                ..
            } => {
                let region = self
                    .module
                    .region(self.module.regions[region.index()].scope);
                let checkpoint = self.bound_symbols.len();
                let constructor = self.constructor.take();
                let parameters = self.parameters(params, region)?;
                match body {
                    ArrowBody::Block(body) => self.statements(body, region)?,
                    ArrowBody::Expr(value) => {
                        let value = self.expression(value, region)?;
                        self.push(region, Statement::Return(Some(value)));
                    }
                }
                self.restore_bindings(checkpoint);
                self.constructor = constructor;
                let id = FunctionId::new(self.module.functions.len());
                self.module.functions.push(Function {
                    parameters,
                    body: region,
                    arrow: true,
                    name: FunctionName::Exact(StringValue::default()),
                    strict: false,
                    length: None,
                    suspension: crate::structured_js::Suspension::None,
                });
                Expr::Function(id)
            }
            _ => return Err(unsupported("expression outside the initial source slice")),
        };
        self.materialize(source, node, facts, use_)
    }

    fn materialize(
        &mut self,
        source: &ast::Expr<'_, 'src>,
        node: Expr,
        facts: SourceFacts<'sem, 'src>,
        use_: Use,
    ) -> Result<ExprId, Unsupported> {
        let origin = source.id;
        let ty = facts.ty;
        let semantics = self.semantics;
        let unsupported = |feature| Unsupported {
            span: source.span(),
            feature,
        };
        if use_ == Use::Place {
            if !matches!(node, Expr::Binding(_) | Expr::Member { .. }) {
                return Err(unsupported(
                    "assignment requires a supported mutable reference",
                ));
            }
            // Forming a place does not read it. A record write must not acquire
            // the nullable-read fallback, and an indexed int write must not
            // turn its target into a converted value.
            return Ok(self.module.expression(node, Some(origin)));
        }
        if use_ == Use::CallTarget && matches!(node, Expr::Member { .. }) {
            // A receiver call needs the property reference itself. The checker
            // has already established callability; nullable value materialization
            // here would discard the receiver before invocation.
            return Ok(self.module.expression(node, Some(origin)));
        }
        let receiver_type = match source {
            ast::Expr {
                kind: ast::ExprKind::Member { object, .. },
                ..
            }
            | ast::Expr {
                kind: ast::ExprKind::Index { object, .. },
                ..
            } => semantics.expression_type(object.id),
            _ => None,
        };
        // A checked Map.get has a nullable language result. Its raw native
        // call can return undefined; that is not a source null value yet.
        // Resolve this obligation by operation identity, never by the name get.
        let nullable_get = matches!(
            facts.resolution,
            Resolution::Primitive(ResolvedIntrinsic::Method(Intrinsic::MapGet))
        ) && matches!(node, Expr::Call { .. } | Expr::Intrinsic { .. });
        let absent = if nullable_get || matches!(receiver_type, Some(Type::Record(_))) {
            Some(Literal::Null)
        } else if matches!(
            source,
            ast::Expr {
                kind: ast::ExprKind::Index { .. },
                ..
            }
        ) && ty == Some(&Type::String)
        {
            Some(Literal::String("".into()))
        } else {
            None
        };
        if let Some(absent) = absent {
            let raw = self.module.expression(node, None);
            let fallback = self.module.expression(Expr::Literal(absent), None);
            return Ok(self.module.expression(
                Expr::Binary {
                    op: Binary::Nullish,
                    left: raw,
                    right: fallback,
                },
                Some(origin),
            ));
        }
        // A source int is the result obligation, not proof that a JavaScript
        // primitive member already produced an i32. For example charCodeAt
        // returns NaN out of bounds and pop may return undefined. The semantic
        // value is their signed normalization; only that result gets the source
        // type proof. The raw implementation call/member has no such proof.
        // Declared extern and LilScript function results keep their typed ABI.
        let normalize_member = !matches!(node, Expr::Intrinsic { .. })
            && ty == Some(&Type::Int)
            && (matches!(
                source,
                ast::Expr {
                    kind: ast::ExprKind::Member { .. },
                    ..
                } | ast::Expr {
                    kind: ast::ExprKind::Index { .. },
                    ..
                }
            ) || matches!(source, ast::Expr { kind: ast::ExprKind::Call { callee, .. }, .. }
                    if matches!(callee, ast::Expr { kind: ast::ExprKind::Member { .. }, .. } | ast::Expr { kind: ast::ExprKind::Index { .. }, .. })));
        if normalize_member {
            let raw = self.module.expression(node, None);
            Ok(self.module.expression(Expr::ToInt32(raw), Some(origin)))
        } else {
            Ok(self.module.expression(node, Some(origin)))
        }
    }
}

#[cfg(test)]
mod binary_worklist_tests {
    use super::*;
    use bumpalo::Bump;

    #[test]
    fn deep_eager_trees_preserve_each_occurrence_and_signed_result_obligation() {
        for nested_on_left in [true, false] {
            let arena = Bump::new();
            let prefix = crate::parser::parse_source(&arena, "int seed=7;").unwrap();
            let nodes = ast::SourceNodes::continuing(prefix.source_identity());
            let span = Span::empty(12);
            let mut source =
                nodes.expression(ast::ExprKind::Ident(ast::Ident { name: "seed", span }));
            let mut binary_ids = Vec::new();
            for _ in 0..4096 {
                let literal = arena.alloc(nodes.expression(ast::ExprKind::Int(1, span)));
                let nested = arena.alloc(source);
                let (lhs, rhs) = if nested_on_left {
                    (&*nested, &*literal)
                } else {
                    (&*literal, &*nested)
                };
                source = nodes.expression(ast::ExprKind::Binary {
                    op: ast::BinaryOp::Add,
                    lhs,
                    rhs,
                    span,
                });
                binary_ids.push(source.id);
            }
            let mut items = prefix.items.to_vec();
            items.push(Item::Stmt(Stmt::Expr(source)));
            let program = prefix.with_items(&nodes, arena.alloc_slice_fill_iter(items));
            let semantics = crate::semantic::analyze(&program).unwrap();
            let lowered = lower_slice(&program, &semantics).unwrap();
            let target = lowered.target();
            let mut seen = vec![false; program.source_identity().len()];
            let mut count = 0;
            for (index, node) in target.expressions.iter().enumerate() {
                if let Expr::IntBinary { op, left, right } = node {
                    assert_eq!(*op, IntBinary::Add);
                    assert!(left.index() < index && right.index() < index);
                    let origin = target.origins[index].expect("source arithmetic keeps its origin");
                    assert_eq!(semantics.expression_type(origin), Some(&Type::Int));
                    assert!(!std::mem::replace(&mut seen[origin.index()], true));
                    count += 1;
                }
            }
            assert_eq!(count, binary_ids.len());
            assert!(binary_ids.iter().all(|id| seen[id.index()]));
        }
    }
}
