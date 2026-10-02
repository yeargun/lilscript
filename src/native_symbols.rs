//! One namespace contract for native runtime, typed interfaces and public
//! exports. Source spellings are encoded injectively, never used as raw C.
use std::fmt;

pub(crate) fn provider(link: &str) -> bool {
    let Some(suffix) = link.strip_prefix("host_") else {
        return false;
    };
    let mut bytes = suffix.bytes();
    bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
pub(crate) fn public_prefix(prefix: &str) -> bool {
    let mut bytes = prefix.bytes();
    bytes.next().is_some_and(|b| b.is_ascii_alphabetic())
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
        && prefix != "ls"
        && prefix != "host"
        && !prefix.starts_with("ls_")
        && !prefix.starts_with("host_")
}
pub(crate) struct Component<'a>(pub &'a str);
impl fmt::Display for Component<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0.bytes() {
            if byte.is_ascii_alphanumeric() {
                write!(f, "{}", byte as char)?;
            } else {
                write!(f, "_{byte:02x}")?;
            }
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
pub(crate) struct ExportName<'a> {
    pub prefix: &'a str,
    pub entry: &'a str,
    pub name: &'a str,
}
impl fmt::Display for ExportName<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}_e_{}__{}",
            self.prefix,
            Component(self.entry),
            Component(self.name)
        )
    }
}
/// C headers and their implementation use precisely the same ABI spellings.
/// Library formation maps these canonical recipe tokens before preprocessing,
/// then removes header-local mappings so two library headers can coexist.
pub(crate) const ABI_NAMES: &[&str] = &[
    "ls_native_arguments",
    "ls_native_object",
    "ls_native_array",
    "ls_native_visit",
    "ls_native_retain",
    "ls_native_release",
    "ls_native_collect_cycles",
    "ls_native_owned_objects",
    "ls_native_exception_pending",
    "ls_native_exception_raise",
    "ls_native_exception_take",
    "ls_string",
    "ls_string_hold",
    "ls_string_release",
    "ls_string_copy",
    "ls_string_take",
    "ls_string_clear",
    "ls_string_from_utf16",
    "ls_value",
    "ls_value_retain",
    "ls_value_release",
    "ls_value_trace",
    "LS_NULL",
    "LS_INT",
    "LS_FLOAT",
    "LS_BOOL",
    "LS_STRING",
    "LS_OBJECT",
    "LS_ARRAY",
    "LS_CALLABLE",
    "LS_SYMBOL",
    "LS_PRODUCT",
    "ls_native_record_new",
    "ls_native_record_get",
    "ls_native_record_set",
    "ls_native_array_new",
    "ls_native_array_length",
    "ls_native_array_get",
    "ls_native_array_set",
    "ls_native_task_new",
    "ls_native_task_resolve",
    "ls_native_task_reject",
    "ls_native_task_state",
    "ls_native_task_result",
    "ls_native_generator_next",
    "ls_native_generator_close",
    "ls_native_new_identity",
];

/// Compare the exact emitted aliases without allocating their spellings or a
/// second symbol table. Leading-zero argument ordinals are not generated names;
/// every non-void value has retain/release; only callables have a call API.
pub(crate) fn generated_host_symbol(
    provider: &str,
    signature: &crate::check::FunctionSignature<'_>,
    name: &str,
) -> bool {
    let Some(suffix) = name.strip_prefix(provider) else {
        return false;
    };
    let callable_wrapper = |suffix: &str, ty: &crate::check::Type<'_>| {
        (matches!(suffix, "_retain" | "_release") && !matches!(ty, crate::check::Type::Void))
            || (matches!(ty, crate::check::Type::Function(_)) && suffix == "_call")
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
