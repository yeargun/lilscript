//! Typed copies of retained targets. Admission precedes every allocation,
//! including nested payloads; a failed copy leaves its source unchanged.
use super::*;
use crate::compilation_policy::WorkKind::Render;
use crate::output_budget::AllocationClass::Retained;

#[cfg(test)]
#[path = "cloning_tests.rs"]
mod tests;

pub(super) fn map<T, U>(
    values: &[T],
    budget: &mut AllocationBudget<'_>,
    mut copy: impl FnMut(&T, &mut AllocationBudget<'_>) -> Result<U, AllocationError>,
) -> Result<Vec<U>, AllocationError> {
    budget.work(Render, values.len() as u64)?;
    let mut result = budget.vector(Retained, values.len())?;
    for value in values {
        result.push(copy(value, budget)?);
    }
    Ok(result)
}

fn text(value: &String, budget: &mut AllocationBudget<'_>) -> Result<String, AllocationError> {
    budget.string(Retained, value)
}
fn string(
    value: &StringValue,
    budget: &mut AllocationBudget<'_>,
) -> Result<StringValue, AllocationError> {
    budget.string_value(Retained, value)
}
fn property(
    value: &Property,
    budget: &mut AllocationBudget<'_>,
) -> Result<Property, AllocationError> {
    Ok(match value {
        Property::Named(name) => Property::Named(budget.string(Retained, name)?),
        Property::Computed(value) => Property::Computed(*value),
    })
}

impl Expr {
    /// Owned nested backing only; the expression's arena slot has its own
    /// reservation. Typed copies allocate each buffer at its exact length.
    pub(super) fn payload_bytes(&self) -> Result<u64, AllocationError> {
        use crate::output_budget::vector_bytes;
        fn add(total: &mut u64, bytes: u64) -> Result<(), AllocationError> {
            *total = total.checked_add(bytes).ok_or(AllocationError::Capacity)?;
            Ok(())
        }
        let mut bytes = 0;
        match self {
            Self::Literal(Literal::String(value)) => bytes = value.capacity_bytes() as u64,
            Self::Host(host) => bytes = host.name.capacity() as u64,
            Self::Member {
                property: Property::Named(name),
                ..
            } => bytes = name.capacity() as u64,
            Self::Intrinsic { arguments, .. }
            | Self::ConstructIntrinsic { arguments, .. }
            | Self::Call { arguments, .. }
            | Self::Construct { arguments, .. }
            | Self::SuperCall { arguments }
            | Self::Sequence(arguments)
            | Self::Array(arguments) => bytes = vector_bytes(arguments)?,
            Self::Template(parts) => {
                bytes = vector_bytes(parts)?;
                for part in parts {
                    if let TemplatePart::String(value) = part {
                        add(&mut bytes, value.capacity_bytes() as u64)?;
                    }
                }
            }
            Self::Object(entries) => {
                bytes = vector_bytes(entries)?;
                for (key, _) in entries {
                    if let Property::Named(name) = key {
                        add(&mut bytes, name.capacity() as u64)?;
                    }
                }
            }
            Self::Class { name, methods, .. } => {
                bytes = vector_bytes(methods)?;
                add(&mut bytes, name.capacity() as u64)?;
                for (name, _) in methods {
                    add(&mut bytes, name.capacity() as u64)?;
                }
            }
            Self::Regex(text) => bytes = text.capacity() as u64,
            Self::LoadModule {
                specifier, members, ..
            } => {
                bytes = vector_bytes(members)?;
                add(&mut bytes, specifier.capacity() as u64)?;
                for (name, _) in members {
                    add(&mut bytes, name.capacity() as u64)?;
                }
            }
            Self::Literal(
                Literal::Number(_) | Literal::Bool(_) | Literal::Null | Literal::Undefined,
            )
            | Self::Binding(_)
            | Self::This
            | Self::Unary { .. }
            | Self::ToInt32(_)
            | Self::IntBinary { .. }
            | Self::IntNegate(_)
            | Self::Binary { .. }
            | Self::Member {
                property: Property::Computed(_),
                ..
            }
            | Self::Conditional { .. }
            | Self::Assign { .. }
            | Self::Function(_)
            | Self::Spread(_)
            | Self::Await(_)
            | Self::Yield { .. } => {}
        }
        Ok(bytes)
    }

    /// The caller's scope owns the copy and rolls back a partially built node.
    pub(super) fn clone_in(
        &self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        use Expr::*;
        Ok(match self {
            Literal(value) => Literal(match value {
                super::Literal::String(value) => super::Literal::String(string(value, budget)?),
                super::Literal::Number(value) => super::Literal::Number(*value),
                super::Literal::Bool(value) => super::Literal::Bool(*value),
                super::Literal::Null => super::Literal::Null,
                super::Literal::Undefined => super::Literal::Undefined,
            }),
            Binding(binding) => Binding(*binding),
            Host(host) => Host(super::Host {
                name: text(&host.name, budget)?,
                kind: host.kind,
            }),
            This => This,
            Unary { op, value } => Unary {
                op: *op,
                value: *value,
            },
            ToInt32(value) => ToInt32(*value),
            IntBinary { op, left, right } => IntBinary {
                op: *op,
                left: *left,
                right: *right,
            },
            IntNegate(value) => IntNegate(*value),
            Intrinsic {
                operation,
                receiver,
                arguments,
            } => Intrinsic {
                operation: *operation,
                receiver: *receiver,
                arguments: budget.copy_slice(Retained, arguments)?,
            },
            ConstructIntrinsic {
                operation,
                arguments,
            } => ConstructIntrinsic {
                operation: *operation,
                arguments: budget.copy_slice(Retained, arguments)?,
            },
            Binary { op, left, right } => Binary {
                op: *op,
                left: *left,
                right: *right,
            },
            Member {
                object,
                property: key,
            } => Member {
                object: *object,
                property: property(key, budget)?,
            },
            Call {
                callee,
                arguments,
                invocation,
            } => Call {
                callee: *callee,
                arguments: budget.copy_slice(Retained, arguments)?,
                invocation: *invocation,
            },
            Construct { callee, arguments } => Construct {
                callee: *callee,
                arguments: budget.copy_slice(Retained, arguments)?,
            },
            Conditional { condition, yes, no } => Conditional {
                condition: *condition,
                yes: *yes,
                no: *no,
            },
            Assign { target, value } => Assign {
                target: *target,
                value: *value,
            },
            Sequence(values) => Sequence(budget.copy_slice(Retained, values)?),
            Template(parts) => Template(map(parts, budget, |part, budget| {
                Ok(match part {
                    TemplatePart::String(value) => TemplatePart::String(string(value, budget)?),
                    TemplatePart::Expression(value) => TemplatePart::Expression(*value),
                })
            })?),
            Array(values) => Array(budget.copy_slice(Retained, values)?),
            Object(entries) => Object(map(entries, budget, |(key, value), budget| {
                Ok((property(key, budget)?, *value))
            })?),
            Function(function) => Function(*function),
            Class {
                name,
                base,
                constructor,
                methods,
            } => Class {
                name: text(name, budget)?,
                base: *base,
                constructor: *constructor,
                methods: map(methods, budget, |(name, function), budget| {
                    Ok((text(name, budget)?, *function))
                })?,
            },
            SuperCall { arguments } => SuperCall {
                arguments: budget.copy_slice(Retained, arguments)?,
            },
            Spread(value) => Spread(*value),
            Await(value) => Await(*value),
            Yield { value, delegate } => Yield {
                value: *value,
                delegate: *delegate,
            },
            Regex(value) => Regex(text(value, budget)?),
            LoadModule {
                module,
                specifier,
                members,
                promise,
                string: host_string,
            } => LoadModule {
                module: *module,
                specifier: text(specifier, budget)?,
                members: map(members, budget, |(name, value), budget| {
                    Ok((text(name, budget)?, *value))
                })?,
                promise: *promise,
                string: *host_string,
            },
        })
    }
}

impl Module {
    pub(crate) fn clone_in(
        &self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        self.clone_with_print_forms(true, budget)
    }

    /// A print-site change keeps structure but must rebuild all print proofs.
    /// Do not overlap stale proof arrays with their replacement.
    pub(crate) fn clone_without_print_forms(
        &self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        self.clone_with_print_forms(false, budget)
    }

    fn clone_with_print_forms(
        &self,
        print_forms: bool,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        let mut phase = budget.scope();
        let budget = &mut phase;
        let result = Self {
            expressions: map(&self.expressions, budget, Expr::clone_in)?,
            const_freezers: budget.copy_slice(Retained, &self.const_freezers)?,
            immutable_data: budget.copy_slice(Retained, &self.immutable_data)?,
            data_estimator: self.data_estimator,
            origins: budget.copy_slice(Retained, &self.origins)?,
            authored_pool: map(&self.authored_pool, budget, string)?,
            authored_pool_formed: self.authored_pool_formed,
            forming_choices: self.forming_choices,
            authored_expressions: budget.copy_slice(Retained, &self.authored_expressions)?,
            authored_regions: budget.copy_slice(Retained, &self.authored_regions)?,
            authored_sites: budget.copy_slice(Retained, &self.authored_sites)?,
            observed_literals: budget.copy_slice(Retained, &self.observed_literals)?,
            behaviours: map(&self.behaviours, budget, |row, budget| {
                Ok(BehaviourRow {
                    expression: row.expression,
                    node: row.node.clone_in(budget)?,
                    behaviour: row.behaviour,
                })
            })?,
            journal: self.journal.clone_in(budget)?,
            settled: budget.copy_slice(Retained, &self.settled)?,
            first_runs: budget.copy_slice(Retained, &self.first_runs)?,
            regions: map(&self.regions, budget, |region, budget| {
                Ok(Region {
                    scope: region.scope,
                    statements: budget.copy_slice(Retained, &region.statements)?,
                })
            })?,
            functions: map(&self.functions, budget, |function, budget| {
                Ok(Function {
                    parameters: budget.copy_slice(Retained, &function.parameters)?,
                    name: match &function.name {
                        FunctionName::Unobserved => FunctionName::Unobserved,
                        FunctionName::Exact(name) => FunctionName::Exact(string(name, budget)?),
                    },
                    rest: function.rest,
                    body: function.body,
                    arrow: function.arrow,
                    strict: function.strict,
                    length: function.length,
                    suspension: function.suspension,
                })
            })?,
            scopes: budget.copy_slice(Retained, &self.scopes)?,
            bindings: map(&self.bindings, budget, |binding, budget| {
                Ok(Binding {
                    spelling: text(&binding.spelling, budget)?,
                    source_symbol: binding.source_symbol,
                    scope: binding.scope,
                    pinned: binding.pinned,
                    class: binding.class,
                    defined: binding.defined,
                })
            })?,
            imports: map(&self.imports, budget, |import, budget| {
                Ok(Import {
                    source: string(&import.source, budget)?,
                    imported: text(&import.imported, budget)?,
                    binding: import.binding,
                })
            })?,
            exports: map(&self.exports, budget, |export, budget| {
                Ok(Export {
                    binding: export.binding,
                    name: text(&export.name, budget)?,
                })
            })?,
            root: self.root,
            pristine_builtins: self.pristine_builtins,
            pure_property_reads: self.pure_property_reads,
            unconstructed_callbacks: self.unconstructed_callbacks,
            root_rows: budget.copy_slice(Retained, &self.root_rows)?,
            entries: map(&self.entries, budget, |entry, budget| {
                Ok(EntryPublic {
                    name: text(&entry.name, budget)?,
                    exports: budget.copy_slice(Retained, &entry.exports)?,
                })
            })?,
            delivery: self
                .delivery
                .as_ref()
                .map(|delivery| delivery.clone_in(budget))
                .transpose()?,
            reserved: map(&self.reserved, budget, text)?,
            carried: map(&self.carried, budget, text)?,
            loop_head_declarations: self.loop_head_declarations,
            logical_statements: self.logical_statements,
            compound_assignments: self.compound_assignments,
            quotes: self.quotes,
            int32_hints: self.int32_hints,
            choice_sites: map(&self.choice_sites, budget, |site, budget| {
                Ok(ChoiceSite {
                    estimate_codec: site.estimate_codec,
                    key: site.key,
                    name: text(&site.name, budget)?,
                    alternatives: budget.copy_slice(Retained, &site.alternatives)?,
                    seed: site.seed,
                    applied: site.applied,
                    pinned: site.pinned,
                })
            })?,
            spelling_head: self.spelling_head,
            spelling_regions: self.spelling_regions,
            spelling_functions: self.spelling_functions,
            spelling_node_count: self.spelling_node_count,
            spelling_nodes: budget.copy_slice(Retained, &self.spelling_nodes)?,
            print_forms: self
                .print_forms
                .as_ref()
                .filter(|_| print_forms)
                .map(|forms| forms.clone_in(budget))
                .transpose()?,
        };
        phase.finish_retained()?;
        Ok(result)
    }
}

impl delivery::DeliveryPlan {
    pub(super) fn clone_in(
        &self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        use delivery::*;
        Ok(Self {
            mode: self.mode,
            files: map(&self.files, budget, |file, budget| file.clone_in(budget))?,
            entries: map(&self.entries, budget, |entry, budget| {
                Ok(EntryDelivery {
                    name: text(&entry.name, budget)?,
                    file: entry.file,
                    dynamic: entry.dynamic,
                    closure: budget.copy_slice(Retained, &entry.closure)?,
                })
            })?,
            setters: budget.copy_slice(Retained, &self.setters)?,
            statics: self.statics,
            entry_names: map(&self.entry_names, budget, text)?,
            naming: map(&self.naming, budget, |file, budget| {
                Ok(FileName {
                    template: text(&file.template, budget)?,
                    name: text(&file.name, budget)?,
                    index: file.index,
                    path: text(&file.path, budget)?,
                    ext: text(&file.ext, budget)?,
                })
            })?,
            preload: self.preload,
            format: self.format,
            container: self.container.clone_in(budget)?,
            request_bytes: self.request_bytes,
            depth_bytes: self.depth_bytes,
        })
    }
}
