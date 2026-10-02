//! The build: source to delivered artifacts, the compiler's public API.
//! Unsupported input is diagnosed here; no text rewrite runs.
#[path = "build_cache.rs"]
mod cache;
#[path = "build_decisions.rs"]
mod decisions;
#[path = "build_graph.rs"]
mod graph;
#[path = "build_native.rs"]
mod native;
pub use native::{check_source_for_target, check_entries_for_target};
use native::NativeSources;
pub use graph::{GraphSession, GraphSessionStats};
#[path = "build_outputs.rs"]
mod outputs;
pub use outputs::{OutputPolicies, ServiceOutput};

use std::fmt;
use std::path::Path;
use std::time::Instant;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::check::{
    with_analyzed_modules_with_contract, with_analyzed_source_with_contract,
    AdmittedCheckError, CheckedModules,
};
#[cfg(test)]
use crate::check::{with_analyzed_modules, with_analyzed_source};
use crate::compilation_policy::{
    BaselineFirstPlan, BudgetLedger, CompilationRequest, ResolvedPolicy, TacticId, WorkDomain,
    WorkKind,
};
use crate::config::ProjectConfig;
use crate::js::selection::{Objective, Objectives, Plan, Sizes};
use crate::module::{
    discover_parsed_modules_admitted, EntrySource, ModuleDiscoveryError, ModuleError, ModuleSet,
    StableSourceArena,
};
use crate::output_budget::AllocationBudget;
pub use crate::output_budget::AllocationError as ServiceResourceError;
use crate::parser::{AdmittedArena, AdmittedParseError};
use crate::program::call_graph::Seal;
use crate::program::facts::CacheLimits;
#[cfg(test)]
use crate::program::from_checked_source_admitted;
use crate::program::publication::*;
use crate::program::rules::RuleRequest;
use crate::program::{
    from_checked_modules_with_rules, from_checked_source_with_rules, ConversionError, RevisionId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceTarget {
    JavaScript,
    Native,
    All,
}

/// The extension delivered chunk files share with their entry file, as the
/// old route named them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChunkExtension {
    #[default]
    Js,
    Mjs,
}

impl ChunkExtension {
    /// `.mjs` for an `.mjs` entry, otherwise `.js`.
    pub fn of(entry: &Path) -> Self {
        match entry.extension().and_then(|extension| extension.to_str()) {
            Some("mjs") => Self::Mjs,
            _ => Self::Js,
        }
    }
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Js => "js",
            Self::Mjs => "mjs",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ServiceOptions {
    pub target: ServiceTarget,
    pub preserve_root_exports: bool,
    /// Delivered chunk file names end in this.
    pub chunk_extension: ChunkExtension,
    /// None requests the TOML objective set; Some overrides that set.
    pub objectives: Option<Objectives>,
    /// Finite defaults, further restricted by policy.resources.
    pub logical_work: u64,
    pub retained_bytes: u64,
}

impl ServiceOptions {
    /// Native libraries retain the checked public interface. Resolving this
    /// before target policies keeps CLI print-policy and service builds equal.
    pub fn for_config(mut self,config:&ProjectConfig)->Self {
        if self.native_request().is_some() && config.target.native.artifact.is_library() {self.preserve_root_exports=true;}
        self
    }
    /// Resolve the single-objective policy, including an explicit API codec
    /// override and finite caller ceilings. For a set this is the configured
    /// primary codec if requested, otherwise the first requested codec. The
    /// build's javascript_policies report lists every independently resolved
    /// policy. A whole-build cache identity must also
    /// include the objective request, input graph and compiler identity.
    pub fn resolve_policy(
        &self,
        config: &ProjectConfig,
        request: CompilationRequest,
    ) -> Result<ResolvedPolicy, String> {
        let objective = match request {
            CompilationRequest::JavaScript { .. } => {
                let requested = self.requested_objectives(config)?;
                let primary = config.objective.codec();
                Some(
                    requested
                        .iter()
                        .find(|codec| *codec == primary)
                        .unwrap_or_else(|| requested.iter().next().unwrap()),
                )
            }
            CompilationRequest::Native => None,
        };
        self.resolve_objective_policy(config, request, objective)
    }

    /// Canonical effective request: an explicit API override, or the TOML set.
    pub fn requested_objectives(&self, config: &ProjectConfig) -> Result<Objectives, String> {
        self.objectives
            .map(Objectives::normalized)
            .or_else(|| Objectives::from_codecs(&config.objective.codecs))
            .ok_or_else(|| "`objective.codecs` names no codec".to_string())
    }

    /// Resolve once per requested JavaScript objective, before source work.
    /// Native-only builds return no JavaScript policies.
    pub fn resolve_javascript_policies(
        &self,
        config: &ProjectConfig,
    ) -> Result<Vec<ResolvedPolicy>, String> {
        let Some(request) = self.javascript_request() else {
            return Ok(Vec::new());
        };
        self.requested_objectives(config)?
            .iter()
            .map(|codec| self.resolve_objective_policy(config, request, Some(codec)))
            .collect()
    }

    fn resolve_objective_policy(
        &self,
        config: &ProjectConfig,
        request: CompilationRequest,
        objective: Option<Objective>,
    ) -> Result<ResolvedPolicy, String> {
        config.resolve_policy_for_objective(
            request,
            crate::compilation_policy::ResourceLimits {
                logical_work: Some(self.logical_work),
                retained_bytes: Some(self.retained_bytes),
                wall_time_ms: None,
            },
            objective,
        )
    }

    /// The JavaScript policy request this service resolves, if it builds
    /// JavaScript. `--print-policy` resolves exactly this.
    pub fn javascript_request(&self) -> Option<CompilationRequest> {
        (self.target != ServiceTarget::Native).then_some(CompilationRequest::JavaScript {
            preserve_root_exports: self.preserve_root_exports,
        })
    }

    /// The native policy request this service resolves, if it builds C.
    pub fn native_request(&self) -> Option<CompilationRequest> {
        (self.target != ServiceTarget::JavaScript).then_some(CompilationRequest::Native)
    }
}

impl Default for ServiceOptions {
    fn default() -> Self {
        Self {
            target: ServiceTarget::JavaScript,
            preserve_root_exports: true,
            chunk_extension: ChunkExtension::Js,
            objectives: None,
            logical_work: 200_000_000,
            retained_bytes: 256_000_000,
        }
    }
}

#[derive(Debug)]
pub struct ServiceError {
    pub phase: &'static str,
    pub message: String,
    pub diagnostic: Option<ModuleError>,
    /// Typed frontend resource refusal; no source diagnostic is copied for it.
    pub resource: Option<ServiceResourceError>,
}

impl ServiceError {
    fn new(phase: &'static str, error: impl fmt::Debug) -> Self {
        Self {
            phase,
            message: format!("{error:?}"),
            diagnostic: None,
            resource: None,
        }
    }

    /// A refusal the output makes on purpose (a delivery plan it cannot
    /// deliver, a construct the edition lacks) reads as its reason; any
    /// other failure keeps its debug form.
    fn output(phase: &'static str, error: impl fmt::Debug + OutputReason) -> Self {
        Self {
            phase,
            message: error.reason().unwrap_or_else(|| format!("{error:?}")),
            diagnostic: None,
            resource: None,
        }
    }

    fn module(phase: &'static str, error: ModuleError) -> Self {
        Self {
            phase,
            message: error.to_string(),
            diagnostic: Some(error),
            resource: None,
        }
    }

    fn resources(phase: &'static str, error: ServiceResourceError) -> Self {
        Self {
            phase,
            message: format!("{error:?}"),
            diagnostic: None,
            resource: Some(error),
        }
    }
}

/// The reason an output refused, when it refused on purpose.
trait OutputReason {
    fn reason(&self) -> Option<String>;
}
impl OutputReason for CandidateError {
    fn reason(&self) -> Option<String> {
        match self {
            Self::Output(
                error @ (crate::js::extract::OutputError::Invalid(_)
                | crate::js::extract::OutputError::Syntax { .. }),
            ) => Some(error.to_string()),
            _ => None,
        }
    }
}
impl OutputReason for SearchError {
    fn reason(&self) -> Option<String> {
        match self {
            Self::Candidate(error) => error.reason(),
            _ => None,
        }
    }
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "compiler ({}): {}", self.phase, self.message)
    }
}
impl std::error::Error for ServiceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Some(error) = &self.resource {
            Some(error)
        } else {
            self.diagnostic
                .as_ref()
                .map(|error| error as &dyn std::error::Error)
        }
    }
}

/// The service returns immutable delivered bytes with their exact scores.
#[derive(Debug, serde::Serialize)]
pub struct ServiceJavaScript {
    /// The unplanned single file. Planned output is stored in `files`.
    javascript: String,
    /// Every file of a delivery plan, with its measured sizes. A plan may
    /// deliver a single file; unplanned output leaves this empty.
    files: Vec<crate::program::publication::DeliveredFile>,
    /// The plan's files, entries and links (manifest v3).
    layout: Option<crate::js::delivery::DeliveredLayout>,
    sha256: String,
    sizes: Sizes,
    details: Value,
}

impl ServiceJavaScript {
    /// The delivered JavaScript when there is exactly one file; empty for
    /// multiple files, including an external source map alongside its code.
    pub fn javascript(&self) -> &str {
        if let [file] = self.files.as_slice() { &file.code } else { &self.javascript }
    }
    /// Every file of a planned delivery, in plan order.
    pub fn files(&self) -> &[crate::program::publication::DeliveredFile] {
        &self.files
    }
    /// The delivery plan's layout, including planned single-file output.
    pub fn layout(&self) -> Option<&crate::js::delivery::DeliveredLayout> {
        self.layout.as_ref()
    }
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
    pub fn sizes(&self) -> Sizes {
        self.sizes
    }
    pub fn details(&self) -> &Value {
        &self.details
    }
}

#[derive(Debug, serde::Serialize)]
pub struct ServiceCompilation {
    javascript: Vec<ServiceJavaScript>,
    winners: [Option<usize>; 3],
    outputs: Vec<ServiceOutput>,
    native_c: Option<String>,
    native_header: Option<String>,
    report: Value,
}

impl ServiceCompilation {
    pub fn javascript(&self, codec: Objective) -> Option<&ServiceJavaScript> {
        self.winners[codec_index(codec)].map(|index| &self.javascript[index])
    }
    pub fn native_c(&self) -> Option<&str> {
        self.native_c.as_deref()
    }
    pub fn native_header(&self) -> Option<&str> { self.native_header.as_deref() }
    pub fn report(&self) -> &Value {
        &self.report
    }
}

struct Frontend<'src> {
    started: Instant,
    options: ServiceOptions,
    javascript: Option<ResolvedPolicy>,
    independent_javascript: Option<Vec<ResolvedPolicy>>,
    additional_outputs: Vec<OutputPolicies>,
    native: Option<ResolvedPolicy>,
    ledger: BudgetLedger,
    phases: Value,
    source_buffer_bytes: Option<u64>,
    /// Relative host modules every output carries.
    hosts: crate::host_modules::HostDelivery,
    source_maps: Option<crate::source_maps::PreparedSources>,
    native_bindings: Vec<(crate::program::CellId, String)>,
    native_sources: NativeSources<'src>,
    decisions: decisions::Request,
}

