use std::fmt::Write;

use crate::stable_hash::{StableHashMap as AHashMap, StableHashSet as AHashSet};

use crate::codegen_js::{CodegenError, CompileError};
use crate::ir::{
    ArrayOperand, BlockId, ConstValue, ControlFlowFunction, ControlFlowInstruction,
    ControlFlowModule, ControlFlowOp, FunctionId, FunctionKind, Intrinsic, IrBinaryOp, IrUnaryOp,
    LocalId, RecordOperand, TemplateOperand, Terminator, ValueId,
};
use crate::lower::lower_to_control_flow;
use crate::optimizer::optimize_control_flow;
use crate::semantic::{analyze, EscapeState, Type};
use crate::typed_array::{classify_typed_array_intrinsic, TypedArrayIntrinsic, TypedArrayKind};
use crate::{ast::Program, span::Span};

pub fn compile_to_c<'ast, 'src>(program: &Program<'ast, 'src>) -> Result<String, CompileError> {
    let semantics = analyze(program)?;
    let mut ir = lower_to_control_flow(program, &semantics)?;
    optimize_control_flow(&mut ir)?;
    emit_native_c(&ir).map_err(Into::into)
}

pub fn emit_native_c(module: &ControlFlowModule<'_>) -> Result<String, CodegenError> {
    emit_native_c_with_options(module, &NativeOptions::default())
}

