//! The public optimization-family registry. Configuration, admission and the
//! generated reference consume these definitions; producer-local proofs still
//! establish legality at each site.
use super::{ResolvedPolicy, RuntimeRisk};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AnalysisRequirement {
    UsesAndEffects,
    Values,
    CallsAndCaptures,
    OwnershipAndObservations,
    TargetSchedule,
    NamesAndBoundary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TacticDefault {
    On,
    Off,
    Preset,
}

impl TacticDefault {
    pub const fn enabled(self, maximum_preset: bool) -> bool {
        match self {
            Self::On => true,
            Self::Off => false,
            Self::Preset => maximum_preset,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TacticProducer {
    SharedRules,
    JavaScriptFormation,
    StructuralSearch,
    OutputFamilies,
    Naming,
}

/// Owners invalidated by a changed representation, not a second scheduler.
/// Program revisions own shared facts; formed heads own target-tree analyses;
/// naming plans and rendered files have separate identity-qualified caches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TacticInvalidation {
    ProgramFacts,
    TargetHead,
    TargetTail,
    Names,
    RenderedFiles,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TacticSpec {
    pub id: TacticId,
    pub name: &'static str,
    pub javascript_only: bool,
    /// Automatic effort gate under the legacy policy. Version 3 requires
    /// explicit permission for families whose implementations add runtime work.
    pub minimum_effort: u8,
    pub startup_at_level_16: bool,
    pub analysis: AnalysisRequirement,
    pub default: TacticDefault,
    pub producers: &'static [TacticProducer],
    pub prerequisites: &'static [TacticId],
    /// Supported provenance classes. Neutral includes permission markers on
    /// unchanged assignments; it is never measured performance evidence.
    pub risks: &'static [RuntimeRisk],
    pub invalidates: &'static [TacticInvalidation],
    pub purpose: &'static str,
    pub tradeoffs: &'static str,
}

impl TacticSpec {
    /// Stage-specific dependencies must not disable other implementations of
    /// the same family. Shared removal-only inlining needs dead-code removal;
    /// optional target-tree work requires target compaction.
    pub fn producer_prerequisites(self, producer: TacticProducer) -> &'static [TacticId] {
        use TacticId as T;
        use TacticProducer as P;
        match (self.id, producer) {
            (T::Inlining, P::SharedRules) => &[T::DeadCodeElimination],
            (T::StringPooling, P::StructuralSearch) => &[T::ConstantFolding],
            (T::TargetCompaction | T::DeadCodeElimination, _) => &[],
            (_, P::JavaScriptFormation | P::OutputFamilies) => &[T::TargetCompaction],
            _ => &[],
        }
    }

    pub fn producer_enabled(self, producer: TacticProducer, policy: &ResolvedPolicy) -> bool {
        policy.tactic(self.id).enabled
            && self.producers.contains(&producer)
            && (!self.javascript_only || policy.javascript_contract().is_some())
            && (policy.javascript_contract().is_some() || producer == TacticProducer::SharedRules)
            && self
                .producer_prerequisites(producer)
                .iter()
                .all(|id| policy.tactic(*id).enabled)
    }
}

macro_rules! declare_tactics {
    ($($id:ident { $($field:ident: $value:expr),+ $(,)? }),+ $(,)?) => {
        /// A permission to compete, never a forced representation.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
        #[serde(rename_all = "kebab-case")]
        #[repr(usize)]
        pub enum TacticId { $($id),+ }

        impl TacticId {
            pub const ALL: [Self; [$(stringify!($id)),+].len()] = [$(Self::$id),+];
            pub const fn spec(self) -> TacticSpec {
                match self { $(Self::$id => TacticSpec { id: self, $($field: $value),+ }),+ }
            }
        }
    };
}

use AnalysisRequirement as A;
use RuntimeRisk as R;
use TacticDefault as D;
use TacticInvalidation as I;
use TacticProducer as P;

// One row per accepted family. config-schema.mjs reads this same declaration.
declare_tactics! {
    DeadCodeElimination {
        name: "dead-code-elimination", javascript_only: false, minimum_effort: 0, startup_at_level_16: false,
        analysis: A::UsesAndEffects, default: D::Preset,
        producers: &[P::SharedRules, P::JavaScriptFormation], prerequisites: &[], risks: &[R::Neutral],
        invalidates: &[I::ProgramFacts, I::TargetHead, I::Names, I::RenderedFiles],
        purpose: "Remove unused operations, stores and private functions while retaining observable effects.",
        tradeoffs: "Usually reduces execution and size; compressed bytes can differ. Off retains optional dead-code work, not mandatory reachability and language lowering."
    },
    ConstantFolding {
        name: "constant-folding", javascript_only: false, minimum_effort: 0, startup_at_level_16: false,
        analysis: A::Values, default: D::Preset,
        producers: &[P::SharedRules, P::JavaScriptFormation, P::StructuralSearch, P::OutputFamilies], prerequisites: &[], risks: &[R::Neutral],
        invalidates: &[I::ProgramFacts, I::TargetHead, I::TargetTail, I::Names, I::RenderedFiles],
        purpose: "Evaluate bounded known operations and calls, propagate constants and consider literal representations.",
        tradeoffs: "Spends proof work to remove runtime work. Larger literals can hurt codec size; representation alternatives compete with full bytes. Off also vetoes root-string forwarding."
    },
    Inlining {
        name: "inlining", javascript_only: false, minimum_effort: 0, startup_at_level_16: false,
        analysis: A::CallsAndCaptures, default: D::Preset,
        producers: &[P::SharedRules, P::JavaScriptFormation, P::StructuralSearch, P::OutputFamilies], prerequisites: &[], risks: &[R::Neutral],
        invalidates: &[I::ProgramFacts, I::TargetHead, I::TargetTail, I::Names, I::RenderedFiles],
        purpose: "Replace proved calls with their bodies to expose simplification and remove call overhead.",
        tradeoffs: "Can increase text and compilation work. Shared removal-only inlining also needs dead-code-elimination; target implementations need target-compaction. Off vetoes all optional inlining."
    },
    ScalarReplacement {
        name: "scalar-replacement", javascript_only: true, minimum_effort: 0, startup_at_level_16: false,
        analysis: A::OwnershipAndObservations, default: D::Preset,
        producers: &[P::JavaScriptFormation, P::StructuralSearch], prerequisites: &[], risks: &[R::Neutral],
        invalidates: &[I::TargetHead, I::Names, I::RenderedFiles],
        purpose: "Replace proved private object storage with independent values.",
        tradeoffs: "May remove allocations and fields but add locals or longer text. Escape, identity, initialization and capture proofs remain mandatory; off keeps the aggregate representation."
    },
    CallSpecialization {
        name: "call-specialization", javascript_only: true, minimum_effort: 0, startup_at_level_16: false,
        analysis: A::CallsAndCaptures, default: D::Preset,
        producers: &[P::StructuralSearch], prerequisites: &[], risks: &[R::Neutral],
        invalidates: &[I::TargetHead, I::Names, I::RenderedFiles],
        purpose: "Specialize proved private function signatures and transport of product arguments.",
        tradeoffs: "Additional proofs and candidates can remove allocations or argument handling; more parameters can cost bytes. On permits competition only where every call is known."
    },
    HelperSharing {
        name: "helper-sharing", javascript_only: true, minimum_effort: 0, startup_at_level_16: false,
        analysis: A::CallsAndCaptures, default: D::Preset,
        producers: &[], prerequisites: &[], risks: &[R::Neutral], invalidates: &[],
        purpose: "Compatibility name for optional parameterized helper sharing; no implementation is currently available.",
        tradeoffs: "Explicit on reports unavailable and does not alter output. Mandatory runtime helpers remain language lowering; this flag does not control them."
    },
    TargetCompaction {
        name: "target-compaction", javascript_only: true, minimum_effort: 0, startup_at_level_16: false,
        analysis: A::TargetSchedule, default: D::On,
        producers: &[P::JavaScriptFormation, P::OutputFamilies], prerequisites: &[], risks: &[R::Neutral],
        invalidates: &[I::TargetHead, I::TargetTail, I::Names, I::RenderedFiles],
        purpose: "Permit optional target-tree cleanup, statement spellings and output families.",
        tradeoffs: "Adds formation and search work; spelling wins depend on the objective. Semantic subfamilies also need their own permissions. Off keeps mandatory lowering and independently permitted structural search."
    },
    IdentifierMangling {
        name: "identifier-mangling", javascript_only: true, minimum_effort: 0, startup_at_level_16: false,
        analysis: A::NamesAndBoundary, default: D::On,
        producers: &[P::Naming], prerequisites: &[], risks: &[R::Neutral],
        invalidates: &[I::Names, I::RenderedFiles],
        purpose: "Assign short hygienic names to private lexical bindings.",
        tradeoffs: "Usually reduces bytes without runtime work; names affect codec repetition. Off retains source naming and disables dependent naming trials; public-name contracts always apply."
    },
    PropertyMangling {
        name: "property-mangling", javascript_only: true, minimum_effort: 0, startup_at_level_16: false,
        analysis: A::NamesAndBoundary, default: D::On,
        producers: &[P::JavaScriptFormation, P::OutputFamilies], prerequisites: &[TacticId::TargetCompaction], risks: &[R::Neutral],
        invalidates: &[I::TargetHead, I::Names, I::RenderedFiles],
        purpose: "Try coherent short names for proved private fields, including reuse across unrelated types.",
        tradeoffs: "Requires observation and inheritance proofs and extra formation/judgments. Preserved or reflected keys keep their names. Independent of lexical mangling; off vetoes cached and direct assignments too."
    },
    StringPooling {
        name: "string-pooling", javascript_only: true, minimum_effort: 0, startup_at_level_16: false,
        analysis: A::Values, default: D::On,
        producers: &[P::StructuralSearch, P::OutputFamilies], prerequisites: &[], risks: &[R::Neutral],
        invalidates: &[I::TargetHead, I::TargetTail, I::Names, I::RenderedFiles],
        purpose: "Share repeated literal strings or numbers through bindings.",
        tradeoffs: "Introduces bindings and reads; repetitive literal bytes can already compress well. Structural computed-string pooling additionally needs constant-folding. Off vetoes pooling in every route."
    },
    StringArrayPacking {
        name: "string-array-packing", javascript_only: true, minimum_effort: 0, startup_at_level_16: true,
        analysis: A::Values, default: D::On,
        producers: &[P::OutputFamilies], prerequisites: &[], risks: &[R::Neutral, R::Startup, R::Recurring],
        invalidates: &[I::TargetTail, I::Names, I::RenderedFiles],
        purpose: "Represent string arrays as delimited text decoded with split under pristine-builtins assumptions.",
        tradeoffs: "Can save bytes while adding decoding and allocations. Version 3 requires explicit on, including callable sites; legacy auto permits startup sites at effort 16 only. Off vetoes packing at all levels."
    },
    StartupReconstruction {
        name: "startup-reconstruction", javascript_only: true, minimum_effort: 16, startup_at_level_16: true,
        analysis: A::Values, default: D::On,
        producers: &[P::JavaScriptFormation], prerequisites: &[], risks: &[R::Neutral, R::Startup],
        invalidates: &[I::TargetHead, I::Names, I::RenderedFiles],
        purpose: "Permit constant-table encodings with startup decoders.",
        tradeoffs: "Trades startup CPU/storage for possible delivery savings and extra search. Version 3 requires explicit on at every effort; legacy auto grants it at 16. Does not permit recurring decoding or string-array packing."
    },
    RecurringReconstruction {
        name: "recurring-reconstruction", javascript_only: true, minimum_effort: 0, startup_at_level_16: false,
        analysis: A::Values, default: D::Off,
        producers: &[], prerequisites: &[], risks: &[R::Neutral, R::Recurring], invalidates: &[],
        purpose: "Compatibility name for recurring table reconstruction; no implementation is currently available.",
        tradeoffs: "Explicit on reports unavailable. String-array-packing owns its own callable-site risk and is not enabled by this flag."
    },
    NamingSearch {
        name: "naming-search", javascript_only: true, minimum_effort: 0, startup_at_level_16: false,
        analysis: A::NamesAndBoundary, default: D::On,
        producers: &[P::Naming], prerequisites: &[TacticId::IdentifierMangling], risks: &[R::Neutral],
        invalidates: &[I::Names, I::RenderedFiles],
        purpose: "Explore lexical allocation styles, name reuse and local refinements under the selected objective.",
        tradeoffs: "Spends rendering and codec work for possible byte savings without intended runtime work. Effort schedules bound exploration; off keeps the baseline allocator."
    },
    NamingAlphabet {
        name: "naming-alphabet", javascript_only: true, minimum_effort: 0, startup_at_level_16: false,
        analysis: A::NamesAndBoundary, default: D::On,
        producers: &[P::Naming], prerequisites: &[TacticId::IdentifierMangling, TacticId::NamingSearch], risks: &[R::Neutral],
        invalidates: &[I::Names, I::RenderedFiles],
        purpose: "Try alternative hygienic naming alphabets as joint name assignments.",
        tradeoffs: "May rename the whole artifact and add expensive codec trials for small or absent savings. Naming-search and identifier-mangling must remain permitted; off preserves their other strategies."
    }
}