#[cfg(test)]
thread_local! {
    /// Set by tests of the publication layer's rewrite lineage, which need a
    /// published program whose literals the rules have not already folded.
    pub(crate) static SKIP_PROGRAM_RULES: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    /// Set by tests of formation's helper inlining (retired by M9.1), whose
    /// fixtures need the helpers the program's inlining rule would remove.
    pub(crate) static SKIP_PROGRAM_INLINING: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

impl<'src> Frontend<'src> {
    /// End all borrowed diagnostic inputs before the factory frees its arena.
    fn finish_inputs(self) -> BudgetLedger {
        let Self { mut ledger, native_sources, source_maps, .. } = self;
        native_sources.discard(&mut ledger);
        if let Some(maps) = source_maps { maps.discard(&mut ledger); }
        ledger
    }
    fn wants_source_maps(&self) -> bool {
        self.javascript.as_ref().into_iter().chain(self.independent_javascript.iter().flatten()).chain(self.additional_outputs.iter().flat_map(|output| &output.policies))
            .any(|policy| policy.delivery().is_some_and(|delivery| delivery.container.source_maps != crate::config::SourceMaps::Off))
    }
    fn trap_index_reads(&self) -> bool {
        self.native.is_some() || self.javascript.as_ref().and_then(ResolvedPolicy::javascript_contract)
            .is_some_and(|contract| contract.checks == crate::compilation_contract::PreconditionChecks::Development)
    }
    fn source_contract(&self) -> crate::config::LanguageConfig {
        self.javascript.as_ref().or(self.native.as_ref())
            .expect("a checked frontend has a target policy").source_contract()
    }
    /// The program rules this build's contracts permit (`program/rules/`):
    /// each where every requested target permits its tactic, with root
    /// storage sealed as the JavaScript execution seals it (a native program
    /// is closed).
    fn rules(&self) -> Option<RuleRequest> {
        #[cfg(test)]
        if SKIP_PROGRAM_RULES.with(std::cell::Cell::get) {
            return None;
        }
        let policies: Vec<&ResolvedPolicy> = [&self.javascript, &self.native]
            .into_iter()
            .flatten()
            .chain(self.independent_javascript.iter().flatten())
            .chain(self.additional_outputs.iter().flat_map(|output| &output.policies))
            .collect();
        if policies.is_empty() {
            return None;
        }
        let permitted = |tactic: TacticId| {
            policies.iter().all(|policy| {
                tactic.spec().producer_enabled(
                    crate::compilation_policy::TacticProducer::SharedRules,
                    policy,
                )
            })
        };
        let seal = self
            .javascript
            .as_ref()
            .and_then(ResolvedPolicy::javascript_contract)
            .map_or(Seal::Module, |contract| {
                Seal::from_execution(contract.execution)
            });
        #[cfg(test)]
        let permitted_inlining =
            permitted(TacticId::Inlining) && !SKIP_PROGRAM_INLINING.with(std::cell::Cell::get);
        #[cfg(not(test))]
        let permitted_inlining = permitted(TacticId::Inlining);
        Some(RuleRequest {
            reuse_normalization: policies.iter().all(|policy| policy.cache().normalization_reuse),
            unroll: permitted(TacticId::LoopUnrolling),
            choices: crate::representation::RegionalChoices::permitted(&policies),
            pool: policies.iter().filter(|policy| policy.javascript_contract().is_some())
                .all(|policy| policy.tactic(TacticId::StringPooling).enabled),
            fold: permitted(TacticId::ConstantFolding),
            dead_code: permitted(TacticId::DeadCodeElimination),
            inline: permitted_inlining,
            scalar: permitted(TacticId::ScalarReplacement),
            native: self.native.is_some(),
            pristine_builtins: self
                .javascript
                .as_ref()
                .and_then(ResolvedPolicy::javascript_contract)
                .is_none_or(|contract| contract.assumptions.pristine_builtins),
            seal,
        })
    }

    fn new(config: &ProjectConfig, options: ServiceOptions) -> Result<Self, ServiceError> {
        let started = Instant::now();
        let options=options.for_config(config);
        let additional_outputs = options.resolve_additional_outputs(config)
            .map_err(|error| ServiceError::new("policy", error))?;
        let mut options = options;
        if options.javascript_request().is_some() {
            options.objectives = Some(
                options
                    .requested_objectives(config)
                    .map_err(|error| ServiceError::new("policy", error))?,
            );
        }
        let mut policies = options
            .resolve_javascript_policies(config)
            .map_err(|error| ServiceError::new("policy", error))?;
        let (javascript, independent_javascript) = if policies.len() <= 1 {
            (policies.pop(), None)
        } else {
            let primary = policies
                .iter()
                .find(|policy| policy.objective().unwrap().codec == config.objective.codec())
                .unwrap_or(&policies[0])
                .clone();
            (Some(primary), Some(policies))
        };
        let native = options
            .native_request()
            .map(|request| options.resolve_policy(config, request))
            .transpose()
            .map_err(|error| ServiceError::new("policy", error))?;
        let policy = javascript.as_ref().or(native.as_ref()).unwrap();
        let ledger = BudgetLedger::new_baseline_first(
            policy.resources(),
            BaselineFirstPlan {
                logical_work: options.logical_work,
                retained_bytes: options.retained_bytes,
                terminal_work: 0,
            },
        )
        .map_err(|error| ServiceError::resources("resources", error.into()))?;
        let mut frontend = Self {
            started,
            options,
            javascript,
            independent_javascript,
            additional_outputs,
            native,
            ledger,
            phases: json!({"policy_ns": nanos(started)}),
            source_buffer_bytes: None,
            hosts: Default::default(),
            source_maps: None,
            native_bindings: Vec::new(),
            native_sources: NativeSources::default(),
            decisions: decisions::Request::new(config, options)?,
        };
        frontend.checkpoint(1)?;
        Ok(frontend)
    }

    fn checkpoint(&mut self, work: u64) -> Result<(), ServiceError> {
        self.ledger
            .charge(WorkDomain::Baseline, WorkKind::Analysis, work)
            .map_err(|error| ServiceError::resources("frontend resources", error.into()))
    }

    fn adopt(
        mut self,
        prepared: PreparedProgram<'src>,
        inputs: Value,
    ) -> Result<CheckedSourceSession<'src>, (ServiceError, BudgetLedger)> {
        if let Err(error) = self.checkpoint(0) {
            self.native_sources.discard(&mut self.ledger);
            if let Some(maps) = self.source_maps.take() { maps.discard(&mut self.ledger); }
            prepared.discard(&mut self.ledger);
            #[cfg(test)]
            record_retained("prepared-discard", self.ledger.retained_bytes());
            return Err((error, self.ledger));
        }
        let Self {
            started,
            options,
            javascript,
            independent_javascript,
            additional_outputs,
            native,
            ledger,
            mut phases,
            source_buffer_bytes,
            hosts,
            source_maps,
            native_bindings,
            native_sources,
            mut decisions,
        } = self;
        let frontend_work = ledger.work_used(WorkDomain::Baseline);
        let source_identity = digest(serde_json::to_vec(&inputs).unwrap());
        decisions.identify(&inputs);
        let program = prepared.program();
        let shape = json!({
            "modules": program.modules().len(), "units": program.units().len(),
            "operations": program.units().iter().map(|unit| unit.data().operations.len()).sum::<usize>(),
            "cells": program.cells().len(),
        });
        let phase = Instant::now();
        let mut compilation =
            match Compilation::new_preserving_ledger(ledger, CheckpointLimit { max_live: 128 }) {
                Ok(compilation) => compilation,
                Err((mut ledger, error)) => {
                    native_sources.discard(&mut ledger);
                    if let Some(maps) = source_maps { maps.discard(&mut ledger); }
                    prepared.discard(&mut ledger);
                    #[cfg(test)]
                    record_retained("prepared-discard", ledger.retained_bytes());
                    return Err((ServiceError::new("adoption", error), ledger));
                }
            };
        if let Some(maps) = source_maps { compilation.adopt_source_maps(maps); }
        let source = match compilation.adopt_prepared(prepared) {
            Ok(source) => source,
            Err(error) => {
                let mut ledger = compilation.finish();
                native_sources.discard(&mut ledger);
                return Err((ServiceError::new("adoption", error), ledger));
            }
        };
        // Module paths name delivered files (`[path]`, `[name]`); they
        // never affect a program: each relative to the common source
        // directory, without extension.
        let paths = inputs["modules"]
            .as_array()
            .map(|modules| {
                modules
                    .iter()
                    .map(|module| {
                        std::path::PathBuf::from(module["path"].as_str().unwrap_or("module"))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let names = module_paths(&paths);
        if let Err(error) = compilation.set_module_names(names) {
            let mut ledger = compilation.finish();
            native_sources.discard(&mut ledger);
            return Err((ServiceError::new("adoption", error), ledger));
        }
        compilation.set_chunk_extension(options.chunk_extension.as_str());
        if !hosts.is_empty() {
            if let Err(error) = compilation.set_host_modules(hosts) {
                let mut ledger = compilation.finish();
                native_sources.discard(&mut ledger);
                return Err((ServiceError::new("adoption", error), ledger));
            }
        }
        phases["adopt_ns"] = json!(nanos(phase));
        Ok(CheckedSourceSession {
            started,
            options,
            javascript,
            independent_javascript,
            additional_outputs,
            native,
            compilation,
            source,
            phases,
            frontend_work,
            source_identity,
            inputs,
            shape,
            source_buffer_bytes,
            native_bindings,
            native_sources,
            decisions,
        })
    }
}

/// One checked source and one compilation owner. Path clients borrow source text
/// inside the factory's scope; parser/checker storage is gone before this exists.
pub struct CheckedSourceSession<'src> {
    started: Instant,
    options: ServiceOptions,
    javascript: Option<ResolvedPolicy>,
    independent_javascript: Option<Vec<ResolvedPolicy>>,
    additional_outputs: Vec<OutputPolicies>,
    native: Option<ResolvedPolicy>,
    compilation: Compilation<'src>,
    source: SemanticId,
    phases: Value,
    frontend_work: u64,
    source_identity: String,
    inputs: Value,
    shape: Value,
    source_buffer_bytes: Option<u64>,
    native_bindings: Vec<(crate::program::CellId, String)>,
    native_sources: NativeSources<'src>,
    decisions: decisions::Request,
}

/// Independently optimized winners retain each objective's qualification.
/// A direct multi-coordinate rendering may still share one artifact.
#[derive(Debug)]
pub struct ServiceJavaScriptBatch {
    artifacts: Vec<ServiceJavaScript>,
    winners: [Option<usize>; 3],
    report: Value,
}
impl ServiceJavaScriptBatch {
    pub fn javascript(&self, codec: Objective) -> Option<&ServiceJavaScript> {
        self.winners[codec_index(codec)].map(|index| &self.artifacts[index])
    }
    pub fn report(&self) -> &Value {
        &self.report
    }
    pub fn into_parts(self) -> (Vec<ServiceJavaScript>, [Option<usize>; 3], Value) {
        (self.artifacts, self.winners, self.report)
    }
}

/// Final receipt after the compilation and any factory-owned source buffers drop.
pub struct FinishedSourceSession {
    pub report: Value,
    pub ledger: BudgetLedger,
}

impl<'src> CheckedSourceSession<'src> {
    pub fn source(&self) -> SemanticId {
        self.source
    }
    pub fn compilation(&self) -> &Compilation<'src> {
        &self.compilation
    }
    pub fn compilation_mut(&mut self) -> &mut Compilation<'src> {
        &mut self.compilation
    }
    /// Borrow the existing owner and primary policy together without cloning it.
    pub fn parts_mut(&mut self) -> (&mut Compilation<'src>, SemanticId, &ResolvedPolicy) {
        (
            &mut self.compilation,
            self.source,
            self.javascript.as_ref().or(self.native.as_ref()).unwrap(),
        )
    }
    pub fn inputs(&self) -> &Value {
        &self.inputs
    }
    pub fn shape(&self) -> &Value {
        &self.shape
    }
    pub fn phases(&self) -> &Value {
        &self.phases
    }

    fn objectives(&self) -> Result<Objectives, ServiceError> {
        let policy = self
            .javascript
            .as_ref()
            .ok_or_else(|| ServiceError::new("javascript", "not a JavaScript session"))?;
        Ok(self
            .options
            .objectives
            .unwrap_or(Objectives::One(policy.objective().unwrap().codec)))
    }

    /// Render an existing candidate directly, measure every requested coordinate,
    /// then qualify before any bytes leave the common artifact owner.
    pub fn render_javascript(
        &mut self,
        candidate: CandidateId,
    ) -> Result<ServiceJavaScript, ServiceError> {
        let started = Instant::now();
        let objectives = self.objectives()?;
        let policy = self.javascript.as_ref().unwrap();
        let style = *Plan::seeds_for_policy(policy)
            .map_err(|error| ServiceError::new("javascript", error))?
            .first()
            .ok_or_else(|| ServiceError::new("javascript", "no eligible naming plan"))?;
        let mut codec_ns = 0;
        let artifact = self
            .compilation
            .with_javascript_output(candidate, policy, |output| {
                let artifact = output.render(&Plan::new(style))?;
                for codec in objectives.iter() {
                    let phase = Instant::now();
                    output.measure(artifact, codec)?;
                    codec_ns += nanos(phase);
                }
                output.retain_artifact(artifact)
            })
            .map_err(|error| ServiceError::output("javascript", error))?
            .map_err(|error| ServiceError::output("javascript", error))?;
        let mut qualified = None;
        for codec in objectives.iter() {
            match self.compilation.qualify_artifact(
                artifact,
                policy,
                codec,
                ArtifactRuntimeEvidence::default(),
                None,
                WorkDomain::Baseline,
            ) {
                Ok(receipt) => qualified = Some(receipt),
                Err(error) => {
                    self.compilation
                        .discard_artifact(artifact)
                        .map_err(|error| ServiceError::new("javascript cleanup", error))?;
                    return Err(ServiceError::new("javascript admission", error));
                }
            }
        }
        let mut delivered = deliver_javascript(&mut self.compilation, qualified.unwrap(), self.decisions.is_writing())?;
        let total_ns = nanos(started);
        delivered.details["formation_naming_render_ns"] = json!(total_ns - codec_ns);
        delivered.details["codec_ns"] = json!(codec_ns);
        delivered.details["complete_artifact_ns"] = json!(total_ns);
        delivered.details["timing_scope"] =
            json!("formation, qualification, exact codecs and terminal delivery metadata");
        Ok(delivered)
    }

    /// Search is the baseline-sealing lifecycle transition. This session must not
    /// reset its ledger to perform later unreserved Baseline edits.
    pub fn search_javascript(
        &mut self,
        source: SemanticId,
        observe: impl FnMut(SearchObservation<'_>),
    ) -> Result<ServiceJavaScriptBatch, ServiceError> {
        let objectives = self.objectives()?;
        // A session client may publish another semantic source. The lock was
        // fingerprinted for this factory's original checked root only.
        let no_decisions = [None, None, None];
        let decisions = if source == self.source { &self.decisions.assignments } else { &no_decisions };
        if objectives.iter().count() > 1 {
            return self.search_independent_javascript(source, observe);
        }
        let policy = self.javascript.as_ref().unwrap();
        let request = search_request(self.options, policy, objectives);
        let resolved_request = search_request_report(request);
        let mut search = self
            .compilation
            .search_javascript_observed(source, policy, request, observe)
            .map_err(|error| ServiceError::output("javascript", error))?;
        // The walk (architecture §9.6, plan M3.5, AM2): each requested
        // objective's list of moves within the level's counted budget, from
        // the level-0 artifact and, from the default level, from the
        // structural search's winner and the naming restarts too.
        let terminal = search
            .challenge_with_decisions(policy, request, decisions)
            .map(|report| serde_json::to_value(report).unwrap_or(Value::Null))
            .map_err(|error| ServiceError::output("javascript", error))?;
        let counters = search.counters();
        let report = search_report(
            resolved_request,
            counters,
            search.stopped().map(|error| format!("{error:?}")),
            terminal,
        );
        let selected = objectives
            .iter()
            .map(|codec| {
                search
                    .winner_qualification(codec)
                    .copied()
                    .map(|receipt| (codec, receipt))
                    .ok_or_else(|| ServiceError::new("handoff", "requested winner is missing"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut handoffs = Vec::new();
        for &(codec, receipt) in &selected {
            if handoffs
                .iter()
                .any(|other: &QualifiedArtifact| other.artifact() == receipt.artifact())
            {
                continue;
            }
            handoffs.push(
                search
                    .take_qualified_winner(codec)
                    .ok_or_else(|| ServiceError::new("handoff", "qualified winner is missing"))?,
            );
        }
        drop(search);
        let mut artifacts = Vec::new();
        let mut winners = [None; 3];
        let mut handoffs = handoffs.into_iter();
        while let Some(receipt) = handoffs.next() {
            for &(codec, selected) in &selected {
                if selected.artifact() == receipt.artifact() {
                    winners[codec_index(codec)] = Some(artifacts.len());
                }
            }
            match deliver_javascript(&mut self.compilation, receipt, self.decisions.is_writing()) {
                Ok(artifact) => artifacts.push(artifact),
                Err(error) => {
                    for pending in handoffs {
                        self.compilation
                            .discard_artifact(pending.artifact())
                            .map_err(|error| ServiceError::new("handoff cleanup", error))?;
                    }
                    return Err(error);
                }
            }
        }
        Ok(ServiceJavaScriptBatch {
            artifacts,
            winners,
            report,
        })
    }

    fn search_independent_javascript(
        &mut self,
        source: SemanticId,
        observe: impl FnMut(SearchObservation<'_>),
    ) -> Result<ServiceJavaScriptBatch, ServiceError> {
        let policies = self.independent_javascript.as_ref().unwrap();
        let no_decisions = [None, None, None];
        let decisions = if source == self.source { &self.decisions.assignments } else { &no_decisions };
        let requests: Vec<_> = policies
            .iter()
            .map(|policy| {
                (
                    policy,
                    search_request(
                        self.options,
                        policy,
                        Objectives::One(policy.objective().unwrap().codec),
                    ),
                )
            })
            .collect();
        let results = self
            .compilation
            .search_javascript_independent(source, &requests, decisions, observe)
            .map_err(|error| ServiceError::output("javascript", error))?;
        let mut report = json!({
            "independent": true,
            "order": self.objectives()?.iter().map(Objective::name).collect::<Vec<_>>(),
            "resource_schedule": "all baselines first; equal shares of remaining optional work; shared hard memory and deadline",
            "request": search_request_report(search_request(self.options, self.javascript.as_ref().unwrap(), self.objectives()?)),
            "terminal": {"objectives": []},
            "objectives": [],
            "stop": null,
        });
        let counters = [
            "proposals",
            "structures",
            "renders",
            "codec_probes",
            "proof_queries",
            "beam_evictions",
            "admitted_artifacts",
        ];
        for field in counters {
            report[field] = json!(0);
        }
        let mut stops = Vec::new();
        for (index, policy) in policies.iter().enumerate() {
            let result = results[codec_index(policy.objective().unwrap().codec)]
                .as_ref()
                .unwrap();
            let mut lane = search_report(
                search_request_report(requests[index].1),
                result.counters,
                result.stopped.clone(),
                serde_json::to_value(&result.terminal).unwrap(),
            );
            lane["policy"] = policies[index].receipt();
            lane["optional_work_allowance"] = json!(result.optional_work_allowance);
            lane["optional_work_used"] = json!(result.optional_work_used);
            for field in counters {
                report[field] =
                    json!(report[field].as_u64().unwrap() + lane[field].as_u64().unwrap());
            }
            if let Some(stop) = &result.stopped {
                stops.push(format!(
                    "{:?}: {stop}",
                    policies[index].objective().unwrap().codec
                ));
            }
            report["terminal"]["objectives"]
                .as_array_mut()
                .unwrap()
                .extend(
                    lane["terminal"]["objectives"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .cloned(),
                );
            report["objectives"].as_array_mut().unwrap().push(lane);
        }
        if !stops.is_empty() {
            report["stop"] = json!(stops.join("; "));
        }
        let mut artifacts = Vec::new();
        let mut winners = [None; 3];
        let mut results = results
            .into_iter()
            .enumerate()
            .filter_map(|(index, result)| result.map(|result| (index, result)));
        while let Some((index, result)) = results.next() {
            match deliver_javascript(&mut self.compilation, result.winner, self.decisions.is_writing()) {
                Ok(artifact) => {
                    winners[index] = Some(artifacts.len());
                    artifacts.push(artifact);
                }
                Err(error) => {
                    for (_, pending) in results {
                        self.compilation
                            .discard_artifact(pending.winner.artifact())
                            .map_err(|error| ServiceError::new("handoff cleanup", error))?;
                    }
                    return Err(error);
                }
            }
        }
        Ok(ServiceJavaScriptBatch {
            artifacts,
            winners,
            report,
        })
    }

    pub fn retain_native(
        &mut self,
        source: SemanticId,
    ) -> Result<QualifiedNativeArtifact, ServiceError> {
        let policy = self
            .native
            .as_ref()
            .ok_or_else(|| ServiceError::new("native", "not a native session"))?;
        let bindings = self.native_bindings.iter().map(|(cell, link_name)| NativeHostBinding {
            cell: *cell, link_name,
        }).collect::<Vec<_>>();
        let hosts = NativeHostBindings { callback_abi_version: NativeHostBindings::ABI_VERSION, bindings: &bindings };
        self.compilation
            .retain_native_c_and_hosts(
                source,
                policy,
                ArtifactRuntimeEvidence::default(),
                WorkDomain::Baseline,
                &hosts,
            )
            .map_err(|error| self.native_error(source, error))
    }

    /// Compile every configured output/objective through this session's one
    /// baseline-sealing transition. Call once, after any source edits.
    pub fn compile_targets(&mut self) -> Result<ServiceCompilation, ServiceError> {
        let started = self.started;
        let mut first_artifact_ns = None;
        let native = if self.native.is_some() {
            let phase = Instant::now();
            let artifact = self.retain_native(self.source)?;
            self.phases["native_ns"] = json!(nanos(phase));
            first_artifact_ns = Some(nanos(started));
            Some(artifact)
        } else {
            None
        };
        let mut output = ServiceCompilation {
            javascript: Vec::new(),
            winners: [None; 3],
            outputs: Vec::new(),
            native_c: None,
            native_header: None,
            report: Value::Null,
        };
        let mut search_report = Value::Null;
        if self.javascript.is_some() {
            let phase = Instant::now();
            if self.additional_outputs.is_empty() {
                let batch = self.search_javascript(self.source, |observation| {
                    if observation.baseline && first_artifact_ns.is_none() {
                        first_artifact_ns = Some(nanos(started));
                    }
                })?;
                (output.javascript, output.winners, search_report) = batch.into_parts();
            } else {
                search_report = self.compile_javascript_outputs(&mut output, |observation| {
                    if observation.baseline && first_artifact_ns.is_none() { first_artifact_ns = Some(nanos(started)); }
                })?;
            }
            self.phases["javascript_ns"] = json!(nanos(phase));
        }
        let native_cost = if let Some(artifact) = native {
            let (semantic, ownership_transfers, initialization_guards_removed) = self
                .compilation
                .with_qualified_native_artifact(&artifact, |view| {
                    (
                        semantic_report(Some(view.snapshot), Some(view.meaning), view.rewrites),
                        view.ownership_transfers,
                        view.initialization_guards_removed,
                    )
                })
                .map_err(|error| ServiceError::new("native metadata", error))?;
            let cost = json!({"c_bytes":artifact.c_bytes(), "header_bytes":artifact.header_bytes(),
                "scope":"C and host-header delivery, not linked executable bytes", "semantic":semantic, "ownership_transfers":ownership_transfers,
                "initialization_guards_removed":initialization_guards_removed});
            let (c, header) = self
                .compilation
                .take_qualified_native_artifact(artifact)
                .map_err(|error| ServiceError::new("native handoff", error))?;
            if !header.is_empty() { output.native_header = Some(header); }
            output.native_c = Some(c);
            Some(cost)
        } else {
            None
        };
        output.report = json!({});
        output.report["artifacts"] =
            json!(output.javascript.iter().map(|artifact| json!({
            "sha256":artifact.sha256, "raw":artifact.sizes.raw, "gzip9":artifact.sizes.gzip9,
            "brotli11":artifact.sizes.brotli11, "details":artifact.details,
        })).collect::<Vec<_>>());
        output.report["winners"] = json!(output.winners);
        output.report["outputs"] = json!(output.outputs);
        output.report["native_sha256"] =
            json!(output.native_c.as_ref().map(|text| digest(text.as_bytes())));
        output.report["native_header_sha256"] = json!(output.native_header.as_ref().map(|text| digest(text.as_bytes())));
        output.report["native_delivery"] = json!(native_cost);
        output.report["first_artifact_ns"] = json!(first_artifact_ns);
        output.report["search"] = search_report;
        self.decisions.finish(&mut output)?;
        Ok(output)
    }

    fn finish_backend(self) -> FinishedSourceSession {
        let Self {
            started,
            options,
            javascript,
            independent_javascript,
            additional_outputs,
            native,
            compilation,
            phases,
            frontend_work,
            source_identity,
            inputs,
            shape,
            source_buffer_bytes,
            native_sources,
            ..
        } = self;
        let before = ledger_report(compilation.ledger());
        let codec_cache = compilation.measurement_stats();
        let phase = Instant::now();
        let mut ledger = compilation.finish();
        native_sources.discard(&mut ledger);
        #[cfg(test)]
        record_retained("compilation", ledger.retained_bytes());
        let mut phases = phases;
        phases["finish_ns"] = json!(nanos(phase));
        let limits = javascript.as_ref().or(native.as_ref()).unwrap().resources();
        let mut report = json!({
            "schema":1, "source_sha256":source_identity,
            "request":{"target":format!("{:?}",options.target), "preserve_root_exports":options.preserve_root_exports,
                "logical_work":options.logical_work, "retained_bytes":options.retained_bytes,
                "effective_logical_work":options.logical_work.min(limits.logical_work.unwrap_or(u64::MAX)),
                "effective_retained_bytes":options.retained_bytes.min(limits.retained_bytes.unwrap_or(u64::MAX))},
            "inputs":inputs,"shape":shape,"javascript_policy":javascript.as_ref().map(ResolvedPolicy::receipt),
            "javascript_policies":independent_javascript.as_ref().map(|policies| policies.iter().map(ResolvedPolicy::receipt).collect::<Vec<_>>()),
            "native_policy":native.as_ref().map(ResolvedPolicy::receipt), "phases_ns":phases,"total_ns":nanos(started),
            "policy_diagnostics":{
                "javascript":javascript.as_ref().map(ResolvedPolicy::diagnostics),
                "native":native.as_ref().map(ResolvedPolicy::diagnostics)
            },
            "policy_resolution":{
                "javascript":javascript.as_ref().map(ResolvedPolicy::resolution),
                "javascript_objectives":independent_javascript.as_ref().map(|policies| policies.iter().map(ResolvedPolicy::resolution).collect::<Vec<_>>()),
                "native":native.as_ref().map(ResolvedPolicy::resolution)
            },
            "ledger_before_finish":before,
            "resources":{
                "baseline_work":ledger.work_used(WorkDomain::Baseline),"optional_work":ledger.work_used(WorkDomain::Optional),
                "codec_work":ledger.work_by_kind(WorkKind::Codec),"peak_retained_bytes":ledger.peak_retained_bytes(),
                "frontend_logical_work":frontend_work,
                "frontend_allocation_accounting":"partial",
                "source_buffer_accounting":if source_buffer_bytes.is_some() {"pre-admitted stable arena backing; insertion String overlap charged"} else {"caller-owned text; excluded"},
                "source_buffer_capacity":source_buffer_bytes,
                "frontend_phase_accounting":{
                    "discovery_parse_arena":if source_buffer_bytes.is_some() {"one pre-admitted arena and owned program list, reused for checking"} else {"not used"},
                    "main_parse_arena":if source_buffer_bytes.is_some() {"not repeated; discovery programs reused"} else {"pre-admitted arena backing"},
                    "lexer_tokens":"pre-admitted Vec capacities, including overlapping nested fragments",
                    "lexer_templates":"pre-admitted scanner frames, interpolation vectors/boxed copies and template tables; scanner byte work charged",
                    "lexer_work":"full input-byte tariff admitted before each lexical stream, including nested fragments; cooperative checks after Logos calls",
                    "source_identity":"inline opaque stamp and node count; included in typed owner capacities, no shared heap allocation",
                    "parser_lookahead_work":"each token probe, including EOF, in arrow and type/reference lookahead admitted before inspection",
                    "parser_non_arena":"diagnostics and comprehensive parser traversal/recursion work uninstrumented; individual Logos calls are not preemptible",
                    "checker":"pre-admitted fixed source-node tables, module facts/interfaces, initialization order and canonical declaration vectors; model stays live through conversion",
                    "checker_module_graph":"ownership/dependency/order validation work and shared iterative schedule work/storage admitted; exact dependency/import/export capacities",
                    "checker_analyzer":"scope/narrowing, type-parameter, return, constructor and generator context vector backing admitted; dropped and released per Analyzer without releasing shared declarations",
                    "checker_binary":"existing iterative expression visits and continuation probes charged; continuation vector backing/growth admitted and released after each expression, including nested overlap and ordinary failures",
                    "checker_narrowing":"leaf queries and iterative traversal probes charged; pending/answer vector backing and growth overlap admitted and released per query; nested maps/types remain uninstrumented",
                    "checker_narrowing_reuse":"binary continuations retain syntax-only guard inputs/projections; empty branches are skipped, retained guards re-resolve in the active scope; no result cache or source reassociation",
                    "checker_remaining":"nested type/signature/default payloads, hash/ordered maps, alias/local-resolution scratch, debug consistency scratch, diagnostics and comprehensive checker traversal/native stack work uninstrumented",
                    "conversion_construction":"pre-admitted graph and temporary storage; retained charges transfer into publication",
                    "semantic_verification":"pre-admitted work and owned scratch",
                    "module_metadata":"uninstrumented"
                },
                "source_discovery_work":"per-read byte/chunk and import traversal charges; admitted growth reuses the allocation owner's Render movement tariff",
                "work_kind_accounting":"Render includes shared buffer allocation/movement tariffs, not only target code generation",
                "deadline":"cooperative; checked between phases and within admitted frontend/backend work; individual uninstrumented operations are not preemptible",
                "scope":"admitted path source buffers, covered parser arenas, token/template storage, semantic construction/verification and target/codec storage; remaining frontend allocations listed by phase, diagnostic copies, caller configuration I/O, returned buffers and process RSS are separate"
            }
        });
        report["native_capabilities"] = json!(native.as_ref().map(|_|crate::native_capabilities::receipt()));
        report["additional_outputs"] = json!(additional_outputs.iter().map(|output| output.receipt()).collect::<Vec<_>>());
        report["resources"]["target_allocation_accounting"] = json!("partial");
        report["resources"]["target_allocation_scope"] = json!("complete retained target copies, journals, delivery plans, render/artifact buffers and covered analysis/normalization owners are admitted; remaining legacy rule scratch, some initial-formation metadata and external admission-parser allocations are not allocation-exact");
        report["codec_cache"] = json!(codec_cache);
        report["execution"] = json!({"jobs": javascript.as_ref().or(native.as_ref()).unwrap().execution().jobs,
            "scoring_batch": 4, "worker_codec_bytes": 8 * 1024 * 1024});
        FinishedSourceSession { report, ledger }
    }
}

fn run_client<'src, R>(
    mut session: CheckedSourceSession<'src>,
    client: impl FnOnce(&mut CheckedSourceSession<'src>) -> R,
) -> (std::thread::Result<R>, FinishedSourceSession) {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| client(&mut session)));
    (outcome, session.finish_backend())
}

fn finish_factory<R>(
    outcome: std::thread::Result<R>,
    mut finished: FinishedSourceSession,
    started: Instant,
    source_release_ns: Option<u64>,
) -> (R, FinishedSourceSession) {
    finished.report["phases_ns"]["source_release_ns"] = json!(source_release_ns);
    finished.report["total_ns"] = json!(nanos(started));
    finished.report["ledger_after_finish"] = ledger_report(&finished.ledger);
    finished.report["resources"]["retained_bytes_after_handoff"] =
        json!(finished.ledger.retained_bytes());
    debug_assert_eq!(finished.ledger.retained_bytes(), 0);
    #[cfg(test)]
    record_retained("finished", finished.ledger.retained_bytes());
    match outcome {
        Ok(value) => (value, finished),
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

#[cfg(test)]
thread_local! {
    static RELEASE_EVENTS: std::cell::RefCell<Option<Vec<(&'static str, Option<u64>)>>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn record_release(event: &'static str) {
    RELEASE_EVENTS.with(|events| {
        if let Some(events) = events.borrow_mut().as_mut() {
            events.push((event, None));
        }
    });
}

#[cfg(test)]
fn record_retained(event: &'static str, bytes: u64) {
    RELEASE_EVENTS.with(|events| {
        if let Some(events) = events.borrow_mut().as_mut() {
            events.push((event, Some(bytes)));
        }
    });
}

fn deliver_javascript(
    compilation: &mut Compilation<'_>,
    receipt: QualifiedArtifact,
    save_decision: bool,
) -> Result<ServiceJavaScript, ServiceError> {
    let delivered = (|| {
        let (sizes, several, details) = compilation.with_qualified_artifact(&receipt, |view, provenance| {
        let plan = provenance.naming();
        (view.sizes, view.layout.is_some(), json!({
            "decision":save_decision.then(|| SavedDecision::capture(view.implementation, &view.output, plan)).flatten(),
            "recipe_words":view.implementation.recipe_words(),
            "recipe_fingerprint":view.recipe_fingerprint,
            "semantic":semantic_report(view.implementation.snapshot_identity(),
                view.implementation.meaning_identity(), view.implementation.rewrites()),
            "execution":format!("{:?}",view.execution),
            "style":format!("{:?}",plan.style),
            "source_names":plan.source_names.iter().map(|id|id.index()).collect::<Vec<_>>(),
            "tactics":provenance.tactics().iter().map(|usage|json!({"tactic":usage.tactic,"risk":usage.risk})).collect::<Vec<_>>(),
            "output":{"dead_code_elimination":view.output.dead_code_elimination,
                "rules":view.output.rules,
                "target_compaction":view.output.target_compaction,"literals":format!("{:?}",view.output.literals),
                "families":format!("{:?}",view.output.families),"property_mangling":view.output.families.property_mangling,"self_named":plan.self_named,"read_order":plan.read_order,"local_read_order":plan.local_read_order,"compact_order":plan.compact_order,"alphabet":plan.alphabet.as_str(),
                "choices":view.output.choices.iter().map(|(key,alternative)| json!({"family":key.family,"site":key.site,"alternative":alternative.0})).collect::<Vec<_>>()},
            "sizes":[Some(view.sizes.raw),view.sizes.gzip9,view.sizes.brotli11],
            "delivery":view.layout.map(|layout| delivery_report(layout, view.files)),
            "policy_fingerprint":receipt.policy_fingerprint(),
        }))
    }).map_err(|error| ServiceError::new("handoff",error))?;
        let (javascript, files, layout) = if several {
            let delivered = compilation
                .take_qualified_files(receipt)
                .map_err(|error| ServiceError::new("handoff", error))?;
            (String::new(), delivered.files, Some(delivered.layout))
        } else {
            (
                compilation
                    .take_qualified_artifact(receipt)
                    .map_err(|error| ServiceError::new("handoff", error))?,
                Vec::new(),
                None,
            )
        };
        let sha256 = if files.is_empty() {
            digest(javascript.as_bytes())
        } else {
            let mut all = String::new();
            for file in &files {
                all.push_str(&file.name);
                all.push('\0');
                all.push_str(&digest(file.code.as_bytes()));
                all.push('\n');
            }
            digest(all.as_bytes())
        };
        Ok(ServiceJavaScript {
            sha256,
            javascript,
            files,
            layout,
            sizes,
            details,
        })
    })();
    if delivered.is_err() {
        compilation
            .discard_artifact(receipt.artifact())
            .map_err(|error| ServiceError::new("handoff cleanup", error))?;
    }
    delivered
}

/// The build report's `delivery` (design §10): files with their roles,
/// labels, links and measured sizes, and each entry's closure and rows.
fn delivery_report(
    layout: &crate::js::delivery::DeliveredLayout,
    files: &[crate::program::publication::ArtifactFile],
) -> Value {
    let name = |index: u32| {
        files
            .get(index as usize)
            .map_or("", |file| file.name.as_str())
    };
    let sizes = files.iter().map(|file| file.sizes()).collect::<Vec<_>>();
    let rows = |pick: fn(&Sizes) -> Option<usize>| -> Option<Vec<u64>> {
        let per_file = sizes.iter().map(pick).collect::<Option<Vec<_>>>()?;
        Some(layout.rows(&per_file))
    };
    // Every file once, per codec: the sum of rows counts a file once per
    // entry loading it, the total once (design §10).
    let total = |pick: fn(&Sizes) -> Option<usize>| -> Option<usize> {
        sizes
            .iter()
            .try_fold(0usize, |total, sizes| total.checked_add(pick(sizes)?))
    };
    json!({
        "mode": layout.mode.name(),
        "format": layout.format.name(),
        "setters": layout.setters,
        "total": [
            total(|sizes| Some(sizes.raw)),
            total(|sizes| sizes.gzip9),
            total(|sizes| sizes.brotli11),
        ],
        "files": layout.files.iter().enumerate().map(|(index, file)| json!({
            "file": name(index as u32),
            "role": file.role.name(),
            "label": layout.label_names(&file.label),
            "modules": file.modules,
            "anchored": file.anchored,
            "imports": file.imports.iter().map(|&target| name(target)).collect::<Vec<_>>(),
            "dynamic_imports": file.dynamic.iter().map(|&target| name(target)).collect::<Vec<_>>(),
            "sizes": [Some(sizes[index].raw), sizes[index].gzip9, sizes[index].brotli11],
        })).collect::<Vec<_>>(),
        "entries": layout.entries.iter().enumerate().map(|(index, entry)| json!({
            "name": entry.name,
            "file": name(entry.file),
            "dynamic": entry.dynamic,
            "closure": entry.closure.iter().map(|&file| name(file)).collect::<Vec<_>>(),
            "rows": [
                rows(|sizes| Some(sizes.raw)).map(|rows| rows[index]),
                rows(|sizes| sizes.gzip9).map(|rows| rows[index]),
                rows(|sizes| sizes.brotli11).map(|rows| rows[index]),
            ],
        })).collect::<Vec<_>>(),
    })
}

fn semantic_report(
    snapshot: Option<RevisionId>,
    meaning: Option<RevisionId>,
    rewrites: RewriteDescription<'_>,
) -> Value {
    json!({
        "snapshot_identity":snapshot.map(|identity| format!("{identity:?}")),
        "meaning_identity":meaning.map(|identity| format!("{identity:?}")),
        "identity_scope":"process-local diagnostics, not canonical recipe identity",
        "rewrite_order":"newest-first",
        "rewrites":rewrites.steps().map(|step| {
            let rule = match step.rule {
                crate::program::publication::RewriteRule::LiteralIntFold(fold) => json!({
                    "rule":"literal-int-binary", "version":fold.version,
                    "unit":fold.unit.index(), "operation":fold.operation.index(),
                    "operator":format!("{:?}",fold.operator),
                    "left_producer":fold.left_producer.index(), "left":fold.left,
                    "right_producer":fold.right_producer.index(), "right":fold.right,
                    "result":fold.result
                }),
                crate::program::publication::RewriteRule::DeadValueDrop(drop) => json!({
                    "rule":"dead-value-drop", "version":drop.version,
                    "unit":drop.unit.index(), "operation":drop.operation.index(),
                    "replacement":format!("{:?}",drop.replacement)
                }),
            };
            json!({
                "meaning_identity":format!("{:?}",step.meaning),
                "before_snapshot":format!("{:?}",step.before),
                "after_snapshot":format!("{:?}",step.after),
                "inherited":meaning != Some(step.meaning),
                "tactic":format!("{:?}",step.rule.tactic()),
                "fold":rule
            })
        }).collect::<Vec<_>>()
    })
}

fn ledger_report(ledger: &BudgetLedger) -> Value {
    json!({"baseline_work":ledger.work_used(WorkDomain::Baseline),"optional_work":ledger.work_used(WorkDomain::Optional),
        "retained_bytes":ledger.retained_bytes(),"peak_retained_bytes":ledger.peak_retained_bytes(),
        "analysis_work":ledger.work_by_kind(WorkKind::Analysis),"edit_work":ledger.work_by_kind(WorkKind::Edit),
        "render_work":ledger.work_by_kind(WorkKind::Render),"codec_work":ledger.work_by_kind(WorkKind::Codec)})
}

fn search_report(
    request: Value,
    counters: SearchCounters,
    stopped: Option<String>,
    terminal: Value,
) -> Value {
    json!({
        "request": request,
        "proposals": counters.proposals, "structures": counters.structures,
        "renders": counters.renders, "codec_probes": counters.codec_probes,
        "proof_queries": counters.proof_queries, "beam_evictions": counters.beam_evictions,
        "structural_opportunities": counters.family_outcomes(),
        "admitted_artifacts": counters.admitted_artifacts,
        "stop": stopped,
        "terminal": terminal,
    })
}

fn search_request_report(request: SearchRequest) -> Value {
    json!({
        "objectives":request.objectives.iter().map(|codec|format!("{codec:?}")).collect::<Vec<_>>(),
        "scalar":{"work":request.scalar.max_work,"scratch_bytes":request.scalar.scratch_bytes,"output_bytes":request.scalar.output_bytes},
        "helper":{"work":request.helper.max_work,"scratch_bytes":request.helper.scratch_bytes,"output_bytes":request.helper.output_bytes},
        "string":{"work":request.string.max_work,"scratch_bytes":request.string.scratch_bytes,"output_bytes":request.string.output_bytes},
        "local_facts":{"work":request.helper.local_facts.work_quota,"result_bytes":request.helper.local_facts.result_bytes},
        "facts_cache":{"entries":request.facts_cache.entries,"bytes":request.facts_cache.bytes,"result_bytes":request.facts_cache.result_bytes}
    })
}

/// A checked module graph and the program it elaborates to, as every build
/// sees them before search. Tools read them; nothing here searches or delivers.
pub struct CheckedProgram<'a, 'ast, 'src> {
    /// Discovery's modules by id: canonical paths, sources and edges.
    pub modules: &'a ModuleSet<&'src str>,
    /// Each module's original syntax, by module id.
    pub syntax: &'a [crate::ast::Program<'ast, 'src>],
    /// The module-graph checker's results.
    pub semantics: &'a CheckedModules<'ast, 'src>,
    /// The elaborated program. Operation spans are local to their unit's module.
    pub program: &'a crate::program::Program<'src>,
}

struct GraphConversion<'a> {
    source_contract: crate::config::LanguageConfig,
    hosts: &'a crate::config::HostConfig,
    defines: &'a crate::config::Defines,
    javascript: Option<&'a crate::compilation_contract::JavaScriptCompilationContract>,
    native: bool,
    library: bool,
    rules: Option<RuleRequest>,
    trap_index_reads: bool,
}
struct PreparedGraph<'src> {
    program: PreparedProgram<'src>,
    rules: crate::program::rules::RuleReceipt,
    native_bindings: Vec<(crate::program::CellId, String)>,
    conversion_ns: u64,
}
/// Capability and conversion ownership shared by cold compilation and retained
/// graph sessions. A warm editor must report the same language/target refusal.
fn prepare_checked_graph<'ast, 'src>(
    modules: &ModuleSet<&'src str>, syntax: &[crate::ast::Program<'ast, 'src>],
    semantics: &CheckedModules<'ast, 'src>, request: GraphConversion<'_>, budget: &mut AllocationBudget<'_>,
) -> Result<PreparedGraph<'src>, ServiceError> {
    for (module, input) in modules.modules.iter().enumerate() {
        crate::check::capabilities::language(semantics.view(module).expect("checked source"),
            Some(module), request.source_contract, budget)
            .map_err(|error| native_check_error(&input.path, input.source, error))?;
        if request.native {
            crate::check::capabilities::native(&syntax[module], semantics.view(module).unwrap(), Some(module),
                if request.library && semantics.roots().contains(&module) { &semantics.interfaces()[module].exports } else { &[] },
                request.hosts, budget).map_err(|error| native_check_error(&input.path, input.source, error))?;
        }
    }
    let native_bindings = if request.native { configured_native_bindings(semantics.symbols(), request.hosts)? } else { Vec::new() };
    budget.work(WorkKind::Analysis, modules.modules.iter().map(|m| m.source.len() as u64).sum())
        .map_err(|error| ServiceError::resources("frontend resources", error))?;
    let started = Instant::now();
    let (program, rules) = from_checked_modules_with_rules(syntax, semantics, request.rules, request.trap_index_reads,
        request.hosts, request.defines, request.javascript, budget).map_err(|error| match error.error {
        ConversionError::Unsupported(unsupported) => {
            let module = &modules.modules[error.module];
            ServiceError::module("conversion", ModuleError::new(&module.path, module.source,
                unsupported.span, format!("unsupported source: {}", unsupported.feature)))
        }
        ConversionError::Contract(violation) => {
            let module = &modules.modules[error.module];
            ServiceError::module("check", ModuleError::new(&module.path, module.source, violation.span, violation.message))
        }
        ConversionError::Resources(error) => ServiceError::resources("conversion resources", error),
    })?;
    Ok(PreparedGraph { program, rules, native_bindings, conversion_ns: nanos(started) })
}

/// The single-source frontend: parsing, checking and conversion under the
/// frontend's ledger. Syntax and checker storage are gone when it returns.
fn check_source_frontend<'src>(
    frontend: &mut Frontend<'src>,
    source: &'src str,
) -> Result<PreparedProgram<'src>, ServiceError> {
    let rules = frontend.rules();
    let source_contract = frontend.source_contract();
    let trap_index_reads = frontend.trap_index_reads();
    let javascript_contract = frontend.javascript.as_ref().and_then(|policy| policy.javascript_contract()).copied();
    let defines = frontend.javascript.as_ref().or(frontend.native.as_ref()).unwrap().defines().clone();
    let host_config = frontend.javascript.as_ref().or(frontend.native.as_ref()).unwrap().hosts().clone();
    let arena = AdmittedArena::new(&mut frontend.ledger, WorkDomain::Baseline);
    let phase = Instant::now();
    let syntax = arena.parse(source).map_err(|error| match error {
        AdmittedParseError::Syntax(error) => ServiceError::module(
            "parse",
            ModuleError::from_parse(Path::new("<source>"), source, error),
        ),
        AdmittedParseError::Resource(error) => ServiceError::resources("parse resources", error),
    })?;
    frontend.phases["parse_ns"] = json!(nanos(phase));
    arena
        .with_ledger(|ledger, domain| {
            ledger.charge(domain, WorkKind::Analysis, source.len() as u64)
        })
        .map_err(|error| ServiceError::resources("frontend resources", error.into()))?;
    let phase = Instant::now();
    let (program, release_started) = arena
        .with_ledger(|ledger, domain| {
            with_analyzed_source_with_contract(
                &syntax,
                source_contract,
                &mut AllocationBudget::new(Some((ledger, domain))),
                |semantics, budget| -> Result<_, ServiceError> {
                    crate::check::capabilities::language(semantics.view(),Some(0),source_contract,budget)
                        .map_err(|error|native_check_error("<source>",source,error))?;
                    if frontend.native.is_some() {
                        crate::check::capabilities::native(
                            &syntax,
                            semantics.view(),
                            Some(0),
                            if frontend.options.preserve_root_exports { semantics.exports() } else { &[] },
                            &host_config,
                            budget,
                        )
                        .map_err(|error| native_check_error("<source>", source, error))?;
                        frontend.native_bindings = configured_native_bindings(semantics.symbols(), &host_config)?;
                    }
                    frontend.phases["check_ns"] = json!(nanos(phase));
                    budget
                        .work(WorkKind::Analysis, source.len() as u64)
                        .map_err(|error| ServiceError::resources("frontend resources", error))?;
                    let phase = Instant::now();
                    let (program, rules) = from_checked_source_with_rules(
                        &syntax, semantics, rules, trap_index_reads, &host_config, &defines, javascript_contract.as_ref(), budget,
                    )
                    .map_err(|error| match error {
                        ConversionError::Unsupported(error) => ServiceError::module(
                            "conversion",
                            ModuleError::new(
                                "<source>",
                                source,
                                error.span,
                                format!("unsupported source: {}", error.feature),
                            ),
                        ),
                        ConversionError::Contract(violation) => ServiceError::module(
                            "check",
                            ModuleError::new("<source>", source, violation.span, violation.message),
                        ),
                        ConversionError::Resources(error) => {
                            ServiceError::resources("conversion resources", error)
                        }
                    })?;
                    frontend.phases["convert_ns"] = json!(nanos(phase));
                    frontend.phases["rules"] = rules.json();
                    Ok((program, Instant::now()))
                },
            )
        })
        .map_err(|error| match error {
            AdmittedCheckError::Semantic(error) => ServiceError::module(
                "check",
                ModuleError::new("<source>", source, error.span, error.message),
            ),
            AdmittedCheckError::Resources(error) => {
                ServiceError::resources("check resources", error)
            }
        })??;
    drop(syntax);
    drop(arena);
    if frontend.wants_source_maps() {
        frontend.source_maps = Some(match crate::source_maps::Sources::prepare(program.program(), [("source.lil", source)], &mut frontend.ledger) {
            Ok(sources) => sources,
            Err(error) => { program.discard(&mut frontend.ledger); return Err(ServiceError::resources("source-map inputs", error)); }
        });
    }
    frontend.phases["frontend_release_ns"] = json!(nanos(release_started));
    if frontend.native.is_some() {
        frontend.native_sources = match NativeSources::prepare(std::iter::once(source), &mut frontend.ledger) {
            Ok(sources) => sources,
            Err(error) => { program.discard(&mut frontend.ledger); return Err(error); }
        };
    }
    Ok(program)
}

/// Check source once, borrow the same compilation owner, and finalize it before
/// returning the client's value and receipt. Caller-owned source text is not freed.
pub fn with_checked_source<R>(
    source: &str,
    config: &ProjectConfig,
    options: ServiceOptions,
    client: impl for<'src> FnOnce(&mut CheckedSourceSession<'src>) -> R,
) -> Result<(R, FinishedSourceSession), ServiceError> {
    let mut frontend = Frontend::new(config, options)?;
    let program = check_source_frontend(&mut frontend, source)?;
    let started = frontend.started;
    let session = frontend.adopt(program, source_inputs(source))
        .map_err(|(error, _ledger)| error)?;
    let (outcome, finished) = run_client(session, client);
    Ok(finish_factory(outcome, finished, started, None))
}

/// The path frontend: discovery, parsing, checking and conversion under the
/// frontend's ledger. `inspect` sees the checked graph and its program while
/// both are alive. A build also takes the relative host modules its output
/// carries; a check does not deliver.
fn check_path_frontend<'src, T>(
    frontend: &mut Frontend<'src>,
    entries: &[EntrySource],
    root_source: Option<&str>,
    overlays: &[crate::module::SourceOverride<'_>],
    config: &ProjectConfig,
    sources: &'src StableSourceArena,
    build: bool,
    inspect: impl for<'a, 'ast> FnOnce(&CheckedProgram<'a, 'ast, 'src>) -> T,
) -> Result<(PreparedProgram<'src>, Value, T), ServiceError> {
    let rules = if build { frontend.rules() } else { None };
    let source_contract = frontend.source_contract();
    let trap_index_reads = frontend.trap_index_reads();
    let javascript_contract = frontend.javascript.as_ref().and_then(|policy| policy.javascript_contract()).copied();
    let defines = frontend.javascript.as_ref().or(frontend.native.as_ref()).unwrap().defines().clone();
    let host_config = frontend.javascript.as_ref().or(frontend.native.as_ref()).unwrap().hosts().clone();
    let arena = AdmittedArena::new(&mut frontend.ledger, WorkDomain::Baseline);
    let phase = Instant::now();
    let (modules, syntax) =
        crate::module::discover_parsed_modules_with_overrides(entries, root_source, overlays, config, sources, &arena).map_err(
            |error| match error {
                ModuleDiscoveryError::Module(error) => ServiceError::module("discovery", error),
                ModuleDiscoveryError::Resources(error) => {
                    ServiceError::resources("discovery resources", error)
                }
            },
        )?;
    frontend.source_buffer_bytes = Some(sources.allocated_bytes() as u64);
    frontend.phases["discovery_parse_ns"] = json!(nanos(phase));
    let bytes = modules
        .modules
        .iter()
        .try_fold(0u64, |total, module| {
            total.checked_add(module.source.len() as u64)
        })
        .ok_or_else(|| ServiceError::new("discovery", "source capacity"))?;
    arena
        .with_ledger(|ledger, domain| ledger.charge(domain, WorkKind::Analysis, bytes))
        .map_err(|error| ServiceError::resources("frontend resources", error.into()))?;
    // Relative host modules travel with the output (008-D3).
    let hosts = if build {
        host_requests(config, frontend.javascript.as_ref(), &modules)
    } else {
        None
    };
    if let Some((root_directory, requests, edition)) = hosts {
        match crate::host_modules::deliver(root_directory, &requests, edition) {
            Ok(delivery) => {
                arena
                    .with_ledger(|ledger, domain| {
                        ledger.charge(domain, WorkKind::Analysis, delivery.retained_bytes())
                    })
                    .map_err(|error| ServiceError::resources("frontend resources", error.into()))?;
                frontend.hosts = delivery;
            }
            Err(reason) if config.delivery.host_modules == crate::config::HostModules::Embed => {
                return Err(ServiceError::new("host modules", reason));
            }
            Err(reason) => frontend.phases["host_modules_external"] = json!(reason),
        }
    }
    let inputs = module_inputs(&modules, &frontend.hosts);
    arena
        .with_ledger(|ledger, domain| ledger.charge(domain, WorkKind::Analysis, bytes))
        .map_err(|error| ServiceError::resources("frontend resources", error.into()))?;
    let phase = Instant::now();
    let (program, inspected, release_started) = arena
        .with_ledger(|ledger, domain| {
            with_analyzed_modules_with_contract(
                &syntax,
                &modules,
                source_contract,
                &mut AllocationBudget::new(Some((ledger, domain))),
                |semantics, budget| -> Result<_, ServiceError> {
                    let prepared = prepare_checked_graph(&modules, &syntax, semantics, GraphConversion {
                        source_contract, hosts: &host_config, defines: &defines, javascript: javascript_contract.as_ref(),
                        native: frontend.native.is_some(), library: frontend.options.preserve_root_exports,
                        rules, trap_index_reads,
                    }, budget)?;
                    frontend.native_bindings = prepared.native_bindings;
                    frontend.phases["check_ns"] = json!(nanos(phase).saturating_sub(prepared.conversion_ns));
                    frontend.phases["convert_ns"] = json!(prepared.conversion_ns);
                    frontend.phases["rules"] = prepared.rules.json();
                    let program = prepared.program;
                    let inspected = inspect(&CheckedProgram {
                        modules: &modules,
                        syntax: &syntax,
                        semantics,
                        program: program.program(),
                    });
                    Ok((program, inspected, Instant::now()))
                },
            )
        })
        .map_err(|error| match error.error {
            AdmittedCheckError::Semantic(error_message) => {
                let module = &modules.modules[error.module];
                ServiceError::module(
                    "check",
                    ModuleError::new(
                        &module.path,
                        module.source,
                        error_message.span,
                        error_message.message,
                    ),
                )
            }
            AdmittedCheckError::Resources(error) => {
                ServiceError::resources("check resources", error)
            }
        })??;
    drop(syntax);
    drop(arena);
    if frontend.wants_source_maps() {
        let paths = modules.modules.iter().map(|module| module.path.clone()).collect::<Vec<_>>();
        let names = module_paths(&paths).into_iter().zip(&paths).map(|(name, path)| format!("{name}.{}", path.extension().and_then(|ext|ext.to_str()).unwrap_or("lil"))).collect::<Vec<_>>();
        frontend.source_maps = Some(match crate::source_maps::Sources::prepare(program.program(), names.iter().zip(&modules.modules).map(|(name, module)| (name.as_str(), module.source)), &mut frontend.ledger) {
            Ok(sources) => sources,
            Err(error) => { program.discard(&mut frontend.ledger); return Err(ServiceError::resources("source-map inputs", error)); }
        });
    }
    if frontend.native.is_some() {
        frontend.native_sources = match NativeSources::prepare(modules.modules.iter().map(|module| module.source), &mut frontend.ledger) {
            Ok(sources) => sources,
            Err(error) => { program.discard(&mut frontend.ledger); return Err(error); }
        };
    }
    drop(modules);
    frontend.phases["frontend_release_ns"] = json!(nanos(release_started));
    Ok((program, inputs, inspected))
}

/// The factory owns source storage and finalization. Source text cannot escape
/// through R; its buffers drop after the compilation and before return or unwind.
pub fn with_checked_path<R>(
    path: &Path,
    config: &ProjectConfig,
    options: ServiceOptions,
    client: impl for<'src> FnOnce(&mut CheckedSourceSession<'src>) -> R,
) -> Result<(R, FinishedSourceSession), ServiceError> {
    with_checked_entries(&[EntrySource::of(path)], config, options, client)
}

/// Several entries of one program (plan M3.3), in any order: they are
/// checked as one module graph whose roots are the entries in name order.
pub fn with_checked_entries<R>(
    entries: &[EntrySource],
    config: &ProjectConfig,
    options: ServiceOptions,
    client: impl for<'src> FnOnce(&mut CheckedSourceSession<'src>) -> R,
) -> Result<(R, FinishedSourceSession), ServiceError> {
    let entries = sorted_entries(entries)?;
    if entries.len() > 1 && options.target != ServiceTarget::JavaScript && !config.target.native.artifact.is_library() {
        return Err(ServiceError::new(
            "entries",
            "a native executable has one entry; select target.native.artifact=\"shared-library\" or \"object\" for a library with several entries",
        ));
    }
    // A script (`bare`, design §4) is one entry: several entries share state
    // only as modules.
    if entries.len() > 1 && !options.preserve_root_exports && !(options.native_request().is_some() && config.target.native.artifact.is_library()) {
        return Err(ServiceError::new(
            "entries",
            "a script build (`--target js`) has one entry; build several entries as modules (`--target js-module`)",
        ));
    }
    let mut frontend = Frontend::new(config, options)?;
    let sources = StableSourceArena::new(WorkDomain::Baseline);
    let prepared = check_path_frontend(
        &mut frontend,
        &entries,
        None,
        &[],
        config,
        &sources,
        true,
        |_| (),
    );
    let (program, inputs, ()) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            let mut ledger = frontend.finish_inputs();
            release_source_buffers(sources, &mut ledger);
            return Err(error);
        }
    };
    let started = frontend.started;
    let session = match frontend.adopt(program, inputs) {
        Ok(session) => session,
        Err((error, mut ledger)) => {
            release_source_buffers(sources, &mut ledger);
            return Err(error);
        }
    };
    let (outcome, mut finished) = run_client(session, client);
    let release_ns = release_source_buffers(sources, &mut finished.ledger);
    Ok(finish_factory(outcome, finished, started, Some(release_ns)))
}

