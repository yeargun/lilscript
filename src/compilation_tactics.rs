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
    NativeFormation,
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
            && match producer {
                TacticProducer::SharedRules => true,
                TacticProducer::NativeFormation => policy.javascript_contract().is_none(),
                _ => policy.javascript_contract().is_some(),
            }
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
        purpose: "Evaluate bounded known operations and calls, propagate exact/finite/path values, forward aliases and defaults, and consider literal representations.",
        tradeoffs: "Spends bounded proof work to remove runtime work; overflow retains unknown values and calls. Larger literals can hurt codec size; representation alternatives compete with full bytes. Off also vetoes common computations, default transport and root-string forwarding."
    },
    Inlining {
        name: "inlining", javascript_only: false, minimum_effort: 0, startup_at_level_16: false,
        analysis: A::CallsAndCaptures, default: D::Preset,
        producers: &[P::SharedRules, P::JavaScriptFormation, P::StructuralSearch, P::OutputFamilies], prerequisites: &[], risks: &[R::Neutral],
        invalidates: &[I::ProgramFacts, I::TargetHead, I::TargetTail, I::Names, I::RenderedFiles],
        purpose: "Replace proved calls with their bodies to expose simplification and remove call overhead.",
        tradeoffs: "Can increase text and compilation work. Shared removal-only inlining also needs dead-code-elimination; target implementations need target-compaction. Off vetoes all optional inlining."
    },
    LoopUnrolling {
        name: "loop-unrolling", javascript_only: false, minimum_effort: 0, startup_at_level_16: false,
        analysis: A::Values, default: D::On,
        producers: &[P::SharedRules], prerequisites: &[], risks: &[R::Neutral],
        invalidates: &[I::ProgramFacts, I::TargetHead, I::Names, I::RenderedFiles],
        purpose: "Honor an authored inline for over a checked constant scalar list on both targets.",
        tradeoffs: "Auto and on permit the explicit source pin at every effort; ordinary loops are unchanged. Expansion costs list length times body size and may grow compressed bytes while exposing constants and removing loop machinery. Off diagnoses a conflicting inline for instead of ignoring it. Private function inlining is independent."
    },
    ScalarReplacement {
        name: "scalar-replacement", javascript_only: false, minimum_effort: 0, startup_at_level_16: false,
        analysis: A::OwnershipAndObservations, default: D::Preset,
        producers: &[P::SharedRules, P::JavaScriptFormation, P::NativeFormation, P::StructuralSearch], prerequisites: &[], risks: &[R::Neutral],
        invalidates: &[I::ProgramFacts, I::TargetHead, I::TargetTail, I::Names, I::RenderedFiles],
        purpose: "Scalarize private aggregate fields, canonicalize record aliases, update uniquely owned products, and transfer consumed native values.",
        tradeoffs: "Trades bounded ownership/use analysis for fewer allocations, field stores and retain/release pairs. Captured banks keep their activation; live value snapshots, dynamic keys and host-visible identity retain storage. Record/product layout alternatives remain codec judged. Off vetoes scalar banks, alias elision, owned product updates and native transfers in every route."
    },
    CallSpecialization {
        name: "call-specialization", javascript_only: true, minimum_effort: 0, startup_at_level_16: false,
        analysis: A::CallsAndCaptures, default: D::Preset,
        producers: &[P::StructuralSearch, P::OutputFamilies], prerequisites: &[], risks: &[R::Neutral],
        invalidates: &[I::TargetHead, I::TargetTail, I::Names, I::RenderedFiles],
        purpose: "Specialize private signatures, product transport and uniform constant actuals as codec-judged alternatives.",
        tradeoffs: "Additional proofs and candidates can remove allocations or argument handling; more parameters can cost bytes. On permits competition only where every call is known."
    },
    HelperSharing {
        name: "helper-sharing", javascript_only: true, minimum_effort: 0, startup_at_level_16: false,
        analysis: A::CallsAndCaptures, default: D::Preset,
        producers: &[P::OutputFamilies], prerequisites: &[], risks: &[R::Neutral, R::Recurring],
        invalidates: &[I::TargetTail, I::Names, I::RenderedFiles],
        purpose: "Share identity-unobserved private primitive helpers as codec-judged alternatives.",
        tradeoffs: "Exact bodies can share without extra runtime operations. Parameterized sharing adds constant arguments and needs recurring runtime permission. Complete direct calls, initialized roots, compatible primitive types and captures are required. Extra matching and codec work may save bytes; off vetoes both optional forms, independently of mandatory runtime helpers."
    },
    TargetCompaction {
        name: "target-compaction", javascript_only: true, minimum_effort: 0, startup_at_level_16: false,
        analysis: A::TargetSchedule, default: D::On,
        producers: &[P::JavaScriptFormation, P::OutputFamilies], prerequisites: &[], risks: &[R::Neutral],
        invalidates: &[I::TargetHead, I::TargetTail, I::Names, I::RenderedFiles],
        purpose: "Permit optional target-tree cleanup, statement spellings and output families.",
        tradeoffs: "Adds formation and search work; spelling wins depend on the objective. Semantic subfamilies also need their own permissions. Off keeps mandatory lowering and independently permitted structural search."
    },
    StatementSpellings {
        name: "statement-spellings", javascript_only: true, minimum_effort: 0, startup_at_level_16: false,
        analysis: A::Values, default: D::On,
        producers: &[P::OutputFamilies], prerequisites: &[TacticId::TargetCompaction], risks: &[R::Neutral],
        invalidates: &[I::TargetTail, I::Names, I::RenderedFiles],
        purpose: "Choose statement, loop, assignment and quote spellings independently at legal sites.",
        tradeoffs: "Exact raw/gzip/Brotli judgments select complete files; more sites cost formation and scoring. Adds no runtime effects. Off keeps canonical spellings and vetoes explicit and cached assignments, including positive regional @choose pins; canonical pins remain permitted."
    },
    DeclarationOrder {
        name: "declaration-order", javascript_only: true, minimum_effort: 14, startup_at_level_16: false,
        analysis: A::NamesAndBoundary, default: D::On,
        producers: &[P::OutputFamilies], prerequisites: &[TacticId::TargetCompaction], risks: &[R::Neutral],
        invalidates: &[I::TargetTail, I::Names, I::RenderedFiles],
        purpose: "Try grouping legally movable declarations by shape and literal content for codec locality.",
        tradeoffs: "Automatic from effort 14; explicit on permits earlier trials. Preserves effects, initialization and module boundaries. Extra discovery and judgments may earn only small codec wins; source order remains eligible."
    },
    ReceiverAliases {
        name: "receiver-aliases", javascript_only: true, minimum_effort: 13, startup_at_level_16: false,
        analysis: A::NamesAndBoundary, default: D::On,
        producers: &[P::OutputFamilies], prerequisites: &[TacticId::TargetCompaction], risks: &[R::Neutral],
        invalidates: &[I::TargetTail, I::Names, I::RenderedFiles],
        purpose: "Try a private local alias for repeated reads of an initialized function receiver.",
        tradeoffs: "Adds a local binding/read and formation/judging work; the direct receiver remains eligible. Never moves a receiver read ahead of super or across lexical-this ownership. Off vetoes explicit and retained choices, including positive regional @choose(receiver_alias=1) pins."
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
        purpose: "Try coherent short names for proved private fields, including generics and reuse across sibling or unrelated types.",
        tradeoffs: "Requires observation and inheritance proofs and extra formation/judgments; adds no runtime work. Kept classes require pristine builtins and cannot declare __proto__. Reflected, external and published families keep their keys. Independent of lexical mangling; off vetoes cached and direct assignments too."
    },
    StringPooling {
        name: "string-pooling", javascript_only: true, minimum_effort: 0, startup_at_level_16: false,
        analysis: A::Values, default: D::On,
        producers: &[P::StructuralSearch, P::OutputFamilies], prerequisites: &[], risks: &[R::Neutral],
        invalidates: &[I::TargetHead, I::TargetTail, I::Names, I::RenderedFiles],
        purpose: "Share repeated literal strings or numbers through bindings and honor authored @pool regions.",
        tradeoffs: "Introduces bindings and reads; repetitive literal bytes can already compress well. An authored pool admits its decoded string values even for one live use, at every effort and objective; it may increase raw or compressed size. Structural computed-string pooling additionally needs constant-folding. Off vetoes pooling in every JavaScript route and diagnoses conflicting source pins. Native string constants already share immutable payloads without an optional transformation."
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
        tradeoffs: "Spends rendering and codec work for possible byte savings without intended runtime work. Effort schedules bound exploration. The separately controlled naming-compaction alternative skips dead bindings and uses printed order/full identifier continuations; the prior result remains eligible. Off keeps the baseline allocator."
    },
    NamingCompaction {
        name: "naming-compaction", javascript_only: true, minimum_effort: 14, startup_at_level_16: false,
        analysis: A::NamesAndBoundary, default: D::On,
        producers: &[P::Naming], prerequisites: &[TacticId::IdentifierMangling, TacticId::NamingSearch], risks: &[R::Neutral],
        invalidates: &[I::Names, I::RenderedFiles],
        purpose: "Try live printed-order allocation without dead identifier reservations.",
        tradeoffs: "Automatic from 14; explicit on enables the final trial at 13. Useful after binding deletion or large-scope rewrites, with full identifier continuation characters. Adds rendering/codec work without runtime work; qualification found no byte gain on the initial small and medium cohorts. Off preserves other naming strategies and the prior allocator."
    },
    RepresentationJoints {
        name: "representation-joints", javascript_only: true, minimum_effort: 14, startup_at_level_16: false,
        analysis: A::TargetSchedule, default: D::On,
        producers: &[P::OutputFamilies], prerequisites: &[TacticId::TargetCompaction], risks: &[R::Neutral],
        invalidates: &[I::TargetTail, I::Names, I::RenderedFiles],
        purpose: "Explore bounded combinations of per-site representations whose individual moves may lose.",
        tradeoffs: "Automatic from effort 14; explicit on enables it earlier. Adds formation and exact-codec work for possibly tiny gains. Off preserves individual choices, whole-family defaults and canonical rollback; selected families still need their own runtime permissions."
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
