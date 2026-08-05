use std::cell::RefCell;
use std::fmt::Write;

use ahash::{AHashMap, AHashSet};

use crate::codegen_js::CodegenError;
use crate::ir::{
    BlockId, ConstValue, ControlFlowFunction, ControlFlowInstruction, ControlFlowModule,
    ControlFlowOp, ControlShape, ExportBinding, FunctionId, FunctionKind, Intrinsic, IrBinaryOp,
    IrUnaryOp, TemplateOperand, Terminator, ValueId,
};
use crate::semantic::{EscapeState, SymbolId, Type};

pub fn emit_optimized_ir_js(module: &ControlFlowModule<'_>) -> Result<String, CodegenError> {
    emit_optimized_ir_js_with_options(module, &IrJsOptions::default())
}

pub fn emit_optimized_ir_js_module(module: &ControlFlowModule<'_>) -> Result<String, CodegenError> {
    emit_optimized_ir_js_module_with_options(module, &IrJsOptions::default())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IrJsOptions {
    pub mangle_identifiers: bool,
    pub mangle_properties: bool,
    pub mangle_exports: bool,
    pub pool_strings: bool,
}

impl Default for IrJsOptions {
    fn default() -> Self {
        Self {
            mangle_identifiers: true,
            mangle_properties: false,
            mangle_exports: false,
            pool_strings: true,
        }
    }
}

pub fn emit_optimized_ir_js_with_options(
    module: &ControlFlowModule<'_>,
    options: &IrJsOptions,
) -> Result<String, CodegenError> {
    IrJsEmitter::new(module, false, *options).emit()
}

pub fn emit_optimized_ir_js_module_with_options(
    module: &ControlFlowModule<'_>,
    options: &IrJsOptions,
) -> Result<String, CodegenError> {
    IrJsEmitter::new(module, true, *options).emit()
}

struct IrJsEmitter<'module, 'src> {
    module: &'module ControlFlowModule<'src>,
    global_names: AHashMap<SymbolId, String>,
    function_names: AHashMap<FunctionId, String>,
    top_level_mangler: Mangler,
    declared_globals: AHashSet<SymbolId>,
    string_aliases: AHashMap<String, String>,
    pooled_strings: Vec<(String, String)>,
    property_names: AHashMap<String, String>,
    module_output: bool,
    options: IrJsOptions,
}

impl<'module, 'src> IrJsEmitter<'module, 'src> {
    fn new(
        module: &'module ControlFlowModule<'src>,
        module_output: bool,
        options: IrJsOptions,
    ) -> Self {
        Self {
            module,
            global_names: AHashMap::new(),
            function_names: AHashMap::new(),
            top_level_mangler: Mangler::default(),
            declared_globals: AHashSet::new(),
            string_aliases: AHashMap::new(),
            pooled_strings: Vec::new(),
            property_names: AHashMap::new(),
            module_output,
            options,
        }
    }

    fn emit(mut self) -> Result<String, CodegenError> {
        self.assign_top_level_names();
        self.assign_string_aliases();
        self.assign_property_names();
        let entry = self.function(self.module.entry)?.clone();
        let entry_is_single_block = entry.blocks.len() == 1 && entry.blocks[0].phis.is_empty();
        let entry_can_structure = can_structure(&entry);
        let mut out = String::new();

        if !self.pooled_strings.is_empty() {
            out.push_str("let ");
            for (index, (value, name)) in self.pooled_strings.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                out.push_str(name);
                out.push('=');
                out.push_str(&render_string_literal(value));
            }
            out.push(';');
        }

        if !self.module.globals.is_empty() {
            out.push_str("let ");
            for (index, global) in self.module.globals.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                out.push_str(self.global_name(global.symbol)?);
                self.declared_globals.insert(global.symbol);
            }
            out.push(';');
        }

        let functions = self.module.functions.clone();
        for function in &functions {
            if !function.live
                || function.kind == FunctionKind::Entry
                || function.kind == FunctionKind::Extern
                || (function.kind == FunctionKind::Closure && can_inline_closure(function))
            {
                continue;
            }
            self.emit_function(function, &mut out)?;
        }

        if entry_is_single_block {
            self.emit_single_block(&entry, false, &mut out)?;
        } else if entry_can_structure {
            self.emit_structured(&entry, false, &mut out)?;
        } else {
            out.push_str("(()=>");
            self.emit_state_machine(&entry, &mut out)?;
            out.push_str(")();");
        }
        if self.module_output {
            self.emit_exports(&mut out)?;
        } else if out.ends_with(';') {
            out.pop();
        }
        Ok(out)
    }

    fn emit_exports(&self, out: &mut String) -> Result<(), CodegenError> {
        let runtime_exports = self
            .module
            .exports
            .iter()
            .filter(|export| export.binding != ExportBinding::TypeOnly)
            .collect::<Vec<_>>();
        if runtime_exports.is_empty() {
            return Ok(());
        }
        if !out.is_empty() && !out.ends_with(';') {
            out.push(';');
        }
        out.push_str("export{");
        for (index, export) in runtime_exports.iter().enumerate() {
            if index != 0 {
                out.push(',');
            }
            let internal = match export.binding {
                ExportBinding::Function(function) => self.function_name(function)?,
                ExportBinding::Global(symbol) => self.global_name(symbol)?,
                ExportBinding::TypeOnly => unreachable!(),
            };
            let public = if self.options.mangle_exports {
                internal
            } else {
                export.name
            };
            out.push_str(internal);
            if internal != public {
                out.push_str(" as ");
                out.push_str(public);
            }
        }
        out.push('}');
        Ok(())
    }

    fn assign_top_level_names(&mut self) {
        for function in &self.module.functions {
            if function.live && function.kind == FunctionKind::Extern {
                if let Some(name) = function.name {
                    self.top_level_mangler.reserve(name);
                    self.function_names.insert(function.id, name.to_string());
                }
            }
        }

        if !self.options.mangle_identifiers {
            for function in &self.module.functions {
                if !function.live
                    || matches!(function.kind, FunctionKind::Entry | FunctionKind::Extern)
                    || (function.kind == FunctionKind::Closure && can_inline_closure(function))
                {
                    continue;
                }
                let source_name = function.name.unwrap_or("closure");
                let preferred = match function.kind {
                    FunctionKind::Method { class } => format!("{class}${source_name}"),
                    FunctionKind::Constructor { class } => format!("{class}$init"),
                    FunctionKind::Closure => format!("closure${}", function.id.0),
                    _ => source_name.to_string(),
                };
                let name = self.top_level_mangler.unique_name(&preferred);
                self.function_names.insert(function.id, name);
            }
            for global in &self.module.globals {
                let name = self.top_level_mangler.unique_name(global.name);
                self.global_names.insert(global.symbol, name);
            }
            return;
        }

        let mut function_uses = AHashMap::<FunctionId, usize>::new();
        let mut global_uses = AHashMap::<SymbolId, usize>::new();
        for function in self
            .module
            .functions
            .iter()
            .filter(|function| function.live)
        {
            for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
                match &instruction.op {
                    ControlFlowOp::LoadGlobal(global)
                    | ControlFlowOp::StoreGlobal { global, .. } => {
                        *global_uses.entry(*global).or_insert(0) += 1;
                    }
                    ControlFlowOp::NewClass {
                        constructor: Some(function),
                        ..
                    }
                    | ControlFlowOp::Closure { function, .. }
                    | ControlFlowOp::CallDirect { function, .. }
                    | ControlFlowOp::CallMethod { function, .. } => {
                        *function_uses.entry(*function).or_insert(0) += 1;
                    }
                    _ => {}
                }
            }
        }

        let mut bindings = Vec::new();
        for function in &self.module.functions {
            if !function.live
                || matches!(function.kind, FunctionKind::Entry | FunctionKind::Extern)
                || (function.kind == FunctionKind::Closure && can_inline_closure(function))
            {
                continue;
            }
            bindings.push((
                function_uses.get(&function.id).copied().unwrap_or(0) + 1,
                0_u8,
                function.id.0,
            ));
        }
        for global in &self.module.globals {
            bindings.push((
                global_uses.get(&global.symbol).copied().unwrap_or(0) + 1,
                1_u8,
                global.symbol.0,
            ));
        }
        bindings.sort_unstable_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| left.1.cmp(&right.1))
                .then_with(|| left.2.cmp(&right.2))
        });
        for (_, kind, id) in bindings {
            let name = self.top_level_mangler.next_name();
            if kind == 0 {
                self.function_names.insert(FunctionId(id), name);
            } else {
                self.global_names.insert(SymbolId(id), name);
            }
        }
    }

    fn assign_string_aliases(&mut self) {
        if !self.options.pool_strings {
            return;
        }
        let mut counts = AHashMap::<String, usize>::new();
        for function in self
            .module
            .functions
            .iter()
            .filter(|function| function.live)
        {
            let uses = use_counts(function);
            for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
                if let (Some(out), ControlFlowOp::Const(ConstValue::String(value))) =
                    (instruction.out, &instruction.op)
                {
                    if uses.get(&out).copied().unwrap_or(0) != 0 {
                        *counts.entry(value.clone()).or_insert(0) += 1;
                    }
                }
            }
        }

        let mut candidates = counts
            .into_iter()
            .filter_map(|(value, count)| {
                let literal_length = value.len() + 2;
                let unaliased = count * literal_length;
                let aliased = literal_length + 7 + count;
                (unaliased > aliased).then(|| (unaliased - aliased, count, value))
            })
            .collect::<Vec<_>>();
        candidates.sort_unstable_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| right.1.cmp(&left.1))
                .then_with(|| left.2.cmp(&right.2))
        });
        for (_, _, value) in candidates {
            let name = self.top_level_mangler.next_name();
            self.string_aliases.insert(value.clone(), name.clone());
            self.pooled_strings.push((value, name));
        }
    }

    fn assign_property_names(&mut self) {
        if !self.options.mangle_properties {
            return;
        }
        let mut mangler = Mangler::default();
        for field in self
            .module
            .structs
            .iter()
            .chain(&self.module.classes)
            .flat_map(|layout| &layout.fields)
        {
            if !self.property_names.contains_key(field.name) {
                self.property_names
                    .insert(field.name.to_string(), mangler.next_name());
            }
        }
    }

    fn property_name<'name>(&'name self, field: &'name str) -> &'name str {
        self.property_names.get(field).map_or(field, String::as_str)
    }

    fn emit_function(
        &mut self,
        function: &ControlFlowFunction<'src>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let name = self.function_name(function.id)?.to_string();
        out.push_str("function ");
        out.push_str(&name);
        out.push('(');
        let single_block = function.blocks.len() == 1 && function.blocks[0].phis.is_empty();
        let structured = !single_block && can_structure(function);
        let mut context = LocalNames::new(
            function,
            !single_block && !structured,
            &self.top_level_mangler,
            self.options.mangle_identifiers,
        );
        context.inline_declarations = structured;
        for (index, param) in function.params.iter().enumerate() {
            if index != 0 {
                out.push(',');
            }
            out.push_str(context.value_name(param.value)?);
        }
        out.push(')');
        if let Some(expression) = self.render_conditional_return(function, &context)? {
            out.push_str("{return ");
            out.push_str(&expression);
            out.push('}');
            return Ok(());
        }
        if single_block {
            self.emit_single_block_with_context(function, true, context, out)
        } else if structured {
            self.emit_structured_with_context(function, true, context, out)
        } else {
            self.emit_state_machine_with_context(function, context, out)
        }
    }

    fn render_conditional_return(
        &mut self,
        function: &ControlFlowFunction<'src>,
        context: &LocalNames,
    ) -> Result<Option<String>, CodegenError> {
        let Some(crate::ir::ControlShape::If {
            then_block,
            else_block,
            ..
        }) = shape_at(function, function.entry)
        else {
            return Ok(None);
        };
        let header = &function.blocks[function.entry.0 as usize];
        if !header.phis.is_empty() {
            return Ok(None);
        }
        let Some(Terminator::Branch { condition, .. }) = header.terminator else {
            return Ok(None);
        };
        let uses = use_counts(function);
        let mut cache = AHashMap::new();
        for instruction in &header.instructions {
            let Some(out) = instruction.out else {
                return Ok(None);
            };
            if !expression_only_op(&instruction.op) {
                return Ok(None);
            }
            let expression = self.render_instruction_op(instruction, context, &mut cache)?;
            cache.insert(out, expression);
        }
        let condition = strip_outer_parens(take_value(condition, context, &mut cache)?);
        let Some(then_value) =
            self.render_linear_return_path(function, then_block, context, &uses, cache.clone())?
        else {
            return Ok(None);
        };
        let Some(else_value) =
            self.render_linear_return_path(function, else_block, context, &uses, cache)?
        else {
            return Ok(None);
        };
        Ok(Some(format!(
            "{condition}?{}:{}",
            strip_outer_parens(then_value),
            strip_outer_parens(else_value)
        )))
    }

    fn render_linear_return_path(
        &mut self,
        function: &ControlFlowFunction<'src>,
        mut block: BlockId,
        context: &LocalNames,
        uses: &AHashMap<ValueId, usize>,
        mut cache: AHashMap<ValueId, String>,
    ) -> Result<Option<String>, CodegenError> {
        let mut visited = AHashSet::new();
        loop {
            if !visited.insert(block) {
                return Ok(None);
            }
            let current = &function.blocks[block.0 as usize];
            if !current.phis.is_empty() {
                return Ok(None);
            }
            for instruction in &current.instructions {
                let Some(out) = instruction.out else {
                    return Ok(None);
                };
                if !expression_only_op(&instruction.op)
                    || (uses.get(&out).copied().unwrap_or(0) > 1 && !op_can_defer(&instruction.op))
                {
                    return Ok(None);
                }
                let expression = self.render_instruction_op(instruction, context, &mut cache)?;
                cache.insert(out, expression);
            }
            match current.terminator {
                Some(Terminator::Jump(target)) => block = target,
                Some(Terminator::Return(Some(value))) => {
                    return take_value(value, context, &mut cache).map(Some)
                }
                _ => return Ok(None),
            }
        }
    }

    fn emit_single_block(
        &mut self,
        function: &ControlFlowFunction<'src>,
        wrapped: bool,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let context = LocalNames::new(
            function,
            false,
            &self.top_level_mangler,
            self.options.mangle_identifiers,
        );
        self.emit_single_block_with_context(function, wrapped, context, out)
    }

    fn emit_single_block_with_context(
        &mut self,
        function: &ControlFlowFunction<'src>,
        wrapped: bool,
        context: LocalNames,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        if wrapped {
            out.push('{');
        }
        let block = &function.blocks[0];
        let uses = use_counts(function);
        let mut cache = AHashMap::<ValueId, String>::new();
        let mut previous_binding = false;
        for (index, instruction) in block.instructions.iter().enumerate() {
            let fuse_with_next = instruction.out.is_some_and(|value| {
                uses.get(&value).copied().unwrap_or(0) == 1 && can_fuse_value(block, index, value)
            });
            let mut statement = String::new();
            self.emit_linear_instruction(
                instruction,
                &uses,
                fuse_with_next,
                false,
                &context,
                &mut cache,
                &mut statement,
            )?;
            if statement.is_empty() {
                continue;
            }
            let binding = is_single_binding_statement(&statement);
            if previous_binding && binding {
                out.pop();
                out.push(',');
                out.push_str(&statement[4..]);
            } else {
                out.push_str(&statement);
            }
            previous_binding = binding;
        }
        match block.terminator.as_ref() {
            Some(Terminator::Return(Some(value))) => {
                out.push_str("return ");
                out.push_str(&take_value(*value, &context, &mut cache)?);
                out.push(';');
            }
            Some(Terminator::Return(None)) if function.kind != FunctionKind::Entry => {
                if function.return_type != Type::Void {
                    return Err(CodegenError::new(
                        function.span,
                        "non-void IR function has no value",
                    ));
                }
            }
            Some(Terminator::Return(None)) => {}
            Some(Terminator::Unreachable) => out.push_str("throw Error();"),
            _ => {
                return Err(CodegenError::new(
                    block.span,
                    "single-block function has a control-flow terminator",
                ));
            }
        }
        if wrapped {
            if out.ends_with(';') {
                out.pop();
            }
            out.push('}');
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_linear_instruction(
        &mut self,
        instruction: &ControlFlowInstruction<'src>,
        uses: &AHashMap<ValueId, usize>,
        fuse_with_next: bool,
        predeclared: bool,
        context: &LocalNames,
        cache: &mut AHashMap<ValueId, String>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        match &instruction.op {
            ControlFlowOp::StoreGlobal { global, value } => {
                let value = take_value(*value, context, cache)?;
                if self.declared_globals.insert(*global) {
                    out.push_str("let ");
                }
                out.push_str(self.global_name(*global)?);
                out.push('=');
                out.push_str(&value);
                out.push(';');
                return Ok(());
            }
            ControlFlowOp::FieldSet {
                object,
                field,
                index,
                value,
                ..
            } => {
                out.push_str(&take_value(*object, context, cache)?);
                if context.is_untyped(*object) {
                    write!(out, ".{}=", self.property_name(field))
                        .expect("writing to String cannot fail");
                } else {
                    write!(out, "[{index}]=").expect("writing to String cannot fail");
                }
                out.push_str(&take_value(*value, context, cache)?);
                out.push(';');
                return Ok(());
            }
            ControlFlowOp::IndexSet {
                object,
                index,
                value,
            } => {
                out.push_str(&take_value(*object, context, cache)?);
                out.push('[');
                out.push_str(&take_value(*index, context, cache)?);
                out.push_str("]=");
                out.push_str(&take_value(*value, context, cache)?);
                out.push(';');
                return Ok(());
            }
            ControlFlowOp::NewClass {
                class,
                constructor: Some(constructor),
                args,
            } => {
                let result = instruction.out.ok_or_else(|| {
                    CodegenError::new(instruction.span, "class construction has no result")
                })?;
                let name = context.value_name(result)?;
                emit_binding_prefix(context, result, predeclared, out)?;
                out.push_str(name);
                out.push('=');
                out.push_str(&self.default_class_value(class, context.is_untyped(result))?);
                out.push(';');
                out.push_str(self.function_name(*constructor)?);
                out.push('(');
                out.push_str(name);
                for arg in args {
                    out.push(',');
                    out.push_str(&take_value(*arg, context, cache)?);
                }
                out.push_str(");");
                return Ok(());
            }
            _ => {}
        }

        if instruction
            .out
            .is_some_and(|value| context.inlined_values.contains_key(&value))
        {
            return Ok(());
        }

        let expression = self.render_instruction_op(instruction, context, cache)?;
        let Some(out_value) = instruction.out else {
            if !expression.is_empty() {
                out.push_str(&expression);
                out.push(';');
            }
            return Ok(());
        };
        let use_count = uses.get(&out_value).copied().unwrap_or(0);
        if use_count == 0 {
            if op_has_side_effects(&instruction.op) {
                out.push_str(&expression);
                out.push(';');
            }
        } else if use_count == 1
            && !context.is_stored(out_value)
            && (op_can_defer(&instruction.op) || fuse_with_next)
        {
            cache.insert(out_value, expression);
        } else {
            emit_binding_prefix(context, out_value, predeclared, out)?;
            out.push_str(context.value_name(out_value)?);
            out.push('=');
            out.push_str(&expression);
            out.push(';');
        }
        Ok(())
    }

    fn emit_state_machine(
        &mut self,
        function: &ControlFlowFunction<'src>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let context = LocalNames::new(
            function,
            true,
            &self.top_level_mangler,
            self.options.mangle_identifiers,
        );
        self.emit_state_machine_with_context(function, context, out)
    }

    fn emit_state_machine_with_context(
        &mut self,
        function: &ControlFlowFunction<'src>,
        context: LocalNames,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        out.push('{');
        let declared = context.non_parameter_names(function);
        if !declared.is_empty() {
            out.push_str("let ");
            for (index, name) in declared.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                out.push_str(name);
            }
            out.push(';');
        }
        let state = context.state_name();
        out.push_str("let ");
        out.push_str(state);
        write!(out, "={};for(;;)switch({state}){{", function.entry.0)
            .expect("writing to String cannot fail");

        let uses = use_counts(function);
        for block in &function.blocks {
            write!(out, "case {}:", block.id.0).expect("writing to String cannot fail");
            let mut cache = AHashMap::new();
            for (index, instruction) in block.instructions.iter().enumerate() {
                let fuse_with_next = instruction.out.is_some_and(|value| {
                    uses.get(&value).copied().unwrap_or(0) == 1
                        && can_fuse_value(block, index, value)
                });
                self.emit_linear_instruction(
                    instruction,
                    &uses,
                    fuse_with_next,
                    true,
                    &context,
                    &mut cache,
                    out,
                )?;
            }
            match block
                .terminator
                .as_ref()
                .ok_or_else(|| CodegenError::new(block.span, "IR block has no terminator"))?
            {
                Terminator::Jump(target) => {
                    self.emit_phi_edge_cached(
                        function, block.id.0, target.0, &context, &mut cache, out,
                    )?;
                    write!(out, "{state}={};continue;", target.0)
                        .expect("writing to String cannot fail");
                }
                Terminator::Branch {
                    condition,
                    then_block,
                    else_block,
                } => {
                    let condition = take_value(*condition, &context, &mut cache)?;
                    out.push_str("if(");
                    out.push_str(&condition);
                    out.push_str("){");
                    let mut then_cache = cache.clone();
                    self.emit_phi_edge_cached(
                        function,
                        block.id.0,
                        then_block.0,
                        &context,
                        &mut then_cache,
                        out,
                    )?;
                    write!(out, "{state}={}", then_block.0).expect("writing to String cannot fail");
                    out.push_str("}else{");
                    self.emit_phi_edge_cached(
                        function,
                        block.id.0,
                        else_block.0,
                        &context,
                        &mut cache,
                        out,
                    )?;
                    write!(out, "{state}={}", else_block.0).expect("writing to String cannot fail");
                    out.push_str("}continue;");
                }
                Terminator::Return(Some(value)) => {
                    out.push_str("return ");
                    out.push_str(&take_value(*value, &context, &mut cache)?);
                    out.push(';');
                }
                Terminator::Return(None) => out.push_str("return;"),
                Terminator::Unreachable => out.push_str("throw Error();"),
            }
        }
        out.push_str("}}");
        Ok(())
    }

    fn emit_structured(
        &mut self,
        function: &ControlFlowFunction<'src>,
        wrapped: bool,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let mut context = LocalNames::new(
            function,
            false,
            &self.top_level_mangler,
            self.options.mangle_identifiers,
        );
        context.inline_declarations = true;
        self.emit_structured_with_context(function, wrapped, context, out)
    }

    fn emit_structured_with_context(
        &mut self,
        function: &ControlFlowFunction<'src>,
        wrapped: bool,
        context: LocalNames,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        if wrapped {
            out.push('{');
        }
        let declared = context.non_parameter_names(function);
        if !context.inline_declarations && !declared.is_empty() {
            out.push_str("let ");
            for (index, name) in declared.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                out.push_str(name);
            }
            out.push(';');
        }
        let uses = use_counts(function);
        let mut visited = AHashSet::new();
        let mut cache = AHashMap::new();
        self.emit_structured_path(
            function,
            function.entry,
            None,
            None,
            &context,
            &uses,
            &mut cache,
            &mut visited,
            out,
        )?;
        if wrapped {
            out.push('}');
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_structured_path(
        &mut self,
        function: &ControlFlowFunction<'src>,
        mut current: BlockId,
        stop: Option<BlockId>,
        loop_context: Option<LoopContext>,
        context: &LocalNames,
        uses: &AHashMap<ValueId, usize>,
        cache: &mut AHashMap<ValueId, String>,
        visited: &mut AHashSet<BlockId>,
        out: &mut String,
    ) -> Result<PathEnd, CodegenError> {
        loop {
            if Some(current) == stop {
                return Ok(PathEnd::ReachedStop);
            }
            if !visited.insert(current) {
                return Err(CodegenError::new(
                    function.blocks[current.0 as usize].span,
                    "structured CFG traversal encountered an unexpected cycle",
                ));
            }

            if let Some(shape) = shape_at(function, current) {
                self.flush_cache(cache, context, out)?;
                match shape {
                    ControlShape::If {
                        header,
                        then_block,
                        else_block,
                        merge_block,
                    } => {
                        let block = &function.blocks[header.0 as usize];
                        self.emit_cached_block(block, uses, context, cache, out)?;
                        let Some(Terminator::Branch { condition, .. }) = block.terminator else {
                            return Err(CodegenError::new(
                                block.span,
                                "if shape header is not a branch",
                            ));
                        };
                        let condition = strip_outer_parens(take_value(condition, context, cache)?);
                        let mut then_visited = visited.clone();
                        let mut then_cache = cache.clone();
                        let mut then_output = String::new();
                        self.emit_structured_path(
                            function,
                            then_block,
                            Some(merge_block),
                            loop_context,
                            context,
                            uses,
                            &mut then_cache,
                            &mut then_visited,
                            &mut then_output,
                        )?;
                        let mut else_visited = visited.clone();
                        let mut else_cache = cache.clone();
                        let mut else_output = String::new();
                        self.emit_structured_path(
                            function,
                            else_block,
                            Some(merge_block),
                            loop_context,
                            context,
                            uses,
                            &mut else_cache,
                            &mut else_visited,
                            &mut else_output,
                        )?;
                        if let Some((target, then_value, else_value)) =
                            merge_conditional_assignments(&then_output, &else_output)
                        {
                            out.push_str(target);
                            out.push('=');
                            out.push_str(&condition);
                            out.push('?');
                            out.push_str(then_value);
                            out.push(':');
                            out.push_str(else_value);
                            out.push(';');
                        } else if else_output.is_empty() {
                            out.push_str("if(");
                            out.push_str(&condition);
                            if matches!(then_output.as_str(), "continue;" | "break;") {
                                out.push(')');
                                out.push_str(&then_output);
                            } else {
                                out.push_str("){");
                                out.push_str(&then_output);
                                out.push('}');
                            }
                        } else if then_output.is_empty() {
                            out.push_str("if(");
                            out.push_str(&negate_condition(condition));
                            out.push_str("){");
                            out.push_str(&else_output);
                            out.push('}');
                        } else {
                            out.push_str("if(");
                            out.push_str(&condition);
                            out.push_str("){");
                            out.push_str(&then_output);
                            out.push_str("}else{");
                            out.push_str(&else_output);
                            out.push('}');
                        }
                        cache.clear();
                        current = merge_block;
                        continue;
                    }
                    ControlShape::Loop {
                        header,
                        body,
                        update,
                        exit,
                    } => {
                        let block = &function.blocks[header.0 as usize];
                        let Some(Terminator::Branch {
                            condition,
                            then_block,
                            else_block,
                        }) = block.terminator
                        else {
                            return Err(CodegenError::new(
                                block.span,
                                "loop shape header is not a branch",
                            ));
                        };
                        let body_on_true = then_block == body && else_block == exit;
                        let body_on_false = else_block == body && then_block == exit;
                        if !body_on_true && !body_on_false {
                            return Err(CodegenError::new(
                                block.span,
                                "loop branch does not target its body and exit",
                            ));
                        }
                        let mut header_output = String::new();
                        self.emit_cached_block(block, uses, context, cache, &mut header_output)?;
                        let condition = strip_outer_parens(take_value(condition, context, cache)?);
                        let mut exit_output = String::new();
                        let mut exit_cache = cache.clone();
                        self.emit_phi_edge_cached(
                            function,
                            header.0,
                            exit.0,
                            context,
                            &mut exit_cache,
                            &mut exit_output,
                        )?;
                        let compact_loop = header_output.is_empty() && exit_output.is_empty();
                        let update_clause = if compact_loop {
                            if let Some(update_block) = update {
                                let mut update_visited = AHashSet::new();
                                let mut update_cache = AHashMap::new();
                                let mut update_output = String::new();
                                let update_end = self.emit_structured_path(
                                    function,
                                    update_block,
                                    Some(header),
                                    None,
                                    context,
                                    uses,
                                    &mut update_cache,
                                    &mut update_visited,
                                    &mut update_output,
                                )?;
                                (update_end == PathEnd::ReachedStop)
                                    .then(|| for_update_clause(&update_output))
                                    .flatten()
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        if let Some(update_clause) = &update_clause {
                            out.push_str("for(;");
                            if body_on_true {
                                out.push_str(&condition);
                            } else {
                                out.push_str(&negate_condition(condition.clone()));
                            }
                            out.push(';');
                            out.push_str(update_clause);
                            out.push_str("){");
                        } else if compact_loop {
                            let reuse_for_spelling =
                                out.matches("for(").count() > out.matches("while(").count();
                            if reuse_for_spelling {
                                out.push_str("for(;");
                            } else {
                                out.push_str("while(");
                            }
                            if body_on_true {
                                out.push_str(&condition);
                            } else {
                                out.push_str(&negate_condition(condition.clone()));
                            }
                            if reuse_for_spelling {
                                out.push(';');
                            }
                            out.push_str("){");
                        } else {
                            out.push_str("for(;;){");
                            out.push_str(&header_output);
                            out.push_str("if(");
                            if body_on_true {
                                out.push_str(&negate_condition(condition.clone()));
                            } else {
                                out.push_str(&condition);
                            }
                            out.push_str("){");
                            out.push_str(&exit_output);
                            out.push_str("break}");
                        }

                        let continue_target = update.unwrap_or(header);
                        let nested_loop = LoopContext {
                            header,
                            continue_target,
                            update: update_clause.is_none().then_some(update).flatten(),
                            exit,
                        };
                        let mut body_visited = visited.clone();
                        let mut body_cache = cache.clone();
                        let body_end = self.emit_structured_path(
                            function,
                            body,
                            Some(continue_target),
                            Some(nested_loop),
                            context,
                            uses,
                            &mut body_cache,
                            &mut body_visited,
                            out,
                        )?;
                        if body_end == PathEnd::ReachedStop && update_clause.is_none() {
                            if let Some(update_block) = update {
                                let mut update_visited = AHashSet::new();
                                let mut update_cache = body_cache;
                                self.emit_structured_path(
                                    function,
                                    update_block,
                                    Some(header),
                                    None,
                                    context,
                                    uses,
                                    &mut update_cache,
                                    &mut update_visited,
                                    out,
                                )?;
                            }
                        }
                        out.push('}');
                        cache.clear();
                        current = exit;
                        continue;
                    }
                }
            }

            let block = &function.blocks[current.0 as usize];
            self.emit_cached_block(block, uses, context, cache, out)?;
            match block
                .terminator
                .as_ref()
                .ok_or_else(|| CodegenError::new(block.span, "IR block has no terminator"))?
            {
                Terminator::Jump(target) => {
                    self.emit_phi_edge_cached(function, current.0, target.0, context, cache, out)?;
                    if Some(*target) == stop {
                        return Ok(PathEnd::ReachedStop);
                    }
                    if let Some(loop_context) = loop_context {
                        if *target == loop_context.exit {
                            out.push_str("break;");
                            return Ok(PathEnd::Terminated);
                        }
                        if *target == loop_context.continue_target {
                            if let Some(update) = loop_context.update {
                                if current != update {
                                    let mut update_visited = AHashSet::new();
                                    let mut update_cache = AHashMap::new();
                                    self.emit_structured_path(
                                        function,
                                        update,
                                        Some(loop_context.header),
                                        None,
                                        context,
                                        uses,
                                        &mut update_cache,
                                        &mut update_visited,
                                        out,
                                    )?;
                                }
                            }
                            out.push_str("continue;");
                            return Ok(PathEnd::Terminated);
                        }
                    }
                    current = *target;
                }
                Terminator::Return(Some(value)) => {
                    out.push_str("return ");
                    out.push_str(&take_value(*value, context, cache)?);
                    out.push(';');
                    return Ok(PathEnd::Terminated);
                }
                Terminator::Return(None) => {
                    if function.kind != FunctionKind::Entry {
                        out.push_str("return;");
                    }
                    return Ok(PathEnd::Terminated);
                }
                Terminator::Unreachable => {
                    out.push_str("throw Error();");
                    return Ok(PathEnd::Terminated);
                }
                Terminator::Branch { .. } => {
                    return Err(CodegenError::new(
                        block.span,
                        "branch block has no structured shape",
                    ));
                }
            }
        }
    }

    fn emit_cached_block(
        &mut self,
        block: &crate::ir::ControlFlowBlock<'src>,
        uses: &AHashMap<ValueId, usize>,
        context: &LocalNames,
        cache: &mut AHashMap<ValueId, String>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        for (index, instruction) in block.instructions.iter().enumerate() {
            let fuse_with_next = instruction.out.is_some_and(|value| {
                uses.get(&value).copied().unwrap_or(0) == 1 && can_fuse_value(block, index, value)
            });
            self.emit_linear_instruction(
                instruction,
                uses,
                fuse_with_next,
                true,
                context,
                cache,
                out,
            )?;
        }
        Ok(())
    }

    fn flush_cache(
        &self,
        cache: &mut AHashMap<ValueId, String>,
        context: &LocalNames,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let mut values = cache.drain().collect::<Vec<_>>();
        values.sort_by_key(|(value, _)| value.0);
        for (value, expression) in values {
            if context.claim_declaration(value)? {
                out.push_str("var ");
            }
            out.push_str(context.value_name(value)?);
            out.push('=');
            out.push_str(&expression);
            out.push(';');
        }
        Ok(())
    }

    fn emit_phi_edge_cached(
        &self,
        function: &ControlFlowFunction<'src>,
        from: u32,
        to: u32,
        context: &LocalNames,
        cache: &mut AHashMap<ValueId, String>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let copies = function.blocks[to as usize]
            .phis
            .iter()
            .filter_map(|phi| {
                phi.incoming
                    .iter()
                    .find(|(block, _)| block.0 == from)
                    .map(|(_, value)| (phi.out, *value))
            })
            .collect::<Vec<_>>();
        let mut sources = Vec::with_capacity(copies.len());
        for (_, source) in &copies {
            sources.push(strip_outer_parens(take_value(*source, context, cache)?));
        }
        let mut assignments = Vec::with_capacity(copies.len());
        let mut declaration_needed = false;
        for ((target, _), source) in copies.iter().zip(sources) {
            let target_value = *target;
            let target = context.value_name(target_value)?.to_string();
            if target != source {
                declaration_needed |= context.claim_declaration(target_value)?;
                assignments.push((target, source));
            }
        }
        if assignments.len() == 1 {
            if declaration_needed {
                out.push_str("var ");
            }
            out.push_str(&assignments[0].0);
            out.push('=');
            out.push_str(&assignments[0].1);
            if declaration_needed {
                for name in context.claim_remaining_declarations() {
                    out.push(',');
                    out.push_str(&name);
                }
            }
            out.push(';');
        } else if !assignments.is_empty() {
            let targets = assignments
                .iter()
                .map(|(target, _)| target.as_str())
                .collect::<AHashSet<_>>();
            let scalar_declaration = declaration_needed
                && assignments
                    .iter()
                    .all(|(_, source)| !targets.contains(source.as_str()));
            if scalar_declaration {
                out.push_str("var ");
                for (index, (target, source)) in assignments.iter().enumerate() {
                    if index != 0 {
                        out.push(',');
                    }
                    out.push_str(target);
                    out.push('=');
                    out.push_str(source);
                }
                for name in context.claim_remaining_declarations() {
                    out.push(',');
                    out.push_str(&name);
                }
                out.push(';');
                return Ok(());
            }
            if !declaration_needed {
                if let Some(ordered) = order_scalar_assignments(&assignments) {
                    let mut scalar = String::new();
                    for (target, source) in ordered {
                        scalar.push_str(target);
                        scalar.push('=');
                        scalar.push_str(source);
                        scalar.push(';');
                    }
                    let tuple_size = assignments
                        .iter()
                        .map(|(target, source)| target.len() + source.len())
                        .sum::<usize>()
                        + assignments.len().saturating_sub(1) * 2
                        + 6;
                    if scalar.len() < tuple_size {
                        out.push_str(&scalar);
                        return Ok(());
                    }
                }
            }
            if declaration_needed {
                out.push_str("var ");
            }
            out.push('[');
            for (index, (target, _)) in assignments.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                out.push_str(target);
            }
            out.push_str("]=[");
            for (index, (_, source)) in assignments.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                out.push_str(source);
            }
            out.push_str("];");
        }
        Ok(())
    }

    fn render_instruction_op(
        &mut self,
        instruction: &ControlFlowInstruction<'src>,
        context: &LocalNames,
        cache: &mut AHashMap<ValueId, String>,
    ) -> Result<String, CodegenError> {
        if instruction.ty.as_ref() == Some(&Type::Int) {
            match &instruction.op {
                ControlFlowOp::Unary {
                    op: IrUnaryOp::Neg,
                    value,
                } => {
                    return Ok(format!("(-{}|0)", take_value(*value, context, cache)?));
                }
                ControlFlowOp::Binary { op, lhs, rhs }
                    if matches!(
                        op,
                        IrBinaryOp::Add
                            | IrBinaryOp::Sub
                            | IrBinaryOp::Mul
                            | IrBinaryOp::Div
                            | IrBinaryOp::Mod
                    ) =>
                {
                    let lhs = take_value(*lhs, context, cache)?;
                    let rhs = take_value(*rhs, context, cache)?;
                    return Ok(match op {
                        IrBinaryOp::Mul => format!("Math.imul({lhs},{rhs})"),
                        _ => format!("({lhs}{}{rhs}|0)", binary_operator(*op)),
                    });
                }
                _ => {}
            }
        }
        let boundary = instruction.out.is_some_and(|out| context.is_untyped(out));
        match &instruction.op {
            ControlFlowOp::Struct { name, fields } if boundary => {
                let layout = self
                    .module
                    .structs
                    .iter()
                    .find(|layout| layout.name == *name)
                    .ok_or_else(|| {
                        CodegenError::new(instruction.span, "missing boundary struct layout")
                    })?;
                let mut rendered = String::from("{");
                for (index, (field, value)) in layout.fields.iter().zip(fields).enumerate() {
                    if index != 0 {
                        rendered.push(',');
                    }
                    rendered.push_str(self.property_name(field.name));
                    rendered.push(':');
                    rendered.push_str(&take_value(*value, context, cache)?);
                }
                rendered.push('}');
                Ok(rendered)
            }
            ControlFlowOp::NewClass {
                class,
                constructor: None,
                args,
            } if boundary && args.is_empty() => self.default_class_value(class, true),
            _ => self.render_op(&instruction.op, context, cache),
        }
    }

    fn render_op(
        &mut self,
        op: &ControlFlowOp<'src>,
        context: &LocalNames,
        cache: &mut AHashMap<ValueId, String>,
    ) -> Result<String, CodegenError> {
        let value = |id, cache: &mut AHashMap<_, _>| take_value(id, context, cache);
        Ok(match op {
            ControlFlowOp::Const(ConstValue::String(value)) => self
                .string_aliases
                .get(value)
                .cloned()
                .unwrap_or_else(|| render_string_literal(value)),
            ControlFlowOp::Const(value) => render_const(value),
            ControlFlowOp::Unary { op, value: operand } => format!(
                "{}{}",
                match op {
                    IrUnaryOp::Neg => "-",
                    IrUnaryOp::Not => "!",
                },
                value(*operand, cache)?
            ),
            ControlFlowOp::Binary { op, lhs, rhs } => format!(
                "({}{}{})",
                value(*lhs, cache)?,
                binary_operator(*op),
                value(*rhs, cache)?
            ),
            ControlFlowOp::Array(values) | ControlFlowOp::Struct { fields: values, .. } => {
                let mut rendered = String::from("[");
                for (index, item) in values.iter().enumerate() {
                    if index != 0 {
                        rendered.push(',');
                    }
                    rendered.push_str(&value(*item, cache)?);
                }
                rendered.push(']');
                rendered
            }
            ControlFlowOp::NewClass {
                class,
                constructor: None,
                args,
            } if args.is_empty() => self.default_class_value(class, false)?,
            ControlFlowOp::Closure { function, captures } => {
                let captures = captures
                    .iter()
                    .map(|capture| value(*capture, cache))
                    .collect::<Result<Vec<_>, _>>()?;
                self.render_closure(*function, &captures)?
            }
            ControlFlowOp::LoadGlobal(symbol) => self.global_name(*symbol)?.to_string(),
            ControlFlowOp::FieldGet {
                object,
                field,
                index,
                ..
            } => {
                let object_value = value(*object, cache)?;
                if context.is_untyped(*object) {
                    format!("{object_value}.{}", self.property_name(field))
                } else {
                    format!("{object_value}[{index}]")
                }
            }
            ControlFlowOp::IndexGet { object, index } => {
                format!("{}[{}]", value(*object, cache)?, value(*index, cache)?)
            }
            ControlFlowOp::CallDirect { function, args } => {
                self.render_call(self.function_name(*function)?, None, args, context, cache)?
            }
            ControlFlowOp::CallValue { callee, args } => {
                let mut callee = value(*callee, cache)?;
                if callee.contains("=>") {
                    callee = format!("({callee})");
                }
                self.render_call(&callee, None, args, context, cache)?
            }
            ControlFlowOp::Intrinsic {
                intrinsic,
                receiver,
                args,
            } => self.render_intrinsic(*intrinsic, *receiver, args, context, cache)?,
            ControlFlowOp::Template(parts) => {
                let mut rendered = String::from("`");
                for part in parts {
                    match part {
                        TemplateOperand::String(string) => rendered.push_str(string),
                        TemplateOperand::Value(item) => {
                            rendered.push_str("${");
                            rendered.push_str(&value(*item, cache)?);
                            rendered.push('}');
                        }
                    }
                }
                rendered.push('`');
                rendered
            }
            ControlFlowOp::StoreLocal { .. }
            | ControlFlowOp::LoadLocal(_)
            | ControlFlowOp::StoreGlobal { .. }
            | ControlFlowOp::FieldSet { .. }
            | ControlFlowOp::IndexSet { .. }
            | ControlFlowOp::NewClass { .. }
            | ControlFlowOp::CallMethod { .. } => {
                return Err(CodegenError::new(
                    self.function(self.module.entry)?.span,
                    "IR contains an operation that must be lowered before expression emission",
                ));
            }
        })
    }

    fn render_call(
        &self,
        callee: &str,
        receiver: Option<ValueId>,
        args: &[ValueId],
        context: &LocalNames,
        cache: &mut AHashMap<ValueId, String>,
    ) -> Result<String, CodegenError> {
        let mut rendered = String::new();
        rendered.push_str(callee);
        rendered.push('(');
        let mut first = true;
        if let Some(receiver) = receiver {
            rendered.push_str(&take_value(receiver, context, cache)?);
            first = false;
        }
        for arg in args {
            if !first {
                rendered.push(',');
            }
            first = false;
            rendered.push_str(&strip_outer_parens(take_value(*arg, context, cache)?));
        }
        rendered.push(')');
        Ok(rendered)
    }

    fn render_intrinsic(
        &self,
        intrinsic: Intrinsic,
        receiver: Option<ValueId>,
        args: &[ValueId],
        context: &LocalNames,
        cache: &mut AHashMap<ValueId, String>,
    ) -> Result<String, CodegenError> {
        if intrinsic == Intrinsic::Print {
            return self.render_call("console.log", None, args, context, cache);
        }
        let receiver = receiver.ok_or_else(|| {
            CodegenError::new(
                self.function(self.module.entry).unwrap().span,
                "missing receiver",
            )
        })?;
        let receiver = take_value(receiver, context, cache)?;
        let property = match intrinsic {
            Intrinsic::UnwrapNullable => return Ok(receiver),
            Intrinsic::ArrayLength | Intrinsic::StringLength => {
                return Ok(format!("{receiver}.length"))
            }
            Intrinsic::ArrayMap => "map",
            Intrinsic::ArrayFilter => "filter",
            Intrinsic::ArrayReduce => "reduce",
            Intrinsic::ArrayForEach => "forEach",
            Intrinsic::ArrayPush => "push",
            Intrinsic::ArrayPop => "pop",
            Intrinsic::StringIncludes => "includes",
            Intrinsic::StringStartsWith => "startsWith",
            Intrinsic::StringEndsWith => "endsWith",
            Intrinsic::StringToUpperCase => "toUpperCase",
            Intrinsic::StringToLowerCase => "toLowerCase",
            Intrinsic::Print => unreachable!(),
        };
        self.render_call(
            &format!("{receiver}.{property}"),
            None,
            args,
            context,
            cache,
        )
    }

    fn render_closure(
        &mut self,
        function: FunctionId,
        captures: &[String],
    ) -> Result<String, CodegenError> {
        let function = self.function(function)?.clone();
        if captures.len() != function.capture_count {
            return Err(CodegenError::new(
                function.span,
                "closure capture count does not match its IR function",
            ));
        }
        if !can_inline_closure(&function) {
            let name = self.function_name(function.id)?;
            if captures.is_empty() {
                return Ok(name.to_string());
            }
            let mut wrapper_mangler = self.top_level_mangler.clone();
            for capture in captures {
                reserve_expression_identifiers(&mut wrapper_mangler, capture);
            }
            let context = LocalNames::new(
                &function,
                false,
                &wrapper_mangler,
                self.options.mangle_identifiers,
            );
            let mut rendered = render_arrow_parameters(&function, &context)?;
            rendered.push_str("=>");
            rendered.push_str(name);
            rendered.push('(');
            for (index, capture) in captures.iter().enumerate() {
                if index != 0 {
                    rendered.push(',');
                }
                rendered.push_str(capture);
            }
            for param in &function.params[function.capture_count..] {
                if !captures.is_empty() || param.value != function.params[0].value {
                    rendered.push(',');
                }
                rendered.push_str(context.value_name(param.value)?);
            }
            rendered.push(')');
            return Ok(rendered);
        }
        let mut context = LocalNames::new(
            &function,
            false,
            &self.top_level_mangler,
            self.options.mangle_identifiers,
        );
        let capture_params = &function.params[..function.capture_count];
        let capture_values = capture_params
            .iter()
            .map(|parameter| parameter.value)
            .collect::<AHashSet<_>>();
        let hidden_names = capture_params
            .iter()
            .filter_map(|parameter| context.value_names.get(&parameter.value))
            .cloned()
            .collect::<AHashSet<_>>();
        let mut mangler = self.top_level_mangler.clone();
        for name in context.value_names.values().chain(captures) {
            mangler.reserve(name);
        }
        let mut replacements = AHashMap::<String, String>::new();
        for (value, name) in &context.value_names {
            if !capture_values.contains(value)
                && (hidden_names.contains(name) || captures.contains(name))
            {
                replacements
                    .entry(name.clone())
                    .or_insert_with(|| mangler.next_name());
            }
        }
        for (value, name) in &mut context.value_names {
            if !capture_values.contains(value) {
                if let Some(replacement) = replacements.get(name) {
                    *name = replacement.clone();
                }
            }
        }
        for (param, capture) in function.params.iter().zip(captures) {
            context.value_names.insert(param.value, capture.clone());
        }
        *context.declared_names.borrow_mut() = function.params[function.capture_count..]
            .iter()
            .filter_map(|parameter| context.value_names.get(&parameter.value))
            .cloned()
            .collect();
        let expression_closure = matches!(
            function.blocks[0].terminator,
            Some(Terminator::Return(Some(_)))
        ) && function.blocks[0]
            .instructions
            .iter()
            .all(|instruction| op_can_defer(&instruction.op));
        if !expression_closure {
            let parameters = render_arrow_parameters(&function, &context)?;
            let mut body = String::new();
            self.emit_single_block_with_context(&function, true, context, &mut body)?;
            return Ok(format!("{parameters}=>{body}"));
        }
        let uses = use_counts(&function);
        let mut cache = AHashMap::new();
        let mut prefix = String::new();
        for instruction in &function.blocks[0].instructions {
            if !op_can_defer(&instruction.op) {
                return Err(CodegenError::new(
                    instruction.span,
                    "effectful closure requires named function emission",
                ));
            }
            let expression = self.render_instruction_op(instruction, &context, &mut cache)?;
            let out = instruction.out.ok_or_else(|| {
                CodegenError::new(instruction.span, "closure value has no output")
            })?;
            if uses.get(&out).copied().unwrap_or(0) == 1 {
                cache.insert(out, expression);
            } else {
                prefix.push_str("let ");
                prefix.push_str(context.value_name(out)?);
                prefix.push('=');
                prefix.push_str(&expression);
                prefix.push(';');
            }
        }
        let Some(Terminator::Return(Some(value))) = function.blocks[0].terminator else {
            return Err(CodegenError::new(
                function.span,
                "closure has no returned expression",
            ));
        };
        let returned = strip_outer_parens(take_value(value, &context, &mut cache)?);
        let mut rendered = render_arrow_parameters(&function, &context)?;
        rendered.push_str("=>");
        if prefix.is_empty() {
            rendered.push_str(&returned);
        } else {
            rendered.push('{');
            rendered.push_str(&prefix);
            rendered.push_str("return ");
            rendered.push_str(&returned);
            rendered.push('}');
        }
        Ok(rendered)
    }

    fn default_class_value(&self, class: &str, boundary: bool) -> Result<String, CodegenError> {
        let layout = self
            .module
            .classes
            .iter()
            .find(|layout| layout.name == class)
            .ok_or_else(|| {
                CodegenError::new(
                    self.function(self.module.entry).unwrap().span,
                    "missing class layout",
                )
            })?;
        let mut value = String::from(if boundary { "{" } else { "[" });
        for (index, field) in layout.fields.iter().enumerate() {
            if index != 0 {
                value.push(',');
            }
            if boundary {
                value.push_str(self.property_name(field.name));
                value.push(':');
            }
            value.push_str(default_value(&field.ty));
        }
        value.push(if boundary { '}' } else { ']' });
        Ok(value)
    }

    fn function(&self, id: FunctionId) -> Result<&ControlFlowFunction<'src>, CodegenError> {
        self.module.functions.get(id.0 as usize).ok_or_else(|| {
            CodegenError::new(
                self.module
                    .functions
                    .first()
                    .map_or(crate::span::Span::empty(0), |function| function.span),
                format!("missing IR function {}", id.0),
            )
        })
    }

    fn function_name(&self, id: FunctionId) -> Result<&str, CodegenError> {
        self.function_names
            .get(&id)
            .map(String::as_str)
            .ok_or_else(|| {
                CodegenError::new(
                    self.function(id)
                        .map_or(crate::span::Span::empty(0), |function| function.span),
                    format!("function {} has no emitted name", id.0),
                )
            })
    }

    fn global_name(&self, symbol: SymbolId) -> Result<&str, CodegenError> {
        self.global_names
            .get(&symbol)
            .map(String::as_str)
            .ok_or_else(|| {
                CodegenError::new(
                    self.function(self.module.entry).unwrap().span,
                    format!("global symbol {} has no emitted name", symbol.0),
                )
            })
    }
}

