//! Output families: shapes whose value depends on the program and the codec,
//! never on meaning (architecture §9, "the objective is a judge, not a rule";
//! plan M9.2 and M9.3).
//!
//! Every family is available under every objective. The objective supplies
//! only each family's *seed*, the alternative tried first: the raw objective
//! starts from the raw-shaped alternatives (Terser's `conditionals`,
//! `if_return`, `sequences` and `join_vars`-style statement forms, Closure's
//! late MinimizeExitPoints / MinimizeConditions / StatementFusion, block
//! inlining, string pooling), the codecs from the canonical ones. The terminal
//! stage (`program::search`) then offers the other alternatives as declared
//! challengers, each kept only when the exact requested codec says the whole
//! artifact shrank. Measured why this is a choice and not a rule: all 13 of
//! Closure's late peepholes cut 13,197 raw bytes and cost 930 Brotli over the
//! reference ports, and Terser's local rules are worth −382 Brotli chosen per
//! artifact against −8 forced on every one (record 013-T7(d); competitors
//! report §5.2).
//!
//! The decisions are data on the artifact: formation applies the families as
//! tree edits and writes the print decisions onto the module, so the printer
//! only renders what the tree says (never thread-local policy, live-16).
use super::selection::Objective;
use crate::compilation_policy::{ResolvedPolicy, RuntimeRisk, TacticId, TacticUse};

/// Where a selected string-array packing pass may add decoding work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArrayPacking {
    Disabled,
    Startup,
    Recurring,
}

impl ArrayPacking {
    fn from_policy(policy: &ResolvedPolicy) -> Self {
        for (permission, risk) in [
            (Self::Recurring, RuntimeRisk::Recurring),
            (Self::Startup, RuntimeRisk::Startup),
        ] {
            if policy
                .check_tactic_permissions(&[TacticUse {
                    tactic: TacticId::StringArrayPacking,
                    risk,
                }])
                .is_ok()
            {
                return permission;
            }
        }
        Self::Disabled
    }

    fn risk(self) -> Option<RuntimeRisk> {
        match self {
            Self::Disabled => None,
            Self::Startup => Some(RuntimeRisk::Startup),
            Self::Recurring => Some(RuntimeRisk::Recurring),
        }
    }
}

/// Optional semantic work performed on the target tree. This assignment is
/// part of a formed head's identity and of its artifacts' permission evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
pub struct TargetRules {
    pub constant_folding: bool,
    pub inlining: bool,
    pub scalar_replacement: bool,
    pub data_encoding: bool,
    pub array_packing: ArrayPacking,
    pub call_specialization: bool,
    pub helper_sharing: bool,
    pub parameterized_helpers: bool,
}

impl TargetRules {
    /// Inspection's neutral semantic rules. Reconstruction requires a
    /// resolved runtime permission, never an inspection default.
    pub const SEMANTIC: Self = Self {
        constant_folding: true,
        inlining: true,
        scalar_replacement: true,
        data_encoding: false,
        array_packing: ArrayPacking::Disabled,
        call_specialization: false,
        helper_sharing: false,
        parameterized_helpers: false,
    };
    pub const NONE: Self = Self {
        constant_folding: false,
        inlining: false,
        scalar_replacement: false,
        data_encoding: false,
        array_packing: ArrayPacking::Disabled,
        call_specialization: false,
        helper_sharing: false,
        parameterized_helpers: false,
    };

    pub fn from_policy(policy: &ResolvedPolicy) -> Self {
        Self {
            constant_folding: policy.tactic(TacticId::ConstantFolding).enabled,
            inlining: policy.tactic(TacticId::Inlining).enabled,
            scalar_replacement: policy.tactic(TacticId::ScalarReplacement).enabled,
            data_encoding: policy
                .check_tactic_permissions(&[TacticUse {
                    tactic: TacticId::StartupReconstruction,
                    risk: RuntimeRisk::Startup,
                }])
                .is_ok(),
            array_packing: ArrayPacking::from_policy(policy),
            call_specialization: policy.tactic(TacticId::CallSpecialization).enabled,
            helper_sharing: policy.tactic(TacticId::HelperSharing).enabled,
            parameterized_helpers: policy.check_tactic_permissions(&[TacticUse { tactic: TacticId::HelperSharing, risk: RuntimeRisk::Recurring }]).is_ok(),
        }
    }

    pub fn tactics(self) -> [(bool, TacticId); 5] {
        [
            (self.constant_folding, TacticId::ConstantFolding),
            (self.inlining, TacticId::Inlining),
            (self.scalar_replacement, TacticId::ScalarReplacement),
            (self.call_specialization, TacticId::CallSpecialization),
            (self.helper_sharing, TacticId::HelperSharing),
        ]
    }

