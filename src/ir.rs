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
    pub exports: Vec<IrExport<'src>>,
    pub lazy_modules: Vec<IrLazyModule<'src>>,
    pub structs: Vec<AggregateLayout<'src>>,
    pub classes: Vec<AggregateLayout<'src>>,
    pub entry: FunctionId,
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

#[derive(Debug, Clone, PartialEq)]
pub struct ControlFlowFunction<'src> {
    pub id: FunctionId,
    pub name: Option<&'src str>,
    pub kind: FunctionKind<'src>,
    pub declared_pure: bool,
    pub params: Vec<IrParameter<'src>>,
    pub capture_count: usize,
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
}

impl ControlShape {
    pub const fn header(&self) -> BlockId {
        match self {
            Self::If { header, .. } | Self::Loop { header, .. } | Self::ForIn { header, .. } => {
                *header
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum IrParamDefault<'src> {
    Const(ConstValue),
    Value(ValueId),
    Name(&'src str),
}

#[derive(Debug, Clone, PartialEq)]
pub struct IrParameter<'src> {
    pub symbol: SymbolId,
    pub local: LocalId,
    pub value: ValueId,
    pub name: &'src str,
    pub ty: Type<'src>,
    pub default: Option<IrParamDefault<'src>>,
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
    pub local: LocalId,
    pub ty: Type<'src>,
    pub incoming: Vec<(BlockId, ValueId)>,
    pub span: Span,
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
    Array(Vec<ValueId>),
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
    IndexSet {
        object: ValueId,
        index: ValueId,
        value: ValueId,
    },
    CallDirect {
        function: FunctionId,
        args: Vec<ValueId>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Intrinsic {
    Print,
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
    ArraySlice,
    ArraySplice,
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
    StringStartsWith,
    StringEndsWith,
    StringToUpperCase,
    StringToLowerCase,
    JsTruthy,
    JsIsArray,
    JsIsObject,
    JsForInKey,
    JsForInHasNext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Terminator {
    Jump(BlockId),
    Branch {
        condition: ValueId,
        then_block: BlockId,
        else_block: BlockId,
    },
    Return(Option<ValueId>),
    Unreachable,
}
