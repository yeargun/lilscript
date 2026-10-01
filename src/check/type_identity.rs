//! Binder-aware structural equality. Generic binders compare by position;
//! free parameters compare by checked identity. No temporary type graph is made.
use super::{FunctionSignature, GenericFunctionType, Type, TypeParameter, TypeParameterId};

type Rename<'a> = &'a dyn Fn(TypeParameterId) -> TypeParameterId;

pub(super) fn generic_equal(a: &GenericFunctionType<'_>, b: &GenericFunctionType<'_>) -> bool {
    generic(a, b, &|id| id, true)
}
pub(super) fn bound_signature_equal(
    a: &FunctionSignature<'_>,
    a_binders: &[TypeParameter<'_>],
    b: &FunctionSignature<'_>,
    b_binders: &[TypeParameter<'_>],
) -> bool {
    if a_binders.len() != b_binders.len() { return false; }
    let rename = |id| b_binders.iter().position(|p| p.identity == id)
        .map_or(id, |index| a_binders[index].identity);
    signature(a, b, &rename, true)
}
/// Storage identity keeps declared binders: a body's free references must
/// continue to name its own signature's binders. Assignability may alpha-rename
/// complete generic signatures; interning must not silently rename body cells.
pub(crate) fn storage_equal(a: &Type<'_>, b: &Type<'_>) -> bool {
    ty(a, b, &|id| id, false)
}
fn generic(
    a: &GenericFunctionType<'_>,
    b: &GenericFunctionType<'_>,
    outer: Rename<'_>,
    alpha: bool,
) -> bool {
    if a.type_params.len() != b.type_params.len() {
        return false;
    }
    if !alpha {
        return a.type_params == b.type_params
            && signature(&a.signature, &b.signature, outer, false);
    }
    let rename = |id| {
        b.type_params
            .iter()
            .position(|p| p.identity == id)
            .map_or_else(|| outer(id), |index| a.type_params[index].identity)
    };
    signature(&a.signature, &b.signature, &rename, true)
}
fn signature(
    a: &FunctionSignature<'_>,
    b: &FunctionSignature<'_>,
    rename: Rename<'_>,
    alpha: bool,
) -> bool {
    a.params.len() == b.params.len()
        && a.params.iter().zip(&b.params).all(|(a, b)| {
            a.receiver == b.receiver
                && a.passing == b.passing
                && a.rest == b.rest
                && a.optional == b.optional
                && ty(&a.ty, &b.ty, rename, alpha)
        })
        && ty(&a.return_type, &b.return_type, rename, alpha)
}
fn list(a: &[Type<'_>], b: &[Type<'_>], rename: Rename<'_>, alpha: bool) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| ty(a, b, rename, alpha))
}
fn ty(a: &Type<'_>, b: &Type<'_>, rename: Rename<'_>, alpha: bool) -> bool {
    match (a, b) {
        (Type::TypeParameter(a), Type::TypeParameter(b)) => a.identity == rename(b.identity),
        (Type::Array(a), Type::Array(b))
        | (Type::Record(a), Type::Record(b))
        | (Type::Set(a), Type::Set(b))
        | (Type::Task(a), Type::Task(b))
        | (Type::Generator(a), Type::Generator(b)) => ty(a, b, rename, alpha),
        (Type::Nullable(a), Type::Nullable(b)) => a.boundary == b.boundary && ty(a, b, rename, alpha),
        (Type::Map(a, b), Type::Map(c, d)) => ty(a, c, rename, alpha) && ty(b, d, rename, alpha),
        (Type::Union(a), Type::Union(b)) | (Type::Intersection(a), Type::Intersection(b)) => list(a, b, rename, alpha),
        (
            Type::ClassInstance {
                declaration: a,
                args: x,
            },
            Type::ClassInstance {
                declaration: b,
                args: y,
            },
        )
        | (
            Type::StructInstance {
                declaration: a,
                args: x,
            },
            Type::StructInstance {
                declaration: b,
                args: y,
            },
        ) => a == b && list(x, y, rename, alpha),
        (Type::Function(a), Type::Function(b)) => signature(a, b, rename, alpha),
        (Type::GenericFunction(a), Type::GenericFunction(b)) => generic(a, b, rename, alpha),
        // No child types or binders remain in matching leaf variants.
        _ => std::mem::discriminant(a) == std::mem::discriminant(b) && a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::SourceNodeId;
    use crate::check::{FunctionParameter, FunctionType, TypeParameter};
    fn parameter(index: u32, name: &str) -> TypeParameter<'_> {
        TypeParameter {
            identity: TypeParameterId {
                module: 0,
                declaration: SourceNodeId::detached(index),
            },
            name,
        }
    }
    fn generic<'src>(binder: TypeParameter<'src>, value: Type<'src>) -> Type<'src> {
        Type::GenericFunction(GenericFunctionType {
            type_params: vec![binder],
            signature: FunctionType::new(FunctionSignature {
                params: vec![FunctionParameter::value(Type::TypeParameter(binder))],
                return_type: Box::new(value),
            }),
        })
    }
    #[test]
    fn s4_generic_binders_compare_by_position_and_free_parameters_by_identity() {
        let a = parameter(1, "T");
        let b = parameter(2, "U");
        assert_eq!(
            generic(a, Type::TypeParameter(a)),
            generic(b, Type::TypeParameter(b))
        );
        let free = parameter(3, "T");
        let other = parameter(4, "T");
        assert_ne!(Type::TypeParameter(free), Type::TypeParameter(other));
        assert_ne!(
            generic(a, Type::TypeParameter(free)),
            generic(b, Type::TypeParameter(other))
        );
        let mut renamed = free;
        renamed.name = "AnotherSpelling";
        assert_eq!(Type::TypeParameter(free), Type::TypeParameter(renamed));
    }
    #[test]
    fn s4_checker_keeps_distinct_generic_declarations_and_alias_calls() {
        let arena = bumpalo::Bump::new();
        let syntax=crate::parse_source(&arena,"T first<T>(T value){return value;}U second<U>(U value){return value;}auto alias=first;alias=second;print(alias(7));").unwrap();
        let checked = crate::analyze(&syntax).unwrap();
        let mut binders = checked
            .symbols()
            .iter()
            .filter_map(|symbol| match &symbol.ty {
                Type::GenericFunction(f) => Some(f.type_params[0].identity),
                _ => None,
            });
        let a = binders.next().unwrap();
        let b = binders.next().unwrap();
        assert_ne!(a, b);
        assert_eq!(
            crate::interpreter::interpret_program(&syntax, &checked).unwrap(),
            "7\n"
        );
        crate::program::from_checked_source(&syntax, &checked)
            .unwrap()
            .verify()
            .unwrap();
    }
    #[test]
    fn s4_statement_origins_are_distinct_and_survive_lowering() {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(
            &arena,
            "int choose(int x){if(x>0){return 1;}return 2;}print(choose(0));",
        )
        .unwrap();
        let crate::ast::Item::Function(function) = &syntax.items[0] else {
            panic!()
        };
        let crate::ast::Stmt::If {
            condition,
            then_branch,
            ..
        } = &function.body[0]
        else {
            panic!()
        };
        let crate::ast::Stmt::Block { body, .. } = then_branch else {
            panic!()
        };
        let ids = [
            function.body[0].id(),
            then_branch.id(),
            body[0].id(),
            function.body[1].id(),
        ];
        for (index, id) in ids.iter().enumerate() {
            assert!(id.index() < syntax.source_identity().len());
            assert_ne!(*id, condition.id);
            assert!(!ids[..index].contains(id));
        }
        let checked = crate::analyze(&syntax).unwrap();
        let program = crate::program::from_checked_source(&syntax, &checked).unwrap();
        program.verify().unwrap();
        let origins = program
            .units()
            .iter()
            .flat_map(|unit| unit.data().operations.iter())
            .filter(|op| {
                matches!(
                    op.kind,
                    crate::program::OperationKind::If { .. }
                        | crate::program::OperationKind::Return
                )
            })
            .filter_map(|op| op.origin)
            .collect::<Vec<_>>();
        for id in [ids[0], ids[2], ids[3]] {
            assert!(origins.contains(&id), "missing {id:?}: {origins:?}");
        }
    }
}