pub fn emit_native_c_with_options(
    module: &ControlFlowModule<'_>,
    options: &NativeOptions,
) -> Result<String, CodegenError> {
    NativeEmitter {
        module,
        options: *options,
    }
    .emit()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeOptions {
    pub partial_escape_analysis: bool,
    pub stack_allocation: bool,
    pub region_allocation: bool,
    pub stack_array_element_limit: usize,
}

impl Default for NativeOptions {
    fn default() -> Self {
        Self {
            partial_escape_analysis: true,
            stack_allocation: true,
            region_allocation: true,
            stack_array_element_limit: 64,
        }
    }
}

struct NativeEmitter<'module, 'src> {
    module: &'module ControlFlowModule<'src>,
    options: NativeOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeStorage<'src> {
    StackArray(usize),
    RegionArray(usize),
    StackClass(&'src str),
    RegionClass,
    StackClosure(FunctionId),
    RegionClosure(FunctionId),
}

impl<'module, 'src> NativeEmitter<'module, 'src> {
    fn emit(&self) -> Result<String, CodegenError> {
        self.validate_host_boundaries()?;
        let mut out = String::from(
            "#include <stdbool.h>\n#include <ctype.h>\n#include <math.h>\n#include <stddef.h>\n#include <stdint.h>\n#include <stdio.h>\n#include <stdlib.h>\n#include <string.h>\n",
        );
        out.push_str(
            "static inline int32_t lilscript_idiv(int32_t a,int32_t b){if(!b)return 0;return a==INT32_MIN&&b==-1?INT32_MIN:a/b;}\n",
        );
        out.push_str(
            "static inline int32_t lilscript_irem(int32_t a,int32_t b){if(!b)return 0;return a==INT32_MIN&&b==-1?0:a%b;}\n",
        );
        out.push_str(
            "static inline int32_t lilscript_mul(int32_t a,int32_t b){int64_t n=(int64_t)((double)a*(double)b);uint32_t u=(uint32_t)(uint64_t)n;return u<=INT32_MAX?(int32_t)u:(int32_t)((int64_t)u-4294967296LL);}\n",
        );
        out.push_str(
            "static inline int32_t lilscript_from_u32(uint32_t u){return u<=INT32_MAX?(int32_t)u:(int32_t)((int64_t)u-4294967296LL);}static inline int32_t lilscript_shl(int32_t a,int32_t b){return lilscript_from_u32((uint32_t)a<<((uint32_t)b&31));}static inline int32_t lilscript_shr(int32_t a,int32_t b){uint32_t n=(uint32_t)b&31;if(!n)return a;uint32_t r=(uint32_t)a>>n;if(a<0)r|=UINT32_MAX<<(32-n);return lilscript_from_u32(r);}static inline int32_t lilscript_ushr(int32_t a,int32_t b){return lilscript_from_u32((uint32_t)a>>((uint32_t)b&31));}\n",
        );
        out.push_str(
            "static inline double lilscript_fmin(double a,double b){if(isnan(a)||isnan(b))return NAN;if(a==0&&b==0)return signbit(a)||signbit(b)?-0.0:0.0;return a<b?a:b;}static inline double lilscript_fmax(double a,double b){if(isnan(a)||isnan(b))return NAN;if(a==0&&b==0)return signbit(a)&&signbit(b)?-0.0:0.0;return a>b?a:b;}\n",
        );
        out.push_str(
            "static inline double lilscript_round(double x){if(signbit(x)&&x>=-0.5)return-0.0;return floor(x+0.5);}\n",
        );
        out.push_str(
            "static inline int32_t lilscript_to_i32(double x){if(!isfinite(x)||x==0)return 0;double n=fmod(trunc(x),4294967296.0);if(n<0)n+=4294967296.0;return n>=2147483648.0?(int32_t)(n-4294967296.0):(int32_t)n;}\n",
        );
        out.push_str(
            "typedef struct LilScriptArrayHeader{void*data;int32_t len,cap;}*LilScriptArray;typedef struct{void*fn;void*env;}LilScriptClosure;typedef char* LilScriptString;typedef struct{uint8_t tag;union{int32_t i;double f;bool b;const char*s;void*p;LilScriptClosure c;};}LilScriptValue;typedef struct{bool has;LilScriptValue value;}LilScriptOptional;typedef struct LilScriptMapHeader{LilScriptValue*keys,*values;int32_t len,cap;}*LilScriptMap;typedef struct LilScriptSetHeader{LilScriptValue*values;int32_t len,cap;}*LilScriptSet;typedef struct LilScriptBufferHeader{uint8_t*data;int32_t len;bool shared;}*LilScriptBuffer;typedef struct LilScriptTypedArrayHeader{LilScriptBuffer buffer;int32_t offset,len;}*LilScriptTypedArray;typedef LilScriptTypedArray LilScriptInt8Array;typedef LilScriptTypedArray LilScriptUint8Array;typedef LilScriptTypedArray LilScriptUint8ClampedArray;typedef LilScriptTypedArray LilScriptInt16Array;typedef LilScriptTypedArray LilScriptUint16Array;typedef LilScriptTypedArray LilScriptInt32Array;typedef LilScriptTypedArray LilScriptUint32Array;typedef LilScriptTypedArray LilScriptFloat32Array;typedef LilScriptTypedArray LilScriptFloat64Array;\n",
        );
        out.push_str(
            "typedef struct LilScriptSymbolHeader{int64_t id;char*description;}*LilScriptSymbol;static int64_t lilscript_symbol_counter;\n",
        );
        out.push_str(
            "static inline LilScriptSymbol lilscript_symbol(const char*desc){LilScriptSymbol s=malloc(sizeof*s);if(!s)abort();s->id=++lilscript_symbol_counter;s->description=desc?strdup(desc):NULL;return s;}\n",
        );
        out.push_str(
            "typedef struct LilScriptRegionChunk{struct LilScriptRegionChunk*next;size_t used,cap;max_align_t align;unsigned char data[];}LilScriptRegionChunk;typedef struct{LilScriptRegionChunk*head;}LilScriptRegion;static inline void*lilscript_region_alloc(LilScriptRegion*r,size_t n){size_t a=_Alignof(max_align_t),p=r->head?(r->head->used+a-1)&~(a-1):0;if(!r->head||p+n>r->head->cap){size_t c=n+a>4096?n+a:4096;LilScriptRegionChunk*x=malloc(sizeof*x+c);if(!x)abort();x->next=r->head;x->used=0;x->cap=c;r->head=x;p=0;}void*v=r->head->data+p;r->head->used=p+n;return v;}static inline void lilscript_region_dispose(LilScriptRegion*r){while(r->head){LilScriptRegionChunk*x=r->head;r->head=x->next;free(x);}}\n",
        );
        out.push_str(
            "static inline LilScriptOptional lilscript_optional_f64(LilScriptOptional v){if(v.has){int32_t i=v.value.i;v.value.tag=2;v.value.f=(double)i;}return v;}\n",
        );
        out.push_str(
            "static inline bool lilscript_value_eq(LilScriptValue a,LilScriptValue b){if(a.tag!=b.tag)return false;switch(a.tag){case 0:return true;case 1:return a.i==b.i;case 2:return a.f==b.f;case 3:return a.b==b.b;case 4:return !strcmp(a.s,b.s);case 5:return a.c.fn==b.c.fn&&a.c.env==b.c.env;default:return a.p==b.p;}}\n",
        );
        out.push_str(
            "static inline bool lilscript_collection_eq(LilScriptValue a,LilScriptValue b){return a.tag==2&&b.tag==2?(a.f==b.f||(a.f!=a.f&&b.f!=b.f)):lilscript_value_eq(a,b);}\n",
        );
        out.push_str(
            "static inline LilScriptValue lilscript_optional_value(LilScriptOptional v){return v.has?v.value:(LilScriptValue){0};}static inline LilScriptOptional lilscript_value_optional(LilScriptValue v){return(LilScriptOptional){v.tag!=0,v};}\n",
        );
        out.push_str(
            "static inline LilScriptArray lilscript_array(int32_t n,size_t z){LilScriptArray a=malloc(sizeof*a);if(!a)abort();a->data=calloc((size_t)n,z);a->len=a->cap=n;if(n&&!a->data)abort();return a;}\n",
        );
        out.push_str(
            "static inline void*lilscript_push(LilScriptArray a,size_t z){if(a->len==a->cap){a->cap=a->cap?a->cap*2:4;a->data=realloc(a->data,(size_t)a->cap*z);if(!a->data)abort();}return(char*)a->data+(size_t)a->len++*z;}\n",
        );
        out.push_str(
            "static inline void*lilscript_pop(LilScriptArray a,size_t z){if(!a->len)abort();return(char*)a->data+(size_t)--a->len*z;}\n",
        );
        out.push_str(
            "static inline int32_t lilscript_array_index(int32_t i,int32_t n){int64_t x=i<0?(int64_t)n+i:i;if(x<0)return 0;if(x>n)return n;return(int32_t)x;}static inline LilScriptArray lilscript_array_slice(LilScriptArray a,int32_t start,int32_t end,size_t z){int32_t x=lilscript_array_index(start,a->len),y=lilscript_array_index(end,a->len);if(y<x)y=x;LilScriptArray r=lilscript_array(y-x,z);if(y>x)memcpy(r->data,(char*)a->data+(size_t)x*z,(size_t)(y-x)*z);return r;}\n",
        );
        out.push_str(
            "static inline int32_t lilscript_array_find_value(LilScriptArray a,LilScriptValue v,int32_t from,bool zero){int32_t i=from<0?from+a->len:from;if(i<0)i=0;for(;i<a->len;i++){LilScriptValue x=((LilScriptValue*)a->data)[i];if(zero?lilscript_collection_eq(x,v):lilscript_value_eq(x,v))return i;}return-1;}\n",
        );
        out.push_str(
            "static inline LilScriptArray lilscript_array_concat(LilScriptArray a,LilScriptArray b){if(a->len>INT32_MAX-b->len)abort();LilScriptArray r=lilscript_array(a->len+b->len,sizeof(LilScriptValue));memcpy(r->data,a->data,(size_t)a->len*sizeof(LilScriptValue));memcpy((LilScriptValue*)r->data+a->len,b->data,(size_t)b->len*sizeof(LilScriptValue));return r;}static inline LilScriptArray lilscript_array_copy_within(LilScriptArray a,int32_t t,int32_t s,int32_t e){t=lilscript_array_index(t,a->len);s=lilscript_array_index(s,a->len);e=lilscript_array_index(e,a->len);int32_t n=e>s?e-s:0;if(n>a->len-t)n=a->len-t;memmove((LilScriptValue*)a->data+t,(LilScriptValue*)a->data+s,(size_t)n*sizeof(LilScriptValue));return a;}static inline LilScriptArray lilscript_array_reverse(LilScriptArray a){LilScriptValue*v=a->data;for(int32_t i=0,j=a->len-1;i<j;i++,j--){LilScriptValue x=v[i];v[i]=v[j];v[j]=x;}return a;}\n",
        );
        out.push_str(
            "static inline LilScriptArray lilscript_array_spread(int32_t n,const uint8_t*k,LilScriptValue*v){int32_t z=0;for(int32_t i=0;i<n;i++){int32_t q=k[i]?((LilScriptArray)v[i].p)->len:1;if(q>INT32_MAX-z)abort();z+=q;}LilScriptArray r=lilscript_array(z,sizeof(LilScriptValue));int32_t p=0;for(int32_t i=0;i<n;i++)if(k[i]){LilScriptArray a=v[i].p;memcpy((LilScriptValue*)r->data+p,a->data,(size_t)a->len*sizeof(LilScriptValue));p+=a->len;}else((LilScriptValue*)r->data)[p++]=v[i];return r;}\n",
        );
        out.push_str(
            "static inline LilScriptMap lilscript_map(void){LilScriptMap m=calloc(1,sizeof*m);if(!m)abort();return m;}static inline void lilscript_map_reserve(LilScriptMap m){if(m->len<m->cap)return;m->cap=m->cap?m->cap*2:4;m->keys=realloc(m->keys,(size_t)m->cap*sizeof*m->keys);m->values=realloc(m->values,(size_t)m->cap*sizeof*m->values);if(!m->keys||!m->values)abort();}static inline int32_t lilscript_map_index(LilScriptMap m,LilScriptValue k){for(int32_t i=0;i<m->len;i++)if(lilscript_collection_eq(m->keys[i],k))return i;return -1;}static inline LilScriptOptional lilscript_map_get(LilScriptMap m,LilScriptValue k){int32_t i=lilscript_map_index(m,k);return i<0?(LilScriptOptional){false,{0}}:lilscript_value_optional(m->values[i]);}static inline bool lilscript_map_has(LilScriptMap m,LilScriptValue k){return lilscript_map_index(m,k)>=0;}static inline LilScriptMap lilscript_map_set(LilScriptMap m,LilScriptValue k,LilScriptValue v){int32_t i=lilscript_map_index(m,k);if(i>=0)m->values[i]=v;else{lilscript_map_reserve(m);m->keys[m->len]=k;m->values[m->len++]=v;}return m;}static inline bool lilscript_map_delete(LilScriptMap m,LilScriptValue k){int32_t i=lilscript_map_index(m,k);if(i<0)return false;int32_t n=--m->len-i;memmove(m->keys+i,m->keys+i+1,(size_t)n*sizeof*m->keys);memmove(m->values+i,m->values+i+1,(size_t)n*sizeof*m->values);return true;}static inline void lilscript_map_clear(LilScriptMap m){m->len=0;}\n",
        );
        out.push_str(
            "static inline LilScriptMap lilscript_record(int32_t n,LilScriptValue*keys,LilScriptValue*values){LilScriptMap m=lilscript_map();for(int32_t i=0;i<n;i++)lilscript_map_set(m,keys[i],values[i]);return m;}\n",
        );
        out.push_str(
            r#"static inline bool lilscript_record_array_index(const char*s,uint32_t*out){if(!*s||(s[0]=='0'&&s[1]))return false;uint64_t n=0;for(const unsigned char*p=(const unsigned char*)s;*p;p++){if(*p<'0'||*p>'9')return false;n=n*10u+(uint64_t)(*p-'0');if(n>=UINT32_MAX)return false;}*out=(uint32_t)n;return true;}
static inline bool lilscript_record_key_before(const char*a,const char*b){uint32_t x=0,y=0;bool ax=lilscript_record_array_index(a,&x),by=lilscript_record_array_index(b,&y);return ax!=by?ax:ax&&x<y;}
static inline int32_t*lilscript_record_order(LilScriptMap m){int32_t*o=malloc((size_t)m->len*sizeof*o);if(m->len&&!o)abort();for(int32_t i=0;i<m->len;i++){int32_t x=i,j=i;while(j&&lilscript_record_key_before(m->keys[x].s,m->keys[o[j-1]].s)){o[j]=o[j-1];j--;}o[j]=x;}return o;}
static inline LilScriptArray lilscript_record_keys(LilScriptMap m){LilScriptArray a=lilscript_array(m->len,sizeof(LilScriptValue));int32_t*o=lilscript_record_order(m);for(int32_t i=0;i<m->len;i++)((LilScriptValue*)a->data)[i]=(LilScriptValue){.tag=4,.s=m->keys[o[i]].s};free(o);return a;}
static inline LilScriptArray lilscript_record_values(LilScriptMap m){LilScriptArray a=lilscript_array(m->len,sizeof(LilScriptValue));int32_t*o=lilscript_record_order(m);for(int32_t i=0;i<m->len;i++)((LilScriptValue*)a->data)[i]=m->values[o[i]];free(o);return a;}
static inline LilScriptMap lilscript_record_assign(LilScriptMap a,LilScriptMap b){int32_t*o=lilscript_record_order(b);for(int32_t i=0;i<b->len;i++){int32_t j=o[i];lilscript_map_set(a,b->keys[j],b->values[j]);}free(o);return a;}
static inline LilScriptMap lilscript_record_spread(int32_t n,const uint8_t*k,LilScriptValue*keys,LilScriptValue*values){LilScriptMap r=lilscript_map();for(int32_t i=0;i<n;i++)if(k[i])lilscript_record_assign(r,values[i].p);else lilscript_map_set(r,keys[i],values[i]);return r;}
static inline LilScriptMap lilscript_record_rest(LilScriptMap a,int32_t n,LilScriptValue*keys){LilScriptMap r=lilscript_map();int32_t*o=lilscript_record_order(a);for(int32_t i=0;i<a->len;i++){int32_t j=o[i],skip=0;for(int32_t k=0;k<n;k++)if(lilscript_value_eq(a->keys[j],keys[k])){skip=1;break;}if(!skip)lilscript_map_set(r,a->keys[j],a->values[j]);}free(o);return r;}
"#,
        );
        out.push_str(
            "static inline LilScriptSet lilscript_set(void){LilScriptSet s=calloc(1,sizeof*s);if(!s)abort();return s;}static inline int32_t lilscript_set_index(LilScriptSet s,LilScriptValue v){for(int32_t i=0;i<s->len;i++)if(lilscript_collection_eq(s->values[i],v))return i;return -1;}static inline LilScriptSet lilscript_set_add(LilScriptSet s,LilScriptValue v){if(lilscript_set_index(s,v)>=0)return s;if(s->len==s->cap){s->cap=s->cap?s->cap*2:4;s->values=realloc(s->values,(size_t)s->cap*sizeof*s->values);if(!s->values)abort();}s->values[s->len++]=v;return s;}static inline bool lilscript_set_has(LilScriptSet s,LilScriptValue v){return lilscript_set_index(s,v)>=0;}static inline bool lilscript_set_delete(LilScriptSet s,LilScriptValue v){int32_t i=lilscript_set_index(s,v);if(i<0)return false;int32_t n=--s->len-i;memmove(s->values+i,s->values+i+1,(size_t)n*sizeof*s->values);return true;}static inline void lilscript_set_clear(LilScriptSet s){s->len=0;}\n",
        );
        out.push_str(
            "static inline LilScriptBuffer lilscript_buffer(int32_t n,bool shared){if(n<0)abort();LilScriptBuffer b=malloc(sizeof*b);if(!b)abort();b->data=calloc((size_t)n,1);if(n&&!b->data)abort();b->len=n;b->shared=shared;return b;}static inline int32_t lilscript_buffer_index(int32_t i,int32_t n){int64_t x=i<0?(int64_t)n+i:i;if(x<0)return 0;if(x>n)return n;return(int32_t)x;}static inline LilScriptBuffer lilscript_buffer_slice(LilScriptBuffer b,int32_t start,int32_t end){int32_t x=lilscript_buffer_index(start,b->len),y=lilscript_buffer_index(end,b->len);if(y<x)y=x;LilScriptBuffer r=lilscript_buffer(y-x,b->shared);if(y>x)memcpy(r->data,b->data+x,(size_t)(y-x));return r;}\n",
        );
        out.push_str(
            "static inline LilScriptTypedArray lilscript_ta_buffer(LilScriptBuffer b,int32_t bpe){if(bpe<=0||b->len%bpe)abort();LilScriptTypedArray v=malloc(sizeof*v);if(!v)abort();v->buffer=b;v->offset=0;v->len=b->len/bpe;return v;}static inline LilScriptTypedArray lilscript_ta_length(int32_t n,int32_t bpe){if(n<0||bpe<=0||n>INT32_MAX/bpe)abort();return lilscript_ta_buffer(lilscript_buffer(n*bpe,false),bpe);}static inline LilScriptTypedArray lilscript_ta_subarray(LilScriptTypedArray v,int32_t start,int32_t end,int32_t bpe){int32_t x=lilscript_buffer_index(start,v->len),y=lilscript_buffer_index(end,v->len);if(y<x)y=x;LilScriptTypedArray r=malloc(sizeof*r);if(!r)abort();r->buffer=v->buffer;r->offset=v->offset+x*bpe;r->len=y-x;return r;}static inline LilScriptTypedArray lilscript_ta_slice(LilScriptTypedArray v,int32_t start,int32_t end,int32_t bpe){int32_t x=lilscript_buffer_index(start,v->len),y=lilscript_buffer_index(end,v->len);if(y<x)y=x;LilScriptTypedArray r=lilscript_ta_length(y-x,bpe);if(y>x)memcpy(r->buffer->data,v->buffer->data+v->offset+(size_t)x*(size_t)bpe,(size_t)(y-x)*(size_t)bpe);return r;}\n",
        );
        out.push_str(
            "static inline int32_t lilscript_ta_get_int(LilScriptTypedArray v,int32_t i,int32_t kind){uint8_t*p=v->buffer->data+v->offset;switch(kind){case 0:return(int32_t)(int8_t)p[i];case 1:case 2:return(int32_t)p[i];case 3:{int16_t x;memcpy(&x,p+(size_t)i*2,2);return(int32_t)x;}case 4:{uint16_t x;memcpy(&x,p+(size_t)i*2,2);return(int32_t)x;}case 5:case 6:{int32_t x;memcpy(&x,p+(size_t)i*4,4);return x;}default:abort();}}\n",
        );
        out.push_str(
            "static inline void lilscript_ta_set_int(LilScriptTypedArray v,int32_t i,int32_t value,int32_t kind){uint8_t*p=v->buffer->data+v->offset;switch(kind){case 0:p[i]=(uint8_t)(int8_t)value;break;case 1:p[i]=(uint8_t)value;break;case 2:p[i]=(uint8_t)(value<0?0:value>255?255:value);break;case 3:{int16_t x=(int16_t)value;memcpy(p+(size_t)i*2,&x,2);break;}case 4:{uint16_t x=(uint16_t)value;memcpy(p+(size_t)i*2,&x,2);break;}case 5:case 6:memcpy(p+(size_t)i*4,&value,4);break;default:abort();}}\n",
        );
        out.push_str(
            "static inline double lilscript_ta_get_f32(LilScriptTypedArray v,int32_t i){float x;memcpy(&x,v->buffer->data+v->offset+(size_t)i*4,4);return(double)x;}static inline void lilscript_ta_set_f32(LilScriptTypedArray v,int32_t i,double value){float x=(float)value;memcpy(v->buffer->data+v->offset+(size_t)i*4,&x,4);}static inline double lilscript_ta_get_f64(LilScriptTypedArray v,int32_t i){double x;memcpy(&x,v->buffer->data+v->offset+(size_t)i*8,8);return x;}static inline void lilscript_ta_set_f64(LilScriptTypedArray v,int32_t i,double value){memcpy(v->buffer->data+v->offset+(size_t)i*8,&value,8);}\n",
        );
        out.push_str(
            "static inline void lilscript_ta_set(LilScriptTypedArray d,LilScriptTypedArray s,int32_t o,int32_t b){if(o<0||s->len>d->len-o)abort();memmove(d->buffer->data+d->offset+(size_t)o*b,s->buffer->data+s->offset,(size_t)s->len*b);}static inline LilScriptTypedArray lilscript_ta_fill_i(LilScriptTypedArray v,int32_t x,int32_t a,int32_t z,int32_t k){a=lilscript_buffer_index(a,v->len);z=lilscript_buffer_index(z,v->len);for(int32_t i=a;i<z;i++)lilscript_ta_set_int(v,i,x,k);return v;}static inline LilScriptTypedArray lilscript_ta_fill_f(LilScriptTypedArray v,double x,int32_t a,int32_t z,int32_t k){a=lilscript_buffer_index(a,v->len);z=lilscript_buffer_index(z,v->len);for(int32_t i=a;i<z;i++)if(k==7)lilscript_ta_set_f32(v,i,x);else lilscript_ta_set_f64(v,i,x);return v;}static inline LilScriptTypedArray lilscript_ta_copy_within(LilScriptTypedArray v,int32_t t,int32_t a,int32_t z,int32_t b){t=lilscript_buffer_index(t,v->len);a=lilscript_buffer_index(a,v->len);z=lilscript_buffer_index(z,v->len);int32_t n=z>a?z-a:0;if(n>v->len-t)n=v->len-t;memmove(v->buffer->data+v->offset+(size_t)t*b,v->buffer->data+v->offset+(size_t)a*b,(size_t)n*b);return v;}\n",
        );
        out.push_str(
            "static inline LilScriptUint8Array lilscript_u8_buffer(LilScriptBuffer b){return lilscript_ta_buffer(b,1);}static inline LilScriptUint8Array lilscript_u8_length(int32_t n){return lilscript_ta_length(n,1);}static inline LilScriptFloat32Array lilscript_f32_buffer(LilScriptBuffer b){return lilscript_ta_buffer(b,4);}static inline LilScriptFloat32Array lilscript_f32_length(int32_t n){return lilscript_ta_length(n,4);}static inline double lilscript_f32_get(LilScriptFloat32Array v,int32_t i){return lilscript_ta_get_f32(v,i);}static inline void lilscript_f32_set(LilScriptFloat32Array v,int32_t i,double value){lilscript_ta_set_f32(v,i,value);}\n",
        );
        out.push_str(
            "static inline double lilscript_float_union_get(LilScriptValue v,int32_t i){if(v.tag==8)return lilscript_f32_get(v.p,i);if(v.tag==6){LilScriptValue e=((LilScriptValue*)((LilScriptArray)v.p)->data)[i];return e.tag==2?e.f:(double)e.i;}abort();}static inline void lilscript_float_union_set(LilScriptValue v,int32_t i,double value){if(v.tag==8){lilscript_f32_set(v.p,i,value);return;}if(v.tag==6){((LilScriptValue*)((LilScriptArray)v.p)->data)[i]=(LilScriptValue){.tag=2,.f=value};return;}abort();}static inline int32_t lilscript_float_union_len(LilScriptValue v){if(v.tag==8)return((LilScriptTypedArray)v.p)->len;if(v.tag==6)return((LilScriptArray)v.p)->len;abort();}\n",
        );
        out.push_str(
            "static inline LilScriptString lilscript_dup(const char*s){size_t n=strlen(s)+1;char*r=malloc(n);if(!r)abort();memcpy(r,s,n);return r;}\n",
        );
        out.push_str(
            "static inline LilScriptString lilscript_cat(const char*a,const char*b){size_t x=strlen(a),y=strlen(b);char*r=malloc(x+y+1);if(!r)abort();memcpy(r,a,x);memcpy(r+x,b,y+1);return r;}\n",
        );
        out.push_str(
            "static inline LilScriptString lilscript_i32(int32_t v){char b[16];snprintf(b,sizeof b,\"%d\",v);return lilscript_dup(b);}\n",
        );
        out.push_str(
            r#"typedef struct{char*p;size_t n,c;}LilScriptJsonBuffer;
static inline void lilscript_json_reserve(LilScriptJsonBuffer*b,size_t n){if(b->n+n+1<=b->c)return;size_t c=b->c?b->c:32;while(c<b->n+n+1)c*=2;b->p=realloc(b->p,c);if(!b->p)abort();b->c=c;}
static inline void lilscript_json_bytes(LilScriptJsonBuffer*b,const char*s,size_t n){lilscript_json_reserve(b,n);memcpy(b->p+b->n,s,n);b->n+=n;b->p[b->n]=0;}
static inline void lilscript_json_char(LilScriptJsonBuffer*b,char c){lilscript_json_bytes(b,&c,1);}
static inline void lilscript_json_quote_into(LilScriptJsonBuffer*b,const char*s){static const char h[]="0123456789abcdef";lilscript_json_char(b,'"');for(const unsigned char*p=(const unsigned char*)s;*p;p++){switch(*p){case '"':lilscript_json_bytes(b,"\\\"",2);break;case '\\':lilscript_json_bytes(b,"\\\\",2);break;case '\b':lilscript_json_bytes(b,"\\b",2);break;case '\f':lilscript_json_bytes(b,"\\f",2);break;case '\n':lilscript_json_bytes(b,"\\n",2);break;case '\r':lilscript_json_bytes(b,"\\r",2);break;case '\t':lilscript_json_bytes(b,"\\t",2);break;default:if(*p<32){char x[6]={'\\','u','0','0',h[*p>>4],h[*p&15]};lilscript_json_bytes(b,x,6);}else lilscript_json_char(b,(char)*p);}}lilscript_json_char(b,'"');}
static inline void lilscript_json_value_into(LilScriptJsonBuffer*b,LilScriptValue v){char n[16];switch(v.tag){case 0:lilscript_json_bytes(b,"null",4);break;case 1:{int z=snprintf(n,sizeof n,"%d",v.i);lilscript_json_bytes(b,n,(size_t)z);break;}case 3:lilscript_json_bytes(b,v.b?"true":"false",v.b?4:5);break;case 4:lilscript_json_quote_into(b,v.s);break;default:abort();}}
static inline LilScriptString lilscript_json_finish(LilScriptJsonBuffer*b){if(!b->p){b->p=malloc(1);if(!b->p)abort();b->p[0]=0;}return b->p;}
static inline LilScriptString lilscript_json_value(LilScriptValue v){LilScriptJsonBuffer b={0};lilscript_json_value_into(&b,v);return lilscript_json_finish(&b);}
static inline LilScriptString lilscript_json_array(LilScriptArray a){LilScriptJsonBuffer b={0};lilscript_json_char(&b,'[');for(int32_t i=0;i<a->len;i++){if(i)lilscript_json_char(&b,',');lilscript_json_value_into(&b,((LilScriptValue*)a->data)[i]);}lilscript_json_char(&b,']');return lilscript_json_finish(&b);}
static inline LilScriptString lilscript_json_record(LilScriptMap m){LilScriptJsonBuffer b={0};int32_t*o=lilscript_record_order(m);lilscript_json_char(&b,'{');for(int32_t i=0;i<m->len;i++){int32_t j=o[i];if(i)lilscript_json_char(&b,',');lilscript_json_quote_into(&b,m->keys[j].s);lilscript_json_char(&b,':');lilscript_json_value_into(&b,m->values[j]);}lilscript_json_char(&b,'}');free(o);return lilscript_json_finish(&b);}
"#,
        );
        out.push_str(
            "static inline LilScriptString lilscript_i32_radix(int32_t v,int32_t radix,bool unsign){if(radix<2||radix>36)abort();static const char d[]=\"0123456789abcdefghijklmnopqrstuvwxyz\";char b[35],*p=b+sizeof b;*--p=0;bool neg=!unsign&&v<0;uint32_t n=unsign?(uint32_t)v:neg?(uint32_t)(0-(uint32_t)v):(uint32_t)v;do{*--p=d[n%(uint32_t)radix];n/=(uint32_t)radix;}while(n);if(neg)*--p='-';return lilscript_dup(p);}\n",
        );
        out.push_str(
            "static inline LilScriptString lilscript_f64(double v){char b[32];snprintf(b,sizeof b,\"%.17g\",v);return lilscript_dup(b);}\n",
        );
        out.push_str(
            "static inline LilScriptString lilscript_value_string(LilScriptValue v){switch(v.tag){case 0:return lilscript_dup(\"null\");case 1:return lilscript_i32(v.i);case 2:return lilscript_f64(v.f);case 3:return lilscript_dup(v.b?\"true\":\"false\");case 4:return lilscript_dup(v.s);default:abort();}}static inline void lilscript_print_value(LilScriptValue v){switch(v.tag){case 0:puts(\"null\");break;case 1:printf(\"%d\\n\",v.i);break;case 2:printf(\"%.17g\\n\",v.f);break;case 3:puts(v.b?\"true\":\"false\");break;case 4:puts(v.s);break;default:abort();}}\n",
        );
        out.push_str(
            "static inline LilScriptString lilscript_array_join(LilScriptArray a,const char*sep){size_t n=0,c=1;char*r=malloc(c);if(!r)abort();r[0]=0;for(int32_t i=0;i<a->len;i++){LilScriptValue v=((LilScriptValue*)a->data)[i];char*x=v.tag?lilscript_value_string(v):lilscript_dup(\"\");size_t p=i?strlen(sep):0,q=strlen(x),need=n+p+q+1;if(need>c){c=need;r=realloc(r,c);if(!r)abort();}if(p){memcpy(r+n,sep,p);n+=p;}memcpy(r+n,x,q);n+=q;r[n]=0;free(x);}return r;}\n",
        );
        out.push_str(
            "static inline bool lilscript_ends(const char*s,const char*x){size_t a=strlen(s),b=strlen(x);return a>=b&&!memcmp(s+a-b,x,b);}\n",
        );
        out.push_str(
            "static inline LilScriptString lilscript_case(const char*s,bool upper){LilScriptString r=lilscript_dup(s);for(char*p=r;*p;p++)*p=(char)(upper?toupper((unsigned char)*p):tolower((unsigned char)*p));return r;}\n",
        );
        out.push_str(
            "static inline uint32_t lilscript_utf8_next(const unsigned char**p){uint32_t c=*(*p)++;if(c<128)return c;if((c&224)==192){uint32_t r=(c&31)<<6;return r|(*(*p)++&63);}if((c&240)==224){uint32_t r=(c&15)<<12;r|=(uint32_t)(*(*p)++&63)<<6;return r|(*(*p)++&63);}uint32_t r=(c&7)<<18;r|=(uint32_t)(*(*p)++&63)<<12;r|=(uint32_t)(*(*p)++&63)<<6;return r|(*(*p)++&63);}\n",
        );
        out.push_str(
            "static inline int32_t lilscript_utf16_len(const char*s){const unsigned char*p=(const unsigned char*)s;int32_t n=0;while(*p){uint32_t c=lilscript_utf8_next(&p);n+=c>65535?2:1;}return n;}static inline int32_t lilscript_char_code_at(const char*s,int32_t i){if(i<0)return 0;const unsigned char*p=(const unsigned char*)s;int32_t n=0;while(*p){uint32_t c=lilscript_utf8_next(&p);if(c<=65535){if(n++==i)return(int32_t)c;}else{c-=65536;uint32_t h=55296+(c>>10),l=56320+(c&1023);if(n++==i)return(int32_t)h;if(n++==i)return(int32_t)l;}}return 0;}static inline LilScriptString lilscript_char_at(const char*s,int32_t i){int32_t code=lilscript_char_code_at(s,i);if(i<0||i>=lilscript_utf16_len(s))return lilscript_dup(\"\");char b[5];uint32_t c=(uint32_t)code;if(c<128){b[0]=(char)c;b[1]=0;}else if(c<2048){b[0]=(char)(192|(c>>6));b[1]=(char)(128|(c&63));b[2]=0;}else{b[0]=(char)(224|(c>>12));b[1]=(char)(128|((c>>6)&63));b[2]=(char)(128|(c&63));b[3]=0;}return lilscript_dup(b);}\n",
        );
        out.push_str(
            "static inline int32_t lilscript_string_index(const char*s,const char*x,int32_t p,bool last){int32_t n=lilscript_utf16_len(s),m=lilscript_utf16_len(x);if(p<0)p=0;if(p>n)p=n;if(!m)return p;if(m>n)return-1;int32_t z=n-m;if(last&&p<z)z=p;if(!last){if(p>z)return-1;z=p;}for(int32_t i=z;last?i>=0:i<=n-m;last?i--:i++){bool same=true;for(int32_t j=0;j<m;j++)if(lilscript_char_code_at(s,i+j)!=lilscript_char_code_at(x,j)){same=false;break;}if(same)return i;}return-1;}static inline LilScriptString lilscript_repeat(const char*s,int32_t n){if(n<0)abort();size_t z=strlen(s);if(z&&((size_t)n>SIZE_MAX/z))abort();size_t q=z*(size_t)n;char*r=malloc(q+1);if(!r)abort();for(int32_t i=0;i<n;i++)memcpy(r+(size_t)i*z,s,z);r[q]=0;return r;}\n",
        );
        out.push_str(
            "static inline void*lilscript_copy(const void*value,size_t size){void*copy=malloc(size);if(!copy)abort();memcpy(copy,value,size);return copy;}\n",
        );

        self.emit_aggregate_types(&mut out)?;

        for global in &self.module.globals {
            writeln!(out, "static {} g{};", c_type(&global.ty), global.symbol.0)
                .expect("writing to String cannot fail");
        }
        for function in &self.module.functions {
            if function.live && function.kind == FunctionKind::Extern {
                self.emit_extern(function, &mut out)?;
            }
        }
        for function in &self.module.functions {
            if function.live
                && function.kind == FunctionKind::Closure
                && function.capture_count != 0
            {
                write!(out, "typedef struct{{").expect("writing to String cannot fail");
                for (index, capture) in function.params[..function.capture_count].iter().enumerate()
                {
                    write!(
                        out,
                        "{}{} c{index};",
                        c_type(&capture.ty),
                        if function.mutable_capture_locals.contains(&capture.local) {
                            "*"
                        } else {
                            ""
                        }
                    )
                    .expect("writing to String cannot fail");
                }
                writeln!(out, "}}E{};", function.id.0).expect("writing to String cannot fail");
            }
        }
        for function in &self.module.functions {
            if !function.live || matches!(function.kind, FunctionKind::Entry | FunctionKind::Extern)
            {
                continue;
            }
            self.emit_function_signature(function, false, &mut out)?;
            out.push_str(";\n");
        }
        let closure_adapters = self.closure_adapter_targets();
        for function in &closure_adapters {
            self.emit_closure_adapter_signature(function, false, &mut out);
            out.push_str(";\n");
        }
        for function in &self.module.functions {
            if !function.live || matches!(function.kind, FunctionKind::Entry | FunctionKind::Extern)
            {
                continue;
            }
            self.emit_function(function, &mut out)?;
        }
        for function in closure_adapters {
            self.emit_closure_adapter(function, &mut out)?;
        }
        self.emit_function(self.function(self.module.entry)?, &mut out)?;
        Ok(out)
    }

    fn validate_host_boundaries(&self) -> Result<(), CodegenError> {
        if let Some(import) = self.module.foreign_imports.first() {
            return Err(CodegenError::new(
                import.span,
                "JavaScript and TypeScript module imports are only available for JavaScript targets",
            ));
        }
        if let Some(class) = self
            .module
            .classes
            .iter()
            .find(|class| class.base.is_some())
        {
            return Err(CodegenError::new(
                self.function(self.module.entry)?.span,
                format!(
                    "class inheritance for `{}` is only available for JavaScript targets until the native subtype ABI is fixed",
                    class.name
                ),
            ));
        }
        if let Some(function) = self
            .module
            .functions
            .iter()
            .find(|function| function.is_async)
        {
            return Err(CodegenError::new(
                function.span,
                "async functions and await are only available for JavaScript targets",
            ));
        }
        if let Some(function) = self
            .module
            .functions
            .iter()
            .find(|function| function.is_generator)
        {
            return Err(CodegenError::new(
                function.span,
                "generators and yield are only available for JavaScript targets",
            ));
        }
        if let Some(function) = self.module.functions.iter().find(|function| {
            function
                .params
                .iter()
                .any(|parameter| contains_generator(&parameter.ty))
                || contains_generator(&function.return_type)
                || function
                    .locals
                    .iter()
                    .any(|local| contains_generator(&local.ty))
                || function.blocks.iter().any(|block| {
                    block.phis.iter().any(|phi| contains_generator(&phi.ty))
                        || block.instructions.iter().any(|instruction| {
                            instruction.ty.as_ref().is_some_and(contains_generator)
                        })
                })
        }) {
            return Err(CodegenError::new(
                function.span,
                "Generator<T> values are only available for JavaScript targets",
            ));
        }
        if let Some(global) = self
            .module
            .globals
            .iter()
            .find(|global| contains_generator(&global.ty))
        {
            return Err(CodegenError::new(
                global.span,
                "Generator<T> values are only available for JavaScript targets",
            ));
        }
        if self
            .module
            .structs
            .iter()
            .chain(&self.module.classes)
            .any(|layout| {
                layout
                    .fields
                    .iter()
                    .any(|field| contains_generator(&field.ty))
            })
        {
            return Err(CodegenError::new(
                self.function(self.module.entry)?.span,
                "Generator<T> fields are only available for JavaScript targets",
            ));
        }
        if let Some(block) = self
            .module
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .find(|block| {
                matches!(
                    block.terminator,
                    Some(Terminator::Throw(_) | Terminator::Try { .. })
                )
            })
        {
            return Err(CodegenError::new(
                block.span,
                "exceptions are only available for JavaScript targets",
            ));
        }
        if let Some(function) = self.module.functions.iter().find(|function| {
            function
                .params
                .iter()
                .any(|parameter| contains_regex(&parameter.ty))
                || contains_regex(&function.return_type)
                || function
                    .locals
                    .iter()
                    .any(|local| contains_regex(&local.ty))
        }) {
            return Err(CodegenError::new(
                function.span,
                "Regex is only available for JavaScript targets",
            ));
        }
        if let Some(global) = self
            .module
            .globals
            .iter()
            .find(|global| contains_regex(&global.ty))
        {
            return Err(CodegenError::new(
                global.span,
                "Regex is only available for JavaScript targets",
            ));
        }
        if let Some(function) = self.module.functions.iter().find(|function| {
            function
                .params
                .iter()
                .any(|parameter| contains_js_value(&parameter.ty))
                || contains_js_value(&function.return_type)
                || function
                    .locals
                    .iter()
                    .any(|local| contains_js_value(&local.ty))
        }) {
            return Err(CodegenError::new(
                function.span,
                "JsValue is only available for JavaScript targets",
            ));
        }
        if let Some(instruction) = self
            .module
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .find(|instruction| {
                matches!(
                    instruction.op,
                    ControlFlowOp::HostFieldGet { .. }
                        | ControlFlowOp::HostFieldSet { .. }
                        | ControlFlowOp::HostCall { .. }
                        | ControlFlowOp::DynamicImport { .. }
                        | ControlFlowOp::Await { .. }
                        | ControlFlowOp::Intrinsic {
                            intrinsic: Intrinsic::TaskResolve
                                | Intrinsic::TaskReject
                                | Intrinsic::TaskAll
                                | Intrinsic::JsonParse
                                | Intrinsic::RegexNew
                                | Intrinsic::RegexTest
                                | Intrinsic::RegexSource
                                | Intrinsic::RegexFlags
                                | Intrinsic::RegexGlobal
                                | Intrinsic::RegexIgnoreCase
                                | Intrinsic::RegexMultiline
                                | Intrinsic::RegexDotAll
                                | Intrinsic::RegexSticky
                                | Intrinsic::RegexUnicode
                                | Intrinsic::JsPlainObject
                                | Intrinsic::JsUndefined
                                | Intrinsic::JsDateNow
                                | Intrinsic::JsParseFloat
                                | Intrinsic::JsParseInt
                                | Intrinsic::JsIsFinite
                                | Intrinsic::JsEncodeURI
                                | Intrinsic::JsEncodeURIComponent
                                | Intrinsic::JsObjectCreate
                                | Intrinsic::JsGetPrototypeOf
                                | Intrinsic::JsMathPI
                                | Intrinsic::JsObjectConstructor
                                | Intrinsic::JsWindow
                                | Intrinsic::JsDocument
                                | Intrinsic::JsSetTimeout
                                | Intrinsic::JsClearTimeout
                                | Intrinsic::JsDomParserNew
                                | Intrinsic::JsXMLHttpRequestNew
                                | Intrinsic::JsNullProtoObject
                                | Intrinsic::JsTypeOf
                                | Intrinsic::JsIsNullish
                                | Intrinsic::JsIsFalse
                                | Intrinsic::JsIsUndefined
                                | Intrinsic::JsStringify
                                | Intrinsic::JsNumber
                                | Intrinsic::JsAdd
                                | Intrinsic::JsMod
                                | Intrinsic::JsLessThan
                                | Intrinsic::JsLessThanOrEqual
                                | Intrinsic::JsGreaterThan
                                | Intrinsic::JsGreaterThanOrEqual
                                | Intrinsic::JsStrictEqual
                                | Intrinsic::JsStrictNotEqual
                                | Intrinsic::JsCall
                                | Intrinsic::JsConstruct
                                | Intrinsic::JsInvoke
                                | Intrinsic::JsApply
                                | Intrinsic::JsMethod0
                                | Intrinsic::JsMethod1
                                | Intrinsic::JsMethod2
                                | Intrinsic::JsMethod3
                                | Intrinsic::JsMethodRest
                                | Intrinsic::JsStaticRest
                                | Intrinsic::JsDeleteProperty
                                | Intrinsic::JsHasProperty
                                | Intrinsic::JsInProperty
                                | Intrinsic::JsBox
                                | Intrinsic::JsArrayPush
                                | Intrinsic::JsArrayPop
                                | Intrinsic::JsArraySlice
                                | Intrinsic::JsArrayIndexOf
                                | Intrinsic::JsArraySort
                                | Intrinsic::JsArraySplice
                                | Intrinsic::JsArrayConcatApply
                                | Intrinsic::JsArrayJoin
                                | Intrinsic::JsArrayShift
                                | Intrinsic::JsArrayUnshift
                                | Intrinsic::JsArrayFlat
                                | Intrinsic::JsIsFunctionValue
                                | Intrinsic::JsIsWindowValue
                                | Intrinsic::JsDefineConfigurable
                                | Intrinsic::JsDefineIterator
                                | Intrinsic::JsArrayIterator
                                | Intrinsic::JsConsoleWarn
                                | Intrinsic::JsRequestAnimationFrameOrNull
                                | Intrinsic::JsStringSlice
                                | Intrinsic::JsStringIndexOf
                                | Intrinsic::JsStringReplace
                                | Intrinsic::JsStringMatch
                                | Intrinsic::JsStringSplit
                                | Intrinsic::StringSlice
                                | Intrinsic::StringReplace
                                | Intrinsic::StringSplit
                                | Intrinsic::StringTrim
                                | Intrinsic::StringTrimStart
                                | Intrinsic::StringTrimEnd
                                | Intrinsic::StringSearch
                                | Intrinsic::StringCodePointLength
                                | Intrinsic::JsRegexExec,
                            ..
                        }
                )
            })
        {
            return Err(CodegenError::new(
                instruction.span,
                "this operation is only available for JavaScript targets",
            ));
        }
        if let Some(global) = self.module.globals.iter().find(|global| global.external) {
            return Err(CodegenError::new(
                global.span,
                "extern host globals are only available for JavaScript targets",
            ));
        }
        Ok(())
    }

    fn closure_adapter_targets(&self) -> Vec<&ControlFlowFunction<'src>> {
        let mut referenced = AHashSet::default();
        for function in &self.module.functions {
            if !function.live {
                continue;
            }
            for block in &function.blocks {
                for instruction in &block.instructions {
                    if let ControlFlowOp::Closure { function, .. } = instruction.op {
                        referenced.insert(function);
                    }
                }
            }
        }
        self.module
            .functions
            .iter()
            .filter(|function| {
                function.live
                    && function.kind != FunctionKind::Closure
                    && referenced.contains(&function.id)
            })
            .collect()
    }

    fn emit_closure_adapter_signature(
        &self,
        function: &ControlFlowFunction<'src>,
        names: bool,
        out: &mut String,
    ) {
        write!(out, "static LilScriptValue a{}(void*", function.id.0)
            .expect("writing to String cannot fail");
        if names {
            out.push_str(" env");
        }
        for (index, _) in function.params.iter().enumerate() {
            out.push_str(",LilScriptValue");
            if names {
                write!(out, " a{index}").expect("writing to String cannot fail");
            }
        }
        out.push(')');
    }

    fn emit_closure_adapter(
        &self,
        function: &ControlFlowFunction<'src>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        self.emit_closure_adapter_signature(function, true, out);
        out.push_str("{(void)env;");
        let mut call = if function.kind == FunctionKind::Extern {
            self.native_function_name(function.id)?.to_string()
        } else {
            format!("f{}", function.id.0)
        };
        call.push('(');
        let universal = Type::TypeParameter("$closure");
        for (index, parameter) in function.params.iter().enumerate() {
            if index != 0 {
                call.push(',');
            }
            call.push_str(&self.render_value_conversion(
                &format!("a{index}"),
                &universal,
                &parameter.ty,
                parameter.span,
            )?);
        }
        call.push(')');
        if function.return_type == Type::Void {
            write!(out, "{call};return (LilScriptValue){{0}};}}")
                .expect("writing to String cannot fail");
        } else {
            let result = self.render_value_conversion(
                &call,
                &function.return_type,
                &universal,
                function.span,
            )?;
            write!(out, "return {result};}}").expect("writing to String cannot fail");
        }
        out.push('\n');
        Ok(())
    }

    fn emit_aggregate_types(&self, out: &mut String) -> Result<(), CodegenError> {
        for layout in &self.module.structs {
            let name = aggregate_type_name("Struct", layout.name);
            writeln!(out, "typedef struct {name} {name};").expect("writing to String cannot fail");
        }
        for layout in &self.module.classes {
            let name = aggregate_type_name("Class", layout.name);
            writeln!(out, "typedef struct {name}*{name};").expect("writing to String cannot fail");
        }

        let mut emitted = AHashSet::default();
        while emitted.len() != self.module.structs.len() {
            let mut changed = false;
            for layout in &self.module.structs {
                if emitted.contains(layout.name)
                    || layout.fields.iter().any(
                        |field| matches!(&field.ty, Type::Struct(name) if !emitted.contains(name)),
                    )
                {
                    continue;
                }
                self.emit_aggregate_body("Struct", layout, out)?;
                emitted.insert(layout.name);
                changed = true;
            }
            if !changed {
                return Err(CodegenError::new(
                    self.function(self.module.entry)?.span,
                    "native value structs contain a recursive by-value cycle",
                ));
            }
        }
        for layout in &self.module.classes {
            self.emit_aggregate_body("Class", layout, out)?;
        }
        Ok(())
    }

    fn emit_aggregate_body(
        &self,
        kind: &str,
        layout: &crate::ir::AggregateLayout<'src>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let name = aggregate_type_name(kind, layout.name);
        write!(out, "struct {name}{{").expect("writing to String cannot fail");
        for field in &layout.fields {
            write!(out, "{} f{};", c_type(&field.ty), field.index)
                .expect("writing to String cannot fail");
        }
        out.push_str("};\n");
        Ok(())
    }

    fn emit_function_signature(
        &self,
        function: &ControlFlowFunction<'src>,
        names: bool,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        write!(
            out,
            "static {} f{}(",
            if function.kind == FunctionKind::Closure {
                "LilScriptValue".to_string()
            } else {
                c_type(&function.return_type)
            },
            function.id.0
        )
        .expect("writing to String cannot fail");
        if function.kind == FunctionKind::Closure {
            out.push_str("void*");
            if names {
                out.push_str(" env");
            }
            for param in &function.params[function.capture_count..] {
                out.push(',');
                out.push_str("LilScriptValue");
                if names {
                    write!(out, " a{}", param.value.0).expect("writing to String cannot fail");
                }
            }
        } else if function.params.is_empty() {
            out.push_str("void");
        } else {
            for (index, param) in function.params.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                out.push_str(&c_type(&param.ty));
                if names {
                    write!(out, " v{}", param.value.0).expect("writing to String cannot fail");
                }
            }
        }
        out.push(')');
        Ok(())
    }

    fn emit_extern(
        &self,
        function: &ControlFlowFunction<'src>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let name = function
            .name
            .ok_or_else(|| CodegenError::new(function.span, "extern function has no name"))?;
        write!(out, "extern {} {name}(", c_type(&function.return_type))
            .expect("writing to String cannot fail");
        self.emit_parameter_types(function, out)?;
        out.push_str(");\n");
        Ok(())
    }

    fn emit_parameter_types(
        &self,
        function: &ControlFlowFunction<'src>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        if function.params.is_empty() {
            out.push_str("void");
        } else {
            for (index, param) in function.params.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                out.push_str(&c_type(&param.ty));
            }
        }
        Ok(())
    }

    fn native_allocation_plan(
        &self,
        function: &ControlFlowFunction<'src>,
    ) -> AHashMap<ValueId, NativeStorage<'src>> {
        if !self.options.partial_escape_analysis {
            return AHashMap::default();
        }
        let mut plan = AHashMap::default();
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            let Some(value) = instruction.out else {
                continue;
            };
            let escape = function
                .value_escapes
                .get(value.0 as usize)
                .copied()
                .unwrap_or(EscapeState::EscapesToUntypedBoundary);
            let bounded = allocation_is_function_bounded(self.module, function, value);
            let storage = match &instruction.op {
                ControlFlowOp::Array(values)
                    if escape == EscapeState::LocalOnly
                        && bounded
                        && array_capacity_is_fixed(function, value) =>
                {
                    if self.options.stack_allocation
                        && values.len() <= self.options.stack_array_element_limit
                    {
                        Some(NativeStorage::StackArray(values.len()))
                    } else if self.options.region_allocation && bounded {
                        Some(NativeStorage::RegionArray(values.len()))
                    } else {
                        None
                    }
                }
                ControlFlowOp::NewClass { class, .. }
                    if escape == EscapeState::LocalOnly
                        && bounded
                        && self.options.stack_allocation =>
                {
                    Some(NativeStorage::StackClass(class))
                }
                ControlFlowOp::NewClass { .. } if self.options.region_allocation && bounded => {
                    Some(NativeStorage::RegionClass)
                }
                ControlFlowOp::Closure { function, captures }
                    if !captures.is_empty()
                        && escape == EscapeState::LocalOnly
                        && bounded
                        && self.options.stack_allocation =>
                {
                    Some(NativeStorage::StackClosure(*function))
                }
                ControlFlowOp::Closure { function, captures }
                    if !captures.is_empty() && self.options.region_allocation && bounded =>
                {
                    Some(NativeStorage::RegionClosure(*function))
                }
                _ => None,
            };
            if let Some(storage) = storage {
                plan.insert(value, storage);
            }
        }
        plan
    }

    fn emit_function(
        &self,
        function: &ControlFlowFunction<'src>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let entry = function.kind == FunctionKind::Entry;
        if entry {
            out.push_str("int main(void){");
        } else {
            self.emit_function_signature(function, true, out)?;
            out.push('{');
        }

        let types = value_types(function);
        let allocation_plan = self.native_allocation_plan(function);
        let uses_region = allocation_plan.values().any(|storage| {
            matches!(
                storage,
                NativeStorage::RegionArray(_)
                    | NativeStorage::RegionClass
                    | NativeStorage::RegionClosure(_)
            )
        });
        let mut declarations = types.iter().collect::<Vec<_>>();
        declarations.sort_unstable_by_key(|(value, _)| value.0);
        for (value, ty) in declarations {
            let declared_parameter = if function.kind != FunctionKind::Closure {
                function.params.iter().any(|param| param.value == *value)
            } else {
                false
            };
            if declared_parameter {
                continue;
            }
            if *ty == Type::Void {
                continue;
            }
            write!(out, "{} v{};", c_type(ty), value.0).expect("writing to String cannot fail");
        }
        for local in &function.mutable_capture_locals {
            let binding = function.locals.get(local.0 as usize).ok_or_else(|| {
                CodegenError::new(function.span, "mutable capture local is missing metadata")
            })?;
            let inherited = function.params[..function.capture_count]
                .iter()
                .any(|parameter| parameter.local == *local);
            write!(out, "{}*l{};", c_type(&binding.ty), local.0)
                .expect("writing to String cannot fail");
            if !inherited {
                write!(
                    out,
                    "l{}=malloc(sizeof*l{});if(!l{})abort();",
                    local.0, local.0, local.0
                )
                .expect("writing to String cannot fail");
            }
        }
        let mut storage = allocation_plan.iter().collect::<Vec<_>>();
        storage.sort_unstable_by_key(|(value, _)| value.0);
        for (value, allocation) in storage {
            match allocation {
                NativeStorage::StackArray(length) => {
                    write!(
                        out,
                        "struct LilScriptArrayHeader h{};LilScriptValue d{}[{}];",
                        value.0,
                        value.0,
                        (*length).max(1)
                    )
                    .expect("writing to String cannot fail");
                }
                NativeStorage::StackClass(class) => {
                    write!(
                        out,
                        "struct {} c{};",
                        aggregate_type_name("Class", class),
                        value.0
                    )
                    .expect("writing to String cannot fail");
                }
                NativeStorage::StackClosure(target) => {
                    write!(out, "E{} e{}_storage;", target.0, value.0)
                        .expect("writing to String cannot fail");
                }
                NativeStorage::RegionArray(_)
                | NativeStorage::RegionClass
                | NativeStorage::RegionClosure(_) => {}
            }
        }
        if uses_region {
            out.push_str("LilScriptRegion r={0};");
        }
        if function.kind == FunctionKind::Closure {
            if function.capture_count == 0 {
                out.push_str("(void)env;");
            } else {
                write!(out, "E{}*e=(E{}*)env;", function.id.0, function.id.0)
                    .expect("writing to String cannot fail");
                for (index, capture) in function.params[..function.capture_count].iter().enumerate()
                {
                    if function.mutable_capture_locals.contains(&capture.local) {
                        write!(out, "l{}=e->c{index};", capture.local.0)
                            .expect("writing to String cannot fail");
                    } else {
                        write!(out, "v{}=e->c{index};", capture.value.0)
                            .expect("writing to String cannot fail");
                    }
                }
            }
            let universal = Type::TypeParameter("$closure");
            for parameter in &function.params[function.capture_count..] {
                let converted = self.render_value_conversion(
                    &format!("a{}", parameter.value.0),
                    &universal,
                    &parameter.ty,
                    parameter.span,
                )?;
                write!(out, "v{}={converted};", parameter.value.0)
                    .expect("writing to String cannot fail");
            }
        }
        write!(out, "uint32_t s={};for(;;)switch(s){{", function.entry.0)
            .expect("writing to String cannot fail");

        let mut phi_temp = 0usize;
        for block in &function.blocks {
            write!(out, "case {}:{{", block.id.0).expect("writing to String cannot fail");
            for instruction in &block.instructions {
                self.emit_instruction(function, instruction, &types, &allocation_plan, out)?;
            }
            self.emit_terminator(function, block.id, &types, &mut phi_temp, uses_region, out)?;
            out.push('}');
        }
        out.push('}');
        if entry {
            out.push_str("return 0;");
        }
        out.push_str("}\n");
        Ok(())
    }

    fn emit_instruction(
        &self,
        owner: &ControlFlowFunction<'src>,
        instruction: &ControlFlowInstruction<'src>,
        types: &AHashMap<ValueId, Type<'src>>,
        allocation_plan: &AHashMap<ValueId, NativeStorage<'src>>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        match &instruction.op {
            ControlFlowOp::StoreLocal { local, value }
                if owner.mutable_capture_locals.contains(local) =>
            {
                write!(out, "*l{}=v{};", local.0, value.0).expect("writing to String cannot fail");
                return Ok(());
            }
            ControlFlowOp::Array(values) => {
                let result = required_output(instruction)?;
                let Some(Type::Array(element)) = instruction.ty.as_ref() else {
                    return Err(CodegenError::new(
                        instruction.span,
                        "array instruction has no array type",
                    ));
                };
                match allocation_plan.get(&result) {
                    Some(NativeStorage::StackArray(_)) => write!(
                        out,
                        "v{}=&h{};v{}->data=d{};v{}->len=v{}->cap={};",
                        result.0,
                        result.0,
                        result.0,
                        result.0,
                        result.0,
                        result.0,
                        values.len()
                    ),
                    Some(NativeStorage::RegionArray(_)) => write!(
                        out,
                        "v{}=lilscript_region_alloc(&r,sizeof*v{});v{}->data=lilscript_region_alloc(&r,{}*sizeof(LilScriptValue));v{}->len=v{}->cap={};",
                        result.0,
                        result.0,
                        result.0,
                        values.len().max(1),
                        result.0,
                        result.0,
                        values.len()
                    ),
                    _ => write!(
                        out,
                        "v{}=lilscript_array({},sizeof(LilScriptValue));",
                        result.0,
                        values.len()
                    ),
                }
                .expect("writing to String cannot fail");
                for (index, value) in values.iter().enumerate() {
                    let normalized = self.render_value_conversion(
                        &format!("v{}", value.0),
                        &types[value],
                        element,
                        instruction.span,
                    )?;
                    let converted = self.render_value_conversion(
                        &normalized,
                        element,
                        &Type::TypeParameter("$array"),
                        instruction.span,
                    )?;
                    write!(
                        out,
                        "((LilScriptValue*)v{}->data)[{index}]={converted};",
                        result.0,
                    )
                    .expect("writing to String cannot fail");
                }
                return Ok(());
            }
            ControlFlowOp::NewClass {
                class,
                constructor,
                args,
            } => {
                let result = required_output(instruction)?;
                match allocation_plan.get(&result) {
                    Some(NativeStorage::StackClass(_)) => write!(
                        out,
                        "v{}=&c{};memset(v{},0,sizeof*v{});",
                        result.0, result.0, result.0, result.0
                    ),
                    Some(NativeStorage::RegionClass) => write!(
                        out,
                        "v{}=lilscript_region_alloc(&r,sizeof*v{});memset(v{},0,sizeof*v{});",
                        result.0, result.0, result.0, result.0
                    ),
                    _ => write!(
                        out,
                        "v{}=calloc(1,sizeof*v{});if(!v{})abort();",
                        result.0, result.0, result.0
                    ),
                }
                .expect("writing to String cannot fail");
                let layout = self
                    .module
                    .classes
                    .iter()
                    .find(|layout| layout.name == *class)
                    .ok_or_else(|| {
                        CodegenError::new(instruction.span, "missing native class layout")
                    })?;
                for field in &layout.fields {
                    match &field.ty {
                        Type::String => write!(out, "v{}->f{}=\"\";", result.0, field.index)
                            .expect("writing to String cannot fail"),
                        Type::Array(_) => write!(
                            out,
                            "v{}->f{}=lilscript_array(0,sizeof(LilScriptValue));",
                            result.0, field.index
                        )
                        .expect("writing to String cannot fail"),
                        Type::Map(_, _)
                        | Type::Set(_)
                        | Type::ArrayBuffer
                        | Type::SharedArrayBuffer
                        | Type::Symbol => {
                            let default = native_default_value(&field.ty)?;
                            write!(out, "v{}->f{}={default};", result.0, field.index)
                                .expect("writing to String cannot fail");
                        }
                        ty if TypedArrayKind::from_type(ty).is_some() => {
                            let default = native_default_value(&field.ty)?;
                            write!(out, "v{}->f{}={default};", result.0, field.index)
                                .expect("writing to String cannot fail");
                        }
                        Type::Union(members) => {
                            let member = members.first().ok_or_else(|| {
                                CodegenError::new(
                                    instruction.span,
                                    "union field has no native member type",
                                )
                            })?;
                            let default = native_default_value(member)?;
                            let converted = self.render_value_conversion(
                                &default,
                                member,
                                &field.ty,
                                instruction.span,
                            )?;
                            write!(out, "v{}->f{}={converted};", result.0, field.index)
                                .expect("writing to String cannot fail");
                        }
                        _ => {}
                    }
                }
                if let Some(constructor) = constructor {
                    let mut call_args = vec![result];
                    call_args.extend(args);
                    out.push_str(&self.render_direct_call(
                        *constructor,
                        &call_args,
                        types,
                        Some(&Type::Void),
                        instruction.span,
                    )?);
                    out.push(';');
                }
                return Ok(());
            }
            ControlFlowOp::Closure { function, captures } => {
                let result = required_output(instruction)?;
                let target = self.function(*function)?;
                let prefix = if target.kind == FunctionKind::Closure {
                    'f'
                } else {
                    'a'
                };
                if captures.is_empty() {
                    write!(
                        out,
                        "v{}=(LilScriptClosure){{(void*){prefix}{},NULL}};",
                        result.0, function.0,
                    )
                    .expect("writing to String cannot fail");
                } else {
                    match allocation_plan.get(&result) {
                        Some(NativeStorage::StackClosure(_)) => {
                            write!(out, "E{}*e{}=&e{}_storage;", function.0, result.0, result.0)
                        }
                        Some(NativeStorage::RegionClosure(_)) => write!(
                            out,
                            "E{}*e{}=lilscript_region_alloc(&r,sizeof(E{}));",
                            function.0, result.0, function.0
                        ),
                        _ => write!(
                            out,
                            "E{}*e{}=malloc(sizeof(E{}));if(!e{})abort();",
                            function.0, result.0, function.0, result.0
                        ),
                    }
                    .expect("writing to String cannot fail");
                    let closure = self.function(*function)?;
                    for (index, capture) in captures.iter().enumerate() {
                        let parameter = &closure.params[index];
                        if closure.mutable_capture_locals.contains(&parameter.local) {
                            let source =
                                capture_local_source(owner, *capture).ok_or_else(|| {
                                    CodegenError::new(
                                        instruction.span,
                                        "mutable closure capture is not a lexical cell",
                                    )
                                })?;
                            write!(out, "e{}->c{index}=l{};", result.0, source.0)
                                .expect("writing to String cannot fail");
                        } else {
                            let converted = self.render_value_conversion(
                                &format!("v{}", capture.0),
                                &types[capture],
                                &parameter.ty,
                                instruction.span,
                            )?;
                            write!(out, "e{}->c{index}={converted};", result.0)
                                .expect("writing to String cannot fail");
                        }
                    }
                    write!(
                        out,
                        "v{}=(LilScriptClosure){{(void*)f{},e{}}};",
                        result.0, function.0, result.0
                    )
                    .expect("writing to String cannot fail");
                }
                return Ok(());
            }
            ControlFlowOp::FieldSet {
                object,
                owner,
                index,
                value,
                ..
            } => {
                let access = match types.get(object) {
                    Some(Type::Struct(_) | Type::StructInstance { .. }) => ".",
                    Some(Type::Class(_) | Type::ClassInstance { .. }) => "->",
                    _ => {
                        return Err(CodegenError::new(
                            instruction.span,
                            "native field store requires an aggregate",
                        ));
                    }
                };
                let storage = self.field_storage_type(owner, *index, instruction.span)?;
                let converted = self.render_value_conversion(
                    &format!("v{}", value.0),
                    &types[value],
                    storage,
                    instruction.span,
                )?;
                write!(out, "v{}{access}f{index}={converted};", object.0)
                    .expect("writing to String cannot fail");
                return Ok(());
            }
            ControlFlowOp::RecordFieldSet {
                object,
                property,
                value,
            } => {
                let Some(Type::Record(element)) = types.get(object) else {
                    return Err(CodegenError::new(
                        instruction.span,
                        "native record store requires a record",
                    ));
                };
                let boxed =
                    self.render_collection_value(*value, element, types, instruction.span)?;
                write!(
                    out,
                    "lilscript_map_set(v{},(LilScriptValue){{.tag=4,.s=\"{}\"}},{boxed});",
                    object.0, property
                )
                .expect("writing to String cannot fail");
                return Ok(());
            }
            ControlFlowOp::IndexSet {
                object,
                index,
                value,
            } => {
                if let Some(Type::Record(element)) = types.get(object) {
                    let key = self.render_collection_value(
                        *index,
                        &Type::String,
                        types,
                        instruction.span,
                    )?;
                    let value =
                        self.render_collection_value(*value, element, types, instruction.span)?;
                    write!(out, "lilscript_map_set(v{},{key},{value});", object.0)
                        .expect("writing to String cannot fail");
                    return Ok(());
                }
                if let Some(kind) = types.get(object).and_then(TypedArrayKind::from_type) {
                    if kind.element_is_float() {
                        let converted = self.render_value_conversion(
                            &format!("v{}", value.0),
                            &types[value],
                            &Type::Float,
                            instruction.span,
                        )?;
                        let setter = if matches!(kind, TypedArrayKind::Float64) {
                            "lilscript_ta_set_f64"
                        } else {
                            "lilscript_ta_set_f32"
                        };
                        write!(out, "{setter}(v{},v{},{converted});", object.0, index.0)
                            .expect("writing to String cannot fail");
                    } else {
                        let converted = self.render_value_conversion(
                            &format!("v{}", value.0),
                            &types[value],
                            &Type::Int,
                            instruction.span,
                        )?;
                        write!(
                            out,
                            "lilscript_ta_set_int(v{},v{},{converted},{});",
                            object.0,
                            index.0,
                            kind.native_kind_code()
                        )
                        .expect("writing to String cannot fail");
                    }
                    return Ok(());
                }
                if types
                    .get(object)
                    .is_some_and(|ty| is_float_vector_union(ty))
                {
                    let converted = self.render_value_conversion(
                        &format!("v{}", value.0),
                        &types[value],
                        &Type::Float,
                        instruction.span,
                    )?;
                    write!(
                        out,
                        "lilscript_float_union_set(v{},v{},{converted});",
                        object.0, index.0
                    )
                    .expect("writing to String cannot fail");
                    return Ok(());
                }
                let Some(Type::Array(element)) = types.get(object) else {
                    return Err(CodegenError::new(
                        instruction.span,
                        "indexed native store requires an array",
                    ));
                };
                let normalized = self.render_value_conversion(
                    &format!("v{}", value.0),
                    &types[value],
                    element,
                    instruction.span,
                )?;
                let converted = self.render_value_conversion(
                    &normalized,
                    element,
                    &Type::TypeParameter("$array"),
                    instruction.span,
                )?;
                write!(
                    out,
                    "((LilScriptValue*)v{}->data)[v{}]={converted};",
                    object.0, index.0
                )
                .expect("writing to String cannot fail");
                return Ok(());
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::ArrayMap,
                receiver: Some(receiver),
                args,
            } => {
                let callback = *args.first().ok_or_else(|| {
                    CodegenError::new(instruction.span, "array map requires a callback")
                })?;
                self.emit_array_map(instruction, *receiver, callback, types, out)?;
                return Ok(());
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::ArrayFilter,
                receiver: Some(receiver),
                args,
            } => {
                self.emit_array_filter(instruction, *receiver, args, types, out)?;
                return Ok(());
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::ArrayReduce,
                receiver: Some(receiver),
                args,
            } => {
                self.emit_array_reduce(instruction, *receiver, args, types, out)?;
                return Ok(());
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::ArrayForEach,
                receiver: Some(receiver),
                args,
            } => {
                self.emit_array_for_each(instruction, *receiver, args, types, out)?;
                return Ok(());
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::ArraySome | Intrinsic::ArrayEvery | Intrinsic::ArrayFindIndex,
                receiver: Some(receiver),
                args,
            } => {
                self.emit_array_predicate(instruction, *receiver, args, types, out)?;
                return Ok(());
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::ArrayPush | Intrinsic::ArrayPop,
                receiver: Some(receiver),
                args,
            } => {
                self.emit_array_mutation(instruction, *receiver, args, types, out)?;
                return Ok(());
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::ArrayFill,
                receiver: Some(receiver),
                args,
            } => {
                self.emit_array_fill(instruction, *receiver, args, types, out)?;
                return Ok(());
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::ArraySlice,
                receiver: Some(receiver),
                args,
            } => {
                let result = required_output(instruction)?;
                let (start, end) = match args.as_slice() {
                    [] => ("0".to_owned(), "INT32_MAX".to_owned()),
                    [start] => (format!("v{}", start.0), "INT32_MAX".to_owned()),
                    [start, end] => (format!("v{}", start.0), format!("v{}", end.0)),
                    _ => {
                        return Err(CodegenError::new(
                            instruction.span,
                            "array slice expects zero, one, or two arguments",
                        ));
                    }
                };
                write!(
                    out,
                    "v{}=lilscript_array_slice(v{},{start},{end},sizeof(LilScriptValue));",
                    result.0, receiver.0,
                )
                .expect("writing to String cannot fail");
                return Ok(());
            }
            ControlFlowOp::Template(parts) => {
                self.emit_template(instruction, parts, types, out)?;
                return Ok(());
            }
            ControlFlowOp::StoreGlobal { global, value } => {
                let storage = self
                    .module
                    .globals
                    .iter()
                    .find(|binding| binding.symbol == *global)
                    .map(|binding| &binding.ty)
                    .ok_or_else(|| {
                        CodegenError::new(instruction.span, "native global has no type")
                    })?;
                let converted = self.render_value_conversion(
                    &format!("v{}", value.0),
                    &types[value],
                    storage,
                    instruction.span,
                )?;
                write!(out, "g{}={converted};", global.0).expect("writing to String cannot fail");
                return Ok(());
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::Print,
                args,
                ..
            } => {
                let value = *args
                    .first()
                    .ok_or_else(|| CodegenError::new(instruction.span, "print requires a value"))?;
                self.emit_print(value, &types[&value], instruction.span, out)?;
                return Ok(());
            }
            _ => {}
        }

        let expression = self.render_instruction(owner, instruction, types)?;
        if instruction.ty.as_ref() == Some(&Type::Void) {
            if !expression.is_empty() {
                out.push_str(&expression);
                out.push(';');
            }
        } else if let Some(value) = instruction.out {
            write!(out, "v{}={expression};", value.0).expect("writing to String cannot fail");
        } else if !expression.is_empty() {
            out.push_str(&expression);
            out.push(';');
        }
        Ok(())
    }

    fn emit_array_map(
        &self,
        instruction: &ControlFlowInstruction<'src>,
        receiver: ValueId,
        callback: ValueId,
        types: &AHashMap<ValueId, Type<'src>>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let Some(Type::Function(signature)) = types.get(&callback) else {
            return Err(CodegenError::new(
                instruction.span,
                "array map callback has no function type",
            ));
        };
        if signature.return_type.is_void() {
            return self.emit_array_for_each(instruction, receiver, &[callback], types, out);
        }
        let result = required_output(instruction)?;
        let Some(Type::Array(input)) = types.get(&receiver) else {
            return Err(CodegenError::new(
                instruction.span,
                "array map receiver has no array type",
            ));
        };
        let Some(Type::Array(output)) = instruction.ty.as_ref() else {
            return Err(CodegenError::new(
                instruction.span,
                "array map output has no array type",
            ));
        };
        write!(
            out,
            "v{}=lilscript_array(v{}->len,sizeof(LilScriptValue));{{int32_t n=v{}->len;for(int32_t i=0;i<n;i++){{",
            result.0,
            receiver.0,
            receiver.0,
        )
        .expect("writing to String cannot fail");
        let item = self.render_value_conversion(
            &format!("((LilScriptValue*)v{}->data)[i]", receiver.0),
            &Type::TypeParameter("$array"),
            input,
            instruction.span,
        )?;
        let call = self.render_closure_call(
            callback,
            &[item],
            &[input.as_ref().clone()],
            signature,
            instruction.span,
        )?;
        let boxed = self.render_value_conversion(
            &call,
            output,
            &Type::TypeParameter("$array"),
            instruction.span,
        )?;
        write!(
            out,
            "((LilScriptValue*)v{}->data)[i]={boxed};}}}}",
            result.0
        )
        .expect("writing to String cannot fail");
        Ok(())
    }

    fn emit_array_filter(
        &self,
        instruction: &ControlFlowInstruction<'src>,
        receiver: ValueId,
        args: &[ValueId],
        types: &AHashMap<ValueId, Type<'src>>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let result = required_output(instruction)?;
        let callback = *args.first().ok_or_else(|| {
            CodegenError::new(instruction.span, "array filter requires a callback")
        })?;
        let Some(Type::Array(element)) = types.get(&receiver) else {
            return Err(CodegenError::new(
                instruction.span,
                "array filter receiver has no array type",
            ));
        };
        let signature = closure_signature(types, callback, instruction.span)?;
        write!(
            out,
            "v{}=lilscript_array(v{}->len,sizeof(LilScriptValue));v{}->len=0;{{int32_t n=v{}->len;for(int32_t i=0;i<n;i++){{",
            result.0, receiver.0, result.0, receiver.0
        )
        .expect("writing to String cannot fail");
        let boxed_item = format!("((LilScriptValue*)v{}->data)[i]", receiver.0);
        let item = self.render_value_conversion(
            &boxed_item,
            &Type::TypeParameter("$array"),
            element,
            instruction.span,
        )?;
        let call = self.render_closure_call(
            callback,
            std::slice::from_ref(&item),
            &[element.as_ref().clone()],
            signature,
            instruction.span,
        )?;
        write!(
            out,
            "if({call})((LilScriptValue*)v{}->data)[v{}->len++]={boxed_item};}}}}",
            result.0, result.0
        )
        .expect("writing to String cannot fail");
        Ok(())
    }

    fn emit_array_reduce(
        &self,
        instruction: &ControlFlowInstruction<'src>,
        receiver: ValueId,
        args: &[ValueId],
        types: &AHashMap<ValueId, Type<'src>>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let result = required_output(instruction)?;
        let [callback, initial] = args else {
            return Err(CodegenError::new(
                instruction.span,
                "array reduce requires a callback and initial value",
            ));
        };
        let Some(Type::Array(element)) = types.get(&receiver) else {
            return Err(CodegenError::new(
                instruction.span,
                "array reduce receiver has no array type",
            ));
        };
        let signature = closure_signature(types, *callback, instruction.span)?;
        write!(
            out,
            "v{}=v{};{{int32_t n=v{}->len;for(int32_t i=0;i<n;i++){{",
            result.0, initial.0, receiver.0
        )
        .expect("writing to String cannot fail");
        let args = [
            format!("v{}", result.0),
            self.render_value_conversion(
                &format!("((LilScriptValue*)v{}->data)[i]", receiver.0),
                &Type::TypeParameter("$array"),
                element,
                instruction.span,
            )?,
        ];
        let accumulator_type = instruction.ty.as_ref().ok_or_else(|| {
            CodegenError::new(instruction.span, "array reduce has no accumulator type")
        })?;
        let call = self.render_closure_call(
            *callback,
            &args,
            &[accumulator_type.clone(), element.as_ref().clone()],
            signature,
            instruction.span,
        )?;
        write!(out, "v{}={call};}}}}", result.0).expect("writing to String cannot fail");
        Ok(())
    }

    fn emit_array_for_each(
        &self,
        instruction: &ControlFlowInstruction<'src>,
        receiver: ValueId,
        args: &[ValueId],
        types: &AHashMap<ValueId, Type<'src>>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let callback = *args.first().ok_or_else(|| {
            CodegenError::new(instruction.span, "array forEach requires a callback")
        })?;
        let Some(Type::Array(element)) = types.get(&receiver) else {
            return Err(CodegenError::new(
                instruction.span,
                "array forEach receiver has no array type",
            ));
        };
        let signature = closure_signature(types, callback, instruction.span)?;
        write!(
            out,
            "{{int32_t n=v{}->len;for(int32_t i=0;i<n;i++){{",
            receiver.0
        )
        .expect("writing to String cannot fail");
        let item = self.render_value_conversion(
            &format!("((LilScriptValue*)v{}->data)[i]", receiver.0),
            &Type::TypeParameter("$array"),
            element,
            instruction.span,
        )?;
        let call = self.render_closure_call(
            callback,
            std::slice::from_ref(&item),
            &[element.as_ref().clone()],
            signature,
            instruction.span,
        )?;
        out.push_str(&call);
        out.push_str(";}}");
        Ok(())
    }

    fn emit_array_predicate(
        &self,
        instruction: &ControlFlowInstruction<'src>,
        receiver: ValueId,
        args: &[ValueId],
        types: &AHashMap<ValueId, Type<'src>>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let result = required_output(instruction)?;
        let callback = *args.first().ok_or_else(|| {
            CodegenError::new(instruction.span, "array predicate requires a callback")
        })?;
        let Some(Type::Array(element)) = types.get(&receiver) else {
            return Err(CodegenError::new(
                instruction.span,
                "array predicate receiver has no array type",
            ));
        };
        let signature = closure_signature(types, callback, instruction.span)?;
        let initial = match instruction.op {
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::ArraySome,
                ..
            } => "false",
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::ArrayEvery,
                ..
            } => "true",
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::ArrayFindIndex,
                ..
            } => "-1",
            _ => unreachable!(),
        };
        write!(
            out,
            "v{}={initial};{{int32_t n=v{}->len;for(int32_t i=0;i<n;i++){{if(i>=v{}->len)continue;",
            result.0, receiver.0, receiver.0
        )
        .expect("writing to String cannot fail");
        let item = self.render_value_conversion(
            &format!("((LilScriptValue*)v{}->data)[i]", receiver.0),
            &Type::TypeParameter("$array"),
            element,
            instruction.span,
        )?;
        let call = self.render_closure_call(
            callback,
            std::slice::from_ref(&item),
            &[element.as_ref().clone()],
            signature,
            instruction.span,
        )?;
        match instruction.op {
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::ArraySome,
                ..
            } => write!(out, "if({call}){{v{}=true;break;}}", result.0),
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::ArrayEvery,
                ..
            } => write!(out, "if(!({call})){{v{}=false;break;}}", result.0),
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::ArrayFindIndex,
                ..
            } => write!(out, "if({call}){{v{}=i;break;}}", result.0),
            _ => unreachable!(),
        }
        .expect("writing to String cannot fail");
        out.push_str("}}}");
        Ok(())
    }

    fn emit_array_mutation(
        &self,
        instruction: &ControlFlowInstruction<'src>,
        receiver: ValueId,
        args: &[ValueId],
        types: &AHashMap<ValueId, Type<'src>>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let result = required_output(instruction)?;
        let Some(Type::Array(element)) = types.get(&receiver) else {
            return Err(CodegenError::new(
                instruction.span,
                "array mutation receiver has no array type",
            ));
        };
        match instruction.op {
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::ArrayPush,
                ..
            } => {
                let value = args.first().ok_or_else(|| {
                    CodegenError::new(instruction.span, "array push requires a value")
                })?;
                let normalized = self.render_value_conversion(
                    &format!("v{}", value.0),
                    &types[value],
                    element,
                    instruction.span,
                )?;
                let boxed = self.render_value_conversion(
                    &normalized,
                    element,
                    &Type::TypeParameter("$array"),
                    instruction.span,
                )?;
                write!(
                    out,
                    "*(LilScriptValue*)lilscript_push(v{},sizeof(LilScriptValue))={boxed};v{}=v{}->len;",
                    receiver.0, result.0, receiver.0
                )
                .expect("writing to String cannot fail");
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::ArrayPop,
                ..
            } => {
                let popped = format!(
                    "*(LilScriptValue*)lilscript_pop(v{},sizeof(LilScriptValue))",
                    receiver.0
                );
                let converted = self.render_value_conversion(
                    &popped,
                    &Type::TypeParameter("$array"),
                    element,
                    instruction.span,
                )?;
                write!(out, "v{}={converted};", result.0).expect("writing to String cannot fail");
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    fn emit_array_fill(
        &self,
        instruction: &ControlFlowInstruction<'src>,
        receiver: ValueId,
        args: &[ValueId],
        types: &AHashMap<ValueId, Type<'src>>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let result = required_output(instruction)?;
        let [value] = args else {
            return Err(CodegenError::new(
                instruction.span,
                "array fill requires one value",
            ));
        };
        let Some(Type::Array(element)) = types.get(&receiver) else {
            return Err(CodegenError::new(
                instruction.span,
                "array fill receiver has no array type",
            ));
        };
        let normalized = self.render_value_conversion(
            &format!("v{}", value.0),
            &types[value],
            element,
            instruction.span,
        )?;
        let boxed = self.render_value_conversion(
            &normalized,
            element,
            &Type::TypeParameter("$array"),
            instruction.span,
        )?;
        write!(
            out,
            "{{LilScriptValue f={boxed};for(size_t i=0;i<v{}->len;i++)((LilScriptValue*)v{}->data)[i]=f;}}v{}=v{};",
            receiver.0, receiver.0, result.0, receiver.0
        )
        .expect("writing to String cannot fail");
        Ok(())
    }

    fn emit_template(
        &self,
        instruction: &ControlFlowInstruction<'src>,
        parts: &[TemplateOperand],
        types: &AHashMap<ValueId, Type<'src>>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let result = required_output(instruction)?;
        write!(out, "v{}=lilscript_dup(\"\");", result.0).expect("writing to String cannot fail");
        for part in parts {
            let value = match part {
                TemplateOperand::String(value) => format!("\"{value}\""),
                TemplateOperand::Value(value) => {
                    self.render_string_value(*value, &types[value], instruction.span)?
                }
            };
            write!(out, "v{}=lilscript_cat(v{},{value});", result.0, result.0)
                .expect("writing to String cannot fail");
        }
        Ok(())
    }

    fn render_collection_value(
        &self,
        value: ValueId,
        expected: &Type<'src>,
        types: &AHashMap<ValueId, Type<'src>>,
        span: Span,
    ) -> Result<String, CodegenError> {
        let normalized =
            self.render_value_conversion(&format!("v{}", value.0), &types[&value], expected, span)?;
        self.render_value_conversion(
            &normalized,
            expected,
            &Type::TypeParameter("$collection"),
            span,
        )
    }

    fn render_instruction(
        &self,
        owner: &ControlFlowFunction<'src>,
        instruction: &ControlFlowInstruction<'src>,
        types: &AHashMap<ValueId, Type<'src>>,
    ) -> Result<String, CodegenError> {
        let value = |value: ValueId| format!("v{}", value.0);
        Ok(match &instruction.op {
            ControlFlowOp::Const(value) => render_c_const(value),
            ControlFlowOp::Unary { op, value: operand } => match (op, instruction.ty.as_ref()) {
                (IrUnaryOp::Neg, Some(Type::Int)) => {
                    format!("(int32_t)(0u-(uint32_t){})", value(*operand))
                }
                (IrUnaryOp::Neg, _) => format!("-{}", value(*operand)),
                (IrUnaryOp::Not, _) => format!("!{}", value(*operand)),
            },
            ControlFlowOp::Binary { op, lhs, rhs } => self.render_binary(
                *op,
                *lhs,
                *rhs,
                instruction.ty.as_ref(),
                types,
                instruction.span,
            )?,
            ControlFlowOp::TypeCheck {
                value: input,
                target,
            } => self.render_type_check(*input, &types[input], target, instruction.span)?,
            ControlFlowOp::Struct { name, fields } => {
                let record = aggregate_type_name("Struct", name);
                let layout = self
                    .module
                    .structs
                    .iter()
                    .find(|layout| layout.name == *name)
                    .ok_or_else(|| {
                        CodegenError::new(instruction.span, "missing native struct layout")
                    })?;
                let mut value = format!("({record}){{");
                for (index, (field, storage)) in fields.iter().zip(&layout.fields).enumerate() {
                    if index != 0 {
                        value.push(',');
                    }
                    value.push_str(&self.render_value_conversion(
                        &format!("v{}", field.0),
                        &types[field],
                        &storage.ty,
                        instruction.span,
                    )?);
                }
                value.push('}');
                value
            }
            ControlFlowOp::Record(entries) => {
                if entries.is_empty() {
                    "lilscript_map()".to_string()
                } else {
                    let Some(Type::Record(element)) = instruction.ty.as_ref() else {
                        return Err(CodegenError::new(
                            instruction.span,
                            "native record literal has no record type",
                        ));
                    };
                    let mut keys = String::new();
                    let mut values = String::new();
                    for (index, (key, value)) in entries.iter().enumerate() {
                        if index != 0 {
                            keys.push(',');
                            values.push(',');
                        }
                        write!(keys, "(LilScriptValue){{.tag=4,.s=\"{key}\"}}")
                            .expect("writing to String cannot fail");
                        values.push_str(&self.render_collection_value(
                            *value,
                            element,
                            types,
                            instruction.span,
                        )?);
                    }
                    format!(
                        "lilscript_record({},(LilScriptValue[]){{{keys}}},(LilScriptValue[]){{{values}}})",
                        entries.len()
                    )
                }
            }
            ControlFlowOp::ArraySpread(operands) => {
                let Some(Type::Array(element)) = instruction.ty.as_ref() else {
                    return Err(CodegenError::new(
                        instruction.span,
                        "native array spread has no array type",
                    ));
                };
                let mut kinds = String::new();
                let mut values = String::new();
                for (index, operand) in operands.iter().enumerate() {
                    if index != 0 {
                        kinds.push(',');
                        values.push(',');
                    }
                    match operand {
                        ArrayOperand::Value(value) => {
                            kinds.push('0');
                            values.push_str(&self.render_collection_value(
                                *value,
                                element,
                                types,
                                instruction.span,
                            )?);
                        }
                        ArrayOperand::Spread(value) => {
                            kinds.push('1');
                            write!(values, "(LilScriptValue){{.tag=6,.p=v{}}}", value.0)
                                .expect("writing to String cannot fail");
                        }
                    }
                }
                format!(
                    "lilscript_array_spread({},(uint8_t[]){{{kinds}}},(LilScriptValue[]){{{values}}})",
                    operands.len()
                )
            }
            ControlFlowOp::RecordSpread(operands) => {
                let Some(Type::Record(element)) = instruction.ty.as_ref() else {
                    return Err(CodegenError::new(
                        instruction.span,
                        "native record spread has no record type",
                    ));
                };
                let mut kinds = String::new();
                let mut keys = String::new();
                let mut values = String::new();
                for (index, operand) in operands.iter().enumerate() {
                    if index != 0 {
                        kinds.push(',');
                        keys.push(',');
                        values.push(',');
                    }
                    match operand {
                        RecordOperand::Entry(key, value) => {
                            kinds.push('0');
                            write!(keys, "(LilScriptValue){{.tag=4,.s=\"{key}\"}}")
                                .expect("writing to String cannot fail");
                            values.push_str(&self.render_collection_value(
                                *value,
                                element,
                                types,
                                instruction.span,
                            )?);
                        }
                        RecordOperand::Spread(value) => {
                            kinds.push('1');
                            keys.push_str("(LilScriptValue){0}");
                            write!(values, "(LilScriptValue){{.tag=6,.p=v{}}}", value.0)
                                .expect("writing to String cannot fail");
                        }
                    }
                }
                format!(
                    "lilscript_record_spread({},(uint8_t[]){{{kinds}}},(LilScriptValue[]){{{keys}}},(LilScriptValue[]){{{values}}})",
                    operands.len()
                )
            }
            ControlFlowOp::LoadGlobal(symbol) => format!("g{}", symbol.0),
            ControlFlowOp::CallDirect { function, args, .. } => self.render_direct_call(
                *function,
                args,
                types,
                instruction.ty.as_ref(),
                instruction.span,
            )?,
            ControlFlowOp::CallValue { callee, args } => {
                let Some(Type::Function(signature)) = types.get(callee) else {
                    return Err(CodegenError::new(
                        instruction.span,
                        "native indirect call has no function type",
                    ));
                };
                let arg_types = args
                    .iter()
                    .map(|value| types[value].clone())
                    .collect::<Vec<_>>();
                let args = args
                    .iter()
                    .map(|value| format!("v{}", value.0))
                    .collect::<Vec<_>>();
                self.render_closure_call(*callee, &args, &arg_types, signature, instruction.span)?
            }
            ControlFlowOp::IndexGet { object, index } => match types.get(object) {
                Some(Type::Array(element)) => self.render_value_conversion(
                    &format!("((LilScriptValue*)v{}->data)[v{}]", object.0, index.0),
                    &Type::TypeParameter("$array"),
                    element,
                    instruction.span,
                )?,
                Some(Type::Record(_)) => {
                    let key = self.render_collection_value(
                        *index,
                        &Type::String,
                        types,
                        instruction.span,
                    )?;
                    format!("lilscript_map_get(v{},{key})", object.0)
                }
                Some(ty) if let Some(kind) = TypedArrayKind::from_type(ty) => {
                    if kind.element_is_float() {
                        let getter = if matches!(kind, TypedArrayKind::Float64) {
                            "lilscript_ta_get_f64"
                        } else {
                            "lilscript_ta_get_f32"
                        };
                        format!("{getter}(v{},v{})", object.0, index.0)
                    } else {
                        format!(
                            "lilscript_ta_get_int(v{},v{},{})",
                            object.0,
                            index.0,
                            kind.native_kind_code()
                        )
                    }
                }
                Some(Type::String) => {
                    format!("lilscript_char_at(v{},v{})", object.0, index.0)
                }
                Some(ty) if is_float_vector_union(ty) => {
                    format!("lilscript_float_union_get(v{},v{})", object.0, index.0)
                }
                _ => {
                    return Err(CodegenError::new(
                        instruction.span,
                        "indexed native load requires an array, typed array, or string",
                    ));
                }
            },
            ControlFlowOp::ArrayGetOptional { object, index } => format!(
                "{}<v{}->len?lilscript_value_optional(((LilScriptValue*)v{}->data)[{}]):(LilScriptOptional){{false,{{0}}}}",
                index, object.0, object.0, index
            ),
            ControlFlowOp::FieldGet {
                object,
                owner,
                index,
                ..
            } => {
                let access = match types.get(object) {
                    Some(Type::Struct(_) | Type::StructInstance { .. }) => ".",
                    Some(Type::Class(_) | Type::ClassInstance { .. }) => "->",
                    _ => {
                        return Err(CodegenError::new(
                            instruction.span,
                            "native field load requires an aggregate",
                        ));
                    }
                };
                let storage = self.field_storage_type(owner, *index, instruction.span)?;
                let loaded = format!("v{}{access}f{index}", object.0);
                self.render_value_conversion(
                    &loaded,
                    storage,
                    instruction.ty.as_ref().ok_or_else(|| {
                        CodegenError::new(instruction.span, "native field load has no type")
                    })?,
                    instruction.span,
                )?
            }
            ControlFlowOp::RecordFieldGet { object, property } => format!(
                "lilscript_map_get(v{},(LilScriptValue){{.tag=4,.s=\"{property}\"}})",
                object.0
            ),
            ControlFlowOp::RecordRest { object, excluded } => {
                let keys = excluded
                    .iter()
                    .map(|key| format!("(LilScriptValue){{.tag=4,.s=\"{key}\"}}"))
                    .collect::<Vec<_>>()
                    .join(",");
                format!(
                    "lilscript_record_rest(v{},{},(LilScriptValue[]){{{keys}}})",
                    object.0,
                    excluded.len()
                )
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::IntImul,
                args,
                ..
            } => {
                let [lhs, rhs] = args.as_slice() else {
                    return Err(CodegenError::new(
                        instruction.span,
                        "Math.imul requires two integer arguments",
                    ));
                };
                format!("(int32_t)((uint32_t)v{}*(uint32_t)v{})", lhs.0, rhs.0)
            }
            ControlFlowOp::Intrinsic {
                intrinsic: intrinsic @ (Intrinsic::IntToString | Intrinsic::IntToUnsignedString),
                receiver: Some(receiver),
                args,
            } => {
                let radix = args
                    .first()
                    .map_or_else(|| "10".to_string(), |radix| format!("v{}", radix.0));
                format!(
                    "lilscript_i32_radix(v{},{radix},{})",
                    receiver.0,
                    matches!(intrinsic, Intrinsic::IntToUnsignedString)
                )
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::MapNew,
                ..
            } => "lilscript_map()".to_string(),
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::SetNew,
                ..
            } => "lilscript_set()".to_string(),
            ControlFlowOp::Intrinsic {
                intrinsic: intrinsic @ (Intrinsic::ArrayBufferNew | Intrinsic::SharedArrayBufferNew),
                args,
                ..
            } => {
                let length = args.first().ok_or_else(|| {
                    CodegenError::new(instruction.span, "buffer constructor requires a length")
                })?;
                format!(
                    "lilscript_buffer(v{},{})",
                    length.0,
                    matches!(intrinsic, Intrinsic::SharedArrayBufferNew)
                )
            }
            ControlFlowOp::Intrinsic {
                intrinsic, args, ..
            } if matches!(
                classify_typed_array_intrinsic(*intrinsic),
                Some((_, TypedArrayIntrinsic::New))
            ) =>
            {
                let Some((kind, _)) = classify_typed_array_intrinsic(*intrinsic) else {
                    unreachable!()
                };
                let source = args.first().ok_or_else(|| {
                    CodegenError::new(
                        instruction.span,
                        format!("{} constructor requires a source", kind.name()),
                    )
                })?;
                let bpe = kind.bytes_per_element();
                match types.get(source) {
                    Some(Type::Int) => format!("lilscript_ta_length(v{},{bpe})", source.0),
                    Some(Type::ArrayBuffer | Type::SharedArrayBuffer) => {
                        format!("lilscript_ta_buffer(v{},{bpe})", source.0)
                    }
                    _ => {
                        return Err(CodegenError::new(
                            instruction.span,
                            format!(
                                "{} constructor has an unsupported native source",
                                kind.name()
                            ),
                        ));
                    }
                }
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::SymbolNew,
                args,
                ..
            } => {
                if let Some(description) = args.first() {
                    let rendered = self.render_string_value(
                        *description,
                        &types[description],
                        instruction.span,
                    )?;
                    format!("lilscript_symbol({rendered})")
                } else {
                    "lilscript_symbol(NULL)".to_string()
                }
            }
            ControlFlowOp::Intrinsic {
                intrinsic: intrinsic @ (Intrinsic::ArrayIndexOf | Intrinsic::ArrayIncludes),
                receiver: Some(receiver),
                args,
            } => {
                let needle = args.first().ok_or_else(|| {
                    CodegenError::new(instruction.span, "array search requires a value")
                })?;
                let Some(Type::Array(element)) = types.get(receiver) else {
                    return Err(CodegenError::new(
                        instruction.span,
                        "array search receiver has no array type",
                    ));
                };
                let needle =
                    self.render_collection_value(*needle, element, types, instruction.span)?;
                let from = if matches!(intrinsic, Intrinsic::ArrayIncludes) {
                    args.get(1)
                        .map_or_else(|| "0".to_string(), |value| format!("v{}", value.0))
                } else {
                    "0".to_string()
                };
                let call = format!(
                    "lilscript_array_find_value(v{},{needle},{from},{})",
                    receiver.0,
                    matches!(intrinsic, Intrinsic::ArrayIncludes)
                );
                if matches!(intrinsic, Intrinsic::ArrayIncludes) {
                    format!("{call}>=0")
                } else {
                    call
                }
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::ArrayJoin,
                receiver: Some(receiver),
                args,
            } => {
                let separator = args
                    .first()
                    .map_or_else(|| "\",\"".to_string(), |value| format!("v{}", value.0));
                format!("lilscript_array_join(v{},{separator})", receiver.0)
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::ArrayConcat,
                receiver: Some(receiver),
                args,
            } => {
                let other = args.first().ok_or_else(|| {
                    CodegenError::new(instruction.span, "array concat requires another array")
                })?;
                format!("lilscript_array_concat(v{},v{})", receiver.0, other.0)
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::ArrayCopyWithin,
                receiver: Some(receiver),
                args,
            } => {
                let [target, start, rest @ ..] = args.as_slice() else {
                    return Err(CodegenError::new(
                        instruction.span,
                        "array copyWithin requires target and start",
                    ));
                };
                if rest.len() > 1 {
                    return Err(CodegenError::new(
                        instruction.span,
                        "array copyWithin accepts at most three arguments",
                    ));
                }
                let end = rest
                    .first()
                    .map_or_else(|| "INT32_MAX".to_string(), |value| format!("v{}", value.0));
                format!(
                    "lilscript_array_copy_within(v{},v{},v{},{end})",
                    receiver.0, target.0, start.0
                )
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::ArrayReverse,
                receiver: Some(receiver),
                args,
            } => {
                if !args.is_empty() {
                    return Err(CodegenError::new(
                        instruction.span,
                        "array reverse takes no arguments",
                    ));
                }
                format!("lilscript_array_reverse(v{})", receiver.0)
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::TypedArraySet,
                receiver: Some(receiver),
                args,
            } => {
                let source = args.first().ok_or_else(|| {
                    CodegenError::new(instruction.span, "typed-array set requires a source")
                })?;
                let kind = types
                    .get(receiver)
                    .and_then(TypedArrayKind::from_type)
                    .ok_or_else(|| {
                        CodegenError::new(instruction.span, "typed-array set has no view type")
                    })?;
                let offset = args
                    .get(1)
                    .map_or_else(|| "0".to_string(), |value| format!("v{}", value.0));
                format!(
                    "lilscript_ta_set(v{},v{},{offset},{})",
                    receiver.0,
                    source.0,
                    kind.bytes_per_element()
                )
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::TypedArrayFill,
                receiver: Some(receiver),
                args,
            } => {
                let value = args.first().ok_or_else(|| {
                    CodegenError::new(instruction.span, "typed-array fill requires a value")
                })?;
                let kind = types
                    .get(receiver)
                    .and_then(TypedArrayKind::from_type)
                    .ok_or_else(|| {
                        CodegenError::new(instruction.span, "typed-array fill has no view type")
                    })?;
                let start = args
                    .get(1)
                    .map_or_else(|| "0".to_string(), |value| format!("v{}", value.0));
                let end = args
                    .get(2)
                    .map_or_else(|| "INT32_MAX".to_string(), |value| format!("v{}", value.0));
                if kind.element_is_float() {
                    format!(
                        "lilscript_ta_fill_f(v{},v{},{start},{end},{})",
                        receiver.0,
                        value.0,
                        kind.native_kind_code()
                    )
                } else {
                    format!(
                        "lilscript_ta_fill_i(v{},v{},{start},{end},{})",
                        receiver.0,
                        value.0,
                        kind.native_kind_code()
                    )
                }
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::TypedArrayCopyWithin,
                receiver: Some(receiver),
                args,
            } => {
                let [target, start, rest @ ..] = args.as_slice() else {
                    return Err(CodegenError::new(
                        instruction.span,
                        "typed-array copyWithin requires target and start",
                    ));
                };
                if rest.len() > 1 {
                    return Err(CodegenError::new(
                        instruction.span,
                        "typed-array copyWithin accepts at most three arguments",
                    ));
                }
                let kind = types
                    .get(receiver)
                    .and_then(TypedArrayKind::from_type)
                    .ok_or_else(|| {
                        CodegenError::new(
                            instruction.span,
                            "typed-array copyWithin has no view type",
                        )
                    })?;
                let end = rest
                    .first()
                    .map_or_else(|| "INT32_MAX".to_string(), |value| format!("v{}", value.0));
                format!(
                    "lilscript_ta_copy_within(v{},v{},v{},{end},{})",
                    receiver.0,
                    target.0,
                    start.0,
                    kind.bytes_per_element()
                )
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::MapSize | Intrinsic::SetSize,
                receiver: Some(receiver),
                ..
            } => format!("v{}->len", receiver.0),
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::BufferByteLength,
                receiver: Some(receiver),
                ..
            } => format!("v{}->len", receiver.0),
            ControlFlowOp::Intrinsic {
                intrinsic,
                receiver: Some(receiver),
                ..
            } if matches!(
                classify_typed_array_intrinsic(*intrinsic),
                Some((_, TypedArrayIntrinsic::Length))
            ) =>
            {
                format!("v{}->len", receiver.0)
            }
            ControlFlowOp::Intrinsic {
                intrinsic,
                receiver: Some(receiver),
                ..
            } if matches!(
                classify_typed_array_intrinsic(*intrinsic),
                Some((_, TypedArrayIntrinsic::ByteLength))
            ) =>
            {
                let Some((kind, _)) = classify_typed_array_intrinsic(*intrinsic) else {
                    unreachable!()
                };
                let bpe = kind.bytes_per_element();
                if bpe == 1 {
                    format!("v{}->len", receiver.0)
                } else {
                    format!("v{}->len*{bpe}", receiver.0)
                }
            }
            ControlFlowOp::Intrinsic {
                intrinsic,
                receiver: Some(receiver),
                ..
            } if matches!(
                classify_typed_array_intrinsic(*intrinsic),
                Some((_, TypedArrayIntrinsic::ByteOffset))
            ) =>
            {
                format!("v{}->offset", receiver.0)
            }
            ControlFlowOp::Intrinsic {
                intrinsic,
                receiver: Some(receiver),
                ..
            } if matches!(
                classify_typed_array_intrinsic(*intrinsic),
                Some((_, TypedArrayIntrinsic::Buffer))
            ) =>
            {
                format!("(LilScriptValue){{.tag=6,.p=v{}->buffer}}", receiver.0)
            }
            ControlFlowOp::Intrinsic {
                intrinsic:
                    intrinsic @ (Intrinsic::MapGet | Intrinsic::MapHas | Intrinsic::MapDelete),
                receiver: Some(receiver),
                args,
            } => {
                let key = args.first().ok_or_else(|| {
                    CodegenError::new(instruction.span, "map operation requires a key")
                })?;
                let Some(Type::Map(key_type, _)) = types.get(receiver) else {
                    return Err(CodegenError::new(
                        instruction.span,
                        "map operation receiver has no map type",
                    ));
                };
                let key = self.render_collection_value(*key, key_type, types, instruction.span)?;
                let function = match intrinsic {
                    Intrinsic::MapGet => "lilscript_map_get",
                    Intrinsic::MapHas => "lilscript_map_has",
                    Intrinsic::MapDelete => "lilscript_map_delete",
                    _ => unreachable!(),
                };
                let call = format!("{function}(v{},{key})", receiver.0);
                if matches!(intrinsic, Intrinsic::MapGet)
                    && instruction.ty.as_ref().is_some_and(|ty| is_erased_type(ty))
                {
                    format!("lilscript_optional_value({call})")
                } else {
                    call
                }
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::MapSet,
                receiver: Some(receiver),
                args,
            } => {
                let [key, value] = args.as_slice() else {
                    return Err(CodegenError::new(
                        instruction.span,
                        "map set requires a key and value",
                    ));
                };
                let Some(Type::Map(key_type, value_type)) = types.get(receiver) else {
                    return Err(CodegenError::new(
                        instruction.span,
                        "map set receiver has no map type",
                    ));
                };
                let key = self.render_collection_value(*key, key_type, types, instruction.span)?;
                let value =
                    self.render_collection_value(*value, value_type, types, instruction.span)?;
                format!("lilscript_map_set(v{},{key},{value})", receiver.0)
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::MapClear,
                receiver: Some(receiver),
                ..
            } => format!("lilscript_map_clear(v{})", receiver.0),
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::RecordKeys,
                args,
                ..
            } => {
                let [record] = args.as_slice() else {
                    return Err(CodegenError::new(
                        instruction.span,
                        "Object.keys requires one record",
                    ));
                };
                format!("lilscript_record_keys(v{})", record.0)
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::RecordValues,
                args,
                ..
            } => {
                let [record] = args.as_slice() else {
                    return Err(CodegenError::new(
                        instruction.span,
                        "Object.values requires one record",
                    ));
                };
                format!("lilscript_record_values(v{})", record.0)
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::RecordHasOwn,
                args,
                ..
            } => {
                let [record, key] = args.as_slice() else {
                    return Err(CodegenError::new(
                        instruction.span,
                        "Object.hasOwn requires a record and key",
                    ));
                };
                let key = self.render_collection_value(
                    *key,
                    &Type::String,
                    types,
                    instruction.span,
                )?;
                format!("lilscript_map_has(v{},{key})", record.0)
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::RecordAssign,
                args,
                ..
            } => {
                let [target, source] = args.as_slice() else {
                    return Err(CodegenError::new(
                        instruction.span,
                        "Object.assign requires two records",
                    ));
                };
                format!("lilscript_record_assign(v{},v{})", target.0, source.0)
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::JsonStringify,
                args,
                ..
            } => {
                let [argument] = args.as_slice() else {
                    return Err(CodegenError::new(
                        instruction.span,
                        "JSON.stringify requires one value",
                    ));
                };
                match &types[argument] {
                    Type::Array(_) => format!("lilscript_json_array(v{})", argument.0),
                    Type::Record(_) => format!("lilscript_json_record(v{})", argument.0),
                    ty => {
                        let value = self.render_collection_value(
                            *argument,
                            ty,
                            types,
                            instruction.span,
                        )?;
                        format!("lilscript_json_value({value})")
                    }
                }
            }
            ControlFlowOp::Intrinsic {
                intrinsic:
                    intrinsic @ (Intrinsic::SetAdd | Intrinsic::SetHas | Intrinsic::SetDelete),
                receiver: Some(receiver),
                args,
            } => {
                let value = args.first().ok_or_else(|| {
                    CodegenError::new(instruction.span, "set operation requires a value")
                })?;
                let Some(Type::Set(element)) = types.get(receiver) else {
                    return Err(CodegenError::new(
                        instruction.span,
                        "set operation receiver has no set type",
                    ));
                };
                let value =
                    self.render_collection_value(*value, element, types, instruction.span)?;
                let function = match intrinsic {
                    Intrinsic::SetAdd => "lilscript_set_add",
                    Intrinsic::SetHas => "lilscript_set_has",
                    Intrinsic::SetDelete => "lilscript_set_delete",
                    _ => unreachable!(),
                };
                format!("{function}(v{},{value})", receiver.0)
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::SetClear,
                receiver: Some(receiver),
                ..
            } => format!("lilscript_set_clear(v{})", receiver.0),
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::BufferSlice,
                receiver: Some(receiver),
                args,
            } => {
                let [start, end] = args.as_slice() else {
                    return Err(CodegenError::new(
                        instruction.span,
                        "buffer slice requires start and end offsets",
                    ));
                };
                format!(
                    "lilscript_buffer_slice(v{},v{},v{})",
                    receiver.0, start.0, end.0
                )
            }
            ControlFlowOp::Intrinsic {
                intrinsic,
                receiver: Some(receiver),
                args,
            } if matches!(
                classify_typed_array_intrinsic(*intrinsic),
                Some((
                    _,
                    TypedArrayIntrinsic::Slice | TypedArrayIntrinsic::Subarray
                ))
            ) =>
            {
                let Some((kind, op)) = classify_typed_array_intrinsic(*intrinsic) else {
                    unreachable!()
                };
                let [start, end] = args.as_slice() else {
                    return Err(CodegenError::new(
                        instruction.span,
                        format!(
                            "{} range operation requires start and end offsets",
                            kind.name()
                        ),
                    ));
                };
                let bpe = kind.bytes_per_element();
                let function = if matches!(op, TypedArrayIntrinsic::Slice) {
                    "lilscript_ta_slice"
                } else {
                    "lilscript_ta_subarray"
                };
                format!("{function}(v{},v{},v{},{bpe})", receiver.0, start.0, end.0)
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::UnwrapNullable | Intrinsic::UnwrapUnion,
                receiver: Some(receiver),
                ..
            } => self.render_value_conversion(
                &format!("v{}", receiver.0),
                &types[receiver],
                instruction.ty.as_ref().ok_or_else(|| {
                    CodegenError::new(instruction.span, "typed unwrap has no output type")
                })?,
                instruction.span,
            )?,
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::ArrayLength,
                receiver: Some(receiver),
                ..
            } => {
                if types
                    .get(receiver)
                    .is_some_and(|ty| is_float_vector_union(ty))
                {
                    format!("lilscript_float_union_len(v{})", receiver.0)
                } else {
                    format!("v{}->len", receiver.0)
                }
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::StringLength,
                receiver: Some(receiver),
                ..
            } => format!("lilscript_utf16_len(v{})", receiver.0),
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::FloatToInt,
                receiver: Some(receiver),
                ..
            } => format!("lilscript_to_i32(v{})", receiver.0),
            ControlFlowOp::Intrinsic {
                intrinsic:
                    Intrinsic::FloatAbs
                    | Intrinsic::FloatFloor
                    | Intrinsic::FloatCeil
                    | Intrinsic::FloatRound
                    | Intrinsic::FloatSqrt
                    | Intrinsic::FloatSin
                    | Intrinsic::FloatCos
                    | Intrinsic::FloatAcos
                    | Intrinsic::FloatExp
                    | Intrinsic::FloatLog
                    | Intrinsic::FloatTan
                    | Intrinsic::FloatAtan2
                    | Intrinsic::FloatHypot
                    | Intrinsic::FloatMin
                    | Intrinsic::FloatMax,
                receiver: Some(receiver),
                args,
            } => {
                let function = match &instruction.op {
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::FloatAbs,
                        ..
                    } => "fabs",
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::FloatFloor,
                        ..
                    } => "floor",
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::FloatCeil,
                        ..
                    } => "ceil",
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::FloatRound,
                        ..
                    } => "lilscript_round",
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::FloatSqrt,
                        ..
                    } => "sqrt",
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::FloatSin,
                        ..
                    } => "sin",
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::FloatCos,
                        ..
                    } => "cos",
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::FloatAcos,
                        ..
                    } => "acos",
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::FloatExp,
                        ..
                    } => "exp",
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::FloatLog,
                        ..
                    } => "log",
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::FloatTan,
                        ..
                    } => "tan",
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::FloatAtan2,
                        ..
                    } => "atan2",
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::FloatHypot,
                        ..
                    } => "hypot",
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::FloatMin,
                        ..
                    } => "lilscript_fmin",
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::FloatMax,
                        ..
                    } => "lilscript_fmax",
                    _ => unreachable!(),
                };
                if let Some(argument) = args.first() {
                    format!("{function}(v{},v{})", receiver.0, argument.0)
                } else {
                    format!("{function}(v{})", receiver.0)
                }
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::StringCharCodeAt,
                receiver: Some(receiver),
                args,
            } => {
                let index = args.first().ok_or_else(|| {
                    CodegenError::new(instruction.span, "charCodeAt requires an index")
                })?;
                format!("lilscript_char_code_at(v{},v{})", receiver.0, index.0)
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::StringCharAt,
                receiver: Some(receiver),
                args,
            } => {
                let index = args.first().ok_or_else(|| {
                    CodegenError::new(instruction.span, "charAt requires an index")
                })?;
                format!("lilscript_char_at(v{},v{})", receiver.0, index.0)
            }
            ControlFlowOp::Intrinsic {
                intrinsic: intrinsic @ (Intrinsic::StringIndexOf | Intrinsic::StringLastIndexOf),
                receiver: Some(receiver),
                args,
            } => {
                let needle = args.first().ok_or_else(|| {
                    CodegenError::new(instruction.span, "string search requires a needle")
                })?;
                let last = matches!(intrinsic, Intrinsic::StringLastIndexOf);
                let position = args.get(1).map_or_else(
                    || {
                        if last {
                            "INT32_MAX".to_string()
                        } else {
                            "0".to_string()
                        }
                    },
                    |value| format!("v{}", value.0),
                );
                format!(
                    "lilscript_string_index(v{},v{},{position},{last})",
                    receiver.0, needle.0
                )
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::StringRepeat,
                receiver: Some(receiver),
                args,
            } => {
                let count = args.first().ok_or_else(|| {
                    CodegenError::new(instruction.span, "string repeat requires a count")
                })?;
                format!("lilscript_repeat(v{},v{})", receiver.0, count.0)
            }
            ControlFlowOp::Intrinsic {
                intrinsic:
                    intrinsic @ (Intrinsic::StringIncludes
                    | Intrinsic::StringStartsWith
                    | Intrinsic::StringEndsWith),
                receiver: Some(receiver),
                args,
            } => {
                let arg = args.first().ok_or_else(|| {
                    CodegenError::new(instruction.span, "string method requires an argument")
                })?;
                match intrinsic {
                    Intrinsic::StringIncludes => {
                        format!("strstr(v{},v{})!=NULL", receiver.0, arg.0)
                    }
                    Intrinsic::StringStartsWith => {
                        format!("strncmp(v{},v{},strlen(v{}))==0", receiver.0, arg.0, arg.0)
                    }
                    Intrinsic::StringEndsWith => {
                        format!("lilscript_ends(v{},v{})", receiver.0, arg.0)
                    }
                    _ => unreachable!(),
                }
            }
            ControlFlowOp::Intrinsic {
                intrinsic: intrinsic @ (Intrinsic::StringToUpperCase | Intrinsic::StringToLowerCase),
                receiver: Some(receiver),
                ..
            } => format!(
                "lilscript_case(v{}, {})",
                receiver.0,
                matches!(intrinsic, Intrinsic::StringToUpperCase)
            ),
            // CaptureLocal is consumed by closure-environment construction.
            // Its nominal SSA output still has the binding's value type, so
            // materialize the current value for any analysis/debug use rather
            // than assigning a cell pointer into that scalar slot.
            ControlFlowOp::CaptureLocal(local) => format!("*l{}", local.0),
            ControlFlowOp::LoadLocal(local) if owner.mutable_capture_locals.contains(local) => {
                format!("*l{}", local.0)
            }
            ControlFlowOp::StoreLocal { .. } | ControlFlowOp::LoadLocal(_) => {
                return Err(CodegenError::new(
                    instruction.span,
                    "native backend received ordinary locals before SSA promotion",
                ));
            }
            _ => {
                return Err(CodegenError::new(
                    instruction.span,
                    "operation is not supported by the native backend yet",
                ));
            }
        })
    }

    fn render_closure_call(
        &self,
        callee: ValueId,
        args: &[String],
        arg_types: &[Type<'src>],
        signature: &crate::semantic::FunctionType<'src>,
        span: Span,
    ) -> Result<String, CodegenError> {
        if args.len() != signature.params.len() || args.len() != arg_types.len() {
            return Err(CodegenError::new(
                span,
                "native closure call argument count does not match its signature",
            ));
        }
        let universal = Type::TypeParameter("$closure");
        let mut cast = String::from("LilScriptValue(*)(void*");
        for _ in &signature.params {
            cast.push_str(",LilScriptValue");
        }
        cast.push(')');
        let mut call = format!("((({cast})v{}.fn)(v{}.env", callee.0, callee.0);
        for ((arg, actual), parameter) in args.iter().zip(arg_types).zip(&signature.params) {
            call.push(',');
            let normalized = self.render_value_conversion(arg, actual, parameter, span)?;
            call.push_str(&self.render_value_conversion(
                &normalized,
                parameter,
                &universal,
                span,
            )?);
        }
        call.push_str("))");
        if signature.return_type.as_ref() == &Type::Void {
            Ok(call)
        } else {
            self.render_value_conversion(&call, &universal, &signature.return_type, span)
        }
    }

    fn render_direct_call(
        &self,
        function: FunctionId,
        args: &[ValueId],
        types: &AHashMap<ValueId, Type<'src>>,
        output_type: Option<&Type<'src>>,
        span: Span,
    ) -> Result<String, CodegenError> {
        let callee = self.function(function)?;
        if callee.params.len() != args.len() {
            return Err(CodegenError::new(
                span,
                "native direct call argument count does not match its function",
            ));
        }
        let mut call = format!("{}(", self.native_function_name(function)?);
        for (index, (argument, parameter)) in args.iter().zip(&callee.params).enumerate() {
            if index != 0 {
                call.push(',');
            }
            let source = types.get(argument).ok_or_else(|| {
                CodegenError::new(span, "native direct call argument has no type")
            })?;
            call.push_str(&self.render_value_conversion(
                &format!("v{}", argument.0),
                source,
                &parameter.ty,
                span,
            )?);
        }
        call.push(')');
        let Some(output_type) = output_type else {
            return Ok(call);
        };
        if output_type == &Type::Void || callee.return_type == Type::Void {
            return Ok(call);
        }
        self.render_value_conversion(&call, &callee.return_type, output_type, span)
    }

    fn field_storage_type(
        &self,
        owner: &str,
        index: usize,
        span: Span,
    ) -> Result<&Type<'src>, CodegenError> {
        self.module
            .structs
            .iter()
            .chain(&self.module.classes)
            .find(|layout| layout.name == owner)
            .and_then(|layout| layout.fields.get(index))
            .map(|field| &field.ty)
            .ok_or_else(|| CodegenError::new(span, "native field has no aggregate layout"))
    }

    fn render_value_conversion(
        &self,
        expression: &str,
        from: &Type<'src>,
        to: &Type<'src>,
        span: Span,
    ) -> Result<String, CodegenError> {
        if let Type::Nullable(inner) = to {
            return match from {
                Type::Null => Ok("(LilScriptOptional){false,{0}}".to_string()),
                Type::Nullable(source) if source == inner => Ok(expression.to_string()),
                Type::Nullable(source)
                    if source.as_ref() == &Type::Int && inner.as_ref() == &Type::Float =>
                {
                    Ok(format!("lilscript_optional_f64({expression})"))
                }
                Type::Nullable(source)
                    if matches!(source.as_ref(), Type::TypeParameter(_))
                        || matches!(inner.as_ref(), Type::TypeParameter(_)) =>
                {
                    Ok(expression.to_string())
                }
                Type::Nullable(_) => Err(CodegenError::new(
                    span,
                    format!("cannot convert native `{from}` to `{to}`"),
                )),
                Type::TypeParameter("$closure" | "$array") | Type::Union(_) => {
                    Ok(format!("lilscript_value_optional({expression})"))
                }
                _ => {
                    if !crate::semantic::is_type_assignable(inner, from)
                        && c_type(inner) != c_type(from)
                    {
                        return Err(CodegenError::new(
                            span,
                            format!("cannot convert native `{from}` to `{to}`"),
                        ));
                    }
                    let normalized = self.render_value_conversion(expression, from, inner, span)?;
                    let boxed = self.render_value_conversion(
                        &normalized,
                        inner,
                        &Type::TypeParameter("$optional"),
                        span,
                    )?;
                    Ok(format!("(LilScriptOptional){{true,{boxed}}}"))
                }
            };
        }
        if is_erased_type(to) && from == &Type::Null {
            return Ok("(LilScriptValue){0}".to_string());
        }
        if is_erased_type(to) && matches!(from, Type::Nullable(_)) {
            return Ok(format!("lilscript_optional_value({expression})"));
        }
        if matches!(from, Type::Nullable(_)) {
            return self.render_value_conversion(
                &format!("({expression}).value"),
                &Type::TypeParameter("$optional"),
                to,
                span,
            );
        }
        if from == to || c_type(from) == c_type(to) {
            return Ok(expression.to_string());
        }
        if from == &Type::Int && to == &Type::Float {
            return Ok(format!("(double)({expression})"));
        }
        if is_erased_type(to) {
            let member = match from {
                Type::Int | Type::Enum(_) => "i",
                Type::Float => "f",
                Type::Bool => "b",
                Type::String => "s",
                Type::Function(_) | Type::GenericFunction(_) => "c",
                Type::Array(_)
                | Type::Record(_)
                | Type::Map(_, _)
                | Type::Set(_)
                | Type::ArrayBuffer
                | Type::SharedArrayBuffer
                | Type::Uint8Array
                | Type::Int8Array
                | Type::Uint8ClampedArray
                | Type::Int16Array
                | Type::Uint16Array
                | Type::Int32Array
                | Type::Uint32Array
                | Type::Float32Array
                | Type::Float64Array
                | Type::Symbol
                | Type::Regex
                | Type::Class(_)
                | Type::ClassInstance { .. }
                | Type::Task(_)
                | Type::Generator(_)
                | Type::ModuleNamespace(_)
                | Type::ModuleLoadError => "p",
                Type::Struct(_) | Type::StructInstance { .. } => {
                    let native = c_type(from);
                    return Ok(format!(
                        "(LilScriptValue){{.tag=7,.p=lilscript_copy(({native}[]){{{expression}}},sizeof({native}))}}"
                    ));
                }
                Type::TypeParameter(_) | Type::Union(_) => return Ok(expression.to_string()),
                Type::Null | Type::Nullable(_) => {
                    return Ok(format!(
                        "(LilScriptValue){{.tag=7,.p=lilscript_copy((LilScriptOptional[]){{{expression}}},sizeof(LilScriptOptional))}}"
                    ));
                }
                Type::Void => {
                    return Err(CodegenError::new(span, "cannot box a native void value"));
                }
            };
            let value = if member == "p" {
                format!("(void*)({expression})")
            } else {
                expression.to_string()
            };
            return Ok(format!(
                "(LilScriptValue){{.tag={},.{member}={value}}}",
                generic_value_tag(from)
            ));
        }
        if is_erased_type(from) {
            return Ok(match to {
                Type::Int | Type::Enum(_) => format!("({expression}).i"),
                Type::Float => format!("({expression}).f"),
                Type::Bool => format!("({expression}).b"),
                Type::String => format!("({expression}).s"),
                Type::Function(_) | Type::GenericFunction(_) => format!("({expression}).c"),
                Type::Array(_)
                | Type::Record(_)
                | Type::Map(_, _)
                | Type::Set(_)
                | Type::ArrayBuffer
                | Type::SharedArrayBuffer
                | Type::Uint8Array
                | Type::Int8Array
                | Type::Uint8ClampedArray
                | Type::Int16Array
                | Type::Uint16Array
                | Type::Int32Array
                | Type::Uint32Array
                | Type::Float32Array
                | Type::Float64Array
                | Type::Symbol
                | Type::Regex
                | Type::Class(_)
                | Type::ClassInstance { .. }
                | Type::Task(_)
                | Type::Generator(_)
                | Type::ModuleNamespace(_)
                | Type::ModuleLoadError => {
                    format!("({})({expression}).p", c_type(to))
                }
                Type::Struct(_) | Type::StructInstance { .. } => {
                    format!("*({}*)({expression}).p", c_type(to))
                }
                Type::TypeParameter(_) | Type::Union(_) => expression.to_string(),
                Type::Nullable(_) => format!("lilscript_value_optional({expression})"),
                Type::Null => "(LilScriptOptional){false,{0}}".to_string(),
                Type::Void => {
                    return Err(CodegenError::new(span, "cannot unbox a native void value"));
                }
            });
        }
        Err(CodegenError::new(
            span,
            format!("cannot convert native `{from}` to `{to}`"),
        ))
    }

    fn render_string_value(
        &self,
        value: ValueId,
        ty: &Type<'src>,
        span: Span,
    ) -> Result<String, CodegenError> {
        Ok(match ty {
            Type::String => format!("v{}", value.0),
            Type::Int => format!("lilscript_i32(v{})", value.0),
            Type::Float => format!("lilscript_f64(v{})", value.0),
            Type::Bool => format!("(v{}?\"true\":\"false\")", value.0),
            Type::Union(_) => format!("lilscript_value_string(v{})", value.0),
            _ => {
                return Err(CodegenError::new(
                    span,
                    "value cannot be converted to a native string",
                ));
            }
        })
    }

    fn render_binary(
        &self,
        op: IrBinaryOp,
        lhs: ValueId,
        rhs: ValueId,
        output_type: Option<&Type<'src>>,
        types: &AHashMap<ValueId, Type<'src>>,
        span: Span,
    ) -> Result<String, CodegenError> {
        let lhs_name = format!("v{}", lhs.0);
        let rhs_name = format!("v{}", rhs.0);
        let lhs_type = &types[&lhs];
        let rhs_type = &types[&rhs];
        if matches!(op, IrBinaryOp::Eq | IrBinaryOp::NotEq)
            && (matches!(lhs_type, Type::Null | Type::Nullable(_))
                || matches!(rhs_type, Type::Null | Type::Nullable(_)))
        {
            let equal =
                self.render_nullable_equality(&lhs_name, lhs_type, &rhs_name, rhs_type, span)?;
            return Ok(if op == IrBinaryOp::Eq {
                equal
            } else {
                format!("!({equal})")
            });
        }
        if matches!(op, IrBinaryOp::Eq | IrBinaryOp::NotEq)
            && (is_erased_type(lhs_type) || is_erased_type(rhs_type))
        {
            let erased = Type::TypeParameter("$equality");
            let lhs = self.render_value_conversion(&lhs_name, lhs_type, &erased, span)?;
            let rhs = self.render_value_conversion(&rhs_name, rhs_type, &erased, span)?;
            let equal = format!("lilscript_value_eq({lhs},{rhs})");
            return Ok(if op == IrBinaryOp::Eq {
                equal
            } else {
                format!("!({equal})")
            });
        }
        if output_type == Some(&Type::Int) {
            return Ok(match op {
                IrBinaryOp::Add => {
                    format!("(int32_t)((uint32_t){lhs_name}+(uint32_t){rhs_name})")
                }
                IrBinaryOp::Sub => {
                    format!("(int32_t)((uint32_t){lhs_name}-(uint32_t){rhs_name})")
                }
                IrBinaryOp::Mul => format!("lilscript_mul({lhs_name},{rhs_name})"),
                IrBinaryOp::Div => format!("lilscript_idiv({lhs_name},{rhs_name})"),
                IrBinaryOp::Mod => format!("lilscript_irem({lhs_name},{rhs_name})"),
                IrBinaryOp::BitAnd => {
                    format!("lilscript_from_u32((uint32_t){lhs_name}&(uint32_t){rhs_name})")
                }
                IrBinaryOp::BitOr => {
                    format!("lilscript_from_u32((uint32_t){lhs_name}|(uint32_t){rhs_name})")
                }
                IrBinaryOp::Xor => {
                    format!("lilscript_from_u32((uint32_t){lhs_name}^(uint32_t){rhs_name})")
                }
                IrBinaryOp::ShiftLeft => format!("lilscript_shl({lhs_name},{rhs_name})"),
                IrBinaryOp::ShiftRight => format!("lilscript_shr({lhs_name},{rhs_name})"),
                IrBinaryOp::UnsignedShiftRight => {
                    format!("lilscript_ushr({lhs_name},{rhs_name})")
                }
                _ => {
                    return Err(CodegenError::new(span, "invalid integer binary operation"));
                }
            });
        }
        let operand_type = types.get(&lhs);
        if output_type == Some(&Type::String) && op == IrBinaryOp::Add {
            return Ok(format!(
                "lilscript_cat({},{})",
                self.render_string_value(lhs, &types[&lhs], span)?,
                self.render_string_value(rhs, &types[&rhs], span)?
            ));
        }
        if matches!(operand_type, Some(Type::String)) {
            return match op {
                IrBinaryOp::Eq => Ok(format!("!strcmp({lhs_name},{rhs_name})")),
                IrBinaryOp::NotEq => Ok(format!("strcmp({lhs_name},{rhs_name})!=0")),
                IrBinaryOp::Less => Ok(format!("strcmp({lhs_name},{rhs_name})<0")),
                IrBinaryOp::LessEq => Ok(format!("strcmp({lhs_name},{rhs_name})<=0")),
                IrBinaryOp::Greater => Ok(format!("strcmp({lhs_name},{rhs_name})>0")),
                IrBinaryOp::GreaterEq => Ok(format!("strcmp({lhs_name},{rhs_name})>=0")),
                _ => Err(CodegenError::new(
                    span,
                    "native string operation is not implemented yet",
                )),
            };
        }
        Ok(format!("({lhs_name}{}{rhs_name})", c_binary_operator(op)))
    }

    fn render_type_check(
        &self,
        value: ValueId,
        source: &Type<'src>,
        target: &Type<'src>,
        span: Span,
    ) -> Result<String, CodegenError> {
        let name = format!("v{}", value.0);
        if let Type::Nullable(inner) = source {
            return Ok(if target == &Type::Null {
                format!("!({name}).has")
            } else if target == inner.as_ref() {
                format!("({name}).has")
            } else {
                "false".to_string()
            });
        }
        let tag = generic_value_tag(target);
        if tag == 0 {
            return Err(CodegenError::new(
                span,
                format!("type `{target}` has no native type guard"),
            ));
        }
        if is_erased_type(source) {
            Ok(format!("({name}).tag=={tag}"))
        } else {
            Ok((source == target).to_string())
        }
    }

    fn render_nullable_equality(
        &self,
        lhs: &str,
        lhs_type: &Type<'src>,
        rhs: &str,
        rhs_type: &Type<'src>,
        span: Span,
    ) -> Result<String, CodegenError> {
        match (lhs_type, rhs_type) {
            (Type::Null, Type::Null) => Ok("true".to_string()),
            (Type::Nullable(_), Type::Null) => Ok(format!("!({lhs}).has")),
            (Type::Null, Type::Nullable(_)) => Ok(format!("!({rhs}).has")),
            (Type::Nullable(lhs_inner), Type::Nullable(rhs_inner)) => {
                let lhs_value = self.render_value_conversion(
                    &format!("({lhs}).value"),
                    &Type::TypeParameter("$optional"),
                    lhs_inner,
                    span,
                )?;
                let rhs_value = self.render_value_conversion(
                    &format!("({rhs}).value"),
                    &Type::TypeParameter("$optional"),
                    rhs_inner,
                    span,
                )?;
                let values =
                    render_native_equality(&lhs_value, lhs_inner, &rhs_value, rhs_inner, span)?;
                Ok(format!(
                    "(({lhs}).has==({rhs}).has&&(!({lhs}).has||{values}))"
                ))
            }
            (Type::Nullable(inner), other) => {
                let value = self.render_value_conversion(
                    &format!("({lhs}).value"),
                    &Type::TypeParameter("$optional"),
                    inner,
                    span,
                )?;
                let values = render_native_equality(&value, inner, rhs, other, span)?;
                Ok(format!("(({lhs}).has&&{values})"))
            }
            (other, Type::Nullable(inner)) => {
                let value = self.render_value_conversion(
                    &format!("({rhs}).value"),
                    &Type::TypeParameter("$optional"),
                    inner,
                    span,
                )?;
                let values = render_native_equality(lhs, other, &value, inner, span)?;
                Ok(format!("(({rhs}).has&&{values})"))
            }
            _ => Err(CodegenError::new(span, "invalid native nullable equality")),
        }
    }

    fn emit_print(
        &self,
        value: ValueId,
        ty: &Type<'src>,
        span: Span,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        match ty {
            Type::Int => write!(out, "printf(\"%d\\n\",v{});", value.0),
            Type::Float => write!(out, "printf(\"%.17g\\n\",v{});", value.0),
            Type::Bool => write!(out, "puts(v{}?\"true\":\"false\");", value.0),
            Type::String => write!(out, "puts(v{});", value.0),
            Type::Union(_) => write!(out, "lilscript_print_value(v{});", value.0),
            _ => {
                return Err(CodegenError::new(
                    span,
                    "native print does not support this type yet",
                ));
            }
        }
        .expect("writing to String cannot fail");
        Ok(())
    }

    fn emit_terminator(
        &self,
        function: &ControlFlowFunction<'src>,
        from: BlockId,
        types: &AHashMap<ValueId, Type<'src>>,
        phi_temp: &mut usize,
        uses_region: bool,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let block = &function.blocks[from.0 as usize];
        match block
            .terminator
            .as_ref()
            .ok_or_else(|| CodegenError::new(block.span, "IR block has no terminator"))?
        {
            Terminator::Jump(target) => {
                self.emit_phi_edge(function, from, *target, types, phi_temp, out)?;
                write!(out, "s={};continue;", target.0).expect("writing to String cannot fail");
            }
            Terminator::Branch {
                condition,
                then_block,
                else_block,
            } => {
                write!(out, "if(v{}){{", condition.0).expect("writing to String cannot fail");
                self.emit_phi_edge(function, from, *then_block, types, phi_temp, out)?;
                write!(out, "s={};}}else{{", then_block.0).expect("writing to String cannot fail");
                self.emit_phi_edge(function, from, *else_block, types, phi_temp, out)?;
                write!(out, "s={};}}continue;", else_block.0)
                    .expect("writing to String cannot fail");
            }
            Terminator::Return(Some(value)) => {
                if function.kind == FunctionKind::Closure {
                    let universal = Type::TypeParameter("$closure");
                    let converted = self.render_value_conversion(
                        &format!("v{}", value.0),
                        &types[value],
                        &universal,
                        block.span,
                    )?;
                    if uses_region {
                        write!(
                            out,
                            "LilScriptValue z={converted};lilscript_region_dispose(&r);return z;"
                        )
                        .expect("writing to String cannot fail");
                    } else {
                        write!(out, "return {converted};").expect("writing to String cannot fail");
                    }
                } else {
                    let converted = self.render_value_conversion(
                        &format!("v{}", value.0),
                        &types[value],
                        &function.return_type,
                        block.span,
                    )?;
                    if uses_region {
                        write!(
                            out,
                            "{} z={converted};lilscript_region_dispose(&r);return z;",
                            c_type(&function.return_type)
                        )
                        .expect("writing to String cannot fail");
                    } else {
                        write!(out, "return {converted};").expect("writing to String cannot fail");
                    }
                }
            }
            Terminator::Return(None) if function.kind == FunctionKind::Entry => {
                if uses_region {
                    out.push_str("lilscript_region_dispose(&r);");
                }
                out.push_str("return 0;");
            }
            Terminator::Return(None) if function.kind == FunctionKind::Closure => {
                if uses_region {
                    out.push_str("lilscript_region_dispose(&r);");
                }
                out.push_str("return (LilScriptValue){0};")
            }
            Terminator::Return(None) => {
                if uses_region {
                    out.push_str("lilscript_region_dispose(&r);");
                }
                out.push_str("return;");
            }
            Terminator::Throw(_) => out.push_str("abort();"),
            Terminator::Try { .. } => out.push_str("abort();"),
            Terminator::Unreachable => out.push_str("abort();"),
        }
        Ok(())
    }

    fn emit_phi_edge(
        &self,
        function: &ControlFlowFunction<'src>,
        from: BlockId,
        to: BlockId,
        types: &AHashMap<ValueId, Type<'src>>,
        phi_temp: &mut usize,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let copies = function.blocks[to.0 as usize]
            .phis
            .iter()
            .filter_map(|phi| {
                phi.incoming
                    .iter()
                    .find(|(block, _)| *block == from)
                    .map(|(_, source)| (phi.out, *source))
            })
            .collect::<Vec<_>>();
        let base = *phi_temp;
        for (index, (target, source)) in copies.iter().enumerate() {
            let ty = types
                .get(target)
                .ok_or_else(|| CodegenError::new(function.span, "phi target has no native type"))?;
            let source_type = types
                .get(source)
                .ok_or_else(|| CodegenError::new(function.span, "phi source has no native type"))?;
            let converted = self.render_value_conversion(
                &format!("v{}", source.0),
                source_type,
                ty,
                function.span,
            )?;
            write!(out, "{} p{}={converted};", c_type(ty), base + index)
                .expect("writing to String cannot fail");
        }
        for (index, (target, _)) in copies.iter().enumerate() {
            write!(out, "v{}=p{};", target.0, base + index).expect("writing to String cannot fail");
        }
        *phi_temp += copies.len();
        Ok(())
    }

    fn native_function_name(&self, id: FunctionId) -> Result<String, CodegenError> {
        let function = self.function(id)?;
        if function.kind == FunctionKind::Extern {
            function
                .name
                .map(ToString::to_string)
                .ok_or_else(|| CodegenError::new(function.span, "extern function has no name"))
        } else {
            Ok(format!("f{}", id.0))
        }
    }

    fn function(&self, id: FunctionId) -> Result<&ControlFlowFunction<'src>, CodegenError> {
        self.module.functions.get(id.0 as usize).ok_or_else(|| {
            CodegenError::new(Span::empty(0), format!("missing IR function {}", id.0))
        })
    }
}

