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
/// depends on the codec. Target compaction's permission governs all of them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OutputFamilies {
    /// A function called once becomes a block at its call, its parameters
    /// copies to forward (Closure's FunctionInjector block mode).
    pub block_inlining: bool,
    /// Nested blocks flatten into their parent where no declaration would
    /// change scope.
    pub flat_blocks: bool,
    /// The statement spellings.
    pub statements: StatementSpellings,
    /// String arrays packed as one split string, then repeated strings and
    /// numbers read through one binding (Closure's AliasStrings).
    pub string_pooling: bool,
    /// Print `{let i=v;for(;c;u)b}` as `for(let i=v;c;u)b`.
    pub loop_heads: bool,
    /// Print `if(c)e;` as `c&&e;` (and `if(!c)e;` as `c||e;`) where neither
    /// side needs grouping. A raw naming plan prints this spelling anyway.
    pub logical_statements: bool,
}

impl OutputFamilies {
    /// No family: an artifact without target compaction.
    pub const NONE: Self = Self {
        block_inlining: false,
        flat_blocks: false,
        statements: StatementSpellings::NONE,
        string_pooling: false,
        loop_heads: false,
        logical_statements: false,
    };

    /// The alternative each family starts from under `codec`. Raw bytes seed
    /// every raw-shaped family on; a codec seeds only the loop-head spelling,
    /// which was never measured larger there. The logical-statement print is
    /// part of the raw naming plan's spelling, so its own flag stays off.
    pub fn seed(codec: Objective) -> Self {
        match codec {
            Objective::Raw => Self {
                block_inlining: true,
                flat_blocks: true,
                statements: StatementSpellings::ALL,
                string_pooling: true,
                loop_heads: true,
                logical_statements: false,
            },
            Objective::Gzip | Objective::Brotli => Self {
                loop_heads: true,
                ..Self::NONE
            },
        }
    }

    /// The same printed program: a raw naming plan already prints logical
    /// statements, so under one the flag makes no difference.
    fn effective(self, raw_spelling: bool) -> Self {
        Self {
            logical_statements: self.logical_statements || raw_spelling,
            ..self
        }
    }
}

/// One complete output assignment a terminal challenger proposes: the
/// families formation applies and the naming plan's raw spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Spelling {
    pub families: OutputFamilies,
    /// `Plan::raw_spelling`: self-named functions, compound assignment,
    /// statement-consuming conditionals, logical statements and quotes.
    pub raw_spelling: bool,
}

impl Spelling {
    /// The seed of `codec`, as the search forms and names its candidates.
    pub fn seed(codec: Objective) -> Self {
        Self {
            families: OutputFamilies::seed(codec),
            raw_spelling: codec == Objective::Raw,
        }
    }

    /// Two assignments that print the same program compare equal here.
    pub fn effective(self) -> Self {
        Self {
            families: self.families.effective(self.raw_spelling),
            raw_spelling: self.raw_spelling,
        }
    }
}

/// A declared terminal challenger: a named alternative applied to the final
/// candidate's assignment. The order is the declared schedule and the
/// policy's effort sets how long a prefix of it is tried, so the order is
/// the expected value per trial. Measured when the stage landed, on seven
/// reference ports with every challenger tried: conditional values, exit
/// points and loop fusion were kept under Brotli on three, three and one
/// ports (−49, −152, −124; record 013-T7.2 found the same codec-selected
/// subset, −462 over six ports); block inlining was kept under Brotli on two
/// ports (−21), and turned off under raw on motionlil (−993); the raw
/// spelling turned off under raw on posthoglil (−161); conditional returns
/// won one port (−27). An earlier run of the same schedule, from seeds
/// without the generalized exit rules, also kept logical statements on
/// katexlil (−51). The rest won nowhere there and stay for the artifact
/// where they do: the other objective's whole seed, flat blocks, loop heads,
/// string pooling and logical branches (if to `&&`, +1,180 in total in the
/// record).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Challenger {
    /// The other objective's whole seed: the raw seed under gzip or Brotli
    /// (katexlil's raw build scored 64,693 Brotli against its Brotli build's
    /// 64,886), the codec seed under raw (probe f1's raw artifact was larger
    /// in raw bytes than its Brotli one).
    OtherSeed,
    ConditionalValues,
    ExitPoints,
    LoopFusion,
    RawSpelling,
    LoopHeads,
    LogicalStatements,
    BlockInlining,
    FlatBlocks,
    StringPooling,
    ConditionalReturns,
    LogicalBranches,
}

impl Challenger {
    /// The declared schedule.
    pub const ORDER: [Self; 12] = [
        Self::ConditionalValues,
        Self::ExitPoints,
        Self::LoopFusion,
        Self::BlockInlining,
        Self::RawSpelling,
        Self::LogicalStatements,
        Self::ConditionalReturns,
        Self::OtherSeed,
        Self::FlatBlocks,
        Self::LoopHeads,
        Self::StringPooling,
        Self::LogicalBranches,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::OtherSeed => "other-objective-seed",
            Self::ConditionalValues => "conditional-values",
            Self::ExitPoints => "exit-points",
            Self::LoopFusion => "loop-fusion",
            Self::RawSpelling => "raw-spelling",
            Self::LoopHeads => "loop-heads",
            Self::LogicalStatements => "logical-statements",
            Self::BlockInlining => "block-inlining",
            Self::FlatBlocks => "flat-blocks",
            Self::StringPooling => "string-pooling",
            Self::ConditionalReturns => "conditional-returns",
            Self::LogicalBranches => "logical-branches",
        }
    }

    /// Whether the challenger changes formation (and so needs target
    /// compaction's permission), rather than only the naming plan.
    pub fn forms(self) -> bool {
        !matches!(self, Self::RawSpelling)
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
            Self::RawSpelling => next.raw_spelling ^= true,
            Self::LoopHeads => families.loop_heads ^= true,
            Self::LogicalStatements => families.logical_statements ^= true,
            Self::BlockInlining => families.block_inlining ^= true,
            Self::FlatBlocks => families.flat_blocks ^= true,
            Self::StringPooling => families.string_pooling ^= true,
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

    #[test]
    fn a_raw_plan_prints_logical_statements_either_way() {
        let raw = Spelling::seed(Objective::Raw);
        assert_eq!(
            Challenger::LogicalStatements
                .apply(Objective::Raw, raw)
                .effective(),
            raw.effective()
        );
        let brotli = Spelling::seed(Objective::Brotli);
        assert_ne!(
            Challenger::LogicalStatements
                .apply(Objective::Brotli, brotli)
                .effective(),
            brotli.effective()
        );
    }
}
