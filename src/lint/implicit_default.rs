//! `migration/implicit-default` (language.md R3 and §14, batch 1 of two): a
//! class field with no initializer that `init` does not assign on every path
//! takes its type's implicit default. The fix writes that default as the
//! field's initializer, which is what every construction evaluates today;
//! the refusal of implicit defaults follows once the ports carry the fix.
use crate::ast::{self, ClassMember, Item};
use crate::check::{CheckedView, Type};
use crate::module::ModuleId;
use crate::span::Span;

use super::{LintEdit, LintFix, PendingDiagnostic};

pub(super) const RULE: &str = "migration/implicit-default";

pub(super) fn lint(
    module: ModuleId,
    source: &str,
    syntax: &ast::Program<'_, '_>,
    view: &CheckedView<'_, '_, '_>,
    pending: &mut Vec<PendingDiagnostic>,
) {
    for item in syntax.items {
        let Item::Class(class) = item else {
            continue;
        };
        let Some(info) = view
            .type_binding(class.name.name)
            .and_then(|identity| view.nominal_class(identity))
        else {
            continue;
        };
        for member in class.members {
            let ClassMember::Field(field) = member else {
                continue;
            };
            if field.initializer.is_some()
                || !info.initialization.implicit.contains(&info.fields[field.name.name].member)
            {
                continue;
            }
            let Some(ty) = info.fields.get(field.name.name).map(|field| &field.ty) else {
                continue;
            };
            let spelled = &source[field.ty.span.start..field.ty.span.end];
            let fix = default_spelling(ty, spelled, view).map(|text| LintFix {
                applicability: "machine-applicable",
                edits: vec![LintEdit {
                    span: Span {
                        start: field.name.span.end,
                        end: field.name.span.end,
                    },
                    replacement: format!(" = {text}"),
                }],
            });
            let help = if fix.is_some() {
                "`lilscript-lint --fix` writes the default as the field's initializer"
            } else {
                "assign the field in `init` on every path, or make its type nullable"
            };
            pending.push(PendingDiagnostic {
                module,
                span: field.name.span,
                rule: RULE,
                message: format!(
                    "field `{}` takes its type's implicit default (R3)",
                    field.name.name
                ),
                evidence: None,
                help: Some(help.to_string()),
                fix,
            });
        }
    }
}

/// The implicit default a construction gives a field of `ty` (spelled
/// `spelled` in the source), as source, or `None` where it is not a value of
/// the type (a class, struct or function left null until `init`), which has
/// no spelling to write.
fn default_spelling(
    ty: &Type<'_>,
    spelled: &str,
    view: &CheckedView<'_, '_, '_>,
) -> Option<String> {
    Some(match ty {
        Type::Enum(declaration) => {
            let info = view.nominal_enum(declaration.identity)?;
            let (variant, _) = info.variants.iter().find(|(_, value)| **value == 0)?;
            if spelled.trim() != info.name {
                return None;
            }
            format!("{}.{variant}", info.name)
        }
        Type::Record(_) => "record {}".to_string(),
        Type::Int => "0".to_string(),
        Type::Float => "0.0".to_string(),
        Type::Bool => "false".to_string(),
        Type::String => "\"\"".to_string(),
        Type::Nullable(_) | Type::Null | Type::Dynamic | Type::Unknown => "null".to_string(),
        Type::Array(_) => "[]".to_string(),
        Type::Map(_, _) => "new Map()".to_string(),
        Type::Set(_) => "new Set()".to_string(),
        Type::ArrayBuffer => "new ArrayBuffer(0)".to_string(),
        Type::SharedArrayBuffer => "new SharedArrayBuffer(0)".to_string(),
        Type::Union(members) => {
            return members
                .first()
                .filter(|first| !matches!(first, Type::Enum(_)))
                .and_then(|first| default_spelling(first, "", view));
        }
        other => {
            let kind = crate::typed_array::TypedArrayKind::from_type(other)?;
            format!("new {}(0)", kind.name())
        }
    })
}