fn contains_js_value(ty: &Type<'_>) -> bool {
    match ty {
        Type::TypeParameter("$js") => true,
        Type::Array(element)
        | Type::Record(element)
        | Type::Set(element)
        | Type::Task(element)
        | Type::Nullable(element) => contains_js_value(element),
        Type::Map(key, value) => contains_js_value(key) || contains_js_value(value),
        Type::Union(members) => members.iter().any(contains_js_value),
        Type::StructInstance { args, .. } | Type::ClassInstance { args, .. } => {
            args.iter().any(contains_js_value)
        }
        Type::Function(signature) => {
            signature.params.iter().any(contains_js_value)
                || contains_js_value(&signature.return_type)
        }
        Type::GenericFunction(function) => {
            function.signature.params.iter().any(contains_js_value)
                || contains_js_value(&function.signature.return_type)
        }
        _ => false,
    }
}

fn contains_generator(ty: &Type<'_>) -> bool {
    match ty {
        Type::Generator(_) => true,
        Type::Array(element)
        | Type::Record(element)
        | Type::Set(element)
        | Type::Task(element)
        | Type::Nullable(element) => contains_generator(element),
        Type::Map(key, value) => contains_generator(key) || contains_generator(value),
        Type::Union(members) => members.iter().any(contains_generator),
        Type::StructInstance { args, .. } | Type::ClassInstance { args, .. } => {
            args.iter().any(contains_generator)
        }
        Type::Function(signature) => {
            signature.params.iter().any(contains_generator)
                || contains_generator(&signature.return_type)
        }
        Type::GenericFunction(function) => {
            function.signature.params.iter().any(contains_generator)
                || contains_generator(&function.signature.return_type)
        }
        _ => false,
    }
}