fn order_scalar_assignments(assignments: &[(String, String)]) -> Option<Vec<(&str, &str)>> {
    let mut remaining = assignments.iter().collect::<Vec<_>>();
    let mut ordered = Vec::with_capacity(assignments.len());
    while !remaining.is_empty() {
        let index = remaining.iter().position(|(target, _)| {
            remaining.iter().all(|(other_target, source)| {
                other_target == target || !expression_references_name(source, target)
            })
        })?;
        let (target, source) = remaining.remove(index);
        ordered.push((target.as_str(), source.as_str()));
    }
    Some(ordered)
}

fn merge_conditional_assignments<'a>(
    then_output: &'a str,
    else_output: &'a str,
) -> Option<(&'a str, &'a str, &'a str)> {
    let (then_target, then_value) = parse_single_assignment(then_output)?;
    let (else_target, else_value) = parse_single_assignment(else_output)?;
    (then_target == else_target).then_some((then_target, then_value, else_value))
}

fn for_update_clause(output: &str) -> Option<String> {
    let clause = output.strip_suffix(';')?;
    (!clause.contains(';') && parse_single_assignment(output).is_some()).then(|| clause.to_string())
}

fn parse_single_assignment(output: &str) -> Option<(&str, &str)> {
    let statement = output.strip_suffix(';')?;
    let assignment = statement.find('=')?;
    let target = &statement[..assignment];
    let value = &statement[assignment + 1..];
    (!target.is_empty()
        && target.bytes().all(is_js_identifier_byte)
        && !value.is_empty()
        && !value.contains(';'))
    .then_some((target, value))
}

