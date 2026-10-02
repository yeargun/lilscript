//! Explicit bundled providers. Selecting a link name is the opt-in; spelling
//! an extern similarly never grants ambient access. The source signature is
//! checked before any native text is formed.
use crate::check::{FunctionSignature, Type};

pub(crate) const NAMES: &[&str] = &[
    "host_lil_arg_count",
    "host_lil_arg",
    "host_lil_env",
    "host_lil_stdout",
    "host_lil_stderr",
    "host_lil_read_text",
    "host_lil_write_text",
    "host_lil_wall_time",
    "host_lil_cpu_time",
    "host_lil_exit",
    "host_lil_stdin",
];
pub(crate) fn bundled(name: &str) -> bool {
    NAMES.contains(&name)
}
pub(crate) fn arguments(name: &str) -> bool {
    matches!(name, "host_lil_arg_count" | "host_lil_arg")
}
pub(crate) fn signature(name: &str, signature: &FunctionSignature<'_>) -> bool {
    fn nullable_string(ty: &Type<'_>) -> bool {
        matches!(ty,Type::Nullable(t) if matches!(t.as_ref(),Type::String))
    }
    let p = &signature.params;
    let result = signature.return_type.as_ref();
    match name {
        "host_lil_arg_count" => p.is_empty() && matches!(result, Type::Int),
        "host_lil_arg" => p.len() == 1 && matches!(p[0].ty, Type::Int) && nullable_string(result),
        "host_lil_env" => {
            p.len() == 1 && matches!(p[0].ty, Type::String) && nullable_string(result)
        }
        "host_lil_stdout" | "host_lil_stderr" => {
            p.len() == 1 && matches!(p[0].ty, Type::String) && matches!(result, Type::Void)
        }
        "host_lil_read_text" => {
            p.len() == 1 && matches!(p[0].ty, Type::String) && matches!(result, Type::String)
        }
        "host_lil_write_text" => {
            p.len() == 2
                && p.iter().all(|p| matches!(p.ty, Type::String))
                && matches!(result, Type::Void)
        }
        "host_lil_wall_time" | "host_lil_cpu_time" => p.is_empty() && matches!(result, Type::Float),
        "host_lil_exit" => {
            p.len() == 1 && matches!(p[0].ty, Type::Int) && matches!(result, Type::Void)
        }
        "host_lil_stdin" => p.is_empty() && matches!(result, Type::String),
        _ => !name.starts_with("host_lil_"),
    }
}