    pub fn runtime_uses(self, families: OutputFamilies) -> impl Iterator<Item = TacticUse> {
        [
            families.parameterized_helpers.then_some(TacticUse { tactic: TacticId::HelperSharing, risk: RuntimeRisk::Recurring }),
            self.data_encoding.then_some(TacticUse {
                tactic: TacticId::StartupReconstruction,
                risk: RuntimeRisk::Startup,
            }),
            self.array_packing
                .risk()
                .filter(|_| families.string_array_packing)
                .map(|risk| TacticUse {
                    tactic: TacticId::StringArrayPacking,
                    risk,
                }),
        ]
        .into_iter()
        .flatten()
    }
}

/// Statement spellings a codec judges (M9.3). None of them removes an
/// operation: each only re-spells the same evaluations, which a codec's
/// repeat matching often prefers in statement form. The rules that do remove
/// operations (same-exit merge, trailing-statement dedup, exit to `break`)
/// are not here: `compress_statements` runs them under every objective.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StatementSpellings {
    /// `if(c)x=a;else x=b` as `x=c?a:b`, for a binding or a property of one
    /// (Closure's MinimizeConditions, Terser's `conditionals`).
    pub conditional_values: bool,
    /// `if(c){A;return}R` ending a body as `if(c){A}else{R}`
    /// (Closure's MinimizeExitPoints).
    pub exit_points: bool,
    /// `while(c){…;u}` as `for(;c;u){…}` (a statement fused into the loop
    /// head, Closure's StatementFusion in its loop form).
    pub loop_fusion: bool,
    /// `if(c)return a;e;return b` as `return c?a:(e,b)`, and an `if` whose
    /// branches both return as one return of a conditional (Terser's
    /// `if_return`).
    pub conditional_returns: bool,
    /// Branches of expression statements as one expression (`c&&(a,b)`,
    /// `c||b`, `c?a:b`), `if(a){if(b)S}` as `if(a&&b)S`, and conditionals
    /// that say less (`x?x:y` as `x||y`, `c?y:!1` as `c&&y`).
    pub logical_branches: bool,
}

impl StatementSpellings {
    pub const NONE: Self = Self {
        conditional_values: false,
        exit_points: false,
        loop_fusion: false,
        conditional_returns: false,
        logical_branches: false,
    };
    pub const ALL: Self = Self {
        conditional_values: true,
        exit_points: true,
        loop_fusion: true,
        conditional_returns: true,
        logical_branches: true,
    };
}

/// The output families of one artifact (M9.2). Each is a formation choice
/// with a written legality: every rewrite it names is exact under the
/// target's rules, so any assignment is a correct program; only its size
/// depends on the codec. Target compaction governs all of them; semantic
/// families additionally require their own tactic's permission.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OutputFamilies {
    /// A function called once becomes a block at its call, its parameters
    /// copies to forward (Closure's FunctionInjector block mode).
    pub block_inlining: bool,
    /// Bounded expression duplication, retained only after exact judging.
    pub expression_inlining: bool,
    /// Specialize private primitive helpers on uniform constant actuals.
    pub call_specialization: bool,
    /// Share alpha-equivalent private primitive implementations.
    pub helper_sharing: bool,
    /// Share literal-differing bodies with extra constant arguments.
    pub parameterized_helpers: bool,
    /// Nested blocks flatten into their parent where no declaration would
    /// change scope.
    pub flat_blocks: bool,
    /// The statement spellings.
    pub statements: StatementSpellings,
    /// Repeated strings and numbers read through one binding (AliasStrings).
    pub string_pooling: bool,
    /// String arrays packed as one split string under pristine builtins.
    pub string_array_packing: bool,
    /// Root constants holding a string read as their literal wherever they
    /// are initialized, rather than through their name (M7.4: a longer
    /// value's forwarding is a choice, and numbers, booleans, `null` and
    /// `undefined` forward by rule). Measured both ways on 2026-09-29 over
    /// the reference ports' main files: named, one is 238 Brotli bytes
    /// smaller and another 264 larger (the record names them).
    pub string_constants: bool,
    /// Print `{let i=v;for(;c;u)b}` as `for(let i=v;c;u)b`.
    pub loop_heads: bool,
    /// Print `if(c)e;` as `c&&e;` (and `if(!c)e;` as `c||e;`) where neither
    /// side needs grouping.
    pub logical_statements: bool,
    /// Print `x=x+y` as `x+=y` (for a binding or a named property of one or
    /// of `this`): shorter, but a repeated `x=x+` is nearly free under a
    /// codec.
    pub compound_assignments: bool,
    /// Print a string in the quote it escapes least (`'{"a":1}'`). A codec
    /// keeps one delimiter: the escaped `\"` pairs of a JSON text compress
    /// well, so fewer escapes need not produce a smaller compressed artifact.
    /// Both spellings are exact-judged choices under every objective.
    pub quotes: bool,
    /// A redundant `|0` where the compiler printed one before R1, R10 and
    /// R11 made it unnecessary: after an `int` field, member or element read,
    /// an `int` host call's result, and an integer method's result without
    /// pristine builtins. An int32 is its own ToInt32, so the spelling means
    /// the same program; a codec's repeat matching sometimes prefers it
    /// (repetition is load-bearing: a tiny artifact measured up to 5 Brotli
    /// bytes larger without it). Every objective seeds it off; its
    /// challenger runs first, so a level that searches keeps the previous
    /// compiler's spelling wherever the codec says it is smaller (AM2).
    pub int32_hints: bool,
    /// Rename proved private instance fields coherently before target cleanup.
    /// This is a concrete representation candidate, not a printer substitution.
    pub property_mangling: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct HeadChoices {
    pub int32_hints: bool,
    pub property_mangling: bool,
}

