use crate::literal::StringValue;
use crate::semantic::{EscapeState, SymbolId, Type};
use crate::span::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FunctionId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(pub u32);

/// Stable identity for a lowered source operation. `Span` remains diagnostic
/// location only; search and explicit-lowering proofs must not key on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u32);

/// Whether an IR operation was authored in source or introduced by a later
/// transform. Shadow-mode provenance: v0.1 `|0` spelling is still owned by
/// `LoweringObligation`, not by this tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum OperationOrigin {
    Source,
    #[default]
    Generated,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConstValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(StringValue),
    Null,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IrBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    BitAnd,
    BitOr,
    Xor,
    ShiftLeft,
    ShiftRight,
    UnsignedShiftRight,
    Eq,
    NotEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IrUnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    Const {
        out: ValueId,
        value: ConstValue,
        span: Span,
    },
    Binary {
        out: ValueId,
        op: IrBinaryOp,
        lhs: ValueId,
        rhs: ValueId,
        span: Span,
    },
    Struct {
        out: ValueId,
        fields: Vec<ValueId>,
        span: Span,
    },
    FieldGet {
        out: ValueId,
        aggregate: ValueId,
        index: usize,
        span: Span,
    },
    Call {
        out: Option<ValueId>,
        callee: FunctionId,
        args: Vec<ValueId>,
        span: Span,
    },
    Return {
        value: Option<ValueId>,
        span: Span,
    },
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct BasicBlock {
    pub id: Option<BlockId>,
    pub instructions: Vec<Instruction>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct IrFunction {
    pub id: Option<FunctionId>,
    pub blocks: Vec<BasicBlock>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct IrModule {
    pub functions: Vec<IrFunction>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ControlFlowModule<'src> {
    pub functions: Vec<ControlFlowFunction<'src>>,
    /// String values authored inside an `@pool` region, collected at lowering
    /// time and keyed by value rather than by location.
    ///
    /// Location does not survive the optimizer: an `@pool` function is a normal
    /// inlining candidate, and once its body is folded into a caller the region
    /// it was written in no longer exists. Keying by value is also the exact
    /// granularity the pool works at — one alias serves every occurrence of a
    /// literal, so there is no such thing as pooling half of them.
    pub pinned_pool_strings: std::collections::BTreeSet<StringValue>,
    pub globals: Vec<IrGlobal<'src>>,
    pub foreign_imports: Vec<IrForeignImport<'src>>,
    pub js_host_aliases: Vec<JsHostAlias>,
    pub exports: Vec<IrExport<'src>>,
    pub lazy_modules: Vec<IrLazyModule<'src>>,
    pub structs: Vec<AggregateLayout<'src>>,
    pub classes: Vec<AggregateLayout<'src>>,
    pub entry: FunctionId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrForeignImport<'src> {
    pub source: &'src str,
    pub specifiers: Vec<IrForeignImportSpecifier<'src>>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IrForeignImportSpecifier<'src> {
    pub imported: &'src str,
    pub local: &'src str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsHostAliasConvention {
    Callee,
    MethodCall,
    BoundMethodCall,
    Apply,
    BoundApply,
    Throw,
    ThrowConstruct,
}

impl JsHostAliasConvention {
    pub fn emits_binding(self) -> bool {
        !matches!(self, Self::Throw | Self::ThrowConstruct)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsHostAlias {
    pub function: FunctionId,
    pub spelling: &'static str,
    pub convention: JsHostAliasConvention,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrLazyModule<'src> {
    pub id: u32,
    pub source: &'src str,
    pub exports: Vec<IrExport<'src>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrExport<'src> {
    pub name: &'src str,
    pub binding: ExportBinding,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportBinding {
    Function(FunctionId),
    Global(SymbolId),
    TypeOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateLayout<'src> {
    pub name: &'src str,
    pub base: Option<&'src str>,
    pub fields: Vec<AggregateField<'src>>,
    pub object: bool,
    pub external: bool,
    /// Constructor identity is a published JS fact (`new`, `.name`, `.prototype`).
    /// Identity-free classes stay dissolved; this mark forces a named `class`.
    pub identity_observed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateField<'src> {
    pub name: &'src str,
    pub ty: Type<'src>,
    pub index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrGlobal<'src> {
    pub symbol: SymbolId,
    pub name: &'src str,
    pub ty: Type<'src>,
    pub external: bool,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionKind<'src> {
    Entry,
    Function,
    Method { class: &'src str },
    Constructor { class: &'src str },
    Closure,
    Extern,
}

/// Records how a control-flow function entered the IR. Compression-generated
/// boundaries can require different candidate-search treatment from source
/// functions even though their runtime calling convention is identical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunctionOrigin {
    Source,
    Synthesized,
    RepeatedRegionOutline,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ControlFlowFunction<'src> {
    pub id: FunctionId,
    pub name: Option<&'src str>,
    pub kind: FunctionKind<'src>,
    pub origin: FunctionOrigin,
    /// Behaviour pinned to this function by an `@` attribute in source. Survives
    /// the project objective; see `ast::RegionPolicy`.
    pub region: crate::ast::RegionPolicy,
    pub declared_pure: bool,
    pub is_async: bool,
    pub is_generator: bool,
    pub params: Vec<IrParameter<'src>>,
    pub capture_count: usize,
    /// Locals whose lexical binding is shared with one or more closures.
    /// Capture parameters in this set receive a cell from their environment;
    /// defining-function locals own the cell.
    pub mutable_capture_locals: Vec<LocalId>,
    pub return_type: Type<'src>,
    pub locals: Vec<IrLocal<'src>>,
    pub blocks: Vec<ControlFlowBlock<'src>>,
    pub shapes: Vec<ControlShape>,
    pub entry: BlockId,
    pub value_count: u32,
    pub value_local_hints: Vec<Option<&'src str>>,
    pub value_escapes: Vec<EscapeState>,
    pub locals_promoted: bool,
    pub live: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlShape {
    If {
        header: BlockId,
        then_block: BlockId,
        else_block: BlockId,
        merge_block: BlockId,
    },
    Loop {
        header: BlockId,
        body: BlockId,
        update: Option<BlockId>,
        exit: BlockId,
    },
    ForIn {
        header: BlockId,
        body: BlockId,
        exit: BlockId,
        object: ValueId,
        key: ValueId,
    },
    ForOf {
        header: BlockId,
        body: BlockId,
        exit: BlockId,
        iterable: ValueId,
        element: ValueId,
    },
    Try {
        header: BlockId,
        body: BlockId,
        catch_block: Option<BlockId>,
        finally_block: Option<BlockId>,
        merge_block: BlockId,
        catch_value: Option<ValueId>,
    },
}

impl ControlShape {
    pub const fn header(&self) -> BlockId {
        match self {
            Self::If { header, .. }
            | Self::Loop { header, .. }
            | Self::ForIn { header, .. }
            | Self::ForOf { header, .. }
            | Self::Try { header, .. } => *header,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum IrParamDefault {
    Const(ConstValue),
    Value(ValueId),
    /// The exact unshadowed `JS.undefined()` language primitive. JavaScript
    /// omission already supplies this value, so codegen emits no initializer.
    Undefined,
    /// The source parameter has a semantic default, but neutral IR cannot
    /// reproduce that value as a JavaScript parameter initializer. Typed call
    /// sites materialize it instead. JavaScript emission must therefore keep
    /// this marker out of the parameter list and reject surviving root/lazy
    /// exports. Address-taken values retain required plain formals under their
    /// typed function contract.
    CallerMaterialized,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IrParameter<'src> {
    pub symbol: SymbolId,
    pub local: LocalId,
    pub value: ValueId,
    pub name: &'src str,
    pub ty: Type<'src>,
    pub default: Option<IrParamDefault>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrLocal<'src> {
    pub id: LocalId,
    pub symbol: SymbolId,
    pub name: &'src str,
    pub ty: Type<'src>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ControlFlowBlock<'src> {
    pub id: BlockId,
    pub phis: Vec<Phi<'src>>,
    pub instructions: Vec<ControlFlowInstruction<'src>>,
    pub terminator: Option<Terminator>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phi<'src> {
    pub out: ValueId,
    pub origin: PhiOrigin,
    pub ty: Type<'src>,
    pub incoming: Vec<(BlockId, ValueId)>,
    pub span: Span,
}

/// Why an SSA merge exists. Backends use this semantic provenance to retain
/// lazy source expressions as expressions instead of first materializing them
/// as mutable locals. `Synthetic` covers control-flow joins introduced by
/// transformations such as inlining, where no source-local or expression
/// identity may be assumed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhiOrigin {
    Local(LocalId),
    Expression(ExpressionPhi),
    Synthetic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExpressionPhi {
    Conditional,
    ShortCircuit {
        op: ShortCircuitOp,
        lhs: ValueId,
    },
    Nullish {
        lhs: ValueId,
    },
    /// A nullable receiver selection introduced by `receiver?.member` or
    /// `receiver?.[index]`. The incoming present value is the access result;
    /// the other incoming value is either canonical `null` or the directly
    /// fused right-hand side of `??`.
    OptionalAccess {
        object: ValueId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShortCircuitOp {
    BooleanAnd,
    BooleanOr,
    JavaScriptAnd,
    JavaScriptOr,
}

impl PhiOrigin {
    pub const fn local(&self) -> Option<LocalId> {
        match self {
            Self::Local(local) => Some(*local),
            Self::Expression(_) | Self::Synthetic => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ControlFlowInstruction<'src> {
    pub out: Option<ValueId>,
    pub ty: Option<Type<'src>>,
    pub op: ControlFlowOp<'src>,
    pub lowering_obligation: LoweringObligation,
    pub origin: OperationOrigin,
    pub node_id: Option<NodeId>,
    pub span: Span,
}

impl<'src> ControlFlowInstruction<'src> {
    pub fn generated(
        out: Option<ValueId>,
        ty: Option<Type<'src>>,
        op: ControlFlowOp<'src>,
        span: Span,
    ) -> Self {
        Self {
            out,
            ty,
            op,
            lowering_obligation: LoweringObligation::Free,
            origin: OperationOrigin::Generated,
            node_id: None,
            span,
        }
    }

    pub fn source(
        out: Option<ValueId>,
        ty: Option<Type<'src>>,
        op: ControlFlowOp<'src>,
        lowering_obligation: LoweringObligation,
        span: Span,
        node_id: NodeId,
    ) -> Self {
        Self {
            out,
            ty,
            op,
            lowering_obligation,
            origin: OperationOrigin::Source,
            node_id: Some(node_id),
            span,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LoweringObligation {
    #[default]
    Free,
    PreserveJavaScriptBitOrZero,
}

impl ControlFlowModule<'_> {
    /// A hand-built or externally edited legacy IR must not bypass the source
    /// boundary's rejection of signatures the value-only IR cannot implement.
    /// This borrows its existing type owners and retains no scan view.
    pub(crate) fn reference_parameter_span(&self) -> Option<Span> {
        for global in &self.globals {
            if global.ty.contains_mutable_reference_parameters() {
                return Some(global.span);
            }
        }
        for function in &self.functions {
            let types = std::iter::once(&function.return_type)
                .chain(function.params.iter().map(|parameter| &parameter.ty))
                .chain(function.locals.iter().map(|local| &local.ty))
                .chain(
                    function
                        .blocks
                        .iter()
                        .flat_map(|block| block.phis.iter().map(|phi| &phi.ty)),
                )
                .chain(function.blocks.iter().flat_map(|block| {
                    block
                        .instructions
                        .iter()
                        .filter_map(|instruction| instruction.ty.as_ref())
                }));
            if types
                .into_iter()
                .any(Type::contains_mutable_reference_parameters)
            {
                return Some(function.span);
            }
            for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
                if let ControlFlowOp::TypeCheck { target, .. } = &instruction.op {
                    if target.contains_mutable_reference_parameters() {
                        return Some(instruction.span);
                    }
                }
            }
        }
        for layout in self.structs.iter().chain(&self.classes) {
            if layout
                .fields
                .iter()
                .any(|field| field.ty.contains_mutable_reference_parameters())
            {
                return Some(
                    self.functions
                        .get(self.entry.0 as usize)
                        .map_or(Span::default(), |function| function.span),
                );
            }
        }
        None
    }

    pub fn runtime_exported_functions(&self) -> Vec<FunctionId> {
        let mut exported = self
            .exports
            .iter()
            .chain(
                self.lazy_modules
                    .iter()
                    .flat_map(|module| module.exports.iter()),
            )
            .filter_map(|export| match export.binding {
                ExportBinding::Function(function) => Some(function),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut public_classes = exported
            .iter()
            .filter_map(|id| self.functions.get(id.0 as usize))
            .filter_map(|function| match function.kind {
                FunctionKind::Constructor { class } if self.class_identity_observed(class) => {
                    Some(class)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        loop {
            let inherited = public_classes
                .iter()
                .filter_map(|name| {
                    self.classes
                        .iter()
                        .find(|layout| layout.name == *name)
                        .and_then(|layout| layout.base)
                })
                .filter(|base| !public_classes.contains(base))
                .collect::<Vec<_>>();
            if inherited.is_empty() {
                break;
            }
            public_classes.extend(inherited);
        }
        for function in &self.functions {
            if matches!(function.kind,
                FunctionKind::Constructor { class } | FunctionKind::Method { class }
                    if public_classes.contains(&class))
                && !exported.contains(&function.id)
            {
                exported.push(function.id);
            }
        }
        exported
    }

    pub fn has_explicit_lowering_obligations(&self) -> bool {
        self.live_instructions()
            .any(|instruction| instruction.lowering_obligation != LoweringObligation::Free)
    }

    pub fn lowering_obligation_count(&self, obligation: LoweringObligation) -> usize {
        self.live_instructions()
            .filter(|instruction| instruction.lowering_obligation == obligation)
            .count()
    }

    pub fn operation_provenance_counts(&self) -> (usize, usize) {
        let mut source = 0usize;
        let mut generated = 0usize;
        for instruction in self.live_instructions() {
            match instruction.origin {
                OperationOrigin::Source => source += 1,
                OperationOrigin::Generated => generated += 1,
            }
        }
        (source, generated)
    }

    pub fn class_identity_observed(&self, name: &str) -> bool {
        self.classes
            .iter()
            .any(|layout| layout.name == name && layout.identity_observed)
    }

    pub fn function_belongs_to_identity_class(&self, function: &ControlFlowFunction<'_>) -> bool {
        match function.kind {
            FunctionKind::Constructor { class } | FunctionKind::Method { class } => {
                self.class_identity_observed(class)
            }
            _ => false,
        }
    }

    fn live_instructions(&self) -> impl Iterator<Item = &ControlFlowInstruction<'_>> {
        self.functions
            .iter()
            .filter(|function| function.live)
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
    }
}

/// The method name of a `HostCall` that is really a host-class `super(...)`.
///
/// An internal class may extend a host (`extern`) class such as `Error`. Its
/// constructor must call the host constructor with `super(...)` before touching
/// `this`, but there is no LilScript function to call. The call rides on a
/// non-pure `HostCall` on `this`, which every analysis already treats as an
/// opaque, non-removable effect on its receiver — exactly what `super(...)` is —
/// and only the JavaScript emitter reads the name. It contains a NUL, so no real
/// host method can collide with it.
pub const HOST_SUPER_METHOD: &str = "\u{0}super";

#[derive(Debug, Clone, PartialEq)]
pub enum ControlFlowOp<'src> {
    Const(ConstValue),
    CaughtException,
    Unary {
        op: IrUnaryOp,
        value: ValueId,
    },
    Binary {
        op: IrBinaryOp,
        lhs: ValueId,
        rhs: ValueId,
    },
    TypeCheck {
        value: ValueId,
        target: Type<'src>,
    },
    Await {
        task: ValueId,
    },
    Array(Vec<ValueId>),
    ArraySpread(Vec<ArrayOperand>),
    Record(Vec<(&'src str, ValueId)>),
    RecordSpread(Vec<RecordOperand<'src>>),
    Struct {
        name: &'src str,
        fields: Vec<ValueId>,
    },
    NewClass {
        class: &'src str,
        constructor: Option<FunctionId>,
        args: Vec<ValueId>,
    },
    Closure {
        function: FunctionId,
        captures: Vec<ValueId>,
    },
    /// Produces the lexical-cell operand for a mutable closure capture. The
    /// JavaScript backend renders the local binding directly; native code
    /// passes the local's shared cell pointer through the closure environment.
    CaptureLocal(LocalId),
    LoadLocal(LocalId),
    StoreLocal {
        local: LocalId,
        value: ValueId,
    },
    LoadGlobal(SymbolId),
    StoreGlobal {
        global: SymbolId,
        value: ValueId,
    },
    FieldGet {
        object: ValueId,
        owner: &'src str,
        field: &'src str,
        index: usize,
    },
    FieldSet {
        object: ValueId,
        owner: &'src str,
        field: &'src str,
        index: usize,
        value: ValueId,
    },
    RecordFieldGet {
        object: ValueId,
        property: &'src str,
    },
    RecordFieldSet {
        object: ValueId,
        property: &'src str,
        value: ValueId,
    },
    RecordRest {
        object: ValueId,
        excluded: Vec<&'src str>,
    },
    HostFieldGet {
        object: ValueId,
        property: &'src str,
    },
    HostFieldSet {
        object: ValueId,
        property: &'src str,
        value: ValueId,
    },
    IndexGet {
        object: ValueId,
        index: ValueId,
    },
    ArrayGetOptional {
        object: ValueId,
        index: usize,
    },
    IndexSet {
        object: ValueId,
        index: ValueId,
        value: ValueId,
    },
    CallDirect {
        function: FunctionId,
        args: Vec<ValueId>,
        /// Number of arguments written at the source call site before typed
        /// defaults were materialized. Target projections may use this to
        /// encode defaults at the callee without guessing from equal values.
        provided_args: usize,
    },
    CallValue {
        callee: ValueId,
        args: Vec<ValueId>,
    },
    CallMethod {
        receiver: ValueId,
        class: &'src str,
        method: &'src str,
        function: FunctionId,
        args: Vec<ValueId>,
    },
    HostCall {
        receiver: ValueId,
        method: &'src str,
        args: Vec<ValueId>,
        pure: bool,
    },
    DynamicImport {
        module: u32,
    },
    Intrinsic {
        intrinsic: Intrinsic,
        receiver: Option<ValueId>,
        args: Vec<ValueId>,
    },
    Template(Vec<TemplateOperand>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TemplateOperand {
    String(StringValue),
    Value(ValueId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayOperand {
    Value(ValueId),
    Spread(ValueId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordOperand<'src> {
    Entry(&'src str, ValueId),
    Spread(ValueId),
}

// One language operation identity is shared by every backend.
pub use crate::primitive::Intrinsic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Terminator {
    Jump(BlockId),
    Branch {
        condition: ValueId,
        then_block: BlockId,
        else_block: BlockId,
    },
    Try {
        body: BlockId,
        catch_block: Option<BlockId>,
    },
    Return(Option<ValueId>),
    Throw(ValueId),
    Unreachable,
}

#[cfg(test)]
mod reference_parameter_tests {
    use super::*;
    use crate::primitive::ParameterPassing;
    use crate::semantic::{FunctionParameter, FunctionSignature, FunctionType};

    #[test]
    fn legacy_emitters_reject_reference_signatures_in_existing_ir_type_owners() {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, "print(1);").unwrap();
        let checked = crate::analyze(&syntax).unwrap();
        let mut module = crate::lower::lower_to_control_flow(&syntax, &checked).unwrap();
        crate::optimizer::optimize_control_flow(&mut module).unwrap();
        let signature = Type::Function(FunctionType::new(FunctionSignature {
            params: vec![FunctionParameter {
                ty: Type::Int,
                passing: ParameterPassing::MutableReference,
                default: None,
            }],
            return_type: Box::new(Type::Void),
        }));
        // This public IR entry bypasses source parsing/lowering; even a nested
        // signature in a declared field cannot be silently emitted as a value.
        module.structs.push(AggregateLayout {
            name: "Holder",
            base: None,
            object: false,
            external: false,
            identity_observed: false,
            fields: vec![AggregateField {
                name: "callable",
                index: 0,
                ty: Type::Array(Box::new(signature)),
            }],
        });
        for error in [
            crate::codegen_ir_js::emit_optimized_ir_js(&module).unwrap_err(),
            crate::codegen_ir_js::emit_optimized_ir_js_module(&module).unwrap_err(),
            crate::codegen_native::emit_native_c(&module).unwrap_err(),
        ] {
            assert!(error.message.contains("reference parameters"), "{error}");
        }
    }
}
