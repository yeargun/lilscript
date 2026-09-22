//! Source-to-artifact service for the semantic compiler. Unsupported input is
//! diagnosed here; this route never invokes legacy lowering or text rewrites.
use std::fmt;
use std::path::Path;
use std::time::Instant;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::compilation_policy::{
    BaselineFirstPlan, BudgetLedger, CompilationRequest, ResolvedPolicy, WorkDomain, WorkKind,
};
use crate::config::ProjectConfig;
use crate::module::{
    ModuleDiscoveryError, ModuleError, StableSourceArena, discover_parsed_modules_admitted,
};
use crate::output_budget::AllocationBudget;
pub use crate::output_budget::AllocationError as ServiceResourceError;
use crate::parser::{AdmittedArena, AdmittedParseError};
use crate::semantic::{AdmittedSemanticError, with_analyzed_modules, with_analyzed_source};
use crate::semantic_program::facts::CacheLimits;
use crate::semantic_program::publication::*;
use crate::semantic_program::{
    ConversionError, RevisionId, from_checked_modules_admitted, from_checked_source_admitted,
};
use crate::structured_js::selection::{Objective, Objectives, Plan, Sizes};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceTarget {
    JavaScript,
    Native,
    All,
}

/// The extension delivered chunk files share with their entry file, as the
/// default route names them.
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
    /// None requests the independently resolved policy's codec.
    pub objectives: Option<Objectives>,
    /// Finite defaults, further restricted by policy.resources.
    pub logical_work: u64,
    pub retained_bytes: u64,
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

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "semantic compiler ({}): {}", self.phase, self.message)
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
#[derive(Debug)]
pub struct ServiceJavaScript {
    javascript: String,
    /// Chunk files the entry loads; empty for single-file delivery.
    chunks: Vec<crate::semantic_program::publication::DeliveredChunk>,
    /// What the entry imports, loads and preloads.
    entry_links: crate::semantic_program::publication::EntryLinks,
    sha256: String,
    sizes: Sizes,
    details: Value,
}

