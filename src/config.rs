//! `lilscript.toml`: the project configuration and its resolution into the
//! compiler's policy.
//!
//! A configuration is read in two steps. The retired-key table
//! (`RETIRED_KEYS`) is applied to the parsed TOML first: a key this compiler no
//! longer reads is removed with a warning, and a key whose meaning it cannot
//! honour refuses the configuration. What remains is deserialized strictly, so
//! an unknown key is an error. Nothing is accepted silently.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::js_syntax_target::{resolve_ecmascript_target, EcmaScriptEdition};

/// What becomes of a configuration key this compiler no longer reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retirement {
    /// The key is removed before the configuration is read, and the loader
    /// warns: "`<key>` has no effect in this compiler: <reason>; remove it".
    NoEffect(&'static str),
    /// The configuration is refused with this reason.
    Refused(&'static str),
    /// Refused with `refused` unless the key holds `value`. With that value
    /// the key is kept when `then` is `None` (the compiler reads it), and
    /// removed with a warning when `then` gives the reason it has no effect.
    RefusedUnless {
        value: RetiredValue,
        then: Option<&'static str>,
        refused: &'static str,
    },
}

/// The one value a conditionally refused key may hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetiredValue {
    String(&'static str),
    Integer(i64),
}

impl RetiredValue {
    fn matches(self, value: &toml::Value) -> bool {
        match (self, value) {
            (Self::String(expected), toml::Value::String(actual)) => expected == actual,
            (Self::Integer(expected), toml::Value::Integer(actual)) => expected == *actual,
            _ => false,
        }
    }
}

const OLD_OPTIMIZER: &str =
    "it switched a pass of the old compiler's optimizer, which was deleted; \
this compiler's optional transformations are permitted per family in `[policy.tactics]`";
const OLD_EMITTER: &str =
    "it chose a spelling in the old compiler's JavaScript emitter, which was deleted";
const OLD_NAMING: &str = "it steered the old compiler's local naming, which was deleted; \
this compiler chooses names per artifact";
const OLD_SEARCH: &str = "it bounded the old compiler's candidate search, which was deleted";
const OLD_INLINER: &str = "it bounded the old compiler's inliner, which was deleted";
const SIBLING_LINE: &str = "it configured the `migration/target-tree` line of the compiler, \
which this compiler does not implement";
const PROFILE_GUIDED: &str =
    "profile-guided optimization belonged to the old compiler and was removed with it";
const RUNTIME_SCORING: &str = "it weighted the old compiler's static runtime-cost scores; \
size is the objective, and runtime limits need runtime estimators that do not exist yet";

/// Every configuration key this compiler no longer reads, by dotted path. A
/// path names a key or a whole table; a table entry covers every key in it.
/// Applied to the parsed TOML before strict deserialization
/// (`parse_project_config`). The list entries of `javascript.compression` and
/// `javascript.optimizations` are retired separately
/// (`RETIRED_COMPRESSION_DECISIONS`, `RETIRED_JAVASCRIPT_OPTIMIZATIONS`).
pub const RETIRED_KEYS: &[(&str, Retirement)] = &[
    (
        "compiler.backend",
        Retirement::Refused("there is one compiler; remove [compiler] backend"),
    ),
    (
        "compiler.resources",
        Retirement::NoEffect(
            "the compiler compiles and encodes on one thread; worker threads and codec workers are not implemented yet",
        ),
    ),
    (
        "policy.search.interaction_interval",
        Retirement::NoEffect("the search has no pairwise interaction phase"),
    ),
    (
        "policy.constraints",
        Retirement::Refused(
            "size is the objective; runtime constraints need runtime estimators that do not exist yet; remove [policy.constraints]",
        ),
    ),
    ("optimization.algebraic_simplification", Retirement::NoEffect(OLD_OPTIMIZER)),
    ("optimization.common_subexpression_elimination", Retirement::NoEffect(OLD_OPTIMIZER)),
    ("optimization.finite_value_propagation", Retirement::NoEffect(OLD_OPTIMIZER)),
    ("optimization.global_optimization", Retirement::NoEffect(OLD_OPTIMIZER)),
    ("optimization.inline_closure_factories", Retirement::NoEffect(OLD_OPTIMIZER)),
    ("optimization.constant_parameter_specialization", Retirement::NoEffect(OLD_OPTIMIZER)),
    ("optimization.specialize_tagged_constants", Retirement::NoEffect(OLD_OPTIMIZER)),
    ("optimization.dead_store_elimination", Retirement::NoEffect(OLD_OPTIMIZER)),
    ("optimization.capture_signature_cloning", Retirement::NoEffect(OLD_OPTIMIZER)),
    ("optimization.identical_function_folding", Retirement::NoEffect(OLD_OPTIMIZER)),
    ("optimization.function_subsumption", Retirement::NoEffect(OLD_OPTIMIZER)),
    ("optimization.pipeline_fusion", Retirement::NoEffect(OLD_OPTIMIZER)),
    ("optimization.partial_escape_sinking", Retirement::NoEffect(OLD_OPTIMIZER)),
    ("optimization.region_outlining", Retirement::NoEffect(OLD_OPTIMIZER)),
    ("optimization.expression_superopt", Retirement::NoEffect(OLD_OPTIMIZER)),
    ("optimization.path_sensitive_propagation", Retirement::NoEffect(OLD_OPTIMIZER)),
    ("optimization.profile_guided", Retirement::NoEffect(PROFILE_GUIDED)),
    (
        "optimization.for_of_specialize_family",
        Retirement::RefusedUnless {
            value: RetiredValue::Integer(0),
            then: Some("the for-of family specialization was an old-compiler source rewrite"),
            refused: "the for-of family specialization was an old-compiler source rewrite and was removed; remove the key",
        },
    ),
    (
        "javascript.priority",
        Retirement::RefusedUnless {
            value: RetiredValue::String("size-first"),
            then: None,
            refused: "size is the objective; runtime priorities need runtime estimators that do not exist yet; use `priority = \"size-first\"` or remove the key",
        },
    ),
    (
        "javascript.public_aggregate_abi",
        Retirement::RefusedUnless {
            value: RetiredValue::String("named"),
            then: Some("public aggregates are plain objects with named fields (D2), the only shape"),
            refused: "public aggregates are plain objects with named fields (D2); the positional shape is not produced; remove the key",
        },
    ),
    ("javascript.pool_numeric_literals", Retirement::NoEffect(OLD_EMITTER)),
    ("javascript.integer_coercions", Retirement::NoEffect(OLD_EMITTER)),
    ("javascript.inline_instruction_limit", Retirement::NoEffect(OLD_INLINER)),
    ("javascript.inline_control_flow_limit", Retirement::NoEffect(OLD_INLINER)),
    ("javascript.max_inline_growth", Retirement::NoEffect(OLD_INLINER)),
    (
        "javascript.terminal_cleanup_finalists",
        Retirement::NoEffect(
            "it carried spellings through the old compiler's text cleanup, which was deleted",
        ),
    ),
    ("javascript.max_candidate_raw_growth_percent", Retirement::NoEffect(OLD_SEARCH)),
    ("javascript.function_layout_exact_limit", Retirement::NoEffect(OLD_SEARCH)),
    ("javascript.precise_cross_scope_shadowing", Retirement::NoEffect(OLD_NAMING)),
    ("javascript.transitive_nested_shadowing", Retirement::NoEffect(OLD_NAMING)),
    ("javascript.frequency_order_local_names", Retirement::NoEffect(OLD_NAMING)),
    ("javascript.local_name_reserve", Retirement::NoEffect(OLD_NAMING)),
    ("javascript.stable_local_names", Retirement::NoEffect(OLD_NAMING)),
    ("javascript.local_name_coalescing", Retirement::NoEffect(OLD_NAMING)),
    ("javascript.idiom_directed_naming", Retirement::NoEffect(OLD_NAMING)),
    (
        "javascript.function_scope",
        Retirement::NoEffect(
            "the module wrapper was an old-compiler emission; it returns as the `format` contract axis (plan M3.1)",
        ),
    ),
    ("javascript.emit_pure_annotations", Retirement::NoEffect(OLD_EMITTER)),
    ("javascript.truthy_nullable_checks", Retirement::NoEffect(OLD_EMITTER)),
    ("javascript.name_ordering", Retirement::NoEffect(SIBLING_LINE)),
    ("javascript.terminal_cleanup_chain", Retirement::NoEffect(SIBLING_LINE)),
    ("javascript.wide_single_use_collapse", Retirement::NoEffect(SIBLING_LINE)),
    ("javascript.iife_private_callee_clusters", Retirement::NoEffect(OLD_EMITTER)),
    ("javascript.nested_once_run_helpers", Retirement::NoEffect(OLD_EMITTER)),
    ("javascript.aggregate_operand_order_fusion", Retirement::NoEffect(OLD_EMITTER)),
    ("javascript.sink_entry_function_declarations", Retirement::NoEffect(OLD_EMITTER)),
    (
        "javascript.function_spelling",
        Retirement::NoEffect(
            "this compiler chooses the spelling of private functions itself, and an exported function keeps the callable kind its source declares (D2)",
        ),
    ),
    ("javascript.struct_method_shorthand", Retirement::NoEffect(OLD_EMITTER)),
    ("javascript.local_phi_expression_regions", Retirement::NoEffect(OLD_EMITTER)),
    ("javascript.rematerialize_member_reads", Retirement::NoEffect(OLD_EMITTER)),
    ("javascript.aggregate_layout", Retirement::NoEffect(OLD_EMITTER)),
    ("javascript.startup", Retirement::NoEffect(RUNTIME_SCORING)),
    ("javascript.performance", Retirement::NoEffect(RUNTIME_SCORING)),
    (
        "mangle.exports",
        Retirement::NoEffect(
            "a library build (`--target js-module`) keeps its export names and an application build has none to keep",
        ),
    ),
    (
        "mangle.extern_fields",
        Retirement::NoEffect("no property is renamed, so extern fields always keep their names"),
    ),
    (
        "mangle.internal_properties",
        Retirement::NoEffect(
            "no property is renamed; typed property renaming (plan M9.6) will take ownership from declared types",
        ),
    ),
    ("profile", Retirement::NoEffect(PROFILE_GUIDED)),
    (
        "native",
        Retirement::NoEffect(
            "these switched the old compiler's C emitter, which was deleted; the native target has no such switches",
        ),
    ),
];

/// `javascript.compression` entries that no longer decide anything. Each chose
/// an old-compiler emission or search family.
pub const RETIRED_COMPRESSION_DECISIONS: &[&str] = &[
    "entropy-aware-mangling",
    "quote-style-selection",
    "export-mangling",
    "size-aware-inlining",
    "safe-integer-coercion-elision",
    "compact-boolean-literals",
    "standard-grammar-elision",
    "structured-closure-inlining",
    "pure-helper-inlining",
    "dense-string-return-tables",
    "host-alias-spelling",
    "regex-literals",
    "unused-catch-binding-elision",
    "compact-generator-star",
    "callee-default-arguments",
    "scalar-phi-copies",
    "phi-affinity-coalescing",
    "ir-inlining-variants",
    "exported-internal-inlining",
    "global-alias-forwarding",
    "ir-closure-factory-variants",
    "ir-phase-ordering-variants",
    "loop-spelling-selection",
    "mutation-spelling-selection",
    "indexed-char-at",
    "effect-ternary",
    "array-pipeline-fusion",
    "partial-escape-sinking",
    "region-outlining",
    "expression-superoptimization",
    "path-sensitive-propagation",
    "joint-representation-search",
    "joint-chunk-symbol-search",
];

/// `javascript.optimizations` entries that no longer decide anything. Each
/// named an old-compiler search family.
pub const RETIRED_JAVASCRIPT_OPTIMIZATIONS: &[&str] = &[
    "ir-inlining-variants",
    "ir-closure-factory-variants",
    "ir-phase-ordering-variants",
    "ir-function-subsumption-variants",
    "ir-specialization-variants",
    "structural-control-flow-variants",
    "ssa-destruction-variants",
    "conditional-expression-variants",
    "expression-phi-region-variants",
    "local-phi-expression-region-variants",
    "phi-edge-value-forwarding-variants",
    "constructor-initializer-fusion-variants",
    "fresh-literal-factory-inlining-variants",
    "default-argument-variants",
    "comma-expression-variants",
    "operand-order-fusion-variants",
    "structural-loop-variants",
    "do-loop-variants",
    "update-loop-variants",
    "switch-lowering-variants",
    "compound-mutation-variants",
    "entropy-property-assignment",
    "parsed-peephole",
    "startup-cost-guard",
    "performance-shape-model",
    "profile-guided-optimization",
    "capture-signature-cloning",
    "identical-function-folding",
    "function-layout-variants",
    "ir-compress-pass-variants",
    "joint-chunk-symbol-search",
    "joint-representation-search",
];

/// Apply the retired-key table to a parsed configuration: remove what has no
/// effect, and return one warning for each removal. The first refusal is the
/// error.
pub fn apply_retired_keys(table: &mut toml::Table) -> Result<Vec<String>, String> {
    let mut warnings = Vec::new();
    for &(key, retirement) in RETIRED_KEYS {
        let path = key.split('.').collect::<Vec<_>>();
        let Some(value) = lookup(table, &path) else {
            continue;
        };
        let no_effect = match retirement {
            Retirement::NoEffect(reason) => reason,
            Retirement::Refused(reason) => return Err(format!("`{key}` is refused: {reason}")),
            Retirement::RefusedUnless {
                value: allowed,
                then,
                refused,
            } => {
                if !allowed.matches(value) {
                    return Err(format!("`{key} = {value}` is refused: {refused}"));
                }
                match then {
                    None => continue,
                    Some(reason) => reason,
                }
            }
        };
        remove(table, &path);
        warnings.push(format!(
            "`{key}` has no effect in this compiler: {no_effect}; remove it"
        ));
    }
    for (key, retired, reason) in [
        (
            "compression",
            RETIRED_COMPRESSION_DECISIONS,
            "they chose old-compiler emission and search families",
        ),
        (
            "optimizations",
            RETIRED_JAVASCRIPT_OPTIMIZATIONS,
            "they named old-compiler search families",
        ),
    ] {
        let Some(toml::Value::Array(entries)) = table
            .get_mut("javascript")
            .and_then(toml::Value::as_table_mut)
            .and_then(|javascript| javascript.get_mut(key))
        else {
            continue;
        };
        let mut removed = Vec::new();
        entries.retain(|entry| match entry.as_str() {
            Some(name) if retired.contains(&name) => {
                if !removed.contains(&name.to_string()) {
                    removed.push(name.to_string());
                }
                false
            }
            _ => true,
        });
        if !removed.is_empty() {
            let names = removed
                .iter()
                .map(|name| format!("`{name}`"))
                .collect::<Vec<_>>()
                .join(", ");
            warnings.push(format!(
                "`javascript.{key}` entries {names} have no effect in this compiler: {reason}, which were deleted; remove them"
            ));
        }
    }
    // A table left empty by the removals, such as `[compiler]`, goes too: its
    // section no longer exists.
    for &(key, _) in RETIRED_KEYS {
        let path = key.split('.').collect::<Vec<_>>();
        for depth in (1..path.len()).rev() {
            if lookup(table, &path[..depth])
                .and_then(toml::Value::as_table)
                .is_some_and(toml::Table::is_empty)
            {
                remove(table, &path[..depth]);
            }
        }
    }
    Ok(warnings)
}

fn lookup<'a>(table: &'a toml::Table, path: &[&str]) -> Option<&'a toml::Value> {
    let (last, parents) = path.split_last()?;
    let mut current = table;
    for part in parents {
        current = current.get(*part)?.as_table()?;
    }
    current.get(*last)
}

