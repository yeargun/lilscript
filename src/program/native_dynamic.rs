//! Tagged values: nullables, unions and type parameters share one C type,
//! `ls_value`. A value moves between a tagged slot and a typed one through
//! a representation conversion, which never changes ownership: boxing a
//! borrowed payload yields a borrowed tagged value, and unboxing an owned
//! one yields the owned payload. Unboxing checks the tag and stops the
//! program on a mismatch, which checked narrowing never produces.
use super::*;

/// Tags follow JavaScript's runtime categories where a type test reads them.
pub(in crate::program) const RUNTIME: &str = r#"typedef struct ls_value {
    uint8_t tag;
    /* A callable payload's physical signature. */
    uint32_t signature;
    union {
        int32_t i;
        double f;
        bool b;
        ls_string s;
        ls_native_object *o;
        struct { void (*code)(void); void *environment; uint64_t identity; } c;
    } as;
} ls_value;
enum { LS_NULL, LS_INT, LS_FLOAT, LS_BOOL, LS_STRING, LS_OBJECT, LS_ARRAY, LS_CALLABLE, LS_SYMBOL };
static void ls_value_mismatch(void) {
    fputs("LilScript native value has an unexpected type\n", stderr);
    abort();
}
static void ls_value_retain(ls_value value) {
    if (value.tag == LS_OBJECT || value.tag == LS_ARRAY || value.tag == LS_SYMBOL) ls_native_retain(value.as.o);
    else if (value.tag == LS_CALLABLE) ls_native_retain(value.as.c.environment);
}
static void ls_value_release(ls_value value) {
    if (value.tag == LS_OBJECT || value.tag == LS_ARRAY || value.tag == LS_SYMBOL) ls_native_release(value.as.o);
    else if (value.tag == LS_CALLABLE) ls_native_release(value.as.c.environment);
}
static void ls_value_copy(ls_value *slot, ls_value value) { ls_value_retain(value); ls_value_release(*slot); *slot = value; }
static void ls_value_take(ls_value *slot, ls_value value) { ls_value_release(*slot); *slot = value; }
static void ls_value_clear(ls_value *slot) { ls_value_release(*slot); *slot = (ls_value){0}; }
static ls_value ls_value_int(int32_t value) { ls_value result = {LS_INT}; result.as.i = value; return result; }
static ls_value ls_value_float(double value) { ls_value result = {LS_FLOAT}; result.as.f = value; return result; }
static ls_value ls_value_bool(bool value) { ls_value result = {LS_BOOL}; result.as.b = value; return result; }
static ls_value ls_value_string(ls_string value) { ls_value result = {LS_STRING}; result.as.s = value; return result; }
static ls_value ls_value_object(ls_native_object *value) { ls_value result = {LS_OBJECT}; result.as.o = value; return result; }
static ls_value ls_value_array(ls_native_object *value) { ls_value result = {LS_ARRAY}; result.as.o = value; return result; }
static ls_value ls_value_symbol(ls_native_object *value) { ls_value result = {LS_SYMBOL}; result.as.o = value; return result; }
static int32_t ls_value_to_int(ls_value value) { if (value.tag != LS_INT) ls_value_mismatch(); return value.as.i; }
static double ls_value_to_number(ls_value value) {
    if (value.tag == LS_INT) return (double)value.as.i;
    if (value.tag != LS_FLOAT) ls_value_mismatch();
    return value.as.f;
}
static bool ls_value_to_bool(ls_value value) { if (value.tag != LS_BOOL) ls_value_mismatch(); return value.as.b; }
static ls_string ls_value_to_string(ls_value value) { if (value.tag != LS_STRING) ls_value_mismatch(); return value.as.s; }
/* A reference slot of a class instance holds null until `init` stores it,
   as in JavaScript: null unboxes to the empty slot there. */
static ls_native_object *ls_value_to_object(ls_value value) {
    if (value.tag == LS_NULL) return NULL;
    if (value.tag != LS_OBJECT) ls_value_mismatch();
    return value.as.o;
}
static ls_native_object *ls_value_to_array(ls_value value) {
    if (value.tag == LS_NULL) return NULL;
    if (value.tag != LS_ARRAY) ls_value_mismatch();
    return value.as.o;
}
static ls_native_object *ls_value_to_symbol(ls_value value) { if (value.tag != LS_SYMBOL) ls_value_mismatch(); return value.as.o; }
static bool ls_value_number(ls_value value) { return value.tag == LS_INT || value.tag == LS_FLOAT; }
/* JavaScript strict equality: numbers by value, strings by code units,
   everything else by identity. */
