//! Tagged values: nullables, unions and type parameters share one C type,
//! `ls_value`. A value moves between a tagged slot and a typed one through
//! a borrowed representation conversion. Product boxing owns an immutable
//! snapshot through the activation's statement-temporary chain; consumers
//! retain escaping results. Calls and adapters explicitly replace ownership
//! when moving between a product box and inline fields. Checked unboxing
//! validates the tag and the product layout witness.
use super::*;

/// Tags follow JavaScript's runtime categories where a type test reads them.
pub(in crate::program) const INTERFACE: &str = include_str!("runtime/value.h");
pub(in crate::program) const RUNTIME: &str = concat!(include_str!("runtime/value.h"), include_str!("runtime/value.c"));

impl Emitter<'_, '_, '_, '_, '_> {
    /// Boxing and unboxing of each callable signature. Every callable record
    /// has the tagged value's callable layout, which the assertion checks.
    pub(super) fn dynamic_callables(&mut self) -> Result<(), NativeError> {
        if !self.plan.helpers.contains(Helper::Dynamic) {
            return Ok(());
        }
        for index in 0..self.plan.signatures.len() {
            self.budget.work(WorkKind::Render, 1)?;
            if !self.plan.callable_signature_needed(index) {
                continue;
            }
            self.write(format_args!(
                "_Static_assert(sizeof(ls_callable{index}) == sizeof(((ls_value *)0)->as.c), \"callable layout\");\n\
static LS_NATIVE_UNUSED ls_value ls_value_callable{index}(ls_callable{index} value) {{ ls_value result = {{.tag = LS_CALLABLE, .signature = {index}}}; memcpy(&result.as.c, &value, sizeof value); return result; }}\n\
static LS_NATIVE_UNUSED ls_callable{index} ls_value_to_callable{index}(ls_value value) {{\n\
if (value.tag == LS_NULL) return (ls_callable{index}){{0}};\n\
if (value.tag != LS_CALLABLE || value.signature != {index}) {{ ls_value_mismatch(); return (ls_callable{index}){{0}}; }}\n\
ls_callable{index} result; memcpy(&result, &value.as.c, sizeof result); return result;\n}}\n"
            ))?;
        }
        Ok(())
    }

    /// The C wrapped around an expression of type `from` to give it type
    /// `to`. The plan admits only these pairs.
    pub(super) fn conversion(from: NativeType, to: NativeType) -> (String, &'static str) {
        use NativeType::*;
        let none = || (std::string::String::new(), "");
        if from == to {
            return none();
        }
        match (from, to) {
            (Dynamic(_), Dynamic(_)) | (Object(_), Object(_)) | (Array(_), Array(_)) => none(),
            (I32, Dynamic(_)) => ("ls_value_int(".into(), ")"),
            (F64, Dynamic(_)) => ("ls_value_float(".into(), ")"),
            (Bool, Dynamic(_)) => ("ls_value_bool(".into(), ")"),
            (String, Dynamic(_)) => ("ls_value_string(".into(), ")"),
            (Struct(index), Dynamic(_)) => (format!("ls_t{index}_box(&ls_temps,"), ")"),
            (Dynamic(_), Struct(index)) => (format!("ls_value_to_t{index}("), ")"),
            (Object(_) | Shape | Record | Map | Set | Regex | Buffer | Typed(_), Dynamic(_)) => {
                ("ls_value_object(".into(), ")")
            }
            (Symbol, Dynamic(_)) => ("ls_value_symbol(".into(), ")"),
            (Array(_), Dynamic(_)) => ("ls_value_array((ls_native_object *)".into(), ")"),
            (Callable(signature), Dynamic(_)) => (format!("ls_value_callable{signature}("), ")"),
            (Dynamic(_), I32) => ("ls_value_to_int(".into(), ")"),
            (Dynamic(_), F64) => ("ls_value_to_number(".into(), ")"),
            (Dynamic(_), Bool) => ("ls_value_to_bool(".into(), ")"),
            (Dynamic(_), String) => ("ls_value_to_string(".into(), ")"),
            (Dynamic(_), Object(_) | Shape | Record | Map | Set | Regex | Buffer | Typed(_)) => {
                ("ls_value_to_object(".into(), ")")
            }
            (Dynamic(_), Symbol) => ("ls_value_to_symbol(".into(), ")"),
            (Dynamic(_), Array(array)) => (format!("(ls_array{array} *)ls_value_to_array("), ")"),
            (Dynamic(_), Callable(signature)) => (format!("ls_value_to_callable{signature}("), ")"),
            (I32, F64) => ("(double)(".into(), ")"),
            _ => unreachable!("native plan admits only these representation changes"),
        }
    }

    /// A value in the representation of `to`.
    pub(super) fn converted(
        &mut self,
        unit: UnitId,
        value: ValueId,
        to: NativeType,
    ) -> Result<(), NativeError> {
        let from = self
            .plan
            .value_type(self.plan.units[unit.index()].values[value.index()]);
        if from != to && self.plan.callable_view(from, to) {
            self.write(format_args!("({to}){{("))?;
            self.value(unit, value)?;
            self.text(").code,(")?;
            self.value(unit, value)?;
            self.text(").environment,(")?;
            self.value(unit, value)?;
            return self.text(").identity}");
        }
        let (prefix, suffix) = Self::conversion(from, to);
        self.text(&prefix)?;
        self.value(unit, value)?;
        self.text(suffix)
    }

    /// `value is T`: the tag of a tagged value, or a fact of a typed one.
    pub(super) fn type_test(
        &mut self,
        unit: UnitId,
        result: ValueId,
        value: ValueId,
        target: crate::primitive::RuntimeTypeTest,
    ) -> Result<(), NativeError> {
        use crate::primitive::RuntimeTypeTest;
        let tags = match target {
            RuntimeTypeTest::TypeOf("number") => "LS_INT || ls_t == LS_FLOAT",
            RuntimeTypeTest::TypeOf("string") => "LS_STRING",
            RuntimeTypeTest::TypeOf("boolean") => "LS_BOOL",
            RuntimeTypeTest::TypeOf("function") => "LS_CALLABLE",
            RuntimeTypeTest::IsArray => "LS_ARRAY",
            RuntimeTypeTest::TypeOf(_) => unreachable!("native plan admits these type tests"),
        };
        let ty = self
            .plan
            .value_type(self.plan.units[unit.index()].values[value.index()]);
        if let NativeType::Dynamic(_) = ty {
            return self.write(format_args!(
                "{{ uint8_t ls_t = ls_v{}.tag; ls_v{} = ls_t == {tags}; }}\n",
                value.index(),
                result.index()
            ));
        }
        let holds = match (ty, target) {
            (NativeType::I32 | NativeType::F64, RuntimeTypeTest::TypeOf("number")) => true,
            (NativeType::String, RuntimeTypeTest::TypeOf("string")) => true,
            (NativeType::Bool, RuntimeTypeTest::TypeOf("boolean")) => true,
            (NativeType::Callable(_), RuntimeTypeTest::TypeOf("function")) => true,
            (NativeType::Array(_), RuntimeTypeTest::IsArray) => true,
            _ => false,
        };
        self.write(format_args!("ls_v{} = {holds};\n", result.index()))
    }
}

