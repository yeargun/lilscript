//! Admission of the direct native value representation. Published semantic
//! units and their complete UseIndex remain the only source graph. This plan
//! owns C storage/call choices, not a second checker or optimization pipeline.
use super::activation::StructuredDominance;
use super::native::{NativeError, NativeHostBinding, NativeHostBindings};
#[path = "native_signatures.rs"]
mod signatures;
use super::native_runtime::{Helper, Helpers};
use super::uses::{CellUse, CellUseSite, UseIndex, ValueUse};
use super::*;
use crate::compilation_policy::WorkKind;
use crate::output_budget::{AllocationBudget, AllocationClass::Scratch, AllocationError};
use crate::primitive::Intrinsic;
pub(super) use signatures::NativeSignature;
use std::{fmt, mem::size_of};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NativeType {
    I32,
    F64,
    Bool,
    String,
    Void,
    Struct(usize),
    Callable(usize),
    /// A reference-counted growable array whose element type is entry `k` of
    /// the plan's interned element table.
    Array(usize),
    /// A tagged value: a nullable, a union or a type parameter.
    Dynamic(Tagged),
    /// An instance of the program class at this index: a reference-counted
    /// C struct. Every class slot is `ls_native_object *`, so an upcast is no
    /// conversion; a member access casts to its declaring class.
    Object(usize),
    /// Builtin reference-counted objects, held as `ls_native_object *`: an
    /// insertion-ordered `Map` or `Set` of tagged keys, and a `Symbol`.
    Map,
    Set,
    Symbol,
    /// An `ArrayBuffer` or `SharedArrayBuffer` (one thread: the same thing).
    Buffer,
    /// A typed array view of a buffer.
    Typed(crate::typed_array::TypedArrayKind),
}
/// What a tagged value may carry beyond scalars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct Tagged {
    /// The tag may select a reference-counted payload.
    pub(super) owns: bool,
    /// The physical signature of the one function type among the members.
    /// A type parameter has none: its callable payloads are checked when
    /// unboxed, against the signature recorded when boxed.
    pub(super) callable: Option<usize>,
}
impl Tagged {
    pub(super) const PLAIN: Self = Self {
        owns: false,
        callable: None,
    };
    pub(super) const ANY: Self = Self {
        owns: true,
        callable: None,
    };
}

impl fmt::Display for NativeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::I32 => f.write_str("int32_t"),
            Self::F64 => f.write_str("double"),
            Self::Bool => f.write_str("bool"),
            Self::String => f.write_str("ls_string"),
            Self::Void => f.write_str("void"),
            Self::Struct(index) => write!(f, "ls_t{index}"),
            Self::Callable(index) => write!(f, "ls_callable{index}"),
            Self::Array(index) => write!(f, "ls_array{index} *"),
            Self::Dynamic(_) => f.write_str("ls_value"),
            Self::Object(_)
            | Self::Map
            | Self::Set
            | Self::Symbol
            | Self::Buffer
            | Self::Typed(_) => f.write_str("ls_native_object *"),
        }
    }
}

