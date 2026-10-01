//! Resolved compilation policy: language/delivery constraints, permitted
//! implementations and resource accounting have separate owners.
//!
//! Legacy options enter through `ProjectConfig::resolve_policy`; new compiler
//! clients consume this immutable result. The compatibility adapters do not
//! certify that an old producer has migrated to candidate admission.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::compilation_contract::JavaScriptCompilationContract;
use crate::config::{CompressionCostModel, JavaScriptPriority};

pub const POLICY_SCHEMA_VERSION: u32 = 3;
/// Omitted policy versions retain the old effort-16 permission contract.
/// New configurations select version 3 explicitly; no file changes silently.
pub const LEGACY_POLICY_VERSION: u32 = 2;
// Version15 unifies representation families and admits per-site call/joint provenance.
// Version14 completes registry admission and versioned explicit risk permissions.
// Version13 scopes all tree-only reach consumers.
// Version12 bounds reach lists once and summarizes references per binding.
// Version11 admits target reach backing and omits unrequested capture facts.
// Version10 resolves configured objective sets and two-objective requests.
// Version9 resolves independent objective searches with shared baseline admission.
// Version8 reuses admitted statement mention facts during target forwarding.
// Version7 jointly identifies private-field and integer-hint formation heads.
// Version16 admits live printed-order allocation and delivered-raw tie breaks.
// Version17 includes class constructors/methods in live naming ownership.
// Version18 names private generic/kept layouts and reuses sibling property slots.
// Version19 forms stable per-site spellings, declaration order and receiver aliases.
// Version32 preserves erased dispatch and fresh captured expression cells.
// Version33 resolves explicit host paths and native provider bindings.
// Version34 qualifies closed erased product transport across private bodies.
// Version35 adapts concrete public callbacks with closed invocation uses.
// Version38 honors authored loop expansion with an independent policy permission.
// Version 40 carries regional representation pins through source/target edits.
// Version51 admits delivery placement, trials, simulation and setter payloads.
// Version55 admits shared source analyses and deterministic effect-summary stages.
// Version56 admits primitive/aggregate/default facts and forwarding/liveness plans.
// Version59 admits exact-value/evaluator storage and local initialization scratch.
pub const POLICY_ALGORITHM_VERSION: u32 = 59;
// Version22 admits state reclamation visits, including physical artifact slots,
// instead of reserving a worst-case Cartesian scan before any inspection.
// Version18 admits and releases Analyzer scope and callable-context backing.
// Version17 admits canonical checker declaration vector backing and growth.
// Version16 admits parser lookahead probes through the existing parser owner.
// Version15 owns lexical input work per stream, including nested fragments.
// Version14 admits checker interfaces and graph validation/initialization work.
// Version13 admits template storage, removes heap source identities and shares
// direct target formation across the baseline/optional naming boundary.
// Version12 added parse-once discovery with admitted stable source storage.
// Resource identity retains version8's complete checked rewrite descriptions.
// Structural cursor/beam scheduling remains the qualified version3 algorithm.
// Version26 reserves half the optional work for the level walk: the
// structural search stops at its share, so a program with many optional
// alternatives cannot starve the walk (AM2).
// Version27 makes terminal proxy pruning/auditing explicit and fingerprinted.
// Version28 admits deferred naming starts after protecting the completed walk.
// Version29 gates deferred naming at 14 by default and combines it with final refinements.
// Version30 adds common per-site moves and a protected, configurable joint tail.
// Version31 protects the prior search before compact allocation.
pub const SEARCH_SCHEDULE_VERSION: u32 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilationRequest {
    JavaScript { preserve_root_exports: bool },
    Native,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompilationContract {
    JavaScript {
        language: JavaScriptCompilationContract,
        preserved_properties: Vec<String>,
        /// How the program is placed in files, and their container.
        delivery: DeliveryContract,
    },
    Native {
        abi_version: u32,
    },
}

/// The delivery part of the contract (plan M3.3, architecture §14
/// `[delivery]`): what the delivered files must be. Placement itself is the
/// compiler's; it never depends on the format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryContract {
    pub mode: crate::config::DeliveryMode,
    pub format: crate::config::JavaScriptFormat,
    /// Which lazily loaded files an entry preloads; `None` for one file.
    pub preload: crate::config::PreloadPolicy,
    /// File name templates (`js::names`); `None` is the default, which
    /// costs the contract no storage.
    pub entry_names: Option<String>,
    pub chunk_names: Option<String>,
    pub module_names: Option<String>,
    /// The world is a library: chunk names default to their plan position,
    /// the shortest delivered bytes; an application's to a content hash,
    /// which caches safely across releases (design §8).
    pub library: bool,
    /// Declared deployment costs (L12), added to an entry's row.
    pub request_bytes: u64,
    pub depth_bytes: u64,
    /// Whether relative host modules travel with the output: they change
    /// the delivered files, so the contract and its fingerprint hold it.
    pub host_modules: crate::config::HostModules,
}

impl DeliveryContract {
    /// Today's single-file output: one entry file, no plan.
    pub fn single() -> Self {
        Self {
            mode: crate::config::DeliveryMode::Single,
            format: crate::config::JavaScriptFormat::Esm,
            preload: crate::config::PreloadPolicy::None,
            entry_names: None,
            chunk_names: None,
            module_names: None,
            library: true,
            request_bytes: 0,
            depth_bytes: 0,
            host_modules: crate::config::HostModules::External,
        }
    }
    pub fn entry_names(&self) -> &str {
        self.entry_names.as_deref().unwrap_or("[name].[ext]")
    }
    pub fn chunk_names(&self) -> &str {
        self.chunk_names.as_deref().unwrap_or(if self.library {
            "[index].[ext]"
        } else {
            "[hash:8].[ext]"
        })
    }
    pub fn module_names(&self) -> &str {
        self.module_names.as_deref().unwrap_or("[path].[ext]")
    }
    /// The templates this contract stores, for its payload.
    pub fn templates(&self) -> impl Iterator<Item = &String> {
        [&self.entry_names, &self.chunk_names, &self.module_names]
            .into_iter()
            .flatten()
    }
}

/// This is a permission to compete, never a forced representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TacticPermission {
    #[default]
    Auto,
    On,
    Off,
}

#[path = "compilation_tactics.rs"]
mod tactics;
pub use tactics::{
    AnalysisRequirement, TacticDefault, TacticId, TacticInvalidation, TacticProducer, TacticSpec,
};

#[cfg(test)]
#[path = "compilation_tactics_tests.rs"]
mod tactic_tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ResolvedTactic {
    pub permission: TacticPermission,
    pub enabled: bool,
}

/// Explanation only: equivalent alias/canonical requests share a fingerprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TacticOrigin {
    Default,
    Preset,
    LegacyAlias,
    Policy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeRisk {
    #[default]
    Neutral,
    Startup,
    Recurring,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TacticUse {
    pub tactic: TacticId,
    pub risk: RuntimeRisk,
}

/// Static work/risk estimates, not measured milliseconds or a runtime promise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CandidateCost {
    pub transfer_bytes: u64,
    pub performance_score: u64,
    pub startup_work: u64,
    pub recurring_work: u64,
    pub runtime_memory_bytes: u64,
}

/// Unavailable runtime evidence is distinct from a proved/estimated zero.
/// Transfer size is exact; runtime fields are qualified static estimates, not
/// measured milliseconds or a promise about arbitrary host implementations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CandidateCostEvidence {
    pub transfer_bytes: u64,
    pub performance_score: Option<u64>,
    pub startup_work: Option<u64>,
    pub recurring_work: Option<u64>,
    pub runtime_memory_bytes: Option<u64>,
}

impl CandidateCostEvidence {
    pub fn size_only(transfer_bytes: u64) -> Self {
        Self {
            transfer_bytes,
            performance_score: None,
            startup_work: None,
            recurring_work: None,
            runtime_memory_bytes: None,
        }
    }
}

impl From<CandidateCost> for CandidateCostEvidence {
    fn from(cost: CandidateCost) -> Self {
        Self {
            transfer_bytes: cost.transfer_bytes,
            performance_score: Some(cost.performance_score),
            startup_work: Some(cost.startup_work),
            recurring_work: Some(cost.recurring_work),
            runtime_memory_bytes: Some(cost.runtime_memory_bytes),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct CandidateConstraints {
    pub max_startup_work: Option<u64>,
    pub max_recurring_work: Option<u64>,
    pub max_runtime_memory_bytes: Option<u64>,
    pub max_performance_regression_percent: Option<u32>,
}

/// Caps are optional; the compiler must still supply a finite, program-sized
/// work plan when creating a ledger. No absent cap is an unbounded work plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ResourceLimits {
    pub logical_work: Option<u64>,
    pub retained_bytes: Option<u64>,
    /// Cooperative deadline. A hard wall-time guarantee requires isolation.
    pub wall_time_ms: Option<u64>,
}

impl ResourceLimits {
    /// Intersect independent hard ceilings before computing policy identity.
    pub(crate) fn restricted_by(self, ceilings: Self) -> Self {
        fn minimum(left: Option<u64>, right: Option<u64>) -> Option<u64> {
            match (left, right) {
                (Some(left), Some(right)) => Some(left.min(right)),
                (left, right) => left.or(right),
            }
        }
        Self {
            logical_work: minimum(self.logical_work, ceilings.logical_work),
            retained_bytes: minimum(self.retained_bytes, ceilings.retained_bytes),
            wall_time_ms: minimum(self.wall_time_ms, ceilings.wall_time_ms),
        }
    }
}

/// Scheduling changes when complete candidates reach the canonical codecs,
/// never their semantic eligibility or exact-artifact comparison rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CodecSchedule {
    Immediate,
    #[default]
    Staged,
}

/// Terminal proxy rejection is a policy choice, including diagnostic work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProxyPruning {
    #[default]
    On,
    /// Record exact scores of rejected moves without selecting from them.
    Audit,
    /// Judge every reached nonidentical move with the exact objective.
    Off,
}

// The first release used a boolean. Keep explicit true/false configurations
// equivalent to on/off while giving omitted values their effort-based default.
fn deferred_naming_permission<'de, D>(deserializer: D) -> Result<TacticPermission, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Setting {
        Permission(TacticPermission),
        Boolean(bool),
    }
    Ok(match Setting::deserialize(deserializer)? {
        Setting::Permission(permission) => permission,
        Setting::Boolean(true) => TacticPermission::On,
        Setting::Boolean(false) => TacticPermission::Off,
    })
}

/// Fixed deterministic scheduling choices. Remaining resource headroom may
/// reject work, but it must not silently change these batch/cadence settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct SearchSchedule {
    pub codec_schedule: CodecSchedule,
    /// Terminal proxy rejection: `on` (default), `audit` (also measure rejected moves) or `off` (judge every reached move exactly).
    pub proxy_pruning: ProxyPruning,
    /// Revisit pruned naming starts: `auto` from effort 14, `on` from 13, `off` never. Boolean true/false remain on/off aliases; extra compile work may yield no size win.
    #[serde(deserialize_with = "deferred_naming_permission")]
    pub deferred_naming_starts: TacticPermission,
    /// Refine admitted deferred starts with local naming and permitted private-field choices. `false` retains their ordinary walk only.
    pub deferred_naming_polish: bool,
    pub render_batch: usize,
    /// Every Nth structural expansion serves an old pending cursor; every Nth
    /// staged scoring event serves an old artifact. These are distinct clocks.
    pub diversity_interval: usize,
}
impl Default for SearchSchedule {
    fn default() -> Self {
        Self {
            codec_schedule: CodecSchedule::Staged,
            proxy_pruning: ProxyPruning::On,
            deferred_naming_starts: TacticPermission::Auto,
            deferred_naming_polish: true,
            render_batch: 8,
            diversity_interval: 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PolicyConfig {
    /// Permission semantics: 2 (the omitted-version compatibility default) retains diagnosed effort-16 startup grants; 3 requires explicit runtime-risk permission independently of effort.
    pub version: u32,
    /// Per-tactic permission (`on`, `off` or `auto`): `off` vetoes a tactic in direct and searched use, `on` permits it but never forces it.
    pub tactics: BTreeMap<TacticId, TacticPermission>,
    /// Hard resource ceilings for one compilation: logical work, retained bytes, codec probes and an optional deadline.
    pub resources: ResourceLimits,
    /// Hard constraints an admitted artifact must meet regardless of objective, checked before size is compared.
    pub constraints: CandidateConstraints,
    /// The versioned search schedule: codec cadence, render batch and diversity interval.
    pub search: SearchSchedule,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            version: LEGACY_POLICY_VERSION,
            tactics: BTreeMap::new(),
            resources: ResourceLimits::default(),
            constraints: CandidateConstraints::default(),
            search: SearchSchedule::default(),
        }
    }
}

impl PolicyConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !matches!(self.version, LEGACY_POLICY_VERSION | POLICY_SCHEMA_VERSION) {
            return Err(format!(
                "unsupported `policy.version` {}; expected 2 (legacy startup grants) or 3 (explicit runtime permissions)",
                self.version
            ));
        }
        if self.resources.wall_time_ms == Some(0) {
            return Err("`policy.resources.wall_time_ms` must be greater than zero".into());
        }
        for (name, value) in [
            ("render_batch", self.search.render_batch),
            ("diversity_interval", self.search.diversity_interval),
        ] {
            if value == 0 {
                return Err(format!("`policy.search.{name}` must be greater than zero"));
            }
        }
        Ok(())
    }
}

