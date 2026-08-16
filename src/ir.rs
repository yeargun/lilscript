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

#[derive(Debug, Clone, PartialEq)]
pub enum ConstValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
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
    pub span: Span,
}

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
    String(String),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Intrinsic {
    Print,
    TaskResolve,
    TaskReject,
    TaskAll,
    IntImul,
    IntToString,
    IntToUnsignedString,
    UnwrapNullable,
    UnwrapUnion,
    ArrayLength,
    ArrayMap,
    ArrayFilter,
    ArrayReduce,
    ArrayForEach,
    ArrayPush,
    ArrayPop,
    ArrayIndexOf,
    ArrayIncludes,
    ArrayJoin,
    ArraySome,
    ArrayEvery,
    ArrayFindIndex,
    ArrayConcat,
    ArrayCopyWithin,
    ArrayReverse,
    ArraySlice,
    ArraySplice,
    ArrayFill,
    TypedArraySet,
    TypedArrayFill,
    TypedArrayCopyWithin,
    MapNew,
    MapSize,
    MapGet,
    MapSet,
    MapHas,
    MapDelete,
    MapClear,
    SetNew,
    SetSize,
    SetAdd,
    SetHas,
    SetDelete,
    SetClear,
    RecordKeys,
    RecordValues,
    RecordHasOwn,
    RecordAssign,
    JsonStringify,
    JsonParse,
    ArrayBufferNew,
    SharedArrayBufferNew,
    BufferByteLength,
    BufferSlice,
    Int8ArrayNew,
    Int8ArrayLength,
    Int8ArrayByteLength,
    Int8ArrayByteOffset,
    Int8ArrayBuffer,
    Int8ArraySlice,
    Int8ArraySubarray,
    Uint8ArrayNew,
    Uint8ArrayLength,
    Uint8ArrayByteLength,
    Uint8ArrayByteOffset,
    Uint8ArrayBuffer,
    Uint8ArraySlice,
    Uint8ArraySubarray,
    Uint8ClampedArrayNew,
    Uint8ClampedArrayLength,
    Uint8ClampedArrayByteLength,
    Uint8ClampedArrayByteOffset,
    Uint8ClampedArrayBuffer,
    Uint8ClampedArraySlice,
    Uint8ClampedArraySubarray,
    Int16ArrayNew,
    Int16ArrayLength,
    Int16ArrayByteLength,
    Int16ArrayByteOffset,
    Int16ArrayBuffer,
    Int16ArraySlice,
    Int16ArraySubarray,
    Uint16ArrayNew,
    Uint16ArrayLength,
    Uint16ArrayByteLength,
    Uint16ArrayByteOffset,
    Uint16ArrayBuffer,
    Uint16ArraySlice,
    Uint16ArraySubarray,
    Int32ArrayNew,
    Int32ArrayLength,
    Int32ArrayByteLength,
    Int32ArrayByteOffset,
    Int32ArrayBuffer,
    Int32ArraySlice,
    Int32ArraySubarray,
    Uint32ArrayNew,
    Uint32ArrayLength,
    Uint32ArrayByteLength,
    Uint32ArrayByteOffset,
    Uint32ArrayBuffer,
    Uint32ArraySlice,
    Uint32ArraySubarray,
    Float32ArrayNew,
    Float32ArrayLength,
    Float32ArrayByteLength,
    Float32ArrayByteOffset,
    Float32ArrayBuffer,
    Float32ArraySlice,
    Float32ArraySubarray,
    Float64ArrayNew,
    Float64ArrayLength,
    Float64ArrayByteLength,
    Float64ArrayByteOffset,
    Float64ArrayBuffer,
    Float64ArraySlice,
    Float64ArraySubarray,
    SymbolNew,
    RegexNew,
    RegexTest,
    RegexSource,
    RegexFlags,
    RegexGlobal,
    RegexIgnoreCase,
    RegexMultiline,
    RegexDotAll,
    RegexSticky,
    RegexUnicode,
    FloatAbs,
    FloatFloor,
    FloatCeil,
    FloatRound,
    FloatSqrt,
    FloatSin,
    FloatCos,
    FloatAcos,
    FloatExp,
    FloatLog,
    FloatTan,
    FloatAtan2,
    FloatHypot,
    FloatMin,
    FloatMax,
    FloatToInt,
    StringLength,
    StringCharCodeAt,
    StringCharAt,
    StringIncludes,
    StringIndexOf,
    StringLastIndexOf,
    StringRepeat,
    StringStartsWith,
    StringEndsWith,
    StringToUpperCase,
    StringToLowerCase,
    JsStringSlice,
    JsStringIndexOf,
    JsStringReplace,
    JsStringMatch,
    JsStringSplit,
    JsRegexExec,
    JsTruthy,
    JsIsArray,
    JsIsObject,
    JsPlainObject,
    JsUndefined,
    JsTypeOf,
    JsIsNullish,
    JsIsFalse,
    JsIsUndefined,
    JsStringify,
    JsDateNow,
    JsParseFloat,
    JsParseInt,
    JsIsFinite,
    JsEncodeURIComponent,
    JsObjectCreate,
    JsGetPrototypeOf,
    JsMathPI,
    JsNullProtoObject,
    JsObjectConstructor,
    JsWindow,
    JsDocument,
    JsSetTimeout,
    JsClearTimeout,
    JsDomParserNew,
    JsXMLHttpRequestNew,
    JsNumber,
    JsAdd,
    JsLessThan,
    JsLessThanOrEqual,
    JsGreaterThan,
    JsGreaterThanOrEqual,
    JsStrictEqual,
    JsStrictNotEqual,
    JsCall,
    JsInvoke,
    JsApply,
    JsMethod0,
    JsMethod1,
    JsMethodRest,
    JsStaticRest,
    JsGetProperty,
    JsDeleteProperty,
    JsHasProperty,
    JsInProperty,
    JsBox,
    JsArrayPush,
    JsArrayPop,
    JsArraySlice,
    JsArrayIndexOf,
    JsArraySort,
    JsArraySplice,
    JsArrayConcatApply,
    JsArrayJoin,
    JsArrayShift,
    JsArrayUnshift,
    JsArrayFlat,
    JsConstruct,
    JsIsFunctionValue,
    JsIsWindowValue,
    JsDefineConfigurable,
    JsDefineIterator,
    JsArrayIterator,
    JsConsoleWarn,
    JsRequestAnimationFrameOrNull,
    JsForInKey,
    JsForInHasNext,
    JsForOfValue,
    JsForOfHasNext,
    GeneratorYield,
    GeneratorYieldDelegated,
}

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
