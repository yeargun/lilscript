use std::cell::RefCell;
use std::fmt::Write;
use std::path::Path;
use std::sync::Arc;

use ahash::{AHashMap, AHashSet};

use crate::codegen_js::CodegenError;
use crate::ir::{
    BlockId, ConstValue, ControlFlowFunction, ControlFlowInstruction, ControlFlowModule,
    ControlFlowOp, ControlShape, ExportBinding, FunctionId, FunctionKind, Intrinsic, IrBinaryOp,
    IrUnaryOp, TemplateOperand, Terminator, ValueId,
};
use crate::semantic::{EscapeState, SymbolId, Type};
use crate::value_analysis::{analyze_integer_values, FunctionIntegerFacts, IntegerValueAnalysis};

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
    pub elide_safe_integer_coercions: bool,
    pub compact_boolean_literals: bool,
    pub inline_structured_closures: bool,
    pub pack_string_arrays: bool,
    pub scalar_phi_copies: bool,
    pub phi_affinity_mode: PhiAffinityMode,
    pub loop_spelling: LoopSpelling,
    pub mutation_spelling: MutationSpelling,
    pub identifier_alphabet: IdentifierAlphabet,
    pub string_quote: StringQuote,
}

impl Default for IrJsOptions {
    fn default() -> Self {
        Self {
            mangle_identifiers: true,
            mangle_properties: false,
            mangle_exports: false,
            pool_strings: true,
            elide_safe_integer_coercions: true,
            compact_boolean_literals: true,
            inline_structured_closures: true,
            pack_string_arrays: true,
            scalar_phi_copies: false,
            phi_affinity_mode: PhiAffinityMode::Grouped,
            loop_spelling: LoopSpelling::Auto,
            mutation_spelling: MutationSpelling::Assignment,
            identifier_alphabet: IdentifierAlphabet::canonical(),
            string_quote: StringQuote::Double,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StringQuote {
    #[default]
    Double,
    Single,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PhiAffinityMode {
    Conservative,
    Direct,
    #[default]
    Grouped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoopSpelling {
    #[default]
    Auto,
    While,
    For,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MutationSpelling {
    #[default]
    Assignment,
    Prefix,
    Postfix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdentifierAlphabet {
    first: [u8; 54],
    rest: [u8; 64],
}

impl IdentifierAlphabet {
    pub const fn canonical() -> Self {
        Self {
            first: *b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_$",
            rest: *b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_$0123456789",
        }
    }

    pub fn for_code(code: &str) -> Self {
        let canonical = Self::canonical();
        let mut counts = [0usize; 128];
        for byte in code.bytes().filter(|byte| byte.is_ascii()) {
            counts[byte as usize] += 1;
        }
        let mut alphabet = canonical;
        alphabet.first.sort_unstable_by(|left, right| {
            counts[*right as usize]
                .cmp(&counts[*left as usize])
                .then_with(|| {
                    canonical_rank(*left, &canonical.first)
                        .cmp(&canonical_rank(*right, &canonical.first))
                })
        });
        alphabet.rest.sort_unstable_by(|left, right| {
            counts[*right as usize]
                .cmp(&counts[*left as usize])
                .then_with(|| {
                    canonical_rank(*left, &canonical.rest)
                        .cmp(&canonical_rank(*right, &canonical.rest))
                })
        });
        alphabet
    }
}

impl Default for IdentifierAlphabet {
    fn default() -> Self {
        Self::canonical()
    }
}

fn canonical_rank(byte: u8, alphabet: &[u8]) -> usize {
    alphabet
        .iter()
        .position(|candidate| *candidate == byte)
        .unwrap_or(usize::MAX)
}

pub fn emit_optimized_ir_js_with_options(
    module: &ControlFlowModule<'_>,
    options: &IrJsOptions,
) -> Result<String, CodegenError> {
    IrJsEmitter::new(module, false, *options).emit()
}

pub(crate) fn emit_optimized_ir_js_with_options_and_analysis(
    module: &ControlFlowModule<'_>,
    options: &IrJsOptions,
    integer_analysis: Arc<IntegerValueAnalysis>,
) -> Result<String, CodegenError> {
    IrJsEmitter::with_integer_analysis(module, false, *options, integer_analysis).emit()
}

pub fn emit_optimized_ir_js_module_with_options(
    module: &ControlFlowModule<'_>,
    options: &IrJsOptions,
) -> Result<String, CodegenError> {
    IrJsEmitter::new(module, true, *options).emit()
}

pub(crate) fn emit_optimized_ir_js_module_with_options_and_analysis(
    module: &ControlFlowModule<'_>,
    options: &IrJsOptions,
    integer_analysis: Arc<IntegerValueAnalysis>,
) -> Result<String, CodegenError> {
    IrJsEmitter::with_integer_analysis(module, true, *options, integer_analysis).emit()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrJsChunkSpec {
    pub file_name: String,
    pub functions: Vec<FunctionId>,
    pub lazy_module: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrJsChunkPlan {
    pub entry_file: String,
    pub chunks: Vec<IrJsChunkSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrJsChunk {
    pub file_name: String,
    pub code: String,
    pub dependencies: Vec<String>,
    pub dynamic_dependencies: Vec<String>,
}

pub fn emit_optimized_ir_js_chunks_with_options(
    module: &ControlFlowModule<'_>,
    options: &IrJsOptions,
    plan: &IrJsChunkPlan,
) -> Result<Vec<IrJsChunk>, CodegenError> {
    IrJsEmitter::new(module, true, *options).emit_chunks(plan)
}

pub fn ir_function_can_move_to_chunk(module: &ControlFlowModule<'_>, function: FunctionId) -> bool {
    module
        .functions
        .get(function.0 as usize)
        .is_some_and(|function| is_emitted_function(function, true))
        && !function_writes_global(module, function, &mut AHashSet::new(), true)
}

struct IrJsEmitter<'module, 'src> {
    module: &'module ControlFlowModule<'src>,
    integer_analysis: Arc<IntegerValueAnalysis>,
    global_names: AHashMap<SymbolId, String>,
    external_export_aliases: AHashMap<SymbolId, String>,
    function_names: AHashMap<FunctionId, String>,
    top_level_mangler: Mangler,
    declared_globals: AHashSet<SymbolId>,
    string_aliases: AHashMap<String, String>,
    pooled_strings: Vec<(String, String)>,
    property_names: AHashMap<String, String>,
    module_output: bool,
    options: IrJsOptions,
    dynamic_chunk_files: AHashMap<u32, String>,
}

impl<'module, 'src> IrJsEmitter<'module, 'src> {
    fn new(
        module: &'module ControlFlowModule<'src>,
        module_output: bool,
        options: IrJsOptions,
    ) -> Self {
        Self::with_integer_analysis(
            module,
            module_output,
            options,
            Arc::new(analyze_integer_values(module)),
        )
    }

    fn with_integer_analysis(
        module: &'module ControlFlowModule<'src>,
        module_output: bool,
        options: IrJsOptions,
        integer_analysis: Arc<IntegerValueAnalysis>,
    ) -> Self {
        Self {
            module,
            integer_analysis,
            global_names: AHashMap::new(),
            external_export_aliases: AHashMap::new(),
            function_names: AHashMap::new(),
            top_level_mangler: Mangler::new(options.identifier_alphabet),
            declared_globals: AHashSet::new(),
            string_aliases: AHashMap::new(),
            pooled_strings: Vec::new(),
            property_names: AHashMap::new(),
            module_output,
            options,
            dynamic_chunk_files: AHashMap::new(),
        }
    }

    fn emit(mut self) -> Result<String, CodegenError> {
        self.prepare();
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
                out.push_str(&render_string_literal(value, self.options.string_quote));
            }
            out.push(';');
        }

        let owned_globals = self
            .module
            .globals
            .iter()
            .filter(|global| !global.external)
            .collect::<Vec<_>>();
        if !owned_globals.is_empty() {
            out.push_str("let ");
            for (index, global) in owned_globals.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                out.push_str(self.global_name(global.symbol)?);
                self.declared_globals.insert(global.symbol);
            }
            out.push(';');
        }
        self.emit_external_export_aliases(&mut out)?;

        let functions = self.module.functions.clone();
        for function in &functions {
            if !function.live
                || function.kind == FunctionKind::Entry
                || function.kind == FunctionKind::Extern
                || (function.kind == FunctionKind::Closure
                    && can_inline_closure(function, self.options.inline_structured_closures))
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

    fn prepare(&mut self) {
        self.assign_top_level_names();
        self.assign_external_export_aliases();
        self.assign_string_aliases();
        self.assign_property_names();
    }

    fn emit_chunks(mut self, plan: &IrJsChunkPlan) -> Result<Vec<IrJsChunk>, CodegenError> {
        let fallback_span = self.function(self.module.entry)?.span;
        let mut files = AHashSet::new();
        for file in std::iter::once(&plan.entry_file)
            .chain(plan.chunks.iter().map(|chunk| &chunk.file_name))
        {
            if file.is_empty()
                || Path::new(file).file_name().and_then(|name| name.to_str()) != Some(file)
            {
                return Err(CodegenError::new(
                    fallback_span,
                    "chunk file names must not contain directory components",
                ));
            }
            if !files.insert(file) {
                return Err(CodegenError::new(
                    fallback_span,
                    format!("duplicate chunk file name `{file}`"),
                ));
            }
        }
        self.prepare();
        for chunk in &plan.chunks {
            if let Some(module) = chunk.lazy_module {
                if self
                    .dynamic_chunk_files
                    .insert(module, chunk.file_name.clone())
                    .is_some()
                {
                    return Err(CodegenError::new(
                        fallback_span,
                        format!("dynamic module {module} belongs to more than one chunk"),
                    ));
                }
            }
        }
        let emitted = self
            .module
            .functions
            .iter()
            .filter(|function| {
                is_emitted_function(function, self.options.inline_structured_closures)
            })
            .map(|function| function.id)
            .collect::<AHashSet<_>>();
        let mut owners = emitted
            .iter()
            .copied()
            .map(|function| (function, None))
            .collect::<AHashMap<_, Option<usize>>>();
        for (chunk_index, chunk) in plan.chunks.iter().enumerate() {
            for function in &chunk.functions {
                if !emitted.contains(function) {
                    return Err(CodegenError::new(
                        self.module
                            .functions
                            .get(function.0 as usize)
                            .map_or(fallback_span, |item| item.span),
                        format!("function {} cannot be emitted as a chunk", function.0),
                    ));
                }
                if owners
                    .insert(*function, Some(chunk_index))
                    .flatten()
                    .is_some()
                {
                    return Err(CodegenError::new(
                        self.function(*function)?.span,
                        format!("function {} belongs to more than one chunk", function.0),
                    ));
                }
                if function_writes_global(
                    self.module,
                    *function,
                    &mut AHashSet::new(),
                    self.options.inline_structured_closures,
                ) {
                    return Err(CodegenError::new(
                        self.function(*function)?.span,
                        "functions that mutate module globals must remain in the entry chunk",
                    ));
                }
            }
        }

        let mut unit_functions = vec![Vec::new(); plan.chunks.len() + 1];
        for function in emitted {
            let unit = owners[&function].map_or(0, |chunk| chunk + 1);
            unit_functions[unit].push(function);
        }
        for functions in &mut unit_functions {
            functions.sort_unstable_by_key(|function| function.0);
        }

        let unit_files = std::iter::once(plan.entry_file.clone())
            .chain(plan.chunks.iter().map(|chunk| chunk.file_name.clone()))
            .collect::<Vec<_>>();
        let mut imports = vec![AHashMap::<usize, AHashSet<String>>::new(); unit_files.len()];
        let mut dynamic_imports = vec![AHashSet::<u32>::new(); unit_files.len()];
        for (unit, functions) in unit_functions.iter().enumerate() {
            let mut roots = functions.clone();
            if unit == 0 {
                roots.push(self.module.entry);
            }
            let references = collect_chunk_references(
                self.module,
                &roots,
                &self.string_aliases,
                self.options.inline_structured_closures,
            );
            dynamic_imports[unit].extend(references.dynamic_modules.iter().copied());
            for function in references.functions {
                let Some(owner) = owners.get(&function) else {
                    continue;
                };
                let source = owner.map_or(0, |chunk| chunk + 1);
                if source != unit {
                    imports[unit]
                        .entry(source)
                        .or_default()
                        .insert(self.function_name(function)?.to_string());
                }
            }
            if unit != 0 {
                let entry_imports = imports[unit].entry(0).or_default();
                for global in references.globals {
                    if !self
                        .module
                        .globals
                        .iter()
                        .any(|candidate| candidate.symbol == global && candidate.external)
                    {
                        entry_imports.insert(self.global_name(global)?.to_string());
                    }
                }
                entry_imports.extend(references.strings);
            }
        }
        for export in &self.module.exports {
            if let ExportBinding::Function(function) = export.binding {
                let source = owners
                    .get(&function)
                    .copied()
                    .flatten()
                    .map_or(0, |chunk| chunk + 1);
                if source != 0 {
                    imports[0]
                        .entry(source)
                        .or_default()
                        .insert(self.function_name(function)?.to_string());
                }
            }
        }
        for (chunk_index, chunk) in plan.chunks.iter().enumerate() {
            let Some(module_id) = chunk.lazy_module else {
                continue;
            };
            let module = self
                .module
                .lazy_modules
                .iter()
                .find(|module| module.id == module_id)
                .ok_or_else(|| {
                    CodegenError::new(
                        fallback_span,
                        format!("chunk references unknown dynamic module {module_id}"),
                    )
                })?;
            let unit = chunk_index + 1;
            for export in &module.exports {
                let (source, name) = match export.binding {
                    ExportBinding::Function(function) => (
                        owners
                            .get(&function)
                            .copied()
                            .flatten()
                            .map_or(0, |owner| owner + 1),
                        self.function_name(function)?.to_string(),
                    ),
                    ExportBinding::Global(global) => (0, self.global_name(global)?.to_string()),
                    ExportBinding::TypeOnly => {
                        return Err(CodegenError::new(
                            export.span,
                            format!("dynamic export `{}` has no runtime binding", export.name),
                        ));
                    }
                };
                if source != unit {
                    imports[unit].entry(source).or_default().insert(name);
                }
            }
        }

        let mut internal_exports = vec![AHashSet::<String>::new(); unit_files.len()];
        for dependencies in &imports {
            for (source, names) in dependencies {
                internal_exports[*source].extend(names.iter().cloned());
            }
        }

        let mut output = Vec::with_capacity(unit_files.len());
        for unit in 0..unit_files.len() {
            let mut code = String::new();
            emit_chunk_imports(
                &mut code,
                unit,
                &unit_files,
                &imports[unit],
                self.options.string_quote,
            );
            if unit == 0 {
                self.emit_module_preamble(&mut code)?;
            }
            for function in &unit_functions[unit] {
                let function = self.function(*function)?.clone();
                self.emit_function(&function, &mut code)?;
            }
            if unit == 0 {
                self.emit_entry_body(&mut code)?;
                self.emit_named_exports(&internal_exports[unit], &mut code);
                self.emit_exports_excluding(&internal_exports[unit], &mut code)?;
            } else {
                self.emit_named_exports(&internal_exports[unit], &mut code);
                if let Some(module) = plan.chunks[unit - 1].lazy_module {
                    self.emit_dynamic_module_exports(module, &mut code)?;
                }
            }
            output.push(IrJsChunk {
                file_name: unit_files[unit].clone(),
                code,
                dependencies: {
                    let mut files = imports[unit]
                        .iter()
                        .filter(|(source, names)| **source != unit && !names.is_empty())
                        .map(|(source, _)| unit_files[*source].clone())
                        .collect::<Vec<_>>();
                    files.sort_unstable();
                    files.dedup();
                    files
                },
                dynamic_dependencies: {
                    let mut files = dynamic_imports[unit]
                        .iter()
                        .filter_map(|module| self.dynamic_chunk_files.get(module).cloned())
                        .collect::<Vec<_>>();
                    files.sort_unstable();
                    files.dedup();
                    files
                },
            });
        }
        Ok(output)
    }

    fn emit_dynamic_module_exports(
        &self,
        module_id: u32,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let module = self
            .module
            .lazy_modules
            .iter()
            .find(|module| module.id == module_id)
            .ok_or_else(|| {
                CodegenError::new(
                    self.module.functions[self.module.entry.0 as usize].span,
                    format!("missing dynamic module {module_id}"),
                )
            })?;
        if module.exports.is_empty() {
            return Ok(());
        }
        out.push_str("export{");
        for (index, export) in module.exports.iter().enumerate() {
            if index != 0 {
                out.push(',');
            }
            let binding = match export.binding {
                ExportBinding::Function(function) => self.function_name(function)?,
                ExportBinding::Global(global) => self.global_name(global)?,
                ExportBinding::TypeOnly => {
                    return Err(CodegenError::new(
                        export.span,
                        format!("dynamic export `{}` has no runtime binding", export.name),
                    ));
                }
            };
            out.push_str(binding);
            if binding != export.name {
                out.push_str(" as ");
                out.push_str(export.name);
            }
        }
        out.push_str("};");
        Ok(())
    }

    fn emit_module_preamble(&mut self, out: &mut String) -> Result<(), CodegenError> {
        if !self.pooled_strings.is_empty() {
            out.push_str("let ");
            for (index, (value, name)) in self.pooled_strings.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                out.push_str(name);
                out.push('=');
                out.push_str(&render_string_literal(value, self.options.string_quote));
            }
            out.push(';');
        }
        let owned_globals = self
            .module
            .globals
            .iter()
            .filter(|global| !global.external)
            .collect::<Vec<_>>();
        if !owned_globals.is_empty() {
            out.push_str("let ");
            for (index, global) in owned_globals.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                out.push_str(self.global_name(global.symbol)?);
                self.declared_globals.insert(global.symbol);
            }
            out.push(';');
        }
        self.emit_external_export_aliases(out)?;
        Ok(())
    }

    fn emit_external_export_aliases(&self, out: &mut String) -> Result<(), CodegenError> {
        let mut aliases = self.external_export_aliases.iter().collect::<Vec<_>>();
        aliases.sort_unstable_by_key(|(symbol, _)| symbol.0);
        for (symbol, alias) in aliases {
            out.push_str("const ");
            out.push_str(alias);
            out.push('=');
            out.push_str(self.global_name(*symbol)?);
            out.push(';');
        }
        Ok(())
    }

    fn emit_entry_body(&mut self, out: &mut String) -> Result<(), CodegenError> {
        let entry = self.function(self.module.entry)?.clone();
        if entry.blocks.len() == 1 && entry.blocks[0].phis.is_empty() {
            self.emit_single_block(&entry, false, out)
        } else if can_structure(&entry) {
            self.emit_structured(&entry, false, out)
        } else {
            out.push_str("(()=>");
            self.emit_state_machine(&entry, out)?;
            out.push_str(")();");
            Ok(())
        }
    }

    fn emit_named_exports(&self, names: &AHashSet<String>, out: &mut String) {
        if names.is_empty() {
            return;
        }
        let mut names = names.iter().collect::<Vec<_>>();
        names.sort_unstable();
        if !out.is_empty() && !out.ends_with(';') {
            out.push(';');
        }
        out.push_str("export{");
        for (index, name) in names.iter().enumerate() {
            if index != 0 {
                out.push(',');
            }
            out.push_str(name);
        }
        out.push_str("};");
    }

    fn emit_exports(&self, out: &mut String) -> Result<(), CodegenError> {
        self.emit_exports_excluding(&AHashSet::new(), out)
    }

    fn emit_exports_excluding(
        &self,
        already_exported: &AHashSet<String>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let mut runtime_exports = Vec::<(&str, &str)>::new();
        for export in &self.module.exports {
            let internal = match export.binding {
                ExportBinding::Function(function) => self.function_name(function)?,
                ExportBinding::Global(symbol) => self
                    .external_export_aliases
                    .get(&symbol)
                    .map_or(self.global_name(symbol)?, String::as_str),
                ExportBinding::TypeOnly => continue,
            };
            let public = if self.options.mangle_exports {
                internal
            } else {
                export.name
            };
            if internal != public || !already_exported.contains(internal) {
                runtime_exports.push((internal, public));
            }
        }
        if runtime_exports.is_empty() {
            return Ok(());
        }
        if !out.is_empty() && !out.ends_with(';') {
            out.push(';');
        }
        out.push_str("export{");
        for (index, (internal, public)) in runtime_exports.iter().enumerate() {
            if index != 0 {
                out.push(',');
            }
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
        for global in &self.module.globals {
            if global.external {
                self.top_level_mangler.reserve(global.name);
                self.global_names
                    .insert(global.symbol, global.name.to_string());
            }
        }

        if !self.options.mangle_identifiers {
            for function in &self.module.functions {
                if !function.live
                    || matches!(function.kind, FunctionKind::Entry | FunctionKind::Extern)
                    || (function.kind == FunctionKind::Closure
                        && can_inline_closure(function, self.options.inline_structured_closures))
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
                if global.external {
                    continue;
                }
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
                || (function.kind == FunctionKind::Closure
                    && can_inline_closure(function, self.options.inline_structured_closures))
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
            if global.external {
                continue;
            }
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

    fn assign_external_export_aliases(&mut self) {
        for export in &self.module.exports {
            let ExportBinding::Global(symbol) = export.binding else {
                continue;
            };
            let Some(global) = self
                .module
                .globals
                .iter()
                .find(|global| global.symbol == symbol && global.external)
            else {
                continue;
            };
            let alias = if self.options.mangle_identifiers {
                self.top_level_mangler.next_name()
            } else {
                self.top_level_mangler
                    .unique_name(&format!("$host${}", global.name))
            };
            self.external_export_aliases.insert(symbol, alias);
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
            self.integer_analysis.function(function.id),
            !single_block && !structured,
            &self.top_level_mangler,
            &self.options,
        );
        context.inline_declarations = structured;
        let uses = &context.use_counts;
        let parameter_count = function
            .params
            .iter()
            .rposition(|param| uses.get(&param.value).copied().unwrap_or(0) != 0)
            .map_or(0, |index| index + 1);
        for (index, param) in function.params.iter().take(parameter_count).enumerate() {
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
        let uses = &context.use_counts;
        let mut cache = AHashMap::new();
        for instruction in &header.instructions {
            let Some(out) = instruction.out else {
                return Ok(None);
            };
            if !expression_only_op(&instruction.op) || uses.get(&out).copied() != Some(1) {
                return Ok(None);
            }
            let expression = self.render_instruction_op(instruction, context, &mut cache)?;
            cache.insert(out, expression);
        }
        let condition = strip_outer_parens(take_value(condition, context, &mut cache)?);
        let Some(then_value) =
            self.render_linear_return_path(function, then_block, context, uses, cache.clone())?
        else {
            return Ok(None);
        };
        let Some(else_value) =
            self.render_linear_return_path(function, else_block, context, uses, cache)?
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
        let mut deferred_effects = 0usize;
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
                let deferred_effect = matches!(instruction.op, ControlFlowOp::CallDirect { .. });
                if (!expression_only_op(&instruction.op) && !deferred_effect)
                    || (uses.get(&out).copied().unwrap_or(0) > 1 && !op_can_defer(&instruction.op))
                    || (deferred_effect && uses.get(&out).copied().unwrap_or(0) != 1)
                {
                    return Ok(None);
                }
                if deferred_effect {
                    deferred_effects += 1;
                    if deferred_effects > 1 {
                        return Ok(None);
                    }
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
            self.integer_analysis.function(function.id),
            false,
            &self.top_level_mangler,
            &self.options,
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
        let uses = &context.use_counts;
        let mut cache = AHashMap::<ValueId, String>::new();
        let mut previous_binding = false;
        for (index, instruction) in block.instructions.iter().enumerate() {
            let fuse_with_next = instruction.out.is_some_and(|value| {
                uses.get(&value).copied().unwrap_or(0) == 1 && can_fuse_value(block, index, value)
            });
            let mut statement = String::new();
            self.emit_linear_instruction(
                instruction,
                uses,
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
                out.push_str(&strip_outer_parens(take_value(
                    *value, &context, &mut cache,
                )?));
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
                let value = strip_outer_parens(take_value(*value, context, cache)?);
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
                out.push_str(&strip_outer_parens(take_value(*value, context, cache)?));
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
                out.push_str(&strip_outer_parens(take_value(*index, context, cache)?));
                out.push_str("]=");
                out.push_str(&strip_outer_parens(take_value(*value, context, cache)?));
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
                    out.push_str(&strip_outer_parens(take_value(*arg, context, cache)?));
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
            out.push_str(&strip_outer_parens(expression));
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
            self.integer_analysis.function(function.id),
            true,
            &self.top_level_mangler,
            &self.options,
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

        let uses = &context.use_counts;
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
                    uses,
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
            self.integer_analysis.function(function.id),
            false,
            &self.top_level_mangler,
            &self.options,
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
        let uses = &context.use_counts;
        let mut visited = AHashSet::new();
        let mut cache = AHashMap::new();
        self.emit_structured_path(
            function,
            function.entry,
            None,
            None,
            &context,
            uses,
            &mut cache,
            &mut visited,
            out,
        )?;
        if out.ends_with("return;") {
            out.truncate(out.len() - "return;".len());
        }
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
                let retained_condition = match &shape {
                    ControlShape::If { header, .. } => {
                        let block = &function.blocks[header.0 as usize];
                        if block.instructions.is_empty() {
                            match block.terminator {
                                Some(Terminator::Branch { condition, .. })
                                    if cache.contains_key(&condition) =>
                                {
                                    Some(condition)
                                }
                                _ => None,
                            }
                        } else {
                            None
                        }
                    }
                    ControlShape::Loop { .. } => None,
                };
                self.flush_cache_except(cache, context, out, retained_condition)?;
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
                        let then_end = self.emit_structured_path(
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
                        let else_end = self.emit_structured_path(
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
                        let mut deferred_merge = None;
                        if is_true_literal(&condition) {
                            out.push_str(&then_output);
                            if then_end == PathEnd::Terminated {
                                return Ok(PathEnd::Terminated);
                            }
                        } else if is_false_literal(&condition) {
                            out.push_str(&else_output);
                            if else_end == PathEnd::Terminated {
                                return Ok(PathEnd::Terminated);
                            }
                        } else if let Some((declare, target, then_value, else_value, trailing)) =
                            merge_conditional_assignments(&then_output, &else_output)
                        {
                            let mut value = String::new();
                            if is_true_literal(then_value) && is_false_literal(else_value) {
                                value.push_str(&condition);
                            } else if is_false_literal(then_value) && is_true_literal(else_value) {
                                value.push_str(&negate_condition(condition.clone()));
                            } else if is_true_literal(then_value) {
                                push_logical_operand(&mut value, &condition, IrBinaryOp::Or);
                                value.push_str("||");
                                push_logical_operand(&mut value, else_value, IrBinaryOp::Or);
                            } else if is_false_literal(else_value) {
                                push_logical_operand(&mut value, &condition, IrBinaryOp::And);
                                value.push_str("&&");
                                push_logical_operand(&mut value, then_value, IrBinaryOp::And);
                            } else {
                                value.push_str(&condition);
                                value.push('?');
                                value.push_str(then_value);
                                value.push(':');
                                value.push_str(else_value);
                            }
                            let declaration_tail = if trailing.is_empty() {
                                Some(None)
                            } else if declare {
                                uninitialized_declaration_tail(trailing).map(Some)
                            } else {
                                None
                            };
                            let deferred = if declaration_tail.is_some() {
                                function.blocks[merge_block.0 as usize]
                                    .phis
                                    .iter()
                                    .find(|phi| {
                                        context.value_name(phi.out).ok() == Some(target)
                                            && uses.get(&phi.out).copied() == Some(1)
                                            && immediately_branches_on_phi(
                                                function,
                                                merge_block,
                                                phi.out,
                                            )
                                    })
                                    .map(|phi| phi.out)
                            } else {
                                None
                            };
                            if let Some(value_id) = deferred {
                                if let Some(names) = declaration_tail.flatten() {
                                    out.push_str("var ");
                                    out.push_str(names);
                                    out.push(';');
                                }
                                deferred_merge = Some((value_id, format!("({value})")));
                            } else {
                                if declare {
                                    out.push_str("var ");
                                }
                                out.push_str(target);
                                out.push('=');
                                out.push_str(&value);
                                out.push_str(trailing);
                                out.push(';');
                            }
                        } else if let Some((then_target, then_value, else_target, else_value)) =
                            conditional_assignment_expression(&then_output, &else_output)
                        {
                            out.push_str(&condition);
                            out.push('?');
                            out.push_str(then_target);
                            out.push('=');
                            out.push_str(then_value);
                            out.push(':');
                            out.push_str(else_target);
                            out.push('=');
                            out.push_str(else_value);
                            out.push(';');
                        } else if else_output.is_empty() {
                            out.push_str("if(");
                            out.push_str(&condition);
                            if is_braceless_statement(&then_output) {
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
                            if is_braceless_statement(&else_output) {
                                out.push(')');
                                out.push_str(&else_output);
                            } else {
                                out.push_str("){");
                                out.push_str(&else_output);
                                out.push('}');
                            }
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
                        if let Some((value, expression)) = deferred_merge {
                            cache.insert(value, expression);
                        }
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
                            let reuse_for_spelling = match self.options.loop_spelling {
                                LoopSpelling::Auto => {
                                    out.matches("for(").count() > out.matches("while(").count()
                                }
                                LoopSpelling::While => false,
                                LoopSpelling::For => true,
                            };
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
                        let loop_body_open = compact_loop.then_some(out.len() - 1);

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
                        if loop_body_open
                            .is_some_and(|open| is_braceless_statement(&out[open + 1..]))
                        {
                            out.remove(loop_body_open.expect("checked loop body opening"));
                        } else {
                            out.push('}');
                        }
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
                    out.push_str(&strip_outer_parens(take_value(*value, context, cache)?));
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

    fn flush_cache_except(
        &self,
        cache: &mut AHashMap<ValueId, String>,
        context: &LocalNames,
        out: &mut String,
        retained: Option<ValueId>,
    ) -> Result<(), CodegenError> {
        let retained =
            retained.and_then(|value| cache.remove(&value).map(|expression| (value, expression)));
        let mut values = cache.drain().collect::<Vec<_>>();
        values.sort_by_key(|(value, _)| value.0);
        for (value, expression) in values {
            if context.claim_declaration(value)? {
                out.push_str("var ");
            }
            out.push_str(context.value_name(value)?);
            out.push('=');
            out.push_str(&strip_outer_parens(expression));
            out.push(';');
        }
        if let Some((value, expression)) = retained {
            cache.insert(value, expression);
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
        let mut single_assignment_copy = None;
        let mut declaration_needed = false;
        for ((target, source_value), source) in copies.iter().zip(sources) {
            let target_value = *target;
            let target = context.value_name(target_value)?.to_string();
            if target != source {
                declaration_needed |= context.claim_declaration(target_value)?;
                single_assignment_copy = assignments
                    .is_empty()
                    .then_some((target_value, *source_value));
                assignments.push((target, source));
            }
        }
        if assignments.len() == 1 {
            let compact_update = (!declaration_needed
                && self.options.mutation_spelling != MutationSpelling::Assignment
                && single_assignment_copy.is_some_and(|(target, source)| {
                    is_one_use_increment_copy(
                        function,
                        BlockId(from),
                        target,
                        source,
                        &context.use_counts,
                        context,
                    )
                }))
            .then_some(self.options.mutation_spelling);
            if let Some(spelling) = compact_update {
                if spelling == MutationSpelling::Prefix {
                    out.push_str("++");
                }
                out.push_str(&assignments[0].0);
                if spelling == MutationSpelling::Postfix {
                    out.push_str("++");
                }
                out.push(';');
                return Ok(());
            }
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
                let reusable_temporary = self
                    .options
                    .scalar_phi_copies
                    .then(|| reusable_parallel_copy_temporary(BlockId(to), context, &assignments));
                let temporary = reusable_temporary
                    .as_ref()
                    .and_then(|name| name.as_deref())
                    .map(|name| (name, false))
                    .or_else(|| {
                        context
                            .parallel_copy_temp
                            .as_deref()
                            .map(|name| (name, true))
                    });
                if let Some(scalar) = scalar_parallel_assignments(&assignments, temporary) {
                    let tuple_size = assignments
                        .iter()
                        .map(|(target, source)| target.len() + source.len())
                        .sum::<usize>()
                        + assignments.len().saturating_sub(1) * 2
                        + 6;
                    if self.options.scalar_phi_copies || scalar.len() < tuple_size {
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
            let coercion_is_elidable = self.options.elide_safe_integer_coercions
                && instruction
                    .out
                    .is_some_and(|out| context.can_elide_i32_coercion(out));
            match &instruction.op {
                ControlFlowOp::Unary {
                    op: IrUnaryOp::Neg,
                    value,
                } => {
                    let value = take_value(*value, context, cache)?;
                    return Ok(if coercion_is_elidable {
                        format!("(-{value})")
                    } else {
                        format!("(-{value}|0)")
                    });
                }
                ControlFlowOp::Binary { op, lhs, rhs }
                    if matches!(
                        op,
                        IrBinaryOp::Add
                            | IrBinaryOp::Sub
                            | IrBinaryOp::Mul
                            | IrBinaryOp::Div
                            | IrBinaryOp::Mod
                            | IrBinaryOp::BitAnd
                            | IrBinaryOp::BitOr
                            | IrBinaryOp::Xor
                            | IrBinaryOp::ShiftLeft
                            | IrBinaryOp::ShiftRight
                            | IrBinaryOp::UnsignedShiftRight
                    ) =>
                {
                    let lhs_child = context.binary_operator(*lhs);
                    let rhs_child = context.binary_operator(*rhs);
                    let mut lhs = take_value(*lhs, context, cache)?;
                    let mut rhs = take_value(*rhs, context, cache)?;
                    if matches!(
                        op,
                        IrBinaryOp::BitAnd
                            | IrBinaryOp::BitOr
                            | IrBinaryOp::Xor
                            | IrBinaryOp::ShiftLeft
                            | IrBinaryOp::ShiftRight
                            | IrBinaryOp::UnsignedShiftRight
                    ) {
                        lhs = strip_redundant_i32_coercion(lhs);
                        rhs = strip_redundant_i32_coercion(rhs);
                    }
                    lhs = render_binary_operand(lhs, lhs_child, *op, BinaryOperandSide::Left);
                    rhs = render_binary_operand(rhs, rhs_child, *op, BinaryOperandSide::Right);
                    let rhs = token_safe_binary_rhs(*op, rhs);
                    return Ok(match op {
                        IrBinaryOp::Mul if coercion_is_elidable => format!("({lhs}*{rhs})"),
                        IrBinaryOp::Mul => format!("({lhs}*{rhs}|0)"),
                        IrBinaryOp::Mod if is_nonzero_i32_literal(&rhs) => {
                            format!("({lhs}%{rhs})")
                        }
                        IrBinaryOp::BitAnd => format!("({lhs}&{rhs})"),
                        IrBinaryOp::BitOr => format!("({lhs}|{rhs})"),
                        IrBinaryOp::Xor => format!("({lhs}^{rhs})"),
                        IrBinaryOp::ShiftLeft => format!("({lhs}<<{rhs})"),
                        IrBinaryOp::ShiftRight => format!("({lhs}>>{rhs})"),
                        IrBinaryOp::UnsignedShiftRight => format!("({lhs}>>>{rhs}|0)"),
                        _ if coercion_is_elidable => {
                            format!("({lhs}{}{rhs})", binary_operator(*op))
                        }
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
                .unwrap_or_else(|| render_string_literal(value, self.options.string_quote)),
            ControlFlowOp::Const(value) => render_const(
                value,
                self.options.compact_boolean_literals,
                self.options.string_quote,
            ),
            ControlFlowOp::Unary { op, value: operand } => format!(
                "{}{}",
                match op {
                    IrUnaryOp::Neg => "-",
                    IrUnaryOp::Not => "!",
                },
                value(*operand, cache)?
            ),
            ControlFlowOp::Binary { op, lhs, rhs } => {
                let lhs = render_binary_operand(
                    value(*lhs, cache)?,
                    context.binary_operator(*lhs),
                    *op,
                    BinaryOperandSide::Left,
                );
                let rhs = render_binary_operand(
                    value(*rhs, cache)?,
                    context.binary_operator(*rhs),
                    *op,
                    BinaryOperandSide::Right,
                );
                let rhs = token_safe_binary_rhs(*op, rhs);
                if matches!(op, IrBinaryOp::Eq | IrBinaryOp::NotEq)
                    && is_rendered_string_literal(&lhs)
                    && is_rendered_string_literal(&rhs)
                {
                    let equal = lhs == rhs;
                    render_const(
                        &ConstValue::Bool(if *op == IrBinaryOp::Eq { equal } else { !equal }),
                        self.options.compact_boolean_literals,
                        self.options.string_quote,
                    )
                } else {
                    format!("({lhs}{}{rhs})", binary_operator(*op))
                }
            }
            ControlFlowOp::TypeCheck {
                value: input,
                target,
            } => render_js_type_check(&value(*input, cache)?, target, self.options.string_quote)?,
            ControlFlowOp::Array(values) => {
                let mut rendered = String::from("[");
                for (index, item) in values.iter().enumerate() {
                    if index != 0 {
                        rendered.push(',');
                    }
                    rendered.push_str(&strip_outer_parens(value(*item, cache)?));
                }
                rendered.push(']');
                if self.options.pack_string_arrays {
                    packed_string_array(values, context, self.options.string_quote)
                        .filter(|packed| packed.len() < rendered.len())
                        .unwrap_or(rendered)
                } else {
                    rendered
                }
            }
            ControlFlowOp::Struct { fields: values, .. } => {
                let mut rendered = String::from("[");
                for (index, item) in values.iter().enumerate() {
                    if index != 0 {
                        rendered.push(',');
                    }
                    rendered.push_str(&strip_outer_parens(value(*item, cache)?));
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
            ControlFlowOp::HostFieldGet { object, property } => {
                format!("{}.{}", value(*object, cache)?, property)
            }
            ControlFlowOp::HostFieldSet {
                object,
                property,
                value: assigned,
            } => format!(
                "{}.{}={}",
                value(*object, cache)?,
                property,
                value(*assigned, cache)?
            ),
            ControlFlowOp::IndexGet { object, index } => {
                format!(
                    "{}[{}]",
                    value(*object, cache)?,
                    strip_outer_parens(value(*index, cache)?)
                )
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
            ControlFlowOp::HostCall {
                receiver,
                method,
                args,
                ..
            } => {
                let receiver = value(*receiver, cache)?;
                self.render_call(&format!("{receiver}.{method}"), None, args, context, cache)?
            }
            ControlFlowOp::DynamicImport { module } => self.render_dynamic_import(*module)?,
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
                            rendered.push_str(&strip_outer_parens(value(*item, cache)?));
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

    fn render_dynamic_import(&self, module_id: u32) -> Result<String, CodegenError> {
        let module = self
            .module
            .lazy_modules
            .iter()
            .find(|module| module.id == module_id)
            .ok_or_else(|| {
                CodegenError::new(
                    self.module.functions[self.module.entry.0 as usize].span,
                    format!("missing dynamic module {module_id}"),
                )
            })?;
        if let Some(file) = self.dynamic_chunk_files.get(&module_id) {
            let file = render_string_literal(&format!("./{file}"), self.options.string_quote);
            let source = render_string_literal(module.source, self.options.string_quote);
            return Ok(format!(
                "import({file}).catch(e=>Promise.reject({{specifier:{source},message:String(e)}}))"
            ));
        }

        let mut namespace = String::from("Promise.resolve({");
        for (index, export) in module.exports.iter().enumerate() {
            if index != 0 {
                namespace.push(',');
            }
            namespace.push_str(&render_string_literal(
                export.name,
                self.options.string_quote,
            ));
            namespace.push(':');
            namespace.push_str(match export.binding {
                ExportBinding::Function(function) => self.function_name(function)?,
                ExportBinding::Global(global) => self.global_name(global)?,
                ExportBinding::TypeOnly => {
                    return Err(CodegenError::new(
                        export.span,
                        format!("dynamic export `{}` has no runtime binding", export.name),
                    ));
                }
            });
        }
        namespace.push_str("})");
        Ok(namespace)
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
        if intrinsic == Intrinsic::IntImul {
            return self.render_call("Math.imul", None, args, context, cache);
        }
        let constructor = match intrinsic {
            Intrinsic::MapNew => Some("Map"),
            Intrinsic::SetNew => Some("Set"),
            Intrinsic::ArrayBufferNew => Some("ArrayBuffer"),
            Intrinsic::SharedArrayBufferNew => Some("SharedArrayBuffer"),
            Intrinsic::Uint8ArrayNew => Some("Uint8Array"),
            _ => None,
        };
        if let Some(constructor) = constructor {
            let mut rendered = format!("new {constructor}(");
            for (index, arg) in args.iter().enumerate() {
                if index != 0 {
                    rendered.push(',');
                }
                rendered.push_str(&strip_outer_parens(take_value(*arg, context, cache)?));
            }
            rendered.push(')');
            return Ok(rendered);
        }
        let receiver = receiver.ok_or_else(|| {
            CodegenError::new(
                self.function(self.module.entry).unwrap().span,
                "missing receiver",
            )
        })?;
        let receiver = take_value(receiver, context, cache)?;
        let property = match intrinsic {
            Intrinsic::UnwrapNullable | Intrinsic::UnwrapUnion => return Ok(receiver),
            Intrinsic::ArrayLength | Intrinsic::StringLength => {
                return Ok(format!("{receiver}.length"))
            }
            Intrinsic::MapSize | Intrinsic::SetSize => return Ok(format!("{receiver}.size")),
            Intrinsic::BufferByteLength | Intrinsic::Uint8ArrayByteLength => {
                return Ok(format!("{receiver}.byteLength"));
            }
            Intrinsic::Uint8ArrayLength => return Ok(format!("{receiver}.length")),
            Intrinsic::Uint8ArrayByteOffset => return Ok(format!("{receiver}.byteOffset")),
            Intrinsic::Uint8ArrayBuffer => return Ok(format!("{receiver}.buffer")),
            Intrinsic::MapGet => {
                let call =
                    self.render_call(&format!("{receiver}.get"), None, args, context, cache)?;
                return Ok(format!("({call}??null)"));
            }
            Intrinsic::StringCharCodeAt => {
                let call = self.render_call(
                    &format!("{receiver}.charCodeAt"),
                    None,
                    args,
                    context,
                    cache,
                )?;
                return Ok(format!("({call}|0)"));
            }
            Intrinsic::IntToString | Intrinsic::IntToUnsignedString => {
                let radix = if let Some(radix) = args.first() {
                    take_value(*radix, context, cache)?
                } else {
                    "10".to_string()
                };
                return Ok(if matches!(intrinsic, Intrinsic::IntToUnsignedString) {
                    format!("({receiver}>>>0).toString({radix})")
                } else {
                    format!("({receiver}).toString({radix})")
                });
            }
            Intrinsic::FloatAbs
            | Intrinsic::FloatFloor
            | Intrinsic::FloatCeil
            | Intrinsic::FloatMin
            | Intrinsic::FloatMax => {
                let method = match intrinsic {
                    Intrinsic::FloatAbs => "abs",
                    Intrinsic::FloatFloor => "floor",
                    Intrinsic::FloatCeil => "ceil",
                    Intrinsic::FloatMin => "min",
                    Intrinsic::FloatMax => "max",
                    _ => unreachable!(),
                };
                let mut rendered = format!("Math.{method}({}", strip_outer_parens(receiver));
                for arg in args {
                    rendered.push(',');
                    rendered.push_str(&strip_outer_parens(take_value(*arg, context, cache)?));
                }
                rendered.push(')');
                return Ok(rendered);
            }
            Intrinsic::ArrayMap => "map",
            Intrinsic::ArrayFilter => "filter",
            Intrinsic::ArrayReduce => "reduce",
            Intrinsic::ArrayForEach => "forEach",
            Intrinsic::ArrayPush => "push",
            Intrinsic::ArrayPop => "pop",
            Intrinsic::MapSet => "set",
            Intrinsic::MapHas => "has",
            Intrinsic::MapDelete => "delete",
            Intrinsic::MapClear => "clear",
            Intrinsic::SetAdd => "add",
            Intrinsic::SetHas => "has",
            Intrinsic::SetDelete => "delete",
            Intrinsic::SetClear => "clear",
            Intrinsic::BufferSlice | Intrinsic::Uint8ArraySlice => "slice",
            Intrinsic::Uint8ArraySubarray => "subarray",
            Intrinsic::StringIncludes => "includes",
            Intrinsic::StringStartsWith => "startsWith",
            Intrinsic::StringEndsWith => "endsWith",
            Intrinsic::StringToUpperCase => "toUpperCase",
            Intrinsic::StringToLowerCase => "toLowerCase",
            Intrinsic::Print
            | Intrinsic::IntImul
            | Intrinsic::MapNew
            | Intrinsic::SetNew
            | Intrinsic::ArrayBufferNew
            | Intrinsic::SharedArrayBufferNew
            | Intrinsic::Uint8ArrayNew => unreachable!(),
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
        if !can_inline_closure(&function, self.options.inline_structured_closures) {
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
                self.integer_analysis.function(function.id),
                false,
                &wrapper_mangler,
                &self.options,
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
            self.integer_analysis.function(function.id),
            false,
            &self.top_level_mangler,
            &self.options,
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
        if function.blocks.len() > 1 {
            context.inline_declarations = true;
            let parameters = render_arrow_parameters(&function, &context)?;
            let mut body = String::new();
            self.emit_structured_with_context(&function, true, context, &mut body)?;
            return Ok(format!("{parameters}=>{body}"));
        }
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
            value.push_str(default_value(
                &field.ty,
                self.options.compact_boolean_literals,
            ));
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

#[derive(Debug, Default)]
struct ChunkReferences {
    functions: AHashSet<FunctionId>,
    globals: AHashSet<SymbolId>,
    strings: AHashSet<String>,
    dynamic_modules: AHashSet<u32>,
}

fn is_emitted_function(function: &ControlFlowFunction<'_>, inline_structured: bool) -> bool {
    function.live
        && !matches!(function.kind, FunctionKind::Entry | FunctionKind::Extern)
        && !(function.kind == FunctionKind::Closure
            && can_inline_closure(function, inline_structured))
}

fn collect_chunk_references(
    module: &ControlFlowModule<'_>,
    roots: &[FunctionId],
    string_aliases: &AHashMap<String, String>,
    inline_structured: bool,
) -> ChunkReferences {
    let mut references = ChunkReferences::default();
    let mut pending = roots.to_vec();
    let mut visited = AHashSet::new();
    while let Some(function_id) = pending.pop() {
        if !visited.insert(function_id) {
            continue;
        }
        let Some(function) = module.functions.get(function_id.0 as usize) else {
            continue;
        };
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            match &instruction.op {
                ControlFlowOp::LoadGlobal(global) | ControlFlowOp::StoreGlobal { global, .. } => {
                    references.globals.insert(*global);
                }
                ControlFlowOp::Const(ConstValue::String(value)) => {
                    if let Some(alias) = string_aliases.get(value) {
                        references.strings.insert(alias.clone());
                    }
                }
                ControlFlowOp::DynamicImport { module } => {
                    references.dynamic_modules.insert(*module);
                }
                ControlFlowOp::NewClass {
                    constructor: Some(target),
                    ..
                }
                | ControlFlowOp::Closure {
                    function: target, ..
                }
                | ControlFlowOp::CallDirect {
                    function: target, ..
                }
                | ControlFlowOp::CallMethod {
                    function: target, ..
                } => {
                    let Some(target_function) = module.functions.get(target.0 as usize) else {
                        continue;
                    };
                    if target_function.kind == FunctionKind::Closure
                        && can_inline_closure(target_function, inline_structured)
                    {
                        pending.push(*target);
                    } else if is_emitted_function(target_function, inline_structured) {
                        references.functions.insert(*target);
                    }
                }
                _ => {}
            }
        }
    }
    references
}

fn function_writes_global(
    module: &ControlFlowModule<'_>,
    function_id: FunctionId,
    visited: &mut AHashSet<FunctionId>,
    inline_structured: bool,
) -> bool {
    if !visited.insert(function_id) {
        return false;
    }
    let Some(function) = module.functions.get(function_id.0 as usize) else {
        return false;
    };
    function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .any(|instruction| match &instruction.op {
            ControlFlowOp::StoreGlobal { .. } => true,
            ControlFlowOp::Closure {
                function: target, ..
            } => module
                .functions
                .get(target.0 as usize)
                .is_some_and(|target_function| {
                    target_function.kind == FunctionKind::Closure
                        && can_inline_closure(target_function, inline_structured)
                        && function_writes_global(module, *target, visited, inline_structured)
                }),
            _ => false,
        })
}

fn emit_chunk_imports(
    out: &mut String,
    current: usize,
    files: &[String],
    imports: &AHashMap<usize, AHashSet<String>>,
    quote: StringQuote,
) {
    let mut sources = imports.iter().collect::<Vec<_>>();
    sources.sort_unstable_by(|(left, _), (right, _)| files[**left].cmp(&files[**right]));
    for (source, names) in sources {
        if *source == current || names.is_empty() {
            continue;
        }
        let mut names = names.iter().collect::<Vec<_>>();
        names.sort_unstable();
        out.push_str("import{");
        for (index, name) in names.iter().enumerate() {
            if index != 0 {
                out.push(',');
            }
            out.push_str(name);
        }
        out.push_str("}from");
        out.push_str(&render_string_literal(
            &format!("./{}", files[*source]),
            quote,
        ));
        out.push(';');
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

fn scalar_parallel_assignments(
    assignments: &[(String, String)],
    temporary: Option<(&str, bool)>,
) -> Option<String> {
    if let Some(ordered) = order_scalar_assignments(assignments) {
        let mut output = String::new();
        for (target, source) in ordered {
            output.push_str(target);
            output.push('=');
            output.push_str(source);
            output.push(';');
        }
        return Some(output);
    }

    let (temporary, declare_temporary) = temporary?;
    let mut remaining = assignments.to_vec();
    let mut output = String::new();
    let mut temporary_declared = false;
    while !remaining.is_empty() {
        if let Some(index) = remaining.iter().position(|(target, _)| {
            remaining.iter().all(|(other_target, source)| {
                other_target == target || !expression_references_name(source, target)
            })
        }) {
            let (target, source) = remaining.remove(index);
            output.push_str(&target);
            output.push('=');
            output.push_str(&source);
            output.push(';');
            continue;
        }

        if remaining
            .iter()
            .any(|(_, source)| expression_references_name(source, temporary))
        {
            return None;
        }
        let saved = remaining[0].0.clone();
        if temporary_declared || !declare_temporary {
            output.push_str(temporary);
        } else {
            output.push_str("var ");
            output.push_str(temporary);
            temporary_declared = true;
        }
        output.push('=');
        output.push_str(&saved);
        output.push(';');
        for (_, source) in &mut remaining {
            *source = replace_identifier(source, &saved, temporary);
        }
    }
    Some(output)
}

fn reusable_parallel_copy_temporary(
    target: BlockId,
    context: &LocalNames,
    assignments: &[(String, String)],
) -> Option<String> {
    let live_names = context.live_in_values[target.0 as usize]
        .iter()
        .filter_map(|value| context.value_names.get(value))
        .collect::<AHashSet<_>>();
    let declared = context.declared_names.borrow();
    let mut candidates = context
        .value_names
        .values()
        .filter(|name| declared.contains(*name) && !live_names.contains(name))
        .filter(|name| {
            assignments.iter().all(|(target, source)| {
                target != *name && !expression_references_name(source, name)
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    candidates
        .sort_unstable_by(|left, right| left.len().cmp(&right.len()).then_with(|| left.cmp(right)));
    candidates.dedup();
    candidates.into_iter().next()
}

fn live_in_values(function: &ControlFlowFunction<'_>) -> Vec<AHashSet<ValueId>> {
    let block_count = function.blocks.len();
    let mut definitions = vec![AHashSet::new(); block_count];
    let mut local_uses = vec![AHashSet::new(); block_count];
    let mut phi_definitions = vec![AHashSet::new(); block_count];
    for block in &function.blocks {
        let index = block.id.0 as usize;
        for phi in &block.phis {
            definitions[index].insert(phi.out);
            phi_definitions[index].insert(phi.out);
        }
        for instruction in &block.instructions {
            for value in op_values(&instruction.op) {
                if !definitions[index].contains(&value) {
                    local_uses[index].insert(value);
                }
            }
            if let Some(out) = instruction.out {
                definitions[index].insert(out);
            }
        }
        for value in block
            .terminator
            .as_ref()
            .into_iter()
            .flat_map(terminator_values)
        {
            if !definitions[index].contains(&value) {
                local_uses[index].insert(value);
            }
        }
    }

    let mut live_in = vec![AHashSet::new(); block_count];
    let mut live_out = vec![AHashSet::new(); block_count];
    loop {
        let mut changed = false;
        for block in function.blocks.iter().rev() {
            let index = block.id.0 as usize;
            let mut output = AHashSet::new();
            for successor in block_successors(block) {
                let successor_index = successor.0 as usize;
                output.extend(
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
                        output.insert(*value);
                    }
                }
            }
            let mut input = local_uses[index].clone();
            input.extend(output.difference(&definitions[index]).copied());
            if output != live_out[index] {
                live_out[index] = output;
                changed = true;
            }
            if input != live_in[index] {
                live_in[index] = input;
                changed = true;
            }
        }
        if !changed {
            return live_in;
        }
    }
}

fn replace_identifier(expression: &str, from: &str, to: &str) -> String {
    let mut output = String::with_capacity(expression.len());
    let mut copied_until = 0usize;
    for (start, end) in expression_identifier_spans(expression) {
        if &expression[start..end] == from {
            output.push_str(&expression[copied_until..start]);
            output.push_str(to);
            copied_until = end;
        }
    }
    output.push_str(&expression[copied_until..]);
    output
}

fn merge_conditional_assignments<'a>(
    then_output: &'a str,
    else_output: &'a str,
) -> Option<(bool, &'a str, &'a str, &'a str, &'a str)> {
    let (then_declare, then_target, then_value, then_trailing) =
        parse_single_assignment(then_output)?;
    let (else_declare, else_target, else_value, else_trailing) =
        parse_single_assignment(else_output)?;
    if !then_trailing.is_empty() && !else_trailing.is_empty() && then_trailing != else_trailing {
        return None;
    }
    (then_target == else_target).then_some((
        then_declare || else_declare,
        then_target,
        then_value,
        else_value,
        if then_trailing.is_empty() {
            else_trailing
        } else {
            then_trailing
        },
    ))
}

fn conditional_assignment_expression<'a>(
    then_output: &'a str,
    else_output: &'a str,
) -> Option<(&'a str, &'a str, &'a str, &'a str)> {
    let (then_declare, then_target, then_value, then_trailing) =
        parse_single_assignment(then_output)?;
    let (else_declare, else_target, else_value, else_trailing) =
        parse_single_assignment(else_output)?;
    (!then_declare
        && !else_declare
        && then_trailing.is_empty()
        && else_trailing.is_empty()
        && then_target != else_target)
        .then_some((then_target, then_value, else_target, else_value))
}

fn push_logical_operand(out: &mut String, value: &str, parent: IrBinaryOp) {
    let needs_parentheses = logical_operand_needs_parentheses(value, parent);
    if needs_parentheses {
        out.push('(');
    }
    out.push_str(value);
    if needs_parentheses {
        out.push(')');
    }
}

fn logical_operand_needs_parentheses(value: &str, parent: IrBinaryOp) -> bool {
    let bytes = value.as_bytes();
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(active) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == active {
                quote = None;
            }
            index += 1;
            continue;
        }
        match byte {
            b'\'' | b'"' | b'`' => quote = Some(byte),
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b',' | b'?' if depth == 0 => return true,
            b'=' if depth == 0 && bytes.get(index + 1) == Some(&b'>') => return true,
            b'|' if depth == 0
                && parent == IrBinaryOp::And
                && bytes.get(index + 1) == Some(&b'|') =>
            {
                return true;
            }
            _ => {}
        }
        index += 1;
    }
    false
}

fn for_update_clause(output: &str) -> Option<String> {
    let clause = output.strip_suffix(';')?;
    (!clause.contains(';')
        && parse_single_assignment(output)
            .is_some_and(|(declare, _, _, trailing)| !declare && trailing.is_empty()))
    .then(|| clause.to_string())
}

fn is_one_use_increment_copy(
    function: &ControlFlowFunction<'_>,
    from: BlockId,
    target: ValueId,
    source: ValueId,
    uses: &AHashMap<ValueId, usize>,
    context: &LocalNames,
) -> bool {
    if uses.get(&source).copied() != Some(1) || !context.can_elide_i32_coercion(source) {
        return false;
    }
    let Some(instruction) = function.blocks[from.0 as usize]
        .instructions
        .iter()
        .find(|instruction| instruction.out == Some(source))
    else {
        return false;
    };
    let ControlFlowOp::Binary {
        op: IrBinaryOp::Add,
        lhs,
        rhs,
    } = instruction.op
    else {
        return false;
    };
    (lhs == target && is_int_constant(function, rhs, 1))
        || (rhs == target && is_int_constant(function, lhs, 1))
}

fn is_int_constant(function: &ControlFlowFunction<'_>, value: ValueId, expected: i64) -> bool {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .any(|instruction| {
            instruction.out == Some(value)
                && matches!(instruction.op, ControlFlowOp::Const(ConstValue::Int(value)) if value == expected)
        })
}

fn parse_single_assignment(output: &str) -> Option<(bool, &str, &str, &str)> {
    let statement = output.strip_suffix(';')?;
    let (declare, statement) = statement
        .strip_prefix("var ")
        .map_or((false, statement), |statement| (true, statement));
    let (assignment_statement, trailing) = if declare {
        split_top_level_comma(statement).map_or((statement, ""), |index| {
            (&statement[..index], &statement[index..])
        })
    } else {
        (statement, "")
    };
    let assignment = assignment_statement.find('=')?;
    let target = &assignment_statement[..assignment];
    let value = &assignment_statement[assignment + 1..];
    (!target.is_empty()
        && target.bytes().all(is_js_identifier_byte)
        && !value.is_empty()
        && !value.contains(';'))
    .then_some((declare, target, value, trailing))
}

fn uninitialized_declaration_tail(trailing: &str) -> Option<&str> {
    let names = trailing.strip_prefix(',')?;
    (!names.is_empty()
        && names.split(',').all(|name| {
            !name.is_empty()
                && is_js_identifier_start(name.as_bytes()[0])
                && name.bytes().all(is_js_identifier_byte)
        }))
    .then_some(names)
}

fn split_top_level_comma(value: &str) -> Option<usize> {
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
        match character {
            '\'' | '"' | '`' => quote = Some(character),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => return Some(index),
            _ => {}
        }
    }
    None
}

fn is_nonzero_i32_literal(expression: &str) -> bool {
    expression.parse::<i32>().is_ok_and(|value| value != 0)
}

fn is_braceless_statement(output: &str) -> bool {
    let Some(statement) = output.strip_suffix(';') else {
        return false;
    };
    !statement.is_empty()
        && !statement.contains([';', '{', '}'])
        && !statement.starts_with("let ")
        && !statement.starts_with("const ")
        && !statement.starts_with("function ")
        && !statement.starts_with("class ")
        && !statement.starts_with("if(")
        && !statement.starts_with("for(")
        && !statement.starts_with("while(")
}

fn expression_references_name(expression: &str, name: &str) -> bool {
    expression_identifier_spans(expression)
        .into_iter()
        .any(|(start, end)| &expression[start..end] == name)
}

fn expression_identifier_spans(expression: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut index = 0usize;
    scan_generated_js(expression.as_bytes(), &mut index, false, &mut spans);
    spans
}

fn scan_generated_js(
    bytes: &[u8],
    index: &mut usize,
    stop_at_brace: bool,
    spans: &mut Vec<(usize, usize)>,
) {
    while *index < bytes.len() {
        match bytes[*index] {
            b'\'' | b'"' => skip_generated_js_string(bytes, index),
            b'`' => scan_generated_js_template(bytes, index, spans),
            b'{' => {
                *index += 1;
                scan_generated_js(bytes, index, true, spans);
            }
            b'}' if stop_at_brace => {
                *index += 1;
                return;
            }
            byte if is_js_identifier_start(byte) => {
                let start = *index;
                *index += 1;
                while *index < bytes.len() && is_js_identifier_byte(bytes[*index]) {
                    *index += 1;
                }
                let property = bytes[..start]
                    .iter()
                    .rfind(|byte| !byte.is_ascii_whitespace())
                    .is_some_and(|byte| *byte == b'.');
                if !property {
                    spans.push((start, *index));
                }
            }
            _ => *index += 1,
        }
    }
}

fn skip_generated_js_string(bytes: &[u8], index: &mut usize) {
    let quote = bytes[*index];
    *index += 1;
    while *index < bytes.len() {
        match bytes[*index] {
            b'\\' => *index = (*index + 2).min(bytes.len()),
            byte if byte == quote => {
                *index += 1;
                return;
            }
            _ => *index += 1,
        }
    }
}

fn scan_generated_js_template(bytes: &[u8], index: &mut usize, spans: &mut Vec<(usize, usize)>) {
    *index += 1;
    while *index < bytes.len() {
        match bytes[*index] {
            b'\\' => *index = (*index + 2).min(bytes.len()),
            b'`' => {
                *index += 1;
                return;
            }
            b'$' if bytes.get(*index + 1) == Some(&b'{') => {
                *index += 2;
                scan_generated_js(bytes, index, true, spans);
            }
            _ => *index += 1,
        }
    }
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
    string_constants: AHashMap<ValueId, String>,
    binary_operators: AHashMap<ValueId, IrBinaryOp>,
    elidable_i32_coercions: AHashSet<ValueId>,
    parallel_copy_temp: Option<String>,
    live_in_values: Vec<AHashSet<ValueId>>,
    use_counts: AHashMap<ValueId, usize>,
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

fn can_inline_closure(function: &ControlFlowFunction<'_>, inline_structured: bool) -> bool {
    if function.kind != FunctionKind::Closure {
        return false;
    }
    if function.blocks.len() == 1 {
        return function.blocks[0].phis.is_empty()
            && matches!(function.blocks[0].terminator, Some(Terminator::Return(_)))
            && function.blocks[0].instructions.len() <= 8;
    }
    inline_structured
        && can_structure(function)
        && function
            .blocks
            .iter()
            .map(|block| block.instructions.len() + block.phis.len())
            .sum::<usize>()
            <= 80
}

fn strip_redundant_i32_coercion(expression: String) -> String {
    expression
        .strip_suffix("|0)")
        .filter(|value| value.starts_with('('))
        .map_or_else(|| expression.clone(), |value| format!("{value})"))
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

fn immediately_branches_on_phi(
    function: &ControlFlowFunction<'_>,
    block: BlockId,
    value: ValueId,
) -> bool {
    let block = &function.blocks[block.0 as usize];
    block.instructions.is_empty()
        && matches!(block.terminator, Some(Terminator::Branch { condition, .. }) if condition == value)
        && function
            .shapes
            .iter()
            .any(|shape| matches!(shape, ControlShape::If { header, .. } if *header == block.id))
}

impl LocalNames {
    fn new(
        function: &ControlFlowFunction<'_>,
        integer_facts: &FunctionIntegerFacts,
        all_values: bool,
        parent: &Mangler,
        options: &IrJsOptions,
    ) -> Self {
        let mangle_identifiers = options.mangle_identifiers;
        let compact_boolean_literals = options.compact_boolean_literals;
        let scalar_phi_copies = options.scalar_phi_copies;
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
                    let rendered =
                        render_const(value, compact_boolean_literals, StringQuote::Double);
                    let use_count = uses.get(&out).copied().unwrap_or(0);
                    let inline_cost = rendered.len() * use_count;
                    let binding_cost = rendered.len() + 7 + use_count;
                    (inline_cost <= binding_cost).then_some((out, rendered))
                }
                _ => None,
            })
            .collect::<AHashMap<_, _>>();
        let string_constants = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match (instruction.out, &instruction.op) {
                (Some(out), ControlFlowOp::Const(ConstValue::String(value))) => {
                    Some((out, value.to_string()))
                }
                _ => None,
            })
            .collect();
        let binary_operators = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(
                |instruction| match (instruction.out, &instruction.ty, &instruction.op) {
                    (Some(out), Some(_), ControlFlowOp::Binary { op, .. }) => Some((out, *op)),
                    _ => None,
                },
            )
            .collect();
        let cross_block = cross_block_values(function);
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
        let stable_constructor_values = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match &instruction.op {
                ControlFlowOp::Const(_) => instruction.out,
                ControlFlowOp::Closure { captures, .. } if captures.is_empty() => instruction.out,
                _ => None,
            })
            .collect::<AHashSet<_>>();
        for argument in function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match &instruction.op {
                ControlFlowOp::NewClass {
                    constructor: Some(_),
                    args,
                    ..
                } => Some(args),
                _ => None,
            })
            .flatten()
        {
            if !stable_constructor_values.contains(argument)
                && !inlined_values.contains_key(argument)
            {
                stored_values.insert(*argument);
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
            let colors = coalesce_value_names(
                function,
                &stored_values,
                &parameter_values,
                &uses,
                options.phi_affinity_mode,
            );
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
        let parallel_copy_temp = scalar_phi_copies.then(|| mangler.next_name());
        let live_in_values = if scalar_phi_copies {
            live_in_values(function)
        } else {
            Vec::new()
        };
        Self {
            value_names,
            parameter_values,
            stored_values,
            untyped_values,
            inlined_values,
            string_constants,
            binary_operators,
            elidable_i32_coercions: function
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .filter_map(|instruction| instruction.out)
                .filter(|value| integer_facts.can_elide_coercion(*value))
                .collect(),
            parallel_copy_temp,
            live_in_values,
            use_counts: uses,
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

    fn can_elide_i32_coercion(&self, value: ValueId) -> bool {
        self.elidable_i32_coercions.contains(&value)
    }

    fn binary_operator(&self, value: ValueId) -> Option<IrBinaryOp> {
        self.binary_operators.get(&value).copied()
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
    phi_affinity_mode: PhiAffinityMode,
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

    let mut deferred_operands = AHashSet::new();
    for block in &function.blocks {
        for (index, instruction) in block.instructions.iter().enumerate() {
            let Some(out) = instruction.out else {
                continue;
            };
            if uses.get(&out).copied().unwrap_or(0) == 1
                && !named.contains(&out)
                && (op_can_defer(&instruction.op) || can_fuse_value(block, index, out))
            {
                deferred_operands.extend(
                    op_values(&instruction.op)
                        .into_iter()
                        .filter(|value| named.contains(value)),
                );
            }
        }
    }
    let definitions = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| instruction.out.map(|out| (out, &instruction.op)))
        .collect::<AHashMap<_, _>>();
    let mut deferred_sink_pairs = AHashSet::new();
    for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
        let Some(sink) = instruction.out.filter(|out| named.contains(out)) else {
            continue;
        };
        for operand in op_values(&instruction.op) {
            collect_deferred_named_operands(
                operand,
                &named,
                &definitions,
                uses,
                &mut AHashSet::new(),
                &mut deferred_sink_pairs,
                sink,
            );
        }
    }
    for phi in function
        .blocks
        .iter()
        .flat_map(|block| &block.phis)
        .filter(|phi| named.contains(&phi.out))
    {
        for (_, incoming) in &phi.incoming {
            collect_deferred_named_operands(
                *incoming,
                &named,
                &definitions,
                uses,
                &mut AHashSet::new(),
                &mut deferred_sink_pairs,
                phi.out,
            );
        }
    }
    let phi_affinity_pairs = if phi_affinity_mode != PhiAffinityMode::Conservative {
        function
            .blocks
            .iter()
            .flat_map(|block| &block.phis)
            .flat_map(|phi| {
                phi.incoming
                    .iter()
                    .flat_map(|(_, incoming)| [(phi.out, *incoming), (*incoming, phi.out)])
            })
            .collect::<AHashSet<_>>()
    } else {
        AHashSet::new()
    };
    for operand in deferred_operands {
        for value in &named {
            if !deferred_sink_pairs.contains(&(operand, *value))
                && !phi_affinity_pairs.contains(&(operand, *value))
            {
                connect(operand, *value);
            }
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
    if phi_affinity_mode == PhiAffinityMode::Grouped {
        return color_phi_affinity_groups(
            &values,
            &interference,
            &affinities,
            &parameter_order,
            uses,
        );
    }
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

fn color_phi_affinity_groups(
    values: &[ValueId],
    interference: &AHashMap<ValueId, AHashSet<ValueId>>,
    affinities: &AHashMap<ValueId, Vec<ValueId>>,
    parameter_order: &AHashMap<ValueId, usize>,
    uses: &AHashMap<ValueId, usize>,
) -> AHashMap<ValueId, usize> {
    let mut groups = values
        .iter()
        .enumerate()
        .map(|(index, value)| (index, vec![*value]))
        .collect::<AHashMap<_, _>>();
    let mut group_of = values
        .iter()
        .enumerate()
        .map(|(index, value)| (*value, index))
        .collect::<AHashMap<_, _>>();
    let mut edges = affinities
        .iter()
        .flat_map(|(left, rights)| {
            rights.iter().map(|right| {
                if left.0 < right.0 {
                    (*left, *right)
                } else {
                    (*right, *left)
                }
            })
        })
        .collect::<Vec<_>>();
    edges.sort_unstable_by_key(|(left, right)| (left.0, right.0));
    edges.dedup();
    for (left, right) in edges {
        let left_group = group_of[&left];
        let right_group = group_of[&right];
        if left_group == right_group {
            continue;
        }
        let left_members = &groups[&left_group];
        let right_members = &groups[&right_group];
        if left_members.iter().any(|value| {
            right_members
                .iter()
                .any(|other| interference[value].contains(other))
        }) {
            continue;
        }
        let retained = left_group.min(right_group);
        let removed = left_group.max(right_group);
        let mut merged = groups.remove(&retained).expect("affinity group exists");
        merged.extend(groups.remove(&removed).expect("affinity group exists"));
        merged.sort_unstable_by_key(|value| value.0);
        for value in &merged {
            group_of.insert(*value, retained);
        }
        groups.insert(retained, merged);
    }

    let mut group_ids = groups.keys().copied().collect::<Vec<_>>();
    group_ids.sort_unstable_by(|left, right| {
        let left_members = &groups[left];
        let right_members = &groups[right];
        let left_parameter = left_members
            .iter()
            .filter_map(|value| parameter_order.get(value))
            .min()
            .copied();
        let right_parameter = right_members
            .iter()
            .filter_map(|value| parameter_order.get(value))
            .min()
            .copied();
        right_parameter
            .is_some()
            .cmp(&left_parameter.is_some())
            .then_with(|| {
                left_parameter
                    .unwrap_or(usize::MAX)
                    .cmp(&right_parameter.unwrap_or(usize::MAX))
            })
            .then_with(|| {
                right_members
                    .iter()
                    .map(|value| uses.get(value).copied().unwrap_or(0))
                    .sum::<usize>()
                    .cmp(
                        &left_members
                            .iter()
                            .map(|value| uses.get(value).copied().unwrap_or(0))
                            .sum::<usize>(),
                    )
            })
            .then_with(|| left_members[0].0.cmp(&right_members[0].0))
    });
    let mut group_colors = AHashMap::<usize, usize>::new();
    let mut colors = AHashMap::<ValueId, usize>::new();
    for group in group_ids {
        let unavailable = groups[&group]
            .iter()
            .flat_map(|value| &interference[value])
            .filter_map(|neighbor| group_colors.get(&group_of[neighbor]).copied())
            .collect::<AHashSet<_>>();
        let color = (0..)
            .find(|color| !unavailable.contains(color))
            .expect("an interference graph always has another color");
        group_colors.insert(group, color);
        for value in &groups[&group] {
            colors.insert(*value, color);
        }
    }
    colors
}

fn collect_deferred_named_operands(
    value: ValueId,
    named: &AHashSet<ValueId>,
    definitions: &AHashMap<ValueId, &ControlFlowOp<'_>>,
    uses: &AHashMap<ValueId, usize>,
    visited: &mut AHashSet<ValueId>,
    pairs: &mut AHashSet<(ValueId, ValueId)>,
    sink: ValueId,
) {
    if named.contains(&value) {
        pairs.insert((value, sink));
        return;
    }
    if !visited.insert(value) || uses.get(&value).copied() != Some(1) {
        return;
    }
    let Some(op) = definitions.get(&value).filter(|op| op_can_defer(op)) else {
        return;
    };
    for operand in op_values(op) {
        collect_deferred_named_operands(operand, named, definitions, uses, visited, pairs, sink);
    }
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
    block
        .terminator
        .as_ref()
        .is_some_and(|terminator| terminator_values(terminator).contains(&value))
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
        ControlFlowOp::Const(_)
        | ControlFlowOp::LoadLocal(_)
        | ControlFlowOp::LoadGlobal(_)
        | ControlFlowOp::DynamicImport { .. } => Vec::new(),
        ControlFlowOp::Unary { value, .. } | ControlFlowOp::TypeCheck { value, .. } => vec![*value],
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
        ControlFlowOp::HostFieldGet { object, .. } => vec![*object],
        ControlFlowOp::HostFieldSet { object, value, .. } => vec![*object, *value],
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
        ControlFlowOp::HostCall { receiver, args, .. } => {
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
            | ControlFlowOp::TypeCheck { .. }
            | ControlFlowOp::Array(_)
            | ControlFlowOp::Struct { .. }
            | ControlFlowOp::Closure { .. }
            | ControlFlowOp::Template(_)
            | ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::StringLength
                    | Intrinsic::IntImul
                    | Intrinsic::IntToString
                    | Intrinsic::IntToUnsignedString,
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
            | ControlFlowOp::HostFieldSet { .. }
            | ControlFlowOp::IndexSet { .. }
            | ControlFlowOp::NewClass {
                constructor: Some(_),
                ..
            }
            | ControlFlowOp::CallDirect { .. }
            | ControlFlowOp::CallValue { .. }
            | ControlFlowOp::CallMethod { .. }
            | ControlFlowOp::HostCall { .. }
            | ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::Print
                    | Intrinsic::ArrayMap
                    | Intrinsic::ArrayFilter
                    | Intrinsic::ArrayReduce
                    | Intrinsic::ArrayForEach
                    | Intrinsic::ArrayPush
                    | Intrinsic::ArrayPop
                    | Intrinsic::MapSet
                    | Intrinsic::MapDelete
                    | Intrinsic::MapClear
                    | Intrinsic::SetAdd
                    | Intrinsic::SetDelete
                    | Intrinsic::SetClear,
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
            | ControlFlowOp::HostFieldGet { .. }
            | ControlFlowOp::HostFieldSet { .. }
            | ControlFlowOp::IndexSet { .. }
            | ControlFlowOp::CallDirect { .. }
            | ControlFlowOp::CallValue { .. }
            | ControlFlowOp::CallMethod { .. }
            | ControlFlowOp::HostCall { pure: false, .. }
            | ControlFlowOp::NewClass { .. }
            | ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::Print
                    | Intrinsic::ArrayMap
                    | Intrinsic::ArrayFilter
                    | Intrinsic::ArrayReduce
                    | Intrinsic::ArrayForEach
                    | Intrinsic::ArrayPush
                    | Intrinsic::ArrayPop
                    | Intrinsic::MapSet
                    | Intrinsic::MapDelete
                    | Intrinsic::MapClear
                    | Intrinsic::SetAdd
                    | Intrinsic::SetDelete
                    | Intrinsic::SetClear,
                ..
            }
    )
}

fn render_const(value: &ConstValue, compact_boolean_literals: bool, quote: StringQuote) -> String {
    match value {
        ConstValue::Int(value) => shortest_integer(*value),
        ConstValue::Float(value) => shortest_float(*value),
        ConstValue::Bool(true) if compact_boolean_literals => "!0".to_string(),
        ConstValue::Bool(false) if compact_boolean_literals => "!1".to_string(),
        ConstValue::Bool(value) => value.to_string(),
        ConstValue::String(value) => render_string_literal(value, quote),
        ConstValue::Null => "null".to_string(),
    }
}

fn shortest_integer(value: i64) -> String {
    let decimal = value.to_string();
    if value == 0 {
        return decimal;
    }
    let negative = value < 0;
    let digits = decimal.trim_start_matches('-');
    let zeros = digits
        .bytes()
        .rev()
        .take_while(|byte| *byte == b'0')
        .count();
    if zeros == 0 {
        return decimal;
    }
    let exponent = format!(
        "{}{}e{zeros}",
        if negative { "-" } else { "" },
        &digits[..digits.len() - zeros]
    );
    if exponent.len() < decimal.len() {
        exponent
    } else {
        decimal
    }
}

fn shortest_float(value: f64) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    if value == 0.0 {
        return if value.is_sign_negative() { "-0" } else { "0" }.to_string();
    }
    let mut candidates = Vec::with_capacity(3);
    if value.fract() == 0.0 && value >= i64::MIN as f64 && value <= i64::MAX as f64 {
        candidates.push(shortest_integer(value as i64));
    }
    let decimal = value.to_string();
    candidates.push(trim_leading_zero(decimal));
    let scientific = normalize_exponent(format!("{value:e}"));
    candidates.push(trim_leading_zero(scientific));
    candidates
        .into_iter()
        .filter(|candidate| candidate.parse::<f64>().ok() == Some(value))
        .min_by_key(String::len)
        .unwrap_or_else(|| value.to_string())
}

fn trim_leading_zero(value: String) -> String {
    if let Some(rest) = value.strip_prefix("0.") {
        format!(".{rest}")
    } else if let Some(rest) = value.strip_prefix("-0.") {
        format!("-.{rest}")
    } else {
        value
    }
}

fn normalize_exponent(value: String) -> String {
    let Some((mantissa, exponent)) = value.split_once('e') else {
        return value;
    };
    let exponent = exponent
        .strip_prefix('+')
        .unwrap_or(exponent)
        .trim_start_matches('0');
    let exponent = if exponent.is_empty() || exponent == "-" {
        "0"
    } else if let Some(rest) = exponent.strip_prefix("-0") {
        if rest.is_empty() {
            "0"
        } else {
            return format!("{mantissa}e-{rest}");
        }
    } else {
        exponent
    };
    format!("{mantissa}e{exponent}")
}

fn render_js_type_check(
    value: &str,
    target: &Type<'_>,
    quote: StringQuote,
) -> Result<String, CodegenError> {
    Ok(match target {
        Type::Int | Type::Float => {
            format!(
                "typeof({value})=={}",
                render_string_literal("number", quote)
            )
        }
        Type::String => {
            format!(
                "typeof({value})=={}",
                render_string_literal("string", quote)
            )
        }
        Type::Bool => {
            format!(
                "typeof({value})=={}",
                render_string_literal("boolean", quote)
            )
        }
        Type::Array(_) => format!("Array.isArray({value})"),
        Type::Function(_) | Type::GenericFunction(_) => {
            format!(
                "typeof({value})=={}",
                render_string_literal("function", quote)
            )
        }
        _ => {
            return Err(CodegenError::new(
                crate::span::Span::empty(0),
                format!("type `{target}` has no JavaScript type guard"),
            ));
        }
    })
}

fn render_string_literal(value: &str, quote: StringQuote) -> String {
    if quote == StringQuote::Double {
        return format!("\"{value}\"");
    }
    let encoded = format!("\"{value}\"");
    let decoded = serde_json::from_str::<String>(&encoded).unwrap_or_else(|_| value.to_string());
    let mut rendered = String::with_capacity(decoded.len() + 2);
    rendered.push('\'');
    for character in decoded.chars() {
        match character {
            '\'' => rendered.push_str("\\'"),
            '\\' => rendered.push_str("\\\\"),
            '\u{0008}' => rendered.push_str("\\b"),
            '\u{000c}' => rendered.push_str("\\f"),
            '\n' => rendered.push_str("\\n"),
            '\r' => rendered.push_str("\\r"),
            '\t' => rendered.push_str("\\t"),
            '\u{2028}' => rendered.push_str("\\u2028"),
            '\u{2029}' => rendered.push_str("\\u2029"),
            control if control <= '\u{001f}' => {
                write!(rendered, "\\u{:04x}", control as u32)
                    .expect("writing to a string cannot fail");
            }
            _ => rendered.push(character),
        }
    }
    rendered.push('\'');
    rendered
}

fn packed_string_array(
    values: &[ValueId],
    context: &LocalNames,
    quote: StringQuote,
) -> Option<String> {
    if values.is_empty() {
        return None;
    }
    let strings = values
        .iter()
        .map(|value| context.string_constants.get(value).map(String::as_str))
        .collect::<Option<Vec<_>>>()?;
    [",", " ", "|", ";", "~", ":"]
        .into_iter()
        .filter(|delimiter| strings.iter().all(|value| !value.contains(delimiter)))
        .map(|delimiter| {
            format!(
                "{}.split({})",
                render_string_literal(&strings.join(delimiter), quote),
                render_string_literal(delimiter, quote)
            )
        })
        .min_by(|left, right| (left.len(), left).cmp(&(right.len(), right)))
}

fn binary_operator(op: IrBinaryOp) -> &'static str {
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

fn token_safe_binary_rhs(op: IrBinaryOp, rhs: String) -> String {
    if op == IrBinaryOp::Sub && rhs.starts_with('-') {
        format!("({rhs})")
    } else {
        rhs
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BinaryOperandSide {
    Left,
    Right,
}

fn render_binary_operand(
    expression: String,
    child: Option<IrBinaryOp>,
    parent: IrBinaryOp,
    side: BinaryOperandSide,
) -> String {
    if expression.ends_with("|0)") {
        return expression;
    }
    let Some(child) = child else {
        return expression;
    };
    let child_precedence = binary_precedence(child);
    let parent_precedence = binary_precedence(parent);
    let can_unwrap = child_precedence > parent_precedence
        || (child_precedence == parent_precedence
            && match side {
                BinaryOperandSide::Left => true,
                BinaryOperandSide::Right => {
                    child == parent
                        && matches!(
                            parent,
                            IrBinaryOp::BitAnd
                                | IrBinaryOp::BitOr
                                | IrBinaryOp::Xor
                                | IrBinaryOp::And
                                | IrBinaryOp::Or
                        )
                }
            });
    if can_unwrap {
        strip_outer_parens(expression)
    } else {
        expression
    }
}

fn binary_precedence(op: IrBinaryOp) -> u8 {
    match op {
        IrBinaryOp::Or => 1,
        IrBinaryOp::And => 2,
        IrBinaryOp::BitOr => 3,
        IrBinaryOp::Xor => 4,
        IrBinaryOp::BitAnd => 5,
        IrBinaryOp::Eq | IrBinaryOp::NotEq => 6,
        IrBinaryOp::Less | IrBinaryOp::LessEq | IrBinaryOp::Greater | IrBinaryOp::GreaterEq => 7,
        IrBinaryOp::ShiftLeft | IrBinaryOp::ShiftRight | IrBinaryOp::UnsignedShiftRight => 8,
        IrBinaryOp::Add | IrBinaryOp::Sub => 9,
        IrBinaryOp::Mul | IrBinaryOp::Div | IrBinaryOp::Mod => 10,
    }
}

fn default_value(ty: &Type<'_>, compact_boolean_literals: bool) -> &'static str {
    match ty {
        Type::Int | Type::Float => "0",
        Type::Bool if compact_boolean_literals => "!1",
        Type::Bool => "false",
        Type::String => "\"\"",
        Type::Array(_) => "[]",
        Type::Map(_, _) => "new Map",
        Type::Set(_) => "new Set",
        Type::ArrayBuffer => "new ArrayBuffer(0)",
        Type::SharedArrayBuffer => "new SharedArrayBuffer(0)",
        Type::Uint8Array => "new Uint8Array",
        Type::Union(members) => members.first().map_or("null", |member| {
            default_value(member, compact_boolean_literals)
        }),
        Type::Null | Type::Nullable(_) => "null",
        Type::Struct(_)
        | Type::Class(_)
        | Type::StructInstance { .. }
        | Type::ClassInstance { .. }
        | Type::TypeParameter(_)
        | Type::Function(_)
        | Type::GenericFunction(_) => "null",
        Type::Void | Type::Task(_) | Type::ModuleNamespace(_) | Type::ModuleLoadError => "void 0",
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

fn is_true_literal(value: &str) -> bool {
    matches!(value, "true" | "!0")
}

fn is_false_literal(value: &str) -> bool {
    matches!(value, "false" | "!1")
}

fn is_rendered_string_literal(value: &str) -> bool {
    value.len() >= 2
        && matches!(value.as_bytes()[0], b'\'' | b'"')
        && value.as_bytes().last() == Some(&value.as_bytes()[0])
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

#[derive(Debug, Clone)]
struct Mangler {
    next: usize,
    reserved: AHashSet<String>,
    alphabet: IdentifierAlphabet,
}

impl Default for Mangler {
    fn default() -> Self {
        Self::new(IdentifierAlphabet::canonical())
    }
}

impl Mangler {
    fn new(alphabet: IdentifierAlphabet) -> Self {
        Self {
            next: 0,
            reserved: AHashSet::new(),
            alphabet,
        }
    }

    fn reserve(&mut self, name: &str) {
        self.reserved.insert(name.to_string());
    }

    fn next_name(&mut self) -> String {
        loop {
            let name = encode_identifier(self.next, &self.alphabet);
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

fn encode_identifier(mut index: usize, alphabet: &IdentifierAlphabet) -> String {
    let mut output = String::new();
    output.push(alphabet.first[index % alphabet.first.len()] as char);
    index /= alphabet.first.len();
    while index > 0 {
        index -= 1;
        output.push(alphabet.rest[index % alphabet.rest.len()] as char);
        index /= alphabet.rest.len();
    }
    output
}

#[cfg(test)]
mod tests {
    use bumpalo::Bump;

    use super::*;
    use crate::{
        analyze, lower_to_control_flow,
        optimizer::{
            optimize_control_flow, optimize_control_flow_for_module,
            optimize_control_flow_with_options, OptimizationOptions,
        },
        parse_source,
    };

    fn compile(source: &str) -> String {
        compile_with_options(source, IrJsOptions::default())
    }

    fn compile_with_options(source: &str, options: IrJsOptions) -> String {
        let arena = Bump::new();
        let program = parse_source(&arena, source).unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut ir).unwrap();
        emit_optimized_ir_js_with_options(&ir, &options).unwrap()
    }

    fn compile_module(source: &str) -> String {
        compile_module_with_options(source, IrJsOptions::default())
    }

    fn compile_module_with_options(source: &str, options: IrJsOptions) -> String {
        let arena = Bump::new();
        let program = parse_source(&arena, source).unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_for_module(&mut ir).unwrap();
        emit_optimized_ir_js_module_with_options(&ir, &options).unwrap()
    }

    fn compile_without_inlining(source: &str, scalar_replacement: bool) -> String {
        let arena = Bump::new();
        let program = parse_source(&arena, source).unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            scalar_replacement,
            ..OptimizationOptions::default()
        };
        optimize_control_flow_with_options(&mut ir, &options, false).unwrap();
        emit_optimized_ir_js_with_options(&ir, &IrJsOptions::default()).unwrap()
    }

    #[test]
    fn emits_compact_straight_line_ir() {
        assert_eq!(compile("print(1+2*3);"), "console.log(7)");
    }

    #[test]
    fn preserves_nested_short_circuit_grouping_after_name_coalescing() {
        let code = compile_module(
            "int depth=0;bool flushing=false;void set(int nextDepth,bool nextFlushing){depth=nextDepth;flushing=nextFlushing;}bool gated(bool user){return user&&(depth>0||flushing);}export{set,gated};",
        );

        assert!(code.contains("&&("), "{code}");
        assert!(!code.contains("&&depth>0||"), "{code}");
    }

    #[test]
    fn defers_one_use_short_circuit_phis_into_their_branch() {
        let code = compile_module(
            "export void report(int left,int right){if(left==0&&right==0){print(1);}print(2);}",
        );

        assert!(code.contains("&&"), "{code}");
        assert!(!code.contains("var "), "{code}");
        assert!(!code.contains(";if("), "{code}");
    }

    #[test]
    fn removes_only_precedence_safe_binary_parentheses() {
        assert_eq!(
            render_binary_operand(
                "(a-b)".to_string(),
                Some(IrBinaryOp::Sub),
                IrBinaryOp::Add,
                BinaryOperandSide::Left,
            ),
            "a-b"
        );
        assert_eq!(
            render_binary_operand(
                "(b*c)".to_string(),
                Some(IrBinaryOp::Mul),
                IrBinaryOp::Sub,
                BinaryOperandSide::Right,
            ),
            "b*c"
        );
        assert_eq!(
            render_binary_operand(
                "(b-c)".to_string(),
                Some(IrBinaryOp::Sub),
                IrBinaryOp::Add,
                BinaryOperandSide::Right,
            ),
            "(b-c)"
        );
        assert_eq!(
            render_binary_operand(
                "(a+b)".to_string(),
                Some(IrBinaryOp::Add),
                IrBinaryOp::Mul,
                BinaryOperandSide::Left,
            ),
            "(a+b)"
        );
        assert_eq!(
            render_binary_operand(
                "(b&&c)".to_string(),
                Some(IrBinaryOp::And),
                IrBinaryOp::And,
                BinaryOperandSide::Right,
            ),
            "b&&c"
        );
    }

    #[test]
    fn removes_outer_parentheses_inside_expression_delimiters() {
        let code = compile(
            "float sample(float[] values,int index,float input){values[index%3]=input-1.0;return values[index%3]+(input-1.0).abs();}print(sample([1.0,2.0,3.0],2,4.0));",
        );

        assert!(!code.contains("[(("), "{code}");
        assert!(!code.contains("Math.abs(("), "{code}");
    }

    #[test]
    fn coalesces_sequential_mutations_across_direct_phi_edges() {
        let source = "export int refine(int hash,int remaining,int byte){if(remaining==3){hash^=byte<<16;}if(remaining>=2){hash^=byte<<8;}if(remaining>=1){hash^=byte;hash=Math.imul(hash,31);}return hash;}";
        let code = compile_module(source);
        let direct = compile_module_with_options(
            source,
            IrJsOptions {
                phi_affinity_mode: PhiAffinityMode::Direct,
                ..IrJsOptions::default()
            },
        );
        let conservative = compile_module_with_options(
            source,
            IrJsOptions {
                phi_affinity_mode: PhiAffinityMode::Conservative,
                ..IrJsOptions::default()
            },
        );

        assert!(!code.contains("else{"), "{code}");
        assert_ne!(direct, conservative);
        assert!(code.len() <= direct.len(), "{code}\n{direct}");
        assert!(
            direct.len() < conservative.len(),
            "{direct}\n{conservative}"
        );
    }

    #[test]
    fn folds_branches_over_literal_string_captures() {
        let code = compile(
            "func(float)->float choose(string direction){return (float value)=>{if(direction==\"end\"){return value+1.0;}return value-1.0;};}func(float)->float end=choose(\"end\");func(float)->float start=choose(\"start\");float[] values=[1.0,2.0];float total=0.0;for(int i=0;i<values.length;i++){total=total+end(values[i])+start(values[i]);}print(total);",
        );

        assert!(!code.contains("\"end\"==\"end\""), "{code}");
        assert!(!code.contains("\"start\"==\"end\""), "{code}");
    }

    #[test]
    fn packs_literal_string_arrays_when_the_raw_candidate_is_shorter() {
        let source = "extern void consume(string[] values);string[] values=[\"aaaaaa\",\"bbbbbb\",\"cccccc\",\"dddddd\",\"eeeeee\",\"ffffff\",\"gggggg\",\"hhhhhh\"];consume(values);";
        let packed = compile(source);
        let unpacked = compile_with_options(
            source,
            IrJsOptions {
                pack_string_arrays: false,
                ..IrJsOptions::default()
            },
        );

        assert!(packed.contains(".split("), "{packed}");
        assert!(!unpacked.contains(".split("), "{unpacked}");
        assert!(packed.len() < unpacked.len(), "{packed}\n{unpacked}");
    }

    #[test]
    fn coalesces_loop_carried_updates_with_their_header_phi() {
        let code = compile(
            "int state=7;for(int index=0;index<5000;index++){state=Math.imul(state,3)+1;}print(state);",
        );
        assert!(!code.contains(",c;"), "{code}");
    }

    #[test]
    fn coalesces_conditional_loop_updates_with_their_merge_phi() {
        let code = compile(
            "extern bool test(int value);int sum=0;for(int index=0;index<100;index++){if(test(index)){sum+=index;}}print(sum);",
        );

        assert!(!code.contains('?'), "{code}");
    }

    #[test]
    fn emits_cross_chunk_imports_and_live_global_exports() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int base=40;void setBase(int value){base=value;}int read(){return base;}int apply(int value){return read()+value;}export{setBase,read,apply};",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_for_module(&mut ir).unwrap();
        let read = ir
            .functions
            .iter()
            .find(|function| function.name == Some("read"))
            .unwrap()
            .id;
        let chunks = emit_optimized_ir_js_chunks_with_options(
            &ir,
            &IrJsOptions::default(),
            &IrJsChunkPlan {
                entry_file: "entry.js".to_string(),
                chunks: vec![IrJsChunkSpec {
                    file_name: "shared.js".to_string(),
                    functions: vec![read],
                    lazy_module: None,
                }],
            },
        )
        .unwrap();

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].file_name, "entry.js");
        assert_eq!(chunks[1].file_name, "shared.js");
        assert!(chunks[0].code.contains("from\"./shared.js\""));
        assert!(chunks[1].code.contains("from\"./entry.js\""));
        assert!(chunks[0].code.contains(" as apply"));
        assert!(chunks[0].code.contains(" as read"));

        let error = emit_optimized_ir_js_chunks_with_options(
            &ir,
            &IrJsOptions::default(),
            &IrJsChunkPlan {
                entry_file: "entry.js".to_string(),
                chunks: vec![IrJsChunkSpec {
                    file_name: "entry.js".to_string(),
                    functions: vec![read],
                    lazy_module: None,
                }],
            },
        )
        .unwrap_err();
        assert!(error.message.contains("duplicate chunk file name"));
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
        let colors = coalesce_value_names(
            function,
            &named,
            &parameters,
            &use_counts(function),
            PhiAffinityMode::Grouped,
        );

        assert_ne!(colors[&callback], colors[&object]);

        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "class Holder<T>{T value;func(T,T)->bool equals;init(T value,func(T,T)->bool equals){this.value=value;this.equals=equals;}}Holder<T> holder<T>(T value,(func(T,T)->bool)? equals=null){if(equals==null){return new Holder(value,(T previous,T next)=>previous==next);}return new Holder(value,equals);}bool same(int a,int b){return a==b;}Holder<int> result=holder(1,same);print(result.value);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        crate::optimizer::optimize_control_flow(&mut ir).unwrap();
        let integer_analysis = analyze_integer_values(&ir);
        let function = ir
            .functions
            .iter()
            .find(|function| function.name == Some("holder"))
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
        let colors = coalesce_value_names(
            function,
            &named,
            &parameters,
            &use_counts(function),
            PhiAffinityMode::Grouped,
        );
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            if let (
                Some(output),
                ControlFlowOp::NewClass {
                    constructor: Some(_),
                    args,
                    ..
                },
            ) = (instruction.out, &instruction.op)
            {
                for argument in args {
                    assert_ne!(colors[&output], colors[argument]);
                }
            }
        }
        let context = LocalNames::new(
            function,
            integer_analysis.function(function.id),
            false,
            &Mangler::default(),
            &IrJsOptions {
                scalar_phi_copies: false,
                ..IrJsOptions::default()
            },
        );
        let mut checked_unwrap = false;
        let mut checked_captureless_closure = false;
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            let (Some(output), ControlFlowOp::NewClass { args, .. }) =
                (instruction.out, &instruction.op)
            else {
                continue;
            };
            for argument in args {
                let is_unwrap = function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .any(|candidate| {
                        candidate.out == Some(*argument)
                            && matches!(
                                &candidate.op,
                                ControlFlowOp::Intrinsic {
                                    intrinsic: Intrinsic::UnwrapNullable,
                                    ..
                                }
                            )
                    });
                if is_unwrap {
                    checked_unwrap = true;
                    assert!(context.is_stored(*argument));
                    assert_ne!(
                        context.value_name(output).unwrap(),
                        context.value_name(*argument).unwrap()
                    );
                }
                let is_captureless_closure = function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .any(|candidate| {
                        candidate.out == Some(*argument)
                            && matches!(
                                &candidate.op,
                                ControlFlowOp::Closure { captures, .. } if captures.is_empty()
                            )
                    });
                if is_captureless_closure {
                    checked_captureless_closure = true;
                    assert!(!context.is_stored(*argument));
                }
            }
        }
        assert!(checked_unwrap);
        assert!(checked_captureless_closure);
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
        let colors = coalesce_value_names(
            function,
            &named,
            &parameters,
            &use_counts(function),
            PhiAffinityMode::Grouped,
        );

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
            "console.log((b=>b*3|0)(4))"
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
        assert_eq!(
            scalar_parallel_assignments(&swap, Some(("c", true))),
            Some("var c=a;a=b;b=c;".to_string())
        );
        assert_eq!(
            replace_identifier("a+(data.a||\"a\")", "a", "b"),
            "b+(data.a||\"a\")"
        );
        assert!(!expression_references_name("data.a+'a'", "a"));
        assert_eq!(
            scalar_parallel_assignments(
                &[
                    ("a".to_string(), "b".to_string()),
                    ("b".to_string(), "data.a".to_string()),
                ],
                Some(("c", true)),
            ),
            Some("a=b;b=data.a;".to_string())
        );
        assert_eq!(
            scalar_parallel_assignments(
                &[
                    ("a".to_string(), "b".to_string()),
                    ("b".to_string(), "`${a}`".to_string()),
                ],
                Some(("c", true)),
            ),
            Some("var c=a;a=b;b=`${c}`;".to_string())
        );
    }

    #[test]
    fn merges_matching_branch_assignments() {
        assert_eq!(
            merge_conditional_assignments("a=(a+1|0);", "a=(a-1|0);"),
            Some((false, "a", "(a+1|0)", "(a-1|0)", ""))
        );
        assert_eq!(
            merge_conditional_assignments("var a=1;", "a=0;"),
            Some((true, "a", "1", "0", ""))
        );
        assert_eq!(
            merge_conditional_assignments("var a=1,b,c;", "a=0;"),
            Some((true, "a", "1", "0", ",b,c"))
        );
        assert!(merge_conditional_assignments("a=1;", "b=2;").is_none());
    }

    #[test]
    fn renders_distinct_branch_assignments_as_an_expression() {
        assert_eq!(
            conditional_assignment_expression("a=b;", "c=d;"),
            Some(("a", "b", "c", "d"))
        );
        assert!(conditional_assignment_expression("var a=b;", "c=d;").is_none());

        let output = compile(
            "extern int read();int left=0;int right=0;void route(int value){if(value>0){left=value;}else{right=value;}}route(read());print(left+right);",
        );
        assert!(output.contains("?"), "{output}");
        assert!(!output.contains("else"), "{output}");
    }

    #[test]
    fn renders_shortest_exact_numeric_literals() {
        assert_eq!(shortest_integer(120_000), "12e4");
        assert_eq!(shortest_float(0.5), ".5");
        assert_eq!(shortest_float(0.0000001), "1e-7");
        assert_eq!(shortest_float(-0.25), "-.25");
    }

    #[test]
    fn derives_identifier_alphabets_from_emitted_character_frequency() {
        let alphabet = IdentifierAlphabet::for_code("nnnnnnneeeeett");
        assert_eq!(encode_identifier(0, &alphabet), "n");
        assert_eq!(encode_identifier(1, &alphabet), "e");
        assert_eq!(encode_identifier(2, &alphabet), "t");
        assert_eq!(
            IdentifierAlphabet::for_code(""),
            IdentifierAlphabet::canonical()
        );
    }

    #[test]
    fn renders_semantically_equivalent_single_quoted_strings() {
        assert_eq!(
            render_string_literal(r#"say \"hi\" and it's\nready"#, StringQuote::Single),
            r#"'say "hi" and it\'s\nready'"#
        );
    }

    #[test]
    fn emits_compact_boolean_constants_and_typed_defaults() {
        let compact = compile(
            "class Flags{bool enabled;}extern void consumeFlags(Flags value);extern void consume(bool value);Flags flags=new Flags();consumeFlags(flags);consume(true);consume(false);",
        );
        assert!(compact.contains("consumeFlags({enabled:!1})"), "{compact}");
        assert!(compact.contains("consume(!0)"), "{compact}");
        assert!(compact.contains("consume(!1)"), "{compact}");

        let keyword = IrJsOptions {
            compact_boolean_literals: false,
            ..IrJsOptions::default()
        };
        let keyword = compile_with_options(
            "extern void consume(bool value);consume(true);consume(false);",
            keyword,
        );
        assert!(keyword.contains("consume(true)"), "{keyword}");
        assert!(keyword.contains("consume(false)"), "{keyword}");
    }

    #[test]
    fn collapses_boolean_phi_identities_without_dropping_effects() {
        let identities = compile(
            "extern int read();int left=read();int right=read();bool first=left == 1 && true;bool second=right == 2 || false;print(first);print(second);",
        );
        assert_eq!(identities.matches("read()").count(), 2, "{identities}");
        assert!(!identities.contains("||"), "{identities}");
        assert!(!identities.contains("&&"), "{identities}");

        let inversion = compile(
            "extern int read();bool value=false;if(read() == 1){value=false;}else{value=true;}print(value);",
        );
        assert!(inversion.contains("read()!=1"), "{inversion}");
        assert!(!inversion.contains('?'), "{inversion}");
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
    fn fuses_deferred_loop_conditions_into_the_header() {
        let output = compile(
            "extern int[] readValues();int[] values=readValues();int total=0;for(int index=0;index<values.length;index++){total+=values[index];}print(total);",
        );
        assert!(output.contains("for(;"), "{output}");
        assert!(!output.contains("for(;;)"), "{output}");
    }

    #[test]
    fn omits_redundant_integer_remainder_coercions() {
        assert_eq!(
            compile("extern int read();print(read()%7);"),
            "console.log(read()%7)"
        );
        assert!(
            compile("extern int read();print(7%read());").contains("|0"),
            "a runtime zero divisor must still produce LilScript's integer zero"
        );
    }

    #[test]
    fn elides_only_range_proven_integer_coercions() {
        let bounded_add = compile("extern int read();print(read()%10+5);");
        assert!(!bounded_add.contains("|0"), "{bounded_add}");

        let bounded_multiply = compile("extern int read();print((read()%10)*(read()%10));");
        assert!(
            !bounded_multiply.contains("Math.imul"),
            "{bounded_multiply}"
        );
        assert!(!bounded_multiply.contains("|0"), "{bounded_multiply}");

        let overflow_capable = compile("extern int read();print(read()+1);");
        assert!(overflow_capable.contains("|0"), "{overflow_capable}");

        let eager = IrJsOptions {
            elide_safe_integer_coercions: false,
            ..IrJsOptions::default()
        };
        let eager = compile_with_options("extern int read();print(read()%10+5);", eager);
        assert!(eager.contains("|0"), "{eager}");
    }

    #[test]
    fn elides_coercions_from_interprocedural_argument_and_return_ranges() {
        let output = compile_without_inlining(
            "extern int read();int digit(int value){return value%10;}int offset(int value){return value+5;}print(offset(digit(read())));",
            true,
        );

        assert!(output.contains("+5}"), "{output}");
        assert!(!output.contains("+5|0"), "{output}");
    }

    #[test]
    fn uses_owned_field_ranges_but_invalidates_untyped_owners() {
        let owned = compile_without_inlining(
            "struct Box{int value;}extern int read();int increment(Box box){return box.value+1;}Box box=Box{read()%10};print(increment(box));",
            false,
        );
        assert!(owned.contains("+1}"), "{owned}");
        assert!(!owned.contains("+1|0"), "{owned}");

        let exposed = compile_without_inlining(
            "struct Box{int value;}extern int read();extern void mutate(Box box);Box box=Box{read()%10};mutate(box);print(box.value+1);",
            false,
        );
        assert!(exposed.contains("+1|0"), "{exposed}");
    }

    #[test]
    fn never_introduces_math_imul_for_ordinary_multiplication() {
        let small = compile("extern int read();print(read()*3);");
        assert!(small.contains("*3|0"), "{small}");
        assert!(!small.contains("Math.imul"), "{small}");

        let large = compile("extern int read();print(read()*8388608);");
        assert!(large.contains("*8388608|0"), "{large}");
        assert!(!large.contains("Math.imul"), "{large}");
    }

    #[test]
    fn separates_subtraction_from_negative_operands() {
        assert_eq!(
            token_safe_binary_rhs(IrBinaryOp::Sub, "-626380242".to_string()),
            "(-626380242)"
        );
        let output = compile("extern int read();print(read()-(-626380242));");
        assert!(!output.contains("--626380242"), "{output}");
    }

    #[test]
    fn preserves_nested_shift_associativity_after_coercion_elision() {
        let output = compile_with_options(
            "extern int read();int value=read();print(value>>((value%2)>>>18));",
            IrJsOptions {
                elide_safe_integer_coercions: false,
                ..IrJsOptions::default()
            },
        );
        assert!(output.contains(">>(a%2>>>18)"), "{output}");
    }

    #[test]
    fn keeps_integer_coercions_grouped_inside_comparisons() {
        let output = compile("extern int read();print(15>=(read()%0));");
        assert!(output.contains(">=(read()%0|0)"), "{output}");
    }

    #[test]
    fn preserves_explicit_math_imul_calls() {
        let output = compile("extern int read();print(Math.imul(read(),8388608));");
        assert!(output.contains("Math.imul(read(),8388608)"), "{output}");
    }

    #[test]
    fn folds_operator_and_imul_multiplication_with_distinct_semantics() {
        assert_eq!(compile("print(2147483647*2147483647);"), "console.log(0)");
        assert_eq!(
            compile("print(Math.imul(2147483647,2147483647));"),
            "console.log(1)"
        );
    }

    #[test]
    fn emits_simple_branch_bodies_without_braces() {
        let output = compile("extern int read();int value=read();if(value==0){print(1);}print(2);");
        assert!(!output.contains("{console.log"), "{output}");
    }

    #[test]
    fn folds_recursive_guard_returns_into_a_conditional_expression() {
        assert_eq!(
            compile(
                "int factorial(int value){if(value<=1){return 1;}return value*factorial(value-1);}print(factorial(7));"
            ),
            "function a(b){return b<=1?1:b*a(b-1|0)|0}console.log(a(7))"
        );
    }

    #[test]
    fn materializes_values_shared_by_conditional_return_arms() {
        let output = compile_without_inlining(
            "extern int read();int classify(int value){int adjusted=value+1;if(adjusted>100){return adjusted-3;}return adjusted*7+11;}print(classify(read()));",
            false,
        );

        assert!(output.contains("=b+1|0"), "{output}");
        assert!(output.matches("b+1|0").count() == 1, "{output}");
    }

    #[test]
    fn emits_simple_loop_bodies_without_braces() {
        let output = compile(
            "extern int read();int count=read();for(int index=0;index<count;index++){print(index);}",
        );
        assert!(!output.contains("{console.log"), "{output}");
    }

    #[test]
    fn obeys_forced_condition_loop_spelling() {
        let source = "extern bool ready();while(ready()){print(1);}";
        let as_while = compile_with_options(
            source,
            IrJsOptions {
                loop_spelling: LoopSpelling::While,
                ..IrJsOptions::default()
            },
        );
        let as_for = compile_with_options(
            source,
            IrJsOptions {
                loop_spelling: LoopSpelling::For,
                ..IrJsOptions::default()
            },
        );

        assert!(as_while.contains("while(ready())"), "{as_while}");
        assert!(as_for.contains("for(;ready();)"), "{as_for}");
    }

    #[test]
    fn compacts_only_range_proven_one_use_increments() {
        let proven = compile_with_options(
            "int total=0;for(int index=0;index<4;index++){total+=index;}print(total);",
            IrJsOptions {
                mutation_spelling: MutationSpelling::Postfix,
                ..IrJsOptions::default()
            },
        );
        let unknown = compile_with_options(
            "extern int read();int index=read();while(index<read()){index++;}print(index);",
            IrJsOptions {
                mutation_spelling: MutationSpelling::Postfix,
                ..IrJsOptions::default()
            },
        );

        assert!(proven.contains("++"), "{proven}");
        assert!(!unknown.contains("++"), "{unknown}");
        assert!(unknown.contains("+1|0"), "{unknown}");
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
            "int factor=2;int[] values=[1,2];auto mapped=values.map((int value)=>value*factor);print(mapped[0]);",
        );
        assert!(output.contains(".map("));
        assert!(output.contains("*2|0"), "{output}");
        assert!(!output.contains("Math.imul"), "{output}");
    }
}
