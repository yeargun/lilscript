//! Direct by-value schema cycles have no finite value representation. Walk
//! canonical field edges without recursion or a second retained schema graph.
//! A nullable/reference-container field ends this direct edge. Cycles exposed
//! only by generic substitution belong to the instantiated-schema proof.

use super::{CheckError, StructInfo, Type};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Visit {
    Unseen,
    Active,
    Complete,
}

struct Frame {
    declaration: usize,
    next_field: usize,
}

pub(super) fn validate(
    structs: &[StructInfo<'_>],
) -> Result<(), (Option<crate::module::ModuleId>, CheckError)> {
    let mut visits = vec![Visit::Unseen; structs.len()];
    let mut path = Vec::new();
    for root in 0..structs.len() {
        if visits[root] != Visit::Unseen {
            continue;
        }
        visits[root] = Visit::Active;
        path.push(Frame {
            declaration: root,
            next_field: 0,
        });
        while let Some(frame) = path.last_mut() {
            let owner = &structs[frame.declaration];
            let Some((_, field)) = owner.fields.get_index(frame.next_field) else {
                visits[frame.declaration] = Visit::Complete;
                path.pop();
                continue;
            };
            frame.next_field += 1;
            let declaration = match &field.ty {
                Type::Struct(declaration) | Type::StructInstance { declaration, .. } => declaration,
                _ => continue,
            };
            let next = declaration.identity.index();
            if declaration.identity.is_class()
                || structs
                    .get(next)
                    .is_none_or(|schema| schema.declaration.identity != declaration.identity)
            {
                return Err((
                    owner.module,
                    CheckError::new(field.span, "invalid canonical struct field declaration"),
                ));
            }
            match visits[next] {
                Visit::Active => {
                    return Err((
                        owner.module,
                        CheckError::new(
                            field.span,
                            format!(
                                "struct value field `{}.{}` creates a recursive value layout through `{}`",
                                owner.declaration.name, field.name, declaration.name
                            ),
                        ),
                    ));
                }
                Visit::Complete => {}
                Visit::Unseen => {
                    visits[next] = Visit::Active;
                    path.push(Frame {
                        declaration: next,
                        next_field: 0,
                    });
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn nominal_direct_value_cycles_report_the_original_field() {
        for (source, owner) in [
            ("struct P{P next;}", "P.next"),
            ("struct A{B next;}struct B{A back;}", "B.back"),
            ("struct Ring<T>{Ring<T> next;}", "Ring.next"),
        ] {
            let arena = bumpalo::Bump::new();
            let source_ast = crate::parse_source(&arena, source).unwrap();
            let error = crate::analyze(&source_ast).unwrap_err();
            assert!(error.message.contains("recursive value layout"), "{error}");
            assert!(error.message.contains(owner), "{error}");
            assert!(
                source[error.span.start..error.span.end]
                    .contains(owner.split('.').next_back().unwrap()),
                "cycle error must retain the closing field's source span: {error}"
            );
        }
    }

    #[test]
    fn nominal_nullable_and_reference_leaves_allow_finite_recursive_values() {
        for source in [
            "struct Tree{Tree? left;Tree? right;}Tree root=Tree{null,null};",
            "struct Tree{Tree[] children;}Tree root=Tree{[]};",
            "struct Tree{Record<Tree> children;}",
            "class Owner{Node? root;}struct Node{Owner owner;}",
            "struct Box<T>{T value;}struct Nested{Box<int> item;}Box<int> item=Box{1};Nested value=Nested{item};",
        ] {
            let arena = bumpalo::Bump::new();
            let source_ast = crate::parse_source(&arena, source).unwrap();
            crate::analyze(&source_ast).unwrap_or_else(|error| panic!("{source}: {error}"));
        }
    }

    #[test]
    fn nominal_deep_schema_chains_do_not_use_the_host_call_stack() {
        use std::fmt::Write;
        let mut source = String::new();
        for index in 0..2048 {
            let child = if index == 2047 {
                "Leaf".to_owned()
            } else {
                format!("Node{}", index + 1)
            };
            write!(source, "struct Node{index}{{{child} value;}}").unwrap();
        }
        source.push_str("struct Leaf{int value;}");
        let arena = bumpalo::Bump::new();
        let source_ast = crate::parse_source(&arena, &source).unwrap();
        crate::analyze(&source_ast).unwrap();
    }
}