impl HeadChoices {
    pub(crate) fn index(self) -> usize {
        usize::from(self.int32_hints) | (usize::from(self.property_mangling) << 1)
    }
}

impl OutputFamilies {
    /// Whole-family flags are defaults/joint moves for per-site choices.
    pub fn site_seed(self, family: crate::representation::ChoiceFamily) -> Option<bool> {
        use crate::representation::ChoiceFamily as F;
        Some(match family {
            F::ExpressionInlining => self.expression_inlining,
            F::ConstantArguments => self.call_specialization,
            F::HelperSharing => self.helper_sharing,
            F::ParameterizedHelpers => self.parameterized_helpers,
            _ => return None,
        })
    }

    /// No family: an artifact without target compaction.
    pub const NONE: Self = Self {
        block_inlining: false,
        expression_inlining: false,
        call_specialization: false,
        helper_sharing: false,
        parameterized_helpers: false,
        flat_blocks: false,
        statements: StatementSpellings::NONE,
        string_pooling: false,
        string_array_packing: false,
        string_constants: false,
        loop_heads: false,
        logical_statements: false,
        compound_assignments: false,
        quotes: false,
        int32_hints: false,
        property_mangling: false,
    };

    /// The alternative each family starts from under `codec`. Raw bytes seed
    /// every raw-shaped family on; a codec seeds only the loop-head spelling,
    /// which was never measured larger there. A codec starts with string
    /// constants named: a walk from forwarded ones judged naming them one
    /// flip at a time and kept the forwarded form on the port that gains
    /// most from names (2026-09-29).
    pub fn seed(codec: Objective) -> Self {
        match codec {
            Objective::Raw => Self {
                block_inlining: true,
                expression_inlining: true,
                call_specialization: false,
                helper_sharing: false,
                parameterized_helpers: false,
                flat_blocks: true,
                statements: StatementSpellings::ALL,
                string_pooling: true,
                string_array_packing: true,
                string_constants: true,
                loop_heads: true,
                logical_statements: true,
                compound_assignments: true,
                quotes: true,
                int32_hints: false,
                property_mangling: false,
            },
            Objective::Gzip | Objective::Brotli => Self {
                loop_heads: true,
                ..Self::NONE
            },
        }
    }

    /// Semantic permissions required by this assignment. Keep generation,
    /// admission and retained provenance on the same registry.
    pub fn tactics(self) -> [(bool, TacticId); 9] {
        [
            (self.expression_inlining, TacticId::Inlining),
            (self.call_specialization, TacticId::CallSpecialization),
            (self.helper_sharing, TacticId::HelperSharing),
            (self.parameterized_helpers, TacticId::HelperSharing),
            (self.block_inlining, TacticId::Inlining),
            (self.string_constants, TacticId::ConstantFolding),
            (self.string_pooling, TacticId::StringPooling),
            (self.string_array_packing, TacticId::StringArrayPacking),
            (self.property_mangling, TacticId::PropertyMangling),
        ]
    }