fn expression_references_name(expression: &str, name: &str) -> bool {
    expression.match_indices(name).any(|(start, _)| {
        let before = start
            .checked_sub(1)
            .and_then(|index| expression.as_bytes().get(index))
            .copied();
        let after = expression.as_bytes().get(start + name.len()).copied();
        before.is_none_or(|byte| !is_js_identifier_byte(byte))
            && after.is_none_or(|byte| !is_js_identifier_byte(byte))
    })
}

fn reserve_expression_identifiers(mangler: &mut Mangler, expression: &str) {
    let bytes = expression.as_bytes();
    let mut start = 0;
    while start < bytes.len() {
        if !is_js_identifier_start(bytes[start]) {
            start += 1;
            continue;
        }
        let mut end = start + 1;
        while end < bytes.len() && is_js_identifier_byte(bytes[end]) {
            end += 1;
        }
        mangler.reserve(&expression[start..end]);
        start = end;
    }
}

const fn is_js_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_' || byte == b'$'
}

const fn is_js_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

struct LocalNames {
    value_names: AHashMap<ValueId, String>,
    parameter_values: AHashSet<ValueId>,
    stored_values: AHashSet<ValueId>,
    untyped_values: AHashSet<ValueId>,
    inlined_values: AHashMap<ValueId, String>,
    declared_names: RefCell<AHashSet<String>>,
    inline_declarations: bool,
    state: String,
    function_name: String,
    function_span: crate::span::Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PathEnd {
    ReachedStop,
    Terminated,
}

#[derive(Debug, Clone, Copy)]
struct LoopContext {
    header: BlockId,
    continue_target: BlockId,
    update: Option<BlockId>,
    exit: BlockId,
}

fn can_structure(function: &ControlFlowFunction<'_>) -> bool {
    let shaped_headers = function
        .shapes
        .iter()
        .map(ControlShape::header)
        .collect::<AHashSet<_>>();
    function.blocks.iter().all(|block| {
        !matches!(block.terminator, Some(Terminator::Branch { .. }))
            || shaped_headers.contains(&block.id)
    })
}

fn can_inline_closure(function: &ControlFlowFunction<'_>) -> bool {
    function.kind == FunctionKind::Closure
        && function.blocks.len() == 1
        && function.blocks[0].phis.is_empty()
        && matches!(function.blocks[0].terminator, Some(Terminator::Return(_)))
        && function.blocks[0].instructions.len() <= 8
}

fn render_arrow_parameters(
    function: &ControlFlowFunction<'_>,
    context: &LocalNames,
) -> Result<String, CodegenError> {
    let params = &function.params[function.capture_count..];
    if let [param] = params {
        return Ok(context.value_name(param.value)?.to_string());
    }
    let mut rendered = String::from("(");
    for (index, param) in params.iter().enumerate() {
        if index != 0 {
            rendered.push(',');
        }
        rendered.push_str(context.value_name(param.value)?);
    }
    rendered.push(')');
    Ok(rendered)
}

fn shape_at(function: &ControlFlowFunction<'_>, block: BlockId) -> Option<ControlShape> {
    if !matches!(
        function.blocks[block.0 as usize].terminator,
        Some(Terminator::Branch { .. })
    ) {
        return None;
    }
    function
        .shapes
        .iter()
        .find(|shape| shape.header() == block)
        .cloned()
}

impl LocalNames {
    fn new(
        function: &ControlFlowFunction<'_>,
        all_values: bool,
        parent: &Mangler,
        mangle_identifiers: bool,
    ) -> Self {
        let mut mangler = parent.clone();
        let mut value_names = AHashMap::new();
        let parameter_values = function
            .params
            .iter()
            .map(|param| param.value)
            .collect::<AHashSet<_>>();
        let mut stored_values = AHashSet::new();
        let untyped_values = function
            .value_escapes
            .iter()
            .enumerate()
            .filter_map(|(index, escape)| {
                (*escape == EscapeState::EscapesToUntypedBoundary).then_some(ValueId(index as u32))
            })
            .collect();
        let uses = use_counts(function);
        let unstable_values = unstable_values(function);
        let captured_values = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match &instruction.op {
                ControlFlowOp::Closure { captures, .. } => Some(captures),
                _ => None,
            })
            .flatten()
            .copied()
            .collect::<AHashSet<_>>();
        let inlined_values = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match (instruction.out, &instruction.op) {
                (
                    Some(out),
                    ControlFlowOp::Const(
                        value @ (ConstValue::Int(_) | ConstValue::Float(_) | ConstValue::Bool(_)),
                    ),
                ) => {
                    let rendered = render_const(value);
                    let use_count = uses.get(&out).copied().unwrap_or(0);
                    let inline_cost = rendered.len() * use_count;
                    let binding_cost = rendered.len() + 7 + use_count;
                    (inline_cost <= binding_cost).then_some((out, rendered))
                }
                _ => None,
            })
            .collect::<AHashMap<_, _>>();
        let cross_block = if all_values {
            cross_block_values(function)
        } else {
            AHashSet::new()
        };
        let mut values = function
            .params
            .iter()
            .map(|param| param.value)
            .collect::<Vec<_>>();
        for block in &function.blocks {
            for phi in &block.phis {
                values.push(phi.out);
                stored_values.insert(phi.out);
            }
            for (index, instruction) in block.instructions.iter().enumerate() {
                if let Some(value) = instruction.out {
                    values.push(value);
                    let use_count = uses.get(&value).copied().unwrap_or(0);
                    let fused = use_count == 1 && can_fuse_value(block, index, value);
                    if (cross_block.contains(&value)
                        || use_count > 1
                        || (captured_values.contains(&value)
                            && !matches!(instruction.op, ControlFlowOp::Const(_)))
                        || (use_count != 0 && unstable_values.contains(&value) && !fused)
                        || matches!(
                            instruction.op,
                            ControlFlowOp::NewClass {
                                constructor: Some(_),
                                ..
                            }
                        ))
                        && !inlined_values.contains_key(&value)
                    {
                        stored_values.insert(value);
                    }
                }
            }
        }
        values.sort_unstable_by_key(|value| value.0);
        values.dedup();
        values.sort_unstable_by(|left, right| {
            let left_emitted = parameter_values.contains(left) || stored_values.contains(left);
            let right_emitted = parameter_values.contains(right) || stored_values.contains(right);
            right_emitted
                .cmp(&left_emitted)
                .then_with(|| {
                    uses.get(right)
                        .copied()
                        .unwrap_or(0)
                        .cmp(&uses.get(left).copied().unwrap_or(0))
                })
                .then_with(|| left.0.cmp(&right.0))
        });
        let state = if all_values {
            if mangle_identifiers {
                mangler.next_name()
            } else {
                mangler.unique_name("$state")
            }
        } else {
            String::new()
        };
        if mangle_identifiers && function.blocks.len() > 1 {
            let colors = coalesce_value_names(function, &stored_values, &parameter_values, &uses);
            let color_count = colors.values().copied().max().map_or(0, |color| color + 1);
            let color_names = (0..color_count)
                .map(|_| mangler.next_name())
                .collect::<Vec<_>>();
            for value in &values {
                if let Some(color) = colors.get(value) {
                    value_names.insert(*value, color_names[*color].clone());
                }
            }
        }
        for value in values {
            value_names.entry(value).or_insert_with(|| {
                if mangle_identifiers {
                    mangler.next_name()
                } else {
                    let preferred = function
                        .params
                        .iter()
                        .find(|parameter| parameter.value == value)
                        .map_or_else(
                            || format!("v{}", value.0),
                            |parameter| parameter.name.into(),
                        );
                    mangler.unique_name(&preferred)
                }
            });
        }
        let declared_names = function
            .params
            .iter()
            .filter_map(|parameter| value_names.get(&parameter.value))
            .cloned()
            .collect();
        Self {
            value_names,
            parameter_values,
            stored_values,
            untyped_values,
            inlined_values,
            declared_names: RefCell::new(declared_names),
            inline_declarations: false,
            state,
            function_name: function
                .name
                .unwrap_or(if function.kind == FunctionKind::Entry {
                    "<entry>"
                } else {
                    "<closure>"
                })
                .to_string(),
            function_span: function.span,
        }
    }

    fn value_name(&self, value: ValueId) -> Result<&str, CodegenError> {
        self.value_names
            .get(&value)
            .map(String::as_str)
            .ok_or_else(|| {
                CodegenError::new(
                    self.function_span,
                    format!(
                        "SSA value {} has no emitted name in function `{}`",
                        value.0, self.function_name
                    ),
                )
            })
    }

    fn state_name(&self) -> &str {
        &self.state
    }

    fn is_untyped(&self, value: ValueId) -> bool {
        self.untyped_values.contains(&value)
    }

    fn is_stored(&self, value: ValueId) -> bool {
        self.stored_values.contains(&value)
    }

    fn claim_declaration(&self, value: ValueId) -> Result<bool, CodegenError> {
        if !self.inline_declarations {
            return Ok(false);
        }
        let name = self.value_name(value)?.to_string();
        Ok(self.declared_names.borrow_mut().insert(name))
    }

    fn claim_remaining_declarations(&self) -> Vec<String> {
        if !self.inline_declarations {
            return Vec::new();
        }
        let mut values = self.stored_values.iter().copied().collect::<Vec<_>>();
        values.sort_unstable_by_key(|value| value.0);
        let mut declared = self.declared_names.borrow_mut();
        values
            .into_iter()
            .filter_map(|value| self.value_names.get(&value))
            .filter(|name| declared.insert((*name).clone()))
            .cloned()
            .collect()
    }

    fn non_parameter_names(&self, function: &ControlFlowFunction<'_>) -> Vec<&str> {
        let parameter_names = function
            .params
            .iter()
            .filter_map(|parameter| self.value_names.get(&parameter.value))
            .cloned()
            .collect::<AHashSet<_>>();
        let mut values = function
            .blocks
            .iter()
            .flat_map(|block| {
                block.phis.iter().map(|phi| phi.out).chain(
                    block
                        .instructions
                        .iter()
                        .filter_map(|instruction| instruction.out),
                )
            })
            .filter(|value| !self.parameter_values.contains(value))
            .filter(|value| self.stored_values.contains(value))
            .collect::<Vec<_>>();
        values.sort_by_key(|value| value.0);
        values.dedup();
        let mut seen = AHashSet::new();
        values
            .into_iter()
            .filter_map(|value| self.value_names.get(&value).map(String::as_str))
            .filter(|name| !parameter_names.contains(*name))
            .filter(|name| seen.insert((*name).to_string()))
            .collect()
    }
}