fn contains_regex(ty: &Type<'_>) -> bool {
    match ty {
        Type::Regex => true,
        Type::Array(element)
        | Type::Record(element)
        | Type::Set(element)
        | Type::Task(element)
        | Type::Nullable(element) => contains_regex(element),
        Type::Map(key, value) => contains_regex(key) || contains_regex(value),
        Type::Union(members) => members.iter().any(contains_regex),
        Type::StructInstance { args, .. } | Type::ClassInstance { args, .. } => {
            args.iter().any(contains_regex)
        }
        Type::Function(signature) => {
            signature.params.iter().any(contains_regex) || contains_regex(&signature.return_type)
        }
        Type::GenericFunction(function) => {
            function.signature.params.iter().any(contains_regex)
                || contains_regex(&function.signature.return_type)
        }
        _ => false,
    }
}

fn allocation_is_function_bounded(
    module: &ControlFlowModule<'_>,
    function: &ControlFlowFunction<'_>,
    value: ValueId,
) -> bool {
    for block in &function.blocks {
        if block
            .phis
            .iter()
            .any(|phi| phi.incoming.iter().any(|(_, incoming)| *incoming == value))
        {
            return false;
        }
        if matches!(
            block.terminator,
            Some(Terminator::Return(Some(returned)) | Terminator::Throw(returned))
                if returned == value
        ) {
            return false;
        }
        for instruction in &block.instructions {
            let unsafe_use = match &instruction.op {
                ControlFlowOp::StoreLocal { value: stored, .. }
                | ControlFlowOp::StoreGlobal { value: stored, .. } => *stored == value,
                ControlFlowOp::Array(values) | ControlFlowOp::Struct { fields: values, .. } => {
                    values.contains(&value)
                }
                ControlFlowOp::Record(entries) => {
                    entries.iter().any(|(_, stored)| *stored == value)
                }
                ControlFlowOp::ArraySpread(operands) => operands.iter().any(|operand| {
                    matches!(operand, ArrayOperand::Value(stored) | ArrayOperand::Spread(stored) if *stored == value)
                }),
                ControlFlowOp::RecordSpread(operands) => operands.iter().any(|operand| {
                    matches!(operand, RecordOperand::Entry(_, stored) | RecordOperand::Spread(stored) if *stored == value)
                }),
                ControlFlowOp::NewClass { args, .. } => args.contains(&value),
                ControlFlowOp::Closure { captures, .. } => captures.contains(&value),
                ControlFlowOp::FieldSet {
                    object,
                    value: stored,
                    ..
                }
                | ControlFlowOp::HostFieldSet {
                    object,
                    value: stored,
                    ..
                }
                | ControlFlowOp::RecordFieldSet {
                    object,
                    value: stored,
                    ..
                } => *stored == value && *object != value,
                ControlFlowOp::IndexSet {
                    object,
                    index,
                    value: stored,
                } => (*stored == value && *object != value) || *index == value,
                ControlFlowOp::CallDirect { function, args, .. } => {
                    direct_call_retains_value(module, *function, args, value)
                }
                ControlFlowOp::CallMethod {
                    receiver,
                    function,
                    args,
                    ..
                } => {
                    let mut direct_args = vec![*receiver];
                    direct_args.extend(args);
                    direct_call_retains_value(module, *function, &direct_args, value)
                }
                ControlFlowOp::CallValue { callee, args } => {
                    *callee == value || args.contains(&value)
                }
                ControlFlowOp::HostCall { receiver, args, .. } => {
                    *receiver == value || args.contains(&value)
                }
                ControlFlowOp::Intrinsic { receiver, args, .. } => {
                    args.contains(&value)
                        || (receiver == &Some(value) && intrinsic_retains_receiver(&instruction.op))
                }
                ControlFlowOp::Template(parts) => parts
                    .iter()
                    .any(|part| matches!(part, TemplateOperand::Value(item) if *item == value)),
                _ => false,
            };
            if unsafe_use {
                return false;
            }
        }
    }
    true
}

