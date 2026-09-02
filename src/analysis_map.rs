use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::codegen_ir_js::{
    IrJsBindingCategory, IrJsExportDecisionTrace, IrJsIdentifierDecisionTrace,
    IrJsIdentifierSelection, IrJsManglingOptionsTrace, IrJsPropertyCategory,
    IrJsPropertyDecisionTrace, IrJsPropertyStabilityReason, IrJsSourceTrace,
};
use crate::config::JavaScriptAnalysisMapLevel;
use crate::js_peephole::generated_javascript_export_names;
use crate::module::ModuleSet;
use crate::source_map::{
    selected_name_records, ComposedJavaScriptProvenance, DebugPositionIndex, OriginKind,
    SelectedNameRecord,
};
use crate::span::Span;

pub const JAVASCRIPT_ANALYSIS_MAP_VERSION: u32 = 1;

/// A deterministic, compiler-specific explanation sidecar for one exact
/// selected JavaScript artifact. It is intentionally separate from Source Map
/// v3 so ordinary debuggers remain standards-compatible and production
/// JavaScript never carries analysis bytes.
#[derive(Debug, Clone)]
pub struct JavaScriptAnalysisMap {
    json: String,
    artifact_sha256: String,
    decisions: usize,
    mangled: usize,
    coalesced: usize,
    level: JavaScriptAnalysisMapLevel,
}

impl JavaScriptAnalysisMap {
    pub fn as_str(&self) -> &str {
        &self.json
    }

    pub fn artifact_sha256(&self) -> &str {
        &self.artifact_sha256
    }

    pub const fn decision_count(&self) -> usize {
        self.decisions
    }

    pub const fn mangled_count(&self) -> usize {
        self.mangled
    }

    pub const fn coalesced_binding_count(&self) -> usize {
        self.coalesced
    }

    pub const fn level(&self) -> JavaScriptAnalysisMapLevel {
        self.level
    }

    pub fn matches_javascript(&self, javascript: &str) -> bool {
        content_hash(javascript.as_bytes()) == self.artifact_sha256
    }
}

impl PartialEq for JavaScriptAnalysisMap {
    fn eq(&self, other: &Self) -> bool {
        self.json == other.json
            && self.artifact_sha256 == other.artifact_sha256
            && self.decisions == other.decisions
            && self.mangled == other.mangled
            && self.coalesced == other.coalesced
            && self.level == other.level
    }
}

