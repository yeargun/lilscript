//! The JavaScript target rules and their schedule (plan M5.3a; architecture
//! §8.2): the passes that edit a formed tree, as data, run by the one rule
//! scheduler (`crate::schedule`) to their fixed point.
//!
//! Each rule set runs its rules in their declared order, round after round,
//! until a round in which no rule recorded an edit in the journal (M5.2).
//! That replaces the hand-written chain and its hand rounds: a rule that
//! exposes work for an earlier one gets it in the next round, however many
//! rounds that takes. The order depends on no printed size, name plan, codec
//! or effort level. A round ceiling that is reached is a compiler bug and
//! fails the build: it never delivers a partial tree.
//!
//! Most rules here are **transitional** (L20): one of today's passes, hosted
//! in its relative order until the task named by `Rule::transitional` lands
//! its replacement and deletes it. A classified rule is legal by the tree's
//! own syntax or its annotations, and strictly decreases the measure
//! (reachable functions, then reachable nodes), which test and debug builds
//! assert after each of its applications.
//!
//! Test and debug builds check every rule's journal against the actual
//! difference (`journal.rs`) and verify the tree after every round.

use super::root_constants::ConstantKind;
use super::*;

/// One rule of the JavaScript target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rule {
    SelfMethodCalls,
    SimplifyOperators,
    InlineExpressionFunctions,
    InlineStatementFunctions,
    EliminateAliases,
    FoldLiteralOperations,
    ForwardRootConstants,
    ForwardRootStrings,
    ForwardSingleUses,
    ElideUndefined,
    MergeDeclarations,
    FoldLogicalAssignments,
    FoldLogicalReturns,
    FlattenConstantObjects,
    UnobserveCalledNames,
    DropTypedDefaultChecks,
    InlineInitializers,
    FoldObjectStores,
    DropDoubleNegations,
    InlineSingleCalls,
    FlattenBlocks,
    CompressStatements,
    PlaceSingleCalls,
    DropRedundantInitStores,
    ScalarizeMemberObjects,
    DropUnreferencedFunctions,
    DropUnreachable,
    GroupPrototypeStores,
    JoinEmptyDeclarations,
    DropBareBlocks,
    TruthyNullTests,
    ArrayReceiverCalls,
    EncodeTables,
    DropDefaultArguments,
    NativeDefaultLengths,
    PackStringArrays,
    PoolStrings,
}

impl Rule {
    /// For a transitional rule, the plan task whose landing deletes it
    /// (architecture §8.2's table); `None` for a classified rule, whose
    /// legality is the tree's own syntax or its annotations.
    pub(crate) fn transitional(self) -> Option<&'static str> {
        Some(match self {
            // Fact-free JS target rules: classified. Each removes nodes
            // and nothing else.
            Self::ElideUndefined
            | Self::DropUnreachable
            | Self::DropBareBlocks
            | Self::DropDoubleNegations => return None,
            Self::SelfMethodCalls => "M10.4/M10.7 (receivers)",
            Self::ArrayReceiverCalls => "M6.4b (array class)",
            Self::InlineExpressionFunctions
            | Self::InlineStatementFunctions
            | Self::InlineSingleCalls
            | Self::PlaceSingleCalls => "M7.5a (removing case), M9.1 (duplicating case)",
            Self::EliminateAliases | Self::ForwardRootConstants => "M7.4",
            // The family's choice goes to the choice system (M9.1).
            Self::ForwardRootStrings => "M7.4 and M9.1",
            Self::FlattenConstantObjects | Self::UnobserveCalledNames => "M7.6",
            Self::InlineInitializers | Self::DropRedundantInitStores | Self::FoldObjectStores => {
                "M9.7 and M7.7"
            }
            Self::DropTypedDefaultChecks
            | Self::DropDefaultArguments
            | Self::NativeDefaultLengths => "M7.3",
            Self::ScalarizeMemberObjects => "M7.9",
            Self::DropUnreferencedFunctions => "M5.1 (DCE as a program edit)",
            // JS target rules kept on the tree, classified once their
            // legality reads the annotations instead of `quiet.rs` (or,
            // for the declaration joins, once a measure covers moves).
            Self::ForwardSingleUses
            | Self::MergeDeclarations
            | Self::JoinEmptyDeclarations
            | Self::SimplifyOperators
            | Self::TruthyNullTests
            | Self::FoldLiteralOperations => "M5.2 (annotations)",
            Self::FoldLogicalAssignments | Self::FoldLogicalReturns => "M8.2 A2",
            Self::CompressStatements | Self::FlattenBlocks => "M8.3 (per-site spellings)",
            Self::EncodeTables | Self::PackStringArrays | Self::PoolStrings => "M9.8",
            Self::GroupPrototypeStores => "M8.7",
        })
    }
}