fn unstable_values(function: &ControlFlowFunction<'_>) -> AHashSet<ValueId> {
    let mut unstable = AHashSet::new();
    loop {
        let mut changed = false;
        for block in &function.blocks {
            for phi in &block.phis {
                if phi
                    .incoming
                    .iter()
                    .any(|(_, value)| unstable.contains(value))
                {
                    changed |= unstable.insert(phi.out);
                }
            }
            for instruction in &block.instructions {
                let Some(out) = instruction.out else {
                    continue;
                };
                if !op_can_defer(&instruction.op)
                    || op_values(&instruction.op)
                        .iter()
                        .any(|value| unstable.contains(value))
                {
                    changed |= unstable.insert(out);
                }
            }
        }
        if !changed {
            return unstable;
        }
    }
}

fn emit_binding_prefix(
    context: &LocalNames,
    value: ValueId,
    predeclared: bool,
    out: &mut String,
) -> Result<(), CodegenError> {
    if !predeclared {
        out.push_str("let ");
    } else if context.claim_declaration(value)? {
        out.push_str("var ");
    }
    Ok(())
}

fn coalesce_value_names(
    function: &ControlFlowFunction<'_>,
    stored_values: &AHashSet<ValueId>,
    parameter_values: &AHashSet<ValueId>,
    uses: &AHashMap<ValueId, usize>,
) -> AHashMap<ValueId, usize> {
    let named = stored_values
        .union(parameter_values)
        .copied()
        .collect::<AHashSet<_>>();
    let block_count = function.blocks.len();
    let mut definitions = vec![AHashSet::<ValueId>::new(); block_count];
    let mut local_uses = vec![AHashSet::<ValueId>::new(); block_count];
    let mut phi_definitions = vec![AHashSet::<ValueId>::new(); block_count];

    for block in &function.blocks {
        let index = block.id.0 as usize;
        for phi in &block.phis {
            if named.contains(&phi.out) {
                definitions[index].insert(phi.out);
                phi_definitions[index].insert(phi.out);
            }
        }
        for instruction in &block.instructions {
            for value in op_values(&instruction.op) {
                if named.contains(&value) && !definitions[index].contains(&value) {
                    local_uses[index].insert(value);
                }
            }
            if let Some(out) = instruction.out.filter(|out| named.contains(out)) {
                definitions[index].insert(out);
            }
        }
        for value in block
            .terminator
            .as_ref()
            .into_iter()
            .flat_map(terminator_values)
        {
            if named.contains(&value) && !definitions[index].contains(&value) {
                local_uses[index].insert(value);
            }
        }
    }

    let mut live_in = vec![AHashSet::<ValueId>::new(); block_count];
    let mut live_out = vec![AHashSet::<ValueId>::new(); block_count];
    loop {
        let mut changed = false;
        for block in function.blocks.iter().rev() {
            let index = block.id.0 as usize;
            let mut out = AHashSet::new();
            for successor in block_successors(block) {
                let successor_index = successor.0 as usize;
                out.extend(
                    live_in[successor_index]
                        .difference(&phi_definitions[successor_index])
                        .copied(),
                );
                for phi in &function.blocks[successor_index].phis {
                    if let Some((_, value)) = phi
                        .incoming
                        .iter()
                        .find(|(predecessor, _)| predecessor == &block.id)
                    {
                        if named.contains(value) {
                            out.insert(*value);
                        }
                    }
                }
            }
            let mut input = local_uses[index].clone();
            input.extend(out.difference(&definitions[index]).copied());
            if out != live_out[index] {
                live_out[index] = out;
                changed = true;
            }
            if input != live_in[index] {
                live_in[index] = input;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let mut interference = named
        .iter()
        .map(|value| (*value, AHashSet::<ValueId>::new()))
        .collect::<AHashMap<_, _>>();
    let mut connect = |left: ValueId, right: ValueId| {
        if left != right {
            interference.entry(left).or_default().insert(right);
            interference.entry(right).or_default().insert(left);
        }
    };
    for block in &function.blocks {
        let index = block.id.0 as usize;
        let mut live = live_out[index].clone();
        for value in block
            .terminator
            .as_ref()
            .into_iter()
            .flat_map(terminator_values)
        {
            if named.contains(&value) {
                live.insert(value);
            }
        }
        for instruction in block.instructions.iter().rev() {
            let operands = op_values(&instruction.op)
                .into_iter()
                .filter(|value| named.contains(value))
                .collect::<Vec<_>>();
            if let Some(out) = instruction.out.filter(|out| named.contains(out)) {
                if matches!(
                    instruction.op,
                    ControlFlowOp::NewClass {
                        constructor: Some(_),
                        ..
                    }
                ) {
                    for operand in &operands {
                        connect(out, *operand);
                    }
                }
                for value in &live {
                    connect(out, *value);
                }
                live.remove(&out);
            }
            for value in operands {
                live.insert(value);
            }
        }
        let phi_values = block
            .phis
            .iter()
            .map(|phi| phi.out)
            .filter(|value| named.contains(value))
            .collect::<Vec<_>>();
        for (position, value) in phi_values.iter().enumerate() {
            for live_value in &live {
                connect(*value, *live_value);
            }
            for other in &phi_values[position + 1..] {
                connect(*value, *other);
            }
            live.remove(value);
        }
    }

    let parameters = function
        .params
        .iter()
        .map(|parameter| parameter.value)
        .filter(|value| named.contains(value))
        .collect::<Vec<_>>();
    for (position, parameter) in parameters.iter().enumerate() {
        for other in &parameters[position + 1..] {
            connect(*parameter, *other);
        }
    }

    let captured = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match &instruction.op {
            ControlFlowOp::Closure { captures, .. } => Some(captures),
            _ => None,
        })
        .flatten()
        .filter(|value| named.contains(value))
        .copied()
        .collect::<AHashSet<_>>();
    for capture in captured {
        for value in &named {
            connect(capture, *value);
        }
    }

    let mut values = named.into_iter().collect::<Vec<_>>();
    let parameter_order = function
        .params
        .iter()
        .enumerate()
        .map(|(index, parameter)| (parameter.value, index))
        .collect::<AHashMap<_, _>>();
    let mut affinities = AHashMap::<ValueId, Vec<ValueId>>::new();
    for block in &function.blocks {
        for phi in &block.phis {
            if !interference.contains_key(&phi.out) {
                continue;
            }
            for (_, incoming) in &phi.incoming {
                if interference.contains_key(incoming) && !interference[&phi.out].contains(incoming)
                {
                    affinities.entry(phi.out).or_default().push(*incoming);
                    affinities.entry(*incoming).or_default().push(phi.out);
                }
            }
        }
    }
    values.sort_unstable_by(|left, right| {
        parameter_order
            .contains_key(right)
            .cmp(&parameter_order.contains_key(left))
            .then_with(|| {
                parameter_order
                    .get(left)
                    .unwrap_or(&usize::MAX)
                    .cmp(parameter_order.get(right).unwrap_or(&usize::MAX))
            })
            .then_with(|| {
                uses.get(right)
                    .copied()
                    .unwrap_or(0)
                    .cmp(&uses.get(left).copied().unwrap_or(0))
            })
            .then_with(|| {
                interference
                    .get(right)
                    .map_or(0, |values| values.len())
                    .cmp(&interference.get(left).map_or(0, |values| values.len()))
            })
            .then_with(|| left.0.cmp(&right.0))
    });
    let mut colors = AHashMap::<ValueId, usize>::new();
    for value in values {
        let unavailable = interference
            .get(&value)
            .into_iter()
            .flatten()
            .filter_map(|neighbor| colors.get(neighbor).copied())
            .collect::<AHashSet<_>>();
        let preferred = affinities
            .get(&value)
            .into_iter()
            .flatten()
            .filter_map(|affinity| colors.get(affinity).copied())
            .filter(|color| !unavailable.contains(color))
            .min();
        let color =
            preferred.unwrap_or_else(|| (0..).find(|color| !unavailable.contains(color)).unwrap());
        colors.insert(value, color);
    }
    colors
}

fn block_successors(block: &crate::ir::ControlFlowBlock<'_>) -> Vec<BlockId> {
    match block.terminator {
        Some(Terminator::Jump(target)) => vec![target],
        Some(Terminator::Branch {
            then_block,
            else_block,
            ..
        }) => vec![then_block, else_block],
        _ => Vec::new(),
    }
}

fn terminator_values(terminator: &Terminator) -> Vec<ValueId> {
    match terminator {
        Terminator::Branch { condition, .. } => vec![*condition],
        Terminator::Return(Some(value)) => vec![*value],
        _ => Vec::new(),
    }
}

fn take_value(
    value: ValueId,
    context: &LocalNames,
    cache: &mut AHashMap<ValueId, String>,
) -> Result<String, CodegenError> {
    Ok(context
        .inlined_values
        .get(&value)
        .cloned()
        .or_else(|| cache.remove(&value))
        .unwrap_or(context.value_name(value)?.to_string()))
}

fn use_counts(function: &ControlFlowFunction<'_>) -> AHashMap<ValueId, usize> {
    let mut counts = AHashMap::new();
    let mut add = |value| *counts.entry(value).or_insert(0) += 1;
    for block in &function.blocks {
        for phi in &block.phis {
            for (_, value) in &phi.incoming {
                add(*value);
            }
        }
        for instruction in &block.instructions {
            for value in op_values(&instruction.op) {
                add(value);
            }
        }
        match block.terminator.as_ref() {
            Some(Terminator::Branch { condition, .. }) => add(*condition),
            Some(Terminator::Return(Some(value))) => add(*value),
            _ => {}
        }
    }
    counts
}

fn can_fuse_value(
    block: &crate::ir::ControlFlowBlock<'_>,
    definition_index: usize,
    value: ValueId,
) -> bool {
    for instruction in &block.instructions[definition_index + 1..] {
        if op_values(&instruction.op).contains(&value) {
            return true;
        }
        if !op_can_defer(&instruction.op) {
            return false;
        }
    }
    false
}

fn cross_block_values(function: &ControlFlowFunction<'_>) -> AHashSet<ValueId> {
    let mut definitions = AHashMap::new();
    for block in &function.blocks {
        for phi in &block.phis {
            definitions.insert(phi.out, block.id);
        }
        for instruction in &block.instructions {
            if let Some(value) = instruction.out {
                definitions.insert(value, block.id);
            }
        }
    }

    let mut crossing = AHashSet::new();
    let mut record_use = |value: ValueId, block: BlockId| {
        if definitions
            .get(&value)
            .is_some_and(|definition| *definition != block)
        {
            crossing.insert(value);
        }
    };
    for block in &function.blocks {
        for phi in &block.phis {
            for (incoming, value) in &phi.incoming {
                record_use(*value, *incoming);
            }
        }
        for instruction in &block.instructions {
            for value in op_values(&instruction.op) {
                record_use(value, block.id);
            }
        }
        match block.terminator.as_ref() {
            Some(Terminator::Branch { condition, .. }) => record_use(*condition, block.id),
            Some(Terminator::Return(Some(value))) => record_use(*value, block.id),
            _ => {}
        }
    }
    crossing
}

fn op_values(op: &ControlFlowOp<'_>) -> Vec<ValueId> {
    match op {
        ControlFlowOp::Const(_) | ControlFlowOp::LoadLocal(_) | ControlFlowOp::LoadGlobal(_) => {
            Vec::new()
        }
        ControlFlowOp::Unary { value, .. } => vec![*value],
        ControlFlowOp::Binary { lhs, rhs, .. } => vec![*lhs, *rhs],
        ControlFlowOp::Array(values) | ControlFlowOp::Struct { fields: values, .. } => {
            values.clone()
        }
        ControlFlowOp::NewClass { args, .. } => args.clone(),
        ControlFlowOp::Closure { captures, .. } => captures.clone(),
        ControlFlowOp::StoreLocal { value, .. } | ControlFlowOp::StoreGlobal { value, .. } => {
            vec![*value]
        }
        ControlFlowOp::FieldGet { object, .. } => vec![*object],
        ControlFlowOp::FieldSet { object, value, .. } => vec![*object, *value],
        ControlFlowOp::IndexGet { object, index } => vec![*object, *index],
        ControlFlowOp::IndexSet {
            object,
            index,
            value,
        } => vec![*object, *index, *value],
        ControlFlowOp::CallDirect { args, .. } => args.clone(),
        ControlFlowOp::CallValue { callee, args } => {
            let mut values = vec![*callee];
            values.extend(args);
            values
        }
        ControlFlowOp::CallMethod { receiver, args, .. } => {
            let mut values = vec![*receiver];
            values.extend(args);
            values
        }
        ControlFlowOp::Intrinsic { receiver, args, .. } => {
            let mut values = receiver.iter().copied().collect::<Vec<_>>();
            values.extend(args);
            values
        }
        ControlFlowOp::Template(parts) => parts
            .iter()
            .filter_map(|part| match part {
                TemplateOperand::Value(value) => Some(*value),
                TemplateOperand::String(_) => None,
            })
            .collect(),
    }
}

fn op_can_defer(op: &ControlFlowOp<'_>) -> bool {
    matches!(
        op,
        ControlFlowOp::Const(_)
            | ControlFlowOp::Unary { .. }
            | ControlFlowOp::Binary { .. }
            | ControlFlowOp::Array(_)
            | ControlFlowOp::Struct { .. }
            | ControlFlowOp::Closure { .. }
            | ControlFlowOp::Template(_)
            | ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::StringLength,
                ..
            }
    )
}

fn expression_only_op(op: &ControlFlowOp<'_>) -> bool {
    !matches!(
        op,
        ControlFlowOp::StoreLocal { .. }
            | ControlFlowOp::StoreGlobal { .. }
            | ControlFlowOp::FieldSet { .. }
            | ControlFlowOp::IndexSet { .. }
            | ControlFlowOp::NewClass {
                constructor: Some(_),
                ..
            }
            | ControlFlowOp::CallDirect { .. }
            | ControlFlowOp::CallValue { .. }
            | ControlFlowOp::CallMethod { .. }
            | ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::Print
                    | Intrinsic::ArrayMap
                    | Intrinsic::ArrayFilter
                    | Intrinsic::ArrayReduce
                    | Intrinsic::ArrayForEach
                    | Intrinsic::ArrayPush
                    | Intrinsic::ArrayPop,
                ..
            }
    )
}

