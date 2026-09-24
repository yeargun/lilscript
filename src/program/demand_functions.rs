//! One transport cursor over the original tagged argument arena. Shared calls
//! use fixed full-width interfaces; inline entries consume the same frozen
//! values directly into their independently selected logical storage.
use super::*;
use crate::compilation_contract::JavaScriptExecution;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::program) enum SharedTransportSupport {
    MayApply,
    AllCreatorsInline,
}

/// The caller pins one already validated semantic snapshot and its families.
/// A body-wide sealed creator set plus exact selected producer replacements
/// proves shared transport unused, independently of current reachability.
/// Compatible map unions retain these helpers, so this support cannot return
/// in a later additive child. The map and its full identity remain unchanged.
pub(in crate::program) fn shared_transport_support<E>(
    map: &ImplementationMap,
    layout: &FunctionLayout,
    execution: JavaScriptExecution,
    mut work: impl FnMut(usize) -> Result<(), E>,
) -> Result<SharedTransportSupport, E> {
    use crate::program::callable_inputs::InputScope;
    work(1)?;
    let inputs = layout.inputs();
    if execution != JavaScriptExecution::Module
        || !inputs.runtime_inputs_sealed()
        || inputs.scope() != InputScope::Body(layout.body())
        || inputs.producers().is_empty()
    {
        return Ok(SharedTransportSupport::MayApply);
    }
    let lookup = 1 + (usize::BITS - map.helpers().len().leading_zeros()) as usize;
    for producer in inputs.producers() {
        work(lookup)?;
        let Some(helper) = map.helper_for_cell(producer.cell) else {
            return Ok(SharedTransportSupport::MayApply);
        };
        if helper.root().body != layout.body()
            || helper.closure() != producer.creation
            || helper.initialize() != producer.initialize
        {
            return Ok(SharedTransportSupport::MayApply);
        }
        // Helper discovery retains all calls from the complete proof for
        // this exact producer. Matching the sealed producer entails their
        // replacement; no second call-membership scan is necessary.
    }
    Ok(SharedTransportSupport::AllCreatorsInline)
}