/// Each module's path relative to the directory every module shares,
/// without extension, each segment spelled for a file name.
fn module_paths(paths: &[std::path::PathBuf]) -> Vec<String> {
    let directories = paths
        .iter()
        .filter(|path| crate::catalog::platform::source(path).is_none())
        .map(|path| path.parent().map(Path::to_path_buf).unwrap_or_default())
        .collect::<Vec<_>>();
    let mut common: Vec<std::ffi::OsString> = directories
        .first()
        .map(|first| first.iter().map(|part| part.to_os_string()).collect())
        .unwrap_or_default();
    for directory in &directories[directories.len().min(1)..] {
        let parts = directory.iter().collect::<Vec<_>>();
        let shared = common
            .iter()
            .zip(&parts)
            .take_while(|(left, right)| left.as_os_str() == **right)
            .count();
        common.truncate(shared);
    }
    let prefix = common.iter().collect::<std::path::PathBuf>();
    paths
        .iter()
        .map(|path| {
            if let Some(name) = crate::catalog::platform::delivery_name(path) {
                return name;
            }
            let relative = path
                .strip_prefix(&prefix)
                .unwrap_or(path)
                .with_extension("");
            relative
                .iter()
                .map(|segment| {
                    let mut escaped=String::new();
                    for byte in segment.to_string_lossy().as_bytes() {
                        if byte.is_ascii_alphanumeric() || matches!(*byte,b'-'|b'_'|b'.') {
                            escaped.push(*byte as char);
                        } else { use std::fmt::Write; write!(&mut escaped,"%{byte:02X}").unwrap(); }
                    }
                    escaped
                })
                .collect::<Vec<_>>()
                .join("/")
        })
        .collect()
}

