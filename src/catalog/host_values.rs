//! Return categories of pristine host calls. A category describes successful
//! completion only: it supplies no purity, termination or exception permission.
//! Consumers must establish the pristine-host contract before using these rows.
use super::Global;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum HostResult {
    Number,
    Boolean,
    String,
}

pub(crate) fn host_result(global: Global, member: Option<&str>) -> Option<HostResult> {
    use HostResult::*;
    Some(match (global, member) {
        (Global::ParseInt | Global::ParseFloat | Global::Number, None) => Number,
        (Global::IsNaN | Global::IsFinite | Global::Boolean, None) => Boolean,
        (
            Global::String
            | Global::EncodeUri
            | Global::EncodeUriComponent
            | Global::DecodeUriComponent,
            None,
        ) => String,
        (
            Global::Math,
            Some(
                "abs" | "acos" | "acosh" | "asin" | "asinh" | "atan" | "atan2" | "atanh" | "cbrt"
                | "ceil" | "clz32" | "cos" | "cosh" | "exp" | "expm1" | "floor" | "fround"
                | "hypot" | "imul" | "log" | "log10" | "log1p" | "log2" | "max" | "min" | "pow"
                | "random" | "round" | "sign" | "sin" | "sinh" | "sqrt" | "tan" | "tanh" | "trunc",
            ),
        ) => Number,
        (Global::Date, Some("now")) | (Global::Number, Some("parseInt" | "parseFloat")) => Number,
        (Global::Number, Some("isInteger" | "isFinite" | "isNaN" | "isSafeInteger"))
        | (Global::Array, Some("isArray"))
        | (Global::Object, Some("is" | "isFrozen" | "isSealed" | "isExtensible" | "hasOwn"))
        | (Global::Reflect, Some("has")) => Boolean,
        (Global::String, Some("fromCharCode" | "fromCodePoint" | "raw")) => String,
        _ => return None,
    })
}

pub(crate) fn host_prototype_result(global: Global, member: &str) -> Option<HostResult> {
    match (global, member) {
        (Global::Object, "toString") => Some(HostResult::String),
        (Global::Object, "hasOwnProperty" | "propertyIsEnumerable" | "isPrototypeOf") => {
            Some(HostResult::Boolean)
        }
        _ => None,
    }
}
