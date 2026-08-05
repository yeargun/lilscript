use std::fmt::Write;

use ahash::{AHashMap, AHashSet};

use crate::codegen_js::{CodegenError, CompileError};
use crate::ir::{
    BlockId, ConstValue, ControlFlowFunction, ControlFlowInstruction, ControlFlowModule,
    ControlFlowOp, FunctionId, FunctionKind, Intrinsic, IrBinaryOp, IrUnaryOp, TemplateOperand,
    Terminator, ValueId,
};
use crate::lower::lower_to_control_flow;
use crate::optimizer::optimize_control_flow;
use crate::semantic::{analyze, Type};
use crate::{ast::Program, span::Span};

pub fn compile_to_c<'ast, 'src>(program: &Program<'ast, 'src>) -> Result<String, CompileError> {
    let semantics = analyze(program)?;
    let mut ir = lower_to_control_flow(program, &semantics)?;
    optimize_control_flow(&mut ir)?;
    emit_native_c(&ir).map_err(Into::into)
}

pub fn emit_native_c(module: &ControlFlowModule<'_>) -> Result<String, CodegenError> {
    NativeEmitter { module }.emit()
}

struct NativeEmitter<'module, 'src> {
    module: &'module ControlFlowModule<'src>,
}

impl<'module, 'src> NativeEmitter<'module, 'src> {
    fn emit(&self) -> Result<String, CodegenError> {
        let mut out = String::from(
            "#include <stdbool.h>\n#include <ctype.h>\n#include <stdint.h>\n#include <stdio.h>\n#include <stdlib.h>\n#include <string.h>\n",
        );
        out.push_str(
            "static inline int32_t lilscript_idiv(int32_t a,int32_t b){if(!b)return 0;return a==INT32_MIN&&b==-1?INT32_MIN:a/b;}\n",
        );
        out.push_str(
            "static inline int32_t lilscript_irem(int32_t a,int32_t b){if(!b)return 0;return a==INT32_MIN&&b==-1?0:a%b;}\n",
        );
        out.push_str(
            "typedef struct LilScriptArrayHeader{void*data;int32_t len,cap;}*LilScriptArray;typedef struct{void*fn;void*env;}LilScriptClosure;typedef char* LilScriptString;typedef struct{uint8_t tag;union{int32_t i;double f;bool b;const char*s;void*p;LilScriptClosure c;};}LilScriptValue;typedef struct{bool has;LilScriptValue value;}LilScriptOptional;\n",
        );
        out.push_str(
            "static inline LilScriptOptional lilscript_optional_f64(LilScriptOptional v){if(v.has){int32_t i=v.value.i;v.value.tag=2;v.value.f=(double)i;}return v;}\n",
        );
        out.push_str(
            "static inline bool lilscript_value_eq(LilScriptValue a,LilScriptValue b){if(a.tag!=b.tag)return false;switch(a.tag){case 0:return true;case 1:return a.i==b.i;case 2:return a.f==b.f;case 3:return a.b==b.b;case 4:return !strcmp(a.s,b.s);case 5:return a.c.fn==b.c.fn&&a.c.env==b.c.env;default:return a.p==b.p;}}\n",
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
            "static inline LilScriptString lilscript_dup(const char*s){size_t n=strlen(s)+1;char*r=malloc(n);if(!r)abort();memcpy(r,s,n);return r;}\n",
        );
        out.push_str(
            "static inline LilScriptString lilscript_cat(const char*a,const char*b){size_t x=strlen(a),y=strlen(b);char*r=malloc(x+y+1);if(!r)abort();memcpy(r,a,x);memcpy(r+x,b,y+1);return r;}\n",
        );
        out.push_str(
            "static inline LilScriptString lilscript_i32(int32_t v){char b[16];snprintf(b,sizeof b,\"%d\",v);return lilscript_dup(b);}\n",
        );
        out.push_str(
            "static inline LilScriptString lilscript_f64(double v){char b[32];snprintf(b,sizeof b,\"%.17g\",v);return lilscript_dup(b);}\n",
        );
        out.push_str(
            "static inline LilScriptString lilscript_value_string(LilScriptValue v){switch(v.tag){case 0:return lilscript_dup(\"null\");case 1:return lilscript_i32(v.i);case 2:return lilscript_f64(v.f);case 3:return lilscript_dup(v.b?\"true\":\"false\");case 4:return lilscript_dup(v.s);default:abort();}}static inline void lilscript_print_value(LilScriptValue v){switch(v.tag){case 0:puts(\"null\");break;case 1:printf(\"%d\\n\",v.i);break;case 2:printf(\"%.17g\\n\",v.f);break;case 3:puts(v.b?\"true\":\"false\");break;case 4:puts(v.s);break;default:abort();}}\n",
        );
        out.push_str(
            "static inline bool lilscript_ends(const char*s,const char*x){size_t a=strlen(s),b=strlen(x);return a>=b&&!memcmp(s+a-b,x,b);}\n",
        );
        out.push_str(
            "static inline LilScriptString lilscript_case(const char*s,bool upper){LilScriptString r=lilscript_dup(s);for(char*p=r;*p;p++)*p=(char)(upper?toupper((unsigned char)*p):tolower((unsigned char)*p));return r;}\n",
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
                    write!(out, "{} c{index};", c_type(&capture.ty))
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

    fn closure_adapter_targets(&self) -> Vec<&ControlFlowFunction<'src>> {
        let mut referenced = AHashSet::new();
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

        let mut emitted = AHashSet::new();
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
        if function.kind == FunctionKind::Closure {
            if function.capture_count == 0 {
                out.push_str("(void)env;");
            } else {
                write!(out, "E{}*e=(E{}*)env;", function.id.0, function.id.0)
                    .expect("writing to String cannot fail");
                for (index, capture) in function.params[..function.capture_count].iter().enumerate()
                {
                    write!(out, "v{}=e->c{index};", capture.value.0)
                        .expect("writing to String cannot fail");
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
                self.emit_instruction(instruction, &types, out)?;
            }
            self.emit_terminator(function, block.id, &types, &mut phi_temp, out)?;
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
        instruction: &ControlFlowInstruction<'src>,
        types: &AHashMap<ValueId, Type<'src>>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        match &instruction.op {
            ControlFlowOp::Array(values) => {
                let result = required_output(instruction)?;
                let Some(Type::Array(element)) = instruction.ty.as_ref() else {
                    return Err(CodegenError::new(
                        instruction.span,
                        "array instruction has no array type",
                    ));
                };
                write!(
                    out,
                    "v{}=lilscript_array({},sizeof(LilScriptValue));",
                    result.0,
                    values.len()
                )
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
                write!(
                    out,
                    "v{}=calloc(1,sizeof*v{});if(!v{})abort();",
                    result.0, result.0, result.0
                )
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
                    write!(
                        out,
                        "E{}*e{}=malloc(sizeof(E{}));if(!e{})abort();",
                        function.0, result.0, function.0, result.0
                    )
                    .expect("writing to String cannot fail");
                    let closure = self.function(*function)?;
                    for (index, capture) in captures.iter().enumerate() {
                        let converted = self.render_value_conversion(
                            &format!("v{}", capture.0),
                            &types[capture],
                            &closure.params[index].ty,
                            instruction.span,
                        )?;
                        write!(out, "e{}->c{index}={converted};", result.0)
                            .expect("writing to String cannot fail");
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
            ControlFlowOp::IndexSet {
                object,
                index,
                value,
            } => {
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
                intrinsic: Intrinsic::ArrayPush | Intrinsic::ArrayPop,
                receiver: Some(receiver),
                args,
            } => {
                self.emit_array_mutation(instruction, *receiver, args, types, out)?;
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

        let expression = self.render_instruction(instruction, types)?;
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
        let Some(Type::Function(signature)) = types.get(&callback) else {
            return Err(CodegenError::new(
                instruction.span,
                "array map callback has no function type",
            ));
        };
        write!(
            out,
            "v{}=lilscript_array(v{}->len,sizeof(LilScriptValue));for(int32_t i{}=0;i{}<v{}->len;i{}++){{",
            result.0,
            receiver.0,
            result.0,
            result.0,
            receiver.0,
            result.0
        )
        .expect("writing to String cannot fail");
        let item = self.render_value_conversion(
            &format!("((LilScriptValue*)v{}->data)[i{}]", receiver.0, result.0),
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
            "((LilScriptValue*)v{}->data)[i{}]={boxed};}}",
            result.0, result.0
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
            "v{}=lilscript_array(v{}->len,sizeof(LilScriptValue));v{}->len=0;for(int32_t i{}=0;i{}<v{}->len;i{}++){{",
            result.0, receiver.0, result.0, result.0, result.0, receiver.0, result.0
        )
        .expect("writing to String cannot fail");
        let boxed_item = format!("((LilScriptValue*)v{}->data)[i{}]", receiver.0, result.0);
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
            "if({call})((LilScriptValue*)v{}->data)[v{}->len++]={boxed_item};}}",
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
            "v{}=v{};for(int32_t i{}=0;i{}<v{}->len;i{}++){{",
            result.0, initial.0, result.0, result.0, receiver.0, result.0
        )
        .expect("writing to String cannot fail");
        let args = [
            format!("v{}", result.0),
            self.render_value_conversion(
                &format!("((LilScriptValue*)v{}->data)[i{}]", receiver.0, result.0),
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
        write!(out, "v{}={call};}}", result.0).expect("writing to String cannot fail");
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
            "for(int32_t i{}=0;i{}<v{}->len;i{}++){{",
            callback.0, callback.0, receiver.0, callback.0
        )
        .expect("writing to String cannot fail");
        let item = self.render_value_conversion(
            &format!("((LilScriptValue*)v{}->data)[i{}]", receiver.0, callback.0),
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
        out.push_str(";}");
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

    fn render_instruction(
        &self,
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
            ControlFlowOp::LoadGlobal(symbol) => format!("g{}", symbol.0),
            ControlFlowOp::CallDirect { function, args } => self.render_direct_call(
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
            ControlFlowOp::IndexGet { object, index } => {
                let Some(Type::Array(element)) = types.get(object) else {
                    return Err(CodegenError::new(
                        instruction.span,
                        "indexed native load requires an array",
                    ));
                };
                self.render_value_conversion(
                    &format!("((LilScriptValue*)v{}->data)[v{}]", object.0, index.0),
                    &Type::TypeParameter("$array"),
                    element,
                    instruction.span,
                )?
            }
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
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::UnwrapNullable,
                receiver: Some(receiver),
                ..
            } => self.render_value_conversion(
                &format!("v{}", receiver.0),
                &types[receiver],
                instruction.ty.as_ref().ok_or_else(|| {
                    CodegenError::new(instruction.span, "nullable unwrap has no output type")
                })?,
                instruction.span,
            )?,
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::ArrayLength,
                receiver: Some(receiver),
                ..
            } => format!("v{}->len", receiver.0),
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::StringLength,
                receiver: Some(receiver),
                ..
            } => format!("(int32_t)strlen(v{})", receiver.0),
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
            ControlFlowOp::StoreLocal { .. } | ControlFlowOp::LoadLocal(_) => {
                return Err(CodegenError::new(
                    instruction.span,
                    "native backend received locals before SSA promotion",
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
                    if !crate::semantic::is_type_assignable(inner, from) {
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
                Type::Int => "i",
                Type::Float => "f",
                Type::Bool => "b",
                Type::String => "s",
                Type::Function(_) | Type::GenericFunction(_) => "c",
                Type::Array(_) | Type::Class(_) | Type::ClassInstance { .. } => "p",
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
                Type::Int => format!("({expression}).i"),
                Type::Float => format!("({expression}).f"),
                Type::Bool => format!("({expression}).b"),
                Type::String => format!("({expression}).s"),
                Type::Function(_) | Type::GenericFunction(_) => format!("({expression}).c"),
                Type::Array(_) | Type::Class(_) | Type::ClassInstance { .. } => {
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
                IrBinaryOp::Mul => {
                    format!("(int32_t)((uint32_t){lhs_name}*(uint32_t){rhs_name})")
                }
                IrBinaryOp::Div => format!("lilscript_idiv({lhs_name},{rhs_name})"),
                IrBinaryOp::Mod => format!("lilscript_irem({lhs_name},{rhs_name})"),
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
                    write!(out, "return {converted};").expect("writing to String cannot fail");
                } else {
                    let converted = self.render_value_conversion(
                        &format!("v{}", value.0),
                        &types[value],
                        &function.return_type,
                        block.span,
                    )?;
                    write!(out, "return {converted};").expect("writing to String cannot fail");
                }
            }
            Terminator::Return(None) if function.kind == FunctionKind::Entry => {
                out.push_str("return 0;");
            }
            Terminator::Return(None) if function.kind == FunctionKind::Closure => {
                out.push_str("return (LilScriptValue){0};")
            }
            Terminator::Return(None) => out.push_str("return;"),
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

fn value_types<'src>(function: &ControlFlowFunction<'src>) -> AHashMap<ValueId, Type<'src>> {
    let mut types = AHashMap::new();
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
        Type::Int => 1,
        Type::Float => 2,
        Type::Bool => 3,
        Type::String => 4,
        Type::Function(_) | Type::GenericFunction(_) => 5,
        Type::Array(_) | Type::Class(_) | Type::ClassInstance { .. } => 6,
        Type::Struct(_) | Type::StructInstance { .. } | Type::Null | Type::Nullable(_) => 7,
        Type::TypeParameter(_) | Type::Union(_) | Type::Void => 0,
    }
}

fn c_type(ty: &Type<'_>) -> String {
    match ty {
        Type::Int => "int32_t".to_string(),
        Type::Float => "double".to_string(),
        Type::Bool => "bool".to_string(),
        Type::String => "const char*".to_string(),
        Type::Null | Type::Nullable(_) => "LilScriptOptional".to_string(),
        Type::Array(_) => "LilScriptArray".to_string(),
        Type::Struct(name) => aggregate_type_name("Struct", name),
        Type::Class(name) => aggregate_type_name("Class", name),
        Type::StructInstance { name, .. } => aggregate_type_name("Struct", name),
        Type::ClassInstance { name, .. } => aggregate_type_name("Class", name),
        Type::TypeParameter(_) | Type::Union(_) => "LilScriptValue".to_string(),
        Type::Function(_) | Type::GenericFunction(_) => "LilScriptClosure".to_string(),
        Type::Void => "void".to_string(),
    }
}

fn is_erased_type(ty: &Type<'_>) -> bool {
    matches!(ty, Type::TypeParameter(_) | Type::Union(_))
}

fn native_default_value(ty: &Type<'_>) -> Result<String, CodegenError> {
    Ok(match ty {
        Type::Int => "(int32_t)0".to_string(),
        Type::Float => "0.0".to_string(),
        Type::Bool => "false".to_string(),
        Type::String => "\"\"".to_string(),
        Type::Null | Type::Nullable(_) => "(LilScriptOptional){false,{0}}".to_string(),
        Type::Array(_) => "lilscript_array(0,sizeof(LilScriptValue))".to_string(),
        Type::Struct(_) | Type::StructInstance { .. } => format!("({}){{0}}", c_type(ty)),
        Type::Class(_) | Type::ClassInstance { .. } => "NULL".to_string(),
        Type::TypeParameter(_) => "(LilScriptValue){0}".to_string(),
        Type::Function(_) | Type::GenericFunction(_) => "(LilScriptClosure){0}".to_string(),
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
    fn adapts_named_functions_used_as_closures() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "bool same(int left,int right){return left==right;}func(int,int)->bool callback=same;print(callback(4,4));",
        )
        .unwrap();
        let c = compile_to_c(&program).unwrap();
        assert!(c.contains("static LilScriptValue a"));
        assert!(c.contains("(LilScriptClosure){(void*)a"));
    }
}