/// Entries sorted by name, each name valid and unique (plan M3.3: bit `i`
/// of every label is the `i`-th name, so the TOML's table order never
/// matters).
fn sorted_entries(entries: &[EntrySource]) -> Result<Vec<EntrySource>, ServiceError> {
    if entries.is_empty() {
        return Err(ServiceError::new(
            "entries",
            "a build needs at least one entry",
        ));
    }
    let mut sorted = entries.to_vec();
    sorted.sort_by(|left, right| left.name.cmp(&right.name));
    for entry in &sorted {
        if !crate::config::valid_entry_name(&entry.name) {
            return Err(ServiceError::new(
                "entries",
                format!(
                    "entry name `{}` must be letters, digits, `_`, `.` or `-`",
                    entry.name
                ),
            ));
        }
    }
    if let Some(pair) = sorted.windows(2).find(|pair| pair[0].name == pair[1].name) {
        return Err(ServiceError::new(
            "entries",
            format!("two entries are named `{}`", pair[0].name),
        ));
    }
    Ok(sorted)
}

/// The relative host modules a JavaScript build carries: the root module's
/// directory, the foreign files the source imports, and the syntax target
/// they must fit. `None` when the output imports them instead.
fn host_requests<'m, S>(
    config: &ProjectConfig,
    javascript: Option<&ResolvedPolicy>,
    modules: &'m crate::module::ModuleSet<S>,
) -> Option<(
    &'m Path,
    Vec<std::path::PathBuf>,
    crate::js_syntax_target::EcmaScriptEdition,
)> {
    let javascript = javascript?;
    if config.delivery.host_modules == crate::config::HostModules::External {
        return None;
    }
    let root_directory = modules.modules[modules.root()]
        .path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let mut requests: Vec<std::path::PathBuf> = Vec::new();
    for module in &modules.modules {
        for dependency in &module.foreign_dependencies {
            if let Some(path) = &dependency.path {
                if !requests.contains(path) {
                    requests.push(path.clone());
                }
            }
        }
    }
    if requests.is_empty() {
        return None;
    }
    let edition = javascript
        .javascript_contract()
        .map(|contract| contract.ecmascript)
        .unwrap_or_default();
    Some((root_directory, requests, edition))
}

