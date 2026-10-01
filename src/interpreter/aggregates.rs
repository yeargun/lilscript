//! Independent aggregate storage and invocation. This module never imports IR,
//! target formation or optimizer facts. Struct stores copy values; class and
//! collection stores preserve identity, including when used as Map/Set keys.
use super::*;
use crate::ast::{Argument, ClassDecl, ClassMember, FieldDecl, TypeRef};

pub(super) fn copy_for_store(value: Value) -> Value {
    match value {
        Value::Struct(value) => Value::Struct(Rc::new(AggregateValue {
            name: value.name.clone(),
            fields: Rc::new(RefCell::new(
                value
                    .fields
                    .borrow()
                    .iter()
                    .map(|(key, value)| (key.clone(), copy_for_store(value.clone())))
                    .collect(),
            )),
        })),
        value => value,
    }
}

impl<'program, 'ast, 'src> ReferenceInterpreter<'program, 'ast, 'src> {
    pub(super) fn evaluate_arguments(
        &mut self,
        args: &[Argument<'ast, 'src>],
    ) -> Result<Vec<Value>, InterpretError> {
        let mut values = Vec::new();
        for argument in args {
            let value = self.evaluate(&argument.expression)?;
            if argument.spread {
                let Value::Array(array) = value else {
                    return Err(InterpretError::new(
                        argument.span,
                        "unsupported argument spread",
                    ));
                };
                values.extend(array.borrow().iter().cloned());
            } else {
                values.push(value);
            }
        }
        Ok(values)
    }