    /// Seeds are defaults, so disabled families take their canonical form.
    /// Explicit assignments are checked rather than silently projected.
    pub fn permitted(mut self, policy: &ResolvedPolicy) -> Self {
        if !policy.tactic(TacticId::TargetCompaction).enabled {
            return Self::NONE;
        }
        self.block_inlining &= policy.tactic(TacticId::Inlining).enabled;
        self.expression_inlining &= policy.tactic(TacticId::Inlining).enabled;
        self.call_specialization &= policy.tactic(TacticId::CallSpecialization).enabled;
        self.helper_sharing &= policy.tactic(TacticId::HelperSharing).enabled;
        self.parameterized_helpers &= TargetRules::from_policy(policy).parameterized_helpers;
        self.string_constants &= policy.tactic(TacticId::ConstantFolding).enabled;
        self.string_pooling &= policy.tactic(TacticId::StringPooling).enabled;
        self.string_array_packing &= ArrayPacking::from_policy(policy) != ArrayPacking::Disabled;
        self.property_mangling &= policy.tactic(TacticId::PropertyMangling).enabled;
        self
    }

    pub(crate) fn head(self) -> HeadChoices {
        HeadChoices {
            int32_hints: self.int32_hints,
            property_mangling: self.property_mangling,
        }
    }
}

/// One complete output assignment a terminal challenger proposes: the
/// families formation applies and the naming plan's two members.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Spelling {
    pub families: OutputFamilies,
    /// `Plan::self_named`: each function that is not an arrow and has an
    /// exact name prints as `function name(){…}`, so the binding holding it
    /// takes a short name.
    pub self_named: bool,
    /// `Plan::read_order`: the root's most read bindings take its shortest
    /// names.
    pub read_order: bool,
}

impl Spelling {
    /// The seed of `codec`, as the search forms and names its candidates.
    pub fn seed(codec: Objective) -> Self {
        Self {
            families: OutputFamilies::seed(codec),
            self_named: codec == Objective::Raw,
            read_order: codec == Objective::Raw,
        }
    }

    /// Two assignments that print the same program compare equal here: since
    /// the raw spelling's members are families of their own (M8.3), each
    /// assignment prints its own program.
    pub fn effective(self) -> Self {
        self
    }
}

/// A declared terminal challenger: a named alternative applied to the final
/// candidate's assignment. The order is the declared schedule and the
/// policy's effort sets how long a prefix of it is tried, so the order is
/// a prior for value per trial, never a semantic rule. The inherited order
/// remains a compatibility policy until generic calibration supports a change;
/// it is not a claim that every workload has that ranking. Complete-artifact
/// exact judging admits each challenger. Source provenance and calibration
/// scope are recorded in `benchmarks/calibration/policy-provenance.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Challenger {
    /// The other objective's whole seed, judged under the current objective.
    /// Compression can reverse the ranking of two raw spellings; neither
    /// seed is assumed to dominate the other.
    OtherSeed,
    ConditionalValues,
    ExitPoints,
    LoopFusion,
    SelfNamed,
    ReadOrder,
    CompoundAssignments,
    Quotes,
    LoopHeads,
    LogicalStatements,
    BlockInlining,
    FlatBlocks,
    StringPooling,
    StringArrayPacking,
    PoolingAndPacking,
    ConditionalReturns,
    LogicalBranches,
    Int32Hints,
    StringConstants,
    ExpressionInlining,
    CallSpecialization,
    HelperSharing,
    ParameterizedHelpers,
}