/// Every file a build of `path` reads, for an external build graph: the
/// source modules in discovery order, then the host modules the JavaScript
/// output carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInputs {
    /// The first entry's module.
    pub entry: std::path::PathBuf,
    /// Every entry's module, in entry name order.
    pub entries: Vec<(String, std::path::PathBuf)>,
    pub files: Vec<std::path::PathBuf>,
}

/// Discover what a build with these options reads, without checking or
/// compiling it: the same module discovery and host-module delivery as the
/// build.
pub fn build_inputs(
    entries: &[EntrySource],
    config: &ProjectConfig,
    options: ServiceOptions,
) -> Result<BuildInputs, ServiceError> {
    let entries = sorted_entries(entries)?;
    let mut frontend = Frontend::new(config, options)?;
    let sources = StableSourceArena::new(WorkDomain::Baseline);
    let inputs = (|| {
        let arena = AdmittedArena::new(&mut frontend.ledger, WorkDomain::Baseline);
        let (modules, syntax) = discover_parsed_modules_admitted(
            &entries, None, config, &sources, &arena,
        )
        .map_err(|error| match error {
            ModuleDiscoveryError::Module(error) => ServiceError::module("discovery", error),
            ModuleDiscoveryError::Resources(error) => {
                ServiceError::resources("discovery resources", error)
            }
        })?;
        let mut inputs = BuildInputs {
            entry: modules.modules[modules.root()].path.clone(),
            entries: modules
                .root_names
                .iter()
                .zip(&modules.roots)
                .map(|(name, &root)| (name.clone(), modules.modules[root].path.clone()))
                .collect(),
            files: modules
                .modules
                .iter()
                .map(|module| module.path.clone())
                .collect(),
        };
        if let Some((root_directory, requests, edition)) =
            host_requests(config, frontend.javascript.as_ref(), &modules)
        {
            match crate::host_modules::delivered_files(root_directory, &requests, edition) {
                Ok(files) => inputs.files.extend(files),
                Err(reason)
                    if config.delivery.host_modules == crate::config::HostModules::Embed =>
                {
                    return Err(ServiceError::new("host modules", reason));
                }
                // Not carried: the output imports them from their specifiers.
                Err(_) => {}
            }
        }
        drop(syntax);
        Ok(inputs)
    })();
    release_source_buffers(sources, &mut frontend.ledger);
    inputs
}

