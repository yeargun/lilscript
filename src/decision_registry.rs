use crate::codegen_ir_js::{
    ControlFlowSpelling, FunctionLayout, FunctionSpelling, HostAliasSpelling, IrJsOptions,
    LoopSpelling, MutationSpelling, PhiAffinityMode, PureHelperInliningPolicy,
    StateMachineSpelling,
};
use crate::config::{JavaScriptOptimization, ProjectConfig};
use crate::optimizer::OptimizationOptions;

pub const DECISION_REGISTRY_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecisionClass {
    Mandatory,
    Abi,
    ExplicitLowering,
    UnsafePrecondition,
    Incumbent,
    Scored,
    IllegalToFlip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecisionScope {
    Program,
    OptimizerPipeline,
    EmissionPlan,
    TerminalArtifact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkClass {
    Representation,
    PoolingAndLayout,
    TerminalContraction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecisionId {
    ScalarReplacement,
    IdentifierStringPooling,
    StringArrayPacking,
    CanonicalPeephole,
    LengthToNumberElision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecisionSpec {
    pub id: DecisionId,
    pub name: &'static str,
    pub class: DecisionClass,
    pub scope: DecisionScope,
    pub work_class: WorkClass,
}

pub const MIGRATED_DECISIONS: [DecisionSpec; 5] = [
    DecisionSpec {
        id: DecisionId::ScalarReplacement,
        name: "scalar-replacement",
        class: DecisionClass::Scored,
        scope: DecisionScope::OptimizerPipeline,
        work_class: WorkClass::Representation,
    },
    DecisionSpec {
        id: DecisionId::IdentifierStringPooling,
        name: "identifier-string-pooling",
        class: DecisionClass::Scored,
        scope: DecisionScope::EmissionPlan,
        work_class: WorkClass::PoolingAndLayout,
    },
    DecisionSpec {
        id: DecisionId::StringArrayPacking,
        name: "string-array-packing",
        class: DecisionClass::Scored,
        scope: DecisionScope::EmissionPlan,
        work_class: WorkClass::PoolingAndLayout,
    },
    DecisionSpec {
        id: DecisionId::CanonicalPeephole,
        name: "canonical-peephole",
        class: DecisionClass::Scored,
        scope: DecisionScope::TerminalArtifact,
        work_class: WorkClass::TerminalContraction,
    },
    DecisionSpec {
        id: DecisionId::LengthToNumberElision,
        name: "length-to-number-elision",
        class: DecisionClass::Scored,
        scope: DecisionScope::EmissionPlan,
        work_class: WorkClass::PoolingAndLayout,
    },
];

pub fn decision_spec(id: DecisionId) -> &'static DecisionSpec {
    MIGRATED_DECISIONS
        .iter()
        .find(|spec| spec.id == id)
        .expect("every migrated decision has one registry row")
}

pub const fn reversible_boolean_alternatives(configured: bool, enabled: bool) -> [bool; 2] {
    if enabled {
        [configured, !configured]
    } else {
        [configured; 2]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IrJsOptionFieldSpec {
    pub field: &'static str,
    pub class: DecisionClass,
}

pub const IR_JS_OPTION_FIELDS: &[IrJsOptionFieldSpec] = &[
    IrJsOptionFieldSpec {
        field: "mangle_identifiers",
        class: DecisionClass::Incumbent,
    },
    IrJsOptionFieldSpec {
        field: "mangle_properties",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "mangle_exports",
        class: DecisionClass::Abi,
    },
    IrJsOptionFieldSpec {
        field: "mangle_extern_fields",
        class: DecisionClass::Abi,
    },
    IrJsOptionFieldSpec {
        field: "public_aggregate_fields",
        class: DecisionClass::Abi,
    },
    IrJsOptionFieldSpec {
        field: "named_aggregate_fields",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "pool_strings",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "string_pool_minimum_savings",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "pool_identifier_strings",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "pool_numeric_literals",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "ordinary_record_literals",
        class: DecisionClass::IllegalToFlip,
    },
    IrJsOptionFieldSpec {
        field: "elide_safe_integer_coercions",
        class: DecisionClass::Incumbent,
    },
    IrJsOptionFieldSpec {
        field: "elide_safe_string_coercions",
        class: DecisionClass::Incumbent,
    },
    IrJsOptionFieldSpec {
        field: "elide_length_tonumber",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "compact_boolean_literals",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "elide_block_terminal_semicolons",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "elide_new_parentheses",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "elide_call_chain_parentheses",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "inline_structured_closures",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "snapshot_immutable_closure_captures",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "struct_method_shorthand",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "truthy_nullable_checks",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "pack_string_arrays",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "regex_literals",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "unused_catch_binding_elision",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "compact_generator_star",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "inline_single_use_functions",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "inline_exclusive_closures",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "iife_private_callee_clusters",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "nested_once_run_helpers",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "batch_property_assigns",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "batch_property_assign_minimum",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "inline_fresh_empty_array_factories",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "constructor_initializer_fusion",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "pure_helper_inlining",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "dense_string_return_tables",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "host_alias_spelling",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "callee_default_arguments",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "scalar_phi_copies",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "local_name_coalescing",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "phi_affinity_mode",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "control_flow_spelling",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "state_machine_spelling",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "conditional_expressions",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "expression_phi_regions",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "local_phi_expression_regions",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "phi_edge_value_forwarding",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "operand_order_fusion",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "comma_expressions",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "update_loop_layout",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "cross_scope_name_reuse",
        class: DecisionClass::Incumbent,
    },
    IrJsOptionFieldSpec {
        field: "transitive_nested_shadowing",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "precise_cross_scope_shadowing",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "reserved_local_name_prefix",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "local_name_reserve",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "stable_local_names",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "frequency_order_local_names",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "entropy_property_names",
        class: DecisionClass::Incumbent,
    },
    IrJsOptionFieldSpec {
        field: "owner_scoped_property_names",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "function_layout",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "function_layout_exact_limit",
        class: DecisionClass::Incumbent,
    },
    IrJsOptionFieldSpec {
        field: "function_spelling",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "public_function_arrows",
        class: DecisionClass::Abi,
    },
    IrJsOptionFieldSpec {
        field: "loop_spelling",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "mutation_spelling",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "identifier_alphabet",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "string_quote",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "pool_window_roots",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "bare_window_root",
        class: DecisionClass::IllegalToFlip,
    },
    IrJsOptionFieldSpec {
        field: "alias_array_prototype_methods",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "aggregate_operand_order_fusion",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "assume_pristine_builtins",
        class: DecisionClass::UnsafePrecondition,
    },
    IrJsOptionFieldSpec {
        field: "assume_pure_property_reads",
        class: DecisionClass::UnsafePrecondition,
    },
    IrJsOptionFieldSpec {
        field: "sink_entry_function_declarations",
        class: DecisionClass::Incumbent,
    },
    IrJsOptionFieldSpec {
        field: "ecmascript",
        class: DecisionClass::Abi,
    },
    IrJsOptionFieldSpec {
        field: "indexed_char_at",
        class: DecisionClass::Scored,
    },
    IrJsOptionFieldSpec {
        field: "effect_ternary",
        class: DecisionClass::Scored,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmissionPhase {
    BeforeEntropy,
    AfterEntropy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeamAdmission {
    Sequential,
    Priority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeamWidthPolicy {
    Full,
    Narrow,
    Half,
    Min2,
    AtomicHelperTable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalistPolicy {
    Top,
    MutationStratified,
    FreshFactoryEligible,
}

pub struct EmissionSearchContext<'a> {
    pub config: &'a ProjectConfig,
    pub configured: IrJsOptions,
    pub module_output: bool,
    pub candidate_limit: usize,
    pub candidate_beam_width: usize,
    pub narrow_candidate_beam_width: usize,
    pub family_candidate_beam_width: usize,
    pub codec_history_window: usize,
    pub declaration_variant_cap: usize,
}

#[derive(Clone, Copy)]
pub struct CartesianEmissionAxis {
    pub name: &'static str,
    pub expand: fn(&EmissionSearchContext<'_>, IrJsOptions) -> Vec<IrJsOptions>,
}

#[derive(Clone, Copy)]
pub struct ScoredEmissionFamily {
    pub name: &'static str,
    pub phase: EmissionPhase,
    pub admission: BeamAdmission,
    pub width: BeamWidthPolicy,
    pub finalists: FinalistPolicy,
    pub admitted: fn(&EmissionSearchContext<'_>) -> bool,
    pub variants: fn(&EmissionSearchContext<'_>, IrJsOptions) -> Vec<IrJsOptions>,
}

fn unique_with<T: Copy>(
    seed: IrJsOptions,
    values: impl IntoIterator<Item = T>,
    apply: impl Fn(&mut IrJsOptions, T),
) -> Vec<IrJsOptions> {
    let mut out = Vec::new();
    for value in values {
        let mut next = seed;
        apply(&mut next, value);
        if !out.contains(&next) {
            out.push(next);
        }
    }
    out
}

pub fn scalar_phi_copy_candidates(config: &ProjectConfig, configured: bool) -> [bool; 2] {
    if config.js_scalar_phi_copy_variants_enabled() {
        [configured, !configured]
    } else {
        [configured; 2]
    }
}

pub fn phi_affinity_candidates(
    config: &ProjectConfig,
    configured: PhiAffinityMode,
) -> [PhiAffinityMode; 4] {
    if config.js_phi_affinity_variants_enabled() {
        [
            configured,
            PhiAffinityMode::Grouped,
            PhiAffinityMode::Direct,
            PhiAffinityMode::Conservative,
        ]
    } else {
        [configured; 4]
    }
}

pub fn local_name_reserve_variants(options: IrJsOptions) -> [IrJsOptions; 4] {
    [0, 8, 16, 32].map(|local_name_reserve| IrJsOptions {
        local_name_reserve,
        ..options
    })
}

pub fn helper_table_atomic_width(ctx: &EmissionSearchContext<'_>) -> usize {
    let helper_policy_count: usize =
        if !ctx.module_output && ctx.config.pure_helper_inlining_candidates_enabled() {
            3
        } else {
            1
        };
    let table_policy_count: usize = if ctx.config.dense_string_return_table_candidates_enabled() {
        2
    } else {
        1
    };
    let family_size = helper_policy_count.saturating_mul(table_policy_count);
    ctx.candidate_limit
        .saturating_sub(1)
        .checked_div(family_size.saturating_mul(ctx.declaration_variant_cap))
        .unwrap_or(0)
        .min(ctx.candidate_beam_width)
}

fn toggle_function_spelling(spelling: FunctionSpelling) -> FunctionSpelling {
    match spelling {
        FunctionSpelling::Arrow => FunctionSpelling::Function,
        FunctionSpelling::Function => FunctionSpelling::Arrow,
    }
}

pub const CARTESIAN_EMISSION_AXES: &[CartesianEmissionAxis] = &[
    CartesianEmissionAxis {
        name: "ordinary-record-literals",
        expand: |ctx, seed| {
            let ordinary_records_safe = false;
            let alternatives =
                if ordinary_records_safe && ctx.config.js_joint_representation_search_enabled() {
                    [ctx.configured.ordinary_record_literals, true]
                } else {
                    [ctx.configured.ordinary_record_literals; 2]
                };
            unique_with(seed, alternatives, |options, ordinary_record_literals| {
                options.ordinary_record_literals = ordinary_record_literals;
            })
        },
    },
    CartesianEmissionAxis {
        name: "string-pooling",
        expand: |ctx, seed| {
            unique_with(
                seed,
                [ctx.configured.pool_strings, false],
                |options, pool_strings| options.pool_strings = pool_strings,
            )
        },
    },
    CartesianEmissionAxis {
        name: "identifier-string-pooling",
        expand: |ctx, seed| {
            unique_with(
                seed,
                reversible_boolean_alternatives(
                    ctx.configured.pool_identifier_strings,
                    ctx.configured.pool_strings
                        && ctx.config.identifier_string_pooling_candidates_enabled(),
                ),
                |options, pool_identifier_strings| {
                    options.pool_identifier_strings = pool_identifier_strings;
                },
            )
        },
    },
    CartesianEmissionAxis {
        name: "numeric-literal-pooling",
        expand: |ctx, seed| {
            unique_with(
                seed,
                [ctx.configured.pool_numeric_literals, false],
                |options, pool_numeric_literals| {
                    options.pool_numeric_literals = pool_numeric_literals;
                },
            )
        },
    },
    CartesianEmissionAxis {
        name: "compact-boolean-literals",
        expand: |ctx, seed| {
            unique_with(
                seed,
                [ctx.configured.compact_boolean_literals, false],
                |options, compact_boolean_literals| {
                    options.compact_boolean_literals = compact_boolean_literals;
                },
            )
        },
    },
    CartesianEmissionAxis {
        name: "structured-closure-inlining",
        expand: |ctx, seed| {
            unique_with(
                seed,
                [ctx.configured.inline_structured_closures, false],
                |options, inline_structured_closures| {
                    options.inline_structured_closures = inline_structured_closures;
                },
            )
        },
    },
    CartesianEmissionAxis {
        name: "string-array-packing",
        expand: |ctx, seed| {
            unique_with(
                seed,
                reversible_boolean_alternatives(
                    ctx.configured.pack_string_arrays,
                    ctx.config.string_array_packing_candidates_enabled(),
                ),
                |options, pack_string_arrays| options.pack_string_arrays = pack_string_arrays,
            )
        },
    },
    CartesianEmissionAxis {
        name: "regex-literals",
        expand: |ctx, seed| {
            unique_with(
                seed,
                [ctx.configured.regex_literals, false],
                |options, regex_literals| options.regex_literals = regex_literals,
            )
        },
    },
    CartesianEmissionAxis {
        name: "unused-catch-binding-elision",
        expand: |ctx, seed| {
            unique_with(
                seed,
                [ctx.configured.unused_catch_binding_elision, false],
                |options, unused_catch_binding_elision| {
                    options.unused_catch_binding_elision = unused_catch_binding_elision;
                },
            )
        },
    },
    CartesianEmissionAxis {
        name: "compact-generator-star",
        expand: |ctx, seed| {
            unique_with(
                seed,
                [ctx.configured.compact_generator_star, false],
                |options, compact_generator_star| {
                    options.compact_generator_star = compact_generator_star;
                },
            )
        },
    },
    CartesianEmissionAxis {
        name: "scalar-phi-copies",
        expand: |ctx, seed| {
            unique_with(
                seed,
                scalar_phi_copy_candidates(ctx.config, ctx.configured.scalar_phi_copies),
                |options, scalar_phi_copies| options.scalar_phi_copies = scalar_phi_copies,
            )
        },
    },
    CartesianEmissionAxis {
        name: "phi-affinity",
        expand: |ctx, seed| {
            unique_with(
                seed,
                phi_affinity_candidates(ctx.config, ctx.configured.phi_affinity_mode),
                |options, phi_affinity_mode| options.phi_affinity_mode = phi_affinity_mode,
            )
        },
    },
];

pub fn cartesian_emission_seeds(ctx: &EmissionSearchContext<'_>) -> Vec<IrJsOptions> {
    let mut options = vec![ctx.configured];
    for axis in CARTESIAN_EMISSION_AXES {
        let mut expanded = Vec::new();
        for seed in options {
            for candidate in (axis.expand)(ctx, seed) {
                if !expanded.contains(&candidate) {
                    expanded.push(candidate);
                }
            }
        }
        options = expanded;
    }
    options
}

macro_rules! family {
    (
        $name:literal,
        $phase:expr,
        $admission:expr,
        $width:expr,
        $finalists:expr,
        $admitted:expr,
        $variants:expr
    ) => {
        ScoredEmissionFamily {
            name: $name,
            phase: $phase,
            admission: $admission,
            width: $width,
            finalists: $finalists,
            admitted: $admitted,
            variants: $variants,
        }
    };
}

pub const SCORED_EMISSION_FAMILIES: &[ScoredEmissionFamily] = &[
    family!(
        "precise-cross-scope-shadowing",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx.configured.cross_scope_name_reuse,
        |_, options| vec![IrJsOptions {
            precise_cross_scope_shadowing: !options.precise_cross_scope_shadowing,
            ..options
        }]
    ),
    family!(
        "frequency-order-local-names",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx.configured.mangle_identifiers,
        |_, options| vec![IrJsOptions {
            frequency_order_local_names: !options.frequency_order_local_names,
            ..options
        }]
    ),
    family!(
        "stable-local-names",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx.configured.stable_local_names,
        |_, options| vec![IrJsOptions {
            stable_local_names: false,
            ..options
        }]
    ),
    family!(
        "struct-method-shorthand",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |_| true,
        |_, options| vec![IrJsOptions {
            struct_method_shorthand: !options.struct_method_shorthand,
            ..options
        }]
    ),
    family!(
        "length-to-number-elision",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx.config.js_length_to_number_elision_variants_enabled(),
        |_, options| vec![IrJsOptions {
            elide_length_tonumber: !options.elide_length_tonumber,
            ..options
        }]
    ),
    family!(
        "pool-window-roots",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |_| true,
        |_, options| vec![IrJsOptions {
            pool_window_roots: !options.pool_window_roots,
            ..options
        }]
    ),
    family!(
        "alias-array-prototype-methods",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |_| true,
        |_, options| vec![IrJsOptions {
            alias_array_prototype_methods: !options.alias_array_prototype_methods,
            ..options
        }]
    ),
    family!(
        "closure-environment-snapshots",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Priority,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx.config.js_joint_representation_search_enabled(),
        |_, options| vec![IrJsOptions {
            snapshot_immutable_closure_captures: !options.snapshot_immutable_closure_captures,
            ..options
        }]
    ),
    family!(
        "inline-exclusive-closures",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |_| true,
        |_, options| vec![IrJsOptions {
            inline_exclusive_closures: !options.inline_exclusive_closures,
            ..options
        }]
    ),
    family!(
        "iife-private-callee-clusters",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |_| true,
        |_, options| vec![IrJsOptions {
            iife_private_callee_clusters: !options.iife_private_callee_clusters,
            ..options
        }]
    ),
    family!(
        "nested-once-run-helpers",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx.configured.nested_once_run_helpers,
        |_, options| vec![IrJsOptions {
            nested_once_run_helpers: false,
            ..options
        }]
    ),
    family!(
        "string-pool-minimum-savings",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |_| true,
        |_, options| [16, 64, 128, 256, 512]
            .map(|string_pool_minimum_savings| IrJsOptions {
                pool_strings: true,
                string_pool_minimum_savings,
                ..options
            })
            .into()
    ),
    family!(
        "batch-property-assigns",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |_| true,
        |_, options| vec![
            IrJsOptions {
                batch_property_assigns: false,
                ..options
            },
            IrJsOptions {
                batch_property_assigns: true,
                batch_property_assign_minimum: 2,
                ..options
            },
            IrJsOptions {
                batch_property_assigns: true,
                batch_property_assign_minimum: 3,
                ..options
            },
            IrJsOptions {
                batch_property_assigns: true,
                batch_property_assign_minimum: 4,
                ..options
            },
            IrJsOptions {
                batch_property_assigns: true,
                batch_property_assign_minimum: 6,
                ..options
            },
            IrJsOptions {
                batch_property_assigns: true,
                batch_property_assign_minimum: 8,
                ..options
            },
        ]
    ),
    family!(
        "constructor-initializer-fusion",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx.config.javascript_optimization_enabled(
            JavaScriptOptimization::ConstructorInitializerFusionVariants
        ),
        |_, options| vec![IrJsOptions {
            constructor_initializer_fusion: true,
            ..options
        }]
    ),
    family!(
        "host-alias-spelling",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx.config.host_alias_spelling_candidates_enabled(),
        |_, options| vec![IrJsOptions {
            host_alias_spelling: HostAliasSpelling::Direct,
            ..options
        }]
    ),
    family!(
        "indexed-char-at",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx.config.indexed_char_at_candidates_enabled(),
        |_, options| vec![IrJsOptions {
            indexed_char_at: true,
            ..options
        }]
    ),
    family!(
        "effect-ternary",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx.config.effect_ternary_candidates_enabled(),
        |_, options| vec![IrJsOptions {
            effect_ternary: false,
            ..options
        }]
    ),
    family!(
        "elide-call-chain-parentheses",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx.configured.elide_call_chain_parentheses,
        |_, options| vec![IrJsOptions {
            elide_call_chain_parentheses: false,
            ..options
        }]
    ),
    family!(
        "elide-new-parentheses",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx.configured.elide_new_parentheses,
        |_, options| vec![IrJsOptions {
            elide_new_parentheses: false,
            ..options
        }]
    ),
    family!(
        "elide-block-terminal-semicolons",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx.configured.elide_block_terminal_semicolons,
        |_, options| vec![IrJsOptions {
            elide_block_terminal_semicolons: false,
            ..options
        }]
    ),
    family!(
        "function-spelling",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Priority,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx.config.javascript.function_spelling.is_none(),
        |_, options| vec![IrJsOptions {
            function_spelling: toggle_function_spelling(options.function_spelling),
            ..options
        }]
    ),
    family!(
        "function-spelling-stable-local-names",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Priority,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx.config.javascript.function_spelling.is_none()
            && ctx.configured.mangle_identifiers,
        |_, options| vec![IrJsOptions {
            function_spelling: toggle_function_spelling(options.function_spelling),
            stable_local_names: !options.stable_local_names,
            ..options
        }]
    ),
    family!(
        "inline-single-use-functions",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Priority,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx
            .config
            .single_use_function_expression_candidates_enabled(),
        |ctx, options| {
            let mut variants = vec![IrJsOptions {
                inline_single_use_functions: true,
                ..options
            }];
            if ctx.config.javascript.function_spelling.is_none() {
                variants.push(IrJsOptions {
                    inline_single_use_functions: true,
                    function_spelling: toggle_function_spelling(options.function_spelling),
                    ..options
                });
            }
            variants
        }
    ),
    family!(
        "pure-helper-dense-tables",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Priority,
        BeamWidthPolicy::AtomicHelperTable,
        FinalistPolicy::Top,
        |ctx| {
            let pure = !ctx.module_output && ctx.config.pure_helper_inlining_candidates_enabled();
            let dense = ctx.config.dense_string_return_table_candidates_enabled();
            (pure || dense) && helper_table_atomic_width(ctx) != 0
        },
        |ctx, options| {
            let helper_policies =
                if !ctx.module_output && ctx.config.pure_helper_inlining_candidates_enabled() {
                    vec![
                        PureHelperInliningPolicy::None,
                        PureHelperInliningPolicy::SingleStaticUse,
                        PureHelperInliningPolicy::AllEligible,
                    ]
                } else {
                    vec![PureHelperInliningPolicy::None]
                };
            let table_policies = if ctx.config.dense_string_return_table_candidates_enabled() {
                vec![false, true]
            } else {
                vec![false]
            };
            helper_policies
                .into_iter()
                .flat_map(|pure_helper_inlining| {
                    table_policies
                        .iter()
                        .copied()
                        .map(move |dense_string_return_tables| IrJsOptions {
                            pure_helper_inlining,
                            dense_string_return_tables,
                            ..options
                        })
                })
                .collect()
        }
    ),
    family!(
        "callee-default-arguments",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx.config.js_default_argument_variants_enabled(),
        |_, options| vec![IrJsOptions {
            callee_default_arguments: !options.callee_default_arguments,
            ..options
        }]
    ),
    family!(
        "truthy-nullable-checks",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |_| true,
        |_, options| vec![IrJsOptions {
            truthy_nullable_checks: !options.truthy_nullable_checks,
            ..options
        }]
    ),
    family!(
        "conditional-expressions",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx
            .config
            .javascript_optimization_enabled(JavaScriptOptimization::ConditionalExpressionVariants),
        |_, options| vec![IrJsOptions {
            conditional_expressions: !options.conditional_expressions,
            ..options
        }]
    ),
    family!(
        "phi-expression-regions",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| {
            ctx.config.javascript_optimization_enabled(
                JavaScriptOptimization::ExpressionPhiRegionVariants,
            ) || ctx.config.javascript_optimization_enabled(
                JavaScriptOptimization::LocalPhiExpressionRegionVariants,
            )
        },
        |ctx, options| {
            let source = ctx.config.javascript_optimization_enabled(
                JavaScriptOptimization::ExpressionPhiRegionVariants,
            );
            let local = ctx.config.javascript_optimization_enabled(
                JavaScriptOptimization::LocalPhiExpressionRegionVariants,
            );
            match (source, local) {
                (true, true) => vec![
                    IrJsOptions {
                        expression_phi_regions: !options.expression_phi_regions,
                        ..options
                    },
                    IrJsOptions {
                        local_phi_expression_regions: !options.local_phi_expression_regions,
                        ..options
                    },
                    IrJsOptions {
                        expression_phi_regions: !options.expression_phi_regions,
                        local_phi_expression_regions: !options.local_phi_expression_regions,
                        ..options
                    },
                ],
                (true, false) => vec![IrJsOptions {
                    expression_phi_regions: !options.expression_phi_regions,
                    ..options
                }],
                (false, true) => vec![IrJsOptions {
                    local_phi_expression_regions: !options.local_phi_expression_regions,
                    ..options
                }],
                (false, false) => Vec::new(),
            }
        }
    ),
    family!(
        "phi-edge-value-forwarding",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx.config.javascript_optimization_enabled(
            JavaScriptOptimization::PhiEdgeValueForwardingVariants
        ),
        |_, options| vec![IrJsOptions {
            phi_edge_value_forwarding: !options.phi_edge_value_forwarding,
            ..options
        }]
    ),
    family!(
        "operand-order-fusion",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx.configured.operand_order_fusion
            && ctx.config.javascript_optimization_enabled(
                JavaScriptOptimization::OperandOrderFusionVariants
            ),
        |_, options| vec![IrJsOptions {
            operand_order_fusion: false,
            ..options
        }]
    ),
    family!(
        "aggregate-operand-order-fusion",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx.configured.operand_order_fusion,
        |_, options| options
            .operand_order_fusion
            .then_some(IrJsOptions {
                aggregate_operand_order_fusion: !options.aggregate_operand_order_fusion,
                ..options
            })
            .into_iter()
            .collect()
    ),
    family!(
        "comma-expressions",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx
            .config
            .javascript_optimization_enabled(JavaScriptOptimization::CommaExpressionVariants),
        |_, options| vec![IrJsOptions {
            comma_expressions: !options.comma_expressions,
            ..options
        }]
    ),
    family!(
        "structural-control-flow",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx
            .config
            .javascript_optimization_enabled(JavaScriptOptimization::StructuralControlFlowVariants),
        |_, options| vec![
            IrJsOptions {
                control_flow_spelling: ControlFlowSpelling::Structured,
                ..options
            },
            IrJsOptions {
                control_flow_spelling: ControlFlowSpelling::StateMachine,
                ..options
            },
        ]
    ),
    family!(
        "loop-spelling",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Narrow,
        FinalistPolicy::Top,
        |ctx| ctx.config.loop_spelling_selection_enabled(),
        |_, options| vec![
            IrJsOptions {
                loop_spelling: LoopSpelling::While,
                ..options
            },
            IrJsOptions {
                loop_spelling: LoopSpelling::For,
                ..options
            },
        ]
    ),
    family!(
        "do-loop",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx
            .config
            .javascript_optimization_enabled(JavaScriptOptimization::DoLoopVariants),
        |_, options| vec![IrJsOptions {
            loop_spelling: LoopSpelling::Do,
            update_loop_layout: false,
            ..options
        }]
    ),
    family!(
        "update-loop-layout",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx
            .config
            .javascript_optimization_enabled(JavaScriptOptimization::UpdateLoopVariants),
        |_, options| vec![IrJsOptions {
            update_loop_layout: !options.update_loop_layout,
            ..options
        }]
    ),
    family!(
        "mutation-spelling",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::MutationStratified,
        |ctx| ctx.config.mutation_spelling_selection_enabled(),
        |_, options| vec![
            IrJsOptions {
                mutation_spelling: MutationSpelling::Prefix,
                ..options
            },
            IrJsOptions {
                mutation_spelling: MutationSpelling::Postfix,
                ..options
            },
            IrJsOptions {
                mutation_spelling: MutationSpelling::Compound,
                ..options
            },
        ]
    ),
    family!(
        "switch-lowering",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx
            .config
            .javascript_optimization_enabled(JavaScriptOptimization::SwitchLoweringVariants),
        |_, options| vec![IrJsOptions {
            control_flow_spelling: ControlFlowSpelling::StateMachine,
            state_machine_spelling: StateMachineSpelling::Conditional,
            ..options
        }]
    ),
    family!(
        "local-name-reserve",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Priority,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx.configured.mangle_identifiers && ctx.configured.cross_scope_name_reuse,
        |_, options| local_name_reserve_variants(options).into()
    ),
    family!(
        "function-layout",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Priority,
        BeamWidthPolicy::Half,
        FinalistPolicy::Top,
        |ctx| ctx
            .config
            .javascript_optimization_enabled(JavaScriptOptimization::FunctionLayoutVariants),
        |ctx, options| vec![
            IrJsOptions {
                function_layout: FunctionLayout::CompressionSimilarity,
                ..options
            },
            IrJsOptions {
                function_layout: FunctionLayout::CompressionWindow(ctx.codec_history_window),
                ..options
            },
        ]
    ),
    family!(
        "named-aggregate-layout",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Priority,
        BeamWidthPolicy::Half,
        FinalistPolicy::Top,
        |ctx| ctx.config.js_joint_representation_search_enabled(),
        |_, options| vec![
            IrJsOptions {
                named_aggregate_fields: false,
                public_aggregate_fields: true,
                ..options
            },
            IrJsOptions {
                named_aggregate_fields: true,
                public_aggregate_fields: true,
                ..options
            },
        ]
    ),
    family!(
        "owner-scoped-property-names",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Priority,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx.config.js_joint_representation_search_enabled()
            && ctx.configured.mangle_properties
            && ctx.configured.mangle_extern_fields,
        |_, options| vec![IrJsOptions {
            owner_scoped_property_names: !options.owner_scoped_property_names,
            ..options
        }]
    ),
    family!(
        "property-mangling",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Sequential,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx.configured.mangle_properties,
        |_, options| vec![IrJsOptions {
            mangle_properties: !options.mangle_properties,
            ..options
        }]
    ),
    family!(
        "stable-local-names-late",
        EmissionPhase::BeforeEntropy,
        BeamAdmission::Priority,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx.configured.stable_local_names,
        |_, options| vec![IrJsOptions {
            stable_local_names: false,
            ..options
        }]
    ),
    family!(
        "fresh-literal-factory",
        EmissionPhase::AfterEntropy,
        BeamAdmission::Priority,
        BeamWidthPolicy::Min2,
        FinalistPolicy::FreshFactoryEligible,
        |ctx| ctx.config.javascript_optimization_enabled(
            JavaScriptOptimization::FreshLiteralFactoryInliningVariants
        ),
        |_, options| vec![IrJsOptions {
            inline_fresh_empty_array_factories: true,
            ..options
        }]
    ),
    family!(
        "local-name-coalescing",
        EmissionPhase::AfterEntropy,
        BeamAdmission::Priority,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx.configured.mangle_identifiers
            && ctx.config.js_local_name_coalescing_variants_enabled(),
        |_, options| vec![IrJsOptions {
            local_name_coalescing: !options.local_name_coalescing,
            ..options
        }]
    ),
    family!(
        "transitive-nested-shadowing",
        EmissionPhase::AfterEntropy,
        BeamAdmission::Priority,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx.configured.cross_scope_name_reuse,
        |_, options| vec![IrJsOptions {
            transitive_nested_shadowing: !options.transitive_nested_shadowing,
            ..options
        }]
    ),
    family!(
        "precise-cross-scope-shadowing-late",
        EmissionPhase::AfterEntropy,
        BeamAdmission::Priority,
        BeamWidthPolicy::Full,
        FinalistPolicy::Top,
        |ctx| ctx.configured.cross_scope_name_reuse,
        |_, options| vec![IrJsOptions {
            precise_cross_scope_shadowing: !options.precise_cross_scope_shadowing,
            ..options
        }]
    ),
];

pub fn admitted_scored_emission_family_names(ctx: &EmissionSearchContext<'_>) -> Vec<&'static str> {
    SCORED_EMISSION_FAMILIES
        .iter()
        .filter(|family| (family.admitted)(ctx))
        .map(|family| family.name)
        .collect()
}

pub fn branching_cartesian_axis_names(ctx: &EmissionSearchContext<'_>) -> Vec<&'static str> {
    CARTESIAN_EMISSION_AXES
        .iter()
        .filter(|axis| (axis.expand)(ctx, ctx.configured).len() > 1)
        .map(|axis| axis.name)
        .collect()
}

#[derive(Clone, Copy)]
pub struct ScoredIrVariant {
    pub name: &'static str,
    pub admitted: fn(&ProjectConfig, OptimizationOptions) -> bool,
    pub apply: fn(&ProjectConfig, OptimizationOptions) -> OptimizationOptions,
}

pub const SCORED_IR_VARIANTS: &[ScoredIrVariant] = &[
    ScoredIrVariant {
        name: "closure-factory-outlining",
        admitted: |config, configured| {
            configured.inlining
                && configured.inline_closure_factories
                && config.ir_closure_factory_variants_enabled()
        },
        apply: |_, configured| OptimizationOptions {
            inline_closure_factories: false,
            ..configured
        },
    },
    ScoredIrVariant {
        name: "ir-inlining-off",
        admitted: |config, configured| configured.inlining && config.ir_inlining_variants_enabled(),
        apply: |_, configured| OptimizationOptions {
            inlining: false,
            inline_instruction_limit: 0,
            inline_control_flow_limit: 0,
            inline_growth_limit: Some(0),
            ..configured
        },
    },
    ScoredIrVariant {
        name: "exported-internal-inlining",
        admitted: |config, configured| {
            configured.inlining && config.exported_internal_inlining_variants_enabled()
        },
        apply: |_, configured| OptimizationOptions {
            inline_exported_internal_calls: true,
            ..configured
        },
    },
    ScoredIrVariant {
        name: "global-alias-forwarding",
        admitted: |config, configured| {
            configured.global_optimization
                && !configured.forward_global_aliases
                && config.global_alias_forwarding_variants_enabled()
        },
        apply: |_, configured| OptimizationOptions {
            forward_global_aliases: true,
            ..configured
        },
    },
    ScoredIrVariant {
        name: "keep-object",
        admitted: |config, configured| {
            configured.scalar_replacement && config.js_keep_object_variants_enabled()
        },
        apply: |_, configured| OptimizationOptions {
            scalar_replacement: false,
            ..configured
        },
    },
    ScoredIrVariant {
        name: "ir-specialization-off",
        admitted: |config, configured| {
            configured.constant_parameter_specialization
                && config.javascript_optimization_enabled(
                    JavaScriptOptimization::IrSpecializationVariants,
                )
        },
        apply: |_, configured| OptimizationOptions {
            constant_parameter_specialization: false,
            ..configured
        },
    },
    ScoredIrVariant {
        name: "call-site-specialization-off",
        admitted: |config, configured| {
            configured.call_site_specialization
                && config
                    .javascript_optimization_enabled(JavaScriptOptimization::CallSiteSpecialization)
        },
        apply: |_, configured| OptimizationOptions {
            call_site_specialization: false,
            ..configured
        },
    },
    ScoredIrVariant {
        name: "call-graph-reusable-helpers",
        admitted: |config, configured| {
            configured.inlining
                && configured.constant_parameter_specialization
                && config.ir_inlining_variants_enabled()
                && config.javascript_optimization_enabled(
                    JavaScriptOptimization::IrSpecializationVariants,
                )
        },
        apply: |config, configured| {
            let mut reusable_helpers = OptimizationOptions {
                inlining: false,
                inline_instruction_limit: 0,
                inline_control_flow_limit: 0,
                inline_growth_limit: Some(0),
                constant_parameter_specialization: false,
                ..configured
            };
            if configured.call_site_specialization
                && config
                    .javascript_optimization_enabled(JavaScriptOptimization::CallSiteSpecialization)
            {
                reusable_helpers.call_site_specialization = false;
            }
            reusable_helpers
        },
    },
    ScoredIrVariant {
        name: "capture-signature-cloning-off",
        admitted: |config, configured| {
            configured.capture_signature_cloning
                && config.javascript_optimization_enabled(
                    JavaScriptOptimization::CaptureSignatureCloning,
                )
        },
        apply: |_, configured| OptimizationOptions {
            capture_signature_cloning: false,
            ..configured
        },
    },
    ScoredIrVariant {
        name: "function-subsumption-on",
        admitted: |config, _| config.js_function_subsumption_variants_enabled(),
        apply: |_, configured| OptimizationOptions {
            function_subsumption: true,
            ..configured
        },
    },
    ScoredIrVariant {
        name: "function-subsumption-off",
        admitted: |config, _| config.js_function_subsumption_variants_enabled(),
        apply: |_, configured| OptimizationOptions {
            function_subsumption: false,
            ..configured
        },
    },
];

pub fn scored_ir_optimizer_clones(
    config: &ProjectConfig,
    configured: OptimizationOptions,
) -> Vec<OptimizationOptions> {
    let mut options = vec![configured];
    for variant in SCORED_IR_VARIANTS {
        if !(variant.admitted)(config, configured) {
            continue;
        }
        let candidate = (variant.apply)(config, configured);
        if !options.contains(&candidate) {
            options.push(candidate);
        }
    }
    options
}

pub fn scored_ir_phase_ordering_clones(
    config: &ProjectConfig,
    configured: OptimizationOptions,
    broad_module: bool,
) -> Vec<OptimizationOptions> {
    if !configured.inlining || !config.ir_phase_ordering_variants_enabled() {
        return Vec::new();
    }
    let mut bases = vec![configured];
    if configured.constant_parameter_specialization {
        let mut without_constant_specialization = configured;
        without_constant_specialization.constant_parameter_specialization = false;
        if broad_module {
            bases.clear();
        }
        bases.push(without_constant_specialization);
    }
    let mut variants = Vec::new();
    for base in bases {
        if broad_module {
            let mut combined = base;
            combined.common_subexpression_elimination = false;
            combined.inline_instruction_limit = combined.inline_instruction_limit.max(48);
            combined.inline_control_flow_limit = combined.inline_control_flow_limit.max(128);
            combined.inline_growth_limit = Some(combined.inline_growth_limit.unwrap_or(0).max(40));
            if !variants.contains(&combined) {
                variants.push(combined);
            }
            continue;
        }

        let mut without_early_cse = base;
        without_early_cse.common_subexpression_elimination = false;
        if !variants.contains(&without_early_cse) {
            variants.push(without_early_cse);
        }

        let mut aggressive_inlining = base;
        aggressive_inlining.inline_instruction_limit =
            aggressive_inlining.inline_instruction_limit.max(48);
        aggressive_inlining.inline_control_flow_limit =
            aggressive_inlining.inline_control_flow_limit.max(128);
        aggressive_inlining.inline_growth_limit =
            Some(aggressive_inlining.inline_growth_limit.unwrap_or(0).max(40));
        if !variants.contains(&aggressive_inlining) {
            variants.push(aggressive_inlining);
        }

        aggressive_inlining.common_subexpression_elimination = false;
        if !variants.contains(&aggressive_inlining) {
            variants.push(aggressive_inlining);
        }
    }
    variants
}

pub fn admitted_scored_ir_variant_names(
    config: &ProjectConfig,
    configured: OptimizationOptions,
) -> Vec<&'static str> {
    SCORED_IR_VARIANTS
        .iter()
        .filter(|variant| (variant.admitted)(config, configured))
        .map(|variant| variant.name)
        .collect()
}

pub fn priority_scored_family_count(ctx: &EmissionSearchContext<'_>) -> usize {
    SCORED_EMISSION_FAMILIES
        .iter()
        .filter(|family| family.admission == BeamAdmission::Priority && (family.admitted)(ctx))
        .count()
}

pub fn ir_js_option_field_class(field: &str) -> Option<DecisionClass> {
    IR_JS_OPTION_FIELDS
        .iter()
        .find(|spec| spec.field == field)
        .map(|spec| spec.class)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{
        admitted_scored_emission_family_names, admitted_scored_ir_variant_names,
        cartesian_emission_seeds, decision_spec, reversible_boolean_alternatives,
        scored_ir_optimizer_clones, scored_ir_phase_ordering_clones, DecisionClass, DecisionId,
        EmissionSearchContext, IR_JS_OPTION_FIELDS, MIGRATED_DECISIONS, SCORED_EMISSION_FAMILIES,
        SCORED_IR_VARIANTS,
    };
    use crate::config::{CompressionCostModel, ProjectConfig};
    use crate::optimizer::OptimizationOptions;

    fn search_context(config: &ProjectConfig, module_output: bool) -> EmissionSearchContext<'_> {
        let candidate_beam_width = config.javascript.effective_candidate_beam_width();
        EmissionSearchContext {
            config,
            configured: config.js_options(),
            module_output,
            candidate_limit: config.javascript.effective_candidate_limit(),
            candidate_beam_width,
            narrow_candidate_beam_width: candidate_beam_width.saturating_mul(2).div_ceil(3),
            family_candidate_beam_width: candidate_beam_width.div_ceil(3),
            codec_history_window: match config.javascript.cost_model {
                CompressionCostModel::Raw | CompressionCostModel::Gzip => 32 * 1024,
                CompressionCostModel::Brotli => 1 << 22,
            },
            declaration_variant_cap: 4,
        }
    }

    #[test]
    fn phase_ordering_recipes_preserve_small_and_broad_variants() {
        let mut config = ProjectConfig::default();
        config.javascript.optimization_level = 15;
        config.javascript.candidate_search = crate::config::CandidateSearch::Always;
        let configured = config.js_optimizer_options();

        let small = scored_ir_phase_ordering_clones(&config, configured, false);
        assert!(small.iter().any(|options| {
            !options.common_subexpression_elimination
                && options.inline_instruction_limit == configured.inline_instruction_limit
        }));
        assert!(small.iter().any(|options| {
            options.inline_instruction_limit >= 48
                && options.inline_control_flow_limit >= 128
                && options.common_subexpression_elimination
        }));

        let broad = scored_ir_phase_ordering_clones(&config, configured, true);
        assert_eq!(broad.len(), 1);
        assert!(!broad[0].common_subexpression_elimination);
        assert!(!broad[0].constant_parameter_specialization);
        assert!(broad[0].inline_instruction_limit >= 48);
    }

    #[test]
    fn migrated_decision_names_and_ids_are_unique() {
        assert_eq!(
            MIGRATED_DECISIONS
                .iter()
                .map(|spec| spec.id)
                .collect::<HashSet<_>>()
                .len(),
            MIGRATED_DECISIONS.len()
        );
        assert_eq!(
            MIGRATED_DECISIONS
                .iter()
                .map(|spec| spec.name)
                .collect::<HashSet<_>>()
                .len(),
            MIGRATED_DECISIONS.len()
        );
        assert_eq!(
            decision_spec(DecisionId::ScalarReplacement).class,
            DecisionClass::Scored
        );
    }

    #[test]
    fn reversible_boolean_family_keeps_the_incumbent_first() {
        assert_eq!(reversible_boolean_alternatives(false, true), [false, true]);
        assert_eq!(reversible_boolean_alternatives(true, true), [true, false]);
        assert_eq!(
            reversible_boolean_alternatives(false, false),
            [false, false]
        );
        assert_eq!(reversible_boolean_alternatives(true, false), [true, true]);
    }

    #[test]
    fn every_ir_js_options_field_is_classified_once() {
        assert_eq!(IR_JS_OPTION_FIELDS.len(), 77);
        assert_eq!(
            IR_JS_OPTION_FIELDS
                .iter()
                .map(|spec| spec.field)
                .collect::<HashSet<_>>()
                .len(),
            IR_JS_OPTION_FIELDS.len()
        );
        assert_eq!(
            ir_js_option_field_class("bare_window_root"),
            Some(DecisionClass::IllegalToFlip)
        );
        assert_eq!(
            ir_js_option_field_class("assume_pure_property_reads"),
            Some(DecisionClass::UnsafePrecondition)
        );
        assert_eq!(
            ir_js_option_field_class("assume_pristine_builtins"),
            Some(DecisionClass::UnsafePrecondition)
        );
        assert_eq!(
            ir_js_option_field_class("public_aggregate_fields"),
            Some(DecisionClass::Abi)
        );
        assert_eq!(
            ir_js_option_field_class("mangle_extern_fields"),
            Some(DecisionClass::Abi)
        );
        assert_eq!(
            ir_js_option_field_class("ecmascript"),
            Some(DecisionClass::Abi)
        );
    }

    fn ir_js_option_field_class(field: &str) -> Option<DecisionClass> {
        super::ir_js_option_field_class(field)
    }

    #[test]
    fn scored_emission_families_are_named_uniquely_and_skip_illegal_axes() {
        assert_eq!(SCORED_EMISSION_FAMILIES.len(), 48);
        assert_eq!(
            SCORED_EMISSION_FAMILIES
                .iter()
                .map(|family| family.name)
                .collect::<HashSet<_>>()
                .len(),
            SCORED_EMISSION_FAMILIES.len()
        );
        let names: HashSet<_> = SCORED_EMISSION_FAMILIES
            .iter()
            .map(|family| family.name)
            .collect();
        assert!(!names.contains("bare-window-root"));
        assert!(!names.contains("assume-pure-property-reads"));
        assert!(!names.contains("assume-pristine-builtins"));
        assert!(!names.contains("ecmascript"));
        assert!(!names.contains("mangle-extern-fields"));
        assert!(!names.contains("mangle-exports"));
        assert!(names.contains("named-aggregate-layout"));
        assert!(names.contains("length-to-number-elision"));
        assert!(names.contains("function-spelling-stable-local-names"));
        assert!(names.contains("closure-environment-snapshots"));
        assert!(names.contains("owner-scoped-property-names"));
        for protected in [
            "local-name-reserve",
            "function-layout",
            "named-aggregate-layout",
            "stable-local-names-late",
        ] {
            assert_eq!(
                SCORED_EMISSION_FAMILIES
                    .iter()
                    .find(|family| family.name == protected)
                    .map(|family| family.admission),
                Some(super::BeamAdmission::Priority),
                "{protected} must retain a reserved proposal slice"
            );
        }
    }

    #[test]
    fn omitting_length_to_number_elision_does_not_admit_that_family() {
        let config: ProjectConfig = toml::from_str(
            "[javascript]\npriority='size-first'\ncompression=['identifier-mangling']\n",
        )
        .unwrap();
        let ctx = search_context(&config, false);
        let names = admitted_scored_emission_family_names(&ctx);
        assert!(!names.contains(&"length-to-number-elision"), "{names:?}");
        assert!(!names.contains(&"named-aggregate-layout"), "{names:?}");
        let configured = ctx.configured;
        for family in SCORED_EMISSION_FAMILIES {
            if !(family.admitted)(&ctx) {
                continue;
            }
            for candidate in (family.variants)(&ctx, configured) {
                assert_eq!(candidate.bare_window_root, configured.bare_window_root);
                assert_eq!(
                    candidate.assume_pure_property_reads,
                    configured.assume_pure_property_reads
                );
                assert_eq!(candidate.ecmascript, configured.ecmascript);
                assert_eq!(
                    candidate.mangle_extern_fields,
                    configured.mangle_extern_fields
                );
                assert_eq!(candidate.mangle_exports, configured.mangle_exports);
                assert_eq!(
                    candidate.public_function_arrows,
                    configured.public_function_arrows
                );
            }
        }
    }

    #[test]
    fn cartesian_seed_keeps_the_configured_incumbent() {
        let config = ProjectConfig::default();
        let ctx = search_context(&config, false);
        let seeds = cartesian_emission_seeds(&ctx);
        assert!(seeds.contains(&ctx.configured), "{}", seeds.len());
        assert!(seeds.iter().all(|seed| !seed.bare_window_root));
        assert!(seeds
            .iter()
            .all(|seed| seed.ecmascript == ctx.configured.ecmascript));
        assert!(seeds.iter().all(
            |seed| seed.assume_pure_property_reads == ctx.configured.assume_pure_property_reads
        ));
        let axes = super::branching_cartesian_axis_names(&ctx);
        assert!(axes.contains(&"string-array-packing"), "{axes:?}");
        assert!(axes.contains(&"identifier-string-pooling"), "{axes:?}");
        assert!(!axes.contains(&"ordinary-record-literals"), "{axes:?}");
        assert!(
            seeds.iter().any(|seed| seed.pack_string_arrays)
                && seeds.iter().any(|seed| !seed.pack_string_arrays),
            "size-first Brotli must still try packing"
        );
        assert!(
            seeds.iter().any(|seed| seed.pool_identifier_strings)
                && seeds.iter().any(|seed| !seed.pool_identifier_strings),
            "size-first Brotli must still try identifier-string pooling"
        );
    }

    #[test]
    fn scored_ir_variants_are_named_uniquely_and_keep_object_is_legal_on_size_first() {
        assert_eq!(
            SCORED_IR_VARIANTS
                .iter()
                .map(|variant| variant.name)
                .collect::<HashSet<_>>()
                .len(),
            SCORED_IR_VARIANTS.len()
        );
        let size = ProjectConfig::default();
        let names = admitted_scored_ir_variant_names(&size, size.js_optimizer_options());
        assert!(names.contains(&"keep-object"), "{names:?}");
        assert!(names.contains(&"ir-inlining-off"), "{names:?}");
        assert!(names.contains(&"call-graph-reusable-helpers"), "{names:?}");
        let clones = scored_ir_optimizer_clones(&size, size.js_optimizer_options());
        assert!(clones.iter().any(|options| !options.scalar_replacement));
        assert!(clones.contains(&size.js_optimizer_options()));

        let exact: ProjectConfig = toml::from_str(
            "[javascript]\npriority='size-first'\ncompression=['identifier-mangling']\n",
        )
        .unwrap();
        let exact_names = admitted_scored_ir_variant_names(&exact, exact.js_optimizer_options());
        assert!(!exact_names.contains(&"keep-object"), "{exact_names:?}");
    }

    #[test]
    fn exported_internal_inlining_is_a_distinct_opt_in_ir_clone() {
        assert!(!OptimizationOptions::default().inline_exported_internal_calls);
        assert!(!OptimizationOptions::disabled().inline_exported_internal_calls);

        let size = ProjectConfig::default();
        let configured = size.js_optimizer_options();
        assert!(!configured.inline_exported_internal_calls);
        let names = admitted_scored_ir_variant_names(&size, configured);
        assert!(names.contains(&"exported-internal-inlining"), "{names:?}");
        let clones = scored_ir_optimizer_clones(&size, configured);
        assert!(clones.contains(&configured));
        assert!(clones
            .iter()
            .any(|candidate| candidate.inline_exported_internal_calls));

        let mut no_inlining = size.clone();
        no_inlining.optimization.inlining = Some(false);
        let no_inlining_options = no_inlining.js_optimizer_options();
        assert!(
            !admitted_scored_ir_variant_names(&no_inlining, no_inlining_options)
                .contains(&"exported-internal-inlining")
        );

        let no_variants: ProjectConfig = toml::from_str(
            "[javascript]\npriority='size-first'\ncompression=['identifier-mangling']\n",
        )
        .unwrap();
        let no_variant_options = no_variants.js_optimizer_options();
        assert!(
            !admitted_scored_ir_variant_names(&no_variants, no_variant_options)
                .contains(&"exported-internal-inlining")
        );

        let legacy_exact: ProjectConfig = toml::from_str(
            "[javascript]\npriority='size-first'\ncompression=['ir-inlining-variants']\n",
        )
        .unwrap();
        assert!(!admitted_scored_ir_variant_names(
            &legacy_exact,
            legacy_exact.js_optimizer_options()
        )
        .contains(&"exported-internal-inlining"));

        let explicit: ProjectConfig = toml::from_str(
            "[javascript]\npriority='size-first'\ncompression=['ir-inlining-variants','exported-internal-inlining']\n",
        )
        .unwrap();
        assert!(
            admitted_scored_ir_variant_names(&explicit, explicit.js_optimizer_options())
                .contains(&"exported-internal-inlining")
        );
    }

    #[test]
    fn global_alias_forwarding_is_a_distinct_exact_list_aware_ir_clone() {
        assert!(!OptimizationOptions::default().forward_global_aliases);
        assert!(!OptimizationOptions::disabled().forward_global_aliases);

        let size = ProjectConfig::default();
        let configured = size.js_optimizer_options();
        assert!(!configured.forward_global_aliases);
        let names = admitted_scored_ir_variant_names(&size, configured);
        assert!(!names.contains(&"global-alias-forwarding"), "{names:?}");
        let clones = scored_ir_optimizer_clones(&size, configured);
        assert_eq!(clones[0], configured);
        assert!(!clones
            .iter()
            .any(|candidate| candidate.forward_global_aliases));

        let exact_omitted: ProjectConfig = toml::from_str(
            "[javascript]\npriority='size-first'\ncompression=['identifier-mangling']\n",
        )
        .unwrap();
        assert!(!admitted_scored_ir_variant_names(
            &exact_omitted,
            exact_omitted.js_optimizer_options()
        )
        .contains(&"global-alias-forwarding"));

        let search_off: ProjectConfig =
            toml::from_str("[javascript]\ncandidate_search='off'\n").unwrap();
        assert!(
            !admitted_scored_ir_variant_names(&search_off, search_off.js_optimizer_options())
                .contains(&"global-alias-forwarding")
        );

        let explicit: ProjectConfig = toml::from_str(
            "[javascript]\npriority='balanced'\ncompression=['global-alias-forwarding']\n",
        )
        .unwrap();
        assert!(
            admitted_scored_ir_variant_names(&explicit, explicit.js_optimizer_options())
                .contains(&"global-alias-forwarding")
        );
    }
}