impl ServiceJavaScript {
    pub fn javascript(&self) -> &str {
        &self.javascript
    }
    pub fn chunks(&self) -> &[crate::semantic_program::publication::DeliveredChunk] {
        &self.chunks
    }
    pub fn entry_dependencies(&self) -> &[String] {
        &self.entry_links.dependencies
    }
    pub fn entry_links(&self) -> &crate::semantic_program::publication::EntryLinks {
        &self.entry_links
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

#[derive(Debug)]
pub struct ServiceCompilation {
    javascript: Vec<ServiceJavaScript>,
    winners: [Option<usize>; 3],
    native_c: Option<String>,
    report: Value,
}

impl ServiceCompilation {
    pub fn javascript(&self, codec: Objective) -> Option<&ServiceJavaScript> {
        self.winners[codec_index(codec)].map(|index| &self.javascript[index])
    }
    pub fn native_c(&self) -> Option<&str> {
        self.native_c.as_deref()
    }
    pub fn report(&self) -> &Value {
        &self.report
    }
}

struct Frontend {
    started: Instant,
    options: ServiceOptions,
    javascript: Option<ResolvedPolicy>,
    native: Option<ResolvedPolicy>,
    ledger: BudgetLedger,
    phases: Value,
    source_buffer_bytes: Option<u64>,
    /// Relative host modules every output carries.
    hosts: crate::host_modules::HostDelivery,
}

impl Frontend {
    fn new(config: &ProjectConfig, options: ServiceOptions) -> Result<Self, ServiceError> {
        let started = Instant::now();
        if config.optimization.for_of_specialize_family() != 0 {
            return Err(ServiceError::new(
                "configuration",
                "semantic compilation does not support optimization.for_of_specialize_family; set it to 0",
            ));
        }
        let javascript = if options.target != ServiceTarget::Native {
            Some(
                config
                    .resolve_policy(CompilationRequest::JavaScript {
                        preserve_root_exports: options.preserve_root_exports,
                    })
                    .map_err(|error| ServiceError::new("policy", error))?,
            )
        } else {
            None
        };
        let native = if options.target != ServiceTarget::JavaScript {
            Some(
                config
                    .resolve_policy(CompilationRequest::Native)
                    .map_err(|error| ServiceError::new("policy", error))?,
            )
        } else {
            None
        };
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
            native,
            ledger,
            phases: json!({"policy_ns": nanos(started)}),
            source_buffer_bytes: None,
            hosts: Default::default(),
        };
        frontend.checkpoint(1)?;
        Ok(frontend)
    }

    fn checkpoint(&mut self, work: u64) -> Result<(), ServiceError> {
        self.ledger
            .charge(WorkDomain::Baseline, WorkKind::Analysis, work)
            .map_err(|error| ServiceError::resources("frontend resources", error.into()))
    }

    fn adopt<'src>(
        mut self,
        prepared: PreparedProgram<'src>,
        inputs: Value,
    ) -> Result<CheckedSourceSession<'src>, (ServiceError, BudgetLedger)> {
        if let Err(error) = self.checkpoint(0) {
            prepared.discard(&mut self.ledger);
            #[cfg(test)]
            record_retained("prepared-discard", self.ledger.retained_bytes());
            return Err((error, self.ledger));
        }
        let Self {
            started,
            options,
            javascript,
            native,
            ledger,
            mut phases,
            source_buffer_bytes,
            hosts,
        } = self;
        let frontend_work = ledger.work_used(WorkDomain::Baseline);
        let source_identity = digest(serde_json::to_vec(&inputs).unwrap());
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
                    prepared.discard(&mut ledger);
                    #[cfg(test)]
                    record_retained("prepared-discard", ledger.retained_bytes());
                    return Err((ServiceError::new("adoption", error), ledger));
                }
            };
        let source = match compilation.adopt_prepared(prepared) {
            Ok(source) => source,
            Err(error) => {
                return Err((ServiceError::new("adoption", error), compilation.finish()));
            }
        };
        // File stems name delivered chunks; they never affect a program.
        let names = inputs["modules"]
            .as_array()
            .map(|modules| {
                modules
                    .iter()
                    .map(|module| {
                        let stem = module["path"]
                            .as_str()
                            .and_then(|path| std::path::Path::new(path).file_stem())
                            .and_then(|stem| stem.to_str())
                            .unwrap_or("module");
                        stem.chars()
                            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
                            .collect::<String>()
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if let Err(error) = compilation.set_module_names(names) {
            return Err((ServiceError::new("adoption", error), compilation.finish()));
        }
        compilation.set_chunk_extension(options.chunk_extension.as_str());
        if !hosts.is_empty() {
            if let Err(error) = compilation.set_host_modules(hosts) {
                return Err((ServiceError::new("adoption", error), compilation.finish()));
            }
        }
        phases["adopt_ns"] = json!(nanos(phase));
        Ok(CheckedSourceSession {
            started,
            options,
            javascript,
            native,
            compilation,
            source,
            phases,
            frontend_work,
            source_identity,
            inputs,
            shape,
            source_buffer_bytes,
        })
    }
}

/// One checked source and one compilation owner. Path clients borrow source text
/// inside the factory's scope; parser/checker storage is gone before this exists.
pub struct CheckedSourceSession<'src> {
    started: Instant,
    options: ServiceOptions,
    javascript: Option<ResolvedPolicy>,
    native: Option<ResolvedPolicy>,
    compilation: Compilation<'src>,
    source: SemanticId,
    phases: Value,
    frontend_work: u64,
    source_identity: String,
    inputs: Value,
    shape: Value,
    source_buffer_bytes: Option<u64>,
}

/// Delivered winners share one owned buffer whenever their artifact is shared.
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
            .map_err(|error| ServiceError::new("javascript", error))?
            .map_err(|error| ServiceError::new("javascript", error))?;
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
        let mut delivered = deliver_javascript(&mut self.compilation, qualified.unwrap())?;
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
        let policy = self.javascript.as_ref().unwrap();
        let request = search_request(self.options, policy, objectives);
        let resolved_request = search_request_report(request);
        let mut search = self
            .compilation
            .search_javascript_observed(source, policy, request, observe)
            .map_err(|error| ServiceError::new("javascript", error))?;
        let counters = search.counters();
        let report = json!({
            "request": resolved_request,
            "proposals": counters.proposals, "structures": counters.structures,
            "renders": counters.renders, "codec_probes": counters.codec_probes,
            "proof_queries": counters.proof_queries, "beam_evictions": counters.beam_evictions,
            "interaction_attempts": counters.interaction_attempts,
            "admitted_artifacts": counters.admitted_artifacts,
            "stop": search.stopped().map(|error| format!("{error:?}")),
        });
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
            match deliver_javascript(&mut self.compilation, receipt) {
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

    pub fn retain_native(
        &mut self,
        source: SemanticId,
    ) -> Result<QualifiedNativeArtifact, ServiceError> {
        let policy = self
            .native
            .as_ref()
            .ok_or_else(|| ServiceError::new("native", "not a native session"))?;
        self.compilation
            .retain_native_c(
                source,
                policy,
                ArtifactRuntimeEvidence::default(),
                WorkDomain::Baseline,
            )
            .map_err(|error| ServiceError::new("native", error))
    }

    fn compile_targets(&mut self) -> Result<ServiceCompilation, ServiceError> {
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
            native_c: None,
            report: Value::Null,
        };
        let mut search_report = Value::Null;
        if self.javascript.is_some() {
            let phase = Instant::now();
            let batch = self.search_javascript(self.source, |observation| {
                if observation.baseline && first_artifact_ns.is_none() {
                    first_artifact_ns = Some(nanos(started));
                }
            })?;
            (output.javascript, output.winners, search_report) = batch.into_parts();
            self.phases["javascript_ns"] = json!(nanos(phase));
        }
        let native_cost = if let Some(artifact) = native {
            let semantic = self
                .compilation
                .with_qualified_native_artifact(&artifact, |view| {
                    semantic_report(Some(view.snapshot), Some(view.meaning), view.rewrites)
                })
                .map_err(|error| ServiceError::new("native metadata", error))?;
            let cost = json!({"c_bytes":artifact.c_bytes(), "header_bytes":artifact.header_bytes(),
                "scope":"C and host-header delivery, not linked executable bytes", "semantic":semantic});
            let (c, header) = self
                .compilation
                .take_qualified_native_artifact(artifact)
                .map_err(|error| ServiceError::new("native handoff", error))?;
            debug_assert!(header.is_empty());
            output.native_c = Some(c);
            Some(cost)
        } else {
            None
        };
        output.report = json!({});
        output.report["artifacts"] = json!(output.javascript.iter().map(|artifact| json!({
            "sha256":artifact.sha256, "raw":artifact.sizes.raw, "gzip9":artifact.sizes.gzip9,
            "brotli11":artifact.sizes.brotli11, "details":artifact.details,
        })).collect::<Vec<_>>());
        output.report["winners"] = json!(output.winners);
        output.report["native_sha256"] =
            json!(output.native_c.as_ref().map(|text| digest(text.as_bytes())));
        output.report["native_delivery"] = json!(native_cost);
        output.report["first_artifact_ns"] = json!(first_artifact_ns);
        output.report["search"] = search_report;
        Ok(output)
    }

    fn finish_backend(self) -> FinishedSourceSession {
        let Self {
            started,
            options,
            javascript,
            native,
            compilation,
            phases,
            frontend_work,
            source_identity,
            inputs,
            shape,
            source_buffer_bytes,
            ..
        } = self;
        let before = ledger_report(compilation.ledger());
        let phase = Instant::now();
        let ledger = compilation.finish();
        #[cfg(test)]
        record_retained("compilation", ledger.retained_bytes());
        let mut phases = phases;
        phases["finish_ns"] = json!(nanos(phase));
        let limits = javascript.as_ref().or(native.as_ref()).unwrap().resources();
        let report = json!({
            "schema":1, "backend":"semantic", "source_sha256":source_identity,
            "request":{"target":format!("{:?}",options.target), "preserve_root_exports":options.preserve_root_exports,
                "logical_work":options.logical_work, "retained_bytes":options.retained_bytes,
                "effective_logical_work":options.logical_work.min(limits.logical_work.unwrap_or(u64::MAX)),
                "effective_retained_bytes":options.retained_bytes.min(limits.retained_bytes.unwrap_or(u64::MAX))},
            "inputs":inputs,"shape":shape,"javascript_policy":javascript.as_ref().map(ResolvedPolicy::receipt),
            "native_policy":native.as_ref().map(ResolvedPolicy::receipt), "phases_ns":phases,"total_ns":nanos(started),
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
                    "legacy_ast_specialization":"nonzero for_of_specialize_family unsupported",
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
) -> Result<ServiceJavaScript, ServiceError> {
    let delivered = (|| {
        let (sizes, bundle, details) = compilation.with_qualified_artifact(&receipt, |view, provenance| {
        if !matches!(view.implementation.resource(), ResourceDescription::Whole) {
            return Err(ServiceError::new("handoff", "this service delivers Whole artifacts, not resource packages"));
        }
        let plan = provenance.naming();
        Ok((view.sizes, !view.chunks.is_empty(), json!({
            "recipe_words":view.implementation.recipe_words(),
            "recipe_fingerprint":view.recipe_fingerprint,
            "semantic":semantic_report(view.implementation.snapshot_identity(),
                view.implementation.meaning_identity(), view.implementation.rewrites()),
            "resource":"whole",
            "execution":format!("{:?}",view.execution),
            "style":format!("{:?}",plan.style),
            "source_names":plan.source_names.iter().map(|id|id.index()).collect::<Vec<_>>(),
            "output":{"dead_code_elimination":view.output.dead_code_elimination,
                "target_compaction":view.output.target_compaction,"literals":format!("{:?}",view.output.literals)},
            "sizes":[Some(view.sizes.raw),view.sizes.gzip9,view.sizes.brotli11],
            "chunks":view.chunks.iter().map(|chunk| json!({"file":chunk.name,"modules":chunk.modules,"bytes":chunk.code.len(),"lazy":chunk.lazy})).collect::<Vec<_>>(),
            "policy_fingerprint":receipt.policy_fingerprint(),
        })))
    }).map_err(|error| ServiceError::new("handoff",error))??;
        let (javascript, entry_links, chunks) = if bundle {
            let delivered = compilation
                .take_qualified_bundle(receipt)
                .map_err(|error| ServiceError::new("handoff", error))?;
            (delivered.entry, delivered.entry_links, delivered.chunks)
        } else {
            (
                compilation
                    .take_qualified_artifact(receipt)
                    .map_err(|error| ServiceError::new("handoff", error))?,
                Default::default(),
                Vec::new(),
            )
        };
        Ok(ServiceJavaScript {
            sha256: digest(javascript.as_bytes()),
            javascript,
            chunks,
            entry_links,
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
                crate::semantic_program::publication::RewriteRule::LiteralIntFold(fold) => json!({
                    "rule":"literal-int-binary", "version":fold.version,
                    "unit":fold.unit.index(), "operation":fold.operation.index(),
                    "operator":format!("{:?}",fold.operator),
                    "left_producer":fold.left_producer.index(), "left":fold.left,
                    "right_producer":fold.right_producer.index(), "right":fold.right,
                    "result":fold.result
                }),
                crate::semantic_program::publication::RewriteRule::DeadValueDrop(drop) => json!({
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

/// Check source once, borrow the same compilation owner, and finalize it before
/// returning the client's value and receipt. Caller-owned source text is not freed.
pub fn with_checked_source<R>(
    source: &str,
    config: &ProjectConfig,
    options: ServiceOptions,
    client: impl for<'src> FnOnce(&mut CheckedSourceSession<'src>) -> R,
) -> Result<(R, FinishedSourceSession), ServiceError> {
    let mut frontend = Frontend::new(config, options)?;
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
            with_analyzed_source(
                &syntax,
                &mut AllocationBudget::new(Some((ledger, domain))),
                |semantics, budget| -> Result<_, ServiceError> {
                    frontend.phases["check_ns"] = json!(nanos(phase));
                    budget
                        .work(WorkKind::Analysis, source.len() as u64)
                        .map_err(|error| ServiceError::resources("frontend resources", error))?;
                    let phase = Instant::now();
                    let program = from_checked_source_admitted(&syntax, semantics, budget)
                        .map_err(|error| match error {
                            ConversionError::Unsupported(error) => ServiceError::module(
                                "conversion",
                                ModuleError::new(
                                    "<source>",
                                    source,
                                    error.span,
                                    format!("unsupported semantic source: {}", error.feature),
                                ),
                            ),
                            ConversionError::Resources(error) => {
                                ServiceError::resources("conversion resources", error)
                            }
                        })?;
                    frontend.phases["convert_ns"] = json!(nanos(phase));
                    Ok((program, Instant::now()))
                },
            )
        })
        .map_err(|error| match error {
            AdmittedSemanticError::Semantic(error) => ServiceError::module(
                "check",
                ModuleError::new("<source>", source, error.span, error.message),
            ),
            AdmittedSemanticError::Resources(error) => {
                ServiceError::resources("check resources", error)
            }
        })??;
    drop(syntax);
    drop(arena);
    frontend.phases["frontend_release_ns"] = json!(nanos(release_started));
    let started = frontend.started;
    let session = frontend.adopt(program, json!({"root": 0, "modules": [{"path": "<source>", "bytes": source.len(), "sha256": digest(source.as_bytes())}]}))
        .map_err(|(error, _ledger)| error)?;
    let (outcome, finished) = run_client(session, client);
    Ok(finish_factory(outcome, finished, started, None))
}

/// The factory owns source storage and finalization. Source text cannot escape
/// through R; its buffers drop after the compilation and before return or unwind.
pub fn with_checked_path<R>(
    path: &Path,
    config: &ProjectConfig,
    options: ServiceOptions,
    client: impl for<'src> FnOnce(&mut CheckedSourceSession<'src>) -> R,
) -> Result<(R, FinishedSourceSession), ServiceError> {
    let mut frontend = Frontend::new(config, options)?;
    let sources = StableSourceArena::new(WorkDomain::Baseline);
    let prepared = (|| {
        let arena = AdmittedArena::new(&mut frontend.ledger, WorkDomain::Baseline);
        let phase = Instant::now();
        let (modules, syntax) = discover_parsed_modules_admitted(path, config, &sources, &arena)
            .map_err(|error| match error {
                ModuleDiscoveryError::Module(error) => ServiceError::module("discovery", error),
                ModuleDiscoveryError::Resources(error) => {
                    ServiceError::resources("discovery resources", error)
                }
            })?;
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
        // Refusal spans are module-local; name each module index once.
        if std::env::var_os("LILSCRIPT_DEBUG_VERIFY").is_some() {
            for (index, module) in modules.modules.iter().enumerate() {
                eprintln!("module {index}: {}", module.path.display());
            }
        }
        // Relative host modules travel with the output (008-D3).
        if config.bundle.host_modules != crate::config::HostModules::External
            && frontend.javascript.is_some()
        {
            let root_directory = modules.modules[modules.root]
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
            if !requests.is_empty() {
                let edition = frontend
                    .javascript
                    .as_ref()
                    .and_then(ResolvedPolicy::javascript_contract)
                    .map(|contract| contract.ecmascript)
                    .unwrap_or_default();
                match crate::host_modules::deliver(root_directory, &requests, edition) {
                    Ok(delivery) => {
                        arena
                            .with_ledger(|ledger, domain| {
                                ledger.charge(domain, WorkKind::Analysis, delivery.retained_bytes())
                            })
                            .map_err(|error| {
                                ServiceError::resources("frontend resources", error.into())
                            })?;
                        frontend.hosts = delivery;
                    }
                    Err(reason)
                        if config.bundle.host_modules == crate::config::HostModules::Embed =>
                    {
                        return Err(ServiceError::new("host modules", reason));
                    }
                    Err(reason) => frontend.phases["host_modules_external"] = json!(reason),
                }
            }
        }
        let inputs = json!({"root": modules.root, "modules": modules.modules.iter().map(|module| json!({
            "path": module.path, "bytes": module.source.len(), "sha256": digest(module.source.as_bytes()),
            "dependencies": module.dependencies, "dynamic_dependencies": module.dynamic_dependencies,
        })).collect::<Vec<_>>(), "host_modules": frontend.hosts.modules.iter().map(|module| json!({
            "specifier": module.specifier, "delivered_bytes": module.delivered_bytes(),
        })).collect::<Vec<_>>()});
        arena
            .with_ledger(|ledger, domain| ledger.charge(domain, WorkKind::Analysis, bytes))
            .map_err(|error| ServiceError::resources("frontend resources", error.into()))?;
        let phase = Instant::now();
        let (program, release_started) = arena
            .with_ledger(|ledger, domain| {
                with_analyzed_modules(
                    &syntax,
                    &modules,
                    &mut AllocationBudget::new(Some((ledger, domain))),
                    |semantics, budget| -> Result<_, ServiceError> {
                        frontend.phases["check_ns"] = json!(nanos(phase));
                        budget.work(WorkKind::Analysis, bytes).map_err(|error| {
                            ServiceError::resources("frontend resources", error)
                        })?;
                        let phase = Instant::now();
                        let program = from_checked_modules_admitted(&syntax, semantics, budget)
                            .map_err(|error| match error.error {
                                ConversionError::Unsupported(unsupported) => {
                                    let module = &modules.modules[error.module];
                                    ServiceError::module(
                                        "conversion",
                                        ModuleError::new(
                                            &module.path,
                                            module.source,
                                            unsupported.span,
                                            format!(
                                                "unsupported semantic source: {}",
                                                unsupported.feature
                                            ),
                                        ),
                                    )
                                }
                                ConversionError::Resources(error) => {
                                    ServiceError::resources("conversion resources", error)
                                }
                            })?;
                        frontend.phases["convert_ns"] = json!(nanos(phase));
                        Ok((program, Instant::now()))
                    },
                )
            })
            .map_err(|error| match error.error {
                AdmittedSemanticError::Semantic(error_message) => {
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
                AdmittedSemanticError::Resources(error) => {
                    ServiceError::resources("check resources", error)
                }
            })??;
        drop(syntax);
        drop(arena);
        drop(modules);
        frontend.phases["frontend_release_ns"] = json!(nanos(release_started));
        Ok((program, inputs))
    })();
    let (program, inputs) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            release_source_buffers(sources, &mut frontend.ledger);
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

fn release_source_buffers(sources: StableSourceArena, ledger: &mut BudgetLedger) -> u64 {
    let started = Instant::now();
    #[cfg(test)]
    record_retained("source", ledger.retained_bytes());
    sources.discard(ledger).expect("owned source arena backing");
    #[cfg(test)]
    record_retained("source-charge", ledger.retained_bytes());
    nanos(started)
}

pub fn compile_source_semantic(
    source: &str,
    config: &ProjectConfig,
    options: ServiceOptions,
) -> Result<ServiceCompilation, ServiceError> {
    let (output, finished) =
        with_checked_source(source, config, options, |session| session.compile_targets())?;
    finish_output(output, finished)
}

pub fn compile_path_semantic(
    path: &Path,
    config: &ProjectConfig,
    options: ServiceOptions,
) -> Result<ServiceCompilation, ServiceError> {
    let (output, finished) =
        with_checked_path(path, config, options, |session| session.compile_targets())?;
    finish_output(output, finished)
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
#[path = "compiler_service_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "compiler_service_budget_tests.rs"]
mod budget_tests;

#[cfg(test)]
#[path = "compiler_service_delivery_tests.rs"]
mod delivery_tests;