/// What a build permits the rules and what its artifact chose.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Context<'a> {
    /// Host reflection over the program's frames is outside the contract
    /// (strict code, or an application's world, Y5): a function's frame may
    /// be elided or moved.
    pub(crate) frames_hidden: bool,
    /// The whole artifact executes in strict mode; frame privacy alone
    /// cannot change failed writes, delete, or inherited strictness.
    pub(crate) strict: bool,
    /// The contract assumes unpatched builtins: stores fold into literals
    /// under new keys as well.
    pub(crate) pristine: bool,
    /// Dead-code elimination is permitted.
    pub(crate) prunes: bool,
    pub(crate) numeric_lengths: bool,
    pub(crate) year: u16,
    pub(crate) statements: StatementSpellings,
    pub(crate) choices: Option<&'a ChoiceMap>,
}

/// Bodies up to six nodes: measured best on the reference ports (a limit of
/// 3 keeps markedlil 97 bytes larger; 10 and 20 add nothing). A parameter of
/// the transitional inliner, deleted with it (M7.5a).
const INLINE_LIMIT: usize = 6;

/// Rounds a rule set may take. Each rule's edits remove or move structure,
/// so a few rounds reach the fixed point on every tree formation produces;
/// this ceiling only catches a pair of rules undoing each other.
const ROUND_CEILING: u32 = 64;

/// The family-independent rules every formed head runs, in the order the
/// hand-written chain ran them.
pub(crate) const HEAD: &[Rule] = &[
    // Exact folds first: an inlining candidate is judged by its size, so
    // `!!Number.isInteger(v)` should already read `Number.isInteger(v)`.
    // `x.m.call(x,…)` is `x.m(…)` for the operator rules too.
    Rule::SelfMethodCalls,
    Rule::SimplifyOperators,
    Rule::InlineExpressionFunctions,
    Rule::InlineStatementFunctions,
    Rule::EliminateAliases,
    Rule::FoldLiteralOperations,
    // Literal root constants of a few characters are their literal
    // wherever they are initialized (by initialization order); strings are
    // the tail's `string_constants` family.
    Rule::ForwardRootConstants,
    Rule::ForwardSingleUses,
    Rule::ElideUndefined,
    // `let o;o={…}` must meet as `let o={…}` before stores fold.
    Rule::MergeDeclarations,
    // `let t=a;if(!t)t=b` is `let t=a||b`, as the source wrote it, and
    // `if(t)return t;return b` is `return t||b`.
    Rule::FoldLogicalAssignments,
    Rule::FoldLogicalReturns,
    Rule::FlattenConstantObjects,
    Rule::UnobserveCalledNames,
    // Typed callers pass every typed argument: their callees' defaults for
    // those never apply.
    Rule::DropTypedDefaultChecks,
    // Field initializers become their stores, for the fold to take.
    Rule::InlineInitializers,
    Rule::FoldObjectStores,
    Rule::DropDoubleNegations,
];

/// The rules of an artifact's output families, in the order the tail ran
/// them: block inlining, flat blocks and the statement rules, then the late
/// passes that follow them.
pub(crate) fn tail(families: &OutputFamilies, prunes: bool) -> Vec<Rule> {
    let mut rules = Vec::with_capacity(24);
    // String root constants read as their literals, when the artifact's
    // family says so (M7.4's longer values: a choice).
    if families.string_constants {
        rules.push(Rule::ForwardRootStrings);
    }
    if families.block_inlining {
        // Functions with one call, as statements, take its place; their
        // parameters are then copies to forward.
        rules.extend([Rule::InlineSingleCalls, Rule::EliminateAliases]);
    }
    rules.push(Rule::ForwardSingleUses);
    if families.flat_blocks {
        rules.push(Rule::FlattenBlocks);
    }
    rules.extend([
        // Redundant exits and repeated statements go under every objective;
        // the statement spellings are this artifact's choice.
        Rule::CompressStatements,
        // A store of a conditional is a store the fold can take, and
        // conditionals built from statements meet the operator rules.
        Rule::FoldObjectStores,
        Rule::SimplifyOperators,
        // `let c;c=x?a:b`, built from statements, is `let c=x?a:b` (M8.2
        // A2's declaration fusion), and then a single use to forward.
        Rule::MergeDeclarations,
        // A function left with one call is created there.
        Rule::PlaceSingleCalls,
        Rule::DropRedundantInitStores,
        Rule::ScalarizeMemberObjects,
    ]);
    if prunes {
        rules.push(Rule::DropUnreferencedFunctions);
    }
    rules.extend([
        Rule::DropUnreachable,
        // Regrouped where the printer lists them.
        Rule::GroupPrototypeStores,
        Rule::JoinEmptyDeclarations,
        Rule::DropBareBlocks,
        // What the source types allow.
        Rule::TruthyNullTests,
        Rule::ArrayReceiverCalls,
        // Constant data tables, as the artifact's choice map names (M9.8).
        Rule::EncodeTables,
        Rule::DropDefaultArguments,
        Rule::NativeDefaultLengths,
    ]);
    rules
}

