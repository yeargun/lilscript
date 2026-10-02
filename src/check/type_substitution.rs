//! Forward substitution from the checker's retained type arguments.
//!
//! The plain checker and admitted verifier share this implementation. It never
//! inverse-matches an effective signature, so normalized unions do not discard
//! or require re-inferring the checker's original substitution decision.
use super::binary_types::{normalize_union_with, TypeConstructionAdmission};
use super::type_relation::{RelationEvent, Unmetered};
use super::{FunctionParameter, FunctionSignature, FunctionType, GenericFunctionType, Type};

/// Callable/metadata constructors used in addition to the common type builders.
/// Every returned allocation stays charged through the surrounding query scope.
pub(crate) trait SubstitutionAdmission: TypeConstructionAdmission {
    fn parameters<'src>(
        &mut self,
        count: usize,
    ) -> Result<Vec<FunctionParameter<'src>>, Self::Error>;
    fn signature<'src>(
        &mut self,
        value: FunctionSignature<'src>,
    ) -> Result<FunctionType<'src>, Self::Error>;
    fn parameter_names<'src>(
        &mut self,
        names: &[crate::check::TypeParameter<'src>],
    ) -> Result<Vec<crate::check::TypeParameter<'src>>, Self::Error>;
}

impl SubstitutionAdmission for Unmetered {
    fn parameters<'src>(
        &mut self,
        count: usize,
    ) -> Result<Vec<FunctionParameter<'src>>, Self::Error> {
        Ok(Vec::with_capacity(count))
    }
    fn signature<'src>(
        &mut self,
        value: FunctionSignature<'src>,
    ) -> Result<FunctionType<'src>, Self::Error> {
        Ok(FunctionType::new(value))
    }
    fn parameter_names<'src>(
        &mut self,
        names: &[crate::check::TypeParameter<'src>],
    ) -> Result<Vec<crate::check::TypeParameter<'src>>, Self::Error> {
        Ok(names.to_vec())
    }
}

/// `lookup` reads the already resolved substitution map/ordered arguments. Its
/// actual lookup work is admitted by that owner before comparison or hashing.
/// Replacement values are copied once at the occurrence, never recursively
/// substituted again; this preserves an enclosing generic binder's identity.
pub(crate) fn substitute_type_with<'types, 'src: 'types, A: SubstitutionAdmission>(
    ty: &Type<'src>,
    lookup: &mut impl FnMut(
        crate::check::TypeParameterId,
        &mut A,
    ) -> Result<Option<&'types Type<'src>>, A::Error>,
    admission: &mut A,
) -> Result<Type<'src>, A::Error> {
    // Prepay this visit and eventual temporary-node destruction.
    admission.admit(RelationEvent::TypeWork(2))?;
    match ty {
        Type::TypeParameter(name) => match lookup(name.identity, admission)? {
            Some(replacement) => admission.clone_type(replacement),
            None => admission.clone_type(ty),
        },
        Type::Array(inner) => {
            let inner = substitute_type_with(inner, lookup, admission)?;
            Ok(Type::Array(admission.box_type(inner)?))
        }
        Type::Record(inner) => {
            let inner = substitute_type_with(inner, lookup, admission)?;
            Ok(Type::Record(admission.box_type(inner)?))
        }
        Type::Map(key, value) => {
            let key = substitute_type_with(key, lookup, admission)?;
            let value = substitute_type_with(value, lookup, admission)?;
            let key = admission.box_type(key)?;
            Ok(Type::Map(key, admission.box_type(value)?))
        }
        Type::Set(inner) => {
            let inner = substitute_type_with(inner, lookup, admission)?;
            Ok(Type::Set(admission.box_type(inner)?))
        }
        Type::Task(inner) => {
            let inner = substitute_type_with(inner, lookup, admission)?;
            super::task_types::task_with(inner,admission)
        }
        Type::Generator(inner) => {
            let inner = substitute_type_with(inner, lookup, admission)?;
            Ok(Type::Generator(admission.box_type(inner)?))
        }
        // `T?` with `T := U?` is `U?` (R2: `T??` is `T?`): an absent value
        // has one kind, whatever made it optional.
        Type::Nullable(inner) => {
            let boundary = inner.boundary;
            let inner = substitute_type_with(inner, lookup, admission)?;
            Ok(Type::pinned_nullable(admission.box_type(inner)?, boundary))
        }
        Type::Intersection(members) => Ok(Type::Intersection(substitute_members(members, lookup, admission)?)),
        Type::Union(members) => {
            let members = substitute_members(members, lookup, admission)?;
            normalize_union_with(members, admission)
        }
        Type::StructInstance { declaration, args } => Ok(Type::StructInstance {
            declaration: *declaration,
            args: substitute_members(args, lookup, admission)?,
        }),
        Type::ClassInstance { declaration, args } => Ok(Type::ClassInstance {
            declaration: *declaration,
            args: substitute_members(args, lookup, admission)?,
        }),
        Type::Function(signature) => Ok(Type::Function(substitute_signature_with(
            signature, lookup, admission,
        )?)),
        Type::GenericFunction(function) => {
            let names = admission.parameter_names(&function.type_params)?;
            let signature = substitute_signature_with(&function.signature, lookup, admission)?;
            Ok(Type::GenericFunction(GenericFunctionType {
                type_params: names,
                signature,
            }))
        }
        _ => admission.clone_type(ty),
    }
}

pub(crate) fn substitute_signature_with<'types, 'src: 'types, A: SubstitutionAdmission>(
    signature: &FunctionSignature<'src>,
    lookup: &mut impl FnMut(
        crate::check::TypeParameterId,
        &mut A,
    ) -> Result<Option<&'types Type<'src>>, A::Error>,
    admission: &mut A,
) -> Result<FunctionType<'src>, A::Error> {
    admission.admit(RelationEvent::TypeWork(1))?;
    admission.admit(RelationEvent::TypeWork(signature.params.len()))?;
    let mut params = admission.parameters(signature.params.len())?;
    for parameter in &signature.params {
        admission.admit(RelationEvent::ParameterPair)?;
        let ty = substitute_type_with(&parameter.ty, lookup, admission)?;
        // Exact parameter capacity was admitted by parameters().
        params.push(FunctionParameter {
            receiver: parameter.receiver,
            ty,
            passing: parameter.passing,
            optional: parameter.optional,
            rest: parameter.rest,
        });
    }
    let result = substitute_type_with(&signature.return_type, lookup, admission)?;
    let return_type = admission.box_type(result)?;
    admission.signature(FunctionSignature {
        params,
        return_type,
    })
}

fn substitute_members<'types, 'src: 'types, A: SubstitutionAdmission>(
    members: &[Type<'src>],
    lookup: &mut impl FnMut(
        crate::check::TypeParameterId,
        &mut A,
    ) -> Result<Option<&'types Type<'src>>, A::Error>,
    admission: &mut A,
) -> Result<Vec<Type<'src>>, A::Error> {
    let mut result = Vec::new();
    for member in members {
        let value = substitute_type_with(member, lookup, admission)?;
        admission.push_type(&mut result, value)?;
    }
    Ok(result)
}

#[cfg(test)]
#[path = "type_substitution_tests.rs"]
mod tests;
