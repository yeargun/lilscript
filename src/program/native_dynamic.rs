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
    /// Tagged callable views only bridge pairs admitted by actual storage and
    /// call demands. The activation owns allocated bridges until a consumer
    /// retains them; the function identity survives every physical view.
    pub(super) fn dynamic_callables(&mut self) -> Result<(), NativeError> {
        if !self.plan.helpers.contains(Helper::Dynamic) { return Ok(()); }
        for index in 0..self.plan.signatures.len() {
            self.budget.work(WorkKind::Render,1)?;
            if !self.plan.callable_signature_needed(index) { continue; }
            self.write(format_args!(
                "_Static_assert(sizeof(ls_callable{index}) == sizeof(((ls_value *)0)->as.c), \"callable layout\");\n\
static LS_NATIVE_UNUSED ls_value ls_value_callable{index}(ls_callable{index} value) {{ ls_value result = {{.tag=LS_CALLABLE,.signature={index}}}; memcpy(&result.as.c,&value,sizeof value); return result; }}\n"))?;
        }
        for &(from,to) in &self.plan.adapters {
            self.write(format_args!("static LS_NATIVE_UNUSED ls_callable{to} ls_borrow_adapt{from}_{to}(ls_native_temporary **temps,ls_callable{from} inner);\n"))?;
        }
        for index in 0..self.plan.signatures.len() {
            self.budget.work(WorkKind::Render,1)?;
            if !self.plan.callable_signature_needed(index) { continue; }
            self.write(format_args!("static LS_NATIVE_UNUSED ls_callable{index} ls_value_to_callable{index}(ls_native_temporary **temps,ls_value value) {{\n(void)temps;\nif(value.tag==LS_CALLABLE) switch(value.signature) {{\ncase {index}: {{ls_callable{index} result; memcpy(&result,&value.as.c,sizeof result); return result;}}\n"))?;
            for &(from,to) in &self.plan.adapters {
                if to==index { self.write(format_args!("case {from}: {{ls_callable{from} inner; memcpy(&inner,&value.as.c,sizeof inner); return ls_borrow_adapt{from}_{to}(temps,inner);}}\n"))?; }
            }
            self.write(format_args!("}}\nls_value_mismatch(); return (ls_callable{index}){{0}};\n}}\n"))?;
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
            (Dynamic(_), Dynamic(_)) | (Array(_), Array(_)) => none(),
            (Object(_), Object(target)) => (format!("ls_object_view{target}("), ")"),
            (I32, Dynamic(_)) => ("ls_value_int(".into(), ")"),
            (F64, Dynamic(_)) => ("ls_value_float(".into(), ")"),
            (Bool, Dynamic(_)) => ("ls_value_bool(".into(), ")"),
            (String, Dynamic(_)) => ("ls_value_string(".into(), ")"),
            (Struct(index), Dynamic(_)) => (format!("ls_t{index}_box(&ls_temps,"), ")"),
            (Dynamic(_), Struct(index)) => (format!("ls_value_to_t{index}("), ")"),
            (Object(_) | Shape | Record | Map | Set | Regex | Generator | Buffer | Typed(_), Dynamic(_)) => {
                ("ls_value_object(".into(), ")")
            }
            (Symbol, Dynamic(_)) => ("ls_value_symbol(".into(), ")"),
            (Array(_), Dynamic(_)) => ("ls_value_array((ls_native_object *)".into(), ")"),
            (Callable(signature), Dynamic(_)) => (format!("ls_value_callable{signature}("), ")"),
            (Dynamic(_), I32) => ("ls_value_to_int(".into(), ")"),
            (Dynamic(_), F64) => ("ls_value_to_number(".into(), ")"),
            (Dynamic(_), Bool) => ("ls_value_to_bool(".into(), ")"),
            (Dynamic(_), String) => ("ls_value_to_string(".into(), ")"),
            (Dynamic(_), Object(target)) => (format!("ls_value_to_object{target}("), ")"),
            (Dynamic(_), Shape|Record) => ("ls_value_to_record(".into(), ")"),
            (Dynamic(_), Map|Set) => ("ls_value_to_map(".into(), ")"),
            (Dynamic(_), Generator) => ("ls_value_to_generator(".into(), ")"),
            (Dynamic(_), Regex) => ("ls_value_to_regex(".into(), ")"),
            (Dynamic(_), Buffer) => ("ls_value_to_buffer(".into(), ")"),
            (Dynamic(_), Typed(kind)) => (format!("ls_value_to_typed({},",kind.bytes_per_element()), ")"),
            (Dynamic(_), Symbol) => ("ls_value_to_symbol(".into(), ")"),
            (Dynamic(_), Array(array)) => (format!("(ls_array{array} *)ls_value_to_array("), ")"),
            (Dynamic(_), Callable(signature)) => (format!("ls_value_to_callable{signature}(&ls_temps,"), ")"),
            (Callable(from), Callable(to)) => (format!("ls_borrow_adapt{from}_{to}(&ls_temps,"), ")"),
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
        if let (NativeType::Object(from),NativeType::Object(to))=(from,to) {
            if self.plan.class_extends(from,to) { return self.value(unit,value); }
        }
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
    /// A bridge owns its original callable and keeps its identity. Reversing
    /// a view recovers that original instead of growing an adapter chain.
    pub(super) fn adapters(&mut self) -> Result<(), NativeError> {
        if !self.plan.adapters.is_empty() { self.text(include_str!("runtime/callable_bridge.c"))?; }
        for index in 0..self.plan.adapters.len() {
            self.budget.work(WorkKind::Render, 1)?;
            let (from,to)=self.plan.adapters[index];
            let (inner,outer)=(self.plan.signatures[from].result,self.plan.signatures[to].result);
            if self.plan.callable_view(NativeType::Callable(from),NativeType::Callable(to)) {
                self.write(format_args!("static LS_NATIVE_UNUSED ls_callable{to} ls_borrow_adapt{from}_{to}(ls_native_temporary **temps,ls_callable{from} inner) {{ (void)temps; return (ls_callable{to}){{inner.code,inner.environment,inner.identity}}; }}\n"))?;
                continue;
            }
            self.write(format_args!("static LS_NATIVE_UNUSED {outer} ls_adapter{from}_{to}_code(void *environment"))?;
            self.signature_parameters(to,true,true)?;
            self.write(format_args!(") {{\nls_callable_bridge *adapter=environment;\nls_callable{from} inner; memcpy(&inner,&adapter->inner.as.c,sizeof inner);\n"))?;
            self.temporary_declaration()?;
            let mut missing=Vec::new();
            for position in 0..self.plan.signatures[to].parameters.len() {
                let actual=self.plan.signatures[to].parameters[position];
                let target=self.plan.signatures[from].parameters[position];
                let argument=format!("ls_p{position}");
                let absent=self.absent_argument(&argument,actual);
                let absent=if self.plan.signatures[to].has_optional() {
                    format!("(ls_args.count<={position} || (ls_args.absent && ls_args.absent[{position}]) || {absent})")
                } else {absent};
                self.write(format_args!("{target} ls_argument{position} = "))?;
                self.physical_argument(&argument,actual,target,self.plan.signatures[from].source.params[position].optional.then_some(absent.as_str()))?;
                self.text(";\n")?;
                missing.push(if self.plan.signatures[from].source.params[position].optional {absent} else {"false".to_owned()});
            }
            self.text("if (ls_native_raised) {\n")?; self.clear_temporaries()?;
            if outer==NativeType::Void {self.text("return;\n")?;} else {self.write(format_args!("return ({outer}){{0}};\n"))?;}
            self.text("}\n")?;
            if inner!=NativeType::Void {self.write(format_args!("{inner} ls_inner = "))?;}
            self.text("inner.code(inner.environment")?;
            for position in 0..self.plan.signatures[from].parameters.len() {self.write(format_args!(",ls_argument{position}"))?;}
            if self.plan.signatures[from].has_optional() {self.argument_presence(missing.len(),&missing)?;}
            self.text(");\nif (ls_native_raised) {\n")?;
            if let Some(drop)=inner.release("ls_inner") {self.text(&drop)?;}
            self.clear_temporaries()?;
            if outer==NativeType::Void {self.text("return;\n")?;} else {self.write(format_args!("return ({outer}){{0}};\n"))?;}
            self.text("}\n")?;
            if outer!=NativeType::Void {
                let (prefix,suffix)=Self::conversion(inner,outer);
                self.write(format_args!("{outer} ls_outer = {prefix}ls_inner{suffix};\n"))?;
                if inner!=outer {
                    if let Some(retain)=outer.retain("ls_outer") {self.text(&retain)?;}
                    if let Some(drop)=inner.release("ls_inner") {self.text(&drop)?;}
                }
            } else if let Some(drop)=inner.release("ls_inner") {self.text(&drop)?;}
            self.clear_temporaries()?;
            if outer!=NativeType::Void {self.text("return ls_outer;\n")?;}
            self.write(format_args!("}}\nstatic LS_NATIVE_UNUSED ls_callable{to} ls_borrow_adapt{from}_{to}(ls_native_temporary **temps,ls_callable{from} inner) {{\nif(!inner.code) {{ls_value_mismatch(); return (ls_callable{to}){{0}};}}\nif(inner.environment && ((ls_native_object *)inner.environment)->destroy==ls_callable_bridge_destroy) {{\nls_callable_bridge *prior=inner.environment;\nif(prior->inner.signature=={to}) {{ls_callable{to} result; memcpy(&result,&prior->inner.as.c,sizeof result); return result;}}\n}}\nls_callable_bridge *adapter=ls_native_allocate(sizeof *adapter,ls_callable_bridge_destroy,ls_callable_bridge_trace);\nadapter->inner=ls_value_callable{from}(inner); ls_value_retain(adapter->inner);\nls_native_temporary_push(temps,&adapter->temporary,&adapter->owner);\nreturn (ls_callable{to}){{ls_adapter{from}_{to}_code,adapter,inner.identity}};\n}}\n"))?;
        }
        Ok(())
    }
}