/// Repeated strings last, once no other rule reads a literal: packed arrays,
/// then root constants.
pub(crate) const POOLING: &[Rule] = &[Rule::PackStringArrays, Rule::PoolStrings];

/// Why a rule set stopped without its fixed point.
#[derive(Debug)]
pub(crate) enum RuleError {
    Allocation(AllocationError),
    /// A compiler bug: the rules did not converge, or a rule's journal
    /// disagreed with its edits (test and debug builds).
    Bug(String),
}

impl From<AllocationError> for RuleError {
    fn from(error: AllocationError) -> Self {
        Self::Allocation(error)
    }
}

impl Module {
    /// Run `rules` to their fixed point; returns the rounds taken.
    pub(crate) fn run_rules(
        &mut self,
        rules: &[Rule],
        context: &Context<'_>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<u32, RuleError> {
        crate::schedule::fixed_point(
            self,
            rules,
            ROUND_CEILING,
            |module, rule| module.apply_rule(rule, context, budget),
            |module, round| {
                if cfg!(any(test, debug_assertions)) {
                    verify::check(module, &mut AllocationBudget::new(None))
                        .map_err(|error| RuleError::Bug(format!("round {round}: {error}")))?;
                }
                Ok(())
            },
            |rounds| {
                RuleError::Bug(format!(
                    "the JavaScript target rules did not reach a fixed point in {rounds} rounds"
                ))
            },
        )
    }

    /// Apply one rule; returns whether it changed the tree.
    fn apply_rule(
        &mut self,
        rule: Rule,
        context: &Context<'_>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, RuleError> {
        #[cfg(any(test, debug_assertions))]
        let before = (self.clone(), self.measure());
        self.open_journal();
        let result = self.run_rule(rule, context, budget);
        let journal = self.take_journal();
        result?;
        #[cfg(any(test, debug_assertions))]
        {
            let (before, measure) = before;
            self.check_journal(&before, &journal)
                .map_err(|error| RuleError::Bug(format!("{rule:?}: {error}")))?;
            // A classified rule's edits strictly decrease the measure.
            if rule.transitional().is_none() && journal.edits() > 0 && self.measure() >= measure {
                return Err(RuleError::Bug(format!(
                    "{rule:?} edited without decreasing the measure: {measure:?} to {:?}",
                    self.measure()
                )));
            }
        }
        Ok(journal.edits() > 0)
    }

    /// The scheduler's measure (architecture §8.2): the reachable functions,
    /// then the reachable nodes (statements and expression nodes).
    #[cfg(any(test, debug_assertions))]
    fn measure(&self) -> (usize, usize) {
        let mut functions = 0;
        let mut nodes = 0;
        let mut reached = vec![false; self.regions.len()];
        let mut regions = vec![self.root];
        let mut pending = Vec::new();
        while let Some(region) = regions.pop() {
            if std::mem::replace(&mut reached[region.index()], true) {
                continue;
            }
            for statement in &self.regions[region.index()].statements {
                nodes += 1;
                statement.visit_regions(|child| regions.push(child));
                if let Statement::Function { function, .. } = statement {
                    functions += 1;
                    regions.push(self.functions[function.index()].body);
                }
                statement.visit_expressions(|root| pending.push(root));
                while let Some(id) = pending.pop() {
                    nodes += 1;
                    let node = &self.expressions[id.index()];
                    for function in node.created_functions() {
                        functions += 1;
                        regions.push(self.functions[function.index()].body);
                    }
                    let _ = node.visit_children(|child| {
                        pending.push(child);
                        Ok::<(), ()>(())
                    });
                }
            }
        }
        (functions, nodes)
    }