fn direct_call_retains_value(
    module: &ControlFlowModule<'_>,
    function: FunctionId,
    args: &[ValueId],
    value: ValueId,
) -> bool {
    let Some(callee) = module.functions.get(function.0 as usize) else {
        return true;
    };
    args.iter()
        .enumerate()
        .filter(|(_, argument)| **argument == value)
        .any(|(index, _)| {
            callee
                .params
                .get(index)
                .and_then(|parameter| callee.value_escapes.get(parameter.value.0 as usize))
                .copied()
                != Some(EscapeState::LocalOnly)
        })
}

fn intrinsic_retains_receiver(operation: &ControlFlowOp<'_>) -> bool {
    match operation {
        ControlFlowOp::Intrinsic {
            intrinsic:
                Intrinsic::ArrayPush
                | Intrinsic::ArrayFill
                | Intrinsic::ArrayCopyWithin
                | Intrinsic::ArrayReverse
                | Intrinsic::TypedArrayFill
                | Intrinsic::TypedArrayCopyWithin
                | Intrinsic::MapSet
                | Intrinsic::SetAdd
                | Intrinsic::BufferSlice
                | Intrinsic::UnwrapNullable
                | Intrinsic::UnwrapUnion,
            ..
        } => true,
        ControlFlowOp::Intrinsic { intrinsic, .. } => matches!(
            classify_typed_array_intrinsic(*intrinsic),
            Some((
                _,
                TypedArrayIntrinsic::Buffer
                    | TypedArrayIntrinsic::Slice
                    | TypedArrayIntrinsic::Subarray
            ))
        ),
        _ => false,
    }
}