fn remove(table: &mut toml::Table, path: &[&str]) {
    let Some((last, parents)) = path.split_last() else {
        return;
    };
    let mut current = table;
    for part in parents {
        match current.get_mut(*part).and_then(toml::Value::as_table_mut) {
            Some(next) => current = next,
            None => return,
        }
    }
    current.remove(*last);
}

const PLACEMENT_FOLLOWS_REACHABILITY: &str =
    "placement follows entry reachability and the objective's codec judges every merge (L8)";
const ONE_CODEC_PRICES_FILES: &str = "the objective's codec prices every delivered file (A6)";

/// What became of each `[bundle]` key when the table became `[delivery]`
/// (plan M3.3): a new key, or no effect.
const BUNDLE_KEYS: &[(&str, Option<&str>, &str)] = &[
    ("mode", Some("mode"), ""),
    ("preload", Some("preload"), ""),
    ("host_modules", Some("host_modules"), ""),
    ("min_chunk_bytes", None, PLACEMENT_FOLLOWS_REACHABILITY),
    ("shared_min_imports", None, PLACEMENT_FOLLOWS_REACHABILITY),
    ("max_chunks", None, PLACEMENT_FOLLOWS_REACHABILITY),
    ("cost.raw_weight", None, ONE_CODEC_PRICES_FILES),
    ("cost.gzip_weight", None, ONE_CODEC_PRICES_FILES),
    ("cost.brotli_weight", None, ONE_CODEC_PRICES_FILES),
    (
        "cost.preload_request_discount_percent",
        None,
        ONE_CODEC_PRICES_FILES,
    ),
    (
        "cost.cache_reuse_discount_percent",
        None,
        ONE_CODEC_PRICES_FILES,
    ),
    ("cost.request_overhead_bytes", Some("request_bytes"), ""),
    (
        "cost.dependency_depth_penalty_bytes",
        Some("depth_bytes"),
        "",
    ),
];

/// Translate `[bundle]` into `[delivery]`, one warning per key. A key set in
/// both tables, or a `[bundle]` key the table never had, is refused.
pub fn translate_bundle_table(table: &mut toml::Table) -> Result<Vec<String>, String> {
    let Some(bundle) = table.remove("bundle") else {
        return Ok(Vec::new());
    };
    let toml::Value::Table(mut bundle) = bundle else {
        return Err("`bundle` must be a table".to_string());
    };
    let mut warnings = Vec::new();
    let mut moved = toml::Table::new();
    for &(key, target, reason) in BUNDLE_KEYS {
        let path = key.split('.').collect::<Vec<_>>();
        let Some(value) = lookup(&bundle, &path).cloned() else {
            continue;
        };
        remove(&mut bundle, &path);
        match target {
            Some(target) => {
                moved.insert(target.to_string(), value);
                warnings.push(format!(
                    "`bundle.{key}` is now `delivery.{target}`; rename it"
                ));
            }
            None => warnings.push(format!(
                "`bundle.{key}` has no effect in this compiler: {reason}; remove it"
            )),
        }
    }
    if let Some(toml::Value::Table(cost)) = bundle.get("cost") {
        if cost.is_empty() {
            bundle.remove("cost");
        }
    }
    if let Some(key) = bundle.keys().next() {
        return Err(format!("invalid config: unknown key `bundle.{key}`"));
    }
    if moved.is_empty() {
        return Ok(warnings);
    }
    let delivery = table
        .entry("delivery")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    let toml::Value::Table(delivery) = delivery else {
        return Err("`delivery` must be a table".to_string());
    };
    for (key, value) in moved {
        if delivery.contains_key(&key) {
            return Err(format!(
                "`delivery.{key}` is set, and so is its old spelling in `[bundle]`; keep `delivery.{key}`"
            ));
        }
        delivery.insert(key, value);
    }
    Ok(warnings)
}