/// Ranking stays independent from legality and hard candidate constraints.
/// The configuration loader admits only `size-first`; the other priorities
/// stay for the day runtime estimators exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectiveRank {
    pub priority: JavaScriptPriority,
}

/// The performance regression `realistic-performance-first` tolerates.
const REALISTIC_PERFORMANCE_LIMIT_PERCENT: u64 = 25;

impl ObjectiveRank {
    /// Compatibility rank for existing clients. Size-first uses exact bytes,
    /// not quantized ratios; use `ResolvedPolicy::admit` before ranking.
    pub fn rank(
        self,
        transfer: u64,
        baseline_transfer: u64,
        performance: u64,
        baseline_performance: u64,
    ) -> (u64, u64) {
        let transfer_ratio = normalized_ratio(transfer, baseline_transfer);
        let performance_ratio = normalized_ratio(performance, baseline_performance);
        match self.priority {
            JavaScriptPriority::PerformanceFirst => (performance_ratio, transfer_ratio),
            JavaScriptPriority::RealisticPerformanceFirst => {
                let limit = 10_000u64
                    .saturating_add(REALISTIC_PERFORMANCE_LIMIT_PERCENT.saturating_mul(100));
                let rejected = u64::from(performance_ratio > limit);
                (
                    rejected
                        .saturating_mul(1_000_000)
                        .saturating_add(transfer_ratio),
                    performance_ratio,
                )
            }
            JavaScriptPriority::Balanced => (
                transfer_ratio
                    .saturating_mul(3)
                    .saturating_add(performance_ratio.saturating_mul(2)),
                transfer_ratio,
            ),
            JavaScriptPriority::SizeFirst => (transfer, performance_ratio),
        }
    }
}

fn normalized_ratio(value: u64, baseline: u64) -> u64 {
    if baseline == 0 {
        return u64::from(value != 0) * 10_000;
    }
    // u128 avoids clamping before division for large exact counts.
    u64::try_from(u128::from(value) * 10_000 / u128::from(baseline)).unwrap_or(u64::MAX)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizationObjective {
    pub codec: CompressionCostModel,
    /// The codec settings the objective judges and reports with (law B2):
    /// `[objective.brotli]` and `[objective.gzip]`.
    pub codec_settings: crate::compression::CodecSettings,
    pub rank: ObjectiveRank,
    pub optional_alternatives: usize,
    pub optional_codec_probes: usize,
    pub retained_candidates: usize,
    pub retained_candidate_bytes: usize,
    pub beam_width: usize,
    /// The walk's budget at this effort level (architecture §9.6, §13.4).
    pub walk: WalkSchedule,
    pub search: SearchSchedule,
}

/// The version of the effort schedule `WalkSchedule::at` and
/// `StructuralSchedule::at` state. Receipts carry it; a changed value is a
/// changed schedule.
// Version 8 gates deferred seeds at 14 by default and adds naming/field refinements.
// Version 9 offers all legal site alternatives and extra joint moves from effort 14.
// Version10 refines live allocation after the completed naming tails.
pub const WALK_SCHEDULE_VERSION: u32 = 10;

/// The walk's budget at one effort level (architecture §9.6, §13.3–§13.4;
/// plan M3.5): budgets are counts (AM1), never the clock.
///
/// - `prefix`: the list positions the walk may examine, p(L);
/// - `exact`: the exact judgements it may make, e(L);
/// - `margin`: a move whose proxy delta exceeds it is pruned without an
///   exact judgement, M (the proxy never keeps);
/// - `passes`: the passes over the list a walk may make; a pass that keeps
///   nothing ends it (the fixed point);
/// - `starts`: whether the objective walks several starts (AM2): the
///   structural search's winner, the level-0 artifact, and the level-0
///   artifact under each other naming seed.
///
/// Every parameter is non-decreasing in the level and none reads the
/// program. Up to level 13 the walk from the level-0 artifact at level L+1
/// passes through level L's stopping point, and every other start replaces
/// the result only on a strict exact win: size(L+1) ≤ size(L) by
/// construction. Above 13 the structural search widens with the level
/// (`StructuralSchedule`), which need not retain the lower level's frontier.
/// These replay statements assume enough resources to reach those stopping
/// points; hard limits can interrupt the structural search or walk earlier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct WalkSchedule {
    pub prefix: usize,
    pub exact: usize,
    pub margin: usize,
    pub passes: usize,
    pub starts: bool,
}

impl WalkSchedule {
    /// No walk: the level-0 artifact is delivered.
    pub const OFF: Self = Self {
        prefix: 0,
        exact: 0,
        margin: 0,
        passes: 0,
        starts: false,
    };

    /// Schedule version 2. Levels 1–12 are the fast tiers (§13.4): one pass
    /// from the level-0 artifact, over short prefixes at 1–9 and every move
    /// from 10. These are inherited bounded-work priors, with provenance in
    /// `benchmarks/calibration/policy-provenance.md`. From default level 13
    /// size comes first (AM2): several starts, each walked without an exact
    /// budget in passes to its fixed point; above 13 the structural search
    /// widens (`StructuralSchedule`).
    /// The margin is a heuristic, not a bound on final search loss. Even
    /// exact raw/gzip rejection can exclude a seed whose later refinement
    /// wins. Generic audit and no-prune runs measure those separately.
    pub fn at(level: u8, codec: CompressionCostModel) -> Self {
        let margin = match codec {
            CompressionCostModel::Brotli => 150,
            CompressionCostModel::Gzip | CompressionCostModel::Raw => 0,
        };
        let (prefix, exact, passes) = match level {
            0 => (0, 0, 0),
            1..=4 => (8, 2, 1),
            5..=9 => (24, 4, 1),
            10 => (usize::MAX, 6, 1),
            11 => (usize::MAX, 8, 1),
            12 => (usize::MAX, 12, 1),
            _ => (usize::MAX, usize::MAX, usize::MAX),
        };
        Self {
            prefix,
            exact,
            margin: if prefix == 0 { 0 } else { margin },
            passes,
            starts: level >= 13,
        }
    }
    /// The schedule as a receipt records it: an unbounded count is null.
    pub fn receipt(self) -> serde_json::Value {
        let bound = |count: usize| (count != usize::MAX).then_some(count);
        serde_json::json!({"version": WALK_SCHEDULE_VERSION, "prefix": bound(self.prefix),
            "exact": bound(self.exact), "margin": self.margin, "passes": bound(self.passes),
            "starts": self.starts})
    }
}

/// The structural search's budget at one effort level (AM2): proposals,
/// codec probes, retained candidates and bytes, and beam width. The search
/// runs from level 13 (`WalkSchedule::starts`) and widens with the level; at
/// each level it is at least what any reference port searched there before
/// the per-project search keys were retired (batch B1b): the widest
/// level-13 tier, and the widest level-15 beam and byte budget.
/// Below 13 only `bytes` is read, as the bound a walk's artifact renders
/// within.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct StructuralSchedule {
    pub proposals: usize,
    pub codec_probes: usize,
    pub candidates: usize,
    pub bytes: usize,
    pub width: usize,
}

impl StructuralSchedule {
    pub fn at(level: u8) -> Self {
        let (proposals, codec_probes, candidates, bytes, width) = match level {
            0..=13 => (1_024, 1_536, 1_024, 768 * 1024, 10),
            14 => (1_024, 1_536, 1_024, 896 * 1024, 11),
            15 => (1_536, 1_536, 1_536, 16 * 1024 * 1024, 24),
            _ => (4_096, 4_096, 4_096, 64 * 1024 * 1024, 32),
        };
        Self {
            proposals,
            codec_probes,
            candidates,
            bytes,
            width,
        }
    }
}