fn op_has_side_effects(op: &ControlFlowOp<'_>) -> bool {
    matches!(
        op,
        ControlFlowOp::StoreLocal { .. }
            | ControlFlowOp::StoreGlobal { .. }
            | ControlFlowOp::FieldSet { .. }
            | ControlFlowOp::IndexSet { .. }
            | ControlFlowOp::CallDirect { .. }
            | ControlFlowOp::CallValue { .. }
            | ControlFlowOp::CallMethod { .. }
            | ControlFlowOp::NewClass { .. }
            | ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::Print
                    | Intrinsic::ArrayMap
                    | Intrinsic::ArrayFilter
                    | Intrinsic::ArrayReduce
                    | Intrinsic::ArrayForEach
                    | Intrinsic::ArrayPush
                    | Intrinsic::ArrayPop,
                ..
            }
    )
}

fn render_const(value: &ConstValue) -> String {
    match value {
        ConstValue::Int(value) => value.to_string(),
        ConstValue::Float(value) if value.fract() == 0.0 => (*value as i64).to_string(),
        ConstValue::Float(value) => value.to_string(),
        ConstValue::Bool(value) => value.to_string(),
        ConstValue::String(value) => render_string_literal(value),
        ConstValue::Null => "null".to_string(),
    }
}

fn render_string_literal(value: &str) -> String {
    format!("\"{value}\"")
}