fn array_capacity_is_fixed(function: &ControlFlowFunction<'_>, value: ValueId) -> bool {
    for block in &function.blocks {
        if block
            .phis
            .iter()
            .any(|phi| phi.incoming.iter().any(|(_, incoming)| *incoming == value))
        {
            return false;
        }
        for instruction in &block.instructions {
            match &instruction.op {
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::ArrayPush,
                    receiver: Some(receiver),
                    ..
                } if *receiver == value => return false,
                ControlFlowOp::CallDirect { args, .. }
                | ControlFlowOp::CallValue { args, .. }
                | ControlFlowOp::HostCall { args, .. }
                    if args.contains(&value) =>
                {
                    return false;
                }
                ControlFlowOp::Closure { captures, .. } if captures.contains(&value) => {
                    return false;
                }
                _ => {}
            }
        }
    }
    true
}

fn required_output(instruction: &ControlFlowInstruction<'_>) -> Result<ValueId, CodegenError> {
    instruction
        .out
        .ok_or_else(|| CodegenError::new(instruction.span, "native instruction has no result"))
}

fn closure_signature<'a, 'src>(
    types: &'a AHashMap<ValueId, Type<'src>>,
    value: ValueId,
    span: Span,
) -> Result<&'a crate::semantic::FunctionType<'src>, CodegenError> {
    match types.get(&value) {
        Some(Type::Function(signature)) => Ok(signature),
        _ => Err(CodegenError::new(
            span,
            "native callback has no function type",
        )),
    }
}