impl Emitter<'_, '_, '_, '_, '_> {
    /// One adapter per admitted `(from, to)` pair: a callable of signature
    /// `to` whose environment owns the adapted callable. Parameters convert
    /// from `to` to `from` and the result back, which changes no ownership.
    /// The adapter keeps the inner identity, so equality sees one function.
    pub(super) fn adapters(&mut self) -> Result<(), NativeError> {
        for index in 0..self.plan.adapters.len() {
            self.budget.work(WorkKind::Render, 1)?;
            let (from, to) = self.plan.adapters[index];
            let name = format!("ls_adapter{from}_{to}");
            self.write(format_args!(
                "typedef struct {{ ls_native_object owner; ls_callable{from} inner; }} {name};\n\
static LS_NATIVE_UNUSED void {name}_destroy(ls_native_object *owner) {{ ls_callable{from}_clear(&(({name} *)owner)->inner); }}\n\
static LS_NATIVE_UNUSED void {name}_trace(ls_native_object *owner, ls_native_visit visit, void *context) {{ visit((({name} *)owner)->inner.environment, context); }}\n\
static LS_NATIVE_UNUSED {} {name}_code(void *environment",
                self.plan.signatures[to].result
            ))?;
            self.signature_parameters(to, true, true)?;
            self.write(format_args!(") {{\n{name} *adapter = environment;\n"))?;
            let (inner, outer) = (
                self.plan.signatures[from].result,
                self.plan.signatures[to].result,
            );
            self.temporary_declaration()?;
            let (prefix, suffix) = Self::conversion(inner, outer);
            if inner != NativeType::Void {
                self.write(format_args!("{inner} ls_inner = "))?;
            }
            self.text("adapter->inner.code(adapter->inner.environment")?;
            for position in 0..self.plan.signatures[to].parameters.len() {
                let (prefix, suffix) = Self::conversion(
                    self.plan.signatures[to].parameters[position],
                    self.plan.signatures[from].parameters[position],
                );
                self.write(format_args!(",{prefix}ls_p{position}{suffix}"))?;
            }
            if self.plan.signatures[from].has_optional() {
                if self.plan.signatures[to].has_optional() {
                    self.text(",ls_argc")?;
                } else {
                    self.write(format_args!(
                        ",{}",
                        self.plan.signatures[to].parameters.len()
                    ))?;
                }
            }
            self.text(");\nif (ls_native_raised) {\n")?;
            if let Some(drop)=inner.release("ls_inner") { self.text(&drop)?; }
            self.clear_temporaries()?;
            if outer==NativeType::Void { self.text("return;\n")?; }
            else { self.write(format_args!("return ({outer}){{0}};\n"))?; }
            self.text("}\n")?;
            if outer != NativeType::Void {
                self.write(format_args!("{outer} ls_outer = {prefix}ls_inner{suffix};\n"))?;
                if Self::product_conversion(inner, outer) {
                    if let Some(retain) = outer.retain("ls_outer") { self.text(&retain)?; }
                    if let Some(drop) = inner.release("ls_inner") { self.text(&drop)?; }
                }
            }
            self.clear_temporaries()?;
            if outer != NativeType::Void { self.text("return ls_outer;\n")?; }
            self.write(format_args!(
                "}}\n\
static LS_NATIVE_UNUSED ls_callable{to} ls_adapt{from}_{to}(ls_callable{from} inner) {{\n\
{name} *adapter = ls_native_allocate(sizeof *adapter, {name}_destroy, {name}_trace);\n\
ls_native_retain(inner.environment);\n\
adapter->inner = inner;\n\
return (ls_callable{to}){{{name}_code, adapter, inner.identity}};\n}}\n\
static LS_NATIVE_UNUSED ls_value ls_adapt_value{from}_{to}(ls_value value) {{\n\
if (value.tag != LS_CALLABLE) {{ ls_value_retain(value); return value; }}\n\
return ls_value_callable{to}(ls_adapt{from}_{to}(ls_value_to_callable{from}(value)));\n}}\n"
            ))?;
        }
        Ok(())
    }
}