impl Challenger {
    /// The declared schedule.
    pub const ORDER: [Self; 23] = [
        Self::Int32Hints,
        Self::StringConstants,
        Self::ExpressionInlining,
        Self::CallSpecialization,
        Self::HelperSharing,
        Self::ParameterizedHelpers,
        Self::ConditionalValues,
        Self::ExitPoints,
        Self::LoopFusion,
        Self::BlockInlining,
        Self::SelfNamed,
        Self::ReadOrder,
        Self::CompoundAssignments,
        Self::Quotes,
        Self::LogicalStatements,
        Self::ConditionalReturns,
        Self::OtherSeed,
        Self::FlatBlocks,
        Self::LoopHeads,
        Self::PoolingAndPacking,
        Self::LogicalBranches,
        Self::StringPooling,
        Self::StringArrayPacking,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::OtherSeed => "other-objective-seed",
            Self::ConditionalValues => "conditional-values",
            Self::ExitPoints => "exit-points",
            Self::LoopFusion => "loop-fusion",
            Self::SelfNamed => "self-named-functions",
            Self::ReadOrder => "read-order",
            Self::CompoundAssignments => "compound-assignments",
            Self::Quotes => "quotes",
            Self::LoopHeads => "loop-heads",
            Self::LogicalStatements => "logical-statements",
            Self::BlockInlining => "block-inlining",
            Self::FlatBlocks => "flat-blocks",
            Self::StringPooling => "string-pooling",
            Self::StringArrayPacking => "string-array-packing",
            Self::PoolingAndPacking => "pooling-and-packing",
            Self::ConditionalReturns => "conditional-returns",
            Self::LogicalBranches => "logical-branches",
            Self::Int32Hints => "int32-hints",
            Self::StringConstants => "string-constants",
            Self::ExpressionInlining => "expression-inlining",
            Self::CallSpecialization => "call-specialization",
            Self::HelperSharing => "helper-sharing",
            Self::ParameterizedHelpers => "parameterized-helpers",
        }
    }

    /// Whether the challenger changes formation (and so needs target
    /// compaction's permission), rather than only the naming plan.
    pub fn forms(self) -> bool {
        !matches!(self, Self::SelfNamed | Self::ReadOrder)
    }

    /// This challenger's assignment, applied to the incumbent's under
    /// `codec`: the other seed replaces it whole; every other challenger
    /// flips its one family.
    pub fn apply(self, codec: Objective, incumbent: Spelling) -> Spelling {
        let mut next = incumbent;
        let families = &mut next.families;
        match self {
            Self::OtherSeed => {
                return Spelling::seed(if codec == Objective::Raw {
                    Objective::Brotli
                } else {
                    Objective::Raw
                });
            }
            Self::ConditionalValues => {
                families.statements.conditional_values ^= true;
            }
            Self::ExitPoints => families.statements.exit_points ^= true,
            Self::LoopFusion => families.statements.loop_fusion ^= true,
            Self::ConditionalReturns => {
                families.statements.conditional_returns ^= true;
            }
            Self::LogicalBranches => families.statements.logical_branches ^= true,
            Self::Int32Hints => families.int32_hints ^= true,
            Self::SelfNamed => next.self_named ^= true,
            Self::ReadOrder => next.read_order ^= true,
            Self::CompoundAssignments => families.compound_assignments ^= true,
            Self::Quotes => families.quotes ^= true,
            Self::LoopHeads => families.loop_heads ^= true,
            Self::LogicalStatements => families.logical_statements ^= true,
            Self::BlockInlining => families.block_inlining ^= true,
            Self::FlatBlocks => families.flat_blocks ^= true,
            Self::StringPooling => families.string_pooling ^= true,
            Self::StringArrayPacking => families.string_array_packing ^= true,
            Self::PoolingAndPacking => {
                families.string_pooling ^= true;
                families.string_array_packing ^= true;
            }
            Self::StringConstants => families.string_constants ^= true,
            Self::ExpressionInlining => families.expression_inlining ^= true,
            Self::CallSpecialization => families.call_specialization ^= true,
            Self::HelperSharing => families.helper_sharing ^= true,
            Self::ParameterizedHelpers => families.parameterized_helpers ^= true,
        }
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_family_is_reachable_from_every_seed() {
        for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
            let seed = Spelling::seed(codec);
            let other = Challenger::OtherSeed.apply(codec, seed);
            assert_ne!(seed, other, "{codec:?}");
            // Each single-family challenger flips exactly its family, in
            // both directions, so the codec can undo any seed's choice.
            for challenger in Challenger::ORDER
                .into_iter()
                .filter(|challenger| *challenger != Challenger::OtherSeed)
            {
                let once = challenger.apply(codec, seed);
                assert_ne!(once, seed, "{challenger:?} changes nothing");
                assert_eq!(challenger.apply(codec, once), seed, "{challenger:?}");
            }
        }
        assert_eq!(
            Spelling::seed(Objective::Brotli),
            Spelling::seed(Objective::Gzip)
        );
        assert_eq!(
            Challenger::OtherSeed.apply(Objective::Raw, Spelling::seed(Objective::Raw)),
            Spelling::seed(Objective::Brotli)
        );
    }

    /// The raw spelling's members are families of their own (M8.3): each
    /// changes the printed program under every seed.
    #[test]
    fn the_raw_spellings_members_are_each_a_family() {
        for codec in [Objective::Raw, Objective::Brotli] {
            let seed = Spelling::seed(codec);
            for challenger in [
                Challenger::SelfNamed,
                Challenger::ReadOrder,
                Challenger::CompoundAssignments,
                Challenger::Quotes,
                Challenger::LogicalStatements,
            ] {
                assert_ne!(
                    challenger.apply(codec, seed).effective(),
                    seed.effective(),
                    "{challenger:?} under {codec:?}"
                );
            }
        }
    }
}