/// A borrowed record from the sole call-argument arena. The caller/call/ordinal
/// identify the original reference preparation for target path composition.
#[derive(Debug, Clone, Copy)]
pub(in crate::program) struct InlineActual<'program> {
    pub caller: ContextId,
    pub call: CallId,
    pub position: u32,
    pub argument: &'program CallArgument,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct FunctionCall {
    layout: usize,
    empty: bool,
}
impl<'program, 'src> DemandPlan<'program, 'src> {
    /// The occurrence visitor admits this lookup before exposing an input to
    /// placement. ProductArgument already proves a projection without a lookup.
    pub(super) fn effective_input_work(&self, input: EffectiveValueUse) -> usize {
        if matches!(input.role, EffectiveUseRole::ProductArgument { .. }) {
            0
        } else {
            (usize::BITS - self.product_value_index.len().leading_zeros()) as usize
        }
    }
    /// Additional target ancestry on the original occurrence: a raw field
    /// projection, or packing one existing immutable component snapshot.
    pub(in crate::program) fn effective_input_depth(
        &self,
        context: ContextId,
        input: EffectiveValueUse,
    ) -> usize {
        usize::from(
            matches!(input.role, EffectiveUseRole::ProductArgument { .. })
                || self
                    .product_for_value(self.context(context).unit, input.value)
                    .is_some(),
        )
    }
    pub(in crate::program) fn function_lookup_work(&self) -> usize {
        [
            self.functions.len(),
            self.function_calls.len(),
            self.max_function_parameters,
        ]
        .into_iter()
        .map(|len| (usize::BITS - len.leading_zeros()) as usize)
        .sum()
    }
    pub(in crate::program) fn function_layout(
        &self,
        body: UnitId,
    ) -> Option<&'program FunctionLayout> {
        self.functions
            .binary_search_by_key(&body, |layout| layout.body())
            .ok()
            .map(|index| self.functions[index])
    }
    fn call_layout(&self, context: ContextId, call: CallId) -> Option<FunctionCall> {
        let unit = self.context(context).unit;
        self.function_calls
            .binary_search_by_key(&(unit, call), |entry| entry.0)
            .ok()
            .map(|index| self.function_calls[index].1)
    }
    pub(in crate::program) fn call_product_parameter(
        &self,
        context: ContextId,
        call: CallId,
        position: u32,
    ) -> Option<&'program ParameterLayout> {
        let layout = self.call_layout(context, call)?.layout;
        self.functions[layout].parameter(position)
    }
    pub(in crate::program) fn call_transport(
        &self,
        context: ContextId,
        call: CallId,
        position: u32,
    ) -> ProductTransport {
        self.call_layout(context, call)
            .map_or(ProductTransport::Packed, |entry| {
                self.functions[entry.layout].transport(position)
            })
    }
    pub(in crate::program) fn call_has_empty_transport(
        &self,
        context: ContextId,
        call: CallId,
    ) -> bool {
        self.call_layout(context, call)
            .is_some_and(|entry| entry.empty)
    }
    /// Constant-time access; cursor/target callers admit the lookup itself.
    pub(in crate::program) fn inline_actual(
        &self,
        context: ContextId,
        position: u32,
    ) -> InlineActual<'program> {
        let ContextKind::Inline { call, .. } = self.context(context).kind else {
            unreachable!("inline actual source")
        };
        let caller = self.context(context).parent.unwrap();
        let data = self.program.units[self.context(caller).unit.index()].data();
        let OperationKind::Call(target) = data.operations[call.index()].kind else {
            unreachable!("inline call")
        };
        InlineActual {
            caller,
            call: target,
            position,
            argument: &data
                .arguments(data.calls[target.index()].arguments)
                .unwrap()[position as usize],
        }
    }
    pub(in crate::program) fn inline_parameter_source(
        &self,
        context: ContextId,
        position: u32,
    ) -> (ContextId, ValueId) {
        let actual = self.inline_actual(context, position);
        let CallArgument::Value(value) = *actual.argument else {
            unreachable!("inline helper value parameter")
        };
        (actual.caller, value)
    }
    pub(super) fn index_functions(
        &mut self,
        map: &'program ImplementationMap,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        for layout in map.functions() {
            budget.work(1)?;
            if shared_transport_support(map, layout, self.contract.execution, |n| budget.work(n))?
                == SharedTransportSupport::AllCreatorsInline
            {
                continue;
            }
            let index = self.functions.len();
            self.max_function_parameters =
                self.max_function_parameters.max(layout.parameters().len());
            budget.push(&mut self.functions, layout)?;
            for call in layout.inputs().calls() {
                let data = self.program.units[call.caller.index()].data();
                let arguments = data
                    .arguments(data.calls[call.target.index()].arguments)
                    .unwrap();
                let mut empty = true;
                for (position, argument) in arguments.iter().enumerate() {
                    budget.work(
                        1 + (usize::BITS - layout.parameters().len().leading_zeros()) as usize,
                    )?;
                    empty &= match argument {
                        CallArgument::Reference(_) => false,
                        CallArgument::Value(_) => {
                            layout.parameter(position as u32).is_some_and(|parameter| {
                                self.program.structs[parameter.schema.index()]
                                    .fields
                                    .is_empty()
                            })
                        }
                    };
                }
                budget.push(
                    &mut self.function_calls,
                    (
                        (call.caller, call.target),
                        FunctionCall {
                            layout: index,
                            empty,
                        },
                    ),
                )?;
            }
        }
        budget.work(sort_work(self.function_calls.len())?)?;
        self.function_calls.sort_unstable_by_key(|entry| entry.0);
        budget.work(self.function_calls.len())?;
        if self
            .function_calls
            .windows(2)
            .any(|pair| pair[0].0 == pair[1].0)
        {
            return Err(unsupported("overlapping function transport calls"));
        }
        Ok(())
    }
    /// A scalar product snapshot is already frozen; a packed source value is
    /// one real target use per raw projection, so placement sees multiplicity.
    fn component_input(
        &self,
        context: ContextId,
        value: ValueId,
        call: CallId,
        position: u32,
        slot: u32,
    ) -> InputStep {
        if self
            .product_for_value(self.context(context).unit, value)
            .is_some()
        {
            InputStep::Dependency(InputDependency::ProductSnapshot(value, slot))
        } else {
            InputStep::Dependency(InputDependency::Value(EffectiveValueUse {
                observation: ObservationDemand::Exact,
                value,
                site: EffectiveUseSite::Operation(self.call_invocation(context, call)),
                role: EffectiveUseRole::ProductArgument {
                    call,
                    position,
                    slot,
                },
            }))
        }
    }
    pub(super) fn next_shared_argument(&self, cursor: &mut InputCursor, call: CallId) -> InputStep {
        let context = cursor.context;
        let data = self.program.units[self.context(context).unit.index()].data();
        let arguments = data.arguments(data.calls[call.index()].arguments).unwrap();
        let position = cursor.index;
        let Some(argument) = arguments.get(position) else {
            cursor.stage = 3;
            return InputStep::Skip;
        };
        let CallArgument::Value(value) = *argument else {
            let CallArgument::Reference(place) = *argument else {
                unreachable!()
            };
            cursor.index += 1;
            return InputStep::Dependency(InputDependency::Location(
                PlaceLocation { context, place },
                LocationUse::Shared,
            ));
        };
        if let Some(parameter) = self.call_product_parameter(context, call, position as u32) {
            let count = self.program.structs[parameter.schema.index()].fields.len();
            if cursor.component == count {
                cursor.component = 0;
                cursor.index += 1;
                return InputStep::Skip;
            }
            let slot = cursor.component;
            cursor.component += 1;
            self.component_input(context, value, call, position as u32, slot as u32)
        } else {
            cursor.index += 1;
            InputStep::Dependency(InputDependency::Value(EffectiveValueUse {
                observation: ObservationDemand::Exact,
                value,
                site: EffectiveUseSite::Operation(cursor.operation),
                role: EffectiveUseRole::Operand(position as u32),
            }))
        }
    }
    pub(super) fn next_inline_argument(
        &self,
        cursor: &mut InputCursor,
        child: ContextId,
    ) -> InputStep {
        let child_data = self.program.units[self.context(child).unit.index()].data();
        let position = cursor.index;
        let Some(&parameter) = child_data.parameters.get(position) else {
            return InputStep::Done;
        };
        if self.is_reference_parameter(parameter) {
            cursor.index += 1;
            let actual = self.inline_actual(child, position as u32);
            let CallArgument::Reference(place) = *actual.argument else {
                unreachable!("inline reference parameter mode")
            };
            return InputStep::Dependency(InputDependency::Location(
                PlaceLocation {
                    context: actual.caller,
                    place,
                },
                LocationUse::Preparation {
                    call: actual.call,
                    position: actual.position,
                },
            ));
        }
        if let Some(family) = self.product_for_cell(parameter) {
            let count = self.products[family].fields().len();
            if cursor.component == count {
                cursor.component = 0;
                cursor.index += 1;
                return InputStep::Skip;
            }
            let slot = cursor.component;
            cursor.component += 1;
            if !self.needs_product_slot(child, parameter, slot as u32) {
                return InputStep::Skip;
            }
            let (caller, value) = self.inline_parameter_source(child, position as u32);
            let OperationKind::Call(call) = self.program.units[self.context(caller).unit.index()]
                .data()
                .operations[cursor.operation.index()]
            .kind
            else {
                unreachable!("inline call recipe")
            };
            self.component_input(caller, value, call, position as u32, slot as u32)
        } else {
            cursor.index += 1;
            if self.needs_cell(child, parameter) {
                InputStep::Dependency(InputDependency::Value(
                    self.inline_parameter_input(child, position as u32),
                ))
            } else {
                InputStep::Skip
            }
        }
    }
    pub(super) fn input_step_work(&self, cursor: &InputCursor) -> usize {
        match cursor.recipe {
            InputRecipe::InlineCall { .. } => {
                1 + 2 * (usize::BITS - self.product_cell_index.len().leading_zeros()) as usize
                    + (usize::BITS - self.product_value_index.len().leading_zeros()) as usize
            }
            InputRecipe::Ordinary if cursor.stage == 0 => 1 + self.product_lookup_work(),
            InputRecipe::Ordinary if cursor.stage == 1 || cursor.stage == 2 => {
                1 + self.function_lookup_work()
                    + self.product_lookup_work()
                    + (usize::BITS - self.product_value_index.len().leading_zeros()) as usize
            }
            _ => 1,
        }
    }
}
