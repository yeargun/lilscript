//! A borrowed structural question about nominal transport, not runtime presence.
//! A false answer supplies no primitive-domain or callable-sealing authority.
//! The caller owns/reuses the admitted branch buffer; no type graph is retained.
use super::super::raw_domains::Admission;
use super::super::Type;

pub(in crate::program) fn contains_nominal_product<'types, 'src, A: Admission>(
    ty: &'types Type<'src>,
    pending: &mut Vec<&'types Type<'src>>,
    admission: &mut A,
) -> Result<bool, A::Error> {
    debug_assert!(pending.is_empty());
    let mut next = Some(ty);
    while let Some(ty) = next.take().or_else(|| pending.pop()) {
        admission.work(1)?;
        match ty {
            Type::Struct(_) | Type::StructInstance { .. } => {
                admission.work(pending.len())?;
                pending.clear();
                return Ok(true);
            }
            Type::Nullable(inner)
            | Type::Array(inner)
            | Type::Record(inner)
            | Type::Set(inner)
            | Type::Task(inner)
            | Type::Generator(inner) => next = Some(inner),
            Type::Map(key, value) => {
                next = Some(key);
                admission.push(pending, value)?;
            }
            Type::Union(members) => {
                for member in members {
                    admission.push(pending, member)?;
                }
            }
            Type::Function(function) => {
                next = Some(&function.return_type);
                for parameter in &function.params {
                    admission.push(pending, &parameter.ty)?;
                }
            }
            Type::GenericFunction(function) => {
                next = Some(&function.signature.return_type);
                for parameter in &function.signature.params {
                    admission.push(pending, &parameter.ty)?;
                }
            }
            _ => {}
        }
    }
    Ok(false)
}