fn binary_operator(op: IrBinaryOp) -> &'static str {
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

fn default_value(ty: &Type<'_>) -> &'static str {
    match ty {
        Type::Int | Type::Float => "0",
        Type::Bool => "false",
        Type::String => "\"\"",
        Type::Array(_) => "[]",
        Type::Null | Type::Nullable(_) => "null",
        Type::Struct(_)
        | Type::Class(_)
        | Type::StructInstance { .. }
        | Type::ClassInstance { .. }
        | Type::TypeParameter(_)
        | Type::Function(_)
        | Type::GenericFunction(_) => "null",
        Type::Void => "void 0",
    }
}

fn strip_outer_parens(value: String) -> String {
    if !value.starts_with('(') || !value.ends_with(')') {
        return value;
    }
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in value.char_indices() {
        if let Some(active) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == active {
                quote = None;
            }
            continue;
        }
        if matches!(character, '\'' | '"' | '`') {
            quote = Some(character);
            continue;
        }
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 && index + character.len_utf8() != value.len() {
                    return value;
                }
            }
            _ => {}
        }
    }
    value[1..value.len() - 1].to_string()
}

fn is_single_binding_statement(statement: &str) -> bool {
    statement.starts_with("let ")
        && statement.ends_with(';')
        && !statement[..statement.len() - 1].contains(';')
}