    fn run_rule(
        &mut self,
        rule: Rule,
        context: &Context<'_>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        let Context {
            frames_hidden,
            strict,
            pristine,
            prunes,
            numeric_lengths,
            year,
            statements,
            choices,
        } = *context;
        // The passes report what they did; the journal is what counts.
        match rule {
            Rule::SelfMethodCalls => {
                let _ = self.self_method_calls(budget)?;
            }
            Rule::SimplifyOperators => {
                let _ = self.simplify_operators(numeric_lengths, year, budget)?;
            }
            Rule::InlineExpressionFunctions => {
                let _ =
                    self.inline_expression_functions(INLINE_LIMIT, frames_hidden, strict, budget)?;
            }
            Rule::InlineStatementFunctions => {
                let _ = self.inline_statement_functions(frames_hidden, strict, budget)?;
            }
            Rule::EliminateAliases => {
                let _ = self.eliminate_aliases(budget)?;
            }
            Rule::FoldLiteralOperations => {
                let _ = self.fold_literal_operations(budget)?;
            }
            Rule::ForwardRootConstants => {
                let _ = self.forward_root_constants(ConstantKind::Scalar, budget)?;
            }
            Rule::ForwardRootStrings => {
                let _ = self.forward_root_constants(ConstantKind::String, budget)?;
            }
            Rule::ForwardSingleUses => {
                let _ = self.forward_single_uses(budget)?;
            }
            Rule::ElideUndefined => {
                let _ = self.elide_undefined(budget)?;
            }
            Rule::MergeDeclarations => {
                let _ = self.merge_declarations(prunes, budget)?;
            }
            Rule::FoldLogicalAssignments => {
                let _ = self.fold_logical_assignments(year >= 2020, budget)?;
            }
            Rule::FoldLogicalReturns => {
                let _ = self.fold_logical_returns(year >= 2020, budget)?;
            }
            Rule::FlattenConstantObjects => {
                let _ = self.flatten_constant_objects(budget)?;
            }
            Rule::UnobserveCalledNames => {
                let _ = self.unobserve_called_names(budget)?;
            }
            Rule::DropTypedDefaultChecks => {
                let _ = self.drop_typed_default_checks(budget)?;
            }
            Rule::InlineInitializers => {
                let _ = self.inline_initializers(budget)?;
            }
            // Without pristine builtins only stores to keys a literal
            // already has fold.
            Rule::FoldObjectStores => {
                let _ = self.fold_object_stores(pristine, budget)?;
            }
            Rule::DropDoubleNegations => {
                let _ = self.drop_double_negations(budget)?;
            }
            Rule::InlineSingleCalls => {
                let _ = self.inline_single_calls(frames_hidden, strict, budget)?;
            }
            Rule::FlattenBlocks => {
                let _ = self.flatten_blocks(budget)?;
            }
            Rule::CompressStatements => {
                let _ = self.compress_statements(statements, budget)?;
            }
            Rule::PlaceSingleCalls => {
                let _ = self.place_single_calls(frames_hidden, strict, budget)?;
            }
            Rule::DropRedundantInitStores => {
                let _ = self.drop_redundant_init_stores(budget)?;
            }
            Rule::ScalarizeMemberObjects => {
                let _ = self.scalarize_member_objects(budget)?;
            }
            Rule::DropUnreferencedFunctions => {
                let _ = self.drop_unreferenced_functions(budget)?;
            }
            Rule::DropUnreachable => {
                let _ = self.drop_unreachable(budget)?;
            }
            Rule::GroupPrototypeStores => {
                let _ = self.group_prototype_stores(budget)?;
            }
            Rule::JoinEmptyDeclarations => {
                let _ = self.join_empty_declarations(budget)?;
            }
            Rule::DropBareBlocks => {
                let _ = self.drop_bare_blocks(budget)?;
            }
            Rule::TruthyNullTests => {
                let _ = self.truthy_null_tests(budget)?;
            }
            Rule::ArrayReceiverCalls => {
                let _ = self.array_receiver_calls(budget)?;
            }
            Rule::DropDefaultArguments => {
                let _ = self.drop_default_arguments(budget)?;
            }
            Rule::NativeDefaultLengths => {
                let _ = self.native_default_lengths(budget)?;
            }
            Rule::PackStringArrays => {
                let _ = self.pack_string_arrays(budget)?;
            }
            Rule::PoolStrings => {
                let _ = self.pool_strings(budget)?;
            }
            Rule::EncodeTables => {
                if let Some(choices) = choices {
                    let _ = self.encode_tables(choices, budget)?;
                }
            }
        }
        Ok(())
    }
}