/// A configuration read from TOML, with a warning for every retired key it set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedConfig {
    pub config: ProjectConfig,
    pub warnings: Vec<String>,
}

/// Read a configuration: retired keys first (`apply_retired_keys`), then
/// strict deserialization and validation.
pub fn parse_project_config(source: &str) -> Result<ParsedConfig, String> {
    let mut table = source
        .parse::<toml::Table>()
        .map_err(|error| format!("invalid config: {error}"))?;
    let mut warnings = apply_retired_keys(&mut table)?;
    warnings.extend(translate_bundle_table(&mut table)?);
    let config =
        ProjectConfig::deserialize(table).map_err(|error| format!("invalid config: {error}"))?;
    config.validate()?;
    Ok(ParsedConfig { config, warnings })
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectConfig {
    /// Schema-v2 policy overlay; absent values use the centralized legacy translator.
    pub policy: Option<crate::compilation_policy::PolicyConfig>,
    pub package: Option<PackageMetadata>,
    pub dependencies: BTreeMap<String, DependencyConfig>,
    pub optimization: OptimizationConfig,
    pub javascript: JavaScriptConfig,
    pub mangle: MangleConfig,
    pub target: TargetConfig,
    pub delivery: DeliveryConfig,
    pub lint: LintConfig,
    pub format: FormatConfig,
    #[serde(skip)]
    pub config_dir: Option<PathBuf>,
}

impl ProjectConfig {
    /// Resolve configuration once at a public compilation boundary: the one
    /// place the compiler's contract, objective, effort and tactic permissions
    /// are built.
    pub fn resolve_policy(
        &self,
        request: crate::compilation_policy::CompilationRequest,
    ) -> Result<crate::compilation_policy::ResolvedPolicy, String> {
        use crate::compilation_contract::{
            JavaScriptAbiContract, JavaScriptCompilationContract, JavaScriptEffectPolicy,
            JavaScriptExecution, JavaScriptUnsafeAssumptions, JavaScriptWorld,
        };
        use crate::compilation_policy::{
            CompilationContract, CompilationRequest, ObjectiveRank, OptimizationObjective,
            PolicyConfig, ResolvedPolicy, ResolvedTactic, TacticId, TacticPermission,
        };
        self.validate()?;
        let defaults = PolicyConfig::default();
        let policy = self.policy.as_ref().unwrap_or(&defaults);
        let javascript = matches!(request, CompilationRequest::JavaScript { .. });
        let effort = if javascript {
            self.javascript.optimization_level
        } else {
            0
        };
        let mut diagnostics = Vec::new();
        if self.policy.is_none() {
            diagnostics.push(format!("legacy optimizer configuration translated to policy schema {}; translation retires at schema {}", crate::compilation_policy::POLICY_SCHEMA_VERSION, crate::compilation_policy::LEGACY_TRANSLATOR_RETIREMENT_SCHEMA));
        }
        let mut tactics = [ResolvedTactic {
            permission: TacticPermission::Auto,
            enabled: false,
        }; TacticId::ALL.len()];
        let maximum_preset = self.optimization.preset == OptimizationPreset::Maximum;
        for tactic in TacticId::ALL {
            let spec = tactic.spec();
            if spec.javascript_only && !javascript {
                continue;
            }
            let (legacy_explicit, configured_default) = self.configured_tactic(tactic, javascript);
            let default = configured_default.unwrap_or(spec.default.enabled(maximum_preset));
            let configured = policy
                .tactics
                .get(&tactic)
                .copied()
                .unwrap_or(TacticPermission::Auto);
            if let Some(legacy) = legacy_explicit {
                if matches!(
                    (configured, legacy),
                    (TacticPermission::On, false) | (TacticPermission::Off, true)
                ) {
                    return Err(format!("`policy.tactics.{}` contradicts its explicit legacy setting; remove the legacy setting or use the same permission", spec.name));
                }
            }
            let permission = match (configured, legacy_explicit) {
                (TacticPermission::Auto, Some(true)) => TacticPermission::On,
                (TacticPermission::Auto, Some(false)) => TacticPermission::Off,
                _ => configured,
            };
            let enabled = (!spec.javascript_only || javascript)
                && match permission {
                    TacticPermission::Off => false,
                    TacticPermission::On => true,
                    TacticPermission::Auto => {
                        default && (!javascript || effort >= spec.minimum_effort)
                    }
                };
            tactics[tactic as usize] = ResolvedTactic {
                permission,
                enabled,
            };
        }
        let (contract, objective) = match request {
            CompilationRequest::Native => (
                CompilationContract::Native {
                    abi_version: crate::package::LILSCRIPT_ABI_VERSION,
                },
                None,
            ),
            CompilationRequest::JavaScript {
                preserve_root_exports,
            } => {
                let language = JavaScriptCompilationContract {
                    // The request's export flag also selects module output.
                    // Record that execution promise explicitly; world and
                    // visibility are no proof that an artifact executes in
                    // strict mode.
                    execution: if preserve_root_exports {
                        JavaScriptExecution::Module
                    } else {
                        JavaScriptExecution::Script
                    },
                    world: if preserve_root_exports {
                        JavaScriptWorld::ReusableLibrary
                    } else {
                        JavaScriptWorld::ClosedApplication
                    },
                    ecmascript: self.javascript.resolved_ecmascript(),
                    abi: JavaScriptAbiContract {
                        preserve_root_exports,
                        keep_function_names: self.javascript.keep_function_names,
                        keep_published_function_names: self
                            .javascript
                            .keep_published_function_names,
                    },
                    assumptions: JavaScriptUnsafeAssumptions {
                        pristine_builtins: self.javascript.assume_pristine_builtins,
                        pure_property_reads: self.javascript.assume_pure_property_reads,
                        unconstructed_callbacks: self.javascript.assume_unconstructed_callbacks,
                        numeric_lengths: self
                            .javascript
                            .compression_enabled(CompressionDecision::LengthToNumberElision),
                    },
                    effects: JavaScriptEffectPolicy {
                        strip_console: self.javascript.strip_console,
                    },
                };
                let mut preserved_properties =
                    self.mangle.preserve_properties.clone().unwrap_or_default();
                preserved_properties.sort();
                preserved_properties.dedup();
                let objective = OptimizationObjective {
                    codec: self.javascript.cost_model,
                    rank: ObjectiveRank {
                        priority: self.javascript.priority,
                    },
                    optional_alternatives: self.javascript.effective_candidate_proposal_limit(),
                    optional_codec_probes: self.javascript.effective_terminal_codec_probe_limit(),
                    retained_candidates: self.javascript.effective_candidate_limit(),
                    retained_candidate_bytes: self.javascript.effective_candidate_byte_budget(),
                    beam_width: self.javascript.effective_candidate_beam_width(),
                    terminal_challengers: self.javascript.effective_terminal_challenger_limit(),
                    terminal_choices: self.javascript.effective_terminal_choice_limit(),
                    search: policy.search,
                };
                let delivery = self.delivery_contract(preserve_root_exports)?;
                (
                    CompilationContract::JavaScript {
                        language,
                        preserved_properties,
                        delivery,
                    },
                    Some(objective),
                )
            }
        };
        Ok(ResolvedPolicy::new(
            contract,
            objective,
            effort,
            tactics,
            policy.resources,
            policy.constraints,
            diagnostics,
        ))
    }

    /// The delivery part of the contract (plan M3.3). `library` is the
    /// world: a library's chunk names default to their plan position, the
    /// shortest delivered bytes; an application's to a content hash, which
    /// caches safely across releases.
    pub fn delivery_contract(
        &self,
        library: bool,
    ) -> Result<crate::compilation_policy::DeliveryContract, String> {
        let delivery = &self.delivery;
        if delivery.mode != DeliveryMode::Single && !library {
            return Err(format!(
                "`delivery.mode = \"{}\"` needs module execution, whose files import each other: build with `--target js-module`",
                delivery.mode.name()
            ));
        }
        Ok(crate::compilation_policy::DeliveryContract {
            mode: delivery.mode,
            format: self.target.javascript.format,
            preload: if delivery.mode == DeliveryMode::Single {
                PreloadPolicy::None
            } else {
                delivery.preload
            },
            entry_names: delivery.entry_names.clone(),
            chunk_names: delivery.chunk_names.clone(),
            module_names: delivery.module_names.clone(),
            library,
            request_bytes: delivery.request_bytes,
            depth_bytes: delivery.depth_bytes,
        })
    }

    /// The sole bridge from the older per-tactic keys and allowlists into the
    /// tactic registry: an explicit setting, and a default that replaces the
    /// spec row's where the configured priority decides it. Explicit omission
    /// from an allowlist means off; it must not reappear through a different
    /// aggregate producer.
    fn configured_tactic(
        &self,
        tactic: crate::compilation_policy::TacticId,
        javascript: bool,
    ) -> (Option<bool>, Option<bool>) {
        use crate::compilation_policy::TacticId as T;
        let compression = |decision| {
            (
                self.javascript
                    .compression
                    .as_ref()
                    .map(|list| list.contains(&decision)),
                Some(self.javascript.compression_enabled(decision)),
            )
        };
        match tactic {
            T::DeadCodeElimination => (self.optimization.dead_code_elimination, None),
            T::ConstantFolding => (self.optimization.constant_folding, None),
            T::Inlining => (self.optimization.inlining, None),
            T::ScalarReplacement => (self.optimization.scalar_replacement, None),
            T::CallSpecialization => {
                let explicit = self.optimization.call_site_specialization.or_else(|| {
                    if javascript {
                        self.javascript
                            .optimizations
                            .as_ref()
                            .map(|v| v.contains(&JavaScriptOptimization::CallSiteSpecialization))
                    } else {
                        None
                    }
                });
                (explicit, None)
            }
            T::HelperSharing => {
                let (legacy, _) = compression(CompressionDecision::ParameterizedFunctionMerging);
                let default = self
                    .optimization
                    .parameterized_function_merging
                    .unwrap_or(self.optimization.preset != OptimizationPreset::None)
                    && self
                        .javascript
                        .compression_enabled(CompressionDecision::ParameterizedFunctionMerging);
                (
                    self.optimization.parameterized_function_merging.or(legacy),
                    Some(default),
                )
            }
            T::TargetCompaction => (
                (!self.javascript.operand_order_fusion).then_some(false),
                None,
            ),
            T::IdentifierMangling => {
                let (legacy, _) = compression(CompressionDecision::IdentifierMangling);
                (self.mangle.identifiers.or(legacy), None)
            }
            T::PropertyMangling => {
                let (legacy, default) = compression(CompressionDecision::PropertyMangling);
                (self.mangle.properties.or(legacy), default)
            }
            T::StringPooling => {
                let (legacy, default) = compression(CompressionDecision::StringPooling);
                (self.mangle.pool_strings.or(legacy), default)
            }
            T::StringArrayPacking => compression(CompressionDecision::StringArrayPacking),
            T::StartupReconstruction | T::RecurringReconstruction => (None, None),
            T::NamingSearch => {
                let explicit = self
                    .javascript
                    .optimizations
                    .as_ref()
                    .map(|v| v.contains(&JavaScriptOptimization::EntropyCrossScopeReuse));
                (explicit, None)
            }
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if let Some(policy) = &self.policy {
            policy.validate()?;
        }
        if let Some(package) = &self.package {
            validate_package_name(&package.name)?;
            semver::Version::parse(&package.version)
                .map_err(|error| format!("invalid `package.version`: {error}"))?;
            if package.abi != crate::package::LILSCRIPT_ABI_VERSION {
                return Err(format!(
                    "`package.abi` is {}, but this compiler supports ABI {}",
                    package.abi,
                    crate::package::LILSCRIPT_ABI_VERSION
                ));
            }
            if package.entry.as_os_str().is_empty() {
                return Err("`package.entry` must not be empty".to_string());
            }
        }
        for (name, dependency) in &self.dependencies {
            validate_package_name(name)?;
            if dependency.path.as_os_str().is_empty() {
                return Err(format!("dependency `{name}` has an empty path"));
            }
            semver::VersionReq::parse(&dependency.version).map_err(|error| {
                format!("dependency `{name}` has invalid version requirement: {error}")
            })?;
            if dependency.abi != crate::package::LILSCRIPT_ABI_VERSION {
                return Err(format!(
                    "dependency `{name}` requests ABI {}, but this compiler supports ABI {}",
                    dependency.abi,
                    crate::package::LILSCRIPT_ABI_VERSION
                ));
            }
        }
        for name in self.delivery.entries.keys() {
            if !valid_entry_name(name) {
                return Err(format!(
                    "`delivery.entries` name `{name}` must be letters, digits, `_`, `.` or `-`"
                ));
            }
        }
        for (key, template) in [
            ("entry_names", &self.delivery.entry_names),
            ("chunk_names", &self.delivery.chunk_names),
            ("module_names", &self.delivery.module_names),
        ] {
            if let Some(template) = template {
                crate::js::names::check_template(key, template)?;
            }
        }
        if let Some(decisions) = &self.javascript.compression {
            let mut unique = HashSet::with_capacity(decisions.len());
            for decision in decisions {
                if !unique.insert(*decision) {
                    return Err(format!(
                        "`javascript.compression` contains duplicate `{}`",
                        decision.name()
                    ));
                }
            }
        }
        resolve_ecmascript_target(self.javascript.ecmascript, &self.javascript.browsers)?;
        if self.javascript.candidate_limit == 0 {
            return Err("`javascript.candidate_limit` must be greater than zero".to_string());
        }
        if self.javascript.candidate_byte_budget == 0 {
            return Err("`javascript.candidate_byte_budget` must be greater than zero".to_string());
        }
        if self.javascript.candidate_beam_width == 0 {
            return Err("`javascript.candidate_beam_width` must be greater than zero".to_string());
        }
        if self.javascript.optimization_level > 16 {
            return Err("`javascript.optimization_level` must be between 0 and 16".to_string());
        }
        if let Some(features) = &self.javascript.optimizations {
            let mut unique = HashSet::with_capacity(features.len());
            for feature in features {
                if !unique.insert(*feature) {
                    return Err(format!(
                        "`javascript.optimizations` contains duplicate `{}`",
                        feature.name()
                    ));
                }
            }
        }
        if self.format.line_width < 40 {
            return Err("`format.line_width` must be at least 40".to_string());
        }
        for rule in self.lint.rules.keys() {
            if rule.trim().is_empty() {
                return Err("`lint.rules` contains an empty rule name".to_string());
            }
        }
        if let Some(providers) = &self.lint.providers {
            let mut unique = HashSet::with_capacity(providers.len());
            for provider in providers {
                if provider.trim().is_empty() || provider.contains('/') {
                    return Err(
                        "`lint.providers` names must be nonempty namespace identifiers without `/`"
                            .to_string(),
                    );
                }
                if !unique.insert(provider) {
                    return Err(format!("`lint.providers` contains duplicate `{provider}`"));
                }
            }
        }
        Ok(())
    }
}

fn validate_package_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(format!(
            "package name `{name}` must contain only ASCII letters, digits, `-`, or `_`"
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PackageMetadata {
    pub name: String,
    pub version: String,
    pub abi: u32,
    pub entry: PathBuf,
}

impl Default for PackageMetadata {
    fn default() -> Self {
        Self {
            name: String::new(),
            version: "0.1.0".to_string(),
            abi: crate::package::LILSCRIPT_ABI_VERSION,
            entry: PathBuf::from("src/lib.lil"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DependencyConfig {
    pub path: PathBuf,
    pub version: String,
    pub abi: u32,
}

impl Default for DependencyConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            version: "*".to_string(),
            abi: crate::package::LILSCRIPT_ABI_VERSION,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JavaScriptPriority {
    PerformanceFirst,
    #[serde(alias = "realisticperf-first", alias = "realistic-perf-first")]
    RealisticPerformanceFirst,
    Balanced,
    #[default]
    SizeFirst,
}

impl JavaScriptPriority {
    /// Whether the priority enables a compression decision when
    /// `javascript.compression` is omitted. Only `size-first` passes the
    /// configuration loader; the others remain for the ranking machinery
    /// that runtime estimators will need.
    const fn enables_compression(self, decision: CompressionDecision) -> bool {
        match decision {
            CompressionDecision::IdentifierMangling => true,
            CompressionDecision::StringPooling => !matches!(self, Self::PerformanceFirst),
            CompressionDecision::LengthToNumberElision
            | CompressionDecision::StringArrayPacking
            | CompressionDecision::PropertyMangling
            | CompressionDecision::ParameterizedFunctionMerging => matches!(self, Self::SizeFirst),
        }
    }
}

/// The `javascript.compression` decisions this compiler reads. Each maps onto
/// a tactic permission or a contract assumption; an explicit list that omits
/// one turns it off. The retired decisions are in
/// `RETIRED_COMPRESSION_DECISIONS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompressionDecision {
    /// The `identifier-mangling` tactic.
    IdentifierMangling,
    /// The `property-mangling` tactic.
    PropertyMangling,
    /// The `string-pooling` tactic.
    StringPooling,
    /// The contract assumption that a host value's `length` is an int32 Number.
    LengthToNumberElision,
    /// The `string-array-packing` tactic.
    StringArrayPacking,
    /// The `helper-sharing` tactic.
    ParameterizedFunctionMerging,
}

impl CompressionDecision {
    pub const ALL: [Self; 6] = [
        Self::IdentifierMangling,
        Self::PropertyMangling,
        Self::StringPooling,
        Self::LengthToNumberElision,
        Self::StringArrayPacking,
        Self::ParameterizedFunctionMerging,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::IdentifierMangling => "identifier-mangling",
            Self::PropertyMangling => "property-mangling",
            Self::StringPooling => "string-pooling",
            Self::LengthToNumberElision => "length-to-number-elision",
            Self::StringArrayPacking => "string-array-packing",
            Self::ParameterizedFunctionMerging => "parameterized-function-merging",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct JavaScriptConfig {
    /// The objective's priority. Only `size-first` is accepted: size is the
    /// objective, and runtime priorities need runtime estimators that do not
    /// exist yet.
    pub priority: JavaScriptPriority,
    /// The ECMAScript edition the output may use.
    pub ecmascript: EcmaScriptEdition,
    /// Browser floors; the output uses the newest edition all of them support,
    /// capped by `ecmascript`.
    pub browsers: Vec<String>,
    /// The effort level, 0 to 16: a versioned schedule of search breadth and
    /// tactic gates.
    pub optimization_level: u8,
    /// An exact allowlist of search families. This compiler reads two entries:
    /// `call-site-specialization` (the `call-specialization` tactic) and
    /// `entropy-cross-scope-reuse` (the `naming-search` tactic); an explicit
    /// list that omits one turns that tactic off.
    pub optimizations: Option<Vec<JavaScriptOptimization>>,
    /// An exact allowlist of compression decisions (`CompressionDecision`);
    /// omitted, `priority` decides. An explicit list that omits a decision
    /// turns it off.
    pub compression: Option<Vec<CompressionDecision>>,
    /// The codec whose bytes the objective minimizes: `raw`, `gzip` or `brotli`.
    pub cost_model: CompressionCostModel,
    /// Whether the candidate search runs: `off`, `production` or `always`.
    /// `--mode development` sets `off`.
    pub candidate_search: CandidateSearch,
    /// The most whole-artifact candidates the search retains.
    pub candidate_limit: usize,
    /// The most bytes of retained candidates.
    pub candidate_byte_budget: usize,
    /// The beam width of the structural search.
    pub candidate_beam_width: usize,
    /// Maximum optional structural emission plans admitted after the scored
    /// context seeds are installed. Omitted values also honor
    /// `candidate_limit`, so a deliberately tiny retained frontier stays a
    /// tiny-work search. An explicit value decouples attempted work from that
    /// survivor count while remaining bounded by the level/search tier.
    /// Zero keeps only scored seeds and the reserved terminal challenger tail.
    pub candidate_proposal_limit: Option<usize>,
    /// Maximum whole-artifact work units in terminal syntax/name search. A
    /// unit is charged before optional repair/validation and bounds one exact
    /// codec call. Omitted values derive from the level; zero disables
    /// optional terminal search while retaining the incumbent.
    pub terminal_codec_probe_limit: Option<usize>,
    /// Rebuild a nested expression when a run of single-use producers all feed
    /// one consumer that reads them in production order. `false` turns the
    /// `target-compaction` tactic off.
    pub operand_order_fusion: bool,
    /// Allow representations that bypass ambient JavaScript constructor
    /// bindings. This is false for open-world library output.
    pub assume_pristine_builtins: bool,
    /// Treat a dynamic member read as free of coercion hooks, the way Terser's
    /// `pure_getters` does. A read like `o[k]` is otherwise a hook: a getter
    /// could run user code while the value is being evaluated, so the value is
    /// unstable and has to take its own statement instead of nesting into its
    /// consumer. Ports whose objects are plain data say so here and measure it;
    /// it is false by default because a library cannot assume its callers'
    /// objects have no accessors.
    pub assume_pure_property_reads: bool,
    /// A function made from a lambda is never constructed (with `new`) nor its
    /// `prototype` read, except through the variable the program declared it
    /// in, the way Terser's `unsafe_arrows` assumes. A lambda that ignores its
    /// receiver may then be an arrow wherever it goes; one the program
    /// constructs or whose `prototype` it reads through its variable stays a
    /// function either way. False by default: a library cannot know what its
    /// callers do with its callbacks.
    pub assume_unconstructed_callbacks: bool,
    /// Keep the exact source `name` of every function whose name some code
    /// could read, not only of published exports. Off by default: an exported
    /// function always keeps its source name (D2), while an internal function
    /// that escapes through a `JsValue` gets whatever name its binding has, as
    /// Terser's mangling gives it. Turn it on for code that reads `fn.name` of
    /// callbacks it did not export.
    pub keep_function_names: bool,
    /// Keep the exact source `name` of published functions (D2). On by
    /// default. A library whose contract is its export names, not the
    /// reflected `fn.name` of what it exports, may turn it off: its published
    /// functions are then named like any other, as a minifier's top-level
    /// mangling names them.
    pub keep_published_function_names: bool,
    /// Drop `print()` / `debugLog` from JavaScript. On by default so production
    /// builds do not ship `console.log`. Test oracles set false. Does not strip
    /// `console.warn` (observable library behavior).
    pub strip_console: bool,
}

impl Default for JavaScriptConfig {
    fn default() -> Self {
        Self {
            priority: JavaScriptPriority::SizeFirst,
            ecmascript: EcmaScriptEdition::Es2022,
            browsers: Vec::new(),
            // Level 13, not the ceiling. Measured on the jQuery port: level 15
            // costs 1829 CPU-seconds against level 13's 89.8 — 20x — to save
            // 426 Brotli bytes, 1.4%, and it issues 500 canonical encodes
            // against 52. Levels 12 through 14 sit on a plateau within 0.15% of
            // each other on both jQuery and acorn, and the curve only breaks
            // down at 11 and below. A project that wants the last percent can
            // still ask for 15 explicitly; it should not be the price of not
            // having an opinion. See finer/hypotheses/007-level-13-sweet-spot.
            optimization_level: 13,
            optimizations: None,
            compression: None,
            cost_model: CompressionCostModel::Brotli,
            candidate_search: CandidateSearch::Production,
            candidate_limit: 1536,
            candidate_byte_budget: 1024 * 1024,
            candidate_beam_width: 12,
            candidate_proposal_limit: None,
            terminal_codec_probe_limit: None,
            operand_order_fusion: true,
            assume_pristine_builtins: false,
            assume_pure_property_reads: false,
            assume_unconstructed_callbacks: false,
            keep_function_names: false,
            keep_published_function_names: true,
            strip_console: true,
        }
    }
}

/// The `javascript.optimizations` entries this compiler reads. The retired
/// entries are in `RETIRED_JAVASCRIPT_OPTIMIZATIONS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JavaScriptOptimization {
    /// The `call-specialization` tactic.
    CallSiteSpecialization,
    /// The `naming-search` tactic.
    EntropyCrossScopeReuse,
}

impl JavaScriptOptimization {
    pub const fn name(self) -> &'static str {
        match self {
            Self::CallSiteSpecialization => "call-site-specialization",
            Self::EntropyCrossScopeReuse => "entropy-cross-scope-reuse",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompressionCostModel {
    Raw,
    Gzip,
    #[default]
    Brotli,
}

impl CompressionCostModel {
    /// The configuration spelling.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Gzip => "gzip",
            Self::Brotli => "brotli",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CandidateSearch {
    Off,
    #[default]
    Production,
    Always,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LintSeverity {
    Off,
    Hint,
    #[default]
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LintPreset {
    Minimal,
    #[default]
    Recommended,
    Strict,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LintConfig {
    pub enabled: bool,
    pub preset: LintPreset,
    pub deny_warnings: bool,
    pub providers: Option<Vec<String>>,
    pub exclude: Vec<String>,
    pub pure_extern_allowlist: Vec<String>,
    pub rules: BTreeMap<String, LintSeverity>,
}

impl Default for LintConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            preset: LintPreset::Recommended,
            deny_warnings: false,
            providers: None,
            exclude: Vec::new(),
            pure_extern_allowlist: Vec::new(),
            rules: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NewlineStyle {
    #[default]
    Lf,
    Crlf,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FormatConfig {
    pub enabled: bool,
    pub line_width: usize,
    pub newline: NewlineStyle,
    pub organize_imports: bool,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            line_width: 100,
            newline: NewlineStyle::Lf,
            organize_imports: true,
        }
    }
}

impl JavaScriptConfig {
    pub fn resolved_ecmascript(&self) -> EcmaScriptEdition {
        resolve_ecmascript_target(self.ecmascript, &self.browsers).unwrap_or(self.ecmascript)
    }

    fn compression_enabled(&self, decision: CompressionDecision) -> bool {
        self.compression.as_ref().map_or_else(
            || self.priority.enables_compression(decision),
            |enabled| enabled.contains(&decision),
        )
    }

    pub fn effective_candidate_limit(&self) -> usize {
        let level_limit = match self.optimization_level {
            0..=2 => 1,
            3..=4 => 16,
            5..=6 => 64,
            7..=8 => 192,
            9..=10 => 384,
            11..=12 => 768,
            13..=14 => 1_024,
            _ => usize::MAX,
        };
        let search_limit = match self.candidate_search {
            CandidateSearch::Off => 1,
            CandidateSearch::Production => 384,
            CandidateSearch::Always => usize::MAX,
        };
        self.candidate_limit.min(level_limit).min(search_limit)
    }

    /// The configured byte pool is a ceiling, while the optimization level
    /// supplies a progressively larger default work tier. The configured root
    /// can always exceed this value: the arena raises its effective byte floor
    /// to retain that mandatory incumbent.
    pub fn effective_candidate_byte_budget(&self) -> usize {
        let level_limit = match self.optimization_level {
            0..=2 => 64 * 1024,
            3..=4 => 128 * 1024,
            5..=6 => 192 * 1024,
            7..=8 => 256 * 1024,
            9..=10 => 384 * 1024,
            11..=12 => 512 * 1024,
            13 => 768 * 1024,
            14 => 896 * 1024,
            _ => usize::MAX,
        };
        let search_limit = match self.candidate_search {
            CandidateSearch::Off => 1,
            CandidateSearch::Production | CandidateSearch::Always => usize::MAX,
        };
        self.candidate_byte_budget
            .min(level_limit)
            .min(search_limit)
    }

    /// Beam width participates in the effort ladder too. Previously every
    /// nonzero level inherited the level-15 width of twelve even when its
    /// candidate cap was intentionally small.
    pub fn effective_candidate_beam_width(&self) -> usize {
        let level_limit = match self.optimization_level {
            0..=2 => 1,
            3..=4 => 2,
            5..=6 => 3,
            7..=8 => 4,
            9..=10 => 6,
            11..=12 => 8,
            13 => 10,
            14 => 11,
            _ => usize::MAX,
        };
        self.candidate_beam_width
            .min(level_limit)
            .min(self.effective_candidate_limit())
            .max(1)
    }

    /// Hard ceiling for optional structural whole-artifact proposals after
    /// the already-scored IR context seeds have been installed. Survivor and
    /// byte limits cannot provide this guarantee: hundreds of rejected plans
    /// may be emitted before a small survivor frontier is chosen.
    /// The ceiling an explicitly configured proposal budget may not exceed.
    /// The optimization level sets the *default* breadth, so an explicit budget
    /// is allowed past it; the search tier is a different thing and stays hard.
    fn candidate_proposal_tier_ceiling(&self) -> usize {
        match self.candidate_search {
            CandidateSearch::Off => 0,
            CandidateSearch::Production => 384,
            CandidateSearch::Always => usize::MAX,
        }
    }

    fn candidate_proposal_level_limit(&self) -> usize {
        let level_limit = match self.optimization_level {
            0..=2 => 0,
            3..=4 => 16,
            5..=6 => 64,
            7..=8 => 192,
            9..=10 => 384,
            11..=12 => 768,
            13..=14 => 1_024,
            _ => 1_536,
        };
        match self.candidate_search {
            CandidateSearch::Off => 0,
            CandidateSearch::Production => level_limit.min(384),
            CandidateSearch::Always => level_limit,
        }
    }

    pub fn effective_candidate_proposal_limit(&self) -> usize {
        let level_limit = self.candidate_proposal_level_limit();
        // A level that turns the search off turns it off for everyone; an
        // explicit budget widens a search that is running, it does not start one.
        if level_limit == 0 {
            return 0;
        }
        self.candidate_proposal_limit.map_or_else(
            || self.effective_candidate_limit().min(level_limit),
            |configured| configured.min(self.candidate_proposal_tier_ceiling()),
        )
    }

    /// Hard ceiling for optional whole-artifact work after structural
    /// candidates have been ranked. This is deliberately independent of
    /// survivor count: one large survivor can expose thousands of proposals.
    fn terminal_codec_probe_level_limit(&self) -> usize {
        let level_limit = match self.optimization_level {
            0..=7 => 0,
            8 => 24,
            9..=10 => 64,
            11..=12 => 128,
            // The terminal probe budget is the one dimension of the effort
            // ladder that measurably buys bytes, and 13 was rationing it.
            // Measured with the budget pinned explicitly, so artifact scaling
            // is out of the picture: on the acorn port the Brotli curve is
            // 3071 at 192 probes and 3063 from 384 onward — flat through 3072 —
            // and 3063 beats what level 15 produces (3069). On jQuery, the
            // budget that level 13 actually reaches after artifact scaling was
            // ~42 probes; doubling this base takes it to ~84 and moves Brotli
            // from 30651 to 30593, which is 87% of the gain an unscaled 384
            // achieves (30587) for half the probes.
            //
            // Levels above 13 deliberately stop here too. Raising them to 512
            // and 768 was tried on the strength of jQuery still gaining at 768
            // unscaled probes, and then measured: level 15 went from 1829 to
            // **5434 CPU-seconds** — three times slower — to save 192 Brotli
            // bytes. That is the same bad trade level 15 was already criticized
            // for, made worse, so it was reverted. 384 is the measured knee on
            // acorn (flat from 384 through 3072) and it restores level 15 to
            // exactly the budget it had before.
            _ => 384,
        };
        match self.candidate_search {
            CandidateSearch::Off => 0,
            CandidateSearch::Production => level_limit,
            CandidateSearch::Always => level_limit.saturating_mul(4),
        }
    }

    /// How many declared terminal challengers the exact codec judges on each
    /// objective's final candidate (M5.4): each is one exact codec score of
    /// the complete artifact, the stage's dominant cost. Challengers that
    /// render the incumbent's bytes cost a formation and a render only, and
    /// the declared schedule bounds those. The schedule's order is fixed, so a
    /// higher level walks further along the same challengers: from the same
    /// final candidate it can only keep more. The search-off levels offer
    /// none, like the probe ladder.
    pub fn effective_terminal_challenger_limit(&self) -> usize {
        // Level 13 reaches every challenger kept on the seven reference ports
        // when the stage landed (the first seven of the schedule).
        let level_limit = match self.optimization_level {
            0..=7 => 0,
            8 => 3,
            9..=10 => 4,
            11..=12 => 5,
            13 => 7,
            14 => 9,
            _ => usize::MAX,
        };
        match self.candidate_search {
            CandidateSearch::Off => 0,
            CandidateSearch::Production => level_limit,
            CandidateSearch::Always if level_limit == 0 => 0,
            CandidateSearch::Always => usize::MAX,
        }
    }

    /// How many choice alternatives (M9.1) the terminal stage judges per
    /// objective: the challengers' ladder, counted apart. A site offers one
    /// to three alternatives the estimator ranks, and most programs have no
    /// site, so the ladder bounds the codec work a program with many tables
    /// can ask for rather than what one table needs.
    pub fn effective_terminal_choice_limit(&self) -> usize {
        self.effective_terminal_challenger_limit()
    }

    pub fn effective_terminal_codec_probe_limit(&self) -> usize {
        let level_limit = self.terminal_codec_probe_level_limit();
        if level_limit == 0 {
            return 0;
        }
        // An explicit limit is a request for more verification, and it is
        // honored. Measured on jQuery: raising the ceiling from 384 to the
        // configured 1536 is 33 Brotli bytes for 24% more compile time, because
        // terminal search on an 84KB artifact is budget-limited rather than
        // idea-limited. Silently clamping to the level meant a config could ask
        // for four times the search and receive none of it.
        self.terminal_codec_probe_limit.unwrap_or(level_limit)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OptimizationPreset {
    None,
    #[default]
    Maximum,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OptimizationConfig {
    /// `maximum` (the default) or `none`: the default of every tactic whose
    /// spec row follows the preset.
    pub preset: OptimizationPreset,
    /// The `constant-folding` tactic, when set.
    pub constant_folding: Option<bool>,
    /// The `inlining` tactic, when set.
    pub inlining: Option<bool>,
    /// The `scalar-replacement` tactic, when set.
    pub scalar_replacement: Option<bool>,
    /// The `dead-code-elimination` tactic, when set.
    pub dead_code_elimination: Option<bool>,
    /// The `call-specialization` tactic, when set.
    pub call_site_specialization: Option<bool>,
    /// The `helper-sharing` tactic, when set.
    pub parameterized_function_merging: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MangleConfig {
    /// The `identifier-mangling` tactic, when set.
    pub identifiers: Option<bool>,
    /// The `property-mangling` tactic, when set.
    pub properties: Option<bool>,
    /// Property names this port's public API exchanges with its callers, which
    /// the compiler cannot see because they are read in code it never compiles:
    /// options a caller sets on an object it authors, fields a callback reads
    /// off a context the program hands it, members of a value the program
    /// returns. A contract: these names are never renamed. The host surface is
    /// already known (`js_platform`); this is the part only the port knows.
    /// Nothing renames properties yet, so the contract holds for every name;
    /// typed property renaming (plan M9.6) reads it.
    pub preserve_properties: Option<Vec<String>>,
    /// The `string-pooling` tactic, when set.
    pub pool_strings: Option<bool>,
}

/// How the program's root statements are placed in files (plan M3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeliveryMode {
    /// One file per entry: every statement an entry reaches, in its
    /// evaluation order.
    #[default]
    Single,
    /// Files by entry reachability: code several entries share lives once.
    Split,
    /// Every source module is a file, as ES modules evaluate the source.
    PreserveModules,
}

impl DeliveryMode {
    /// The configuration spelling.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Split => "split",
            Self::PreserveModules => "preserve-modules",
        }
    }
}

/// The container a delivered file is written in (architecture §14
/// `[target.javascript] format`). Placement never depends on it; only the
/// link spellings and the frame around program code do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum JavaScriptFormat {
    /// ES modules: `import`/`export`, `import()`.
    #[default]
    Esm,
    /// CommonJS: `require`, `exports`.
    Cjs,
}

impl JavaScriptFormat {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Esm => "esm",
            Self::Cjs => "cjs",
        }
    }
    /// The `[ext]` of a delivered file in this container.
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Esm => "js",
            Self::Cjs => "cjs",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HostModules {
    /// Import them from their original specifiers.
    #[default]
    External,
    /// Carry them when every one can be delivered; otherwise import them.
    Auto,
    /// Carry them, and refuse the build when one cannot be delivered.
    Embed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PreloadPolicy {
    #[default]
    None,
    Entry,
    All,
}

/// `[delivery]` (architecture §14): how the one program is placed in files
/// and named. Entries are the program's roots; every other key is contract.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DeliveryConfig {
    pub mode: DeliveryMode,
    /// Entry name to source path, relative to this file. Sorted by name:
    /// entry `i` is bit `i` of every reachability label.
    pub entries: BTreeMap<String, PathBuf>,
    /// File name templates: `[name]` (entry), `[index]` (plan position),
    /// `[hash:N]` (content hash), `[path]` (source module path) and `[ext]`.
    pub entry_names: Option<String>,
    pub chunk_names: Option<String>,
    pub module_names: Option<String>,
    /// Which lazily loaded files an entry preloads.
    pub preload: PreloadPolicy,
    /// Whether relative host modules travel with the output.
    pub host_modules: HostModules,
    /// Declared deployment costs (L12): bytes per file a row loads beyond its
    /// first, and per static import level beyond the first.
    pub request_bytes: u64,
    pub depth_bytes: u64,
}

impl Default for DeliveryConfig {
    fn default() -> Self {
        Self {
            mode: DeliveryMode::Single,
            entries: BTreeMap::new(),
            entry_names: None,
            chunk_names: None,
            module_names: None,
            preload: PreloadPolicy::None,
            host_modules: HostModules::External,
            request_bytes: 0,
            depth_bytes: 0,
        }
    }
}

/// Whether `name` may name an entry: it becomes a file name and an entry
/// label, so it is a plain path segment.
pub fn valid_entry_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
}

/// `[target]`: the contract axes of schema v3 this compiler reads so far.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TargetConfig {
    pub javascript: TargetJavaScriptConfig,
}

/// `[target.javascript]`: only `format` so far (plan M3.1 brings the
/// other axes here).
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TargetJavaScriptConfig {
    pub format: JavaScriptFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedConfig {
    pub config: ProjectConfig,
    pub path: Option<PathBuf>,
    /// One warning for each retired key the file set (`RETIRED_KEYS`). The
    /// CLI prints them and `--print-policy` reports them.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    pub path: PathBuf,
    pub message: String,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)
    }
}

impl std::error::Error for ConfigError {}

pub fn load_project_config(
    input: &Path,
    explicit: Option<&Path>,
) -> Result<LoadedConfig, ConfigError> {
    let path = explicit.map(Path::to_path_buf).or_else(|| discover(input));
    let Some(path) = path else {
        return Ok(LoadedConfig {
            config: ProjectConfig::default(),
            path: None,
            warnings: Vec::new(),
        });
    };
    let source = fs::read_to_string(&path).map_err(|error| ConfigError {
        path: path.clone(),
        message: format!("failed to read config: {error}"),
    })?;
    let ParsedConfig {
        mut config,
        warnings,
    } = parse_project_config(&source).map_err(|message| ConfigError {
        path: path.clone(),
        message,
    })?;
    config.config_dir = path
        .parent()
        .and_then(|directory| directory.canonicalize().ok());
    Ok(LoadedConfig {
        config,
        path: Some(path),
        warnings,
    })
}

/// The directory a project-config search starts from, for an input that is a
/// file rather than a directory.
///
/// `Path::parent()` of a bare relative filename is `Some("")`, not `None`, and
/// the empty path does not canonicalize. Treating that as "no project config"
/// made `lilscript main.lil` silently drop every setting in the
/// `lilscript.toml` sitting beside it while `lilscript ./main.lil` honoured it —
/// two different programs from one source, with no diagnostic. Every key was
/// affected, including `function_spelling` (which rebinds `this`) and
/// `strip_console` (which decides whether the program produces output at all).
fn config_search_parent(input: &Path) -> &Path {
    match input.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
}

fn discover(input: &Path) -> Option<PathBuf> {
    let start = if input.is_dir() {
        input
    } else {
        config_search_parent(input)
    };
    let mut directory = start.canonicalize().ok()?;
    loop {
        let candidate = directory.join("lilscript.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !directory.pop() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compilation_policy::{CompilationRequest, TacticId, TacticPermission};

    fn parse(source: &str) -> ParsedConfig {
        parse_project_config(source).unwrap()
    }

    fn refusal(source: &str) -> String {
        parse_project_config(source).unwrap_err()
    }

    /// A bare relative filename must find the same project config as the same
    /// file spelled `./name`. `Path::parent()` returns `Some("")` for the bare
    /// spelling, and the empty path does not canonicalize, so treating it as
    /// "no config" silently dropped every setting -- including `strip_console`,
    /// which decides whether the program produces output at all.
    #[test]
    fn a_bare_filename_searches_the_same_directory_as_a_dotted_one() {
        assert_eq!(config_search_parent(Path::new("main.lil")), Path::new("."));
        assert_eq!(
            config_search_parent(Path::new("./main.lil")),
            Path::new(".")
        );
        assert_eq!(
            config_search_parent(Path::new("ports/main.lil")),
            Path::new("ports")
        );
        assert_eq!(
            config_search_parent(Path::new("/abs/ports/main.lil")),
            Path::new("/abs/ports")
        );
    }

    #[test]
    fn the_route_selector_is_refused_with_an_actionable_message() {
        for route in ["semantic", "legacy"] {
            let error = refusal(&format!("[compiler]\nbackend = \"{route}\"\n"));
            assert!(
                error.contains("there is one compiler; remove [compiler] backend"),
                "{error}"
            );
        }
    }

    #[test]
    fn keys_with_no_effect_are_removed_with_one_warning_each() {
        let parsed = parse(
            "[compiler.resources]\nthreads=12\n[optimization]\nfinite_value_propagation=false\n\
             [javascript]\nfunction_scope=true\nlocal_name_reserve=48\n\
             [javascript.startup]\nparse_weight=1\n[mangle]\nexports=false\n\
             [profile]\nspecialization_min_count=50\n[native]\nstack_allocation=false\n",
        );
        assert_eq!(parsed.config, ProjectConfig::default());
        let keys = [
            "compiler.resources",
            "optimization.finite_value_propagation",
            "javascript.local_name_reserve",
            "javascript.function_scope",
            "javascript.startup",
            "mangle.exports",
            "profile",
            "native",
        ];
        assert_eq!(parsed.warnings.len(), keys.len(), "{:?}", parsed.warnings);
        for key in keys {
            assert!(
                parsed.warnings.iter().any(|warning| warning
                    .starts_with(&format!("`{key}` has no effect in this compiler: "))
                    && warning.ends_with("; remove it")),
                "{key}: {:?}",
                parsed.warnings
            );
        }
        assert!(parse("").warnings.is_empty());
    }

    /// The sibling line's knobs warn whatever their value, as every retired key does.
    #[test]
    fn sibling_line_knobs_are_retired_keys() {
        let parsed = parse(
            "[javascript]\nname_ordering = \"idiom-converged\"\nterminal_cleanup_chain = false\nwide_single_use_collapse = true\n",
        );
        assert_eq!(parsed.warnings.len(), 3, "{:?}", parsed.warnings);
        assert!(parsed.warnings[0].contains("javascript.name_ordering"));
        assert!(parsed.warnings[0].contains("migration/target-tree"));
    }

    #[test]
    fn runtime_priorities_and_constraints_are_refused() {
        for priority in [
            "performance-first",
            "realistic-performance-first",
            "realisticperf-first",
            "balanced",
        ] {
            let error = refusal(&format!("[javascript]\npriority = \"{priority}\"\n"));
            assert!(
                error.contains(&format!(
                    "`javascript.priority = \"{priority}\"` is refused"
                )),
                "{error}"
            );
            assert!(error.contains("size is the objective"), "{error}");
            assert!(error.contains("runtime estimators"), "{error}");
        }
        let size = parse("[javascript]\npriority = \"size-first\"\n");
        assert!(size.warnings.is_empty());
        assert_eq!(
            size.config.javascript.priority,
            JavaScriptPriority::SizeFirst
        );
        for limit in [
            "max_startup_work",
            "max_recurring_work",
            "max_runtime_memory_bytes",
            "max_performance_regression_percent",
        ] {
            let error = refusal(&format!("[policy.constraints]\n{limit} = 1\n"));
            assert!(error.contains("`policy.constraints` is refused"), "{error}");
            assert!(error.contains("runtime estimators"), "{error}");
        }
    }

    #[test]
    fn the_positional_public_shape_is_refused_and_named_has_no_effect() {
        let error = refusal("[javascript]\npublic_aggregate_abi = \"positional\"\n");
        assert!(
            error.contains("plain objects with named fields (D2)"),
            "{error}"
        );
        let named = parse("[javascript]\npublic_aggregate_abi = \"named\"\n");
        assert_eq!(named.warnings.len(), 1);
        assert!(named.warnings[0].contains("`javascript.public_aggregate_abi` has no effect"));
    }

    #[test]
    fn a_for_of_family_specialization_is_refused() {
        let error = refusal("[optimization]\nfor_of_specialize_family = 4\n");
        assert!(error.contains("for_of_specialize_family = 4"), "{error}");
        let zero = parse("[optimization]\nfor_of_specialize_family = 0\n");
        assert_eq!(zero.warnings.len(), 1);
    }

    /// Retired list entries are dropped with one warning per list; the entries
    /// this compiler reads keep their exact-allowlist meaning, and a misspelled
    /// entry is still an error.
    #[test]
    fn retired_list_entries_warn_and_the_read_entries_keep_their_meaning() {
        let parsed = parse(
            "[javascript]\ncompression = [\"identifier-mangling\", \"quote-style-selection\", \"loop-spelling-selection\"]\n\
             optimizations = [\"parsed-peephole\", \"call-site-specialization\"]\n",
        );
        assert_eq!(parsed.warnings.len(), 2, "{:?}", parsed.warnings);
        assert!(parsed.warnings[0].contains("`quote-style-selection`, `loop-spelling-selection`"));
        assert_eq!(
            parsed.config.javascript.compression,
            Some(vec![CompressionDecision::IdentifierMangling])
        );
        assert_eq!(
            parsed.config.javascript.optimizations,
            Some(vec![JavaScriptOptimization::CallSiteSpecialization])
        );
        let policy = parsed
            .config
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            })
            .unwrap();
        assert_eq!(
            policy.tactic(TacticId::IdentifierMangling).permission,
            TacticPermission::On
        );
        for omitted in [
            TacticId::PropertyMangling,
            TacticId::StringPooling,
            TacticId::NamingSearch,
        ] {
            assert_eq!(
                policy.tactic(omitted).permission,
                TacticPermission::Off,
                "{omitted:?}"
            );
        }
        assert_eq!(
            policy.tactic(TacticId::CallSpecialization).permission,
            TacticPermission::On
        );
        assert!(
            !policy
                .javascript_contract()
                .unwrap()
                .assumptions
                .numeric_lengths
        );
        assert!(
            parse_project_config("[javascript]\ncompression = [\"string-poolin\"]\n")
                .unwrap_err()
                .contains("unknown variant")
        );
    }

    #[test]
    fn an_emptied_retired_section_goes_with_its_keys() {
        let parsed = parse("[compiler]\n[compiler.resources]\ncodec_workers=8\n");
        assert_eq!(parsed.warnings.len(), 1);
        assert_eq!(parsed.config, ProjectConfig::default());
    }

    #[test]
    fn parses_javascript_strip_console() {
        let enabled = parse("[javascript]\nstrip_console=true\n").config;
        assert!(enabled.javascript.strip_console);
        let disabled = parse("[javascript]\nstrip_console=false\n").config;
        assert!(!disabled.javascript.strip_console);
        assert!(ProjectConfig::default().javascript.strip_console);
    }

    #[test]
    fn rejects_unknown_and_invalid_settings() {
        assert!(parse_project_config("[mangle]\nmagic=true").is_err());
        assert!(parse_project_config("[javascript]\nno_such_knob = true\n").is_err());
        assert!(parse_project_config("[bundle]\nmagic=0")
            .unwrap_err()
            .contains("bundle.magic"));
        assert!(
            parse_project_config("[delivery]\nentries={\"a/b\"=\"x.lil\"}")
                .unwrap_err()
                .contains("delivery.entries")
        );
        assert!(parse_project_config(
            "[javascript]\ncompression=['string-pooling','string-pooling']\n"
        )
        .unwrap_err()
        .contains("duplicate"));
        assert!(
            parse_project_config("[javascript]\noptimization_level=17\n")
                .unwrap_err()
                .contains("between 0 and 16")
        );
        assert!(
            parse_project_config("[javascript]\ncandidate_beam_width=0\n")
                .unwrap_err()
                .contains("candidate_beam_width")
        );
        assert!(
            parse_project_config("[javascript]\ncandidate_byte_budget=0\n")
                .unwrap_err()
                .contains("candidate_byte_budget")
        );
        assert!(parse_project_config(
            "[javascript]\noptimizations=['call-site-specialization','call-site-specialization']\n"
        )
        .unwrap_err()
        .contains("duplicate"));
    }

    #[test]
    fn resolves_javascript_ecmascript_and_browser_floors() {
        let defaults = ProjectConfig::default();
        assert_eq!(defaults.javascript.ecmascript, EcmaScriptEdition::Es2022);
        assert!(parse_project_config("[javascript]\necmascript='es2014'\n").is_err());
        assert!(parse_project_config("[javascript]\nbrowsers=['opera80']\n").is_err());
        let intersected =
            parse("[javascript]\necmascript='es2022'\nbrowsers=['chrome80','firefox78']\n").config;
        assert_eq!(
            intersected.javascript.resolved_ecmascript(),
            EcmaScriptEdition::Es2020
        );
    }

    #[test]
    fn effort_levels_set_the_search_breadth() {
        let disabled = parse("[javascript]\noptimization_level=0\ncandidate_limit=1536\n").config;
        assert_eq!(disabled.javascript.effective_candidate_limit(), 1);
        assert_eq!(disabled.javascript.effective_candidate_beam_width(), 1);
        assert_eq!(
            disabled.javascript.effective_candidate_byte_budget(),
            64 * 1024
        );
        assert_eq!(
            disabled.javascript.effective_terminal_codec_probe_limit(),
            0
        );
        assert_eq!(disabled.javascript.effective_candidate_proposal_limit(), 0);

        let standard = parse("[javascript]\noptimization_level=9\n").config;
        assert_eq!(standard.javascript.effective_candidate_limit(), 384);
        assert_eq!(standard.javascript.effective_candidate_beam_width(), 6);
        assert_eq!(
            standard.javascript.effective_candidate_byte_budget(),
            384 * 1024
        );
        assert_eq!(
            standard.javascript.effective_terminal_codec_probe_limit(),
            64
        );
        assert_eq!(
            standard.javascript.effective_candidate_proposal_limit(),
            384
        );

        let level_fourteen = parse("[javascript]\noptimization_level=14\n").config;
        assert_eq!(
            level_fourteen.javascript.effective_candidate_beam_width(),
            11
        );
        assert_eq!(
            level_fourteen.javascript.effective_candidate_byte_budget(),
            896 * 1024
        );
        assert_eq!(
            level_fourteen
                .javascript
                .effective_terminal_codec_probe_limit(),
            384
        );
    }

    #[test]
    fn proposal_defaults_follow_survivor_limits_but_explicit_work_is_independent() {
        let mut config = parse(
            "[javascript]\noptimization_level=15\ncandidate_search='always'\ncandidate_limit=2\n",
        )
        .config;
        assert_eq!(config.javascript.effective_candidate_limit(), 2);
        assert_eq!(config.javascript.effective_candidate_proposal_limit(), 2);
        config.javascript.candidate_proposal_limit = Some(23);
        assert_eq!(config.javascript.effective_candidate_proposal_limit(), 23);
        config.javascript.candidate_proposal_limit = Some(1);
        assert_eq!(config.javascript.effective_candidate_proposal_limit(), 1);
        config.javascript.optimization_level = 0;
        config.javascript.candidate_proposal_limit = Some(23);
        assert_eq!(
            config.javascript.effective_candidate_proposal_limit(),
            0,
            "an explicit proposal ceiling cannot bypass level zero"
        );
    }

    #[test]
    fn discovers_the_nearest_project_config_and_reports_its_warnings() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-config-discovery-test-{}",
            std::process::id()
        ));
        let nested = directory.join("src");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(
            directory.join("lilscript.toml"),
            "[mangle]\nidentifiers=false\n[javascript]\nstable_local_names=true\n",
        )
        .unwrap();
        let loaded = load_project_config(&nested.join("main.lil"), None).unwrap();
        assert_eq!(
            loaded.path,
            Some(directory.join("lilscript.toml").canonicalize().unwrap())
        );
        assert_eq!(loaded.config.mangle.identifiers, Some(false));
        assert_eq!(loaded.warnings.len(), 1);
        std::fs::write(
            directory.join("lilscript.toml"),
            "[compiler]\nbackend=\"legacy\"\n",
        )
        .unwrap();
        let error = load_project_config(&nested.join("main.lil"), None).unwrap_err();
        assert!(
            error.to_string().contains("there is one compiler"),
            "{error}"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn default_optimization_level_is_the_measured_effort_plateau() {
        // Guards the deliberate choice of 13 over the 0..=15 ceiling. Raising
        // this is a 20x compile-time decision on a large artifact, not a
        // tuning tweak, so it should not happen by accident.
        assert_eq!(JavaScriptConfig::default().optimization_level, 13);
    }

    /// Every key removed as having no effect names a key or table the
    /// configuration no longer declares: one the structs still accepted would
    /// be dropped instead of read.
    #[test]
    fn retired_keys_are_not_accepted_keys() {
        for &(key, retirement) in RETIRED_KEYS {
            if !matches!(
                retirement,
                Retirement::NoEffect(_) | Retirement::RefusedUnless { then: Some(_), .. }
            ) {
                continue;
            }
            let mut table = toml::Table::new();
            let path = key.split('.').collect::<Vec<_>>();
            let mut current = &mut table;
            for part in &path[..path.len() - 1] {
                current = current
                    .entry(part.to_string())
                    .or_insert_with(|| toml::Value::Table(toml::Table::new()))
                    .as_table_mut()
                    .unwrap();
            }
            current.insert(path[path.len() - 1].to_string(), toml::Value::Boolean(true));
            assert!(
                ProjectConfig::deserialize(table).is_err(),
                "`{key}` is retired but still deserializes"
            );
        }
    }
}