fn negate_condition(value: String) -> String {
    let value = strip_outer_parens(value);
    for (operator, inverse) in [
        ("!=", "=="),
        ("==", "!="),
        ("<=", ">"),
        (">=", "<"),
        ("<", ">="),
        (">", "<="),
    ] {
        if let Some(index) = value.find(operator) {
            let mut negated = value.clone();
            negated.replace_range(index..index + operator.len(), inverse);
            return negated;
        }
    }
    if let Some(value) = value.strip_prefix('!') {
        return strip_outer_parens(value.to_string());
    }
    format!("!{value}")
}

#[derive(Debug, Default, Clone)]
struct Mangler {
    next: usize,
    reserved: AHashSet<String>,
}

impl Mangler {
    fn reserve(&mut self, name: &str) {
        self.reserved.insert(name.to_string());
    }

    fn next_name(&mut self) -> String {
        loop {
            let name = encode_identifier(self.next);
            self.next += 1;
            if !self.reserved.contains(&name) && !is_js_reserved(&name) {
                self.reserved.insert(name.clone());
                return name;
            }
        }
    }

    fn unique_name(&mut self, preferred: &str) -> String {
        let base = if is_js_reserved(preferred) {
            format!("${preferred}")
        } else {
            preferred.to_string()
        };
        if self.reserved.insert(base.clone()) {
            return base;
        }
        let mut suffix = 2;
        loop {
            let candidate = format!("{base}${suffix}");
            if self.reserved.insert(candidate.clone()) {
                return candidate;
            }
            suffix += 1;
        }
    }
}

