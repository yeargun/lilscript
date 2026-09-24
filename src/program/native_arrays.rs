//! Native arrays: one reference-counted, growable C array per element kind.
//! An element is plain value storage, so element copies carry no ownership.
//! Reads and pops past the end follow the JavaScript recipes: an `int`
//! reads 0 (`a[i]|0`) and a `string` reads `""` (`a[i]??""`); every other
//! element would be `undefined`, which native values cannot hold, so it
//! stops the program. Setting one past the end appends; further would leave
//! holes, which native arrays do not have.
//!
//! Methods follow ECMA-262 step by step where it is observable: callbacks
//! see the length fixed when the method starts, and indices removed by a
//! callback are skipped, except by `findIndex`, which reads them.
use super::*;
use crate::primitive::Intrinsic;

impl Emitter<'_, '_, '_, '_, '_> {
    /// Arrays may hold each other and appear in callable signatures, so
    /// every array type is declared before any is defined.
    pub(super) fn array_declarations(&mut self) -> Result<(), NativeError> {
        for index in 0..self.plan.arrays.len() {
            self.budget.work(WorkKind::Render, 1)?;
            self.write(format_args!(
                "typedef struct ls_array{index} ls_array{index};\n"
            ))?;
        }
        Ok(())
    }

    pub(super) fn array_types(&mut self) -> Result<(), NativeError> {
        if self.plan.arrays.is_empty() {
            return Ok(());
        }
        self.text(
            "static void ls_native_undefined_element(void) {\n\
             fputs(\"LilScript native array element is undefined\\n\", stderr);\n\
             abort();\n}\n\
             static size_t ls_array_relative(int32_t index, size_t length) {\n\
             if (index < 0) return (size_t)-(int64_t)index >= length ? 0 : length - (size_t)-(int64_t)index;\n\
             return (size_t)index < length ? (size_t)index : length;\n}\n",
        )?;
        for (s, element) in self.plan.arrays.clone().into_iter().enumerate() {
            self.budget.work(WorkKind::Render, 1)?;
            let e = element;
            // An element that owns: stores acquire, overwrites and the
            // array's destruction release. Reads borrow; `pop` hands its
            // owner to the caller.
            let acquire = element.retain("value").unwrap_or_default();
            let drop = element.release("value").unwrap_or_default();
            let absent = match element {
                NativeType::I32 => "return 0;".to_owned(),
                NativeType::String => "return (ls_string){NULL,0};".to_owned(),
                NativeType::Dynamic(_) => "return (ls_value){0};".to_owned(),
                _ => "ls_native_undefined_element(); return array->items[0];".to_owned(),
            };
            let hole = match element {
                NativeType::I32 => format!("ls_array{s}_push(array, 0);"),
                NativeType::String => format!("ls_array{s}_push(array, (ls_string){{NULL,0}});"),
                NativeType::Dynamic(_) => format!("ls_array{s}_push(array, (ls_value){{0}});"),
                _ => "(void)array; ls_native_undefined_element();".to_owned(),
            };
            self.write(format_args!(
                "struct ls_array{s} {{ ls_native_object owner; size_t length; size_t capacity; {e} *items; }};\n\
static void ls_array{s}_acquire({e} value) {{ (void)value; {acquire}}}\n\
static void ls_array{s}_drop({e} value) {{ (void)value; {drop}}}\n\
static void ls_array{s}_destroy(ls_native_object *owner) {{\n\
ls_array{s} *array = (ls_array{s} *)owner;\n\
for (size_t index = 0; index < array->length; index++) ls_array{s}_drop(array->items[index]);\n\
free(array->items);\n}}\n\
static ls_array{s} *ls_array{s}_new(size_t capacity) {{\n\
ls_array{s} *array = ls_native_allocate(sizeof *array, ls_array{s}_destroy);\n\
array->length = 0; array->capacity = 0; array->items = NULL;\n\
if (capacity) {{\n\
if (capacity > SIZE_MAX / sizeof *array->items) ls_native_resource_failure();\n\
array->items = malloc(capacity * sizeof *array->items);\n\
if (!array->items) ls_native_resource_failure();\n\
array->capacity = capacity;\n}}\n\
return array;\n}}\n\
static void ls_array{s}_reserve(ls_array{s} *array, size_t length) {{\n\
if (length <= array->capacity) return;\n\
size_t capacity = array->capacity ? array->capacity : 4;\n\
while (capacity < length) {{ if (capacity > SIZE_MAX / 2 / sizeof *array->items) ls_native_resource_failure(); capacity *= 2; }}\n\
{e} *items = realloc(array->items, capacity * sizeof *items);\n\
if (!items) ls_native_resource_failure();\n\
array->items = items; array->capacity = capacity;\n}}\n\
static int32_t ls_array{s}_push_owned(ls_array{s} *array, {e} value) {{\n\
if (array->length >= (size_t)INT32_MAX) ls_native_resource_failure();\n\
ls_array{s}_reserve(array, array->length + 1);\n\
array->items[array->length++] = value;\n\
return (int32_t)array->length;\n}}\n\
static int32_t ls_array{s}_push(ls_array{s} *array, {e} value) {{\n\
ls_array{s}_acquire(value);\n\
return ls_array{s}_push_owned(array, value);\n}}\n\
static void ls_array{s}_hole(ls_array{s} *array) {{ {hole} }}\n\
static {e} ls_array{s}_get(ls_array{s} *array, int32_t index) {{\n\
if (index < 0 || (size_t)index >= array->length) {{ {absent} }}\n\
return array->items[index];\n}}\n\
static void ls_array{s}_set(ls_array{s} *array, int32_t index, {e} value) {{\n\
if (index >= 0 && (size_t)index < array->length) {{ ls_array{s}_acquire(value); ls_array{s}_drop(array->items[index]); array->items[index] = value; return; }}\n\
if (index >= 0 && (size_t)index == array->length) {{ ls_array{s}_push(array, value); return; }}\n\
ls_native_undefined_element();\n}}\n\
static {e} ls_array{s}_pop(ls_array{s} *array) {{\n\
if (!array->length) {{ {absent} }}\n\
return array->items[--array->length];\n}}\n\
static ls_array{s} *ls_array{s}_slice(ls_array{s} *array, size_t start, size_t end) {{\n\
ls_array{s} *result = ls_array{s}_new(end > start ? end - start : 0);\n\
for (size_t index = start; index < end && index < array->length; index++) ls_array{s}_push(result, array->items[index]);\n\
return result;\n}}\n\
static ls_array{s} *ls_array{s}_concat(ls_array{s} *left, ls_array{s} *right) {{\n\
size_t left_length = left->length, right_length = right->length;\n\
ls_array{s} *result = ls_array{s}_new(left_length + right_length);\n\
for (size_t index = 0; index < left_length; index++) ls_array{s}_push(result, left->items[index]);\n\
for (size_t index = 0; index < right_length; index++) ls_array{s}_push(result, right->items[index]);\n\
return result;\n}}\n\
static ls_array{s} *ls_array{s}_reverse(ls_array{s} *array) {{\n\
for (size_t low = 0, high = array->length; low + 1 < high; low++, high--) {{ {e} value = array->items[low]; array->items[low] = array->items[high - 1]; array->items[high - 1] = value; }}\n\
return array;\n}}\n\
static ls_array{s} *ls_array{s}_fill(ls_array{s} *array, {e} value) {{\n\
for (size_t index = 0; index < array->length; index++) {{ ls_array{s}_acquire(value); ls_array{s}_drop(array->items[index]); array->items[index] = value; }}\n\
return array;\n}}\n\
static ls_array{s} *ls_array{s}_splice(ls_array{s} *array, int32_t start, int32_t count) {{\n\
size_t from = ls_array_relative(start, array->length);\n\
size_t removed = count <= 0 ? 0 : (size_t)count;\n\
if (removed > array->length - from) removed = array->length - from;\n\
ls_array{s} *result = ls_array{s}_new(removed);\n\
for (size_t index = 0; index < removed; index++) ls_array{s}_push_owned(result, array->items[from + index]);\n\
memmove(array->items + from, array->items + from + removed, (array->length - from - removed) * sizeof *array->items);\n\
array->length -= removed;\n\
return result;\n}}\n\
static ls_array{s} *ls_array{s}_copy_within(ls_array{s} *array, int32_t target, int32_t start, bool bounded, int32_t end) {{\n\
size_t length = array->length;\n\
size_t to = ls_array_relative(target, length), from = ls_array_relative(start, length);\n\
size_t final = bounded ? ls_array_relative(end, length) : length;\n\
if (final > from) {{\n\
size_t count = final - from;\n\
if (count > length - to) count = length - to;\n\
for (size_t index = 0; index < count; index++) ls_array{s}_acquire(array->items[from + index]);\n\
for (size_t index = 0; index < count; index++) ls_array{s}_drop(array->items[to + index]);\n\
memmove(array->items + to, array->items + from, count * sizeof *array->items);\n}}\n\
return array;\n}}\n\
static void ls_array{s}_copy(ls_array{s} **slot, ls_array{s} *value) {{ ls_native_retain(value); ls_native_release(*slot); *slot = value; }}\n\
static void ls_array{s}_take(ls_array{s} **slot, ls_array{s} *value) {{ ls_native_release(*slot); *slot = value; }}\n\
static void ls_array{s}_clear(ls_array{s} **slot) {{ ls_native_release(*slot); *slot = NULL; }}\n"
            ))?;
            // An optional read, past the end null, boxes a present element.
            if self.plan.helpers.contains(Helper::Dynamic)
                && !matches!(element, NativeType::Struct(_) | NativeType::Dynamic(_))
            {
                let (prefix, suffix) = Self::conversion(element, NativeType::Dynamic(Tagged::ANY));
                self.write(format_args!(
                    "static ls_value ls_array{s}_optional(ls_array{s} *array, int32_t index) {{\n\
if (index < 0 || (size_t)index >= array->length) return (ls_value){{0}};\n\
return {prefix}array->items[index]{suffix};\n}}\n"
                ))?;
            }
            // Strict equality for `indexOf`; SameValueZero (NaN finds NaN)
            // for `includes`. Products never reach here.
            let (strict, zero) = match element {
                NativeType::I32
                | NativeType::Bool
                | NativeType::Array(_)
                | NativeType::Object(_) => ("left == right", "left == right"),
                NativeType::F64 => (
                    "left == right",
                    "left == right || (left != left && right != right)",
                ),
                NativeType::String => (
                    "ls_string_equal(left, right)",
                    "ls_string_equal(left, right)",
                ),
                NativeType::Callable(_) => (
                    "left.identity == right.identity",
                    "left.identity == right.identity",
                ),
                NativeType::Dynamic(_) => (
                    "ls_value_equal(left, right)",
                    "ls_value_same_zero(left, right)",
                ),
                _ => continue,
            };
            self.write(format_args!(
                "static int32_t ls_array{s}_index_of(ls_array{s} *array, {e} right) {{\n\
for (size_t index = 0; index < array->length; index++) {{ {e} left = array->items[index]; if ({strict}) return (int32_t)index; }}\n\
return -1;\n}}\n\
static bool ls_array{s}_includes(ls_array{s} *array, {e} right, int32_t start) {{\n\
size_t length = array->length;\n\
size_t from = start >= 0 ? (size_t)start : ls_array_relative(start, length);\n\
for (size_t index = from; index < length; index++) {{ {e} left = array->items[index]; if ({zero}) return true; }}\n\
return false;\n}}\n"
            ))?;
        }
        Ok(())
    }

    /// One call of a callback value with already-rendered C arguments.
    fn callback_call(
        &mut self,
        unit: UnitId,
        callback: ValueId,
        arguments: &[&str],
    ) -> Result<(), NativeError> {
        let storage = self.plan.units[unit.index()].values[callback.index()];
        let NativeType::Callable(signature) = self.plan.value_type(storage) else {
            unreachable!("native plan admits only callable callbacks")
        };
        let floating = self.plan.signatures[signature].result == NativeType::F64;
        if floating {
            self.text("ls_f64(")?;
        }
        let mut leading = false;
        match storage {
            ValueStorage::Function(body) => self.write(format_args!("ls_fn{}(", body.index()))?,
            ValueStorage::Host(binding) => self.write(format_args!(
                "{}(",
                self.plan.hosts.bindings[binding].link_name
            ))?,
            ValueStorage::Value(_) => {
                self.write(format_args!(
                    "ls_v{0}.code(ls_v{0}.environment",
                    callback.index()
                ))?;
                leading = true;
            }
        }
        for (index, argument) in arguments.iter().enumerate() {
            if index != 0 || leading {
                self.text(",")?;
            }
            self.text(argument)?;
        }
        self.text(")")?;
        if floating {
            self.text(")")?;
        }
        Ok(())
    }

    pub(super) fn array_method(
        &mut self,
        unit: UnitId,
        call: CallId,
        result: Option<ValueId>,
        receiver: ValueId,
        array: usize,
        method: Intrinsic,
    ) -> Result<(), NativeError> {
        let data = self.plan.program.unit(unit).unwrap();
        let arguments = data.arguments(data.calls[call.index()].arguments).unwrap();
        let value = |position: usize| match arguments.get(position) {
            Some(CallArgument::Value(value)) => Some(*value),
            _ => None,
        };
        let s = array;
        let e = self.plan.arrays[array];
        let r = receiver.index();
        // The receiver's value slot owns the array for the whole loop, so a
        // callback that drops every other reference cannot free it.
        let head = format!(
            "{{\nls_array{s} *ls_src = ls_v{r};\nsize_t ls_len = ls_src->length;\nfor (size_t ls_k = 0; ls_k < ls_len; ls_k++) {{\n"
        );
        let result_value = result.unwrap();
        let destination = Destination::Value(result_value);
        match method {
            Intrinsic::ArrayForEach => {
                self.text(&head)?;
                self.text("if (ls_k >= ls_src->length) continue;\n")?;
                self.callback_call(unit, value(0).unwrap(), &["ls_src->items[ls_k]"])?;
                self.text(";\n}\n}\n")
            }
            Intrinsic::ArrayMap => {
                let ValueStorage::Value(NativeType::Array(o)) =
                    self.plan.units[unit.index()].values[result_value.index()]
                else {
                    unreachable!("native map result is an array")
                };
                // The callback's result is a fresh owner: the new array takes it.
                self.write(format_args!(
                    "{{\nls_array{s} *ls_src = ls_v{r};\nsize_t ls_len = ls_src->length;\nls_array{o} *ls_out = ls_array{o}_new(ls_len);\nfor (size_t ls_k = 0; ls_k < ls_len; ls_k++) {{\nif (ls_k >= ls_src->length) {{ ls_array{o}_hole(ls_out); continue; }}\nls_array{o}_push_owned(ls_out,"
                ))?;
                self.callback_call(unit, value(0).unwrap(), &["ls_src->items[ls_k]"])?;
                self.text(");\n}\n")?;
                self.assignment_start(unit, destination, true)?;
                self.text("ls_out")?;
                self.assignment_end(unit, destination)?;
                self.text("}\n")
            }
            Intrinsic::ArrayFilter => {
                self.write(format_args!(
                    "{{\nls_array{s} *ls_src = ls_v{r};\nsize_t ls_len = ls_src->length;\nls_array{s} *ls_out = ls_array{s}_new(0);\nfor (size_t ls_k = 0; ls_k < ls_len; ls_k++) {{\nif (ls_k >= ls_src->length) continue;\n{e} ls_item = ls_src->items[ls_k];\nif ("
                ))?;
                self.callback_call(unit, value(0).unwrap(), &["ls_item"])?;
                self.write(format_args!(") ls_array{s}_push(ls_out, ls_item);\n}}\n"))?;
                self.assignment_start(unit, destination, true)?;
                self.text("ls_out")?;
                self.assignment_end(unit, destination)?;
                self.text("}\n")
            }
            Intrinsic::ArrayReduce => {
                let ValueStorage::Value(accumulator) =
                    self.plan.units[unit.index()].values[result_value.index()]
                else {
                    unreachable!("native reduce result is a value")
                };
                self.write(format_args!(
                    "{{\n{accumulator} ls_acc = ls_v{};\n",
                    value(1).unwrap().index()
                ))?;
                self.text(&head)?;
                self.text("if (ls_k >= ls_src->length) continue;\nls_acc = ")?;
                self.callback_call(unit, value(0).unwrap(), &["ls_acc", "ls_src->items[ls_k]"])?;
                self.write(format_args!(
                    ";\n}}\n}}\nls_v{} = ls_acc;\n}}\n",
                    result_value.index()
                ))
            }
            Intrinsic::ArraySome | Intrinsic::ArrayEvery => {
                let some = method == Intrinsic::ArraySome;
                self.write(format_args!(
                    "{{\nbool ls_found = {};\n",
                    if some { "false" } else { "true" }
                ))?;
                self.text(&head)?;
                self.write(format_args!(
                    "if (ls_k >= ls_src->length) continue;\nif ({}",
                    if some { "" } else { "!" }
                ))?;
                self.callback_call(unit, value(0).unwrap(), &["ls_src->items[ls_k]"])?;
                self.write(format_args!(
                    ") {{ ls_found = {}; break; }}\n}}\n}}\nls_v{} = ls_found;\n}}\n",
                    if some { "true" } else { "false" },
                    result_value.index()
                ))
            }
            Intrinsic::ArrayFindIndex => {
                self.text("{\nint32_t ls_found = -1;\n")?;
                self.text(&head)?;
                self.text("if (")?;
                self.callback_call(
                    unit,
                    value(0).unwrap(),
                    &[&format!("ls_array{s}_get(ls_src, (int32_t)ls_k)")],
                )?;
                self.write(format_args!(
                    ") {{ ls_found = (int32_t)ls_k; break; }}\n}}\n}}\nls_v{} = ls_found;\n}}\n",
                    result_value.index()
                ))
            }
            Intrinsic::ArrayIndexOf => self.write(format_args!(
                "ls_v{} = ls_array{s}_index_of(ls_v{r}, ls_v{});\n",
                result_value.index(),
                value(0).unwrap().index()
            )),
            Intrinsic::ArrayIncludes => {
                let start =
                    value(1).map_or("0".to_owned(), |start| format!("ls_v{}", start.index()));
                self.write(format_args!(
                    "ls_v{} = ls_array{s}_includes(ls_v{r}, ls_v{}, {start});\n",
                    result_value.index(),
                    value(0).unwrap().index()
                ))
            }
            Intrinsic::ArrayConcat | Intrinsic::ArraySlice | Intrinsic::ArraySplice => {
                let expression = match method {
                    Intrinsic::ArrayConcat => format!(
                        "ls_array{s}_concat(ls_v{r}, ls_v{})",
                        value(0).unwrap().index()
                    ),
                    Intrinsic::ArraySplice => format!(
                        "ls_array{s}_splice(ls_v{r}, ls_v{}, ls_v{})",
                        value(0).unwrap().index(),
                        value(1).unwrap().index()
                    ),
                    _ => {
                        let start = value(0).map_or("0".to_owned(), |start| {
                            format!("ls_array_relative(ls_v{}, ls_v{r}->length)", start.index())
                        });
                        let end = value(1).map_or(format!("ls_v{r}->length"), |end| {
                            format!("ls_array_relative(ls_v{}, ls_v{r}->length)", end.index())
                        });
                        format!("ls_array{s}_slice(ls_v{r}, {start}, {end})")
                    }
                };
                self.assignment_start(unit, destination, true)?;
                self.text(&expression)?;
                self.assignment_end(unit, destination)
            }
            Intrinsic::ArrayReverse | Intrinsic::ArrayFill | Intrinsic::ArrayCopyWithin => {
                // These return their receiver: the result is one more owner.
                let expression = match method {
                    Intrinsic::ArrayReverse => format!("ls_array{s}_reverse(ls_v{r})"),
                    Intrinsic::ArrayFill => {
                        format!(
                            "ls_array{s}_fill(ls_v{r}, ls_v{})",
                            value(0).unwrap().index()
                        )
                    }
                    _ => format!(
                        "ls_array{s}_copy_within(ls_v{r}, ls_v{}, ls_v{}, {}, {})",
                        value(0).unwrap().index(),
                        value(1).unwrap().index(),
                        if value(2).is_some() { "true" } else { "false" },
                        value(2).map_or("0".to_owned(), |end| format!("ls_v{}", end.index()))
                    ),
                };
                self.assignment_start(unit, destination, false)?;
                self.text(&expression)?;
                self.assignment_end(unit, destination)
            }
            _ => unreachable!("native plan admits only these array methods"),
        }
    }
}