/// Serializes one of a walk's schedule counts: an unbounded one
/// (`usize::MAX`) as null.
pub fn serialize_bound<S: serde::Serializer>(
    count: &usize,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match *count {
        usize::MAX => serializer.serialize_none(),
        count => serializer.serialize_u64(count as u64),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPolicy {
    cache: crate::config::CacheConfig,
    source_contract: crate::config::LanguageConfig,
    hosts: crate::config::HostConfig,
    contract: CompilationContract,
    objective: Option<OptimizationObjective>,
    effort: u8,
    tactics: [ResolvedTactic; TacticId::ALL.len()],
    configuration_version: u32,
    origins: [TacticOrigin; TacticId::ALL.len()],
    resources: ResourceLimits,
    constraints: CandidateConstraints,
    diagnostics: Vec<String>,
    fingerprint: [u8; 32],
}

impl ResolvedPolicy {
    pub(crate) fn new(
        source_contract: crate::config::LanguageConfig,
        contract: CompilationContract,
        objective: Option<OptimizationObjective>,
        effort: u8,
        tactics: [ResolvedTactic; TacticId::ALL.len()],
        configuration_version: u32,
        origins: [TacticOrigin; TacticId::ALL.len()],
        resources: ResourceLimits,
        constraints: CandidateConstraints,
        diagnostics: Vec<String>,
    ) -> Self {
        let mut policy = Self {
            cache: Default::default(),
            source_contract,
            hosts: Default::default(),
            contract,
            objective,
            effort,
            tactics,
            configuration_version,
            origins,
            resources,
            constraints,
            diagnostics,
            fingerprint: [0; 32],
        };
        policy.fingerprint = Sha256::digest(policy.receipt().to_string().as_bytes()).into();
        policy
    }
    pub(crate) fn with_cache(mut self, cache: crate::config::CacheConfig) -> Self {
        // Physical reuse cannot perturb semantic identity or search ordering.
        self.cache = cache;
        self
    }
    pub fn cache(&self) -> &crate::config::CacheConfig { &self.cache }

    pub fn source_contract(&self) -> crate::config::LanguageConfig {
        self.source_contract
    }
    pub(crate) fn with_hosts(mut self, hosts: crate::config::HostConfig) -> Self {
        self.hosts = hosts;
        self.fingerprint = Sha256::digest(self.receipt().to_string().as_bytes()).into();
        self
    }
    pub fn hosts(&self) -> &crate::config::HostConfig { &self.hosts }
    pub fn contract(&self) -> &CompilationContract {
        &self.contract
    }
    pub fn javascript_contract(&self) -> Option<&JavaScriptCompilationContract> {
        match &self.contract {
            CompilationContract::JavaScript { language, .. } => Some(language),
            CompilationContract::Native { .. } => None,
        }
    }
    /// The delivery contract of a JavaScript policy.
    pub fn delivery(&self) -> Option<&DeliveryContract> {
        match &self.contract {
            CompilationContract::JavaScript { delivery, .. } => Some(delivery),
            CompilationContract::Native { .. } => None,
        }
    }
    pub fn objective(&self) -> Option<OptimizationObjective> {
        self.objective
    }
    pub fn effort(&self) -> u8 {
        self.effort
    }
    /// Effective permission for the deferred tail; actual work also needs a
    /// proxy-rejected seed and remaining resources. Explicit on preserves the
    /// level-13 opt-in, but never expands the fast tiers or overrides a veto.
    pub(crate) fn deferred_naming_starts_enabled(&self) -> bool {
        self.objective.is_some_and(|objective| {
            objective.walk.starts
                && self.tactic(TacticId::NamingSearch).enabled
                && objective.search.proxy_pruning != ProxyPruning::Off
                && match objective.search.deferred_naming_starts {
                    TacticPermission::Auto => self.effort >= 14,
                    TacticPermission::On => true,
                    TacticPermission::Off => false,
                }
        })
    }
    pub fn resources(&self) -> ResourceLimits {
        self.resources
    }
    pub fn constraints(&self) -> CandidateConstraints {
        self.constraints
    }
    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }
    pub fn tactic(&self, id: TacticId) -> ResolvedTactic {
        self.tactics[id as usize]
    }

    /// Policy resolution, separate from candidate outcomes and canonical
    /// identity. In particular an enabled producer does not assert a legal site.
    pub fn resolution(&self) -> serde_json::Value {
        use serde_json::json;
        json!({"configuration_version": self.configuration_version,
        "cache": self.cache,
        "runtime_permissions": if self.configuration_version == LEGACY_POLICY_VERSION {
            "legacy-effort-16-startup"
        } else { "explicit" },
        "tactics": TacticId::ALL.map(|id| {
            let spec = id.spec();
            let state = self.tactic(id);
            let available = !spec.producers.is_empty()
                && (!spec.javascript_only || self.javascript_contract().is_some());
            let missing = spec.prerequisites.iter().copied()
                .filter(|required| !self.tactic(*required).enabled).collect::<Vec<_>>();
            let status = if state.permission == TacticPermission::Off { "disabled" }
                else if !available { "unavailable" }
                else if !missing.is_empty() { "prerequisite-disabled" }
                else if !state.enabled { "automatic-off" }
                else { "enabled" };
            json!({"id": id, "origin": self.origins[id as usize], "status": status,
                "missing_prerequisites": missing,
                "permitted_risks": spec.risks.iter().copied().filter(|risk| {
                    self.check_tactic_permissions(&[TacticUse {tactic:id, risk:*risk}]).is_ok()
                }).collect::<Vec<_>>(),
                "producers": spec.producers.iter().map(|producer| json!({
                    "stage": producer, "enabled": spec.producer_enabled(*producer, self),
                    "prerequisites": spec.producer_prerequisites(*producer)
                })).collect::<Vec<_>>()})
        })})
    }

    /// The caller supplies semantically validated uses from the candidate's
    /// provenance, including cached candidates. A shared exact-byte score is
    /// not permission to omit these uses or their legality proof.
    pub fn admit(
        &self,
        uses: &[TacticUse],
        cost: CandidateCost,
        baseline: CandidateCost,
    ) -> Result<(), AdmissionError> {
        self.admit_evidence(uses, cost.into(), baseline.into())
    }

    /// The search owner may rank size without runtime estimates only when no
    /// selected priority or explicit bound requires the missing evidence.
    pub fn admit_evidence(
        &self,
        uses: &[TacticUse],
        cost: CandidateCostEvidence,
        baseline: CandidateCostEvidence,
    ) -> Result<(), AdmissionError> {
        self.check_tactic_permissions(uses)?;
        self.admit_cost_evidence(uses, cost, baseline)
    }

    /// Reusing frozen output must retain the original transformation/risk
    /// permissions even before complete-artifact cost evidence is available.
    pub(crate) fn check_tactic_permissions(
        &self,
        uses: &[TacticUse],
    ) -> Result<(), AdmissionError> {
        for usage in uses {
            let state = self.tactic(usage.tactic);
            if !state.enabled {
                return Err(AdmissionError::ForbiddenTactic(usage.tactic));
            }
            if !usage.tactic.spec().risks.contains(&usage.risk) {
                return Err(AdmissionError::RuntimePermission {
                    tactic: usage.tactic,
                    risk: usage.risk,
                });
            }
            match usage.risk {
                RuntimeRisk::Neutral => (),
                RuntimeRisk::Startup
                    if state.permission == TacticPermission::On
                        || (self.configuration_version == LEGACY_POLICY_VERSION
                            && self.effort >= 16
                            && usage.tactic.spec().startup_at_level_16) =>
                {
                    ()
                }
                RuntimeRisk::Recurring if state.permission == TacticPermission::On => (),
                risk => {
                    return Err(AdmissionError::RuntimePermission {
                        tactic: usage.tactic,
                        risk,
                    });
                }
            }
        }
        Ok(())
    }
    fn admit_cost_evidence(
        &self,
        uses: &[TacticUse],
        cost: CandidateCostEvidence,
        baseline: CandidateCostEvidence,
    ) -> Result<(), AdmissionError> {
        // Cost evidence cannot quietly introduce a risk absent from the
        // candidate's declared transformation provenance.
        if cost
            .recurring_work
            .zip(baseline.recurring_work)
            .is_some_and(|(cost, baseline)| cost > baseline)
            && !uses.iter().any(|u| u.risk == RuntimeRisk::Recurring)
        {
            return Err(AdmissionError::UndeclaredRuntimeRisk(
                RuntimeRisk::Recurring,
            ));
        }
        if cost
            .startup_work
            .zip(baseline.startup_work)
            .is_some_and(|(cost, baseline)| cost > baseline)
            && !uses
                .iter()
                .any(|u| matches!(u.risk, RuntimeRisk::Startup | RuntimeRisk::Recurring))
        {
            return Err(AdmissionError::UndeclaredRuntimeRisk(RuntimeRisk::Startup));
        }
        for (name, value, limit) in [
            (
                "startup work",
                cost.startup_work,
                self.constraints.max_startup_work,
            ),
            (
                "recurring work",
                cost.recurring_work,
                self.constraints.max_recurring_work,
            ),
            (
                "runtime memory",
                cost.runtime_memory_bytes,
                self.constraints.max_runtime_memory_bytes,
            ),
        ] {
            if let Some(limit) = limit {
                let value = value.ok_or(AdmissionError::MissingCostEvidence(name))?;
                if value > limit {
                    return Err(AdmissionError::Constraint(name));
                }
            }
        }
        if let Some(percent) = self.constraints.max_performance_regression_percent {
            let performance = cost
                .performance_score
                .ok_or(AdmissionError::MissingCostEvidence("performance estimate"))?;
            let baseline =
                baseline
                    .performance_score
                    .ok_or(AdmissionError::MissingCostEvidence(
                        "baseline performance estimate",
                    ))?;
            if u128::from(performance) * 100 > u128::from(baseline) * (100 + u128::from(percent)) {
                return Err(AdmissionError::Constraint("performance estimate"));
            }
        }
        if self
            .objective
            .is_some_and(|objective| objective.rank.priority != JavaScriptPriority::SizeFirst)
        {
            cost.performance_score
                .ok_or(AdmissionError::MissingCostEvidence("performance estimate"))?;
            baseline
                .performance_score
                .ok_or(AdmissionError::MissingCostEvidence(
                    "baseline performance estimate",
                ))?;
        }
        Ok(())
    }

    /// Ordering compares only candidates already admitted under this policy.
    /// Canonical implementation/naming descriptors break remaining ties;
    /// allocation handles and completion order must not decide them.
    pub fn compare(
        &self,
        left: CandidateCost,
        right: CandidateCost,
        baseline: CandidateCost,
    ) -> Option<Ordering> {
        self.compare_evidence(left.into(), right.into(), baseline.into())
            .expect("complete legacy cost evidence")
    }

    pub fn compare_evidence(
        &self,
        left: CandidateCostEvidence,
        right: CandidateCostEvidence,
        baseline: CandidateCostEvidence,
    ) -> Result<Option<Ordering>, AdmissionError> {
        let Some(objective) = self.objective else {
            return Ok(None);
        };
        let rank = objective.rank;
        if rank.priority == JavaScriptPriority::SizeFirst {
            let size = left.transfer_bytes.cmp(&right.transfer_bytes);
            if size != Ordering::Equal {
                return Ok(Some(size));
            }
            // Evidence availability is an ordered category, not a fabricated
            // runtime value. Treating mixed known/unknown pairs as Equal while
            // comparing known pairs by cost would make tie ordering nontransitive.
            let performance = match baseline.performance_score {
                Some(baseline) => match (left.performance_score, right.performance_score) {
                    (Some(left), Some(right)) => {
                        normalized_ratio(left, baseline).cmp(&normalized_ratio(right, baseline))
                    }
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => Ordering::Equal,
                },
                None => Ordering::Equal,
            };
            return Ok(Some(performance));
        }
        let baseline_performance =
            baseline
                .performance_score
                .ok_or(AdmissionError::MissingCostEvidence(
                    "baseline performance estimate",
                ))?;
        let left_performance = left
            .performance_score
            .ok_or(AdmissionError::MissingCostEvidence("performance estimate"))?;
        let right_performance = right
            .performance_score
            .ok_or(AdmissionError::MissingCostEvidence("performance estimate"))?;
        Ok(Some(
            rank.rank(
                left.transfer_bytes,
                baseline.transfer_bytes,
                left_performance,
                baseline_performance,
            )
            .cmp(&rank.rank(
                right.transfer_bytes,
                baseline.transfer_bytes,
                right_performance,
                baseline_performance,
            )),
        ))
    }

    pub fn ledger(&self, mut plan: BudgetPlan) -> Result<BudgetLedger, BudgetError> {
        if self
            .objective
            .is_some_and(|objective| objective.optional_alternatives == 0)
        {
            plan.optional_work = 0;
        }
        BudgetLedger::new(self.resources, plan)
    }

    /// Canonical, versioned policy receipt. Source contents, semantic queries,
    /// compiler version and codec implementation identities are separate keys.
    pub fn receipt(&self) -> serde_json::Value {
        use serde_json::json;
        let contract = match &self.contract {
            CompilationContract::Native { abi_version } => {
                json!({"target":"native", "abi_version":abi_version})
            }
            CompilationContract::JavaScript {
                language,
                preserved_properties,
                delivery,
            } => json!({
                "target":"javascript", "world":format!("{:?}",language.world), "execution":format!("{:?}",language.execution), "ecmascript":language.ecmascript.name(),
                "preserve_root_exports":language.abi.preserve_root_exports,
                "keep_function_names":language.abi.keep_function_names,
                "keep_published_function_names":language.abi.keep_published_function_names,
                "pristine_builtins":language.assumptions.pristine_builtins,
                "pure_property_reads":language.assumptions.pure_property_reads,
                "unconstructed_callbacks":language.assumptions.unconstructed_callbacks,
                "numeric_lengths":language.assumptions.numeric_lengths,
                "strip_debug":language.effects.strip_debug,
                "strip_console_calls":language.effects.strip_console_calls,
                "checks":language.checks.name(),
                "preserved_properties":preserved_properties,
                "delivery":{"mode":delivery.mode.name(), "format":delivery.format.name(),
                    "preload":delivery.preload.name(), "host_modules":delivery.host_modules.name(), "entry_names":delivery.entry_names(),
                    "chunk_names":delivery.chunk_names(), "module_names":delivery.module_names(),
                    "request_bytes":delivery.request_bytes, "depth_bytes":delivery.depth_bytes},
            }),
        };
        let objective = self.objective.map(|o| json!({"codec":format!("{:?}",o.codec), "codec_settings":o.codec_settings, "priority":format!("{:?}",o.rank.priority), "optional_alternatives":o.optional_alternatives, "optional_codec_probes":o.optional_codec_probes, "retained_candidates":o.retained_candidates, "retained_candidate_bytes":o.retained_candidate_bytes, "beam_width":o.beam_width, "walk":o.walk.receipt(), "search":{"version":SEARCH_SCHEDULE_VERSION,"codec_schedule":o.search.codec_schedule,"proxy_pruning":o.search.proxy_pruning,"deferred_naming_starts":o.search.deferred_naming_starts,"deferred_naming_starts_enabled":self.deferred_naming_starts_enabled(),"deferred_naming_polish":o.search.deferred_naming_polish,"render_batch":o.search.render_batch,"diversity_interval":o.search.diversity_interval}}));
        json!({"schema":POLICY_SCHEMA_VERSION, "algorithm":POLICY_ALGORITHM_VERSION, "configuration_version":self.configuration_version, "source_contract":self.source_contract, "host":self.hosts, "contract":contract, "objective":objective, "effort":self.effort, "tactics":TacticId::ALL.map(|id| {
            let spec = id.spec();
            let available = !spec.producers.is_empty() && (!spec.javascript_only || self.javascript_contract().is_some());
            json!({"id":id, "state":self.tactic(id), "available":available, "definition":spec})
        }), "resources":self.resources, "constraints":self.constraints})
    }
    pub fn fingerprint(&self) -> [u8; 32] {
        self.fingerprint
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionError {
    ForbiddenTactic(TacticId),
    RuntimePermission { tactic: TacticId, risk: RuntimeRisk },
    UndeclaredRuntimeRisk(RuntimeRisk),
    Constraint(&'static str),
    MissingCostEvidence(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetPlan {
    pub baseline_work: u64,
    pub optional_work: u64,
    pub baseline_retained_bytes: u64,
    pub retained_bytes: u64,
}

/// Finite ceilings for completing mandatory output before exploration starts.
/// ResourceLimits can reduce these ceilings; this plan is not a prediction of
/// the resources sufficient to compile a particular source program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BaselineFirstPlan {
    pub logical_work: u64,
    pub retained_bytes: u64,
    /// Work withheld from both initial baseline construction and exploration,
    /// available to the Baseline domain after a successful seal. Terminal
    /// work cannot allocate new Baseline storage; retained output can be handed
    /// off or released without rebuilding its target or repeating codecs.
    pub terminal_work: u64,
}

/// Copied receipt from the one-way transition into optional exploration.
/// Headroom describes this instant, not a second independently spendable budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BaselineSeal {
    pub baseline_work: u64,
    pub terminal_work: u64,
    pub optional_work: u64,
    pub baseline_retained_bytes: u64,
    pub optional_memory_headroom: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkDomain {
    Baseline,
    Optional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum WorkKind {
    Analysis,
    Edit,
    Render,
    Codec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AnalysisAttempt {
    pub plan: u32,
    pub work_quota: u64,
    pub algorithm_version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisCompletion {
    Complete,
    Truncated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnalysisWorkReceipt {
    pub attempt: AnalysisAttempt,
    pub completion: AnalysisCompletion,
    pub logical_work: u64,
}

impl AnalysisWorkReceipt {
    pub fn reusable_for(self, requested: AnalysisAttempt) -> bool {
        self.attempt == requested
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetError {
    InvalidPlan,
    BaselineExceedsLimit,
    /// Optional admission is unavailable until mandatory output is sealed.
    BaselineNotSealed,
    /// Only a preparing baseline-first ledger can be sealed, exactly once.
    InvalidBaselinePhase,
    /// A sealed baseline can release storage but cannot grow it again.
    BaselineAllocationSealed,
    WorkExhausted(WorkDomain),
    MemoryExhausted(WorkDomain),
    /// The compilation's cooperative wall-time limit has been reached.
    DeadlineExceeded,
    InvalidRelease,
    AnalysisAttemptMismatch,
    InvalidAnalysisReceipt,
}

/// One ledger per compilation attempt. The query owner invokes analysis
/// charging once per unique dependency attempt; a cache hit takes exactly the
/// same charging route as a cold execution. Physical CPU is measured separately.
/// Work and memory admissions check one monotonic deadline, including in clones.
/// Release remains available after expiry. This does not preempt work between
/// admission boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetLedger {
    recording: Option<crate::admission_replay::Recording>,
    baseline_work_limit: u64,
    optional_work_limit: u64,
    // The current independent objective's structural/walk reserve is relative
    // to its own start, while all admissions keep the cumulative counters.
    optional_work_origin: u64,
    work_used: [u64; 2],
    work_by_kind: [u64; 4],
    baseline_memory_reserve: u64,
    memory_limit: u64,
    memory_used: [u64; 2],
    peak_memory: u64,
    deadline: Option<BudgetDeadline>,
    phase: BudgetPhase,
}

/// Internal one-use restoration token; this does not own or replenish work.
pub(crate) struct OptionalWorkShare {
    previous_limit: u64,
    previous_origin: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BudgetPhase {
    Fixed,
    PreparingBaseline { total_work: u64, terminal_work: u64 },
    SealedBaseline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BudgetDeadline {
    started_at: Instant,
    limit_ms: u64,
    #[cfg(test)]
    elapsed_override: Option<Duration>,
}

impl BudgetDeadline {
    fn elapsed(&self) -> Duration {
        #[cfg(test)]
        if let Some(elapsed) = self.elapsed_override {
            return elapsed;
        }
        self.started_at.elapsed()
    }
}

impl BudgetLedger {
    pub fn new(limits: ResourceLimits, plan: BudgetPlan) -> Result<Self, BudgetError> {
        if plan.baseline_retained_bytes > plan.retained_bytes
            || plan.baseline_work.checked_add(plan.optional_work).is_none()
        {
            return Err(BudgetError::InvalidPlan);
        }
        let work_limit = limits
            .logical_work
            .unwrap_or(plan.baseline_work + plan.optional_work);
        let memory_limit = limits
            .retained_bytes
            .unwrap_or(plan.retained_bytes)
            .min(plan.retained_bytes);
        if work_limit < plan.baseline_work || memory_limit < plan.baseline_retained_bytes {
            return Err(BudgetError::BaselineExceedsLimit);
        }
        Ok(Self {
            recording: None,
            baseline_work_limit: plan.baseline_work,
            optional_work_limit: plan.optional_work.min(work_limit - plan.baseline_work),
            optional_work_origin: 0,
            work_used: [0; 2],
            work_by_kind: [0; 4],
            baseline_memory_reserve: plan.baseline_retained_bytes,
            memory_limit,
            memory_used: [0; 2],
            peak_memory: 0,
            deadline: limits.wall_time_ms.map(|limit_ms| BudgetDeadline {
                started_at: Instant::now(),
                limit_ms,
                #[cfg(test)]
                elapsed_override: None,
            }),
            phase: BudgetPhase::Fixed,
        })
    }

    /// Start one compilation ledger with Optional disabled. The direct route
    /// may use all effective work except the explicit terminal reserve, and
    /// all effective memory. An insufficient ceiling fails at the ordinary
    /// admission boundary; it is not hidden by a guessed source-size reserve.
    pub fn new_baseline_first(
        limits: ResourceLimits,
        plan: BaselineFirstPlan,
    ) -> Result<Self, BudgetError> {
        if plan.terminal_work > plan.logical_work {
            return Err(BudgetError::InvalidPlan);
        }
        let total_work = limits
            .logical_work
            .unwrap_or(plan.logical_work)
            .min(plan.logical_work);
        let baseline_work = total_work
            .checked_sub(plan.terminal_work)
            .ok_or(BudgetError::BaselineExceedsLimit)?;
        let mut ledger = Self::new(
            limits,
            BudgetPlan {
                baseline_work,
                optional_work: 0,
                baseline_retained_bytes: 0,
                retained_bytes: plan.retained_bytes,
            },
        )?;
        ledger.phase = BudgetPhase::PreparingBaseline {
            total_work,
            terminal_work: plan.terminal_work,
        };
        Ok(ledger)
    }

    /// Reject an incompatible compilation lifecycle before mandatory output is
    /// built. This observes the common deadline and phase without consuming
    /// work, reserving memory, or changing any allowance.
    pub fn require_preparing_baseline(&self) -> Result<(), BudgetError> {
        self.check_deadline()?;
        if matches!(self.phase, BudgetPhase::PreparingBaseline { .. }) {
            Ok(())
        } else {
            Err(BudgetError::InvalidBaselinePhase)
        }
    }

    /// Seal once, after the compilation owner has retained valid mandatory
    /// output and every requested score. This ledger cannot establish that
    /// semantic/output condition itself. Failure changes no counters, limits,
    /// deadline or phase. The same live allocations keep their original domain.
    pub fn seal_baseline(&mut self) -> Result<BaselineSeal, BudgetError> {
        self.check_deadline()?;
        let BudgetPhase::PreparingBaseline {
            total_work,
            terminal_work,
        } = self.phase
        else {
            return Err(BudgetError::InvalidBaselinePhase);
        };
        if self.work_used[1] != 0 || self.memory_used[1] != 0 {
            return Err(BudgetError::InvalidBaselinePhase);
        }
        let baseline_limit = self.work_used[0]
            .checked_add(terminal_work)
            .filter(|limit| *limit <= total_work)
            .ok_or(BudgetError::InvalidPlan)?;
        let optional_work = total_work - baseline_limit;
        let optional_memory_headroom = self
            .memory_limit
            .checked_sub(self.memory_used[0])
            .ok_or(BudgetError::InvalidPlan)?;
        let receipt = BaselineSeal {
            baseline_work: self.work_used[0],
            terminal_work,
            optional_work,
            baseline_retained_bytes: self.memory_used[0],
            optional_memory_headroom,
        };
        self.baseline_work_limit = baseline_limit;
        self.optional_work_limit = optional_work;
        // Actual live Baseline bytes already protect the incumbent. A frozen
        // floor here would keep released baseline caches/artifacts unavailable
        // to Optional even though they no longer occupy memory.
        self.baseline_memory_reserve = 0;
        self.phase = BudgetPhase::SealedBaseline;
        Ok(receipt)
    }

    /// Bound the next independent objective to an equal share of the work
    /// still available to the remaining objectives. Consumed work never resets.
    /// The caller restores this opaque ceiling after the search, even on error.
    pub(crate) fn begin_objective_share(&mut self, remaining: usize) -> OptionalWorkShare {
        assert!(self.baseline_is_sealed() && remaining != 0);
        let share = OptionalWorkShare {
            previous_limit: self.optional_work_limit,
            previous_origin: self.optional_work_origin,
        };
        let used = self.work_used[1];
        self.optional_work_limit = used + (self.optional_work_limit - used) / remaining as u64;
        self.optional_work_origin = used;
        share
    }

    /// Release the temporary ceiling without forgiving work or moving the
    /// deadline. Cleanup remains possible after work/deadline exhaustion.
    pub(crate) fn end_objective_share(&mut self, share: OptionalWorkShare) {
        assert!(self.optional_work_limit <= share.previous_limit);
        self.optional_work_limit = share.previous_limit;
        self.optional_work_origin = share.previous_origin;
    }

    pub(crate) fn optional_search_work(&self) -> (u64, u64) {
        (
            self.work_used[1] - self.optional_work_origin,
            self.optional_work_limit - self.optional_work_origin,
        )
    }

    pub fn charge(
        &mut self,
        domain: WorkDomain,
        kind: WorkKind,
        units: u64,
    ) -> Result<(), BudgetError> {
        let result = (|| {
        self.check_deadline()?;
        if domain == WorkDomain::Optional
            && matches!(self.phase, BudgetPhase::PreparingBaseline { .. })
        {
            return Err(BudgetError::BaselineNotSealed);
        }
        let (index, limit) = match domain {
            WorkDomain::Baseline => (0, self.baseline_work_limit),
            WorkDomain::Optional => (1, self.optional_work_limit),
        };
        let next = self.work_used[index]
            .checked_add(units)
            .filter(|n| *n <= limit)
            .ok_or(BudgetError::WorkExhausted(domain))?;
        let by_kind = self.work_by_kind[kind as usize]
            .checked_add(units)
            .ok_or(BudgetError::WorkExhausted(domain))?;
        self.work_used[index] = next;
        self.work_by_kind[kind as usize] = by_kind;
        Ok(())
        })();
        if result.is_err() { self.record_refusal(); }
        result
    }
    pub fn charge_analysis(
        &mut self,
        domain: WorkDomain,
        requested: AnalysisAttempt,
        receipt: AnalysisWorkReceipt,
    ) -> Result<(), BudgetError> {
        if !receipt.reusable_for(requested) {
            return Err(BudgetError::AnalysisAttemptMismatch);
        }
        if receipt.logical_work > requested.work_quota {
            return Err(BudgetError::InvalidAnalysisReceipt);
        }
        self.charge(domain, WorkKind::Analysis, receipt.logical_work)
    }
    pub fn retain(&mut self, domain: WorkDomain, bytes: u64) -> Result<(), BudgetError> {
        let result = self.retain_inner(domain, bytes);
        if result.is_err() { self.record_refusal(); }
        result
    }
    fn retain_inner(&mut self, domain: WorkDomain, bytes: u64) -> Result<(), BudgetError> {
        self.check_deadline()?;
        match (self.phase, domain) {
            (BudgetPhase::PreparingBaseline { .. }, WorkDomain::Optional) => {
                return Err(BudgetError::BaselineNotSealed);
            }
            (BudgetPhase::SealedBaseline, WorkDomain::Baseline) if bytes != 0 => {
                return Err(BudgetError::BaselineAllocationSealed);
            }
            _ => {}
        }
        let index = match domain {
            WorkDomain::Baseline => 0,
            WorkDomain::Optional => 1,
        };
        let next = self.memory_used[index]
            .checked_add(bytes)
            .ok_or(BudgetError::MemoryExhausted(domain))?;
        let other = if index == 1 {
            self.memory_used[0].max(self.baseline_memory_reserve)
        } else {
            self.memory_used[1]
        };
        if next
            .checked_add(other)
            .is_none_or(|total| total > self.memory_limit)
        {
            return Err(BudgetError::MemoryExhausted(domain));
        }
        self.memory_used[index] = next;
        self.peak_memory = self
            .peak_memory
            .max(self.memory_used[0] + self.memory_used[1]);
        if let Some(recording) = &mut self.recording { recording.retain(domain, bytes); }
        Ok(())
    }
    pub fn release(&mut self, domain: WorkDomain, bytes: u64) -> Result<(), BudgetError> {
        let index = match domain {
            WorkDomain::Baseline => 0,
            WorkDomain::Optional => 1,
        };
        self.memory_used[index] = self.memory_used[index]
            .checked_sub(bytes)
            .ok_or(BudgetError::InvalidRelease)?;
        if let Some(recording) = &mut self.recording { recording.release(domain, bytes); }
        Ok(())
    }
    pub(crate) fn record_refusal(&mut self) {
        if let Some(recording) = &mut self.recording { recording.refuse(); }
    }
    pub(crate) fn record_scratch_check(&mut self, domain: WorkDomain, bytes: u64) {
        if let Some(recording) = &mut self.recording { recording.scratch(domain, bytes); }
    }
    pub(crate) fn begin_recording(&mut self, domain: WorkDomain) -> Option<crate::admission_replay::Recording> {
        self.recording.replace(crate::admission_replay::Recording::new(domain, self.work_by_kind, self.work_used))
    }
    pub(crate) fn end_recording(
        &mut self, mut parent: Option<crate::admission_replay::Recording>,
    ) -> Option<crate::admission_replay::Receipt> {
        let recording = self.recording.take().expect("active formation admission");
        if let Some(parent) = &mut parent { parent.absorb(&recording); }
        self.recording = parent;
        recording.finish(self.work_by_kind, self.work_used)
    }
    pub fn work_used(&self, domain: WorkDomain) -> u64 {
        self.work_used[if domain == WorkDomain::Baseline { 0 } else { 1 }]
    }
    pub fn work_limit(&self, domain: WorkDomain) -> u64 {
        match domain {
            WorkDomain::Baseline => self.baseline_work_limit,
            WorkDomain::Optional => self.optional_work_limit,
        }
    }
    pub fn work_by_kind(&self, kind: WorkKind) -> u64 {
        self.work_by_kind[kind as usize]
    }
    pub fn retained_bytes(&self) -> u64 {
        self.memory_used[0] + self.memory_used[1]
    }
    pub fn retained_bytes_in(&self, domain: WorkDomain) -> u64 {
        self.memory_used[if domain == WorkDomain::Baseline { 0 } else { 1 }]
    }
    /// Fixed-partition ledgers have no baseline-first phase and return false.
    pub fn baseline_is_sealed(&self) -> bool {
        self.phase == BudgetPhase::SealedBaseline
    }
    pub fn peak_retained_bytes(&self) -> u64 {
        self.peak_memory
    }
    pub fn wall_time_ms(&self) -> Option<u64> {
        self.deadline.as_ref().map(|deadline| deadline.limit_ms)
    }
    /// Pure limit comparison for externally reported elapsed time. Admissions
    /// use the ledger's own monotonic origin, not this caller-supplied value.
    pub fn deadline_reached(&self, elapsed_ms: u64) -> bool {
        self.wall_time_ms().is_some_and(|limit| elapsed_ms >= limit)
    }
    fn check_deadline(&self) -> Result<(), BudgetError> {
        if self
            .deadline
            .as_ref()
            .is_some_and(|deadline| deadline.elapsed() >= Duration::from_millis(deadline.limit_ms))
        {
            return Err(BudgetError::DeadlineExceeded);
        }
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn set_deadline_elapsed_for_test(&mut self, elapsed: Duration) {
        self.deadline
            .as_mut()
            .expect("test elapsed override requires a configured deadline")
            .elapsed_override = Some(elapsed);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProjectConfig;

    fn config(source: &str) -> ProjectConfig {
        toml::from_str(source).unwrap()
    }
    fn js(source: &str) -> ResolvedPolicy {
        config(source)
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            })
            .unwrap()
    }
    fn usage(tactic: TacticId, risk: RuntimeRisk) -> TacticUse {
        TacticUse { tactic, risk }
    }

    #[test]
    fn search_schedule_defaults_and_partial_configuration_are_resolved_once() {
        let default = SearchSchedule {
            codec_schedule: CodecSchedule::Staged,
            proxy_pruning: ProxyPruning::On,
            deferred_naming_starts: TacticPermission::Auto,
            deferred_naming_polish: true,
            render_batch: 8,
            diversity_interval: 4,
        };
        assert_eq!(SearchSchedule::default(), default);
        assert_eq!(PolicyConfig::default().search, default);
        assert_eq!(js("").objective().unwrap().search, default);
        assert_eq!(js("[policy.search]").objective().unwrap().search, default);
        let immediate = js("[policy.search]\ncodec_schedule='immediate'");
        assert_eq!(
            immediate.objective().unwrap().search,
            SearchSchedule {
                codec_schedule: CodecSchedule::Immediate,
                ..default
            }
        );
        assert_eq!(
            immediate.receipt()["objective"]["search"],
            serde_json::json!({
                "version": SEARCH_SCHEDULE_VERSION,
                "codec_schedule": "immediate",
                "proxy_pruning": "on",
                "deferred_naming_starts": "auto",
                "deferred_naming_starts_enabled": false,
                "deferred_naming_polish": true,
                "render_batch": 8,
                "diversity_interval": 4,
            })
        );
    }

    #[test]
    fn explicit_search_schedules_round_trip_through_the_common_config_owner() {
        for codec_schedule in [CodecSchedule::Immediate, CodecSchedule::Staged] {
            let expected = PolicyConfig {
                search: SearchSchedule {
                    codec_schedule,
                    proxy_pruning: ProxyPruning::Audit,
                    deferred_naming_starts: TacticPermission::Off,
                    deferred_naming_polish: false,
                    render_batch: 3,
                    diversity_interval: 7,
                },
                ..PolicyConfig::default()
            };
            let text = toml::to_string(&expected).unwrap();
            let decoded: PolicyConfig = toml::from_str(&text).unwrap();
            assert_eq!(decoded, expected);
            decoded.validate().unwrap();
            let mut project = config("");
            project.policy = Some(decoded);
            let resolved = project
                .resolve_policy(CompilationRequest::JavaScript {
                    preserve_root_exports: true,
                })
                .unwrap();
            assert_eq!(resolved.objective().unwrap().search, expected.search);
        }
    }

    #[test]
    fn deferred_naming_auto_starts_at_fourteen_with_an_explicit_thirteen_opt_in() {
        for level in 0..=16 {
            for mode in ["'auto'", "'on'", "'off'", "true", "false"] {
                let policy = js(&format!(
                    "effort.level={level}\n[policy.search]\ndeferred_naming_starts={mode}"
                ));
                let enabled = match mode {
                    "'auto'" => level >= 14,
                    "'on'" | "true" => level >= 13,
                    _ => false,
                };
                assert_eq!(
                    policy.deferred_naming_starts_enabled(),
                    enabled,
                    "{level}/{mode}"
                );
                assert_eq!(
                    policy.receipt()["objective"]["search"]["deferred_naming_starts_enabled"],
                    enabled
                );
            }
            let omitted = js(&format!("effort.level={level}"));
            let auto = js(&format!(
                "effort.level={level}\n[policy.search]\ndeferred_naming_starts='auto'"
            ));
            assert_eq!(omitted.fingerprint(), auto.fingerprint());
            for (alias, canonical) in [("true", "on"), ("false", "off")] {
                let resolve = |value| {
                    js(&format!(
                        "effort.level={level}\n[policy.search]\ndeferred_naming_starts={value}"
                    ))
                };
                let old = resolve(alias);
                let explicit = resolve(&format!("'{canonical}'"));
                assert_eq!(old.fingerprint(), explicit.fingerprint());
                assert_eq!(
                    old.receipt()["objective"]["search"]["deferred_naming_starts"],
                    canonical
                );
            }
        }
        for veto in [
            "proxy_pruning='off'",
            "[policy.tactics]\nnaming-search='off'",
            "[policy.tactics]\nidentifier-mangling='off'",
            "[javascript]\ncandidate_search='off'",
        ] {
            let policy = js(&format!(
                "effort.level=16\n[policy.search]\ndeferred_naming_starts='on'\n{veto}"
            ));
            assert!(!policy.deferred_naming_starts_enabled(), "{veto}");
        }
    }

    #[test]
    fn invalid_search_cadences_and_unknown_schedule_controls_are_rejected() {
        for setting in ["render_batch", "diversity_interval"] {
            for schedule in ["immediate", "staged"] {
                let project = config(&format!(
                    "[policy.search]\ncodec_schedule='{schedule}'\n{setting}=0"
                ));
                let expected = format!("`policy.search.{setting}` must be greater than zero");
                assert_eq!(
                    project.policy.as_ref().unwrap().validate().unwrap_err(),
                    expected
                );
                assert_eq!(project.validate().unwrap_err(), expected);
                assert_eq!(
                    project
                        .resolve_policy(CompilationRequest::JavaScript {
                            preserve_root_exports: true,
                        })
                        .unwrap_err(),
                    expected
                );
            }
        }
        for invalid in [
            "codec_schedule='adaptive'",
            "render_batch=-1",
            "diversity_interval=-1",
            "remaining_budget_resizes_batch=true",
            "proxy_pruning=true",
            "proxy_pruning='sometimes'",
            "deferred_naming_starts='sometimes'",
            "deferred_naming_starts=13",
            "deferred_naming_polish='off'",
        ] {
            assert!(
                toml::from_str::<ProjectConfig>(&format!("[policy.search]\n{invalid}")).is_err()
            );
        }
    }

    #[test]
    fn every_schedule_field_changes_the_receipt_without_changing_contract_or_caps() {
        let ordinary = js("");
        let objective = ordinary.objective().unwrap();
        let mut fingerprints = std::collections::BTreeSet::new();
        fingerprints.insert(ordinary.fingerprint());
        let canonical: [u8; 32] = Sha256::digest(ordinary.receipt().to_string().as_bytes()).into();
        assert_eq!(ordinary.fingerprint(), canonical);
        assert_eq!(ordinary.fingerprint(), js("[policy.search]").fingerprint());
        for configured in [
            "codec_schedule='immediate'",
            "proxy_pruning='audit'",
            "proxy_pruning='off'",
            "deferred_naming_starts=false",
            "deferred_naming_polish=false",
            "render_batch=9",
            "diversity_interval=5",
        ] {
            let resolved = js(&format!("[policy.search]\n{configured}"));
            let canonical: [u8; 32] =
                Sha256::digest(resolved.receipt().to_string().as_bytes()).into();
            assert_eq!(resolved.fingerprint(), canonical);
            assert_eq!(resolved.contract(), ordinary.contract());
            assert_eq!(resolved.resources(), ordinary.resources());
            assert_eq!(resolved.constraints(), ordinary.constraints());
            assert_eq!(
                resolved.objective().unwrap(),
                OptimizationObjective {
                    search: resolved.objective().unwrap().search,
                    ..objective
                }
            );
            for tactic in TacticId::ALL {
                assert_eq!(resolved.tactic(tactic), ordinary.tactic(tactic));
            }
            assert!(fingerprints.insert(resolved.fingerprint()));
            assert_eq!(
                resolved.receipt()["objective"]["search"]["version"],
                SEARCH_SCHEDULE_VERSION
            );
        }
        assert_eq!(fingerprints.len(), 8);
    }

    #[test]
    fn native_policy_has_no_search_objective_or_schedule_identity() {
        let ordinary = config("")
            .resolve_policy(CompilationRequest::Native)
            .unwrap();
        for schedule in ["immediate", "staged"] {
            let resolved = config(&format!("[policy.search]\ncodec_schedule='{schedule}'\nrender_batch=17\ndiversity_interval=9"))
                .resolve_policy(CompilationRequest::Native).unwrap();
            assert!(resolved.objective().is_none());
            assert!(resolved.javascript_contract().is_none());
            assert!(resolved.receipt()["objective"].is_null());
            assert_eq!(resolved.fingerprint(), ordinary.fingerprint());
            for tactic in TacticId::ALL {
                if tactic.spec().javascript_only {
                    assert!(!resolved.tactic(tactic).enabled);
                }
            }
        }
    }

    #[test]
    fn startup_effort_and_recurring_permissions_are_candidate_specific() {
        for level in [0, 13, 15, 16] {
            let p = js(&format!("effort.level={level}"));
            assert!(p
                .admit(
                    &[usage(TacticId::StringArrayPacking, RuntimeRisk::Neutral)],
                    CandidateCost::default(),
                    CandidateCost::default()
                )
                .is_ok());
            assert_eq!(
                p.tactic(TacticId::StartupReconstruction).enabled,
                level == 16
            );
            assert_eq!(
                p.admit(
                    &[usage(TacticId::StringArrayPacking, RuntimeRisk::Startup)],
                    CandidateCost::default(),
                    CandidateCost::default()
                )
                .is_ok(),
                level == 16
            );
            assert!(p
                .admit(
                    &[usage(TacticId::StringArrayPacking, RuntimeRisk::Recurring)],
                    CandidateCost::default(),
                    CandidateCost::default()
                )
                .is_err());
        }
        let on = js("effort.level=0\n[policy.tactics]\nstring-array-packing='on'");
        assert!(on
            .admit(
                &[usage(TacticId::StringArrayPacking, RuntimeRisk::Startup)],
                CandidateCost::default(),
                CandidateCost::default()
            )
            .is_ok());
        assert!(on
            .admit(
                &[usage(TacticId::StringArrayPacking, RuntimeRisk::Recurring)],
                CandidateCost::default(),
                CandidateCost::default()
            )
            .is_ok());
        // An unrelated opted-in tactic cannot license another tactic's risk.
        assert!(on
            .admit(
                &[
                    usage(TacticId::StringArrayPacking, RuntimeRisk::Neutral),
                    usage(TacticId::StringPooling, RuntimeRisk::Recurring)
                ],
                CandidateCost::default(),
                CandidateCost::default()
            )
            .is_err());
    }

    #[test]
    fn every_tactic_obeys_explicit_off_and_on_without_forcing_a_winner() {
        for tactic in TacticId::ALL {
            let off = js(&format!(
                "effort.level=16\n[policy.tactics]\n{}='off'",
                tactic.spec().name
            ));
            assert!(!off.tactic(tactic).enabled, "{tactic:?}");
            assert_eq!(
                off.admit(
                    &[usage(tactic, RuntimeRisk::Neutral)],
                    CandidateCost::default(),
                    CandidateCost::default()
                ),
                Err(AdmissionError::ForbiddenTactic(tactic))
            );
            let on = js(&format!(
                "effort.level=0\n[policy.tactics]\n{}='on'",
                tactic.spec().name
            ));
            assert_eq!(
                on.tactic(tactic).enabled,
                !tactic.spec().producers.is_empty(),
                "{tactic:?}"
            );
            assert_eq!(
                on.compare(
                    CandidateCost {
                        transfer_bytes: 101,
                        ..CandidateCost::default()
                    },
                    CandidateCost {
                        transfer_bytes: 100,
                        ..CandidateCost::default()
                    },
                    CandidateCost::default()
                ),
                Some(Ordering::Greater)
            );
        }
    }

    #[test]
    fn legacy_lists_cannot_reenable_off_through_another_producer() {
        let p = js(
            "[javascript]\ncompression=[]\noptimizations=[]\n[policy.tactics]\nstring-array-packing='off'",
        );
        for tactic in [
            TacticId::StringArrayPacking,
            TacticId::StringPooling,
            TacticId::IdentifierMangling,
            TacticId::PropertyMangling,
            TacticId::NamingSearch,
        ] {
            assert_eq!(p.tactic(tactic).permission, TacticPermission::Off);
            assert!(!p.tactic(tactic).enabled);
        }
        let conflict = config("[optimization]\ninlining=false\n[policy.tactics]\ninlining='on'")
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            });
        assert!(conflict.unwrap_err().contains("contradicts"));
    }

    #[test]
    fn unavailable_producers_keep_requested_permissions_and_explain_the_refusal() {
        for tactic in [TacticId::RecurringReconstruction] {
            for permission in ["auto", "on", "off"] {
                let p = js(&format!(
                    "[policy.tactics]\n{}='{permission}'",
                    tactic.spec().name
                ));
                assert!(!p.tactic(tactic).enabled);
                assert_eq!(
                    p.tactic(tactic).permission,
                    match permission {
                        "on" => TacticPermission::On,
                        "off" => TacticPermission::Off,
                        _ => TacticPermission::Auto,
                    }
                );
                assert_eq!(
                    p.diagnostics()
                        .iter()
                        .any(|text| text.contains("no implementation")),
                    permission == "on"
                );
                assert_eq!(
                    p.admit(
                        &[usage(tactic, RuntimeRisk::Neutral)],
                        CandidateCost::default(),
                        CandidateCost::default()
                    ),
                    Err(AdmissionError::ForbiddenTactic(tactic))
                );
                let receipt = p.receipt();
                let row = &receipt["tactics"][tactic as usize];
                assert_eq!(row["available"], false);
                assert_eq!(row["definition"]["producers"], serde_json::json!([]));
            }
        }
    }

    #[test]
    fn registry_dependencies_propagate_parent_vetoes_and_native_availability() {
        let p = js(
            "[policy.tactics]\nidentifier-mangling='off'\nnaming-search='on'\nnaming-alphabet='on'",
        );
        assert!(!p.tactic(TacticId::NamingSearch).enabled);
        assert!(!p.tactic(TacticId::NamingAlphabet).enabled);
        assert_eq!(p.diagnostics().len(), 2);
        let p = config("[policy.tactics]\nscalar-replacement='on'\ncall-specialization='on'")
            .resolve_policy(CompilationRequest::Native)
            .unwrap();
        assert!(p.tactic(TacticId::ScalarReplacement).enabled);
        assert!(!p.tactic(TacticId::CallSpecialization).enabled);
        assert_eq!(p.diagnostics().len(), 1);
        for tactic in TacticId::ALL {
            let spec = tactic.spec();
            assert_eq!(spec.id, tactic);
            for dependency in spec.prerequisites {
                assert_ne!(*dependency, tactic, "a tactic cannot enable itself");
            }
            assert_eq!(
                p.tactic(tactic).enabled,
                spec.producers.contains(&TacticProducer::SharedRules)
            );
        }
    }

    #[test]
    fn explicit_alias_conflicts_fail_instead_of_selecting_an_order() {
        for (left, right) in [
            (
                "[mangle]\nidentifiers=false",
                "[javascript]\ncompression=['identifier-mangling']",
            ),
            (
                "[mangle]\nproperties=false",
                "[javascript]\ncompression=['property-mangling']",
            ),
            (
                "[mangle]\npool_strings=false",
                "[javascript]\ncompression=['string-pooling']",
            ),
            (
                "[optimization]\nparameterized_function_merging=false",
                "[javascript]\ncompression=['parameterized-function-merging']",
            ),
            (
                "[optimization]\ncall_site_specialization=false",
                "[javascript]\noptimizations=['call-site-specialization']",
            ),
        ] {
            for source in [format!("{left}\n{right}"), format!("{right}\n{left}")] {
                let p = config(&source).resolve_policy(CompilationRequest::JavaScript {
                    preserve_root_exports: true,
                });
                assert!(p.unwrap_err().contains("contradicts"), "{source}");
            }
            let consistent = format!("{}\n{right}", left.replace("false", "true"));
            config(&consistent)
                .resolve_policy(CompilationRequest::JavaScript {
                    preserve_root_exports: true,
                })
                .unwrap();
        }
    }

    #[test]
    fn compatibility_startup_grants_are_diagnosed_until_explicitly_migrated() {
        let legacy = js("effort.level=16\n[policy]");
        assert_eq!(
            legacy
                .diagnostics()
                .iter()
                .filter(|text| text.starts_with("compatibility:"))
                .count(),
            2
        );
        for permission in ["off", "on"] {
            let p = js(&format!("effort.level=16\n[policy.tactics]\nstring-array-packing='{permission}'\nstartup-reconstruction='{permission}'"));
            assert!(p.diagnostics().is_empty(), "{:?}", p.diagnostics());
        }
    }

    #[test]
    fn resolved_script_containers_have_distinct_identity_without_changing_strictness() {
        use crate::config::JavaScriptFormat as F;
        let resolve = |source| {
            config(source)
                .resolve_policy(CompilationRequest::JavaScript {
                    preserve_root_exports: false,
                })
                .unwrap()
        };
        let default = resolve("");
        let iife = resolve("[target.javascript]\nformat='iife'");
        let bare = resolve("[target.javascript]\nformat='bare'");
        assert_eq!(default.delivery().unwrap().format, F::Iife);
        assert_eq!(default.fingerprint(), iife.fingerprint());
        assert_ne!(default.fingerprint(), bare.fingerprint());
        assert_eq!(default.javascript_contract(), bare.javascript_contract());
        assert!(!default
            .javascript_contract()
            .unwrap()
            .execution
            .guarantees_strict_execution());
        assert_eq!(js("").delivery().unwrap().format, F::Esm);
    }

    #[test]
    fn legacy_and_new_permissions_have_the_same_resolved_identity() {
        let old = js("[optimization]\ninlining=false");
        let new = js("[policy]\nversion=2\n[policy.tactics]\ninlining='off'");
        assert_eq!(old.fingerprint(), new.fingerprint());
        assert!(!old.diagnostics().is_empty());
        assert!(new.diagnostics().is_empty());
        assert_eq!(
            TacticId::Inlining.spec().analysis,
            AnalysisRequirement::CallsAndCaptures
        );
    }

    #[test]
    fn logging_and_boundaries_are_independent_of_effort_and_codec() {
        let low = js("effort.level=0\nobjective.codecs='raw'");
        let high = js("effort.level=16\nobjective.codecs='brotli'");
        assert_eq!(low.contract(), high.contract());
        assert!(!low.javascript_contract().unwrap().effects.strip_debug);
        assert_ne!(low.fingerprint(), high.fingerprint());
        let closed = config("")
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: false,
            })
            .unwrap();
        assert_ne!(
            low.javascript_contract().unwrap().world,
            closed.javascript_contract().unwrap().world
        );
    }

    #[test]
    fn the_effort_level_alone_sets_the_walk_and_retired_budgets_change_nothing() {
        // A per-project search budget is retired (M3.5): the product parser
        // drops it with a warning, the effort level's schedule is the only
        // budget, and the schedule is part of the identity.
        let product = |source: &str| {
            let parsed = crate::config::parse_project_config(source).unwrap();
            assert!(!parsed.warnings.is_empty());
            parsed
                .config
                .resolve_policy(CompilationRequest::JavaScript {
                    preserve_root_exports: true,
                })
                .unwrap()
        };
        let one = product("[javascript]\nterminal_codec_probe_limit=1");
        let two = product("[javascript]\nterminal_codec_probe_limit=2");
        assert_eq!(one.fingerprint(), two.fingerprint());
        assert_eq!(one.fingerprint(), js("").fingerprint());
        let low = js("effort.level=8");
        let high = js("effort.level=13");
        assert_ne!(low.fingerprint(), high.fingerprint());
        assert!(low.objective().unwrap().walk.exact < high.objective().unwrap().walk.exact);
        assert_eq!(
            high.receipt()["objective"]["walk"]["version"],
            WALK_SCHEDULE_VERSION
        );
    }

    #[test]
    fn native_policy_ignores_javascript_effort_and_build_choices() {
        let a = config("")
            .resolve_policy(CompilationRequest::Native)
            .unwrap();
        let b = config("effort.level=16\nobjective.codecs='raw'")
            .resolve_policy(CompilationRequest::Native)
            .unwrap();
        assert!(a.objective().is_none());
        assert!(a.javascript_contract().is_none());
        assert_eq!(a.fingerprint(), b.fingerprint());
        for tactic in TacticId::ALL {
            if tactic.spec().javascript_only {
                assert!(!a.tactic(tactic).enabled);
            }
        }
    }

    #[test]
    fn version_and_level_boundaries_are_explicit() {
        for level in [0, 13, 15, 16] {
            assert!(config(&format!("effort.level={level}")).validate().is_ok());
        }
        assert!(config("effort.level=17").validate().is_err());
        for version in [2, 3] {
            assert!(config(&format!("[policy]\nversion={version}"))
                .validate()
                .is_ok());
        }
        for version in [1, 4] {
            assert!(config(&format!("[policy]\nversion={version}"))
                .validate()
                .unwrap_err()
                .contains("expected 2 (legacy startup grants) or 3"));
        }
        assert!(toml::from_str::<ProjectConfig>("[policy.tactics]\nnot-a-tactic='on'").is_err());
    }

    #[test]
    fn exact_size_ranking_does_not_quantize_away_real_bytes() {
        let rank = ObjectiveRank {
            priority: JavaScriptPriority::SizeFirst,
        };
        assert!(rank.rank(100_001, 100_000, 0, 100) > rank.rank(100_000, 100_000, 100, 100));
        let balanced = ObjectiveRank {
            priority: JavaScriptPriority::Balanced,
            ..rank
        };
        assert!(balanced.rank(110, 100, 10, 100) < balanced.rank(100, 100, 100, 100));
        let performance = ObjectiveRank {
            priority: JavaScriptPriority::PerformanceFirst,
            ..rank
        };
        assert!(performance.rank(200, 100, 1, 100) < performance.rank(100, 100, 100, 100));
        assert_eq!(normalized_ratio(u64::MAX, u64::MAX), 10_000);
    }

    #[test]
    fn hard_constraints_precede_size_and_explicit_tactic_permissions() {
        let p = js(
            "[policy.tactics]\nstartup-reconstruction='on'\n[policy.constraints]\nmax_startup_work=4\nmax_runtime_memory_bytes=8\nmax_performance_regression_percent=10",
        );
        let baseline = CandidateCost {
            transfer_bytes: 100,
            performance_score: 100,
            ..CandidateCost::default()
        };
        let cheap = CandidateCost {
            transfer_bytes: 1,
            performance_score: 100,
            startup_work: 5,
            ..CandidateCost::default()
        };
        assert_eq!(
            p.admit(
                &[usage(TacticId::StartupReconstruction, RuntimeRisk::Startup)],
                cheap,
                baseline
            ),
            Err(AdmissionError::Constraint("startup work"))
        );
        let slower = CandidateCost {
            transfer_bytes: 1,
            performance_score: 111,
            ..CandidateCost::default()
        };
        assert_eq!(
            p.admit(&[], slower, baseline),
            Err(AdmissionError::Constraint("performance estimate"))
        );
        let undeclared = CandidateCost {
            recurring_work: 1,
            ..CandidateCost::default()
        };
        assert_eq!(
            p.admit(&[], undeclared, baseline),
            Err(AdmissionError::UndeclaredRuntimeRisk(
                RuntimeRisk::Recurring
            ))
        );
    }

    fn plan() -> BudgetPlan {
        BudgetPlan {
            baseline_work: 10,
            optional_work: 20,
            baseline_retained_bytes: 8,
            retained_bytes: 32,
        }
    }

    #[test]
    fn optional_work_and_memory_cannot_consume_the_direct_artifact_reserve() {
        let mut ledger = BudgetLedger::new(ResourceLimits::default(), plan()).unwrap();
        ledger
            .charge(WorkDomain::Optional, WorkKind::Edit, 20)
            .unwrap();
        assert_eq!(
            ledger.charge(WorkDomain::Optional, WorkKind::Codec, 1),
            Err(BudgetError::WorkExhausted(WorkDomain::Optional))
        );
        assert_eq!(ledger.work_by_kind(WorkKind::Codec), 0);
        ledger
            .charge(WorkDomain::Baseline, WorkKind::Render, 8)
            .unwrap();
        ledger
            .charge(WorkDomain::Baseline, WorkKind::Codec, 2)
            .unwrap();
        ledger.retain(WorkDomain::Optional, 24).unwrap();
        assert_eq!(
            ledger.retain(WorkDomain::Optional, 1),
            Err(BudgetError::MemoryExhausted(WorkDomain::Optional))
        );
        ledger.retain(WorkDomain::Baseline, 8).unwrap();
        assert_eq!(ledger.peak_retained_bytes(), 32);
        ledger.release(WorkDomain::Optional, 24).unwrap();
        assert_eq!(
            ledger.release(WorkDomain::Optional, 1),
            Err(BudgetError::InvalidRelease)
        );
        assert_eq!(ledger.retained_bytes(), 8);
    }

    #[test]
    fn zero_optional_work_still_has_a_measured_direct_route() {
        let p = js(
            "[javascript]\ncandidate_search='off'\n[policy.resources]\nlogical_work=10\nretained_bytes=8",
        );
        assert_eq!(p.objective().unwrap().optional_alternatives, 0);
        let mut unrestricted_caps = js("[javascript]\ncandidate_search='off'")
            .ledger(plan())
            .unwrap();
        assert_eq!(
            unrestricted_caps.charge(WorkDomain::Optional, WorkKind::Edit, 1),
            Err(BudgetError::WorkExhausted(WorkDomain::Optional))
        );
        let mut ledger = p.ledger(plan()).unwrap();
        assert_eq!(
            ledger.charge(WorkDomain::Optional, WorkKind::Edit, 1),
            Err(BudgetError::WorkExhausted(WorkDomain::Optional))
        );
        assert!(p.tactic(TacticId::TargetCompaction).enabled);
        ledger
            .charge(WorkDomain::Baseline, WorkKind::Edit, 3)
            .unwrap();
        ledger
            .charge(WorkDomain::Baseline, WorkKind::Render, 5)
            .unwrap();
        ledger
            .charge(WorkDomain::Baseline, WorkKind::Codec, 2)
            .unwrap();
    }

    #[test]
    fn unavailable_baseline_and_overflow_fail_without_partial_charges() {
        assert_eq!(
            BudgetLedger::new(
                ResourceLimits {
                    logical_work: Some(9),
                    ..ResourceLimits::default()
                },
                plan()
            ),
            Err(BudgetError::BaselineExceedsLimit)
        );
        assert_eq!(
            BudgetLedger::new(
                ResourceLimits::default(),
                BudgetPlan {
                    optional_work: u64::MAX,
                    ..plan()
                }
            ),
            Err(BudgetError::InvalidPlan)
        );
        let mut ledger = BudgetLedger::new(ResourceLimits::default(), plan()).unwrap();
        let before = ledger.clone();
        assert!(ledger
            .charge(WorkDomain::Optional, WorkKind::Analysis, u64::MAX)
            .is_err());
        assert_eq!(ledger, before);
        assert!(ledger.retain(WorkDomain::Baseline, u64::MAX).is_err());
        assert_eq!(ledger, before);
    }

    #[test]
    fn stronger_cached_analysis_cannot_leak_into_lower_effort() {
        let low = AnalysisAttempt {
            plan: 0,
            work_quota: 3,
            algorithm_version: 1,
        };
        let high = AnalysisAttempt {
            plan: 1,
            work_quota: 12,
            algorithm_version: 1,
        };
        let incomplete = AnalysisWorkReceipt {
            attempt: low,
            completion: AnalysisCompletion::Truncated,
            logical_work: 3,
        };
        let completed = AnalysisWorkReceipt {
            attempt: high,
            completion: AnalysisCompletion::Complete,
            logical_work: 7,
        };
        assert!(!incomplete.reusable_for(high));
        let mut cold = BudgetLedger::new(ResourceLimits::default(), plan()).unwrap();
        cold.charge_analysis(WorkDomain::Optional, low, incomplete)
            .unwrap();
        let mut warm = BudgetLedger::new(ResourceLimits::default(), plan()).unwrap();
        assert_eq!(
            warm.charge_analysis(WorkDomain::Optional, low, completed),
            Err(BudgetError::AnalysisAttemptMismatch)
        );
        assert_eq!(warm.work_used(WorkDomain::Optional), 0);
        warm.charge_analysis(WorkDomain::Optional, low, incomplete)
            .unwrap();
        assert_eq!(cold, warm);
        warm.charge_analysis(WorkDomain::Optional, high, completed)
            .unwrap();
        assert_eq!(warm.work_used(WorkDomain::Optional), 10);
    }

    #[test]
    fn deadlines_are_explicit_cooperative_limits() {
        let ledger = BudgetLedger::new(
            ResourceLimits {
                wall_time_ms: Some(20),
                ..ResourceLimits::default()
            },
            plan(),
        )
        .unwrap();
        assert!(!ledger.deadline_reached(19));
        assert!(ledger.deadline_reached(20));
        assert!(config("[policy.resources]\nwall_time_ms=0")
            .validate()
            .is_err());
    }

    fn timed_ledger(elapsed: Duration) -> BudgetLedger {
        let mut ledger = BudgetLedger::new(
            ResourceLimits {
                wall_time_ms: Some(20),
                ..ResourceLimits::default()
            },
            plan(),
        )
        .unwrap();
        ledger.set_deadline_elapsed_for_test(elapsed);
        ledger
    }

    #[test]
    fn deadline_checks_all_work_admissions_without_partial_charges() {
        let mut ledger = timed_ledger(Duration::from_nanos(19_999_999));
        ledger
            .charge(WorkDomain::Baseline, WorkKind::Render, 1)
            .unwrap();
        ledger
            .charge(WorkDomain::Optional, WorkKind::Codec, 2)
            .unwrap();
        ledger.set_deadline_elapsed_for_test(Duration::from_millis(20));
        let before = ledger.clone();
        for domain in [WorkDomain::Baseline, WorkDomain::Optional] {
            for kind in [
                WorkKind::Analysis,
                WorkKind::Edit,
                WorkKind::Render,
                WorkKind::Codec,
            ] {
                // Zero-work boundary checks must also observe the deadline.
                for units in [0, 1, u64::MAX] {
                    assert_eq!(
                        ledger.charge(domain, kind, units),
                        Err(BudgetError::DeadlineExceeded)
                    );
                    assert_eq!(ledger, before);
                }
            }
        }
        let attempt = AnalysisAttempt {
            plan: 0,
            work_quota: 3,
            algorithm_version: 1,
        };
        assert_eq!(
            ledger.charge_analysis(
                WorkDomain::Optional,
                attempt,
                AnalysisWorkReceipt {
                    attempt,
                    completion: AnalysisCompletion::Complete,
                    logical_work: 2,
                }
            ),
            Err(BudgetError::DeadlineExceeded)
        );
        assert_eq!(ledger, before);
    }

    #[test]
    fn deadline_blocks_memory_admission_but_always_allows_cleanup() {
        let mut ledger = timed_ledger(Duration::ZERO);
        ledger.retain(WorkDomain::Baseline, 8).unwrap();
        ledger.retain(WorkDomain::Optional, 24).unwrap();
        ledger.set_deadline_elapsed_for_test(Duration::from_millis(21));
        let before = ledger.clone();
        for domain in [WorkDomain::Baseline, WorkDomain::Optional] {
            for bytes in [0, 1, u64::MAX] {
                assert_eq!(
                    ledger.retain(domain, bytes),
                    Err(BudgetError::DeadlineExceeded)
                );
                assert_eq!(ledger, before);
            }
        }
        ledger.release(WorkDomain::Optional, 24).unwrap();
        ledger.release(WorkDomain::Baseline, 8).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
        assert_eq!(ledger.peak_retained_bytes(), 32);
        assert_eq!(
            ledger.release(WorkDomain::Optional, 1),
            Err(BudgetError::InvalidRelease)
        );
        assert_eq!(ledger.retained_bytes(), 0);
    }

    #[test]
    fn cloned_ledgers_keep_the_compilation_deadline_origin() {
        let original = timed_ledger(Duration::from_millis(19));
        let mut clone = original.clone();
        assert_eq!(clone.deadline, original.deadline);
        assert_eq!(clone.wall_time_ms(), Some(20));
        clone
            .charge(WorkDomain::Optional, WorkKind::Analysis, 1)
            .unwrap();
        clone.set_deadline_elapsed_for_test(Duration::from_millis(20));
        assert_eq!(
            clone.charge(WorkDomain::Optional, WorkKind::Analysis, 1),
            Err(BudgetError::DeadlineExceeded)
        );
        // Re-cloning an expired ledger cannot grant a fresh wall-time window.
        let mut expired_clone = clone.clone();
        assert_eq!(
            expired_clone.retain(WorkDomain::Baseline, 1),
            Err(BudgetError::DeadlineExceeded)
        );
        assert_eq!(
            expired_clone.deadline.as_ref().unwrap().started_at,
            original.deadline.as_ref().unwrap().started_at
        );
    }

    #[test]
    fn absent_deadline_has_no_clock_owner_and_large_limits_do_not_overflow() {
        let mut ledger = BudgetLedger::new(ResourceLimits::default(), plan()).unwrap();
        assert!(ledger.deadline.is_none());
        assert_eq!(ledger.wall_time_ms(), None);
        assert!(!ledger.deadline_reached(u64::MAX));
        ledger
            .charge(WorkDomain::Baseline, WorkKind::Render, 10)
            .unwrap();
        ledger.retain(WorkDomain::Baseline, 8).unwrap();
        ledger.release(WorkDomain::Baseline, 8).unwrap();

        let mut huge = BudgetLedger::new(
            ResourceLimits {
                wall_time_ms: Some(u64::MAX),
                ..ResourceLimits::default()
            },
            plan(),
        )
        .unwrap();
        huge.set_deadline_elapsed_for_test(Duration::from_millis(u64::MAX - 1));
        huge.charge(WorkDomain::Baseline, WorkKind::Render, 1)
            .unwrap();
        huge.set_deadline_elapsed_for_test(Duration::from_millis(u64::MAX));
        assert_eq!(
            huge.charge(WorkDomain::Baseline, WorkKind::Render, 0),
            Err(BudgetError::DeadlineExceeded)
        );
    }

    #[test]
    fn independent_objective_shares_preserve_counters_deadline_and_released_allowance() {
        let mut ledger = BudgetLedger::new_baseline_first(
            ResourceLimits {
                wall_time_ms: Some(100),
                ..ResourceLimits::default()
            },
            BaselineFirstPlan {
                logical_work: 100,
                retained_bytes: 16,
                terminal_work: 0,
            },
        )
        .unwrap();
        ledger
            .charge(WorkDomain::Baseline, WorkKind::Analysis, 10)
            .unwrap();
        ledger.retain(WorkDomain::Baseline, 8).unwrap();
        ledger.seal_baseline().unwrap();
        let first = ledger.begin_objective_share(3);
        assert_eq!(ledger.optional_search_work(), (0, 30));
        ledger
            .charge(WorkDomain::Optional, WorkKind::Analysis, 10)
            .unwrap();
        ledger.end_objective_share(first);
        assert_eq!(ledger.optional_search_work(), (10, 90));
        let second = ledger.begin_objective_share(2);
        assert_eq!(ledger.optional_search_work(), (0, 40));
        ledger
            .charge(WorkDomain::Optional, WorkKind::Render, 40)
            .unwrap();
        assert_eq!(
            ledger.charge(WorkDomain::Optional, WorkKind::Render, 1),
            Err(BudgetError::WorkExhausted(WorkDomain::Optional))
        );
        ledger.end_objective_share(second);
        let third = ledger.begin_objective_share(1);
        assert_eq!(ledger.optional_search_work(), (0, 40));
        ledger.set_deadline_elapsed_for_test(Duration::from_millis(100));
        assert_eq!(
            ledger.charge(WorkDomain::Optional, WorkKind::Render, 0),
            Err(BudgetError::DeadlineExceeded)
        );
        ledger.end_objective_share(third);
        assert_eq!(ledger.work_used(WorkDomain::Optional), 50);
        assert_eq!(ledger.work_by_kind(WorkKind::Analysis), 20);
        assert_eq!(ledger.work_by_kind(WorkKind::Render), 40);
        assert_eq!(ledger.work_limit(WorkDomain::Optional), 90);
        assert_eq!(
            ledger.charge(WorkDomain::Optional, WorkKind::Analysis, 0),
            Err(BudgetError::DeadlineExceeded)
        );
        ledger.release(WorkDomain::Baseline, 8).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    }

    fn baseline_first_plan() -> BaselineFirstPlan {
        BaselineFirstPlan {
            logical_work: 100,
            retained_bytes: 80,
            terminal_work: 7,
        }
    }

    #[test]
    fn baseline_entry_guard_checks_phase_and_deadline_without_mutating_allowances() {
        let fixed = BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 20,
                optional_work: 30,
                baseline_retained_bytes: 0,
                retained_bytes: 80,
            },
        )
        .unwrap();
        let before = fixed.clone();
        assert_eq!(
            fixed.require_preparing_baseline(),
            Err(BudgetError::InvalidBaselinePhase)
        );
        assert_eq!(fixed, before);

        let mut preparing = BudgetLedger::new_baseline_first(
            ResourceLimits {
                wall_time_ms: Some(20),
                ..ResourceLimits::default()
            },
            baseline_first_plan(),
        )
        .unwrap();
        preparing.set_deadline_elapsed_for_test(Duration::from_millis(19));
        preparing
            .charge(WorkDomain::Baseline, WorkKind::Analysis, 3)
            .unwrap();
        preparing.retain(WorkDomain::Baseline, 8).unwrap();
        let before = preparing.clone();
        assert_eq!(preparing.require_preparing_baseline(), Ok(()));
        assert_eq!(preparing, before);

        let mut expired = preparing.clone();
        expired.set_deadline_elapsed_for_test(Duration::from_millis(20));
        let before = expired.clone();
        assert_eq!(
            expired.require_preparing_baseline(),
            Err(BudgetError::DeadlineExceeded)
        );
        assert_eq!(expired, before);
        expired.release(WorkDomain::Baseline, 8).unwrap();

        preparing.seal_baseline().unwrap();
        let before = preparing.clone();
        assert_eq!(
            preparing.require_preparing_baseline(),
            Err(BudgetError::InvalidBaselinePhase)
        );
        assert_eq!(preparing, before);
        preparing.set_deadline_elapsed_for_test(Duration::from_millis(20));
        let before = preparing.clone();
        assert_eq!(
            preparing.require_preparing_baseline(),
            Err(BudgetError::DeadlineExceeded)
        );
        assert_eq!(preparing, before);
        preparing.release(WorkDomain::Baseline, 8).unwrap();
    }

    #[test]
    fn baseline_first_seal_opens_only_actual_remaining_headroom() {
        let mut ledger = BudgetLedger::new_baseline_first(
            ResourceLimits {
                logical_work: Some(80),
                retained_bytes: Some(50),
                ..ResourceLimits::default()
            },
            baseline_first_plan(),
        )
        .unwrap();
        // Mandatory construction can occupy the entire effective memory cap.
        assert!(!ledger.baseline_is_sealed());
        ledger.retain(WorkDomain::Baseline, 50).unwrap();
        ledger
            .charge(WorkDomain::Baseline, WorkKind::Render, 60)
            .unwrap();
        // Target/codec scratch has gone; the artifact and source remain live.
        ledger.release(WorkDomain::Baseline, 35).unwrap();
        assert_eq!(
            ledger.seal_baseline().unwrap(),
            BaselineSeal {
                baseline_work: 60,
                terminal_work: 7,
                optional_work: 13,
                baseline_retained_bytes: 15,
                optional_memory_headroom: 35,
            }
        );
        assert!(ledger.baseline_is_sealed());
        assert_eq!(ledger.retained_bytes_in(WorkDomain::Baseline), 15);
        assert_eq!(ledger.retained_bytes_in(WorkDomain::Optional), 0);
        ledger
            .charge(WorkDomain::Optional, WorkKind::Analysis, 13)
            .unwrap();
        ledger
            .charge(WorkDomain::Baseline, WorkKind::Edit, 7)
            .unwrap();
        assert_eq!(ledger.work_used(WorkDomain::Baseline), 67);
        assert_eq!(ledger.work_used(WorkDomain::Optional), 13);
        assert_eq!(
            ledger.charge(WorkDomain::Optional, WorkKind::Codec, 1),
            Err(BudgetError::WorkExhausted(WorkDomain::Optional))
        );
        assert_eq!(
            ledger.charge(WorkDomain::Baseline, WorkKind::Codec, 1),
            Err(BudgetError::WorkExhausted(WorkDomain::Baseline))
        );
        ledger.retain(WorkDomain::Optional, 35).unwrap();
        assert_eq!(
            ledger.retain(WorkDomain::Optional, 1),
            Err(BudgetError::MemoryExhausted(WorkDomain::Optional))
        );
        // Discarding a Baseline cache frees real headroom, not just a counter
        // hidden beneath a permanently frozen baseline reserve.
        ledger.release(WorkDomain::Baseline, 5).unwrap();
        ledger.retain(WorkDomain::Optional, 5).unwrap();
        assert_eq!(ledger.retained_bytes(), 50);
        assert_eq!(ledger.peak_retained_bytes(), 50);
        ledger.release(WorkDomain::Optional, 40).unwrap();
        assert_eq!(ledger.retained_bytes(), 10);
        ledger.release(WorkDomain::Baseline, 10).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    }

    #[test]
    fn baseline_first_phase_gates_are_atomic_including_zero_admissions() {
        let mut ledger =
            BudgetLedger::new_baseline_first(ResourceLimits::default(), baseline_first_plan())
                .unwrap();
        let before = ledger.clone();
        for amount in [0, 1, u64::MAX] {
            for kind in [
                WorkKind::Analysis,
                WorkKind::Edit,
                WorkKind::Render,
                WorkKind::Codec,
            ] {
                assert_eq!(
                    ledger.charge(WorkDomain::Optional, kind, amount),
                    Err(BudgetError::BaselineNotSealed)
                );
                assert_eq!(ledger, before);
            }
            assert_eq!(
                ledger.retain(WorkDomain::Optional, amount),
                Err(BudgetError::BaselineNotSealed)
            );
            assert_eq!(ledger, before);
        }
        ledger
            .charge(WorkDomain::Baseline, WorkKind::Render, 1)
            .unwrap();
        ledger.retain(WorkDomain::Baseline, 8).unwrap();
        ledger.seal_baseline().unwrap();
        let sealed = ledger.clone();
        assert_eq!(
            ledger.seal_baseline(),
            Err(BudgetError::InvalidBaselinePhase)
        );
        assert_eq!(ledger, sealed);
        for amount in [1, u64::MAX] {
            assert_eq!(
                ledger.retain(WorkDomain::Baseline, amount),
                Err(BudgetError::BaselineAllocationSealed)
            );
            assert_eq!(ledger, sealed);
        }
        ledger.retain(WorkDomain::Baseline, 0).unwrap();
        ledger
            .charge(WorkDomain::Baseline, WorkKind::Render, 0)
            .unwrap();
        assert_eq!(ledger, sealed);
        ledger.release(WorkDomain::Baseline, 8).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    }

    #[test]
    fn baseline_first_terminal_work_is_reserved_before_and_after_seal() {
        let mut ledger = BudgetLedger::new_baseline_first(
            ResourceLimits::default(),
            BaselineFirstPlan {
                logical_work: 12,
                retained_bytes: 16,
                terminal_work: 4,
            },
        )
        .unwrap();
        ledger
            .charge(WorkDomain::Baseline, WorkKind::Render, 8)
            .unwrap();
        let before = ledger.clone();
        assert_eq!(
            ledger.charge(WorkDomain::Baseline, WorkKind::Codec, 1),
            Err(BudgetError::WorkExhausted(WorkDomain::Baseline))
        );
        assert_eq!(ledger, before);
        let sealed = ledger.seal_baseline().unwrap();
        assert_eq!(sealed.optional_work, 0);
        assert_eq!(
            ledger.charge(WorkDomain::Optional, WorkKind::Edit, 1),
            Err(BudgetError::WorkExhausted(WorkDomain::Optional))
        );
        ledger
            .charge(WorkDomain::Baseline, WorkKind::Edit, 4)
            .unwrap();
        assert_eq!(ledger.work_used(WorkDomain::Baseline), 12);

        let mut no_terminal = BudgetLedger::new_baseline_first(
            ResourceLimits::default(),
            BaselineFirstPlan {
                logical_work: 12,
                retained_bytes: 16,
                terminal_work: 0,
            },
        )
        .unwrap();
        no_terminal
            .charge(WorkDomain::Baseline, WorkKind::Render, 3)
            .unwrap();
        assert_eq!(no_terminal.seal_baseline().unwrap().optional_work, 9);
        no_terminal
            .charge(WorkDomain::Baseline, WorkKind::Edit, 0)
            .unwrap();
        assert_eq!(
            no_terminal.charge(WorkDomain::Baseline, WorkKind::Edit, 1),
            Err(BudgetError::WorkExhausted(WorkDomain::Baseline))
        );
    }

    #[test]
    fn baseline_first_limits_cap_plans_and_reject_unavailable_terminal_reserve() {
        assert_eq!(
            BudgetLedger::new_baseline_first(
                ResourceLimits::default(),
                BaselineFirstPlan {
                    terminal_work: 101,
                    ..baseline_first_plan()
                }
            ),
            Err(BudgetError::InvalidPlan)
        );
        assert_eq!(
            BudgetLedger::new_baseline_first(
                ResourceLimits {
                    logical_work: Some(6),
                    ..ResourceLimits::default()
                },
                baseline_first_plan()
            ),
            Err(BudgetError::BaselineExceedsLimit)
        );
        let mut capped = BudgetLedger::new_baseline_first(
            ResourceLimits {
                logical_work: Some(200),
                retained_bytes: Some(160),
                ..ResourceLimits::default()
            },
            baseline_first_plan(),
        )
        .unwrap();
        // Larger user ceilings never enlarge the finite supplied plan.
        capped
            .charge(WorkDomain::Baseline, WorkKind::Render, 93)
            .unwrap();
        assert_eq!(
            capped.charge(WorkDomain::Baseline, WorkKind::Render, 1),
            Err(BudgetError::WorkExhausted(WorkDomain::Baseline))
        );
        capped.retain(WorkDomain::Baseline, 80).unwrap();
        assert_eq!(
            capped.retain(WorkDomain::Baseline, 1),
            Err(BudgetError::MemoryExhausted(WorkDomain::Baseline))
        );
        assert_eq!(capped.seal_baseline().unwrap().optional_memory_headroom, 0);
        capped.release(WorkDomain::Baseline, 80).unwrap();
    }

    #[test]
    fn baseline_first_deadline_seal_failure_and_clone_keep_cleanup_available() {
        let mut ledger = BudgetLedger::new_baseline_first(
            ResourceLimits {
                wall_time_ms: Some(20),
                ..ResourceLimits::default()
            },
            baseline_first_plan(),
        )
        .unwrap();
        ledger.set_deadline_elapsed_for_test(Duration::from_millis(19));
        ledger.retain(WorkDomain::Baseline, 8).unwrap();
        ledger
            .charge(WorkDomain::Baseline, WorkKind::Render, 3)
            .unwrap();
        let mut expired = ledger.clone();
        assert_eq!(expired.phase, ledger.phase);
        assert_eq!(expired.deadline, ledger.deadline);
        expired.set_deadline_elapsed_for_test(Duration::from_millis(20));
        let before = expired.clone();
        assert_eq!(expired.seal_baseline(), Err(BudgetError::DeadlineExceeded));
        assert_eq!(expired, before);
        expired.release(WorkDomain::Baseline, 8).unwrap();
        assert_eq!(expired.retained_bytes(), 0);

        ledger.seal_baseline().unwrap();
        ledger.retain(WorkDomain::Optional, 10).unwrap();
        ledger.set_deadline_elapsed_for_test(Duration::from_millis(20));
        let mut sealed_clone = ledger.clone();
        assert_eq!(sealed_clone.phase, BudgetPhase::SealedBaseline);
        assert_eq!(sealed_clone.deadline, ledger.deadline);
        for domain in [WorkDomain::Baseline, WorkDomain::Optional] {
            assert_eq!(
                sealed_clone.charge(domain, WorkKind::Render, 0),
                Err(BudgetError::DeadlineExceeded)
            );
            assert_eq!(
                sealed_clone.retain(domain, 0),
                Err(BudgetError::DeadlineExceeded)
            );
        }
        sealed_clone.release(WorkDomain::Baseline, 8).unwrap();
        sealed_clone.release(WorkDomain::Optional, 10).unwrap();
        assert_eq!(sealed_clone.retained_bytes(), 0);
        assert_eq!(sealed_clone.peak_retained_bytes(), 18);
    }

    #[test]
    fn baseline_first_fixed_partition_compatibility_and_extreme_caps() {
        let mut fixed = BudgetLedger::new(ResourceLimits::default(), plan()).unwrap();
        assert!(!fixed.baseline_is_sealed());
        let before = fixed.clone();
        assert_eq!(
            fixed.seal_baseline(),
            Err(BudgetError::InvalidBaselinePhase)
        );
        assert_eq!(fixed, before);
        fixed
            .charge(WorkDomain::Optional, WorkKind::Render, 20)
            .unwrap();
        fixed.retain(WorkDomain::Optional, 24).unwrap();
        fixed.retain(WorkDomain::Baseline, 8).unwrap();
        fixed.release(WorkDomain::Baseline, 8).unwrap();
        // Fixed-plan callers retain their original reserved memory floor.
        assert_eq!(
            fixed.retain(WorkDomain::Optional, 1),
            Err(BudgetError::MemoryExhausted(WorkDomain::Optional))
        );

        let mut huge = BudgetLedger::new_baseline_first(
            ResourceLimits::default(),
            BaselineFirstPlan {
                logical_work: u64::MAX,
                retained_bytes: u64::MAX,
                terminal_work: 1,
            },
        )
        .unwrap();
        assert!(huge.deadline.is_none());
        huge.charge(WorkDomain::Baseline, WorkKind::Render, u64::MAX - 2)
            .unwrap();
        huge.retain(WorkDomain::Baseline, u64::MAX).unwrap();
        let receipt = huge.seal_baseline().unwrap();
        assert_eq!(receipt.optional_work, 1);
        assert_eq!(receipt.optional_memory_headroom, 0);
        huge.charge(WorkDomain::Optional, WorkKind::Analysis, 1)
            .unwrap();
        huge.charge(WorkDomain::Baseline, WorkKind::Edit, 1)
            .unwrap();
        huge.release(WorkDomain::Baseline, u64::MAX).unwrap();
        huge.retain(WorkDomain::Optional, u64::MAX).unwrap();
        assert_eq!(huge.retained_bytes(), u64::MAX);
    }
}
