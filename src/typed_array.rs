use crate::ir::Intrinsic;
use crate::semantic::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypedArrayKind {
    Int8,
    Uint8,
    Uint8Clamped,
    Int16,
    Uint16,
    Int32,
    Uint32,
    Float32,
    Float64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypedArrayIntrinsic {
    New,
    Length,
    ByteLength,
    ByteOffset,
    Buffer,
    Slice,
    Subarray,
}

impl TypedArrayKind {
    pub const ALL: [Self; 9] = [
        Self::Int8,
        Self::Uint8,
        Self::Uint8Clamped,
        Self::Int16,
        Self::Uint16,
        Self::Int32,
        Self::Uint32,
        Self::Float32,
        Self::Float64,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Int8 => "Int8Array",
            Self::Uint8 => "Uint8Array",
            Self::Uint8Clamped => "Uint8ClampedArray",
            Self::Int16 => "Int16Array",
            Self::Uint16 => "Uint16Array",
            Self::Int32 => "Int32Array",
            Self::Uint32 => "Uint32Array",
            Self::Float32 => "Float32Array",
            Self::Float64 => "Float64Array",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }

    pub fn bytes_per_element(self) -> u32 {
        match self {
            Self::Int8 | Self::Uint8 | Self::Uint8Clamped => 1,
            Self::Int16 | Self::Uint16 => 2,
            Self::Int32 | Self::Uint32 | Self::Float32 => 4,
            Self::Float64 => 8,
        }
    }

    pub fn element_is_float(self) -> bool {
        matches!(self, Self::Float32 | Self::Float64)
    }

    pub fn clamps_on_write(self) -> bool {
        matches!(self, Self::Uint8Clamped)
    }

    pub fn as_type<'src>(self) -> Type<'src> {
        match self {
            Self::Int8 => Type::Int8Array,
            Self::Uint8 => Type::Uint8Array,
            Self::Uint8Clamped => Type::Uint8ClampedArray,
            Self::Int16 => Type::Int16Array,
            Self::Uint16 => Type::Uint16Array,
            Self::Int32 => Type::Int32Array,
            Self::Uint32 => Type::Uint32Array,
            Self::Float32 => Type::Float32Array,
            Self::Float64 => Type::Float64Array,
        }
    }

    pub fn from_type(ty: &Type<'_>) -> Option<Self> {
        match ty {
            Type::Int8Array => Some(Self::Int8),
            Type::Uint8Array => Some(Self::Uint8),
            Type::Uint8ClampedArray => Some(Self::Uint8Clamped),
            Type::Int16Array => Some(Self::Int16),
            Type::Uint16Array => Some(Self::Uint16),
            Type::Int32Array => Some(Self::Int32),
            Type::Uint32Array => Some(Self::Uint32),
            Type::Float32Array => Some(Self::Float32),
            Type::Float64Array => Some(Self::Float64),
            _ => None,
        }
    }

    pub fn index_value_type<'src>(self) -> Type<'src> {
        if self.element_is_float() {
            Type::Float
        } else {
            Type::Int
        }
    }

    pub fn new_intrinsic(self) -> Intrinsic {
        match self {
            Self::Int8 => Intrinsic::Int8ArrayNew,
            Self::Uint8 => Intrinsic::Uint8ArrayNew,
            Self::Uint8Clamped => Intrinsic::Uint8ClampedArrayNew,
            Self::Int16 => Intrinsic::Int16ArrayNew,
            Self::Uint16 => Intrinsic::Uint16ArrayNew,
            Self::Int32 => Intrinsic::Int32ArrayNew,
            Self::Uint32 => Intrinsic::Uint32ArrayNew,
            Self::Float32 => Intrinsic::Float32ArrayNew,
            Self::Float64 => Intrinsic::Float64ArrayNew,
        }
    }

    pub fn length_intrinsic(self) -> Intrinsic {
        match self {
            Self::Int8 => Intrinsic::Int8ArrayLength,
            Self::Uint8 => Intrinsic::Uint8ArrayLength,
            Self::Uint8Clamped => Intrinsic::Uint8ClampedArrayLength,
            Self::Int16 => Intrinsic::Int16ArrayLength,
            Self::Uint16 => Intrinsic::Uint16ArrayLength,
            Self::Int32 => Intrinsic::Int32ArrayLength,
            Self::Uint32 => Intrinsic::Uint32ArrayLength,
            Self::Float32 => Intrinsic::Float32ArrayLength,
            Self::Float64 => Intrinsic::Float64ArrayLength,
        }
    }

    pub fn byte_length_intrinsic(self) -> Intrinsic {
        match self {
            Self::Int8 => Intrinsic::Int8ArrayByteLength,
            Self::Uint8 => Intrinsic::Uint8ArrayByteLength,
            Self::Uint8Clamped => Intrinsic::Uint8ClampedArrayByteLength,
            Self::Int16 => Intrinsic::Int16ArrayByteLength,
            Self::Uint16 => Intrinsic::Uint16ArrayByteLength,
            Self::Int32 => Intrinsic::Int32ArrayByteLength,
            Self::Uint32 => Intrinsic::Uint32ArrayByteLength,
            Self::Float32 => Intrinsic::Float32ArrayByteLength,
            Self::Float64 => Intrinsic::Float64ArrayByteLength,
        }
    }

    pub fn byte_offset_intrinsic(self) -> Intrinsic {
        match self {
            Self::Int8 => Intrinsic::Int8ArrayByteOffset,
            Self::Uint8 => Intrinsic::Uint8ArrayByteOffset,
            Self::Uint8Clamped => Intrinsic::Uint8ClampedArrayByteOffset,
            Self::Int16 => Intrinsic::Int16ArrayByteOffset,
            Self::Uint16 => Intrinsic::Uint16ArrayByteOffset,
            Self::Int32 => Intrinsic::Int32ArrayByteOffset,
            Self::Uint32 => Intrinsic::Uint32ArrayByteOffset,
            Self::Float32 => Intrinsic::Float32ArrayByteOffset,
            Self::Float64 => Intrinsic::Float64ArrayByteOffset,
        }
    }

    pub fn buffer_intrinsic(self) -> Intrinsic {
        match self {
            Self::Int8 => Intrinsic::Int8ArrayBuffer,
            Self::Uint8 => Intrinsic::Uint8ArrayBuffer,
            Self::Uint8Clamped => Intrinsic::Uint8ClampedArrayBuffer,
            Self::Int16 => Intrinsic::Int16ArrayBuffer,
            Self::Uint16 => Intrinsic::Uint16ArrayBuffer,
            Self::Int32 => Intrinsic::Int32ArrayBuffer,
            Self::Uint32 => Intrinsic::Uint32ArrayBuffer,
            Self::Float32 => Intrinsic::Float32ArrayBuffer,
            Self::Float64 => Intrinsic::Float64ArrayBuffer,
        }
    }

    pub fn slice_intrinsic(self) -> Intrinsic {
        match self {
            Self::Int8 => Intrinsic::Int8ArraySlice,
            Self::Uint8 => Intrinsic::Uint8ArraySlice,
            Self::Uint8Clamped => Intrinsic::Uint8ClampedArraySlice,
            Self::Int16 => Intrinsic::Int16ArraySlice,
            Self::Uint16 => Intrinsic::Uint16ArraySlice,
            Self::Int32 => Intrinsic::Int32ArraySlice,
            Self::Uint32 => Intrinsic::Uint32ArraySlice,
            Self::Float32 => Intrinsic::Float32ArraySlice,
            Self::Float64 => Intrinsic::Float64ArraySlice,
        }
    }

    pub fn subarray_intrinsic(self) -> Intrinsic {
        match self {
            Self::Int8 => Intrinsic::Int8ArraySubarray,
            Self::Uint8 => Intrinsic::Uint8ArraySubarray,
            Self::Uint8Clamped => Intrinsic::Uint8ClampedArraySubarray,
            Self::Int16 => Intrinsic::Int16ArraySubarray,
            Self::Uint16 => Intrinsic::Uint16ArraySubarray,
            Self::Int32 => Intrinsic::Int32ArraySubarray,
            Self::Uint32 => Intrinsic::Uint32ArraySubarray,
            Self::Float32 => Intrinsic::Float32ArraySubarray,
            Self::Float64 => Intrinsic::Float64ArraySubarray,
        }
    }

    pub fn property_intrinsic(self, property: &str) -> Option<Intrinsic> {
        Some(match property {
            "length" => self.length_intrinsic(),
            "byteLength" => self.byte_length_intrinsic(),
            "byteOffset" => self.byte_offset_intrinsic(),
            "buffer" => self.buffer_intrinsic(),
            _ => return None,
        })
    }

    pub fn method_intrinsic(self, method: &str) -> Option<Intrinsic> {
        Some(match method {
            "slice" => self.slice_intrinsic(),
            "subarray" => self.subarray_intrinsic(),
            _ => return None,
        })
    }

    pub fn native_ctype_alias(self) -> &'static str {
        match self {
            Self::Int8 => "LilScriptInt8Array",
            Self::Uint8 => "LilScriptUint8Array",
            Self::Uint8Clamped => "LilScriptUint8ClampedArray",
            Self::Int16 => "LilScriptInt16Array",
            Self::Uint16 => "LilScriptUint16Array",
            Self::Int32 => "LilScriptInt32Array",
            Self::Uint32 => "LilScriptUint32Array",
            Self::Float32 => "LilScriptFloat32Array",
            Self::Float64 => "LilScriptFloat64Array",
        }
    }

    pub fn generic_value_tag(self) -> u8 {
        match self {
            Self::Float32 => 8,
            Self::Float64 => 10,
            _ => 6,
        }
    }

    pub fn native_kind_code(self) -> i32 {
        match self {
            Self::Int8 => 0,
            Self::Uint8 => 1,
            Self::Uint8Clamped => 2,
            Self::Int16 => 3,
            Self::Uint16 => 4,
            Self::Int32 => 5,
            Self::Uint32 => 6,
            Self::Float32 => 7,
            Self::Float64 => 8,
        }
    }
}