fn capture_local_source(function: &ControlFlowFunction<'_>, value: ValueId) -> Option<LocalId> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| (instruction.out == Some(value)).then_some(&instruction.op))
        .and_then(|operation| match operation {
            ControlFlowOp::CaptureLocal(local) => Some(*local),
            _ => None,
        })
}

fn value_types<'src>(function: &ControlFlowFunction<'src>) -> AHashMap<ValueId, Type<'src>> {
    let mut types = AHashMap::default();
    for param in &function.params {
        types.insert(param.value, param.ty.clone());
    }
    for block in &function.blocks {
        for phi in &block.phis {
            types.insert(phi.out, phi.ty.clone());
        }
        for instruction in &block.instructions {
            if let (Some(value), Some(ty)) = (instruction.out, &instruction.ty) {
                types.insert(value, ty.clone());
            }
        }
    }
    types
}

fn render_native_equality(
    lhs: &str,
    lhs_type: &Type<'_>,
    rhs: &str,
    rhs_type: &Type<'_>,
    span: Span,
) -> Result<String, CodegenError> {
    if is_erased_type(lhs_type) || is_erased_type(rhs_type) {
        return Err(CodegenError::new(
            span,
            "erased native equality must be normalized before rendering",
        ));
    }
    if lhs_type == &Type::String && rhs_type == &Type::String {
        return Ok(format!("!strcmp({lhs},{rhs})"));
    }
    if matches!(
        (lhs_type, rhs_type),
        (
            Type::Struct(_) | Type::StructInstance { .. },
            Type::Struct(_) | Type::StructInstance { .. }
        )
    ) {
        return Err(CodegenError::new(
            span,
            "native struct equality is not supported",
        ));
    }
    Ok(format!("({lhs}=={rhs})"))
}

fn generic_value_tag(ty: &Type<'_>) -> u8 {
    match ty {
        Type::Int | Type::Enum(_) => 1,
        Type::Float => 2,
        Type::Bool => 3,
        Type::String => 4,
        Type::Function(_) | Type::GenericFunction(_) => 5,
        Type::Array(_)
        | Type::Record(_)
        | Type::Map(_, _)
        | Type::Set(_)
        | Type::ArrayBuffer
        | Type::SharedArrayBuffer
        | Type::Task(_)
        | Type::Generator(_)
        | Type::ModuleNamespace(_)
        | Type::ModuleLoadError
        | Type::Class(_)
        | Type::ClassInstance { .. } => 6,
        Type::Regex => 6,
        Type::Symbol => 9,
        Type::Struct(_) | Type::StructInstance { .. } | Type::Null | Type::Nullable(_) => 7,
        Type::TypeParameter(_) | Type::Union(_) | Type::Void => 0,
        Type::Int8Array
        | Type::Uint8Array
        | Type::Uint8ClampedArray
        | Type::Int16Array
        | Type::Uint16Array
        | Type::Int32Array
        | Type::Uint32Array
        | Type::Float32Array
        | Type::Float64Array => TypedArrayKind::from_type(ty)
            .expect("typed array")
            .generic_value_tag(),
    }
}

