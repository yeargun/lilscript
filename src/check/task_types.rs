//! A task contains a settled value: resolution adopts another task instead of
//! storing its handle. Canonicalization also runs after generic substitution.
use super::binary_types::{normalize_union_with, TypeConstructionAdmission};
use super::type_relation::{RelationEvent, Unmetered};
use super::Type;

pub(crate) fn settled_with<'src, A: TypeConstructionAdmission>(
    mut value: Type<'src>,
    admission: &mut A,
) -> Result<Type<'src>, A::Error> {
    loop {
        admission.admit(RelationEvent::TypeWork(1))?;
        match value {
            Type::Task(inner) => value = *inner,
            Type::Union(members) => {
                let mut out = Vec::new();
                for member in members {
                    let settled = settled_with(member, admission)?;
                    admission.push_type(&mut out, settled)?;
                }
                return normalize_union_with(out, admission);
            }
            Type::Nullable(inner) => {
                let boundary = inner.boundary;
                let value = settled_with(inner.into_inner(), admission)?;
                return Ok(Type::pinned_nullable(admission.box_type(value)?, boundary));
            }
            other => return Ok(other),
        }
    }
}
pub(crate) fn task_with<'src, A: TypeConstructionAdmission>(
    value: Type<'src>,
    admission: &mut A,
) -> Result<Type<'src>, A::Error> {
    let value = settled_with(value, admission)?;
    Ok(Type::Task(admission.box_type(value)?))
}
pub(super) fn task(value: Type<'_>) -> Type<'_> {
    task_with(value, &mut Unmetered).unwrap_or_else(|never| match never {})
}