static bool ls_value_equal(ls_value left, ls_value right) {
    if (ls_value_number(left) && ls_value_number(right)) return ls_value_to_number(left) == ls_value_to_number(right);
    if (left.tag != right.tag) return false;
    switch (left.tag) {
    case LS_NULL: return true;
    case LS_BOOL: return left.as.b == right.as.b;
    case LS_STRING: return ls_string_equal(left.as.s, right.as.s);
    case LS_OBJECT: case LS_ARRAY: case LS_SYMBOL: return left.as.o == right.as.o;
    case LS_CALLABLE: return left.as.c.identity == right.as.c.identity;
    default: return false;
    }
}
/* SameValueZero, as `includes` compares: NaN finds NaN. */
static bool ls_value_same_zero(ls_value left, ls_value right) {
    if (ls_value_number(left) && ls_value_number(right)) {
        double x = ls_value_to_number(left), y = ls_value_to_number(right);
        return x == y || (x != x && y != y);
    }
    return ls_value_equal(left, right);
}
static void ls_print_value(ls_value value) {
    switch (value.tag) {
    case LS_NULL: puts("null"); break;
    case LS_INT: printf("%ld\n", (long)value.as.i); break;
    case LS_FLOAT: ls_print_number(value.as.f); break;
    case LS_BOOL: puts(value.as.b ? "true" : "false"); break;
    case LS_STRING: ls_print_string(value.as.s); break;
    default:
        fputs("LilScript native cannot print this value\n", stderr);
        abort();
    }
}
"#;

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
static ls_value ls_value_callable{index}(ls_callable{index} value) {{ ls_value result = {{LS_CALLABLE, {index}}}; memcpy(&result.as.c, &value, sizeof value); return result; }}\n\
static ls_callable{index} ls_value_to_callable{index}(ls_value value) {{\n\
if (value.tag == LS_NULL) return (ls_callable{index}){{0}};\n\
if (value.tag != LS_CALLABLE || value.signature != {index}) ls_value_mismatch();\n\
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
            (Dynamic(_), Dynamic(_)) | (Object(_), Object(_)) => none(),
            (I32, Dynamic(_)) => ("ls_value_int(".into(), ")"),
            (F64, Dynamic(_)) => ("ls_value_float(".into(), ")"),
            (Bool, Dynamic(_)) => ("ls_value_bool(".into(), ")"),
            (String, Dynamic(_)) => ("ls_value_string(".into(), ")"),
            (Object(_) | Map | Set | Buffer | Typed(_), Dynamic(_)) => {
                ("ls_value_object(".into(), ")")
            }
            (Symbol, Dynamic(_)) => ("ls_value_symbol(".into(), ")"),
            (Array(_), Dynamic(_)) => ("ls_value_array((ls_native_object *)".into(), ")"),
            (Callable(signature), Dynamic(_)) => (format!("ls_value_callable{signature}("), ")"),
            (Dynamic(_), I32) => ("ls_value_to_int(".into(), ")"),
            (Dynamic(_), F64) => ("ls_value_to_number(".into(), ")"),
            (Dynamic(_), Bool) => ("ls_value_to_bool(".into(), ")"),
            (Dynamic(_), String) => ("ls_value_to_string(".into(), ")"),
            (Dynamic(_), Object(_) | Map | Set | Buffer | Typed(_)) => {
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
static void {name}_destroy(ls_native_object *owner) {{ ls_callable{from}_clear(&(({name} *)owner)->inner); }}\n\
static {} {name}_code(void *environment",
                self.plan.signatures[to].result
            ))?;
            self.signature_parameters(to, true, true)?;
            self.write(format_args!(") {{\n{name} *adapter = environment;\n"))?;
            let (inner, outer) = (
                self.plan.signatures[from].result,
                self.plan.signatures[to].result,
            );
            let (prefix, suffix) = Self::conversion(inner, outer);
            if outer != NativeType::Void {
                self.text("return ")?;
            }
            self.text(&prefix)?;
            self.text("adapter->inner.code(adapter->inner.environment")?;
            for position in 0..self.plan.signatures[to].parameters.len() {
                let (prefix, suffix) = Self::conversion(
                    self.plan.signatures[to].parameters[position],
                    self.plan.signatures[from].parameters[position],
                );
                self.write(format_args!(",{prefix}ls_p{position}{suffix}"))?;
            }
            self.write(format_args!(
                "){suffix};\n}}\n\
static ls_callable{to} ls_adapt{from}_{to}(ls_callable{from} inner) {{\n\
{name} *adapter = ls_native_allocate(sizeof *adapter, {name}_destroy);\n\
ls_native_retain(inner.environment);\n\
adapter->inner = inner;\n\
return (ls_callable{to}){{{name}_code, adapter, inner.identity}};\n}}\n\
static ls_value ls_adapt_value{from}_{to}(ls_value value) {{\n\
if (value.tag != LS_CALLABLE) {{ ls_value_retain(value); return value; }}\n\
return ls_value_callable{to}(ls_adapt{from}_{to}(ls_value_to_callable{from}(value)));\n}}\n"
            ))?;
        }
        Ok(())
    }
}