pub fn is_typed_array_type(ty: &Type<'_>) -> bool {
    TypedArrayKind::from_type(ty).is_some()
}

pub fn classify_typed_array_intrinsic(
    intrinsic: Intrinsic,
) -> Option<(TypedArrayKind, TypedArrayIntrinsic)> {
    use Intrinsic::*;
    Some(match intrinsic {
        Int8ArrayNew => (TypedArrayKind::Int8, TypedArrayIntrinsic::New),
        Int8ArrayLength => (TypedArrayKind::Int8, TypedArrayIntrinsic::Length),
        Int8ArrayByteLength => (TypedArrayKind::Int8, TypedArrayIntrinsic::ByteLength),
        Int8ArrayByteOffset => (TypedArrayKind::Int8, TypedArrayIntrinsic::ByteOffset),
        Int8ArrayBuffer => (TypedArrayKind::Int8, TypedArrayIntrinsic::Buffer),
        Int8ArraySlice => (TypedArrayKind::Int8, TypedArrayIntrinsic::Slice),
        Int8ArraySubarray => (TypedArrayKind::Int8, TypedArrayIntrinsic::Subarray),
        Uint8ArrayNew => (TypedArrayKind::Uint8, TypedArrayIntrinsic::New),
        Uint8ArrayLength => (TypedArrayKind::Uint8, TypedArrayIntrinsic::Length),
        Uint8ArrayByteLength => (TypedArrayKind::Uint8, TypedArrayIntrinsic::ByteLength),
        Uint8ArrayByteOffset => (TypedArrayKind::Uint8, TypedArrayIntrinsic::ByteOffset),
        Uint8ArrayBuffer => (TypedArrayKind::Uint8, TypedArrayIntrinsic::Buffer),
        Uint8ArraySlice => (TypedArrayKind::Uint8, TypedArrayIntrinsic::Slice),
        Uint8ArraySubarray => (TypedArrayKind::Uint8, TypedArrayIntrinsic::Subarray),
        Uint8ClampedArrayNew => (TypedArrayKind::Uint8Clamped, TypedArrayIntrinsic::New),
        Uint8ClampedArrayLength => (TypedArrayKind::Uint8Clamped, TypedArrayIntrinsic::Length),
        Uint8ClampedArrayByteLength => (
            TypedArrayKind::Uint8Clamped,
            TypedArrayIntrinsic::ByteLength,
        ),
        Uint8ClampedArrayByteOffset => (
            TypedArrayKind::Uint8Clamped,
            TypedArrayIntrinsic::ByteOffset,
        ),
        Uint8ClampedArrayBuffer => (TypedArrayKind::Uint8Clamped, TypedArrayIntrinsic::Buffer),
        Uint8ClampedArraySlice => (TypedArrayKind::Uint8Clamped, TypedArrayIntrinsic::Slice),
        Uint8ClampedArraySubarray => (TypedArrayKind::Uint8Clamped, TypedArrayIntrinsic::Subarray),
        Int16ArrayNew => (TypedArrayKind::Int16, TypedArrayIntrinsic::New),
        Int16ArrayLength => (TypedArrayKind::Int16, TypedArrayIntrinsic::Length),
        Int16ArrayByteLength => (TypedArrayKind::Int16, TypedArrayIntrinsic::ByteLength),
        Int16ArrayByteOffset => (TypedArrayKind::Int16, TypedArrayIntrinsic::ByteOffset),
        Int16ArrayBuffer => (TypedArrayKind::Int16, TypedArrayIntrinsic::Buffer),
        Int16ArraySlice => (TypedArrayKind::Int16, TypedArrayIntrinsic::Slice),
        Int16ArraySubarray => (TypedArrayKind::Int16, TypedArrayIntrinsic::Subarray),
        Uint16ArrayNew => (TypedArrayKind::Uint16, TypedArrayIntrinsic::New),
        Uint16ArrayLength => (TypedArrayKind::Uint16, TypedArrayIntrinsic::Length),
        Uint16ArrayByteLength => (TypedArrayKind::Uint16, TypedArrayIntrinsic::ByteLength),
        Uint16ArrayByteOffset => (TypedArrayKind::Uint16, TypedArrayIntrinsic::ByteOffset),
        Uint16ArrayBuffer => (TypedArrayKind::Uint16, TypedArrayIntrinsic::Buffer),
        Uint16ArraySlice => (TypedArrayKind::Uint16, TypedArrayIntrinsic::Slice),
        Uint16ArraySubarray => (TypedArrayKind::Uint16, TypedArrayIntrinsic::Subarray),
        Int32ArrayNew => (TypedArrayKind::Int32, TypedArrayIntrinsic::New),
        Int32ArrayLength => (TypedArrayKind::Int32, TypedArrayIntrinsic::Length),
        Int32ArrayByteLength => (TypedArrayKind::Int32, TypedArrayIntrinsic::ByteLength),
        Int32ArrayByteOffset => (TypedArrayKind::Int32, TypedArrayIntrinsic::ByteOffset),
        Int32ArrayBuffer => (TypedArrayKind::Int32, TypedArrayIntrinsic::Buffer),
        Int32ArraySlice => (TypedArrayKind::Int32, TypedArrayIntrinsic::Slice),
        Int32ArraySubarray => (TypedArrayKind::Int32, TypedArrayIntrinsic::Subarray),
        Uint32ArrayNew => (TypedArrayKind::Uint32, TypedArrayIntrinsic::New),
        Uint32ArrayLength => (TypedArrayKind::Uint32, TypedArrayIntrinsic::Length),
        Uint32ArrayByteLength => (TypedArrayKind::Uint32, TypedArrayIntrinsic::ByteLength),
        Uint32ArrayByteOffset => (TypedArrayKind::Uint32, TypedArrayIntrinsic::ByteOffset),
        Uint32ArrayBuffer => (TypedArrayKind::Uint32, TypedArrayIntrinsic::Buffer),
        Uint32ArraySlice => (TypedArrayKind::Uint32, TypedArrayIntrinsic::Slice),
        Uint32ArraySubarray => (TypedArrayKind::Uint32, TypedArrayIntrinsic::Subarray),
        Float32ArrayNew => (TypedArrayKind::Float32, TypedArrayIntrinsic::New),
        Float32ArrayLength => (TypedArrayKind::Float32, TypedArrayIntrinsic::Length),
        Float32ArrayByteLength => (TypedArrayKind::Float32, TypedArrayIntrinsic::ByteLength),
        Float32ArrayByteOffset => (TypedArrayKind::Float32, TypedArrayIntrinsic::ByteOffset),
        Float32ArrayBuffer => (TypedArrayKind::Float32, TypedArrayIntrinsic::Buffer),
        Float32ArraySlice => (TypedArrayKind::Float32, TypedArrayIntrinsic::Slice),
        Float32ArraySubarray => (TypedArrayKind::Float32, TypedArrayIntrinsic::Subarray),
        Float64ArrayNew => (TypedArrayKind::Float64, TypedArrayIntrinsic::New),
        Float64ArrayLength => (TypedArrayKind::Float64, TypedArrayIntrinsic::Length),
        Float64ArrayByteLength => (TypedArrayKind::Float64, TypedArrayIntrinsic::ByteLength),
        Float64ArrayByteOffset => (TypedArrayKind::Float64, TypedArrayIntrinsic::ByteOffset),
        Float64ArrayBuffer => (TypedArrayKind::Float64, TypedArrayIntrinsic::Buffer),
        Float64ArraySlice => (TypedArrayKind::Float64, TypedArrayIntrinsic::Slice),
        Float64ArraySubarray => (TypedArrayKind::Float64, TypedArrayIntrinsic::Subarray),
        _ => return None,
    })
}

pub fn is_typed_array_range_intrinsic(intrinsic: Intrinsic) -> bool {
    matches!(
        classify_typed_array_intrinsic(intrinsic),
        Some((
            _,
            TypedArrayIntrinsic::Slice | TypedArrayIntrinsic::Subarray
        ))
    )
}