fn is_js_reserved(name: &str) -> bool {
    matches!(
        name,
        "await"
            | "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "implements"
            | "import"
            | "in"
            | "instanceof"
            | "interface"
            | "let"
            | "new"
            | "null"
            | "package"
            | "private"
            | "protected"
            | "public"
            | "return"
            | "static"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
            | "yield"
    )
}

fn encode_identifier(mut index: usize) -> String {
    const FIRST: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_$";
    const REST: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_$0123456789";
    let mut output = String::new();
    output.push(FIRST[index % FIRST.len()] as char);
    index /= FIRST.len();
    while index > 0 {
        index -= 1;
        output.push(REST[index % REST.len()] as char);
        index /= REST.len();
    }
    output
}

#[cfg(test)]
mod tests {
    use bumpalo::Bump;

    use super::*;
    use crate::{analyze, lower_to_control_flow, optimizer::optimize_control_flow, parse_source};

    fn compile(source: &str) -> String {
        let arena = Bump::new();
        let program = parse_source(&arena, source).unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut ir).unwrap();
        emit_optimized_ir_js(&ir).unwrap()
    }

    #[test]
    fn emits_compact_straight_line_ir() {
        assert_eq!(compile("print(1+2*3);"), "console.log(7)");
    }

    #[test]
    fn constructor_results_do_not_coalesce_with_constructor_arguments() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "class Task{func()->void callback;bool marker;init(func()->void callback,bool marker){this.callback=callback;this.marker=marker;}}int install(func()->void callback,bool marker){Task task=new Task(callback,marker);return 1;}print(install(()=>{print(1);},true));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        crate::optimizer::promote_locals_to_ssa(&mut ir).unwrap();
        let function = ir
            .functions
            .iter()
            .find(|function| function.name == Some("install"))
            .unwrap();
        let callback = function.params[0].value;
        let object = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find_map(|instruction| {
                matches!(instruction.op, ControlFlowOp::NewClass { .. })
                    .then_some(instruction.out)
                    .flatten()
            })
            .unwrap();
        let named = function
            .params
            .iter()
            .map(|parameter| parameter.value)
            .chain(
                function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .filter_map(|instruction| instruction.out),
            )
            .collect::<AHashSet<_>>();
        let parameters = function
            .params
            .iter()
            .map(|parameter| parameter.value)
            .collect::<AHashSet<_>>();
        let colors = coalesce_value_names(function, &named, &parameters, &use_counts(function));

        assert_ne!(colors[&callback], colors[&object]);
    }

    #[test]
    fn captured_values_keep_a_dedicated_color() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern void retain(func()->int callback);void install(int value){func()->int callback=()=>value;retain(callback);int later=value+1;print(later);}install(1);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        crate::optimizer::promote_locals_to_ssa(&mut ir).unwrap();
        let function = ir
            .functions
            .iter()
            .find(|function| function.name == Some("install"))
            .unwrap();
        let captured = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find_map(|instruction| match &instruction.op {
                ControlFlowOp::Closure { captures, .. } => captures.first().copied(),
                _ => None,
            })
            .unwrap();
        let named = function
            .params
            .iter()
            .map(|parameter| parameter.value)
            .chain(
                function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .filter_map(|instruction| instruction.out),
            )
            .collect::<AHashSet<_>>();
        let parameters = function
            .params
            .iter()
            .map(|parameter| parameter.value)
            .collect::<AHashSet<_>>();
        let colors = coalesce_value_names(function, &named, &parameters, &use_counts(function));

        for (value, color) in &colors {
            if *value != captured {
                assert_ne!(colors[&captured], *color);
            }
        }
    }

    #[test]
    fn closure_wrappers_reserve_capture_expression_identifiers() {
        let mut mangler = Mangler::default();
        reserve_expression_identifiers(&mut mangler, "a[0]+b.c+d");

        assert_eq!(mangler.next_name(), "e");
    }

    #[test]
    fn emits_structs_from_ir() {
        assert_eq!(
            compile("struct Point{int x;int y;}Point p=Point{10,20};print(p.x);"),
            "console.log(10)"
        );
    }

    #[test]
    fn emits_invoked_capturing_closures() {
        assert_eq!(
            compile(
                "int apply(int factor){auto callback=(int value)=>value*factor;return callback(4);}print(apply(3));"
            ),
            "console.log((b=>Math.imul(b,3))(4))"
        );
    }

    #[test]
    fn folds_signed_i32_overflow() {
        assert_eq!(compile("print(2147483647+1);"), "console.log(-2147483648)");
    }

    #[test]
    fn preserves_extern_names() {
        assert_eq!(
            compile("extern int hostAdd(int a,int b);int result=hostAdd(1,2);"),
            "hostAdd(1,2)"
        );
    }

    #[test]
    fn propagates_shared_constants_through_deep_inlining() {
        assert_eq!(
            compile(
                "int factor=3;int add(int value){return value+factor;}int twice(int value){return add(add(value));}print(twice(4));"
            ),
            "console.log(10)"
        );
    }

    #[test]
    fn eliminates_algebraic_identities_and_pure_calls() {
        assert_eq!(
            compile(
                "extern int read();int square(int value){return value*value;}int value=read();square(9);print((value+0)*1);"
            ),
            "console.log(read())"
        );
    }

    #[test]
    fn eliminates_unused_calls_to_declared_pure_externs() {
        assert_eq!(
            compile("pure extern int stableHash(int value);stableHash(7);print(2);"),
            "console.log(2)"
        );
    }

    #[test]
    fn orders_acyclic_phi_copies_and_preserves_cycles() {
        let assignments = vec![
            ("b".to_string(), "(b+Math.imul(a,2)|0)".to_string()),
            ("a".to_string(), "(a+1|0)".to_string()),
        ];
        assert_eq!(
            order_scalar_assignments(&assignments).unwrap(),
            vec![("b", "(b+Math.imul(a,2)|0)"), ("a", "(a+1|0)")]
        );

        let swap = vec![
            ("a".to_string(), "b".to_string()),
            ("b".to_string(), "a".to_string()),
        ];
        assert!(order_scalar_assignments(&swap).is_none());
    }

    #[test]
    fn merges_matching_branch_assignments() {
        assert_eq!(
            merge_conditional_assignments("a=(a+1|0);", "a=(a-1|0);"),
            Some(("a", "(a+1|0)", "(a-1|0)"))
        );
        assert!(merge_conditional_assignments("a=1;", "b=2;").is_none());
    }

    #[test]
    fn hoists_loop_locals_into_the_first_var_group() {
        let output = compile(
            "int total=0;for(int outer=0;outer<12;outer++){if(outer%3==0){continue;}int inner=0;while(inner<4){total+=inner;inner++;}}print(total);",
        );
        assert!(output.starts_with("var "), "{output}");
        assert!(output.contains(";for("), "{output}");
        assert_eq!(output.matches("for(").count(), 2, "{output}");
        assert!(!output.contains("while("), "{output}");
        assert_eq!(output.matches("var ").count(), 1, "{output}");
    }

    #[test]
    fn aliases_repeated_long_strings_when_it_reduces_size() {
        let output = compile(
            "extern void sink(string value);sink(\"a-repeated-application-string\");sink(\"a-repeated-application-string\");sink(\"a-repeated-application-string\");",
        );
        assert!(output.starts_with("let a=\"a-repeated-application-string\";"));
        assert_eq!(output.matches("a-repeated-application-string").count(), 1);
        assert_eq!(output.matches("sink(a)").count(), 3);
    }

    #[test]
    fn materializes_named_structs_at_extern_boundaries() {
        assert_eq!(
            compile(
                "struct Point{int x;int y;}extern void consume(Point p);Point p=Point{1,2};consume(p);"
            ),
            "consume({x:1,y:2})"
        );
    }

    #[test]
    fn inlines_closures_that_read_typed_globals() {
        let output = compile(
            "int factor=2;int[] values=[1,2];auto mapped=values.map((int value)=>value*factor);print(mapped.length);",
        );
        assert!(output.contains(".map("));
        assert!(output.contains("Math.imul"));
    }
}
