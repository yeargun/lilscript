use crate::codegen_ir_js::FunctionSpelling;
use crate::config::{
    CandidateSearch, CompressionCostModel, JavaScriptPriority, ProjectConfig, PublicAggregateAbi,
};
use crate::ir::{ControlFlowModule, ExportBinding, FunctionKind};
use crate::js_syntax_target::EcmaScriptEdition;
use serde::Serialize;

/// The set of consumers whose observations constrain JavaScript lowering.
/// Both variants still optimize the complete statically linked LilScript graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JavaScriptWorld {
    ClosedApplication,
    ReusableLibrary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JavaScriptAbiContract {
    pub preserve_root_exports: bool,
    pub public_aggregate_abi: PublicAggregateAbi,
    pub preserve_extern_fields: bool,
    pub internal_export_bindings_may_mangle: bool,
    pub public_function_spelling: Option<FunctionSpelling>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JavaScriptUnsafeAssumptions {
    pub pristine_builtins: bool,
    pub pure_property_reads: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JavaScriptEffectPolicy {
    pub strip_console: bool,
}

/// Immutable legality input for JavaScript compilation. This is intentionally
/// separate from profitability and search effort.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JavaScriptCompilationContract {
    pub world: JavaScriptWorld,
    pub ecmascript: EcmaScriptEdition,
    pub abi: JavaScriptAbiContract,
    pub assumptions: JavaScriptUnsafeAssumptions,
    pub effects: JavaScriptEffectPolicy,
}

/// The objective can choose among programs admitted by the compilation
/// contract, but cannot alter that contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JavaScriptOptimizationObjective {
    pub transfer: CompressionCostModel,
    pub priority: JavaScriptPriority,
    pub search: CandidateSearch,
    pub candidate_limit: usize,
    pub candidate_byte_budget: usize,
    pub candidate_beam_width: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum JavaScriptExportKind {
    Function,
    Constructor,
    Global,
    TypeOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JavaScriptExportAbi {
    pub name: String,
    pub kind: JavaScriptExportKind,
    pub arity: Option<usize>,
    pub constructible: Option<bool>,
    pub methods: Vec<JavaScriptMethodAbi>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JavaScriptMethodAbi {
    pub name: String,
    pub arity: usize,
    pub is_async: bool,
    pub is_generator: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JavaScriptAbiManifest {
    pub world: &'static str,
    pub exports: Vec<JavaScriptExportAbi>,
    pub public_aggregate_abi: &'static str,
    pub stable_aggregate_fields: Vec<String>,
    pub stable_extern_fields: Vec<String>,
}

impl JavaScriptCompilationContract {
    pub fn abi_manifest(&self, module: &ControlFlowModule<'_>) -> JavaScriptAbiManifest {
        let mut exports = if self.abi.preserve_root_exports {
            module
                .exports
                .iter()
                .map(|export| {
                    let (kind, arity, constructible, methods) = match export.binding {
                        ExportBinding::Function(function) => {
                            let ir_function = module.functions.get(function.0 as usize);
                            let arity = ir_function.map(javascript_function_arity);
                            let class = ir_function.and_then(|function| match function.kind {
                                FunctionKind::Constructor { class } => Some(class),
                                _ => None,
                            });
                            let hierarchy = class
                                .map(|class| javascript_class_hierarchy(module, class))
                                .unwrap_or_default();
                            let mut methods = Vec::new();
                            for owner in hierarchy {
                                for function in &module.functions {
                                    let FunctionKind::Method { class } = function.kind else {
                                        continue;
                                    };
                                    let name = function.name.unwrap_or("method");
                                    if class != owner
                                        || methods
                                            .iter()
                                            .any(|method: &JavaScriptMethodAbi| method.name == name)
                                    {
                                        continue;
                                    }
                                    methods.push(JavaScriptMethodAbi {
                                        name: name.to_string(),
                                        arity: javascript_function_arity(function),
                                        is_async: function.is_async,
                                        is_generator: function.is_generator,
                                    });
                                }
                            }
                            methods.sort_by(|left, right| left.name.cmp(&right.name));
                            (
                                if ir_function.is_some_and(|function| {
                                    matches!(function.kind, FunctionKind::Constructor { .. })
                                }) {
                                    JavaScriptExportKind::Constructor
                                } else {
                                    JavaScriptExportKind::Function
                                },
                                arity,
                                Some(ir_function.is_some_and(|function| {
                                    matches!(function.kind, FunctionKind::Constructor { .. })
                                        || (!function.is_async
                                            && !function.is_generator
                                            && self.abi.public_function_spelling
                                                != Some(FunctionSpelling::Arrow))
                                })),
                                methods,
                            )
                        }
                        ExportBinding::Global(_) => {
                            (JavaScriptExportKind::Global, None, None, Vec::new())
                        }
                        ExportBinding::TypeOnly => {
                            (JavaScriptExportKind::TypeOnly, None, None, Vec::new())
                        }
                    };
                    JavaScriptExportAbi {
                        name: export.name.to_string(),
                        kind,
                        arity,
                        constructible,
                        methods,
                    }
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        exports.sort_by(|left, right| left.name.cmp(&right.name));

        let mut stable_aggregate_fields = if self.abi.preserve_root_exports
            && self.abi.public_aggregate_abi == PublicAggregateAbi::Named
        {
            module
                .structs
                .iter()
                .chain(&module.classes)
                .filter(|layout| !layout.external)
                .flat_map(|layout| layout.fields.iter().map(|field| field.name.to_string()))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        stable_aggregate_fields.sort();
        stable_aggregate_fields.dedup();

        let mut stable_extern_fields = if self.abi.preserve_extern_fields {
            module
                .structs
                .iter()
                .chain(&module.classes)
                .filter(|layout| layout.external)
                .flat_map(|layout| layout.fields.iter().map(|field| field.name.to_string()))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        stable_extern_fields.sort();
        stable_extern_fields.dedup();

        JavaScriptAbiManifest {
            world: match self.world {
                JavaScriptWorld::ClosedApplication => "closed-application",
                JavaScriptWorld::ReusableLibrary => "reusable-library",
            },
            exports,
            public_aggregate_abi: match self.abi.public_aggregate_abi {
                PublicAggregateAbi::Named => "named",
                PublicAggregateAbi::Positional => "positional",
            },
            stable_aggregate_fields,
            stable_extern_fields,
        }
    }
}

fn javascript_function_arity(function: &crate::ir::ControlFlowFunction<'_>) -> usize {
    let skip_receiver = usize::from(matches!(
        function.kind,
        FunctionKind::Constructor { .. } | FunctionKind::Method { .. }
    ));
    let parameters = &function.params[skip_receiver.min(function.params.len())..];
    parameters
        .iter()
        .position(|parameter| parameter.default.is_some())
        .unwrap_or(parameters.len())
}

fn javascript_class_hierarchy<'src>(
    module: &ControlFlowModule<'src>,
    class: &'src str,
) -> Vec<&'src str> {
    let mut hierarchy = Vec::new();
    let mut current = Some(class);
    while let Some(name) = current {
        if hierarchy.contains(&name) {
            break;
        }
        hierarchy.push(name);
        current = module
            .classes
            .iter()
            .find(|layout| layout.name == name)
            .and_then(|layout| layout.base);
    }
    hierarchy
}

impl ProjectConfig {
    pub fn javascript_compilation_contract(
        &self,
        module_output: bool,
    ) -> JavaScriptCompilationContract {
        let options = self.js_options();
        JavaScriptCompilationContract {
            world: if module_output {
                JavaScriptWorld::ReusableLibrary
            } else {
                JavaScriptWorld::ClosedApplication
            },
            ecmascript: self.javascript.resolved_ecmascript(),
            abi: JavaScriptAbiContract {
                preserve_root_exports: module_output,
                public_aggregate_abi: self.javascript.public_aggregate_abi,
                preserve_extern_fields: options.mangle_extern_fields,
                internal_export_bindings_may_mangle: options.mangle_exports,
                public_function_spelling: self.javascript.function_spelling,
            },
            assumptions: JavaScriptUnsafeAssumptions {
                pristine_builtins: self.javascript.assume_pristine_builtins,
                pure_property_reads: self.javascript.assume_pure_property_reads,
            },
            effects: JavaScriptEffectPolicy {
                strip_console: self.javascript.strip_console,
            },
        }
    }

    pub fn javascript_optimization_objective(&self) -> JavaScriptOptimizationObjective {
        JavaScriptOptimizationObjective {
            transfer: self.javascript.cost_model,
            priority: self.javascript.priority,
            search: self.javascript.candidate_search,
            candidate_limit: self.javascript.effective_candidate_limit(),
            candidate_byte_budget: self.javascript.effective_candidate_byte_budget(),
            candidate_beam_width: self.javascript.effective_candidate_beam_width(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{JavaScriptWorld, ProjectConfig};
    use crate::codegen_ir_js::FunctionSpelling;
    use crate::config::{CompressionCostModel, JavaScriptPriority, PublicAggregateAbi};
    use crate::{analyze, lower_to_control_flow, parse_source};
    use bumpalo::Bump;

    #[test]
    fn separates_library_contract_from_optimization_objective() {
        let mut config = ProjectConfig::default();
        config.javascript.cost_model = CompressionCostModel::Raw;
        config.javascript.priority = JavaScriptPriority::Balanced;
        config.javascript.public_aggregate_abi = PublicAggregateAbi::Positional;
        config.javascript.function_spelling = Some(FunctionSpelling::Function);
        config.javascript.assume_pure_property_reads = true;
        config.mangle.exports = Some(true);
        config.mangle.extern_fields = Some(false);

        let contract = config.javascript_compilation_contract(true);
        let objective = config.javascript_optimization_objective();

        assert_eq!(contract.world, JavaScriptWorld::ReusableLibrary);
        assert!(contract.abi.preserve_root_exports);
        assert_eq!(
            contract.abi.public_aggregate_abi,
            PublicAggregateAbi::Positional
        );
        assert_eq!(
            contract.abi.public_function_spelling,
            Some(FunctionSpelling::Function)
        );
        assert!(!contract.abi.preserve_extern_fields);
        assert!(contract.abi.internal_export_bindings_may_mangle);
        assert!(contract.assumptions.pure_property_reads);

        assert_eq!(objective.transfer, CompressionCostModel::Raw);
        assert_eq!(objective.priority, JavaScriptPriority::Balanced);
    }

    #[test]
    fn closed_and_library_worlds_share_optimization_policy() {
        let config = ProjectConfig::default();
        let closed = config.javascript_compilation_contract(false);
        let library = config.javascript_compilation_contract(true);

        assert_eq!(closed.world, JavaScriptWorld::ClosedApplication);
        assert_eq!(library.world, JavaScriptWorld::ReusableLibrary);
        assert!(!closed.abi.preserve_root_exports);
        assert!(library.abi.preserve_root_exports);
        assert_eq!(
            config.javascript_optimization_objective(),
            config.javascript_optimization_objective()
        );
    }

    #[test]
    fn reusable_manifest_records_exported_callable_contract() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "export int add(int left,int right=1){return left+right;}",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let config = ProjectConfig::default();

        let library = config
            .javascript_compilation_contract(true)
            .abi_manifest(&ir);
        assert_eq!(library.world, "reusable-library");
        assert_eq!(library.exports.len(), 1);
        assert_eq!(library.exports[0].name, "add");
        assert_eq!(library.exports[0].arity, Some(1));
        assert_eq!(library.exports[0].constructible, Some(true));
        assert!(library.exports[0].methods.is_empty());

        let closed = config
            .javascript_compilation_contract(false)
            .abi_manifest(&ir);
        assert!(closed.exports.is_empty());
    }

    #[test]
    fn reusable_manifest_distinguishes_constructor_exports() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "export constructor Box;class Box{int value;init(int value){this.value=value;}int read(int offset=0){return this.value+offset;}}",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let manifest = ProjectConfig::default()
            .javascript_compilation_contract(true)
            .abi_manifest(&ir);

        assert_eq!(manifest.exports.len(), 1);
        assert_eq!(
            manifest.exports[0].kind,
            super::JavaScriptExportKind::Constructor
        );
        assert_eq!(manifest.exports[0].arity, Some(1));
        assert_eq!(manifest.exports[0].constructible, Some(true));
        assert_eq!(manifest.exports[0].methods.len(), 1);
        assert_eq!(manifest.exports[0].methods[0].name, "read");
        assert_eq!(manifest.exports[0].methods[0].arity, 0);
    }
}