impl NativeType {
    /// A value slot of this type owns a reference-counted object: copies
    /// retain, overwrites and scope exits release.
    pub(super) fn managed(self) -> bool {
        matches!(
            self,
            Self::Callable(_)
                | Self::Array(_)
                | Self::Object(_)
                | Self::Map
                | Self::Set
                | Self::Symbol
                | Self::Buffer
                | Self::Typed(_)
                | Self::Dynamic(Tagged { owns: true, .. })
        )
    }
    /// The statement that acquires one more owner of `value`, if it owns.
    pub(super) fn retain(self, value: &str) -> Option<String> {
        match self {
            Self::Callable(_) => Some(format!("ls_native_retain({value}.environment);\n")),
            Self::Array(_)
            | Self::Object(_)
            | Self::Map
            | Self::Set
            | Self::Symbol
            | Self::Buffer
            | Self::Typed(_) => Some(format!("ls_native_retain({value});\n")),
            Self::Dynamic(Tagged { owns: true, .. }) => {
                Some(format!("ls_value_retain({value});\n"))
            }
            _ => None,
        }
    }
    /// The statement that gives up one owner of `value`, if it owns.
    pub(super) fn release(self, value: &str) -> Option<String> {
        match self {
            Self::Callable(_) => Some(format!("ls_native_release({value}.environment);\n")),
            Self::Array(_)
            | Self::Object(_)
            | Self::Map
            | Self::Set
            | Self::Symbol
            | Self::Buffer
            | Self::Typed(_) => Some(format!("ls_native_release({value});\n")),
            Self::Dynamic(Tagged { owns: true, .. }) => {
                Some(format!("ls_value_release({value});\n"))
            }
            _ => None,
        }
    }
    /// The C prefix of this managed type's `_copy`/`_take`/`_clear` helpers.
    pub(super) fn owner_prefix(self) -> Option<String> {
        match self {
            Self::Callable(signature) => Some(format!("ls_callable{signature}")),
            Self::Array(index) => Some(format!("ls_array{index}")),
            Self::Object(_)
            | Self::Map
            | Self::Set
            | Self::Symbol
            | Self::Buffer
            | Self::Typed(_) => Some("ls_object".to_owned()),
            Self::Dynamic(Tagged { owns: true, .. }) => Some("ls_value".to_owned()),
            _ => None,
        }
    }
    /// The C initializer of an empty managed slot.
    pub(super) fn empty_slot(self) -> &'static str {
        match self {
            Self::Callable(_) => " = {0}",
            Self::Array(_)
            | Self::Object(_)
            | Self::Map
            | Self::Set
            | Self::Symbol
            | Self::Buffer
            | Self::Typed(_) => " = NULL",
            Self::Dynamic(_) => " = {0}",
            _ => "",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ValueStorage {
    Value(NativeType),
    Function(UnitId),
    Host(usize),
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CellPlan {
    pub(super) storage: ValueStorage,
    /// Held in a shared box because a closure captures it.
    pub(super) captured: bool,
    /// A module binding that functions read or write: C file-scope storage
    /// that outlives its module's initializer, cleared when `main` ends.
    pub(super) global: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PreparedTarget {
    Function(UnitId),
    Callable {
        callee: ValueId,
        signature: usize,
    },
    /// A callable read from a place when the call is prepared, before its
    /// arguments, and held until the call returns.
    Placed {
        place: PlaceId,
        signature: usize,
    },
    Host {
        binding: usize,
        signature: usize,
    },
    Print,
    MathImul,
    CharCodeAt {
        receiver: ValueId,
    },
    CharAt {
        receiver: ValueId,
    },
    ArrayPush {
        receiver: ValueId,
        array: usize,
    },
    ArrayPop {
        receiver: ValueId,
        array: usize,
    },
    /// A string, `int` or `float` method; `native_strings` owns each recipe.
    ScalarMethod {
        receiver: ValueId,
        method: Intrinsic,
    },
    /// `new Map()`, `new Set()`, `new Symbol(...)`, buffers and typed arrays.
    Construct(Intrinsic),
    /// A typed array's or buffer's `slice`/`subarray`.
    BinaryMethod {
        receiver: ValueId,
        method: Intrinsic,
    },
    /// A `Map` or `Set` method; `native_collections` owns each recipe.
    CollectionMethod {
        receiver: ValueId,
        method: Intrinsic,
    },
    /// Any other admitted array method; `native_arrays` owns each recipe.
    ArrayMethod {
        receiver: ValueId,
        array: usize,
        method: Intrinsic,
    },
}

#[derive(Debug, Clone, Copy)]
pub(super) enum PlaceRecipe {
    Cell(CellId),
    Value(ValueId),
    Field {
        base: PlaceId,
        slot: usize,
    },
    /// A field of a class instance, by its flattened slot, spelled through
    /// the class that declares it.
    Member {
        receiver: ValueId,
        class: usize,
        slot: usize,
    },
    /// `length` or `receiver[index]` of a tagged value that is an array of
    /// interned kind `array` or a typed array of `kind`: decided by its tag.
    /// `index` is `None` for `length`.
    IndexedUnion {
        receiver: ValueId,
        index: Option<ValueId>,
        array: usize,
        kind: crate::typed_array::TypedArrayKind,
    },
    /// `receiver[index]` of a typed array: read and written by value.
    TypedElement {
        receiver: ValueId,
        index: ValueId,
        kind: crate::typed_array::TypedArrayKind,
    },
    /// `receiver[index]` of a native array.
    Element {
        receiver: ValueId,
        index: ValueId,
        array: usize,
    },
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PlacePlan {
    pub(super) recipe: PlaceRecipe,
    storage: ValueStorage,
    root_cell: Option<CellId>,
    writable: bool,
}

#[derive(Debug)]
pub(super) struct UnitPlan {
    pub(super) values: Vec<ValueStorage>,
    pub(super) calls: Vec<PreparedTarget>,
    pub(super) places: Vec<PlacePlan>,
    pub(super) return_type: NativeType,
    pub(super) has_environment: bool,
    pub(super) named_adapter_needed: bool,
}

#[derive(Debug)]
pub(super) struct NativePlan<'program, 'src> {
    pub(super) program: &'program Program<'src>,
    pub(super) units: Vec<UnitPlan>,
    pub(super) cells: Vec<CellPlan>,
    pub(super) signatures: Vec<NativeSignature<'program, 'src>>,
    pub(super) hosts: &'program NativeHostBindings<'program>,
    pub(super) strings: Vec<bool>,
    pub(super) helpers: Helpers,
    types: Vec<TypeClass>,
    pub(super) struct_order: Vec<usize>,
    /// Interned element type of each `NativeType::Array(k)`.
    pub(super) arrays: Vec<NativeType>,
    /// Each internal class's flattened (base-first) field representations.
    pub(super) class_fields: Vec<Vec<NativeType>>,
    /// Callable adapters `(from, to)` between physical signatures, used where
    /// a call passes a callable to a parameter of another signature.
    pub(super) adapters: Vec<(usize, usize)>,
}

#[derive(Debug, Clone, Copy)]
enum TypeClass {
    Value(NativeType),
    Function(usize),
}
#[derive(Clone, Copy)]
enum Initialization {
    Missing,
    Parameter,
    Foreign,
    Operation(OpId),
}

fn fail(
    unit: Option<UnitId>,
    operation: Option<OpId>,
    span: Span,
    feature: &'static str,
) -> NativeError {
    NativeError::unsupported(unit, operation, span, feature)
}
fn work(budget: &mut AllocationBudget<'_>, count: usize) -> Result<(), NativeError> {
    let count =
        u64::try_from(count).map_err(|_| NativeError::Allocation(AllocationError::Capacity))?;
    budget
        .work(WorkKind::Analysis, count)
        .map_err(NativeError::Allocation)
}
fn bytes<T>(capacity: usize) -> Result<u64, NativeError> {
    capacity
        .checked_mul(size_of::<T>())
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(NativeError::Allocation(AllocationError::Capacity))
}
fn release<T>(value: Vec<T>, budget: &mut AllocationBudget<'_>) -> Result<(), NativeError> {
    let charge = bytes::<T>(value.capacity())?;
    drop(value);
    budget
        .release(Scratch, charge)
        .map_err(NativeError::Allocation)
}
/// Tables a source type's representation may extend: interned array
/// element types and physical callable signatures.
pub(super) struct TypeTables<'program, 'src> {
    pub(super) arrays: Vec<NativeType>,
    pub(super) signatures: Vec<NativeSignature<'program, 'src>>,
}

fn intern_array(
    tables: &mut TypeTables<'_, '_>,
    element: NativeType,
    budget: &mut AllocationBudget<'_>,
) -> Result<usize, NativeError> {
    work(budget, tables.arrays.len())?;
    if let Some(index) = tables.arrays.iter().position(|&known| known == element) {
        return Ok(index);
    }
    budget.push(Scratch, &mut tables.arrays, element)?;
    Ok(tables.arrays.len() - 1)
}

pub(super) fn native_type<'program, 'src>(
    program: &'program Program<'src>,
    ty: &'program Type<'src>,
    tables: &mut TypeTables<'program, 'src>,
    budget: &mut AllocationBudget<'_>,
) -> Result<Option<NativeType>, NativeError> {
    Ok(Some(match ty {
        // A closed enum is its declaration-order discriminant.
        Type::Int | Type::Enum(_) => NativeType::I32,
        Type::Float => NativeType::F64,
        Type::Bool => NativeType::Bool,
        Type::String => NativeType::String,
        Type::Void => NativeType::Void,
        Type::Struct(declaration) => {
            // The common declaration owner supplies identity. Diagnostic names
            // may collide across original modules and never select a layout.
            work(budget, 1)?;
            let index = declaration.identity.index();
            let Some(definition) = program.structs.get(index) else {
                return Ok(None);
            };
            if definition.identity != declaration.identity || !definition.type_parameters.is_empty()
            {
                return Ok(None);
            }
            NativeType::Struct(index)
        }
        Type::Array(element) => {
            work(budget, 1)?;
            let element = match &**element {
                Type::Function(_) | Type::GenericFunction(_) => {
                    NativeType::Callable(signatures::register(program, tables, element, budget)?)
                }
                element => match native_type(program, element, tables, budget)? {
                    Some(ty) if ty != NativeType::Void => ty,
                    _ => return Ok(None),
                },
            };
            NativeType::Array(intern_array(tables, element, budget)?)
        }
        // A host (extern) class instance is a JavaScript object.
        Type::Class(name) | Type::ClassInstance { name, .. } => {
            work(budget, program.classes.len())?;
            match program
                .classes
                .iter()
                .position(|class| class.name == *name && !class.external)
            {
                Some(index) => NativeType::Object(index),
                None => return Ok(None),
            }
        }
        // Keys and values live in tagged entries.
        Type::Map(key, value) => {
            if dynamic_member(program, key, tables, budget)?.is_none()
                || dynamic_member(program, value, tables, budget)?.is_none()
            {
                return Ok(None);
            }
            NativeType::Map
        }
        Type::Set(key) => {
            if dynamic_member(program, key, tables, budget)?.is_none() {
                return Ok(None);
            }
            NativeType::Set
        }
        Type::Symbol => NativeType::Symbol,
        Type::ArrayBuffer | Type::SharedArrayBuffer => NativeType::Buffer,
        ty if crate::typed_array::TypedArrayKind::from_type(ty).is_some() => {
            NativeType::Typed(crate::typed_array::TypedArrayKind::from_type(ty).unwrap())
        }
        Type::Null => NativeType::Dynamic(Tagged::PLAIN),
        Type::Nullable(inner) => match dynamic_member(program, inner, tables, budget)? {
            Some(tagged) => NativeType::Dynamic(tagged),
            None => return Ok(None),
        },
        Type::Union(members) => {
            let mut tagged = Tagged::PLAIN;
            for member in members {
                let Some(member) = dynamic_member(program, member, tables, budget)? else {
                    return Ok(None);
                };
                tagged.owns |= member.owns;
                // Two function members would share one runtime category.
                match (tagged.callable, member.callable) {
                    (Some(left), Some(right)) if left != right => return Ok(None),
                    (None, Some(signature)) => tagged.callable = Some(signature),
                    _ => {}
                }
            }
            NativeType::Dynamic(tagged)
        }
        // A type parameter may stand for anything the tag can carry.
        Type::TypeParameter(name) if *name != "$js" => NativeType::Dynamic(Tagged::ANY),
        _ => return Ok(None),
    }))
}

/// What a nullable or union member adds to a tagged value, if it fits one.
/// A product would need a heap box: not native yet.
fn dynamic_member<'program, 'src>(
    program: &'program Program<'src>,
    ty: &'program Type<'src>,
    tables: &mut TypeTables<'program, 'src>,
    budget: &mut AllocationBudget<'_>,
) -> Result<Option<Tagged>, NativeError> {
    if matches!(ty, Type::Function(_) | Type::GenericFunction(_)) {
        let signature = signatures::register(program, tables, ty, budget)?;
        return Ok(Some(Tagged {
            owns: true,
            callable: Some(signature),
        }));
    }
    Ok(match native_type(program, ty, tables, budget)? {
        Some(NativeType::Void | NativeType::Struct(_)) | None => None,
        Some(NativeType::Dynamic(tagged)) => Some(tagged),
        Some(ty) => Some(Tagged {
            owns: ty.managed(),
            callable: None,
        }),
    })
}

fn compatible(expected: ValueStorage, actual: ValueStorage) -> bool {
    expected == actual
        || matches!(
            (expected, actual),
            (
                ValueStorage::Value(NativeType::F64),
                ValueStorage::Value(NativeType::I32)
            )
        )
}

/// C requires complete field layouts before a containing declaration. One
/// explicit DFS over the admitted schema edges rejects infinite value layout;
/// it neither follows the Rust stack nor expands nested structs into fields.
fn struct_layouts(
    program: &Program<'_>,
    types: &[TypeClass],
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<usize>, NativeError> {
    let mut order = budget.vector(Scratch, program.structs.len())?;
    let mut states = budget.filled(Scratch, program.structs.len(), 0u8)?;
    let mut stack = budget.vector(Scratch, program.structs.len())?;
    for root in 0..program.structs.len() {
        work(budget, 1)?;
        if states[root] == 2 {
            continue;
        }
        states[root] = 1;
        stack.push((root, program.structs[root].fields.start));
        while let Some((index, next)) = stack.last_mut() {
            work(budget, 1)?;
            let definition = &program.structs[*index];
            if !definition.type_parameters.is_empty() {
                return Err(fail(
                    None,
                    None,
                    Span::default(),
                    "native generic struct layout",
                ));
            }
            if *next == definition.fields.end {
                states[*index] = 2;
                order.push(*index);
                stack.pop();
                continue;
            }
            let field = &program.fields[*next];
            *next += 1;
            match types[field.ty.index()] {
                TypeClass::Value(NativeType::Struct(child)) => match states[child] {
                    0 => {
                        states[child] = 1;
                        stack.push((child, program.structs[child].fields.start));
                    }
                    1 => {
                        return Err(fail(
                            None,
                            None,
                            Span::default(),
                            "native recursive value struct",
                        ))
                    }
                    _ => {}
                },
                // A value struct is copied bit for bit, so a field may not
                // own anything: no retain would follow the copy.
                TypeClass::Value(ty) if ty.managed() => {
                    return Err(fail(
                        None,
                        None,
                        Span::default(),
                        "native managed struct field",
                    ))
                }
                TypeClass::Value(ty) if ty != NativeType::Void => {}
                _ => {
                    return Err(fail(
                        None,
                        None,
                        Span::default(),
                        "native struct field representation",
                    ))
                }
            }
        }
    }
    release(stack, budget)?;
    release(states, budget)?;
    Ok(order)
}

fn plan_places(
    program: &Program<'_>,
    unit: &UnitData,
    types: &[TypeClass],
    arrays: &[NativeType],
    cells: &[CellPlan],
    values: &[ValueStorage],
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<PlacePlan>, NativeError> {
    let mut places: Vec<PlacePlan> = budget.vector(Scratch, unit.places.len())?;
    for place in &unit.places {
        work(budget, 1)?;
        let plan = match *place {
            Place::Cell(cell) => PlacePlan {
                recipe: PlaceRecipe::Cell(cell),
                storage: cells[cell.index()].storage,
                root_cell: Some(cell),
                writable: true,
            },
            Place::Value(value) => PlacePlan {
                recipe: PlaceRecipe::Value(value),
                storage: values[value.index()],
                root_cell: None,
                writable: false,
            },
            Place::Index { receiver, .. } | Place::Member { receiver, .. }
                if matches!(
                    values[receiver.index()],
                    ValueStorage::Value(NativeType::Dynamic(_))
                ) =>
            {
                let (index, element) = match *place {
                    Place::Index { key, .. }
                        if values[key.index()] == ValueStorage::Value(NativeType::I32) =>
                    {
                        (Some(key), None)
                    }
                    Place::Member { key, .. }
                        if program.strings[key.index()].as_unicode() == Some("length") =>
                    {
                        (None, Some(NativeType::I32))
                    }
                    _ => {
                        return Err(fail(
                            None,
                            None,
                            Span::default(),
                            "native host aggregate place",
                        ))
                    }
                };
                let Some((array, kind, item)) = indexed_union(
                    program,
                    &program.types[unit.values[receiver.index()].ty.index()],
                    arrays,
                ) else {
                    return Err(fail(
                        None,
                        None,
                        Span::default(),
                        "native host aggregate place",
                    ));
                };
                PlacePlan {
                    recipe: PlaceRecipe::IndexedUnion {
                        receiver,
                        index,
                        array,
                        kind,
                    },
                    storage: ValueStorage::Value(element.unwrap_or(item)),
                    root_cell: None,
                    writable: index.is_some(),
                }
            }
            Place::Index { receiver, key }
                if matches!(
                    (values[receiver.index()], values[key.index()]),
                    (
                        ValueStorage::Value(NativeType::Typed(_)),
                        ValueStorage::Value(NativeType::I32)
                    )
                ) =>
            {
                let ValueStorage::Value(NativeType::Typed(kind)) = values[receiver.index()] else {
                    unreachable!()
                };
                PlacePlan {
                    recipe: PlaceRecipe::TypedElement {
                        receiver,
                        index: key,
                        kind,
                    },
                    storage: ValueStorage::Value(if kind.element_is_float() {
                        NativeType::F64
                    } else {
                        NativeType::I32
                    }),
                    root_cell: None,
                    writable: true,
                }
            }
            Place::Index { receiver, key } => {
                let (
                    ValueStorage::Value(NativeType::Array(array)),
                    ValueStorage::Value(NativeType::I32),
                ) = (values[receiver.index()], values[key.index()])
                else {
                    return Err(fail(
                        None,
                        None,
                        Span::default(),
                        "native host aggregate place",
                    ));
                };
                PlacePlan {
                    recipe: PlaceRecipe::Element {
                        receiver,
                        index: key,
                        array,
                    },
                    storage: ValueStorage::Value(arrays[array]),
                    root_cell: None,
                    writable: true,
                }
            }
            Place::Member { receiver, key } => {
                let ValueStorage::Value(NativeType::Object(class)) = values[receiver.index()]
                else {
                    return Err(fail(
                        None,
                        None,
                        Span::default(),
                        "native host member place",
                    ));
                };
                let fields = &program.classes[class].fields;
                work(budget, fields.len())?;
                let slot = fields
                    .iter()
                    .position(|(name, _)| *name == key)
                    .ok_or_else(|| fail(None, None, Span::default(), "native class member"))?;
                // The field's storage is laid out by the class declaring it,
                // whose view of a generic base's field may be a tagged value.
                let declaring = declaring_class(program, class, slot);
                let ty = match types[program.classes[declaring].fields[slot].1.index()] {
                    TypeClass::Value(ty) => ty,
                    TypeClass::Function(signature) => NativeType::Callable(signature),
                };
                PlacePlan {
                    recipe: PlaceRecipe::Member {
                        receiver,
                        class: declaring,
                        slot,
                    },
                    storage: ValueStorage::Value(ty),
                    root_cell: None,
                    writable: true,
                }
            }
            Place::Field { base, field } => {
                // The shared verifier proves the canonical, earlier-base DAG.
                let parent = places[base.index()];
                let ValueStorage::Value(NativeType::Struct(index)) = parent.storage else {
                    return Err(fail(
                        None,
                        None,
                        Span::default(),
                        "native field receiver representation",
                    ));
                };
                let mut found = None;
                for slot in program.structs[index].fields.clone() {
                    work(budget, 1)?;
                    if program.fields[slot].identity == field {
                        found = Some(slot);
                        break;
                    }
                }
                let field = found
                    .map(|slot| &program.fields[slot])
                    .ok_or_else(|| fail(None, None, Span::default(), "native field identity"))?;
                let TypeClass::Value(ty) = types[field.ty.index()] else {
                    return Err(fail(
                        None,
                        None,
                        Span::default(),
                        "native field representation",
                    ));
                };
                PlacePlan {
                    recipe: PlaceRecipe::Field {
                        base,
                        slot: field.index,
                    },
                    storage: ValueStorage::Value(ty),
                    root_cell: parent.root_cell,
                    // An element is read by value, so a field of one is no C
                    // lvalue: updating it is not native yet.
                    writable: parent.writable && !element_rooted(&places, base),
                }
            }
            _ => {
                return Err(fail(
                    None,
                    None,
                    Span::default(),
                    "native host aggregate place",
                ))
            }
        };
        places.push(plan);
    }
    Ok(places)
}

/// A union of one array and one typed array whose elements share a
/// representation: its interned array, typed kind and element type.
fn indexed_union(
    _program: &Program<'_>,
    ty: &Type<'_>,
    arrays: &[NativeType],
) -> Option<(usize, crate::typed_array::TypedArrayKind, NativeType)> {
    let Type::Union(members) = ty else {
        return None;
    };
    let (mut array, mut typed) = (None, None);
    for member in members {
        match member {
            Type::Array(element) => {
                let element = match **element {
                    Type::Int => NativeType::I32,
                    Type::Float => NativeType::F64,
                    _ => return None,
                };
                let index = arrays.iter().position(|&known| known == element)?;
                if array.replace((index, element)).is_some() {
                    return None;
                }
            }
            member => {
                let kind = crate::typed_array::TypedArrayKind::from_type(member)?;
                if typed.replace(kind).is_some() {
                    return None;
                }
            }
        }
    }
    let ((array, element), kind) = (array?, typed?);
    (kind.element_is_float() == (element == NativeType::F64)).then_some((array, kind, element))
}

/// The class along `class`'s base chain whose own fields include `slot` of
/// the flattened (base-first) field list.
pub(super) fn declaring_class(program: &Program<'_>, mut class: usize, slot: usize) -> usize {
    loop {
        let Some(base) = program.classes[class].base.as_deref().and_then(|name| {
            program
                .classes
                .iter()
                .position(|candidate| candidate.name == name)
        }) else {
            return class;
        };
        if slot >= program.classes[base].fields.len() {
            return class;
        }
        class = base;
    }
}

fn element_rooted(places: &[PlacePlan], mut place: PlaceId) -> bool {
    loop {
        match places[place.index()].recipe {
            PlaceRecipe::Element { .. }
            | PlaceRecipe::TypedElement { .. }
            | PlaceRecipe::IndexedUnion { .. } => return true,
            PlaceRecipe::Field { base, .. } => place = base,
            _ => return false,
        }
    }
}

fn reference_parameter(program: &Program<'_>, cell: CellId) -> bool {
    let source = &program.cells[cell.index()];
    let CellBinding::Parameter(position) = source.binding else {
        return false;
    };
    let data = program.unit(source.owner).unwrap();
    let Type::Function(signature) = &program.types[data.callable_type.unwrap().index()] else {
        return false;
    };
    signature.params[position as usize].passing
        == crate::primitive::ParameterPassing::MutableReference
}

fn host_for_cell(
    hosts: &[NativeHostBinding<'_>],
    cell: CellId,
    budget: &mut AllocationBudget<'_>,
) -> Result<Option<usize>, NativeError> {
    let mut low = 0;
    let mut high = hosts.len();
    while low < high {
        work(budget, 1)?;
        let middle = low + (high - low) / 2;
        match hosts[middle].cell.cmp(&cell) {
            std::cmp::Ordering::Less => low = middle + 1,
            std::cmp::Ordering::Greater => high = middle,
            std::cmp::Ordering::Equal => return Ok(Some(middle)),
        }
    }
    Ok(None)
}

fn validate_hosts(
    program: &Program<'_>,
    hosts: &[NativeHostBinding<'_>],
    budget: &mut AllocationBudget<'_>,
) -> Result<(), NativeError> {
    let mut previous = None;
    for (index, host) in hosts.iter().enumerate() {
        work(
            budget,
            host.link_name
                .len()
                .checked_add(1)
                .ok_or(AllocationError::Capacity)?,
        )?;
        let error = |feature| fail(None, None, Span::default(), feature);
        if previous.is_some_and(|cell| cell >= host.cell) {
            return Err(error("native host binding order or duplicate"));
        }
        previous = Some(host.cell);
        let Some(cell) = program.cells.get(host.cell.index()) else {
            return Err(error("native host binding cell"));
        };
        if cell.binding != CellBinding::Foreign {
            return Err(error("native host binding declaration"));
        }
        let Type::Function(signature) = &program.types[cell.ty.index()] else {
            return Err(error("native host binding signature"));
        };
        for parameter in &signature.params {
            work(budget, 1)?;
            if parameter.passing != crate::primitive::ParameterPassing::Value
                || parameter.default.is_some()
            {
                return Err(error("native host passing/default ABI"));
            }
        }
        let Some(suffix) = host.link_name.strip_prefix("host_") else {
            return Err(error("native callback provider symbol namespace"));
        };
        let mut name = suffix.bytes();
        if !name
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic() || first == b'_')
            || !name.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(error("native host link identifier"));
        }
        for old in &hosts[..index] {
            work(
                budget,
                old.link_name
                    .len()
                    .checked_add(host.link_name.len())
                    .and_then(|bytes| bytes.checked_mul(8))
                    .ok_or(AllocationError::Capacity)?,
            )?;
            if old.link_name == host.link_name {
                return Err(error("native host duplicate link identifier"));
            }
            let Type::Function(old_signature) =
                &program.types[program.cells[old.cell.index()].ty.index()]
            else {
                unreachable!("earlier host row was validated")
            };
            if generated_host_symbol(old.link_name, old_signature, host.link_name)
                || generated_host_symbol(host.link_name, signature, old.link_name)
            {
                return Err(error("native host generated interface symbol collision"));
            }
        }
    }
    Ok(())
}

/// Compare the exact emitted aliases without allocating their spellings or a
/// second symbol table. Leading-zero argument ordinals are not generated names;
/// primitive arguments have an alias but no callable retain/release/call API.
fn generated_host_symbol(
    provider: &str,
    signature: &crate::check::FunctionSignature<'_>,
    name: &str,
) -> bool {
    let Some(suffix) = name.strip_prefix(provider) else {
        return false;
    };
    let callable_wrapper = |suffix: &str, ty: &Type<'_>| {
        matches!(ty, Type::Function(_)) && matches!(suffix, "_retain" | "_release" | "_call")
    };
    if let Some(suffix) = suffix.strip_prefix("_result") {
        return suffix.is_empty() || callable_wrapper(suffix, &signature.return_type);
    }
    let Some(argument) = suffix.strip_prefix("_arg") else {
        return false;
    };
    let count = argument.bytes().take_while(u8::is_ascii_digit).count();
    let (digits, suffix) = argument.split_at(count);
    if digits.is_empty() || (digits.len() > 1 && digits.starts_with('0')) {
        return false;
    }
    let Some(parameter) = digits
        .parse::<usize>()
        .ok()
        .and_then(|index| signature.params.get(index))
    else {
        return false;
    };
    suffix.is_empty() || callable_wrapper(suffix, &parameter.ty)
}

impl<'program, 'src> NativePlan<'program, 'src> {
    /// All returned backing is Scratch in the caller's lexical owner. On Err,
    /// Rust drops partial buffers before that caller rolls back the phase.
    pub(super) fn build(
        program: &'program Program<'src>,
        uses: &UseIndex,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, NativeError> {
        Self::build_with_hosts(program, uses, &NativeHostBindings::EMPTY, budget)
    }

    pub(super) fn build_with_hosts(
        program: &'program Program<'src>,
        uses: &UseIndex,
        hosts: &'program NativeHostBindings<'program>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, NativeError> {
        work(budget, 1)?;
        // Type-only exports have no native runtime ABI. Charge the borrowed
        // interface filter's visits, including entries it does not yield.
        work(budget, program.exports().len())?;
        if program.value_exports().next().is_some() {
            return Err(fail(None, None, Span::default(), "native exported ABI"));
        }
        if uses.tables_revision() != program.tables_revision {
            return Err(fail(None, None, Span::default(), "stale native use index"));
        }
        if hosts.callback_abi_version != super::native_memory::CALLBACK_ABI_VERSION {
            return Err(fail(
                None,
                None,
                Default::default(),
                "native callback ABI version",
            ));
        }
        validate_hosts(program, hosts.bindings, budget)?;
        let mut tables = TypeTables {
            arrays: Vec::new(),
            signatures: Vec::new(),
        };
        let mut classes = budget.vector(Scratch, program.types.len())?;
        for ty in program.types.iter() {
            work(budget, 1)?;
            let class = if let Some(ty) = native_type(program, ty, &mut tables, budget)? {
                TypeClass::Value(ty)
            } else if matches!(ty, Type::Function(_) | Type::GenericFunction(_)) {
                TypeClass::Function(signatures::register(program, &mut tables, ty, budget)?)
            } else {
                return Err(fail(None, None, Span::default(), "native source type"));
            };
            classes.push(class);
        }
        let TypeTables {
            arrays,
            mut signatures,
        } = tables;
        let struct_order = struct_layouts(program, &classes, budget)?;
        let mut class_fields = budget.vector(Scratch, program.classes.len())?;
        for class in program.classes.iter() {
            work(budget, class.fields.len() + 1)?;
            let mut fields = budget.vector(Scratch, class.fields.len())?;
            if !class.external {
                for &(_, ty) in &class.fields {
                    fields.push(match classes[ty.index()] {
                        TypeClass::Value(ty) => ty,
                        TypeClass::Function(signature) => {
                            signatures::require(&mut signatures, signature, budget)?;
                            NativeType::Callable(signature)
                        }
                    });
                }
            }
            class_fields.push(fields);
        }
        let mut cells = budget
            .vector(Scratch, program.cells.len())
            .map_err(NativeError::Allocation)?;
        let mut initializations = budget
            .vector(Scratch, program.cells.len())
            .map_err(NativeError::Allocation)?;
        let mut function_initializations = 0usize;
        for (index, cell) in program.cells.iter().enumerate() {
            work(budget, 1)?;
            let id = CellId::from_index(index).unwrap();
            let error = |feature| fail(Some(cell.owner), None, cell.declaration, feature);
            let storage = match (cell.binding, classes[cell.ty.index()]) {
                (CellBinding::Function(function), TypeClass::Function(_)) if !cell.reassigned => {
                    if program.unit(cell.owner).unwrap().kind != UnitKind::ModuleInitialization
                        || program.unit(function).unwrap().kind != UnitKind::Function
                    {
                        return Err(error("native non-global function storage"));
                    }
                    ValueStorage::Function(function)
                }
                (CellBinding::Local | CellBinding::Parameter(_), TypeClass::Value(ty))
                    if ty != NativeType::Void =>
                {
                    ValueStorage::Value(ty)
                }
                (
                    CellBinding::Local | CellBinding::Parameter(_),
                    TypeClass::Function(signature),
                ) => {
                    signatures::require(&mut signatures, signature, budget)?;
                    ValueStorage::Value(NativeType::Callable(signature))
                }
                (CellBinding::Foreign, TypeClass::Function(signature)) => {
                    let binding = host_for_cell(hosts.bindings, id, budget)?
                        .ok_or_else(|| error("native foreign storage"))?;
                    signatures::require(&mut signatures, signature, budget)?;
                    ValueStorage::Host(binding)
                }
                (CellBinding::Foreign, _) => return Err(error("native foreign storage")),
                _ => return Err(error("native cell type")),
            };
            let mut initialization = if matches!(storage, ValueStorage::Host(_)) {
                Initialization::Foreign
            } else {
                Initialization::Missing
            };
            let mut captured = false;
            let sites = uses.cell(id).unwrap().sites();
            for site in sites {
                work(budget, 1)?;
                let CellUseSite::Unit { unit, usage } = *site else {
                    return Err(error("native exported cell"));
                };
                match usage {
                    CellUse::Initialize(operation) => {
                        if unit != cell.owner || !matches!(initialization, Initialization::Missing)
                        {
                            return Err(error("native unique cell initialization"));
                        }
                        initialization = Initialization::Operation(operation);
                    }
                    CellUse::Parameter(_) => {
                        if unit != cell.owner || !matches!(initialization, Initialization::Missing)
                        {
                            return Err(error("native parameter initialization"));
                        }
                        initialization = Initialization::Parameter;
                    }
                    CellUse::Write { .. }
                        if matches!(storage, ValueStorage::Function(_) | ValueStorage::Host(_)) =>
                    {
                        return Err(error("native mutable static callable identity"));
                    }
                    CellUse::CatchBinding { .. } => return Err(error("native catch storage")),
                    CellUse::Capture if matches!(storage, ValueStorage::Value(_)) => {
                        captured = true
                    }
                    _ => {}
                }
            }
            // Source declaration metadata (for example detached extern formal
            // names) can occupy a canonical Cell without an executable storage
            // occurrence. The complete occurrence owner proves the absence;
            // no target declaration or initialized-state claim is invented.
            // A later read/write/reference/capture edit must establish ordinary
            // initialization again through its new, nonempty use list.
            if matches!(initialization, Initialization::Missing)
                && !(cell.binding == CellBinding::Local && sites.is_empty())
            {
                return Err(error("native missing cell initialization"));
            }
            // A module binding is not a closure payload: every function reads
            // the one file-scope slot, so it needs no box or environment.
            let global = captured
                && program.unit(cell.owner).unwrap().kind == UnitKind::ModuleInitialization;
            if global {
                captured = false;
            }
            if captured {
                if matches!(storage, ValueStorage::Value(NativeType::Callable(_))) {
                    return Err(error("native captured dynamic callable payload"));
                }
                if reference_parameter(program, id) {
                    return Err(error("native captured reference parameter lifetime"));
                }
            }
            if matches!(storage, ValueStorage::Function(_)) {
                function_initializations = function_initializations
                    .checked_add(1)
                    .ok_or(NativeError::Allocation(AllocationError::Capacity))?;
            }
            cells.push(CellPlan {
                storage,
                captured,
                global,
            });
            initializations.push(initialization);
        }
        prove_function_prefix(program, &cells, function_initializations, budget)?;
        work(
            budget,
            signatures
                .len()
                .checked_add(cells.len())
                .ok_or(AllocationError::Capacity)?,
        )?;
        let mut helpers = Helpers::default();
        if signatures.iter().any(|signature| signature.needed)
            || cells.iter().any(|cell| cell.captured)
            || classes
                .iter()
                .any(|class| matches!(class, TypeClass::Value(ty) if ty.managed()))
        {
            helpers.require(Helper::ClosureRuntime);
        }
        if classes
            .iter()
            .any(|class| matches!(class, TypeClass::Value(NativeType::Dynamic(_))))
        {
            helpers.require(Helper::Dynamic);
        }
        let mut plan = Self {
            program,
            units: budget
                .vector(Scratch, program.units.len())
                .map_err(NativeError::Allocation)?,
            cells,
            signatures,
            hosts,
            strings: budget
                .filled(Scratch, program.strings.len(), false)
                .map_err(NativeError::Allocation)?,
            helpers,
            types: classes,
            struct_order,
            arrays,
            class_fields,
            adapters: Vec::new(),
        };
        for frozen in &program.units {
            work(budget, 1)?;
            let unit = frozen.id();
            let data = frozen.data();
            let unit_error = |feature| {
                fail(
                    Some(unit),
                    None,
                    data.regions[data.entry.index()].span,
                    feature,
                )
            };
            if uses
                .unit(unit)
                .is_none_or(|uses| uses.revision() != frozen.revision())
            {
                return Err(unit_error("stale native unit index"));
            }
            let return_type = if data.kind == UnitKind::ModuleInitialization {
                NativeType::Void
            } else {
                let TypeClass::Function(signature) =
                    plan.types[data.callable_type.unwrap().index()]
                else {
                    return Err(unit_error("native function signature"));
                };
                plan.signatures[signature].result
            };
            let mut has_environment = data.kind == UnitKind::Closure;
            for &capture in &data.captures {
                work(budget, 1)?;
                let cell = plan.cells[capture.index()];
                has_environment |= cell.captured;
                if matches!(cell.storage, ValueStorage::Value(_)) && !cell.captured && !cell.global
                {
                    return Err(unit_error("native missing captured box"));
                }
            }
            if has_environment {
                plan.helpers.require(Helper::ClosureRuntime);
            }
            let mut values = budget.vector(Scratch, data.values.len())?;
            for (value_index, value) in data.values.iter().enumerate() {
                work(budget, 1)?;
                let operation = &data.operations[value.definition.index()];
                let storage = match plan.types[value.ty.index()] {
                    TypeClass::Value(ty) => {
                        if ty == NativeType::F64 {
                            plan.helpers.require(Helper::RoundBinary64);
                        }
                        ValueStorage::Value(ty)
                    }
                    TypeClass::Function(signature) => {
                        let fixed = match operation.kind {
                            OperationKind::Closure(child) => {
                                // Prefix membership is structured order, never
                                // an arena-ID threshold. Its sole canonical
                                // Initialize use has already been certified by
                                // common verification and prove_function_prefix.
                                let readers = uses
                                    .unit(unit)
                                    .unwrap()
                                    .value_uses(ValueId::from_index(value_index).unwrap())
                                    .unwrap();
                                work(budget, readers.len())?;
                                let canonical = match readers {
                                    [ValueUse::Operand {
                                        operation,
                                        position: 0,
                                    }] => {
                                        matches!(data.operations[operation.index()].kind, OperationKind::Initialize(cell) if plan.cell_storage(cell) == ValueStorage::Function(child))
                                    }
                                    _ => false,
                                };
                                canonical.then_some(ValueStorage::Function(child))
                            }
                            OperationKind::Load(place) => match data.places[place.index()] {
                                Place::Cell(cell)
                                    if matches!(
                                        plan.cell_storage(cell),
                                        ValueStorage::Function(_) | ValueStorage::Host(_)
                                    ) =>
                                {
                                    Some(plan.cell_storage(cell))
                                }
                                _ => None,
                            },
                            _ => None,
                        };
                        if let Some(fixed) = fixed {
                            fixed
                        } else {
                            signatures::require(&mut plan.signatures, signature, budget)?;
                            plan.helpers.require(Helper::ClosureRuntime);
                            ValueStorage::Value(NativeType::Callable(signature))
                        }
                    }
                };
                values.push(storage);
            }
            let places = plan_places(
                program,
                data,
                &plan.types,
                &plan.arrays,
                &plan.cells,
                &values,
                budget,
            )?;
            let mut calls = budget
                .vector(Scratch, data.calls.len())
                .map_err(NativeError::Allocation)?;
            for (index, call) in data.calls.iter().enumerate() {
                work(budget, 1)?;
                let at = uses
                    .unit(unit)
                    .unwrap()
                    .call_operation(CallId::from_index(index).unwrap())
                    .unwrap();
                let error = |feature| {
                    fail(
                        Some(unit),
                        Some(at),
                        data.operations[at.index()].span,
                        feature,
                    )
                };
                let target = match call.target {
                    CallTarget::Value {
                        callee,
                        invocation: Invocation::Value,
                    } => match values[callee.index()] {
                        ValueStorage::Function(function) => PreparedTarget::Function(function),
                        ValueStorage::Value(NativeType::Callable(signature)) => {
                            PreparedTarget::Callable { callee, signature }
                        }
                        ValueStorage::Host(binding) => PreparedTarget::Host {
                            binding,
                            signature: plan
                                .signature_for_type(
                                    program.cells[hosts.bindings[binding].cell.index()].ty,
                                )
                                .unwrap(),
                        },
                        _ => return Err(error("native dynamic value call")),
                    },
                    CallTarget::Reference { place } => match data.places[place.index()] {
                        Place::Cell(cell)
                            if matches!(plan.cell_storage(cell), ValueStorage::Function(_)) =>
                        {
                            let ValueStorage::Function(function) = plan.cell_storage(cell) else {
                                unreachable!()
                            };
                            PreparedTarget::Function(function)
                        }
                        _ => match places[place.index()].storage {
                            ValueStorage::Value(NativeType::Callable(signature)) => {
                                PreparedTarget::Placed { place, signature }
                            }
                            _ => return Err(error("native host reference call")),
                        },
                    },
                    CallTarget::Builtin(BuiltinCall::Print) => PreparedTarget::Print,
                    CallTarget::Intrinsic {
                        operation:
                            ResolvedIntrinsic::Constructor(
                                intrinsic @ (Intrinsic::MapNew
                                | Intrinsic::SetNew
                                | Intrinsic::SymbolNew),
                            ),
                        receiver: None,
                    } => {
                        plan.helpers.require(Helper::Collections);
                        PreparedTarget::Construct(intrinsic)
                    }
                    CallTarget::Intrinsic {
                        operation: ResolvedIntrinsic::Constructor(intrinsic),
                        receiver: None,
                    } if matches!(
                        intrinsic,
                        Intrinsic::ArrayBufferNew | Intrinsic::SharedArrayBufferNew
                    ) || matches!(
                        crate::typed_array::classify_typed_array_intrinsic(intrinsic),
                        Some((_, crate::typed_array::TypedArrayIntrinsic::New))
                    ) =>
                    {
                        plan.helpers.require(Helper::Binary);
                        PreparedTarget::Construct(intrinsic)
                    }
                    CallTarget::Intrinsic {
                        operation: ResolvedIntrinsic::Method(intrinsic),
                        receiver: Some(receiver),
                    } if (intrinsic == Intrinsic::BufferSlice
                        && values[receiver.index()] == ValueStorage::Value(NativeType::Buffer))
                        || (matches!(
                            values[receiver.index()],
                            ValueStorage::Value(NativeType::Typed(_))
                        ) && matches!(
                            crate::typed_array::classify_typed_array_intrinsic(intrinsic),
                            Some((
                                _,
                                crate::typed_array::TypedArrayIntrinsic::Slice
                                    | crate::typed_array::TypedArrayIntrinsic::Subarray
                            ))
                        )) =>
                    {
                        plan.helpers.require(Helper::Binary);
                        PreparedTarget::BinaryMethod {
                            receiver,
                            method: intrinsic,
                        }
                    }
                    CallTarget::Intrinsic {
                        operation: ResolvedIntrinsic::Method(intrinsic),
                        receiver: Some(receiver),
                    } if matches!(
                        (values[receiver.index()], intrinsic),
                        (
                            ValueStorage::Value(NativeType::Map),
                            Intrinsic::MapGet
                                | Intrinsic::MapSet
                                | Intrinsic::MapHas
                                | Intrinsic::MapDelete
                                | Intrinsic::MapClear
                        ) | (
                            ValueStorage::Value(NativeType::Set),
                            Intrinsic::SetAdd
                                | Intrinsic::SetHas
                                | Intrinsic::SetDelete
                                | Intrinsic::SetClear
                        )
                    ) =>
                    {
                        plan.helpers.require(Helper::Collections);
                        PreparedTarget::CollectionMethod {
                            receiver,
                            method: intrinsic,
                        }
                    }
                    CallTarget::Builtin(BuiltinCall::MathImul) => {
                        plan.helpers.require(Helper::Imul);
                        PreparedTarget::MathImul
                    }
                    CallTarget::Intrinsic {
                        operation: ResolvedIntrinsic::Method(intrinsic),
                        receiver: Some(receiver),
                    } if values[receiver.index()] == ValueStorage::Value(NativeType::String) => {
                        match intrinsic {
                            Intrinsic::StringCharCodeAt => {
                                plan.helpers.require(Helper::CharCodeAt);
                                PreparedTarget::CharCodeAt { receiver }
                            }
                            Intrinsic::StringCharAt => {
                                plan.helpers.require(Helper::CharAt);
                                PreparedTarget::CharAt { receiver }
                            }
                            Intrinsic::StringReplace | Intrinsic::StringSearch => {
                                return Err(error("native regular expression"))
                            }
                            Intrinsic::StringIncludes
                            | Intrinsic::StringIndexOf
                            | Intrinsic::StringLastIndexOf
                            | Intrinsic::StringRepeat
                            | Intrinsic::StringStartsWith
                            | Intrinsic::StringEndsWith
                            | Intrinsic::StringToUpperCase
                            | Intrinsic::StringToLowerCase
                            | Intrinsic::StringTrim
                            | Intrinsic::StringTrimStart
                            | Intrinsic::StringTrimEnd
                            | Intrinsic::StringSlice
                            | Intrinsic::StringSplit
                            | Intrinsic::StringCodePointLength => {
                                plan.helpers.require(Helper::Strings);
                                if intrinsic == Intrinsic::StringSplit {
                                    plan.helpers.require(Helper::ClosureRuntime);
                                }
                                PreparedTarget::ScalarMethod {
                                    receiver,
                                    method: intrinsic,
                                }
                            }
                            _ => return Err(error("native method primitive")),
                        }
                    }
                    CallTarget::Intrinsic {
                        operation: ResolvedIntrinsic::Method(intrinsic),
                        receiver: Some(receiver),
                    } if matches!(
                        (values[receiver.index()], intrinsic),
                        (
                            ValueStorage::Value(NativeType::I32),
                            Intrinsic::IntToString | Intrinsic::IntToUnsignedString
                        ) | (
                            ValueStorage::Value(NativeType::F64),
                            Intrinsic::FloatAbs
                                | Intrinsic::FloatFloor
                                | Intrinsic::FloatCeil
                                | Intrinsic::FloatRound
                                | Intrinsic::FloatSqrt
                                | Intrinsic::FloatSin
                                | Intrinsic::FloatCos
                                | Intrinsic::FloatAcos
                                | Intrinsic::FloatExp
                                | Intrinsic::FloatLog
                                | Intrinsic::FloatTan
                                | Intrinsic::FloatAtan2
                                | Intrinsic::FloatHypot
                                | Intrinsic::FloatMin
                                | Intrinsic::FloatMax
                                | Intrinsic::FloatToInt
                        )
                    ) =>
                    {
                        plan.helpers.require(Helper::Strings);
                        plan.helpers.require(Helper::RoundBinary64);
                        if intrinsic == Intrinsic::FloatToInt {
                            plan.helpers.require(Helper::ToInt32);
                        }
                        PreparedTarget::ScalarMethod {
                            receiver,
                            method: intrinsic,
                        }
                    }
                    CallTarget::Intrinsic {
                        operation: ResolvedIntrinsic::Method(intrinsic),
                        receiver: Some(receiver),
                    } if matches!(
                        values[receiver.index()],
                        ValueStorage::Value(NativeType::Array(_))
                    ) =>
                    {
                        let ValueStorage::Value(NativeType::Array(array)) =
                            values[receiver.index()]
                        else {
                            unreachable!()
                        };
                        plan.helpers.require(Helper::ClosureRuntime);
                        match intrinsic {
                            Intrinsic::ArrayPush => PreparedTarget::ArrayPush { receiver, array },
                            Intrinsic::ArrayPop => PreparedTarget::ArrayPop { receiver, array },
                            Intrinsic::ArrayForEach
                            | Intrinsic::ArrayMap
                            | Intrinsic::ArrayFilter
                            | Intrinsic::ArrayReduce
                            | Intrinsic::ArraySome
                            | Intrinsic::ArrayEvery
                            | Intrinsic::ArrayFindIndex
                            | Intrinsic::ArrayIndexOf
                            | Intrinsic::ArrayIncludes
                            | Intrinsic::ArrayConcat
                            | Intrinsic::ArrayReverse
                            | Intrinsic::ArraySlice
                            | Intrinsic::ArraySplice
                            | Intrinsic::ArrayFill
                            | Intrinsic::ArrayCopyWithin => {
                                if matches!(
                                    intrinsic,
                                    Intrinsic::ArrayIndexOf | Intrinsic::ArrayIncludes
                                ) {
                                    match plan.arrays[array] {
                                        // Products compare backing identity in JavaScript.
                                        NativeType::Struct(_) => {
                                            return Err(error("native product array search"))
                                        }
                                        NativeType::String => {
                                            plan.helpers.require(Helper::StringEqual)
                                        }
                                        _ => {}
                                    }
                                }
                                PreparedTarget::ArrayMethod {
                                    receiver,
                                    array,
                                    method: intrinsic,
                                }
                            }
                            _ => return Err(error("native array method")),
                        }
                    }
                    _ => return Err(error("native prepared call convention")),
                };
                calls.push(target);
            }
            let unit_plan = UnitPlan {
                values,
                calls,
                places,
                return_type,
                has_environment,
                named_adapter_needed: false,
            };
            let parents = budget
                .filled(Scratch, data.regions.len(), None)
                .map_err(NativeError::Allocation)?;
            let positions = budget
                .filled(Scratch, data.operations.len(), 0usize)
                .map_err(NativeError::Allocation)?;
            let dominance_bytes = bytes::<Option<OpId>>(parents.capacity())?
                .checked_add(bytes::<usize>(positions.capacity())?)
                .ok_or(NativeError::Allocation(AllocationError::Capacity))?;
            let dominance =
                StructuredDominance::build(data, parents, positions, |n| work(budget, n))?;
            for (index, operation) in data.operations.iter().enumerate() {
                work(budget, 1)?;
                let at = OpId::from_index(index).unwrap();
                let operands = data.operands(operation.operands).unwrap();
                work(budget, operands.len())?;
                plan.check_operation(
                    unit,
                    data,
                    &unit_plan,
                    at,
                    operation,
                    operands,
                    &initializations,
                    &dominance,
                    budget,
                )?;
            }
            drop(dominance);
            budget
                .release(Scratch, dominance_bytes)
                .map_err(NativeError::Allocation)?;
            if return_type != NativeType::Void && !must_return(data, budget)? {
                return Err(unit_error("native nonvoid fallthrough"));
            }
            plan.units.push(unit_plan);
        }
        // Materialize a named callable only for an actual non-direct use.
        // Existing ordered UseIndex lists, not a second call graph, supply it.
        for unit_index in 0..plan.units.len() {
            let unit = UnitId::from_index(unit_index).unwrap();
            let data = program.unit(unit).unwrap();
            for index in 0..plan.units[unit_index].values.len() {
                work(budget, 1)?;
                let storage = plan.units[unit_index].values[index];
                if let ValueStorage::Value(NativeType::Callable(_)) = storage {
                    if let OperationKind::Closure(child) =
                        data.operations[data.values[index].definition.index()].kind
                    {
                        if !plan.units[child.index()].has_environment {
                            plan.units[child.index()].named_adapter_needed = true;
                        }
                    }
                }
                if !matches!(storage, ValueStorage::Function(_) | ValueStorage::Host(_)) {
                    continue;
                }
                for usage in uses
                    .unit(unit)
                    .unwrap()
                    .value_uses(ValueId::from_index(index).unwrap())
                    .unwrap()
                {
                    work(budget, 1)?;
                    if matches!(usage, ValueUse::CallCallee { .. }) {
                        continue;
                    }
                    if let (ValueStorage::Function(function), ValueUse::Operand { operation, .. }) =
                        (storage, *usage)
                    {
                        if matches!(data.operations[operation.index()].kind, OperationKind::Initialize(cell) if plan.cell_storage(cell) == ValueStorage::Function(function))
                        {
                            continue;
                        }
                    }
                    match storage {
                        ValueStorage::Function(function) => {
                            if plan.units[function.index()].has_environment {
                                return Err(fail(
                                    Some(unit),
                                    None,
                                    Default::default(),
                                    "native static callable environment",
                                ));
                            }
                            plan.units[function.index()].named_adapter_needed = true;
                            plan.helpers.require(Helper::ClosureRuntime);
                            let signature = plan.signature_for_unit(function);
                            signatures::require(&mut plan.signatures, signature, budget)?;
                        }
                        ValueStorage::Host(_) => {
                            return Err(fail(
                                Some(unit),
                                None,
                                Default::default(),
                                "native host callable value escape",
                            ))
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
        for cell in &plan.cells {
            work(budget, 1)?;
            if let ValueStorage::Function(function) = cell.storage {
                if plan.units[function.index()].has_environment {
                    return Err(fail(
                        Some(function),
                        None,
                        Default::default(),
                        "native static function environment",
                    ));
                }
            }
        }
        release(initializations, budget)?;
        Ok(plan)
    }

    pub(super) fn field_type(&self, index: usize) -> NativeType {
        let TypeClass::Value(ty) = self.types[self.program.fields[index].ty.index()] else {
            unreachable!("native plan admits only by-value fields")
        };
        ty
    }

    pub(super) fn cell_storage(&self, cell: CellId) -> ValueStorage {
        self.cells[cell.index()].storage
    }
    pub(super) fn boxed_cell(&self, cell: CellId) -> bool {
        self.cells[cell.index()].captured
    }
    pub(super) fn global_cell(&self, cell: CellId) -> bool {
        self.cells[cell.index()].global
    }
    pub(super) fn signature_for_type(&self, ty: TypeId) -> Option<usize> {
        match self.types[ty.index()] {
            TypeClass::Function(index) => Some(index),
            _ => None,
        }
    }
    pub(super) fn signature_for_unit(&self, unit: UnitId) -> usize {
        self.signature_for_type(self.program.unit(unit).unwrap().callable_type.unwrap())
            .unwrap()
    }
    pub(super) fn value_type(&self, value: ValueStorage) -> NativeType {
        match value {
            ValueStorage::Value(ty) => ty,
            ValueStorage::Function(unit) => NativeType::Callable(self.signature_for_unit(unit)),
            ValueStorage::Host(binding) => NativeType::Callable(
                self.signature_for_type(
                    self.program.cells[self.hosts.bindings[binding].cell.index()].ty,
                )
                .unwrap(),
            ),
        }
    }
    /// Whether `derived` is `base` or inherits from it.
    pub(super) fn class_extends(&self, mut derived: usize, base: usize) -> bool {
        loop {
            if derived == base {
                return true;
            }
            let Some(next) = self.program.classes[derived]
                .base
                .as_deref()
                .and_then(|name| {
                    self.program
                        .classes
                        .iter()
                        .position(|candidate| candidate.name == name)
                })
            else {
                return false;
            };
            derived = next;
        }
    }
    pub(super) fn compatible(&self, expected: ValueStorage, actual: ValueStorage) -> bool {
        // A tagged slot accepts any payload the tag can carry, and a typed
        // slot accepts a tagged value by checked unboxing.
        let payload = |ty: NativeType| {
            matches!(
                ty,
                NativeType::I32
                    | NativeType::F64
                    | NativeType::Bool
                    | NativeType::String
                    | NativeType::Array(_)
                    | NativeType::Object(_)
                    | NativeType::Callable(_)
                    | NativeType::Map
                    | NativeType::Set
                    | NativeType::Symbol
                    | NativeType::Buffer
                    | NativeType::Typed(_)
            )
        };
        compatible(expected, actual)
            || match (expected, actual) {
                // A callable crosses only at its one physical signature.
                (
                    ValueStorage::Value(NativeType::Dynamic(expected)),
                    ValueStorage::Value(NativeType::Dynamic(actual)),
                ) => match (expected.callable, actual.callable) {
                    (Some(left), Some(right)) => left == right,
                    _ => true,
                },
                (
                    ValueStorage::Value(NativeType::Dynamic(tagged)),
                    ValueStorage::Value(NativeType::Callable(signature)),
                )
                | (
                    ValueStorage::Value(NativeType::Callable(signature)),
                    ValueStorage::Value(NativeType::Dynamic(tagged)),
                ) => tagged.callable.is_none_or(|callable| callable == signature),
                (
                    ValueStorage::Value(NativeType::Dynamic(tagged)),
                    ValueStorage::Function(unit),
                ) => tagged
                    .callable
                    .is_none_or(|callable| callable == self.signature_for_unit(unit)),
                (ValueStorage::Value(NativeType::Dynamic(_)), ValueStorage::Value(actual)) => {
                    payload(actual)
                }
                (ValueStorage::Value(expected), ValueStorage::Value(NativeType::Dynamic(_))) => {
                    payload(expected)
                }
                (
                    ValueStorage::Value(NativeType::Object(base)),
                    ValueStorage::Value(NativeType::Object(derived)),
                ) => self.class_extends(derived, base),
                (
                    ValueStorage::Value(NativeType::Callable(wanted)),
                    ValueStorage::Function(function),
                ) => self.signature_for_unit(function) == wanted,
                _ => false,
            }
    }
    pub(super) fn reference_parameter(&self, cell: CellId) -> bool {
        reference_parameter(self.program, cell)
    }
    /// The adapter `(from, to)` a callable argument needs to reach a
    /// parameter of another physical signature: same arity, value passing,
    /// each parameter and the result converting without another adapter.
    pub(super) fn adaptation(
        &self,
        expected: ValueStorage,
        actual: ValueStorage,
    ) -> Option<(usize, usize)> {
        let target = match expected {
            ValueStorage::Value(NativeType::Callable(target))
            | ValueStorage::Value(NativeType::Dynamic(Tagged {
                callable: Some(target),
                ..
            })) => target,
            _ => return None,
        };
        let source = match actual {
            ValueStorage::Value(NativeType::Callable(source))
            | ValueStorage::Value(NativeType::Dynamic(Tagged {
                callable: Some(source),
                ..
            })) => source,
            ValueStorage::Function(unit) => self.signature_for_unit(unit),
            _ => return None,
        };
        if source == target {
            return None;
        }
        let (from, to) = (&self.signatures[source], &self.signatures[target]);
        let by_value = |signature: &NativeSignature<'_, '_>| {
            signature
                .source
                .params
                .iter()
                .all(|parameter| parameter.passing == crate::primitive::ParameterPassing::Value)
        };
        (from.parameters.len() == to.parameters.len()
            && by_value(from)
            && by_value(to)
            && from
                .parameters
                .iter()
                .zip(&to.parameters)
                .all(|(&inner, &outer)| {
                    !matches!(inner, NativeType::Callable(_))
                        && self.compatible(ValueStorage::Value(inner), ValueStorage::Value(outer))
                })
            && !matches!(from.result, NativeType::Callable(_))
            && self.compatible(
                ValueStorage::Value(to.result),
                ValueStorage::Value(from.result),
            ))
        .then_some((source, target))
    }
    fn admit_adapter(
        &mut self,
        expected: ValueStorage,
        actual: ValueStorage,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, NativeError> {
        let Some((from, to)) = self.adaptation(expected, actual) else {
            return Ok(false);
        };
        signatures::require(&mut self.signatures, from, budget)?;
        signatures::require(&mut self.signatures, to, budget)?;
        work(budget, self.adapters.len())?;
        if !self.adapters.contains(&(from, to)) {
            budget.push(Scratch, &mut self.adapters, (from, to))?;
        }
        self.helpers.require(Helper::ClosureRuntime);
        Ok(true)
    }
    /// A call may stop early only before trailing callable parameters whose
    /// default is an arrow: the callee builds those itself.
    pub(super) fn omits_only_arrow_defaults(&self, signature: usize, supplied: usize) -> bool {
        let signature = &self.signatures[signature];
        supplied <= signature.parameters.len()
            && (supplied..signature.parameters.len()).all(|position| {
                matches!(signature.parameters[position], NativeType::Callable(_))
                    && matches!(
                        signature.source.params[position].default,
                        Some(crate::check::DefaultValue::Arrow(_))
                    )
            })
    }
    /// The interned array whose elements are strings, if any.
    pub(super) fn string_array(&self) -> Option<usize> {
        self.arrays
            .iter()
            .position(|&element| element == NativeType::String)
    }

    pub(super) fn uses_objects(&self) -> bool {
        self.types.iter().any(|class| {
            matches!(
                class,
                TypeClass::Value(
                    NativeType::Object(_)
                        | NativeType::Map
                        | NativeType::Set
                        | NativeType::Symbol
                        | NativeType::Buffer
                        | NativeType::Typed(_)
                )
            )
        })
    }

    pub(super) fn needs_callable_runtime(&self) -> bool {
        self.helpers.contains(Helper::ClosureRuntime)
    }

    pub(super) fn callable_signature_needed(&self, index: usize) -> bool {
        self.signatures[index].needed
    }
    pub(super) fn named_adapter_needed(&self, unit: UnitId) -> bool {
        self.units[unit.index()].named_adapter_needed
    }
    pub(super) fn capture_slot(&self, body: UnitId, cell: CellId) -> Option<usize> {
        self.boxed_cell(cell)
            .then(|| {
                self.program
                    .unit(body)
                    .unwrap()
                    .captures
                    .iter()
                    .position(|&capture| capture == cell)
            })
            .flatten()
    }

    pub(super) fn place_type(&self, unit: UnitId, place: PlaceId) -> NativeType {
        let ValueStorage::Value(ty) = self.units[unit.index()].places[place.index()].storage else {
            unreachable!("native reference places have a concrete value type")
        };
        ty
    }

    fn check_operation(
        &mut self,
        unit: UnitId,
        data: &UnitData,
        plan: &UnitPlan,
        at: OpId,
        operation: &Operation,
        operands: &[ValueId],
        initializations: &[Initialization],
        dominance: &StructuredDominance,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), NativeError> {
        use NativeType::{Bool, String as Text, Void, F64, I32};
        use ValueStorage::Value as Stored;
        let error = |feature| fail(Some(unit), Some(at), operation.span, feature);
        let value = |id: ValueId| plan.values[id.index()];
        let operand = |index: usize| value(operands[index]);
        let result = operation.result.map(value);
        let region_result = |region: RegionId| data.regions[region.index()].result.map(value);
        let numeric = |value| matches!(value, Stored(I32 | F64));
        // Primitives whose string conversion needs no host: ToString of a
        // number, boolean or string.
        let stringable = |value| matches!(value, Stored(I32 | F64 | Bool | Text));
        let expect = |yes, feature| if yes { Ok(()) } else { Err(error(feature)) };
        let initialized = |cell: CellId, budget: &mut AllocationBudget<'_>| {
            if self.program.cells[cell.index()].owner != unit {
                // Every root's callable instantiation precedes every root's
                // evaluation suffix. The verified static identity is therefore
                // available to another module initializer as well as a named
                // function body; scalar environments remain unsupported.
                if matches!(
                    self.cell_storage(cell),
                    ValueStorage::Function(_) | ValueStorage::Host(_)
                ) {
                    return Ok(());
                }
                // A module binding's reads from functions are guarded at run
                // time: JavaScript throws if one runs before its initializer.
                if self.global_cell(cell) {
                    return Ok(());
                }
                work(budget, data.captures.len())?;
                if self.boxed_cell(cell) && data.captures.contains(&cell) {
                    return Ok(());
                }
                return Err(error("native captured storage"));
            }
            match initializations[cell.index()] {
                Initialization::Parameter | Initialization::Foreign => Ok(()),
                Initialization::Operation(initialize)
                    if dominance.after(data, initialize, at, |n| work(budget, n))? =>
                {
                    Ok(())
                }
                _ => Err(error("native cell access before initialization")),
            }
        };
        match &operation.kind {
            OperationKind::Constant(Constant::Integer(_)) => {
                self.helpers.require(Helper::FromU32);
                Ok(())
            }
            OperationKind::Constant(Constant::Number(_)) => {
                self.helpers.require(Helper::DoubleBits);
                Ok(())
            }
            OperationKind::Constant(Constant::String(id)) => {
                self.strings[id.index()] = true;
                Ok(())
            }
            OperationKind::Constant(Constant::Boolean(_)) => Ok(()),
            OperationKind::Constant(Constant::Null) => expect(
                matches!(result, Some(Stored(NativeType::Dynamic(_)))),
                "native null representation",
            ),
            OperationKind::TypeTest(target) => expect(
                operands.len() == 1
                    && result == Some(Stored(Bool))
                    && crate::primitive::runtime_type_test(&self.program.types[target.index()])
                        .is_some(),
                "native type test",
            ),
            OperationKind::Initialize(cell) => expect(
                matches!(initializations[cell.index()], Initialization::Operation(found) if found == at)
                    && self.program.cells[cell.index()].owner == unit
                    && self.compatible(self.cell_storage(*cell), operand(0)),
                "native initialization representation",
            ),
            OperationKind::CheckPlace(place) => {
                let place = plan.places[place.index()];
                expect(
                    operands.is_empty() && result.is_none() && place.writable,
                    "native place check representation",
                )?;
                if let Some(cell) = place.root_cell {
                    initialized(cell, budget)?;
                }
                // Supported native fields are nonnullable, statically
                // addressable C value storage. Checking that location never
                // reads or converts its leaf payload.
                Ok(())
            }
            OperationKind::Load(place) | OperationKind::Store(place) => {
                let place = plan.places[place.index()];
                if let Some(cell) = place.root_cell {
                    initialized(cell, budget)?;
                }
                if matches!(operation.kind, OperationKind::Store(_)) {
                    expect(
                        place.writable
                            && matches!(place.storage, Stored(_))
                            && self.compatible(place.storage, operand(0)),
                        "native store representation",
                    )
                } else {
                    expect(
                        result.is_some_and(|result| self.compatible(result, place.storage)),
                        "native load representation",
                    )
                }
            }
            OperationKind::CopyValue => expect(
                result.is_some_and(|result| {
                    matches!(result, Stored(ty) if ty != Void)
                        && self.compatible(result, operand(0))
                }),
                "native copy representation",
            ),
            OperationKind::Allocate {
                kind: AllocationKind::Struct(identity),
                ..
            } => {
                let Some(Stored(NativeType::Struct(index))) = result else {
                    return Err(error("native struct construction representation"));
                };
                let definition = &self.program.structs[index];
                expect(
                    definition.identity == *identity && operands.len() == definition.fields.len(),
                    "native struct construction schema",
                )?;
                for (field, &argument) in definition.fields.clone().zip(operands) {
                    work(budget, 1)?;
                    let TypeClass::Value(ty) = self.types[self.program.fields[field].ty.index()]
                    else {
                        return Err(error("native struct field representation"));
                    };
                    expect(
                        self.compatible(Stored(ty), value(argument)),
                        "native struct field value",
                    )?;
                }
                Ok(())
            }
            OperationKind::Allocate {
                kind: AllocationKind::Object(_),
                ..
            } => {
                let Some(Stored(NativeType::Object(class))) = result else {
                    return Err(error("native object representation"));
                };
                let fields = self.class_fields[class].len();
                expect(
                    fields == operands.len()
                        && operands.iter().enumerate().all(|(slot, &argument)| {
                            let declaring = declaring_class(self.program, class, slot);
                            self.compatible(
                                Stored(self.class_fields[declaring][slot]),
                                value(argument),
                            )
                        }),
                    "native class instance fields",
                )
            }
            OperationKind::Allocate {
                kind: AllocationKind::Array,
                ..
            } => {
                let Some(Stored(NativeType::Array(array))) = result else {
                    return Err(error("native array construction representation"));
                };
                for &argument in operands {
                    work(budget, 1)?;
                    expect(
                        self.compatible(Stored(self.arrays[array]), value(argument)),
                        "native array element value",
                    )?;
                }
                self.helpers.require(Helper::ClosureRuntime);
                Ok(())
            }
            OperationKind::Intrinsic(ResolvedIntrinsic::Property(Intrinsic::BufferByteLength)) => {
                expect(
                    operand(0) == Stored(NativeType::Buffer) && result == Some(Stored(I32)),
                    "native buffer length",
                )?;
                self.helpers.require(Helper::Binary);
                Ok(())
            }
            OperationKind::Intrinsic(ResolvedIntrinsic::Property(intrinsic))
                if crate::typed_array::classify_typed_array_intrinsic(*intrinsic).is_some() =>
            {
                use crate::typed_array::TypedArrayIntrinsic as Typed;
                let (kind, property) =
                    crate::typed_array::classify_typed_array_intrinsic(*intrinsic).unwrap();
                let output = match property {
                    Typed::Length | Typed::ByteLength | Typed::ByteOffset => Stored(I32),
                    Typed::Buffer => Stored(NativeType::Buffer),
                    _ => return Err(error("native typed array property")),
                };
                // `buffer` may be typed as either buffer kind: a tagged value.
                expect(
                    operand(0) == Stored(NativeType::Typed(kind))
                        && result.is_some_and(|result| self.compatible(result, output)),
                    "native typed array property",
                )?;
                self.helpers.require(Helper::Binary);
                Ok(())
            }
            OperationKind::Intrinsic(ResolvedIntrinsic::Property(
                intrinsic @ (Intrinsic::MapSize | Intrinsic::SetSize),
            )) => {
                expect(
                    operand(0)
                        == Stored(if *intrinsic == Intrinsic::MapSize {
                            NativeType::Map
                        } else {
                            NativeType::Set
                        })
                        && result == Some(Stored(I32)),
                    "native collection size",
                )?;
                self.helpers.require(Helper::Collections);
                Ok(())
            }
            OperationKind::Intrinsic(ResolvedIntrinsic::Property(Intrinsic::ArrayLength)) => {
                expect(
                    matches!(operand(0), Stored(NativeType::Array(_)))
                        && result == Some(Stored(I32)),
                    "native array length",
                )?;
                self.helpers.require(Helper::ClosureRuntime);
                Ok(())
            }
            OperationKind::IntBinary(kind) => {
                expect(
                    operands.len() == 2
                        && operand(0) == Stored(I32)
                        && operand(1) == Stored(I32)
                        && result == Some(Stored(I32)),
                    "native integer recipe",
                )?;
                self.helpers.require(match kind {
                    IntBinary::Add | IntBinary::Subtract => Helper::FromU32,
                    IntBinary::Multiply => Helper::Multiply,
                    IntBinary::Divide => Helper::Divide,
                    IntBinary::Remainder => Helper::Remainder,
                    IntBinary::UnsignedShiftRight => Helper::UnsignedShiftRight,
                });
                Ok(())
            }
            OperationKind::Binary(BinaryOp::Add) if result == Some(Stored(Text)) => {
                expect(
                    operands.len() == 2 && (0..2).all(|index| stringable(operand(index))),
                    "native string concatenation",
                )?;
                self.helpers.require(Helper::Strings);
                Ok(())
            }
            OperationKind::Binary(
                kind @ (BinaryOp::Eq
                | BinaryOp::NotEq
                | BinaryOp::Less
                | BinaryOp::LessEq
                | BinaryOp::Greater
                | BinaryOp::GreaterEq),
            ) if (operand(0) == Stored(Text) || operand(1) == Stored(Text))
                && !matches!(operand(0), Stored(NativeType::Dynamic(_)))
                && !matches!(operand(1), Stored(NativeType::Dynamic(_))) =>
            {
                expect(
                    operand(0) == Stored(Text)
                        && operand(1) == Stored(Text)
                        && result == Some(Stored(Bool)),
                    "native string comparison",
                )?;
                self.helpers
                    .require(if matches!(kind, BinaryOp::Eq | BinaryOp::NotEq) {
                        Helper::StringEqual
                    } else {
                        Helper::Strings
                    });
                Ok(())
            }
            OperationKind::Template => {
                expect(
                    result == Some(Stored(Text))
                        && operands.iter().all(|&id| stringable(value(id))),
                    "native template operands",
                )?;
                self.helpers.require(Helper::Strings);
                Ok(())
            }
            OperationKind::Binary(kind) => match kind {
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
                    expect(
                        numeric(operand(0)) && numeric(operand(1)) && result == Some(Stored(F64)),
                        "native floating binary recipe",
                    )?;
                    self.helpers.require(Helper::RoundBinary64);
                    Ok(())
                }
                BinaryOp::BitAnd
                | BinaryOp::BitOr
                | BinaryOp::Xor
                | BinaryOp::ShiftLeft
                | BinaryOp::ShiftRight
                | BinaryOp::UnsignedShiftRight => {
                    expect(
                        operand(0) == Stored(I32)
                            && operand(1) == Stored(I32)
                            && result == Some(Stored(I32)),
                        "native bitwise recipe",
                    )?;
                    self.helpers.require(match kind {
                        BinaryOp::ShiftLeft => Helper::ShiftLeft,
                        BinaryOp::ShiftRight => Helper::ShiftRight,
                        BinaryOp::UnsignedShiftRight => Helper::UnsignedShiftRight,
                        _ => Helper::FromU32,
                    });
                    Ok(())
                }
                BinaryOp::Eq | BinaryOp::NotEq => expect(
                    result == Some(Stored(Bool))
                        && ((numeric(operand(0)) && numeric(operand(1)))
                            || (operand(0) == Stored(Bool) && operand(1) == Stored(Bool))
                            // References compare by identity.
                            || (matches!(operand(0), Stored(NativeType::Object(_)))
                                && matches!(operand(1), Stored(NativeType::Object(_))))
                            || (matches!(
                                operand(0),
                                Stored(
                                    NativeType::Map
                                        | NativeType::Set
                                        | NativeType::Symbol
                                        | NativeType::Buffer
                                        | NativeType::Typed(_)
                                )
                            ) && operand(0) == operand(1))
                            // Tagged operands compare by JavaScript strict equality.
                            || ((matches!(operand(0), Stored(NativeType::Dynamic(_)))
                                || matches!(operand(1), Stored(NativeType::Dynamic(_))))
                                && self.compatible(Stored(NativeType::Dynamic(Tagged::ANY)), operand(0))
                                && self.compatible(Stored(NativeType::Dynamic(Tagged::ANY)), operand(1)))
                            || (matches!(self.value_type(operand(0)), NativeType::Callable(_))
                                && self.value_type(operand(0)) == self.value_type(operand(1))
                                && !matches!(operand(0), ValueStorage::Host(_))
                                && !matches!(operand(1), ValueStorage::Host(_)))),
                    "native equality recipe",
                ),
                BinaryOp::Less | BinaryOp::LessEq | BinaryOp::Greater | BinaryOp::GreaterEq => {
                    expect(
                        result == Some(Stored(Bool)) && numeric(operand(0)) && numeric(operand(1)),
                        "native ordered comparison",
                    )
                }
                _ => Err(error("native binary recipe")),
            },
            OperationKind::Unary {
                op: UnaryOp::Neg,
                integer: true,
            } => {
                expect(
                    operand(0) == Stored(I32) && result == Some(Stored(I32)),
                    "native integer negation",
                )?;
                self.helpers.require(Helper::FromU32);
                Ok(())
            }
            OperationKind::Unary {
                op: UnaryOp::Neg,
                integer: false,
            } => {
                expect(
                    operand(0) == Stored(F64) && result == Some(Stored(F64)),
                    "native floating negation",
                )?;
                self.helpers.require(Helper::RoundBinary64);
                Ok(())
            }
            OperationKind::Unary {
                op: UnaryOp::Not,
                integer: false,
            } => expect(
                operand(0) == Stored(Bool) && result == Some(Stored(Bool)),
                "native boolean negation",
            ),
            OperationKind::Intrinsic(ResolvedIntrinsic::Property(Intrinsic::StringLength)) => {
                expect(
                    operand(0) == Stored(Text) && result == Some(Stored(I32)),
                    "native string length",
                )?;
                self.helpers.require(Helper::StringLength);
                Ok(())
            }
            OperationKind::PrepareCall(call) => {
                if let CallTarget::Reference { place } = data.calls[call.index()].target {
                    if let Some(cell) = plan.places[place.index()].root_cell {
                        initialized(cell, budget)?;
                    }
                }
                Ok(())
            }
            OperationKind::PrepareReference { call, position } => {
                let argument =
                    data.arguments(data.calls[call.index()].arguments).unwrap()[*position as usize];
                let CallArgument::Reference(place) = argument else {
                    return Err(error("native reference preparation argument"));
                };
                let place = plan.places[place.index()];
                expect(
                    place.writable && matches!(place.storage, Stored(ty) if ty != Void),
                    "native reference address representation",
                )?;
                let cell = place
                    .root_cell
                    .ok_or_else(|| error("native reference temporary root"))?;
                initialized(cell, budget)
            }
            OperationKind::Call(call) => {
                let site = &data.calls[call.index()];
                let arguments = data.arguments(site.arguments).unwrap();
                let argument = |index: usize| match arguments.get(index) {
                    Some(CallArgument::Value(id)) => Some(value(*id)),
                    _ => None,
                };
                // A caller-evaluated default follows the supplied arguments;
                // a preserved omission would need `undefined`.
                expect(
                    site.contract.supplied as usize == arguments.len()
                        || site.contract.defaults == DefaultConvention::MaterializeAtCaller,
                    "native omitted arguments",
                )?;
                match plan.calls[call.index()] {
                    PreparedTarget::Function(function) => {
                        expect(
                            site.contract.defaults == DefaultConvention::MaterializeAtCaller,
                            "native direct call defaults",
                        )?;
                        let target = self.program.unit(function).unwrap();
                        let signature = self.signature_for_unit(function);
                        expect(
                            target.parameters.len() == arguments.len()
                                || self.omits_only_arrow_defaults(signature, arguments.len()),
                            "native direct call arity",
                        )?;
                        for (&parameter, &argument) in target.parameters.iter().zip(arguments) {
                            work(budget, 1)?;
                            let expected = self.cell_storage(parameter);
                            let compatible = match argument {
                                CallArgument::Value(argument) => {
                                    !self.reference_parameter(parameter)
                                        && (self.compatible(expected, value(argument))
                                            || self.admit_adapter(
                                                expected,
                                                value(argument),
                                                budget,
                                            )?)
                                }
                                CallArgument::Reference(place) => {
                                    self.reference_parameter(parameter)
                                        && plan.places[place.index()].writable
                                        && expected == plan.places[place.index()].storage
                                }
                            };
                            expect(compatible, "native call argument representation")?;
                        }
                        let expected = self.signatures[self.signature_for_unit(function)].result;
                        expect(
                            result.is_some_and(|result| self.compatible(result, Stored(expected))),
                            "native call return representation",
                        )
                    }
                    PreparedTarget::Callable { signature, .. }
                    | PreparedTarget::Placed { signature, .. }
                    | PreparedTarget::Host { signature, .. } => {
                        // A host call preserves omissions; a C provider takes
                        // every parameter, so only a complete call reaches one.
                        let complete =
                            arguments.len() == self.signatures[signature].parameters.len();
                        expect(
                            match site.contract.defaults {
                                DefaultConvention::MaterializeAtCaller => {
                                    complete
                                        || self
                                            .omits_only_arrow_defaults(signature, arguments.len())
                                }
                                DefaultConvention::PreserveOmission => {
                                    complete
                                        && matches!(
                                            plan.calls[call.index()],
                                            PreparedTarget::Host { .. }
                                        )
                                }
                            },
                            "native callable arity/default convention",
                        )?;
                        for (position, argument) in arguments.iter().enumerate() {
                            work(budget, 1)?;
                            let expected = Stored(self.signatures[signature].parameters[position]);
                            let passing =
                                self.signatures[signature].source.params[position].passing;
                            let compatible = match argument {
                                CallArgument::Value(argument) => {
                                    passing == crate::primitive::ParameterPassing::Value
                                        && (self.compatible(expected, value(*argument))
                                            || self.admit_adapter(
                                                expected,
                                                value(*argument),
                                                budget,
                                            )?)
                                }
                                CallArgument::Reference(place) => {
                                    passing == crate::primitive::ParameterPassing::MutableReference
                                        && plan.places[place.index()].writable
                                        && expected == plan.places[place.index()].storage
                                }
                            };
                            expect(compatible, "native callable argument representation")?;
                        }
                        let produced = self.signatures[signature].result;
                        expect(
                            result.is_some_and(|result| self.compatible(result, Stored(produced))),
                            "native callable result representation",
                        )
                    }
                    PreparedTarget::Print => {
                        expect(
                            arguments.len() == 1
                                && matches!(
                                    argument(0),
                                    Some(Stored(I32 | Bool | F64 | Text | NativeType::Dynamic(_)))
                                )
                                && result == Some(Stored(Void)),
                            "native print representation",
                        )?;
                        if matches!(argument(0), Some(Stored(F64 | Text))) {
                            self.helpers.require(Helper::Strings);
                        }
                        Ok(())
                    }
                    PreparedTarget::Construct(intrinsic) => {
                        let (admitted, output) = match intrinsic {
                            Intrinsic::MapNew => (arguments.is_empty(), NativeType::Map),
                            Intrinsic::SetNew => (arguments.is_empty(), NativeType::Set),
                            Intrinsic::ArrayBufferNew | Intrinsic::SharedArrayBufferNew => (
                                arguments.len() == 1 && argument(0) == Some(Stored(I32)),
                                NativeType::Buffer,
                            ),
                            intrinsic if intrinsic != Intrinsic::SymbolNew => {
                                let (kind, _) =
                                    crate::typed_array::classify_typed_array_intrinsic(intrinsic)
                                        .unwrap();
                                (
                                    arguments.len() == 1
                                        && matches!(
                                            argument(0),
                                            Some(Stored(I32 | NativeType::Buffer))
                                        ),
                                    NativeType::Typed(kind),
                                )
                            }
                            _ => (
                                arguments.is_empty()
                                    || (arguments.len() == 1 && argument(0) == Some(Stored(Text))),
                                NativeType::Symbol,
                            ),
                        };
                        expect(
                            admitted && result == Some(Stored(output)),
                            "native builtin construction",
                        )
                    }
                    PreparedTarget::BinaryMethod { receiver, .. } => expect(
                        // Range ends are materialized by the caller.
                        arguments.len() == 2
                            && argument(0) == Some(Stored(I32))
                            && argument(1) == Some(Stored(I32))
                            && result == Some(value(receiver)),
                        "native binary range operands",
                    ),
                    PreparedTarget::CollectionMethod { receiver, method } => {
                        // Every key and value travels as a tagged entry.
                        let tagged = |position: usize| {
                            argument(position).is_some_and(|value| {
                                self.compatible(Stored(NativeType::Dynamic(Tagged::ANY)), value)
                            })
                        };
                        let own = value(receiver);
                        let (admitted, output) = match method {
                            Intrinsic::MapGet => (
                                arguments.len() == 1 && tagged(0),
                                result.filter(|result| {
                                    matches!(result, Stored(NativeType::Dynamic(_)))
                                }),
                            ),
                            Intrinsic::MapSet => {
                                (arguments.len() == 2 && tagged(0) && tagged(1), Some(own))
                            }
                            Intrinsic::SetAdd => (arguments.len() == 1 && tagged(0), Some(own)),
                            Intrinsic::MapHas
                            | Intrinsic::MapDelete
                            | Intrinsic::SetHas
                            | Intrinsic::SetDelete => {
                                (arguments.len() == 1 && tagged(0), Some(Stored(Bool)))
                            }
                            _ => (arguments.is_empty(), Some(Stored(Void))),
                        };
                        expect(
                            admitted
                                && output.is_some()
                                && result == output
                                && site.contract.defaults == DefaultConvention::PreserveOmission,
                            "native collection method operands",
                        )
                    }
                    PreparedTarget::ScalarMethod { method, .. } => {
                        let count = arguments.len();
                        let int = |position: usize| argument(position) == Some(Stored(I32));
                        let text = |position: usize| argument(position) == Some(Stored(Text));
                        let float =
                            |position: usize| numeric(argument(position).unwrap_or(Stored(Void)));
                        let (admitted, output) = match method {
                            Intrinsic::StringIncludes
                            | Intrinsic::StringStartsWith
                            | Intrinsic::StringEndsWith => (count == 1 && text(0), Bool),
                            Intrinsic::StringIndexOf | Intrinsic::StringLastIndexOf => {
                                ((count == 1 || (count == 2 && int(1))) && text(0), I32)
                            }
                            Intrinsic::StringRepeat => (count == 1 && int(0), Text),
                            Intrinsic::StringToUpperCase
                            | Intrinsic::StringToLowerCase
                            | Intrinsic::StringTrim
                            | Intrinsic::StringTrimStart
                            | Intrinsic::StringTrimEnd => (count == 0, Text),
                            Intrinsic::StringSlice => {
                                ((1..=2).contains(&count) && (0..count).all(int), Text)
                            }
                            Intrinsic::StringSplit => (
                                count == 1 && text(0) && self.string_array().is_some(),
                                NativeType::Array(self.string_array().unwrap_or(usize::MAX)),
                            ),
                            Intrinsic::StringCodePointLength => (count == 0, I32),
                            Intrinsic::IntToString | Intrinsic::IntToUnsignedString => {
                                (count == 0 || (count == 1 && int(0)), Text)
                            }
                            Intrinsic::FloatAtan2
                            | Intrinsic::FloatHypot
                            | Intrinsic::FloatMin
                            | Intrinsic::FloatMax => (count == 1 && float(0), F64),
                            Intrinsic::FloatToInt => (count == 0, I32),
                            _ => (count == 0, F64),
                        };
                        expect(
                            admitted
                                && result == Some(Stored(output))
                                && site.contract.defaults == DefaultConvention::PreserveOmission,
                            "native scalar method operands",
                        )
                    }
                    PreparedTarget::MathImul => expect(
                        arguments.len() == 2
                            && argument(0) == Some(Stored(I32))
                            && argument(1) == Some(Stored(I32))
                            && result == Some(Stored(I32)),
                        "native imul representation",
                    ),
                    PreparedTarget::ArrayPush { array, .. } => expect(
                        arguments.len() == 1
                            && argument(0).is_some_and(|value| {
                                self.compatible(Stored(self.arrays[array]), value)
                            })
                            && result == Some(Stored(I32)),
                        "native array push operands",
                    ),
                    PreparedTarget::ArrayPop { array, .. } => expect(
                        arguments.is_empty()
                            && result.is_some_and(|result| {
                                self.compatible(result, Stored(self.arrays[array]))
                            }),
                        "native array pop operands",
                    ),
                    PreparedTarget::ArrayMethod {
                        array: kind,
                        method,
                        ..
                    } => {
                        let item = Stored(self.arrays[kind]);
                        let array = Stored(NativeType::Array(kind));
                        // A callback's physical signature: exact arity, each
                        // parameter accepting what the loop passes.
                        let callback = |position: usize, passed: &[ValueStorage]| {
                            let Some(storage) = argument(position) else {
                                return None;
                            };
                            let NativeType::Callable(signature) = self.value_type(storage) else {
                                return None;
                            };
                            let signature = &self.signatures[signature];
                            (signature.parameters.len() == passed.len()
                                && signature.parameters.iter().zip(passed).all(
                                    |(&parameter, &value)| {
                                        self.compatible(Stored(parameter), value)
                                    },
                                )
                                && signature.source.params.iter().all(|parameter| {
                                    parameter.passing == crate::primitive::ParameterPassing::Value
                                }))
                            .then_some(signature.result)
                        };
                        let int = |position: usize| argument(position) == Some(Stored(I32));
                        let admitted = match method {
                            Intrinsic::ArrayForEach => {
                                arguments.len() == 1
                                    && callback(0, &[item]).is_some()
                                    && result == Some(Stored(Void))
                            }
                            Intrinsic::ArrayMap => {
                                arguments.len() == 1
                                    && match (callback(0, &[item]), result) {
                                        (Some(mapped), Some(Stored(NativeType::Array(out)))) => {
                                            self.arrays[out] == mapped
                                        }
                                        _ => false,
                                    }
                            }
                            Intrinsic::ArrayFilter => {
                                arguments.len() == 1
                                    && callback(0, &[item]) == Some(Bool)
                                    && result == Some(array)
                            }
                            Intrinsic::ArraySome | Intrinsic::ArrayEvery => {
                                arguments.len() == 1
                                    && callback(0, &[item]) == Some(Bool)
                                    && result == Some(Stored(Bool))
                            }
                            Intrinsic::ArrayFindIndex => {
                                arguments.len() == 1
                                    && callback(0, &[item]) == Some(Bool)
                                    && result == Some(Stored(I32))
                            }
                            Intrinsic::ArrayReduce => match (argument(1), result) {
                                (Some(initial), Some(Stored(accumulator))) => {
                                    arguments.len() == 2
                                        && !accumulator.managed()
                                        && accumulator != Void
                                        && self.compatible(Stored(accumulator), initial)
                                        && callback(0, &[Stored(accumulator), item]).is_some_and(
                                            |next| {
                                                self.compatible(Stored(accumulator), Stored(next))
                                            },
                                        )
                                }
                                _ => false,
                            },
                            Intrinsic::ArrayIndexOf => {
                                arguments.len() == 1
                                    && argument(0).is_some_and(|value| self.compatible(item, value))
                                    && result == Some(Stored(I32))
                            }
                            Intrinsic::ArrayIncludes => {
                                (1..=2).contains(&arguments.len())
                                    && argument(0).is_some_and(|value| self.compatible(item, value))
                                    && (arguments.len() == 1 || int(1))
                                    && result == Some(Stored(Bool))
                            }
                            Intrinsic::ArrayConcat => {
                                arguments.len() == 1
                                    && argument(0) == Some(array)
                                    && result == Some(array)
                            }
                            Intrinsic::ArrayReverse => {
                                arguments.is_empty() && result == Some(array)
                            }
                            Intrinsic::ArraySlice => {
                                arguments.len() <= 2
                                    && (0..arguments.len()).all(int)
                                    && result == Some(array)
                            }
                            Intrinsic::ArraySplice => {
                                arguments.len() == 2 && int(0) && int(1) && result == Some(array)
                            }
                            Intrinsic::ArrayFill => {
                                arguments.len() == 1
                                    && argument(0).is_some_and(|value| self.compatible(item, value))
                                    && result == Some(array)
                            }
                            Intrinsic::ArrayCopyWithin => {
                                (2..=3).contains(&arguments.len())
                                    && (0..arguments.len()).all(int)
                                    && result == Some(array)
                            }
                            _ => false,
                        };
                        expect(
                            admitted
                                && site.contract.defaults == DefaultConvention::PreserveOmission,
                            "native array method operands",
                        )
                    }
                    PreparedTarget::CharCodeAt { .. } | PreparedTarget::CharAt { .. } => {
                        let expected =
                            if matches!(plan.calls[call.index()], PreparedTarget::CharAt { .. }) {
                                Text
                            } else {
                                I32
                            };
                        expect(
                            site.contract.defaults == DefaultConvention::PreserveOmission
                                && arguments.len() == 1
                                && argument(0) == Some(Stored(I32))
                                && result == Some(Stored(expected)),
                            "native string method operands",
                        )
                    }
                }
            }
            OperationKind::Closure(child) => {
                let body = self.program.unit(*child).unwrap();
                expect(
                    result.is_some_and(|result| {
                        self.value_type(result)
                            == NativeType::Callable(self.signature_for_unit(*child))
                    }),
                    "native closure representation",
                )?;
                for &cell in &body.captures {
                    work(budget, 1)?;
                    if matches!(
                        self.cell_storage(cell),
                        ValueStorage::Function(_) | ValueStorage::Host(_)
                    ) {
                        continue;
                    }
                    // Creating a function reads no module binding; its body's
                    // guarded accesses do, whenever it runs.
                    if self.global_cell(cell) {
                        continue;
                    }
                    initialized(cell, budget)?;
                    expect(self.boxed_cell(cell), "native closure captured box")?;
                }
                Ok(())
            }
            // `left ?? right`: the present left, unboxed as the result needs,
            // or the lazily evaluated right.
            OperationKind::ShortCircuit {
                kind: ShortCircuit::Nullish,
                right,
            } => expect(
                matches!(operand(0), Stored(NativeType::Dynamic(_)))
                    && result.is_some_and(|result| {
                        matches!(result, Stored(ty) if ty != Void)
                            && self.compatible(result, operand(0))
                            && region_result(*right)
                                .is_some_and(|value| self.compatible(result, value))
                    }),
                "native nullish coalescing",
            ),
            OperationKind::ShortCircuit {
                kind: ShortCircuit::BooleanAnd | ShortCircuit::BooleanOr,
                right,
            } => expect(
                operand(0) == Stored(Bool)
                    && region_result(*right) == Some(Stored(Bool))
                    && result == Some(Stored(Bool)),
                "native lazy boolean",
            ),
            OperationKind::Select { yes, no } => expect(
                operand(0) == Stored(Bool)
                    && result.is_some_and(|target| {
                        matches!(target, Stored(ty) if ty != Void)
                            && region_result(*yes)
                                .is_some_and(|value| self.compatible(target, value))
                            && region_result(*no)
                                .is_some_and(|value| self.compatible(target, value))
                    }),
                "native selected value",
            ),
            OperationKind::If { .. } => expect(operand(0) == Stored(Bool), "native if condition"),
            OperationKind::Loop { test, .. } => expect(
                region_result(*test).is_none_or(|value| value == Stored(Bool)),
                "native loop condition",
            ),
            OperationKind::Return => expect(
                match operands {
                    [] => plan.return_type == Void,
                    [value] => {
                        self.compatible(Stored(plan.return_type), plan.values[value.index()])
                    }
                    _ => false,
                },
                "native return representation",
            ),
            // Host-class inheritance is JavaScript-only: its instances are
            // host objects with a native prototype chain.
            OperationKind::ConstructClass | OperationKind::SuperConstruct => {
                Err(error("native host class inheritance"))
            }
            // Only an omitted arrow default arrives as the empty callable;
            // every other default was evaluated by the caller.
            OperationKind::IsUndefined => expect(
                operands.len() == 1 && result == Some(Stored(Bool)),
                "native undefined test",
            ),
            OperationKind::Block(_) | OperationKind::Break | OperationKind::Continue => Ok(()),
            _ => Err(error("native operation coverage")),
        }
    }
}

/// All module callable prefixes instantiate before any module evaluation suffix.
/// Static C symbols implement only these explicit, effect-free initializations;
/// their existence cannot make an ordinary later cell initialization entry-live.
/// Common verification owns prefix pairing and module order. This target check
/// confirms that every retained function cell has the supported static recipe.
fn prove_function_prefix(
    program: &Program<'_>,
    cells: &[CellPlan],
    mut remaining: usize,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), NativeError> {
    for &unit in program.initialization.iter() {
        work(budget, 1)?;
        let data = program.unit(unit).unwrap();
        let Some(prefix) = data.regions[data.entry.index()]
            .operations
            .get(..data.instantiation_prefix as usize)
        else {
            return Err(fail(
                Some(unit),
                None,
                data.regions[data.entry.index()].span,
                "native function initialization prefix",
            ));
        };
        for &id in prefix {
            work(budget, 1)?;
            let operation = &data.operations[id.index()];
            match operation.kind {
                OperationKind::Closure(child)
                    if program.unit(child).unwrap().kind == UnitKind::Function => {}
                OperationKind::Initialize(cell)
                    if program.cells[cell.index()].owner == unit
                        && matches!(cells[cell.index()].storage, ValueStorage::Function(_)) =>
                {
                    remaining = remaining.checked_sub(1).ok_or_else(|| {
                        fail(
                            Some(unit),
                            Some(id),
                            operation.span,
                            "native function initialization prefix",
                        )
                    })?;
                }
                _ => {
                    return Err(fail(
                        Some(unit),
                        Some(id),
                        operation.span,
                        "native function initialization prefix",
                    ))
                }
            }
        }
    }
    if remaining == 0 {
        Ok(())
    } else {
        Err(fail(
            None,
            None,
            Span::default(),
            "native incomplete function initialization prefix",
        ))
    }
}

/// Conservative target obligation: C nonvoid functions must not fall through.
/// Loops never establish return here. A final Return, a returning Block, or an
/// If with both arms returning is sufficient; source completion stays structured.
fn must_return(data: &UnitData, budget: &mut AllocationBudget<'_>) -> Result<bool, NativeError> {
    let mut returning = budget
        .filled(Scratch, data.regions.len(), false)
        .map_err(NativeError::Allocation)?;
    let capacity = data
        .regions
        .len()
        .checked_mul(2)
        .ok_or(NativeError::Allocation(AllocationError::Capacity))?;
    let mut pending = budget
        .vector(Scratch, capacity)
        .map_err(NativeError::Allocation)?;
    pending.push((data.entry, false));
    while let Some((region, visited)) = pending.pop() {
        work(budget, 1)?;
        if !visited {
            pending.push((region, true));
            for &operation in &data.regions[region.index()].operations {
                work(budget, 1)?;
                for child in data.operations[operation.index()].kind.child_regions() {
                    work(budget, 1)?;
                    assert!(
                        pending.len() < pending.capacity(),
                        "checked region tree bounds native completion stack"
                    );
                    pending.push((child, false));
                }
            }
        } else {
            for &operation in &data.regions[region.index()].operations {
                work(budget, 1)?;
                let ends = match data.operations[operation.index()].kind {
                    OperationKind::Return => true,
                    OperationKind::Block(child) => returning[child.index()],
                    OperationKind::If { yes, no: Some(no) } => {
                        returning[yes.index()] && returning[no.index()]
                    }
                    _ => false,
                };
                if ends {
                    returning[region.index()] = true;
                    break;
                }
            }
        }
    }
    let result = returning[data.entry.index()];
    release(pending, budget)?;
    release(returning, budget)?;
    Ok(result)
}

#[cfg(test)]
#[path = "native_plan_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "native_closure_plan_tests.rs"]
mod closure_tests;