impl Eq for JavaScriptAnalysisMap {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AnalysisMapBuildError(pub String);

impl std::fmt::Display for AnalysisMapBuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for AnalysisMapBuildError {}

/// Selected-search facts included without retaining the candidate population.
/// These values explain the winning naming context while keeping map size and
/// capture work proportional to the winner rather than candidate count.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JavaScriptAnalysisSearchEvidence {
    pub strategy: String,
    pub objective: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_transfer_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidates_evaluated: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plans_registered: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emissions_attempted: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_codec_probes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_scope_naming_selected: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_scope_naming_incumbent_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_scope_naming_best_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_stop_reason: Option<String>,
    pub decision_registry_version: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnalysisDocument {
    version: u32,
    kind: &'static str,
    level: JavaScriptAnalysisMapLevel,
    coordinate_system: CoordinateSystem,
    artifact: ArtifactIdentity,
    compiler: CompilerIdentity,
    configuration: ConfigurationIdentity,
    sources: Vec<SourceIdentity>,
    search: JavaScriptAnalysisSearchEvidence,
    summary: AnalysisSummary,
    decisions: Vec<NameDecision>,
    notes: [&'static str; 3],
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CoordinateSystem {
    line_base: u8,
    column_base: u8,
    columns: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactIdentity {
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    sha256: String,
    bytes: usize,
    identity_scope: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompilerIdentity {
    name: &'static str,
    version: &'static str,
    analysis_schema_version: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfigurationIdentity {
    mangling_policy_sha256: String,
    mangling: ManglingPolicy,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ManglingPolicy {
    identifiers: bool,
    properties: bool,
    exports: bool,
    preserve_extern_fields: bool,
    owner_scoped_properties: bool,
    entropy_property_names: bool,
    stable_local_names: bool,
    frequency_order_local_names: bool,
    local_name_coalescing: bool,
    identifier_alphabet_first_symbols: usize,
    identifier_alphabet_rest_symbols: usize,
}

impl From<IrJsManglingOptionsTrace> for ManglingPolicy {
    fn from(options: IrJsManglingOptionsTrace) -> Self {
        Self {
            identifiers: options.identifiers,
            properties: options.properties,
            exports: options.exports,
            preserve_extern_fields: options.preserve_extern_fields,
            owner_scoped_properties: options.owner_scoped_properties,
            entropy_property_names: options.entropy_property_names,
            stable_local_names: options.stable_local_names,
            frequency_order_local_names: options.frequency_order_local_names,
            local_name_coalescing: options.local_name_coalescing,
            identifier_alphabet_first_symbols: options.alphabet_first,
            identifier_alphabet_rest_symbols: options.alphabet_rest,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceIdentity {
    path: String,
    sha256: String,
    bytes: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnalysisSummary {
    decisions: usize,
    identifiers: usize,
    properties: usize,
    exports: usize,
    mangled: usize,
    preserved: usize,
    coalesced_bindings: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
enum DecisionKind {
    Identifier,
    Property,
    Export,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum NameOutcome {
    Mangled,
    Preserved,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NameDecision {
    id: String,
    kind: DecisionKind,
    category: String,
    source: SourceName,
    generated: GeneratedName,
    outcome: NameOutcome,
    primary_rule: String,
    explanation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    owner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    slot: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    binding_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    coalesced_values: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rules: Option<Vec<RuleEvaluation>>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    evidence: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceName {
    name: String,
    path: String,
    line: u32,
    column: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeneratedName {
    name: String,
    first_line: u32,
    first_column: u32,
    occurrences: usize,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
enum RuleResult {
    Matched,
    NotMatched,
    Applied,
    Skipped,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuleEvaluation {
    rule: &'static str,
    result: RuleResult,
    detail: String,
}

pub(crate) struct AnalysisMapBuildContext<'a> {
    pub generated: &'a str,
    pub file_name: Option<&'a str>,
    pub trace: &'a IrJsSourceTrace,
    pub composed: &'a ComposedJavaScriptProvenance,
    pub modules: &'a ModuleSet,
    pub source_base: Option<&'a Path>,
    pub level: JavaScriptAnalysisMapLevel,
    pub search: JavaScriptAnalysisSearchEvidence,
}

pub(crate) fn build_javascript_analysis_map(
    context: AnalysisMapBuildContext<'_>,
) -> Result<JavaScriptAnalysisMap, AnalysisMapBuildError> {
    if !context.level.enabled() {
        return Err(AnalysisMapBuildError(
            "analysis-map builder requires summary or full detail".to_string(),
        ));
    }
    let mangling = context.trace.mangling.as_ref().ok_or_else(|| {
        AnalysisMapBuildError("selected JavaScript is missing mangling analysis".to_string())
    })?;
    let positions =
        DebugPositionIndex::new(context.generated, context.modules, context.source_base);
    let selected_names = selected_name_records(context.composed);
    let full = context.level == JavaScriptAnalysisMapLevel::Full;
    let mut decisions = Vec::new();
    for selected in &selected_names {
        if selected.kind == OriginKind::Property {
            if let Some(trace) = match_property_trace(selected, &mangling.properties) {
                decisions.push(property_decision(
                    selected,
                    trace,
                    mangling.options,
                    &positions,
                    full,
                ));
            }
        } else if let Some(category) = origin_binding_category(selected.kind) {
            if let Some(trace) = match_identifier_trace(selected, category, &mangling.identifiers) {
                decisions.push(identifier_decision(
                    selected,
                    trace,
                    mangling.options,
                    &positions,
                    full,
                ));
            }
        }
    }
    let exported = generated_javascript_export_names(context.generated)
        .map_err(|error| {
            AnalysisMapBuildError(format!("cannot inspect selected exports: {error}"))
        })?
        .into_iter()
        .collect::<BTreeSet<_>>();
    for export in &mangling.exports {
        if let Some(decision) = export_decision(
            context.generated,
            export,
            &selected_names,
            &exported,
            &positions,
            full,
        ) {
            decisions.push(decision);
        }
    }
    decisions.sort_by(|left, right| {
        (
            left.kind,
            &left.source.path,
            left.source.line,
            left.source.column,
            &left.source.name,
            &left.generated.name,
        )
            .cmp(&(
                right.kind,
                &right.source.path,
                right.source.line,
                right.source.column,
                &right.source.name,
                &right.generated.name,
            ))
    });
    let mut merged = Vec::<NameDecision>::with_capacity(decisions.len());
    for decision in decisions {
        if let Some(existing) = merged
            .last_mut()
            .filter(|existing| existing.id == decision.id)
        {
            existing.generated.occurrences = existing
                .generated
                .occurrences
                .saturating_add(decision.generated.occurrences);
            if (
                decision.generated.first_line,
                decision.generated.first_column,
            ) < (
                existing.generated.first_line,
                existing.generated.first_column,
            ) {
                existing.generated.first_line = decision.generated.first_line;
                existing.generated.first_column = decision.generated.first_column;
            }
        } else {
            merged.push(decision);
        }
    }
    let decisions = merged;

    let summary = AnalysisSummary {
        decisions: decisions.len(),
        identifiers: decisions
            .iter()
            .filter(|decision| decision.kind == DecisionKind::Identifier)
            .count(),
        properties: decisions
            .iter()
            .filter(|decision| decision.kind == DecisionKind::Property)
            .count(),
        exports: decisions
            .iter()
            .filter(|decision| decision.kind == DecisionKind::Export)
            .count(),
        mangled: decisions
            .iter()
            .filter(|decision| matches!(decision.outcome, NameOutcome::Mangled))
            .count(),
        preserved: decisions
            .iter()
            .filter(|decision| matches!(decision.outcome, NameOutcome::Preserved))
            .count(),
        coalesced_bindings: decisions
            .iter()
            .filter(|decision| decision.coalesced_values.is_some_and(|count| count > 1))
            .count(),
    };
    let policy = ManglingPolicy::from(mangling.options);
    let policy_json = serde_json::to_vec(&policy).map_err(|error| {
        AnalysisMapBuildError(format!("cannot fingerprint mangling policy: {error}"))
    })?;
    let sources = context
        .modules
        .modules
        .iter()
        .enumerate()
        .map(|(source_id, module)| SourceIdentity {
            path: positions.source_path(source_id).to_string(),
            sha256: content_hash(module.source.as_bytes()),
            bytes: module.source.len(),
        })
        .collect();
    let artifact_sha256 = content_hash(context.generated.as_bytes());
    let document = AnalysisDocument {
        version: JAVASCRIPT_ANALYSIS_MAP_VERSION,
        kind: "lilscript-javascript-analysis-map",
        level: context.level,
        coordinate_system: CoordinateSystem {
            line_base: 0,
            column_base: 0,
            columns: "utf-16-code-units",
        },
        artifact: ArtifactIdentity {
            file: context.file_name.map(str::to_string),
            sha256: artifact_sha256.clone(),
            bytes: context.generated.len(),
            identity_scope: "selected-javascript-before-publication-comments",
        },
        compiler: CompilerIdentity {
            name: "lilscript",
            version: env!("CARGO_PKG_VERSION"),
            analysis_schema_version: JAVASCRIPT_ANALYSIS_MAP_VERSION,
        },
        configuration: ConfigurationIdentity {
            mangling_policy_sha256: content_hash(&policy_json),
            mangling: policy,
        },
        sources,
        search: context.search,
        summary,
        decisions,
        notes: [
            "The artifact hash covers selected JavaScript before linked/inline source-map comments.",
            "Only retained names have decisions; eliminated source constructs have no generated name.",
            "Rule identifiers are semantic schema values, not Rust control-flow or log strings.",
        ],
    };
    let decisions = document.summary.decisions;
    let mangled = document.summary.mangled;
    let coalesced = document.summary.coalesced_bindings;
    let json = serde_json::to_string(&document).map_err(|error| {
        AnalysisMapBuildError(format!("cannot serialize JavaScript analysis map: {error}"))
    })?;
    Ok(JavaScriptAnalysisMap {
        json,
        artifact_sha256,
        decisions,
        mangled,
        coalesced,
        level: context.level,
    })
}

fn identifier_decision(
    selected: &SelectedNameRecord,
    trace: &IrJsIdentifierDecisionTrace,
    options: IrJsManglingOptionsTrace,
    positions: &DebugPositionIndex<'_>,
    full: bool,
) -> NameDecision {
    let (path, line, column) = positions.source(trace.span);
    let (first_line, first_column) = positions.generated(selected.first_generated);
    let (primary_rule, explanation) =
        identifier_primary_reason(trace, options, &selected.generated);
    let mut evidence = BTreeMap::new();
    evidence.insert(
        "selection".to_string(),
        serde_json::Value::String(identifier_selection_name(trace.selection).to_string()),
    );
    if trace.emitted != selected.generated {
        evidence.insert(
            "winnerReplaySpelling".to_string(),
            serde_json::Value::String(trace.emitted.clone()),
        );
    }
    NameDecision {
        id: decision_id(
            DecisionKind::Identifier,
            &path,
            line,
            column,
            &selected.original,
            &selected.generated,
        ),
        kind: DecisionKind::Identifier,
        category: origin_kind_name(selected.kind).to_string(),
        source: SourceName {
            name: selected.original.clone(),
            path,
            line,
            column,
        },
        generated: GeneratedName {
            name: selected.generated.clone(),
            first_line,
            first_column,
            occurrences: selected.occurrences,
        },
        outcome: name_outcome(&selected.original, &selected.generated),
        primary_rule: primary_rule.to_string(),
        explanation,
        owner: None,
        slot: None,
        binding_source: None,
        coalesced_values: (trace.coalesced_values > 1).then_some(trace.coalesced_values),
        rules: full.then(|| identifier_rules(trace, options, &selected.generated)),
        evidence,
    }
}

fn property_decision(
    selected: &SelectedNameRecord,
    trace: &IrJsPropertyDecisionTrace,
    options: IrJsManglingOptionsTrace,
    positions: &DebugPositionIndex<'_>,
    full: bool,
) -> NameDecision {
    let source_span = trace.provenance.span.unwrap_or(selected.span);
    let (path, line, column) = positions.source(source_span);
    let (first_line, first_column) = positions.generated(selected.first_generated);
    let (primary_rule, explanation) = property_primary_reason(trace, options, &selected.generated);
    let mut evidence = BTreeMap::new();
    evidence.insert(
        "propertyCategory".to_string(),
        serde_json::Value::String(property_category_name(trace.provenance.category).to_string()),
    );
    NameDecision {
        id: decision_id(
            DecisionKind::Property,
            &path,
            line,
            column,
            &selected.original,
            &selected.generated,
        ),
        kind: DecisionKind::Property,
        category: property_category_name(trace.provenance.category).to_string(),
        source: SourceName {
            name: selected.original.clone(),
            path,
            line,
            column,
        },
        generated: GeneratedName {
            name: selected.generated.clone(),
            first_line,
            first_column,
            occurrences: selected.occurrences,
        },
        outcome: name_outcome(&selected.original, &selected.generated),
        primary_rule: primary_rule.to_string(),
        explanation,
        owner: trace.provenance.owner.clone(),
        slot: trace.provenance.slot,
        binding_source: None,
        coalesced_values: None,
        rules: full.then(|| property_rules(trace, options, &selected.generated)),
        evidence,
    }
}

fn export_decision(
    generated: &str,
    export: &IrJsExportDecisionTrace,
    selected: &[SelectedNameRecord],
    exported: &BTreeSet<String>,
    positions: &DebugPositionIndex<'_>,
    full: bool,
) -> Option<NameDecision> {
    let generated_public = if export.mangling_enabled {
        selected
            .iter()
            .filter(|record| record.original == export.binding_source)
            .filter(|record| origin_binding_category(record.kind) == Some(export.category))
            .min_by_key(|record| span_distance(record.span, export.span))
            .map(|record| record.generated.clone())
            .unwrap_or_else(|| export.public.clone())
    } else {
        export.public.clone()
    };
    if !exported.contains(&generated_public) {
        return None;
    }
    let (path, line, column) = positions.source(export.span);
    let first_generated = find_identifier(generated, &generated_public).unwrap_or(0);
    let (first_line, first_column) = positions.generated(first_generated);
    let outcome = name_outcome(&export.source, &generated_public);
    let (primary_rule, explanation) = if export.mangling_enabled {
        (
            "export.internal-binding-surfaced",
            format!(
                "Export mangling exposed selected internal binding `{generated_public}` as the public ESM name."
            ),
        )
    } else {
        (
            "export.public-name-preserved",
            format!(
                "The JavaScript boundary preserved declared export name `{}`.",
                export.source
            ),
        )
    };
    let rules = full.then(|| {
        vec![
            rule(
                "export.javascript-boundary",
                RuleResult::Applied,
                "The retained binding crosses an ESM boundary.",
            ),
            rule(
                "export.mangling-enabled",
                if export.mangling_enabled {
                    RuleResult::Matched
                } else {
                    RuleResult::NotMatched
                },
                if export.mangling_enabled {
                    "This export boundary permits the internal selected name to become public."
                } else {
                    "This export boundary requires the declared public spelling."
                },
            ),
            rule(
                if export.mangling_enabled {
                    "export.internal-binding-surfaced"
                } else {
                    "export.public-name-preserved"
                },
                RuleResult::Applied,
                format!("Selected public spelling `{generated_public}`."),
            ),
        ]
    });
    let mut evidence = BTreeMap::new();
    evidence.insert(
        "selectedInternal".to_string(),
        serde_json::Value::String(export.internal.clone()),
    );
    Some(NameDecision {
        id: decision_id(
            DecisionKind::Export,
            &path,
            line,
            column,
            &export.source,
            &generated_public,
        ),
        kind: DecisionKind::Export,
        category: origin_category_name(export.category).to_string(),
        source: SourceName {
            name: export.source.clone(),
            path,
            line,
            column,
        },
        generated: GeneratedName {
            name: generated_public,
            first_line,
            first_column,
            occurrences: 1,
        },
        outcome,
        primary_rule: primary_rule.to_string(),
        explanation,
        owner: None,
        slot: None,
        binding_source: Some(export.binding_source.clone()),
        coalesced_values: None,
        rules,
        evidence,
    })
}

fn identifier_primary_reason(
    trace: &IrJsIdentifierDecisionTrace,
    options: IrJsManglingOptionsTrace,
    selected: &str,
) -> (&'static str, String) {
    if trace.coalesced_values > 1 {
        return (
            "identifier.noninterfering-values-coalesced",
            format!(
                "{} noninterfering SSA values share `{}` after lexical-liveness coalescing.",
                trace.coalesced_values, selected
            ),
        );
    }
    match trace.selection {
        IrJsIdentifierSelection::ExternalAbi => (
            "identifier.external-abi",
            if trace.source == selected {
                format!("External JavaScript binding `{selected}` retained its local ABI spelling.")
            } else {
                format!(
                    "External JavaScript binding `{}` received collision-free local alias `{selected}`.",
                    trace.source
                )
            },
        ),
        IrJsIdentifierSelection::PublicAbi => (
            "identifier.public-identity-abi",
            format!(
                "Public constructible identity `{}` is ABI-stable.",
                selected
            ),
        ),
        IrJsIdentifierSelection::ReadableHygienic => (
            "identifier.mangling-disabled",
            format!(
                "Identifier mangling is disabled; `{}` is the collision-free readable spelling.",
                selected
            ),
        ),
        IrJsIdentifierSelection::StableLocalPreference => (
            "identifier.stable-local-preference",
            format!(
                "Cross-scope stable-local preference selected reusable compact spelling `{}`.",
                selected
            ),
        ),
        IrJsIdentifierSelection::FrequencyRanked => (
            "identifier.frequency-ranked",
            format!(
                "Weighted use-frequency ordering assigned compact spelling `{}`.",
                selected
            ),
        ),
        IrJsIdentifierSelection::CompactSequential => (
            if options.identifiers {
                "identifier.compact-sequential"
            } else {
                "identifier.mangling-disabled"
            },
            format!(
                "The deterministic legal-name allocator selected `{}`.",
                selected
            ),
        ),
    }
}

fn property_primary_reason(
    trace: &IrJsPropertyDecisionTrace,
    options: IrJsManglingOptionsTrace,
    selected: &str,
) -> (&'static str, String) {
    if let Some(reason) = trace.stability {
        let (rule, description) = stability_rule(reason);
        return (
            rule,
            format!(
                "Property `{}` stayed stable because {description}.",
                trace.provenance.source
            ),
        );
    }
    if options.owner_scoped_properties && trace.provenance.owner.is_some() {
        (
            "property.owner-scoped-frequency-ranked",
            format!(
                "Closed owner `{}` ranked field `{}` within its independent property namespace.",
                trace.provenance.owner.as_deref().unwrap_or(""),
                trace.provenance.source
            ),
        )
    } else {
        (
            "property.shared-frequency-ranked",
            format!(
                "Weighted property frequency selected compact spelling `{}` for `{}`.",
                selected, trace.provenance.source
            ),
        )
    }
}

fn identifier_rules(
    trace: &IrJsIdentifierDecisionTrace,
    options: IrJsManglingOptionsTrace,
    selected: &str,
) -> Vec<RuleEvaluation> {
    vec![
        condition_rule(
            "identifier.external-abi",
            trace.selection == IrJsIdentifierSelection::ExternalAbi,
            "Imported and ambient bindings remain tied to their external ABI while local aliases stay hygienic.",
        ),
        condition_rule(
            "identifier.public-identity-abi",
            trace.selection == IrJsIdentifierSelection::PublicAbi,
            "Published constructible/class identity can pin a binding spelling.",
        ),
        condition_rule(
            "identifier.mangling-enabled",
            options.identifiers,
            "Private identifier mangling is enabled for the selected emission plan.",
        ),
        rule(
            "identifier.mangling-disabled",
            if trace.selection == IrJsIdentifierSelection::ReadableHygienic {
                RuleResult::Applied
            } else {
                RuleResult::Skipped
            },
            "Readable source-derived names are allocated hygienically when identifier mangling is disabled.",
        ),
        rule(
            "identifier.noninterfering-values-coalesced",
            if options.local_name_coalescing && trace.coalesced_values > 1 {
                RuleResult::Applied
            } else {
                RuleResult::Skipped
            },
            format!(
                "The selected binding represents {} named SSA value(s).",
                trace.coalesced_values
            ),
        ),
        rule(
            "identifier.stable-local-preference",
            if !options.stable_local_names {
                RuleResult::Skipped
            } else if trace.selection == IrJsIdentifierSelection::StableLocalPreference {
                RuleResult::Applied
            } else {
                RuleResult::NotMatched
            },
            "Stable-local preferences are claimed only when legal and collision-free.",
        ),
        rule(
            "identifier.frequency-ranked",
            if trace.selection == IrJsIdentifierSelection::FrequencyRanked {
                RuleResult::Applied
            } else if options.frequency_order_local_names {
                RuleResult::NotMatched
            } else {
                RuleResult::Skipped
            },
            "More frequently used retained bindings/colors receive earlier compact names.",
        ),
        rule(
            "identifier.compact-sequential",
            if trace.selection == IrJsIdentifierSelection::CompactSequential {
                RuleResult::Applied
            } else {
                RuleResult::Skipped
            },
            "The deterministic allocator assigns the next legal compact spelling when no stronger preference wins.",
        ),
        rule(
            "identifier.reserved-and-capture-exclusion",
            RuleResult::Applied,
            "Reserved words, occupied names, and lexical capture hazards were excluded.",
        ),
        rule(
            "identifier.selected-spelling",
            RuleResult::Applied,
            format!("Final selected-artifact spelling is `{selected}`."),
        ),
    ]
}

fn property_rules(
    trace: &IrJsPropertyDecisionTrace,
    options: IrJsManglingOptionsTrace,
    selected: &str,
) -> Vec<RuleEvaluation> {
    let checks = [
        (
            IrJsPropertyStabilityReason::ManglingDisabled,
            "property.mangling-disabled",
        ),
        (
            IrJsPropertyStabilityReason::PrototypeSensitive,
            "property.prototype-sensitive",
        ),
        (
            IrJsPropertyStabilityReason::ExternalAbi,
            "property.external-abi",
        ),
        (
            IrJsPropertyStabilityReason::DynamicBoundary,
            "property.dynamic-boundary",
        ),
        (
            IrJsPropertyStabilityReason::PublicAggregateAbi,
            "property.public-aggregate-abi",
        ),
        (
            IrJsPropertyStabilityReason::HostMember,
            "property.host-member",
        ),
        (
            IrJsPropertyStabilityReason::HostMethod,
            "property.host-method",
        ),
        (
            IrJsPropertyStabilityReason::UnownedKeySafety,
            "property.unowned-key-safety",
        ),
    ];
    let mut rules = vec![condition_rule(
        "property.mangling-enabled",
        options.properties,
        "Property mangling is enabled for the selected emission plan.",
    )];
    rules.extend(checks.into_iter().map(|(reason, id)| {
        condition_rule(
            id,
            trace.stability == Some(reason),
            stability_rule(reason).1,
        )
    }));
    rules.push(rule(
        "property.owner-scoped-namespace",
        if options.owner_scoped_properties && trace.provenance.owner.is_some() {
            RuleResult::Applied
        } else {
            RuleResult::Skipped
        },
        "Closed owners may reuse short property names in independent namespaces.",
    ));
    rules.push(rule(
        "property.weighted-frequency-order",
        if trace.stability.is_none() && options.properties {
            RuleResult::Applied
        } else {
            RuleResult::Skipped
        },
        "Loop-weighted retained uses rank eligible properties before allocation.",
    ));
    let owner_scoped = trace.stability.is_none()
        && options.properties
        && options.owner_scoped_properties
        && trace.provenance.owner.is_some();
    rules.push(rule(
        "property.owner-scoped-frequency-ranked",
        if owner_scoped {
            RuleResult::Applied
        } else {
            RuleResult::Skipped
        },
        "The field was ranked within its closed owner's independent property namespace.",
    ));
    rules.push(rule(
        "property.shared-frequency-ranked",
        if trace.stability.is_none() && options.properties && !owner_scoped {
            RuleResult::Applied
        } else {
            RuleResult::Skipped
        },
        "The field was ranked in the shared property namespace.",
    ));
    rules.push(rule(
        "property.selected-spelling",
        RuleResult::Applied,
        format!("Final selected-artifact spelling is `{selected}`."),
    ));
    rules
}

fn condition_rule(id: &'static str, matched: bool, detail: impl Into<String>) -> RuleEvaluation {
    rule(
        id,
        if matched {
            RuleResult::Matched
        } else {
            RuleResult::NotMatched
        },
        detail,
    )
}

fn rule(id: &'static str, result: RuleResult, detail: impl Into<String>) -> RuleEvaluation {
    RuleEvaluation {
        rule: id,
        result,
        detail: detail.into(),
    }
}

fn match_identifier_trace<'a>(
    selected: &SelectedNameRecord,
    category: IrJsBindingCategory,
    traces: &'a [IrJsIdentifierDecisionTrace],
) -> Option<&'a IrJsIdentifierDecisionTrace> {
    traces
        .iter()
        .filter(|trace| trace.source == selected.original && trace.category == category)
        .min_by_key(|trace| {
            (
                usize::from(trace.emitted != selected.generated),
                span_distance(trace.span, selected.span),
            )
        })
}

fn match_property_trace<'a>(
    selected: &SelectedNameRecord,
    traces: &'a [IrJsPropertyDecisionTrace],
) -> Option<&'a IrJsPropertyDecisionTrace> {
    traces
        .iter()
        .filter(|trace| trace.provenance.source == selected.original)
        .min_by_key(|trace| {
            let span = trace.provenance.span.unwrap_or(selected.span);
            (
                usize::from(trace.provenance.emitted != selected.generated),
                span_distance(span, selected.span),
                usize::from(trace.provenance.owner.is_none()),
            )
        })
}

const fn origin_binding_category(kind: OriginKind) -> Option<IrJsBindingCategory> {
    match kind {
        OriginKind::Function => Some(IrJsBindingCategory::Function),
        OriginKind::Global => Some(IrJsBindingCategory::Global),
        OriginKind::Parameter => Some(IrJsBindingCategory::Parameter),
        OriginKind::Local => Some(IrJsBindingCategory::Local),
        OriginKind::Temporary => Some(IrJsBindingCategory::Temporary),
        OriginKind::Instruction | OriginKind::Property => None,
    }
}

fn span_distance(left: Span, right: Span) -> usize {
    if left.start <= right.end && right.start <= left.end {
        0
    } else {
        left.start.abs_diff(right.start)
    }
}

fn name_outcome(original: &str, generated: &str) -> NameOutcome {
    if original == generated {
        NameOutcome::Preserved
    } else {
        NameOutcome::Mangled
    }
}

const fn origin_kind_name(kind: OriginKind) -> &'static str {
    match kind {
        OriginKind::Function => "function",
        OriginKind::Global => "global",
        OriginKind::Parameter => "parameter",
        OriginKind::Local => "local",
        OriginKind::Temporary => "temporary",
        OriginKind::Instruction => "instruction",
        OriginKind::Property => "property",
    }
}

const fn origin_category_name(category: IrJsBindingCategory) -> &'static str {
    match category {
        IrJsBindingCategory::Function => "function",
        IrJsBindingCategory::Global => "global",
        IrJsBindingCategory::Parameter => "parameter",
        IrJsBindingCategory::Local => "local",
        IrJsBindingCategory::Temporary => "temporary",
    }
}

const fn property_category_name(category: IrJsPropertyCategory) -> &'static str {
    match category {
        IrJsPropertyCategory::Owned => "owned",
        IrJsPropertyCategory::External => "external",
        IrJsPropertyCategory::Unowned => "unowned",
    }
}

const fn identifier_selection_name(selection: IrJsIdentifierSelection) -> &'static str {
    match selection {
        IrJsIdentifierSelection::ExternalAbi => "external-abi",
        IrJsIdentifierSelection::PublicAbi => "public-abi",
        IrJsIdentifierSelection::ReadableHygienic => "readable-hygienic",
        IrJsIdentifierSelection::StableLocalPreference => "stable-local-preference",
        IrJsIdentifierSelection::FrequencyRanked => "frequency-ranked",
        IrJsIdentifierSelection::CompactSequential => "compact-sequential",
    }
}

const fn stability_rule(reason: IrJsPropertyStabilityReason) -> (&'static str, &'static str) {
    match reason {
        IrJsPropertyStabilityReason::ManglingDisabled => (
            "property.mangling-disabled",
            "property mangling is disabled",
        ),
        IrJsPropertyStabilityReason::PrototypeSensitive => (
            "property.prototype-sensitive",
            "the key can affect JavaScript prototype semantics",
        ),
        IrJsPropertyStabilityReason::ExternalAbi => (
            "property.external-abi",
            "the extern-class contract pins the host-visible key",
        ),
        IrJsPropertyStabilityReason::DynamicBoundary => (
            "property.dynamic-boundary",
            "the aggregate crosses a dynamic/untyped boundary",
        ),
        IrJsPropertyStabilityReason::PublicAggregateAbi => (
            "property.public-aggregate-abi",
            "the reusable-module aggregate ABI exposes the named field",
        ),
        IrJsPropertyStabilityReason::HostMember => (
            "property.host-member",
            "a host member access requires the JavaScript spelling",
        ),
        IrJsPropertyStabilityReason::HostMethod => (
            "property.host-method",
            "a host method call requires the JavaScript spelling",
        ),
        IrJsPropertyStabilityReason::UnownedKeySafety => (
            "property.unowned-key-safety",
            "the compiler cannot prove a closed owned-key namespace",
        ),
    }
}

fn decision_id(
    kind: DecisionKind,
    path: &str,
    line: u32,
    column: u32,
    original: &str,
    generated: &str,
) -> String {
    let material = format!(
        "v{JAVASCRIPT_ANALYSIS_MAP_VERSION}:{kind:?}:{path}:{line}:{column}:{original}:{generated}"
    );
    format!("name-{}", &content_hash(material.as_bytes())[..16])
}

fn find_identifier(source: &str, name: &str) -> Option<usize> {
    source.match_indices(name).find_map(|(start, _)| {
        let end = start + name.len();
        let before = source[..start].chars().next_back();
        let after = source[end..].chars().next();
        (!before.is_some_and(identifier_continue) && !after.is_some_and(identifier_continue))
            .then_some(start)
    })
}

fn identifier_continue(character: char) -> bool {
    character == '_' || character == '$' || character.is_alphanumeric()
}

fn content_hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(encoded, "{byte:02x}").expect("writing a digest to String cannot fail");
    }
    encoded
}