/// Checking runs no search and delivers nothing, so it takes no service
/// ceiling: only the policy's declared resources bound it.
fn check_options() -> ServiceOptions {
    ServiceOptions {
        logical_work: u64::MAX,
        retained_bytes: u64::MAX,
        ..ServiceOptions::default()
    }
}

/// Check a path's module graph exactly as a build does (discovery, parsing,
/// checking and conversion) without searching or delivering, and hand `client`
/// the checked graph and its program. `source` replaces the entry file's text,
/// as an editor's unsaved buffer does. Every refusal a build would report for
/// this source is returned here, with its module and span.
pub fn with_checked_program<R>(
    path: &Path,
    source: Option<&str>,
    config: &ProjectConfig,
    client: impl for<'a, 'ast, 'src> FnOnce(&CheckedProgram<'a, 'ast, 'src>) -> R,
) -> Result<R, ServiceError> {
    let overlays = source.map(|source| crate::module::SourceOverride { path, source });
    with_checked_graph(&configured_entries(path, config), overlays.as_slice(), config, client)
}

/// Entries used by editor/lint graph clients. Configured entries define the
/// public surfaces; opening a private dependency does not publish another root.
pub fn configured_entries(path: &Path, config: &ProjectConfig) -> Vec<EntrySource> {
    if config.delivery.entries.is_empty() { return vec![EntrySource::of(path)]; }
    let base = config.config_dir.as_deref().unwrap_or_else(|| Path::new("."));
    config.delivery.entries.iter().map(|(name, path)| EntrySource {
        name: name.clone(), path: base.join(path),
    }).collect()
}

