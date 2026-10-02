//! Typed views of the shared native array buffer. Descriptor identity proves
//! the direct access fast path; other views use checked tagged conversion.
use super::*;

impl Emitter<'_, '_, '_, '_, '_> {
    pub(super) fn array_declarations(&mut self) -> Result<(), NativeError> {
        for index in 0..self.plan.arrays.len() {
            self.budget.work(WorkKind::Render, 1)?;
            self.write(format_args!("typedef struct ls_native_array ls_array{index};\n"))?;
        }
        Ok(())
    }
    pub(super) fn array_types(&mut self) -> Result<(), NativeError> {
        for (s, e) in self.plan.arrays.iter().copied().enumerate() {
            self.budget.work(WorkKind::Render, 1)?;
            let retain=e.retain("value").unwrap_or_default();
            let drop=e.release("value").unwrap_or_default();
            let trace=e.trace("value").unwrap_or_default();
            let (boxed, boxed_end)=Self::conversion(e,NativeType::Dynamic(Tagged::ANY));
            let boxed=if let NativeType::Struct(index)=e { format!("ls_t{index}_box(temps,") } else { boxed };
            let (unbox, unbox_end)=Self::conversion(NativeType::Dynamic(Tagged::ANY),e);
            let absent=match e {
                NativeType::I32=>"return 0;".to_owned(),
                NativeType::String|NativeType::Dynamic(_)=>format!("return ({e}){{0}};"),
                _=>format!("ls_native_undefined_element(); return ({e}){{0}};"),
            };
            self.write(format_args!(r#"
static LS_NATIVE_UNUSED void ls_array{s}_acquire({e} value) {{ (void)value; {retain} }}
static LS_NATIVE_UNUSED void ls_array{s}_drop({e} value) {{ (void)value; {drop} }}
static LS_NATIVE_UNUSED void ls_array{s}_retain_slot(const void *slot) {{ {e} value=*({e} const *)slot; ls_array{s}_acquire(value); }}
static LS_NATIVE_UNUSED void ls_array{s}_drop_slot(void *slot) {{ {e} value=*({e} *)slot; ls_array{s}_drop(value); }}
static LS_NATIVE_UNUSED void ls_array{s}_trace_slot(const void *slot, ls_native_visit visit, void *context) {{
{e} value=*({e} const *)slot; (void)value; (void)visit; (void)context; {trace}
}}
static LS_NATIVE_UNUSED ls_value ls_array{s}_read_slot(ls_native_temporary **temps, const void *slot) {{
(void)temps; {e} value=*({e} const *)slot; return {boxed}value{boxed_end};
}}
static LS_NATIVE_UNUSED void ls_array{s}_write_slot(void *slot, ls_value input) {{
{e} value={unbox}input{unbox_end}; ls_array{s}_acquire(value); ls_array{s}_drop(*({e} *)slot); *({e} *)slot=value;
}}
static const ls_array_ops ls_array{s}_ops={{sizeof({e}),ls_array{s}_retain_slot,ls_array{s}_drop_slot,ls_array{s}_trace_slot,ls_array{s}_read_slot,ls_array{s}_write_slot}};
static LS_NATIVE_UNUSED ls_array{s} *ls_array{s}_new(size_t capacity) {{ return ls_array_new(&ls_array{s}_ops,capacity); }}
static LS_NATIVE_UNUSED void ls_array{s}_hole(ls_array{s} *array) {{ ls_array_hole(array); }}
static LS_NATIVE_UNUSED {e} ls_array{s}_absent(void) {{ {absent} }}
static LS_NATIVE_UNUSED {e} ls_array{s}_get(ls_array{s} *array, int32_t index, ls_native_temporary **temps) {{
if(index<0 || (size_t)index>=array->length) ls_native_undefined_element();
if(!ls_array_has(array,(size_t)index)) return ls_array{s}_absent();
if(array->ops==&ls_array{s}_ops) return (({e} *)array->items)[index];
return {unbox}ls_array_read(array,(size_t)index,temps){unbox_end};
}}
static LS_NATIVE_UNUSED ls_value ls_array{s}_optional(ls_array{s} *array, int32_t index, ls_native_temporary **temps) {{
if(index<0 || (size_t)index>=array->length) return (ls_value){{0}};
return ls_array_read(array,(size_t)index,temps);
}}
static LS_NATIVE_UNUSED int32_t ls_array{s}_push_owned(ls_array{s} *array, {e} value) {{
if(array->ops==&ls_array{s}_ops) {{ *({e} *)ls_array_append_slot(array)=value; }}
else {{
ls_native_temporary *ls_temps=NULL;
ls_value boxed=ls_array{s}_read_slot(&ls_temps,&value);
ls_array_write(array,(int32_t)array->length,boxed); ls_array{s}_drop(value);
ls_native_temporaries_clear(&ls_temps);
}}
return (int32_t)array->length;
}}
static LS_NATIVE_UNUSED int32_t ls_array{s}_push(ls_array{s} *array, {e} value) {{
ls_array{s}_acquire(value); return ls_array{s}_push_owned(array,value);
}}
static LS_NATIVE_UNUSED void ls_array{s}_set(ls_array{s} *array, int32_t index, {e} value) {{
if(array->ops==&ls_array{s}_ops) {{
ls_array{s}_acquire(value); {e} *slot=ls_array_store_slot(array,index); ls_array{s}_drop(*slot); *slot=value;
}} else {{
ls_native_temporary *ls_temps=NULL; ls_value boxed=ls_array{s}_read_slot(&ls_temps,&value);
ls_array_write(array,index,boxed); ls_native_temporaries_clear(&ls_temps);
}}
}}
static LS_NATIVE_UNUSED {e} ls_array{s}_pop(ls_array{s} *array) {{
if(!array->length) return ls_array{s}_absent();
size_t index=array->length-1;
if(!ls_array_has(array,index)) {{ --array->length; return ls_array{s}_absent(); }}
if(array->ops==&ls_array{s}_ops) {{ {e} result=(({e} *)array->items)[index]; --array->length; return result; }}
ls_native_temporary *ls_temps=NULL;
{e} result=ls_array{s}_get(array,(int32_t)index,&ls_temps); ls_array{s}_acquire(result);
ls_array_drop_last(array); ls_native_temporaries_clear(&ls_temps); return result;
}}
static LS_NATIVE_UNUSED ls_array{s} *ls_array{s}_slice(ls_array{s} *array, size_t start, size_t end) {{ return ls_array_slice(array,start,end); }}
static LS_NATIVE_UNUSED ls_array{s} *ls_array{s}_concat(ls_array{s} *left, ls_array{s} *right) {{ return ls_array_concat(left,right); }}
static LS_NATIVE_UNUSED ls_array{s} *ls_array{s}_reverse(ls_array{s} *array) {{ return ls_array_reverse(array); }}
static LS_NATIVE_UNUSED ls_array{s} *ls_array{s}_splice(ls_array{s} *array, int32_t start, int32_t count) {{ return ls_array_splice(array,start,count); }}
static LS_NATIVE_UNUSED ls_array{s} *ls_array{s}_copy_within(ls_array{s} *array, int32_t target, int32_t start, bool bounded, int32_t end) {{ return ls_array_copy_within(array,target,start,bounded,end); }}
static LS_NATIVE_UNUSED ls_array{s} *ls_array{s}_fill(ls_array{s} *array, {e} value) {{
for(size_t i=0;i<array->length;++i) {{ ls_array{s}_set(array,(int32_t)i,value); }}
return array;
}}
static LS_NATIVE_UNUSED void ls_array{s}_copy(ls_array{s} **slot, ls_array{s} *value) {{ ls_native_retain(value); ls_native_release(*slot); *slot=value; }}
static LS_NATIVE_UNUSED void ls_array{s}_take(ls_array{s} **slot, ls_array{s} *value) {{ ls_native_release(*slot); *slot=value; }}
static LS_NATIVE_UNUSED void ls_array{s}_clear(ls_array{s} **slot) {{ ls_native_release(*slot); *slot=NULL; }}
"#))?;
            let (strict,zero)=match e {
                NativeType::F64=>("left==right","left==right || (left!=left && right!=right)"),
                NativeType::String=>("ls_string_equal(left,right)","ls_string_equal(left,right)"),
                NativeType::Callable(_)=>("left.identity==right.identity","left.identity==right.identity"),
                NativeType::Dynamic(_)=>("ls_value_equal(left,right)","ls_value_same_zero(left,right)"),
                NativeType::Struct(_)=>continue,
                _=>("left==right","left==right"),
            };
            self.write(format_args!(r#"
static LS_NATIVE_UNUSED int32_t ls_array{s}_index_of(ls_array{s} *array, {e} right) {{
ls_native_temporary *ls_temps=NULL;
for(size_t i=0;i<array->length;++i) if(ls_array_has(array,i)) {{
{e} left=ls_array{s}_get(array,(int32_t)i,&ls_temps); bool same=({strict});
ls_native_temporaries_clear(&ls_temps); if(same) return (int32_t)i;
}}
return -1;
}}
static LS_NATIVE_UNUSED bool ls_array{s}_includes(ls_array{s} *array, {e} right, int32_t start) {{
size_t from=start>=0 ? (size_t)start : ls_array_relative(start,array->length);
ls_native_temporary *ls_temps=NULL;
for(size_t i=from;i<array->length;++i) if(ls_array_has(array,i)) {{
{e} left=ls_array{s}_get(array,(int32_t)i,&ls_temps); bool same=({zero});
ls_native_temporaries_clear(&ls_temps); if(same) return true;
}}
return false;
}}
"#))?;
        }
        Ok(())
    }
}