fn c_type(ty: &Type<'_>) -> String {
    if let Some(kind) = TypedArrayKind::from_type(ty) {
        return kind.native_ctype_alias().to_string();
    }
    match ty {
        Type::Int | Type::Enum(_) => "int32_t".to_string(),
        Type::Float => "double".to_string(),
        Type::Bool => "bool".to_string(),
        Type::String => "const char*".to_string(),
        Type::Null | Type::Nullable(_) => "LilScriptOptional".to_string(),
        Type::Array(_) => "LilScriptArray".to_string(),
        Type::Record(_) => "LilScriptMap".to_string(),
        Type::Map(_, _) => "LilScriptMap".to_string(),
        Type::Set(_) => "LilScriptSet".to_string(),
        Type::ArrayBuffer | Type::SharedArrayBuffer => "LilScriptBuffer".to_string(),
        Type::Int8Array
        | Type::Uint8Array
        | Type::Uint8ClampedArray
        | Type::Int16Array
        | Type::Uint16Array
        | Type::Int32Array
        | Type::Uint32Array
        | Type::Float32Array
        | Type::Float64Array => unreachable!("typed arrays handled above"),
        Type::Symbol => "LilScriptSymbol".to_string(),
        Type::Regex => "void*".to_string(),
        Type::Struct(name) => aggregate_type_name("Struct", name),
        Type::Class(name) => aggregate_type_name("Class", name),
        Type::StructInstance { name, .. } => aggregate_type_name("Struct", name),
        Type::ClassInstance { name, .. } => aggregate_type_name("Class", name),
        Type::TypeParameter(_) | Type::Union(_) => "LilScriptValue".to_string(),
        Type::Function(_) | Type::GenericFunction(_) => "LilScriptClosure".to_string(),
        Type::Task(_) | Type::Generator(_) | Type::ModuleNamespace(_) | Type::ModuleLoadError => {
            "void*".to_string()
        }
        Type::Void => "void".to_string(),
    }
}

fn is_erased_type(ty: &Type<'_>) -> bool {
    matches!(ty, Type::TypeParameter(_) | Type::Union(_))
}

fn is_float_vector_union(ty: &Type<'_>) -> bool {
    match ty {
        Type::Union(members) => !members.is_empty() && members.iter().all(|member| {
            matches!(member, Type::Float32Array)
                || matches!(member, Type::Array(element) if matches!(element.as_ref(), Type::Float))
        }),
        _ => false,
    }
}

fn native_default_value(ty: &Type<'_>) -> Result<String, CodegenError> {
    if let Some(kind) = TypedArrayKind::from_type(ty) {
        return Ok(format!(
            "lilscript_ta_length(0,{})",
            kind.bytes_per_element()
        ));
    }
    Ok(match ty {
        Type::Int | Type::Enum(_) => "(int32_t)0".to_string(),
        Type::Float => "0.0".to_string(),
        Type::Bool => "false".to_string(),
        Type::String => "\"\"".to_string(),
        Type::Null | Type::Nullable(_) => "(LilScriptOptional){false,{0}}".to_string(),
        Type::Array(_) => "lilscript_array(0,sizeof(LilScriptValue))".to_string(),
        Type::Record(_) => "lilscript_map()".to_string(),
        Type::Map(_, _) => "lilscript_map()".to_string(),
        Type::Set(_) => "lilscript_set()".to_string(),
        Type::ArrayBuffer => "lilscript_buffer(0,false)".to_string(),
        Type::SharedArrayBuffer => "lilscript_buffer(0,true)".to_string(),
        Type::Int8Array
        | Type::Uint8Array
        | Type::Uint8ClampedArray
        | Type::Int16Array
        | Type::Uint16Array
        | Type::Int32Array
        | Type::Uint32Array
        | Type::Float32Array
        | Type::Float64Array => unreachable!("typed arrays handled above"),
        Type::Symbol => "lilscript_symbol(NULL)".to_string(),
        Type::Regex => "NULL".to_string(),
        Type::Struct(_) | Type::StructInstance { .. } => format!("({}){{0}}", c_type(ty)),
        Type::Class(_) | Type::ClassInstance { .. } => "NULL".to_string(),
        Type::TypeParameter(_) => "(LilScriptValue){0}".to_string(),
        Type::Function(_) | Type::GenericFunction(_) => "(LilScriptClosure){0}".to_string(),
        Type::Task(_) | Type::Generator(_) | Type::ModuleNamespace(_) | Type::ModuleLoadError => {
            "NULL".to_string()
        }
        Type::Union(members) => {
            let member = members.first().ok_or_else(|| {
                CodegenError::new(Span::empty(0), "union has no native member type")
            })?;
            native_default_value(member)?
        }
        Type::Void => {
            return Err(CodegenError::new(
                Span::empty(0),
                "native void has no default value",
            ));
        }
    })
}

fn aggregate_type_name(kind: &str, name: &str) -> String {
    let mut encoded = format!("LilScript{kind}_");
    for byte in name.bytes() {
        write!(encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

fn render_c_const(value: &ConstValue) -> String {
    match value {
        ConstValue::Int(value) => format!("((int32_t){value})"),
        ConstValue::Float(value) => format!("{value:.17}"),
        ConstValue::Bool(value) => value.to_string(),
        ConstValue::String(value) => format!("\"{value}\""),
        ConstValue::Null => "(LilScriptOptional){false,{0}}".to_string(),
    }
}

fn c_binary_operator(op: IrBinaryOp) -> &'static str {
    match op {
        IrBinaryOp::Add => "+",
        IrBinaryOp::Sub => "-",
        IrBinaryOp::Mul => "*",
        IrBinaryOp::Div => "/",
        IrBinaryOp::Mod => "%",
        IrBinaryOp::BitAnd => "&",
        IrBinaryOp::BitOr => "|",
        IrBinaryOp::Xor => "^",
        IrBinaryOp::ShiftLeft => "<<",
        IrBinaryOp::ShiftRight => ">>",
        IrBinaryOp::UnsignedShiftRight => ">>>",
        IrBinaryOp::Eq => "==",
        IrBinaryOp::NotEq => "!=",
        IrBinaryOp::Less => "<",
        IrBinaryOp::LessEq => "<=",
        IrBinaryOp::Greater => ">",
        IrBinaryOp::GreaterEq => ">=",
        IrBinaryOp::And => "&&",
        IrBinaryOp::Or => "||",
    }
}

#[cfg(test)]
mod tests {
    use bumpalo::Bump;

    use super::*;
    use crate::parse_source;

    #[test]
    fn emits_enums_as_int32_without_runtime_metadata() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "enum Status{Draft,Active,Sold}Status active(){return Status.Active;}int code(Status value){return match(value){Status.Draft=>10,Status.Active=>20,Status.Sold=>30};}print(code(active()));",
        )
        .unwrap();
        let c = compile_to_c(&program).unwrap();
        assert!(!c.contains("Draft"), "{c}");
        assert!(!c.contains("Active"), "{c}");
        assert!(!c.contains("Sold"), "{c}");
        assert!(c.contains("int32_t"), "{c}");
    }

    #[test]
    fn emits_structural_records_through_the_native_map_runtime() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "Record<int> values=record{alpha:1};values.beta=3;print(values.alpha??0);print(values[\"beta\"]??0);",
        )
        .unwrap();
        let c = compile_to_c(&program).unwrap();
        assert!(c.contains("lilscript_record("), "{c}");
        assert!(c.contains("lilscript_map_set("), "{c}");
        assert!(c.contains("lilscript_map_get("), "{c}");
    }

    #[test]
    fn emits_portable_record_object_and_json_runtime_calls() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "Record<int> values=record{alpha:1};string[] keys=Object.keys(values);int[] entries=Object.values(values);print(keys.length);print(entries.length);print(Object.hasOwn(values,\"alpha\"));Object.assign(values,record{beta:2});print(JSON.stringify(values));",
        )
        .unwrap();
        let c = compile_to_c(&program).unwrap();
        assert!(c.contains("lilscript_record_keys(v"), "{c}");
        assert!(c.contains("lilscript_record_values(v"), "{c}");
        assert!(c.contains("lilscript_record_assign(v"), "{c}");
        assert!(c.contains("lilscript_json_record(v"), "{c}");
    }

    #[test]
    fn rejects_json_parse_for_native_targets() {
        let arena = Bump::new();
        let program = parse_source(&arena, "JSON.parse(\"null\");").unwrap();
        let error = compile_to_c(&program).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("only available for JavaScript targets"),
            "{error}"
        );
    }

    #[test]
    fn rejects_first_class_javascript_abi_for_native_targets() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "JsValue value=JS.object();JS.set(value,\"x\",1);JS.number(value);JS.strictEqual(value,0);JS.or(value,JS.undefined());JS.invoke(value,\"run\");",
        )
        .unwrap();
        let error = compile_to_c(&program).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("only available for JavaScript targets"),
            "{error}"
        );
    }

    #[test]
    fn rejects_typed_javascript_adapters_for_native_targets() {
        let arena = Bump::new();
        let program =
            parse_source(&arena, "JsValue wrapped=JS.method0((JsValue self)=>self);").unwrap();
        let error = compile_to_c(&program).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("only available for JavaScript targets"),
            "{error}"
        );
    }

    #[test]
    fn rejects_regex_for_native_targets_without_approximating_ecmascript() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "Regex pattern=new Regex(\"sale\",\"i\");print(pattern.test(\"SALE\"));",
        )
        .unwrap();
        let error = compile_to_c(&program).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Regex is only available for JavaScript targets"),
            "{error}"
        );
    }

    #[test]
    fn rejects_exceptions_for_native_targets() {
        let arena = Bump::new();
        let program = parse_source(&arena, "try{throw \"bad\";}catch{}").unwrap();
        let error = compile_to_c(&program).unwrap_err();
        assert!(
            error.to_string().contains("exceptions are only available"),
            "{error}"
        );
    }

    #[test]
    fn rejects_generators_and_inheritance_until_native_abis_are_exact() {
        let arena = Bump::new();
        let generator = parse_source(
            &arena,
            "generator int values(){yield 1;}for(int value of values()){print(value);}",
        )
        .unwrap();
        let error = compile_to_c(&generator).unwrap_err();
        assert!(
            error.to_string().contains("generators and yield"),
            "{error}"
        );

        let arena = Bump::new();
        let inherited = parse_source(
            &arena,
            "class Base{}class Child extends Base{}Child child=new Child();",
        )
        .unwrap();
        let error = compile_to_c(&inherited).unwrap_err();
        assert!(error.to_string().contains("native subtype ABI"), "{error}");

        let arena = Bump::new();
        let imported_generator =
            parse_source(&arena, "extern Generator<int> values();values();").unwrap();
        let error = compile_to_c(&imported_generator).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Generator<T> values are only available"),
            "{error}"
        );
    }

    #[test]
    fn emits_native_indexed_for_of_loops() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int[] values=[1,2];int total=0;for(int value of values){total+=value;}print(total);",
        )
        .unwrap();
        let c = compile_to_c(&program).unwrap();
        assert!(c.contains("->len"), "{c}");
        assert!(c.contains("->data"), "{c}");
    }

    #[test]
    fn emits_native_shallow_spread_runtime_calls() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int[] base=[1,2];int[] values=[0,...base,3];Record<int> source=record{a:1};Record<int> merged=record{...source,b:2};print(values.length);print(merged.b??0);",
        )
        .unwrap();
        let c = compile_to_c(&program).unwrap();
        assert!(c.contains("lilscript_array_spread(3"), "{c}");
        assert!(c.contains("lilscript_record_spread(2"), "{c}");
    }

    #[test]
    fn emits_native_bounds_checked_destructuring_and_copied_rest() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int[] values=[1];auto [first,,third,...rest]=values;print(first??0);print(third??0);print(rest.length);Record<int> source=record{a:1,b:2};auto {a,...remaining}=source;print(a??0);print(JSON.stringify(remaining));",
        )
        .unwrap();
        let c = compile_to_c(&program).unwrap();
        assert!(
            c.contains("<v") && c.contains("->len?lilscript_value_optional"),
            "{c}"
        );
        assert!(c.contains("lilscript_array_slice"), "{c}");
        assert!(c.contains("lilscript_map_get"), "{c}");
        assert!(c.contains("lilscript_record_rest"), "{c}");
    }

    #[test]
    fn emits_c_from_optimized_ssa() {
        let arena = Bump::new();
        let program =
            parse_source(&arena, "int sum=0;for(int i=0;i<4;i++){sum+=i;}print(sum);").unwrap();
        let c = compile_to_c(&program).unwrap();
        assert!(c.contains("int main(void)"));
        assert!(c.contains("switch(s)"));
        assert!(c.contains("printf(\"%d\\n\""));
        assert!(!c.contains("StoreLocal"));
    }

    #[test]
    fn rejects_javascript_values_at_the_native_boundary() {
        let arena = Bump::new();
        let program =
            parse_source(&arena, "export string inspect(JsValue value){return \"\";}").unwrap();
        let error = compile_to_c(&program).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("JsValue is only available for JavaScript targets"),
            "{error}"
        );
    }

    #[test]
    fn emits_nominal_c_abi_for_escaping_aggregates() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Point{int x;int y;}class Box{int value;init(int value){this.value=value;}}extern int consumePoint(Point point);extern int consumeBox(Box value);Point point=Point{1,2};Box value=new Box(3);print(consumePoint(point)+consumeBox(value));",
        )
        .unwrap();
        let c = compile_to_c(&program).unwrap();
        assert!(c.contains("typedef struct LilScriptStruct_506f696e74 LilScriptStruct_506f696e74;"));
        assert!(c.contains("typedef struct LilScriptClass_426f78*LilScriptClass_426f78;"));
        assert!(c.contains("extern int32_t consumePoint(LilScriptStruct_506f696e74);"));
        assert!(c.contains("extern int32_t consumeBox(LilScriptClass_426f78);"));
    }

    #[test]
    fn emits_tagged_generic_equality() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "bool same<T>(T left,T right){return left==right;}print(same(7,7));print(same(1.0,-0.0));print(same(\"lil\",\"lil\"));",
        )
        .unwrap();
        let c = compile_to_c(&program).unwrap();
        assert!(c.contains("lilscript_value_eq"));
        assert!(c.contains(".tag=1,.i="));
        assert!(c.contains(".tag=2,.f="));
        assert!(c.contains(".tag=4,.s="));
    }

    #[test]
    fn devirtualizes_named_functions_used_as_local_closures() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "bool same(int left,int right){return left==right;}func(int,int)->bool callback=same;print(callback(4,4));",
        )
        .unwrap();
        let c = compile_to_c(&program).unwrap();
        assert!(!c.contains("static LilScriptValue a"));
    }

    #[test]
    fn retained_and_thrown_allocations_are_not_function_bounded() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "class Box{int value;init(int value){this.value=value;}}Record<Box> direct(){return record{item:new Box(1)};}Record<Box> spreadEntry(Record<Box> base){return record{...base,item:new Box(2)};}Record<Box> setEntry(){Record<Box> result=record{};Box box=new Box(3);result.item=box;return result;}Box[] spreadArray(Box[] tail){return [new Box(4),...tail];}int[] spreadSource(){int[] source=[1,2];return [...source];}Box unwrap(){Box? maybe=new Box(5);if(maybe!=null){return maybe;}return new Box(6);}void fail(){throw new Box(7);}",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        crate::optimizer::promote_locals_to_ssa(&mut module).unwrap();

        for (name, expected_allocations) in [
            ("direct", 1),
            ("spreadEntry", 1),
            ("setEntry", 1),
            ("spreadArray", 1),
            ("spreadSource", 1),
            ("unwrap", 2),
            ("fail", 1),
        ] {
            let function = module
                .functions
                .iter()
                .find(|function| function.name == Some(name))
                .expect("fixture function");
            let allocations = function
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .filter_map(|instruction| {
                    matches!(
                        instruction.op,
                        ControlFlowOp::Array(_) | ControlFlowOp::NewClass { .. }
                    )
                    .then_some(instruction.out)
                    .flatten()
                })
                .collect::<Vec<_>>();
            assert_eq!(
                allocations.len(),
                expected_allocations,
                "{name}: {function:#?}"
            );
            for allocation in allocations {
                assert!(
                    !allocation_is_function_bounded(&module, function, allocation),
                    "{name} retained allocation {allocation:?} was classified function-bounded"
                );
            }
        }
    }

    #[test]
    fn stack_allocates_fixed_local_arrays_and_keeps_resizable_arrays_on_heap() {
        let arena = Bump::new();
        let fixed =
            parse_source(&arena, "int[] values=[1,2,3];values[1]=7;print(values[1]);").unwrap();
        let fixed_c = compile_to_c(&fixed).unwrap();
        assert!(
            fixed_c.contains("struct LilScriptArrayHeader h"),
            "{fixed_c}"
        );

        let arena = Bump::new();
        let resizable = parse_source(
            &arena,
            "int[] values=[1,2,3];values.push(4);print(values.length);",
        )
        .unwrap();
        let resizable_c = compile_to_c(&resizable).unwrap();
        assert!(resizable_c.contains("=lilscript_array(3"), "{resizable_c}");

        let arena = Bump::new();
        let aliased = parse_source(
            &arena,
            "class Bag{int[] values;init(){this.values=[];}}Bag bag=new Bag();bag.values.push(9);print(bag.values.length);",
        )
        .unwrap();
        let aliased_c = compile_to_c(&aliased).unwrap();
        assert!(aliased_c.contains("=lilscript_array(0"), "{aliased_c}");
    }

    #[test]
    fn emits_in_place_array_fill_for_native_targets() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int[] values=[1,2,3];int[] alias=values.fill(7);print(alias[1]);",
        )
        .unwrap();
        let c = compile_to_c(&program).unwrap();

        assert!(c.contains("for(size_t i=0;i<"), "{c}");
        assert!(c.contains("LilScriptValue f="), "{c}");
    }

    #[test]
    fn emits_native_collection_search_join_and_typed_bulk_operations() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int[] values=[1,2,3];print(values.includes(2));print(values.join(\"-\"));print(values.some((int value)=>value>2));print(values.every((int value)=>value>0));print(values.findIndex((int value)=>value==2));int[] combined=values.concat([4]);values.copyWithin(1,0,2).reverse();print(combined.length);Uint8Array a=new Uint8Array(4);Uint8Array b=new Uint8Array(2);a.fill(7);a.set(b,1);a.copyWithin(2,0,2);print(a[3]);",
        )
        .unwrap();
        let c = compile_to_c(&program).unwrap();

        assert!(c.contains("lilscript_array_find_value("), "{c}");
        assert!(c.contains("lilscript_array_join("), "{c}");
        assert!(c.contains("=false;{int32_t n="), "{c}");
        assert!(c.contains("=true;{int32_t n="), "{c}");
        assert!(c.contains("=-1;{int32_t n="), "{c}");
        assert!(c.contains("lilscript_array_concat("), "{c}");
        assert!(c.contains("lilscript_array_copy_within("), "{c}");
        assert!(c.contains("lilscript_array_reverse("), "{c}");
        assert!(c.contains("lilscript_ta_fill_i("), "{c}");
        assert!(c.contains("lilscript_ta_set("), "{c}");
        assert!(c.contains("lilscript_ta_copy_within("), "{c}");
    }

    #[test]
    fn emits_native_utf16_string_search_and_repeat_operations() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "string value=\"a😀b😀\";print(value.indexOf(\"😀\",2));print(value.lastIndexOf(\"😀\"));print(value.repeat(2));",
        )
        .unwrap();
        let c = compile_to_c(&program).unwrap();

        assert!(c.contains("lilscript_string_index("), "{c}");
        assert!(c.contains("lilscript_repeat("), "{c}");
    }

    #[test]
    fn emits_native_lazy_nullish_control_flow() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "string fallback(){return \"fallback\";}string choose(string? value){return value??fallback();}print(choose(null));",
        )
        .unwrap();
        let c = compile_to_c(&program).unwrap();

        assert!(c.contains(".has"), "{c}");
        assert!(c.contains("fallback"), "{c}");
    }

    #[test]
    fn emits_native_optional_access_control_flow() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int[]? values=null;print(values?.length??-1);print(values?.[0]??-1);",
        )
        .unwrap();
        let c = compile_to_c(&program).unwrap();
        assert!(c.contains("LilScriptOptional"), "{c}");
    }

    #[test]
    fn lowers_number_alias_to_native_binary64() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "number step(number value){return value*3+1;}print(step(4));",
        )
        .unwrap();
        let c = compile_to_c(&program).unwrap();
        assert!(c.contains("double"), "{c}");
    }

    #[test]
    fn region_allocates_bounded_arrays_above_the_stack_limit() {
        let arena = Bump::new();
        let program =
            parse_source(&arena, "int[] values=[1,2,3];values[1]=7;print(values[1]);").unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut ir).unwrap();
        let c = emit_native_c_with_options(
            &ir,
            &NativeOptions {
                stack_array_element_limit: 1,
                ..NativeOptions::default()
            },
        )
        .unwrap();
        assert!(c.contains("=lilscript_region_alloc(&r,sizeof*v"), "{c}");
        assert!(c.contains("lilscript_region_dispose(&r)"), "{c}");
    }

    #[test]
    fn heap_allocates_closures_retained_in_class_fields() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "class Counter{int value;init(){this.value=0;}void increment(){this.value+=1;}}class Task{func()->void callback;init(func()->void callback){this.callback=callback;}void run(){func()->void callback=this.callback;callback();}}Task makeTask(Counter counter){return new Task(()=>counter.increment());}Counter counter=new Counter();Task task=makeTask(counter);task.run();print(counter.value);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut ir).unwrap();
        let c = emit_native_c(&ir).unwrap();
        assert!(!c.contains("_storage;"), "{c}");
        assert!(c.contains("=malloc(sizeof(E"), "{c}");
    }
}