/// Inspect one configured graph with simultaneous source overrides. This is
/// the same discovery, checking, capability and conversion path as compilation.
pub fn with_checked_graph<R>(
    entries: &[EntrySource],
    overlays: &[crate::module::SourceOverride<'_>],
    config: &ProjectConfig,
    client: impl for<'a, 'ast, 'src> FnOnce(&CheckedProgram<'a, 'ast, 'src>) -> R,
) -> Result<R, ServiceError> {
    let entries = sorted_entries(entries)?;
    let mut frontend = Frontend::new(config, check_options())?;
    let sources = StableSourceArena::new(WorkDomain::Baseline);
    let checked = check_path_frontend(
        &mut frontend,
        &entries,
        None,
        overlays,
        config,
        &sources,
        false,
        client,
    )
    .map(|(program, _inputs, inspected)| {
        program.discard(&mut frontend.ledger);
        inspected
    });
    let mut ledger = frontend.finish_inputs();
    release_source_buffers(sources, &mut ledger);
    checked
}

/// The diagnostics a build of this path would report, without the build.
pub fn check_path(
    path: &Path,
    source: Option<&str>,
    config: &ProjectConfig,
) -> Result<(), ServiceError> {
    with_checked_program(path, source, config, |_| ())
}

/// The diagnostics a build of this single source would report, without the build.
pub fn check_source(source: &str, config: &ProjectConfig) -> Result<(), ServiceError> {
    let mut frontend = Frontend::new(config, check_options())?;
    let program = check_source_frontend(&mut frontend, source)?;
    program.discard(&mut frontend.ledger);
    Ok(())
}