    pub(super) fn evaluate_struct(
        &mut self,
        name: &str,
        values: &[Expr<'ast, 'src>],
        span: Span,
    ) -> Result<Value, InterpretError> {
        let declaration = self
            .program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Struct(declaration) if declaration.name.name == name => Some(declaration),
                _ => None,
            })
            .ok_or_else(|| InterpretError::new(span, "unknown struct declaration"))?;
        let mut fields = IndexMap::new();
        for (index, field) in declaration.fields.iter().enumerate() {
            let value = if let Some(value) = values.get(index) {
                self.evaluate(value)?
            } else {
                self.field_default(field)?
            };
            fields.insert(field.name.name.to_string(), copy_for_store(value));
        }
        Ok(Value::Struct(Rc::new(AggregateValue {
            name: name.into(),
            fields: Rc::new(RefCell::new(fields)),
        })))
    }

    pub(super) fn class_declaration(&self, name: &str) -> Option<&'program ClassDecl<'ast, 'src>> {
        self.program.items.iter().find_map(|item| match item {
            Item::Class(class) if class.name.name == name => Some(class),
            _ => None,
        })
    }

    fn base_class(
        &self,
        class: &ClassDecl<'ast, 'src>,
    ) -> Result<Option<&'program ClassDecl<'ast, 'src>>, InterpretError> {
        match class.base {
            None => Ok(None),
            Some(TypeRef {
                kind: TypeKind::Named { name, .. },
                span,
            }) => self
                .class_declaration(name)
                .map(Some)
                .ok_or_else(|| InterpretError::new(span, "unsupported external base class")),
            Some(base) => Err(InterpretError::new(base.span, "invalid class base")),
        }
    }

    pub(super) fn class_method(
        &self,
        name: &str,
        method: &str,
    ) -> Option<&'program FunctionDecl<'ast, 'src>> {
        let mut class = self.class_declaration(name)?;
        loop {
            if let Some(function) = class.members.iter().find_map(|member| match member {
                ClassMember::Method(function) if function.name.name == method => Some(function),
                _ => None,
            }) {
                return Some(function);
            }
            class = self.base_class(class).ok()??;
        }
    }

    pub(super) fn invoke_method(
        &mut self,
        receiver: Rc<AggregateValue>,
        function: &'program FunctionDecl<'ast, 'src>,
        values: Vec<Value>,
        span: Span,
    ) -> Result<Value, InterpretError> {
        if function.is_async || function.is_generator {
            return Err(InterpretError::new(
                span,
                "unsupported async/generator method",
            ));
        }
        let mut frame = AHashMap::new();
        frame.insert(
            self.symbol(&function.this)?,
            Rc::new(RefCell::new(Value::Instance(receiver))),
        );
        self.execute_callable_frame(frame, function.params, values, function.body, None, span)
            .map(|value| {
                if matches!(function.return_type.kind, TypeKind::Float) {
                    coerce_value_to_type(value, &Type::Float)
                } else {
                    copy_for_store(value)
                }
            })
    }

    pub(super) fn initialize_class(
        &mut self,
        receiver: Rc<AggregateValue>,
        class: &'program ClassDecl<'ast, 'src>,
        values: Vec<Value>,
        span: Span,
    ) -> Result<(), InterpretError> {
        self.step(span)?;
        let constructor = class.members.iter().find_map(|member| match member {
            ClassMember::Constructor(constructor) => Some(constructor),
            _ => None,
        });
        let base = self.base_class(class)?;
        if let Some(constructor) = constructor {
            let mut frame = AHashMap::new();
            frame.insert(
                self.symbol(&constructor.this)?,
                Rc::new(RefCell::new(Value::Instance(receiver.clone()))),
            );
            if base.is_none() {
                self.initialize_fields(&receiver, class)?;
            }
            self.constructors.push((receiver, class));
            let result = self.execute_callable_frame(
                frame,
                constructor.params,
                values,
                constructor.body,
                None,
                span,
            );
            self.constructors.pop();
            result?;
        } else {
            if !values.is_empty() {
                return Err(InterpretError::new(
                    span,
                    "implicit constructor takes no arguments",
                ));
            }
            if let Some(base) = base {
                self.initialize_class(receiver.clone(), base, Vec::new(), span)?;
            }
            self.initialize_fields(&receiver, class)?;
        }
        Ok(())
    }

    pub(super) fn evaluate_super(
        &mut self,
        args: &[Argument<'ast, 'src>],
        span: Span,
    ) -> Result<(), InterpretError> {
        let (receiver, class) = self
            .constructors
            .last()
            .cloned()
            .ok_or_else(|| InterpretError::new(span, "super outside a constructor"))?;
        let base = self
            .base_class(class)?
            .ok_or_else(|| InterpretError::new(span, "super has no base class"))?;
        let values = self.evaluate_arguments(args)?;
        self.initialize_class(receiver.clone(), base, values, span)?;
        self.initialize_fields(&receiver, class)
    }

    fn initialize_fields(
        &mut self,
        receiver: &AggregateValue,
        class: &ClassDecl<'ast, 'src>,
    ) -> Result<(), InterpretError> {
        for member in class.members {
            if let ClassMember::Field(field) = member {
                let value = self.field_default(field)?;
                receiver
                    .fields
                    .borrow_mut()
                    .insert(field.name.name.into(), copy_for_store(value));
            }
        }
        Ok(())
    }

    fn field_default(&mut self, field: &FieldDecl<'ast, 'src>) -> Result<Value, InterpretError> {
        if let Some(initializer) = &field.initializer {
            return self.evaluate(initializer);
        }
        self.default_value(field.ty)
    }

    fn default_value(&mut self, ty: TypeRef<'ast, 'src>) -> Result<Value, InterpretError> {
        self.step(ty.span)?;
        Ok(match ty.kind {
            TypeKind::Int => Value::Int(0),
            TypeKind::Float => Value::Float(0.0),
            TypeKind::Bool => Value::Bool(false),
            TypeKind::String => Value::String(String::new()),
            TypeKind::Nullable(_) => Value::Null,
            TypeKind::Array(_) => Value::Array(Rc::default()),
            TypeKind::Named { name: "Map", .. } => Value::Map(Rc::default()),
            TypeKind::Named { name: "Set", .. } => Value::Set(Rc::default()),
            TypeKind::Named { name: "Record", .. } => Value::Record(Rc::default()),
            TypeKind::Named { name, .. } if self.semantics.struct_info(name).is_some() => {
                self.evaluate_struct(name, &[], ty.span)?
            }
            TypeKind::Named { name, .. }
                if self.program.items.iter().any(
                    |item| matches!(item, Item::Enum(declaration) if declaration.name.name == name),
                ) =>
            {
                Value::Int(0)
            }
            TypeKind::Union(members) if !members.is_empty() => self.default_value(members[0])?,
            // Checked constructors assign these before reads. Do not invent a
            // host constructor or a default instance to approximate a class.
            _ => Value::Void,
        })
    }

    pub(super) fn evaluate_collection_method(
        &mut self,
        receiver: Value,
        method: &str,
        args: &[Value],
        span: Span,
    ) -> Result<Value, InterpretError> {
        match &receiver {
            Value::Map(map) => match (method, args) {
                ("set", [key, value]) => {
                    let mut entries = map.borrow_mut();
                    if let Some((_, old)) = entries
                        .iter_mut()
                        .flatten()
                        .find(|(found, _)| values_same_value_zero(found, key))
                    {
                        *old = copy_for_store(value.clone());
                    } else {
                        entries.push(Some((key.clone(), copy_for_store(value.clone()))));
                    }
                    Ok(receiver.clone())
                }
                ("get", [key]) => Ok(map
                    .borrow()
                    .iter()
                    .flatten()
                    .find(|(found, _)| values_same_value_zero(found, key))
                    .map_or(Value::Null, |(_, value)| value.clone())),
                ("has", [key]) => Ok(Value::Bool(
                    map.borrow()
                        .iter()
                        .flatten()
                        .any(|(found, _)| values_same_value_zero(found, key)),
                )),
                ("delete", [key]) => {
                    let mut entries = map.borrow_mut();
                    let found = entries.iter_mut().find(|entry| {
                        entry
                            .as_ref()
                            .is_some_and(|(found, _)| values_same_value_zero(found, key))
                    });
                    Ok(Value::Bool(if let Some(entry) = found {
                        *entry = None;
                        true
                    } else {
                        false
                    }))
                }
                ("clear", []) => {
                    for entry in map.borrow_mut().iter_mut() {
                        *entry = None;
                    }
                    Ok(Value::Void)
                }
                ("keys" | "values", []) => Ok(Value::Array(Rc::new(RefCell::new(
                    map.borrow()
                        .iter()
                        .flatten()
                        .map(|(key, value)| {
                            if method == "keys" {
                                key.clone()
                            } else {
                                copy_for_store(value.clone())
                            }
                        })
                        .collect(),
                )))),
                _ => Err(InterpretError::new(
                    span,
                    format!("unsupported Map method `{method}`"),
                )),
            },
            Value::Set(set) => match (method, args) {
                ("add", [value]) => {
                    if !set
                        .borrow()
                        .iter()
                        .flatten()
                        .any(|found| values_same_value_zero(found, value))
                    {
                        set.borrow_mut().push(Some(value.clone()));
                    }
                    Ok(receiver.clone())
                }
                ("has", [value]) => Ok(Value::Bool(
                    set.borrow()
                        .iter()
                        .flatten()
                        .any(|found| values_same_value_zero(found, value)),
                )),
                ("delete", [value]) => {
                    let mut entries = set.borrow_mut();
                    let found = entries.iter_mut().find(|entry| {
                        entry
                            .as_ref()
                            .is_some_and(|found| values_same_value_zero(found, value))
                    });
                    Ok(Value::Bool(if let Some(entry) = found {
                        *entry = None;
                        true
                    } else {
                        false
                    }))
                }
                ("clear", []) => {
                    for entry in set.borrow_mut().iter_mut() {
                        *entry = None;
                    }
                    Ok(Value::Void)
                }
                ("values" | "keys", []) => Ok(Value::Array(Rc::new(RefCell::new(
                    set.borrow().iter().flatten().cloned().collect(),
                )))),
                _ => Err(InterpretError::new(
                    span,
                    format!("unsupported Set method `{method}`"),
                )),
            },
            _ => unreachable!(),
        }
    }
}
