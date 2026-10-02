//! Closed monomorphic generic calls can use concrete physical storage without
//! cloning the semantic body. The shared call graph supplies completeness;
//! checker-owned instantiations supply types. Unknown, public, recursive or
//! polymorphic uses keep the tagged ABI. No names or sampled runtime types
//! participate in this decision.
use super::*;
use crate::check::TypeParameterId;

#[derive(Default)]
pub(super) struct Bindings {
    pub(super) types: Vec<(TypeParameterId, Option<TypeId>)>,
    pub(super) callbacks: Vec<(CellId, UnitId)>,
}
pub(super) fn bindings(
    program: &Program<'_>,
    uses: &UseIndex,
    budget: &mut AllocationBudget<'_>,
) -> Result<Bindings, NativeError> {
    budget.with_temporary(
        |budget| {
            Ok::<_, NativeError>(super::super::call_graph::CallGraph::build_in(
                program,
                super::super::call_graph::Seal::Module,
                budget,
            )?)
        },
        |graph, budget| {
            let mut result = Bindings::default();
            // Nominal layout parameters are shared by every instance and by
            // field initialization before its constructor runs. A constructor's
            // closed callers do not prove that this layout can change. Keep
            // these binders erased; only independent function binders qualify.
            for parameters in program
                .classes
                .iter()
                .map(|class| class.type_params.as_slice())
                .chain(
                    program
                        .structs
                        .iter()
                        .map(|product| product.type_parameters.as_slice()),
                )
            {
                for &parameter in parameters {
                    work(budget, 1)?;
                    budget.push(Scratch, &mut result.types, (parameter, None))?;
                }
            }
            for unit in &program.units {
                work(budget, 1)?;
                let Some(ty) = unit.data().callable_type else {
                    continue;
                };
                let Type::GenericFunction(generic) = &program.types[ty.index()] else {
                    continue;
                };
                let mut chosen: Option<&[TypeId]> = None;
                let mut valid = unit.data().suspension == Suspension::None;
                // Generic callable types may have been interned using alpha
                // equality. Do not bind one declaration's parameter identities
                // to a different body's independently named type variables.
                valid &= unit.data().parameters.len() == generic.signature.params.len();
                for (&cell, parameter) in
                    unit.data().parameters.iter().zip(&generic.signature.params)
                {
                    let mut query = budget.scope();
                    valid &= crate::check::type_relation::type_equal_with(
                        &program.types[program.cells[cell.index()].ty.index()],
                        &parameter.ty,
                        &mut crate::check::type_admission::TypeQueryAdmission::new(&mut query),
                    )?;
                }
                if let Some(callers) = graph.complete_callers(unit.id()) {
                    for edge in callers {
                        work(budget, 1)?;
                        let caller = program.unit(edge.caller).unwrap();
                        let call = &caller.calls[edge.call.index()];
                        let Some(instance) = call
                            .contract
                            .instantiation
                            .map(|id| &caller.call_instantiations[id.index()])
                        else {
                            valid = false;
                            break;
                        };
                        if instance.declaration != ty
                            || instance.arguments.len() != generic.type_params.len()
                        {
                            valid = false;
                            break;
                        }
                        if let Some(prior) = chosen {
                            work(budget, prior.len())?;
                            if prior != instance.arguments {
                                valid = false;
                                break;
                            }
                        } else {
                            chosen = Some(&instance.arguments);
                        }
                    }
                } else {
                    valid = false;
                }
                for (index, parameter) in generic.type_params.iter().enumerate() {
                    let actual = chosen
                        .and_then(|args| args.get(index))
                        .copied()
                        .filter(|&id| {
                            // Concrete scalar/handle types need no extra layout
                            // substitution or recursive specialization frontier.
                            valid
                                && matches!(
                                    program.types[id.index()],
                                    Type::Int
                                        | Type::Float
                                        | Type::Bool
                                        | Type::String
                                        | Type::Symbol
                                        | Type::Regex
                                        | Type::Enum(_)
                                )
                        });
                    work(budget, result.types.len() + 1)?;
                    if let Some((_, prior)) = result
                        .types
                        .iter_mut()
                        .find(|(id, _)| *id == parameter.identity)
                    {
                        if *prior != actual {
                            *prior = None;
                        }
                    } else {
                        budget.push(Scratch, &mut result.types, (parameter.identity, actual))?;
                    }
                }
                // A closed higher-order call whose immutable parameter always
                // receives the same static function can call that body directly.
                // Keep the actual argument and its evaluation/identity intact.
                let Some(callers) = graph.complete_callers(unit.id()).filter(|c| !c.is_empty())
                else {
                    continue;
                };
                for (position, &cell) in unit.data().parameters.iter().enumerate() {
                    if !matches!(
                        program.types[program.cells[cell.index()].ty.index()],
                        Type::Function(_)
                    ) || generic.signature.params.get(position).is_none_or(|p| {
                        p.optional
                            || p.rest
                            || p.passing != crate::primitive::ParameterPassing::Value
                    }) {
                        continue;
                    }
                    let mut immutable = true;
                    for usage in uses.cell(cell).unwrap().sites() {
                        work(budget, 1)?;
                        if matches!(
                            usage,
                            CellUseSite::Unit {
                                usage: CellUse::Write { .. } | CellUse::Reference { .. },
                                ..
                            }
                        ) {
                            immutable = false;
                            break;
                        }
                    }
                    if !immutable {
                        continue;
                    }
                    let mut known = None;
                    for edge in callers {
                        let data = program.unit(edge.caller).unwrap();
                        let call = &data.calls[edge.call.index()];
                        let Some(&CallArgument::Value(value)) =
                            data.arguments(call.arguments).unwrap().get(position)
                        else {
                            known = None;
                            break;
                        };
                        work(budget, data.values.len() + 1)?;
                        let super::super::call_graph::Callee::Unit(target) =
                            graph.callee_of_value(program, data, value)
                        else {
                            known = None;
                            break;
                        };
                        let body = program.unit(target).unwrap();
                        work(budget, body.captures.len() + 1)?;
                        if body.kind != UnitKind::Function
                            || body.captures.iter().any(|c| {
                                program.unit(program.cells[c.index()].owner).unwrap().kind
                                    != UnitKind::ModuleInitialization
                            })
                            || known.is_some_and(|prior| prior != target)
                        {
                            known = None;
                            break;
                        }
                        known = Some(target);
                    }
                    if let Some(target) = known {
                        budget.push(Scratch, &mut result.callbacks, (cell, target))?;
                    }
                }
            }
            Ok(result)
        },
    )
}

pub(super) fn callback(
    data: &UnitData,
    target: PreparedTarget,
    known: &[(CellId, UnitId)],
    budget: &mut AllocationBudget<'_>,
) -> Result<Option<UnitId>, NativeError> {
    let mut value = match target {
        PreparedTarget::Callable { callee, .. } => Some(callee),
        _ => None,
    };
    let mut place = match target {
        PreparedTarget::Placed { place, .. } => Some(place),
        _ => None,
    };
    for _ in 0..64 {
        work(budget, 1)?;
        if let Some(at) = place {
            let Place::Cell(cell) = data.places[at.index()] else {
                return Ok(None);
            };
            work(budget, known.len())?;
            return Ok(known
                .iter()
                .find_map(|&(id, body)| (id == cell).then_some(body)));
        }
        let Some(current) = value else {
            return Ok(None);
        };
        let op = &data.operations[data.values[current.index()].definition.index()];
        match op.kind {
            OperationKind::CopyValue => {
                value = data.operands(op.operands).unwrap().first().copied()
            }
            OperationKind::Load(at) => place = Some(at),
            _ => return Ok(None),
        }
    }
    Ok(None)
}