fn release_source_buffers(sources: StableSourceArena, ledger: &mut BudgetLedger) -> u64 {
    let started = Instant::now();
    #[cfg(test)]
    record_retained("source", ledger.retained_bytes());
    sources.discard(ledger).expect("owned source arena backing");
    #[cfg(test)]
    record_retained("source-charge", ledger.retained_bytes());
    nanos(started)
}

pub fn compile_source(
    source: &str,
    config: &ProjectConfig,
    options: ServiceOptions,
) -> Result<ServiceCompilation, ServiceError> {
    let mut cache = cache::Request::source(source, config, options)?;
    if let Some(output) = cache.as_mut().and_then(cache::Request::read) { return Ok(output); }
    let (output, finished) =
        with_checked_source(source, config, options, |session| session.compile_targets())?;
    let mut output = finish_output(output, finished)?;
    if let Some(cache) = &mut cache { cache.write(&mut output); }
    Ok(output)
}

pub fn compile_path(
    path: &Path,
    config: &ProjectConfig,
    options: ServiceOptions,
) -> Result<ServiceCompilation, ServiceError> {
    compile_entries(&[EntrySource::of(path)], config, options)
}

/// One program with several entries (plan M3.3): one compilation, one
/// delivery with a facade per entry.
pub fn compile_entries(
    entries: &[EntrySource],
    config: &ProjectConfig,
    options: ServiceOptions,
) -> Result<ServiceCompilation, ServiceError> {
    let mut cache = cache::Request::entries(entries, config, options)?;
    if let Some(output) = cache.as_mut().and_then(cache::Request::read) { return Ok(output); }
    let (output, finished) = with_checked_entries(entries, config, options, |session| {
        session.compile_targets()
    })?;
    let mut output = finish_output(output, finished)?;
    if let Some(cache) = &mut cache { cache.write(&mut output); }
    Ok(output)
}

/// The bytes and resolved graph actually checked. Embedded host identities
/// include code and linkage, not just the delivered body's byte count.
fn source_inputs(source: &str) -> Value {
    json!({"root": 0, "modules": [{"path": "<source>", "bytes": source.len(), "sha256": digest(source.as_bytes())}]})
}

fn module_inputs<S: AsRef<str>>(modules: &ModuleSet<S>, hosts: &crate::host_modules::HostDelivery) -> Value {
    json!({"root": modules.root(),
        "entries": modules.roots.iter().zip(&modules.root_names).map(|(module, name)| json!({"name": name, "module": module})).collect::<Vec<_>>(),
        "modules": modules.modules.iter().map(|module| json!({
            "path": module.path, "bytes": module.source.as_ref().len(), "sha256": digest(module.source.as_ref().as_bytes()),
            "dependencies": module.dependencies, "dynamic_dependencies": module.dynamic_dependencies,
        })).collect::<Vec<_>>(),
        "host_modules": hosts.modules.iter().map(|module| json!({
            "specifier": module.specifier, "stem": module.stem, "delivered_bytes": module.delivered_bytes(),
            "sha256": digest(module.body().as_bytes()), "exports": module.exports(), "imports": module.imports(),
        })).collect::<Vec<_>>(), "host_reserved": hosts.reserved,
    })
}

fn finish_output(
    output: Result<ServiceCompilation, ServiceError>,
    finished: FinishedSourceSession,
) -> Result<ServiceCompilation, ServiceError> {
    let mut output = output?;
    let Value::Object(report) = finished.report else {
        unreachable!("finished session report is an object");
    };
    output.report.as_object_mut().unwrap().extend(report);
    Ok(output)
}

fn search_request(
    options: ServiceOptions,
    policy: &ResolvedPolicy,
    objectives: Objectives,
) -> SearchRequest {
    let limits = policy.resources();
    let effective_work = options
        .logical_work
        .min(limits.logical_work.unwrap_or(options.logical_work));
    // A proof's maximum must leave room for the baseline, discovery and its
    // local-facts prerequisites in the same compilation allowance.
    let work = (effective_work / 8).min(2_000_000);
    let memory = options
        .retained_bytes
        .min(limits.retained_bytes.unwrap_or(options.retained_bytes));
    let scratch = (memory / 8).min(4_000_000);
    let output = (memory / 8).min(2_000_000);
    let facts_cache = CacheLimits::within_budget(128, (memory / 8).min(16_000_000), 512_000);
    let local_facts = LocalFactsRequest {
        work_quota: work.min(1_000_000),
        result_bytes: facts_cache.result_bytes,
    };
    SearchRequest {
        objectives,
        scalar: ScalarRequest {
            max_work: work,
            scratch_bytes: scratch,
            output_bytes: output,
        },
        helper: HelperRequest {
            max_work: work,
            scratch_bytes: scratch,
            output_bytes: output,
            local_facts,
        },
        string: StringRequest {
            max_work: work,
            scratch_bytes: scratch,
            output_bytes: output,
            local_facts,
        },
        facts_cache,
    }
}

fn codec_index(codec: Objective) -> usize {
    match codec {
        Objective::Raw => 0,
        Objective::Gzip => 1,
        Objective::Brotli => 2,
    }
}

fn nanos(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX)
}
fn digest(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}

#[cfg(test)]
#[path = "build_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "build_terminal_tests.rs"]
mod terminal_tests;

#[cfg(test)]
#[path = "build_budget_tests.rs"]
mod budget_tests;

#[cfg(test)]
#[path = "build_delivery_tests.rs"]
mod delivery_tests;
#[cfg(test)]
#[path = "build_consumer_tests.rs"]
mod consumer_tests;

#[cfg(test)]
#[path = "build_objective_tests.rs"]
mod objective_tests;

fn configured_native_bindings(
    symbols: &[crate::check::Symbol<'_>],
    hosts: &crate::config::HostConfig,
) -> Result<Vec<(crate::program::CellId, String)>, ServiceError> {
    for name in hosts.native.keys() {
        if !symbols.iter().any(|s| s.is_foreign() && s.name == name && s.ty.callable_signature().is_some()) {
            return Err(ServiceError::new("policy", format!("host.native.{name} does not name an extern function in this program")));
        }
    }
    symbols.iter().filter(|s| s.is_foreign()).filter_map(|symbol| {
        hosts.native.get(symbol.name).map(|link| {
            let cell = crate::program::CellId::from_index(symbol.id.0 as usize)
                .ok_or_else(|| ServiceError::new("check", "native provider cell capacity"))?;
            Ok((cell, link.clone()))
        })
    }).collect()
}

fn native_check_error(
    path: impl AsRef<Path>,
    source: &str,
    error: AdmittedCheckError,
) -> ServiceError {
    match error {
        AdmittedCheckError::Semantic(error) => ServiceError::module(
            "check",
            ModuleError::new(path.as_ref(), source, error.span, error.message),
        ),
        AdmittedCheckError::Resources(error) => ServiceError::resources("check resources", error),
    }
}
