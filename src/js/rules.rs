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
//! Target normalization consumes current physical storage and operation facts,
//! including helpers created after source optimization. Its retained rules have
//! explicit progress measures below. Registered representation choices and
//! bounded inlining can expand a tree; their producer bounds and the scheduler's
//! fail-closed round ceiling apply instead of a false node-decrease claim.
//!
//! Test and debug builds check every rule's journal against the actual
//! difference (`journal.rs`) and verify the tree after every round.

use super::constants::ConstantKind;
use super::*;

// The enum and its static telemetry buckets share one declaration, so adding
// or reordering a rule cannot attribute its time to another rule's counter.
macro_rules! declare_rules {
    ($($rule:ident),+ $(,)?) => {
        /// One rule of the JavaScript target.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum Rule { $($rule),+ }
        static RULE_TIMINGS: [crate::timing::Bucket; [$(stringify!($rule)),+].len()] = [
            $(crate::timing::Bucket::new(concat!("js_rule_", stringify!($rule)))),+
        ];
    };
}

declare_rules! {
    SelfMethodCalls,
    SimplifyOperators,
    InlineExpressionFunctions,
    DuplicateExpressionFunctions,
    PrivateCallRepresentations,
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
    InlineInitializers,
    FoldObjectStores,
    DropDoubleNegations,
    InlineSingleCalls,
    FlattenBlocks,
    CompressStatements,
    PlaceSingleCalls,
    DropRedundantInitStores,
    ScalarizeMemberObjects,
    PruneDeclarations,
    DropUnreachable,
    GroupPrototypeStores,
    JoinEmptyDeclarations,
    DropBareBlocks,
    TruthyNullTests,
    ArrayReceiverCalls,
    EncodeTables,
    PackStringArrays,
    PoolStrings,
}

pub(crate) fn append_rule_timing(out: &mut String) {
    STABLE_RULE_REUSE.append_snapshot(out);
    for bucket in &RULE_TIMINGS {
        bucket.append_snapshot(out);
    }
}

impl Rule {
    /// The owning permission for every scheduled rule. Target-only spelling
    /// and normalization rules use the umbrella permission; semantic work
    /// additionally uses its independent switch, just as it does in the IR.
    fn permitted(self, context: &Context<'_>) -> bool {
        match self {
            Self::InlineExpressionFunctions
            | Self::DuplicateExpressionFunctions
            | Self::InlineStatementFunctions
            | Self::InlineSingleCalls
            | Self::PlaceSingleCalls
            | Self::InlineInitializers => context.rules.inlining,
            Self::FlattenConstantObjects
            | Self::ScalarizeMemberObjects
            | Self::FoldObjectStores
            | Self::DropRedundantInitStores => context.rules.scalar_replacement,
            Self::FoldLiteralOperations
            | Self::ForwardRootConstants
            | Self::EliminateAliases
            | Self::ForwardRootStrings
            | Self::ForwardSingleUses
            | Self::SimplifyOperators => context.rules.constant_folding,
            Self::PruneDeclarations | Self::DropUnreachable => context.prunes,
            Self::PrivateCallRepresentations => {
                context.rules.call_specialization || context.rules.helper_sharing
            }
            Self::EncodeTables => context.rules.data_encoding,
            Self::PackStringArrays => context.rules.array_packing != ArrayPacking::Disabled,
            Self::SelfMethodCalls
            | Self::ElideUndefined
            | Self::MergeDeclarations
            | Self::FoldLogicalAssignments
            | Self::FoldLogicalReturns
            | Self::UnobserveCalledNames
            | Self::DropDoubleNegations
            | Self::FlattenBlocks
            | Self::CompressStatements
            | Self::GroupPrototypeStores
            | Self::JoinEmptyDeclarations
            | Self::DropBareBlocks
            | Self::TruthyNullTests
            | Self::ArrayReceiverCalls
            | Self::PoolStrings => true,
        }
    }

    /// Legality stays with each producer. This table records its termination
    /// obligation independently from permissions and objective selection.
    fn progress(self) -> Progress {
        match self {
            // These rewrites remove syntax while retaining operand order.
            // Source behaviour stamps and physical initialization facts are
            // consumed by placement/folding, never guessed from printed code.
            Self::SelfMethodCalls | Self::ArrayReceiverCalls
            | Self::EliminateAliases | Self::ForwardSingleUses
            | Self::MergeDeclarations
            | Self::TruthyNullTests | Self::FoldLiteralOperations
            | Self::ElideUndefined | Self::DropUnreachable
            | Self::DropBareBlocks | Self::DropDoubleNegations
            | Self::FoldLogicalAssignments | Self::FoldLogicalReturns
            | Self::FlattenBlocks | Self::DropRedundantInitStores | Self::FoldObjectStores
            | Self::GroupPrototypeStores => Progress::Nodes,
            // A literal replaces a binding read without necessarily removing
            // a node. Pooling runs after this fixed point, so cannot reverse it.
            Self::ForwardRootConstants | Self::ForwardRootStrings => Progress::BindingReads,
            Self::SimplifyOperators => Progress::Operators,
            // Complete-use/storage witnesses remove physical observations.
            Self::FlattenConstantObjects => Progress::MemberReads,
            Self::ScalarizeMemberObjects => Progress::Objects,
            Self::InlineInitializers => Progress::Calls,
            Self::UnobserveCalledNames => Progress::Names,
            // Only an uninitialized declaration moves; its position decreases
            // and it crosses no other declaration or reference to the binding.
            Self::JoinEmptyDeclarations => Progress::DeclarationOrder,
            // Inlining admits bounded templates and nesting, forbids recursive
            // substitution, and never delivers a tree before fixed point.
            Self::InlineExpressionFunctions | Self::InlineStatementFunctions
            | Self::InlineSingleCalls | Self::PlaceSingleCalls => Progress::BoundedExpansion,
            // Q1 site assignments are one-way in each fresh candidate. They
            // may increase nodes; exact final bytes decide admission afterward.
            Self::DuplicateExpressionFunctions | Self::PrivateCallRepresentations
            | Self::CompressStatements => Progress::SelectedRepresentation,
            // One rooted use closure removes entire dead recursive groups.
            Self::PruneDeclarations => Progress::Nodes,
            Self::EncodeTables | Self::PackStringArrays | Self::PoolStrings => Progress::SelectedRepresentation,
        }
    }

}

#[derive(Clone, Copy)]
enum Progress {
    Nodes,
    BindingReads,
    Operators,
    MemberReads,
    Objects,
    Calls,
    Names,
    DeclarationOrder,
    BoundedExpansion,
    SelectedRepresentation,
}

impl Progress {
    fn measured(self) -> bool {
        !matches!(self, Self::BoundedExpansion | Self::SelectedRepresentation)
    }
}

/// What a build permits the rules and what its artifact chose.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Context<'a> {
    pub(crate) reuse_normalization: bool,
    pub(crate) rules: TargetRules,
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
    pub(crate) families: OutputFamilies,
}

impl Context<'_> {
    fn folds_literals(&self) -> bool {
        self.choices.is_none() || self.families.expression_inlining
            || self.families.call_specialization || self.choices.is_some_and(|choices| {
                choices.iter().any(|(key, alt)| alt != AltId(0) && matches!(key.family,
                    ChoiceFamily::ExpressionInlining | ChoiceFamily::ConstantArguments))
            })
    }
}

#[derive(Default)]
struct Change { edited: bool, domains: u8 }

/// Only rules with an explicit complete dependency mask and cold-work replay
/// participate. Cross-function rules conservatively run on every pending round.
struct StableRules {
    epochs: [u64; 5],
    seen: [Option<[u64; 5]>; RULE_TIMINGS.len()],
}
impl StableRules {
    fn new() -> Self { Self { epochs: [0; 5], seen: [None; RULE_TIMINGS.len()] } }
    fn stamp(&self, rule: Rule) -> Option<[u64; 5]> {
        let mask = match rule {
            Rule::FoldLiteralOperations => 1 | 16, // nodes and observed-literal tables
            Rule::ElideUndefined | Rule::DropUnreachable => 1 | 2 | 8,
            _ => return None,
        };
        Some(std::array::from_fn(|index| if mask & (1 << index) != 0 { self.epochs[index] } else { 0 }))
    }
    fn stable(&self, rule: Rule) -> bool {
        self.stamp(rule).is_some_and(|stamp| self.seen[rule as usize] == Some(stamp))
    }
    fn record(&mut self, rule: Rule, change: &Change) {
        for index in 0..5 {
            if change.domains & (1 << index) != 0 {
                if let Some(next) = self.epochs[index].checked_add(1) { self.epochs[index] = next; }
                else { self.epochs = [0; 5]; self.seen.fill(None); break; }
            }
        }
        self.seen[rule as usize] = if change.edited { None } else { self.stamp(rule) };
    }
}

static STABLE_RULE_REUSE: crate::timing::Bucket = crate::timing::Bucket::new("js_rule_reuse");

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
    // Layouts and inlining create fresh transports after the shared IR.
    // The common reachable binding owner supplies their current storage facts.
    Rule::EliminateAliases,
    Rule::FoldLiteralOperations,
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
    // Discover every permitted site, even when its family default retains
    // calls. Explicit site assignments and whole-family joint moves use the
    // same producers, cleanup and artifact admission.
    rules.extend([
        Rule::PrivateCallRepresentations,
        Rule::DuplicateExpressionFunctions,
        Rule::FoldLiteralOperations,
    ]);
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
        rules.push(Rule::PruneDeclarations);
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
    ]);
    rules
}

/// Repeated strings last, once no other rule reads a literal: packed arrays,
/// then root constants.
pub(crate) fn pooling(families: &OutputFamilies) -> impl Iterator<Item = Rule> {
    [
        (families.string_array_packing, Rule::PackStringArrays),
        (families.string_pooling, Rule::PoolStrings),
    ]
    .into_iter()
    .filter_map(|(selected, rule)| selected.then_some(rule))
}

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
        self.run_rules_from(rules, context, budget, false)
    }

    /// A fresh copy of a completed head has already settled the overlapping
    /// rules. These five read the same context in head and tail; printer flags,
    /// statement spellings and choice maps are not their inputs. The first
    /// actual tail edit invalidates this certificate for the whole tree.
    pub(crate) fn run_tail_rules(
        &mut self,
        rules: &[Rule],
        context: &Context<'_>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<u32, RuleError> {
        self.run_rules_from(rules, context, budget, crate::schedule::reuses_stability())
    }

    fn run_rules_from(
        &mut self,
        rules: &[Rule],
        context: &Context<'_>,
        budget: &mut AllocationBudget<'_>,
        mut head_unchanged: bool,
    ) -> Result<u32, RuleError> {
        let mut stable = StableRules::new();
        let last_edited = std::cell::Cell::new(None);
        #[cfg(any(test, debug_assertions))]
        if std::env::var_os("LILSCRIPT_DEBUG_VERIFY").is_some() {
            verify::check(self, &mut AllocationBudget::new(None))
                .map_err(|error| RuleError::Bug(format!("before target rules: {error}")))?;
        }
        crate::schedule::fixed_point(
            self,
            rules,
            ROUND_CEILING,
            |module, rule| {
                if head_unchanged
                    && matches!(
                        rule,
                        Rule::EliminateAliases
                            | Rule::ForwardSingleUses
                            | Rule::FoldObjectStores
                            | Rule::SimplifyOperators
                            | Rule::MergeDeclarations
                    )
                {
                    return Ok(false);
                }
                let reuse = context.reuse_normalization && crate::schedule::reuses_stability() && stable.stable(rule);
                let change = module.apply_rule(rule, context, budget, reuse)?;
                if change.edited {
                    last_edited.set(Some(rule));
                }
                stable.record(rule, &change);
                head_unchanged &= !change.edited;
                Ok(change.edited)
            },
            |module, round| {
                if cfg!(any(test, debug_assertions)) {
                    verify::check(module, &mut AllocationBudget::new(None))
                        .map_err(|error| RuleError::Bug(format!("round {round}: {error}")))?;
                }
                Ok(())
            },
            |rounds| {
                RuleError::Bug(format!(
                    "the JavaScript target rules did not reach a fixed point in {rounds} rounds; last changing rule: {:?}", last_edited.get()
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
        stable: bool,
    ) -> Result<Change, RuleError> {
        if !rule.permitted(context) {
            return Ok(Change::default());
        }
        let _timing = if stable { STABLE_RULE_REUSE.scope(0) } else { crate::timing::JS_RULE.scope(0) };
        let _rule_timing = (!stable).then(|| RULE_TIMINGS[rule as usize].scope(0));
        let lengths = [self.expressions.len(), self.regions.len(), self.bindings.len(), self.functions.len(), self.scopes.len()];
        #[cfg(any(test, debug_assertions))]
        let before = (self.clone(), self.measure(rule.progress()));
        self.open_journal_admitted(budget)?;
        let result = if stable { self.replay_stable_rule(rule, context, budget) } else { self.run_rule(rule, context, budget) };
        let journal = self.take_journal();
        let outcome = (|| {
            result?;
            #[cfg(any(test, debug_assertions))]
            {
                let (before, measure) = before;
                self.check_journal(&before, &journal)
                    .map_err(|error| RuleError::Bug(format!("{rule:?}: {error}")))?;
                if journal.edits() > 0 && std::env::var_os("LILSCRIPT_DEBUG_VERIFY").is_some() {
                    verify::check(self, &mut AllocationBudget::new(None))
                        .map_err(|error| RuleError::Bug(format!("{rule:?}: {error}")))?;
                }
                // Normalization removes nodes. Spelling choices are bounded,
                // one-way rewrites under the explicit per-site assignment.
                if rule.progress().measured()
                    && journal.edits() > 0
                    && self.measure(rule.progress()) >= measure
                {
                    return Err(RuleError::Bug(format!(
                        "{rule:?} edited without decreasing the measure: {measure:?} to {:?}",
                        self.measure(rule.progress())
                    )));
                }
            }
            let next = [self.expressions.len(), self.regions.len(), self.bindings.len(), self.functions.len(), self.scopes.len()];
            let mut domains = journal.changed_domains();
            for index in 0..5 { if lengths[index] != next[index] { domains |= 1 << index; } }
            Ok(Change { edited: journal.edits() > 0, domains })
        })();
        journal.release(budget)?;
        outcome
    }

    /// Same logical calls and journal admission as an unchanged cold pass.
    /// Replay preserves partial work-limit refusal as well as completed bills.
    fn replay_stable_rule(&self, rule: Rule, context: &Context<'_>, budget: &mut AllocationBudget<'_>) -> Result<(), AllocationError> {
        use crate::compilation_policy::WorkKind::Analysis;
        match rule {
            Rule::FoldLiteralOperations => {
                if context.folds_literals() { budget.work(Analysis, self.expressions.len() as u64)?; }
            }
            Rule::ElideUndefined | Rule::DropUnreachable => {
                if rule == Rule::DropUnreachable {
                    for region in &self.regions { budget.work(Analysis, 1 + region.statements.len() as u64)?; }
                }
                for region in &self.regions { budget.work(Analysis, 1 + region.statements.len() as u64)?; }
                budget.work(Analysis, self.functions.len() as u64)?;
            }
            _ => unreachable!("only classified stable rules can replay"),
        }
        Ok(())
    }

    /// Reachable progress only: dead arena nodes cannot hide an edit. Node
    /// normalization orders functions before nodes; other rules use the
    /// specific observation they retire, then nodes to break ties.
    #[cfg(any(test, debug_assertions))]
    fn measure(&self, progress: Progress) -> (usize, usize) {
        let mut functions = 0;
        let mut nodes = 0;
        let mut observations = 0;
        let mut reached = vec![false; self.regions.len()];
        let mut regions = vec![self.root];
        let mut pending = Vec::new();
        while let Some(region) = regions.pop() {
            if std::mem::replace(&mut reached[region.index()], true) {
                continue;
            }
            for (index, statement) in self.regions[region.index()].statements.iter().enumerate() {
                nodes += 1;
                if matches!(progress, Progress::DeclarationOrder) && matches!(statement, Statement::Let { value: None, .. }) {
                    observations += index;
                }
                statement.visit_regions(|child| regions.push(child));
                if let Statement::Function { function, .. } = statement {
                    functions += 1;
                    if matches!(progress, Progress::Names) && !matches!(self.functions[function.index()].name, FunctionName::Unobserved) { observations += 1; }
                    regions.push(self.functions[function.index()].body);
                }
                statement.visit_expressions(|root| pending.push(root));
                while let Some(id) = pending.pop() {
                    nodes += 1;
                    let node = &self.expressions[id.index()];
                    observations += usize::from(match progress {
                        Progress::BindingReads => matches!(node, Expr::Binding(_)),
                        Progress::Operators => matches!(node, Expr::Binary { op: Binary::StrictEqual | Binary::StrictNotEqual, .. }),
                        Progress::MemberReads => matches!(node, Expr::Member { .. }),
                        Progress::Objects => matches!(node, Expr::Object(_)),
                        Progress::Calls => matches!(node, Expr::Call { .. }),
                        _ => false,
                    });
                    for function in node.created_functions() {
                        functions += 1;
                        if matches!(progress, Progress::Names) && !matches!(self.functions[function.index()].name, FunctionName::Unobserved) { observations += 1; }
                        regions.push(self.functions[function.index()].body);
                    }
                    let _ = node.visit_children(|child| {
                        pending.push(child);
                        Ok::<(), ()>(())
                    });
                }
            }
        }
        if matches!(progress, Progress::Operators) { (nodes, observations) }
        else { (if matches!(progress, Progress::Nodes) { functions } else { observations }, nodes) }
    }

    fn run_rule(
        &mut self,
        rule: Rule,
        context: &Context<'_>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        let Context {
            reuse_normalization: _,
            rules,
            frames_hidden,
            strict,
            pristine,
            prunes,
            numeric_lengths,
            year,
            statements,
            choices,
            families,
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
                let _ = self.inline_expression_functions(0, frames_hidden, strict, budget)?;
            }
            Rule::DuplicateExpressionFunctions => {
                if let Some(choices) = choices {
                    let _ = self.inline_expression_functions_chosen(
                        256,
                        frames_hidden,
                        strict,
                        Some((choices, families.expression_inlining)),
                        budget,
                    )?;
                }
            }
            Rule::PrivateCallRepresentations => {
                if let Some(choices) = choices {
                    use super::private_calls::Mode;
                    for (mode, permitted, seed) in [
                        (
                            Mode::Specialize,
                            rules.call_specialization,
                            families.call_specialization,
                        ),
                        (Mode::Share, rules.helper_sharing, families.helper_sharing),
                        (
                            Mode::Parameterize,
                            rules.parameterized_helpers,
                            families.parameterized_helpers,
                        ),
                    ] {
                        if permitted {
                            self.private_calls(mode, frames_hidden, choices, seed, budget)?;
                        }
                    }
                }
            }
            Rule::InlineStatementFunctions => {
                let _ = self.inline_statement_functions(frames_hidden, strict, budget)?;
            }
            Rule::EliminateAliases => {
                let _ = self.eliminate_aliases(budget)?;
            }
            Rule::ForwardRootConstants => {
                let _ = self.forward_root_constants(ConstantKind::Scalar, budget)?;
            }
            Rule::FoldLiteralOperations => {
                if context.folds_literals() {
                    let _ = self.fold_literal_operations(budget)?;
                }
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
                let _ = self.compress_statements_chosen(
                    statements,
                    choices.filter(|_| rules.statement_spellings),
                    budget,
                )?;
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
            Rule::PruneDeclarations => {
                let _ = self.prune_declarations(budget)?;
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
            Rule::PackStringArrays => {
                let _ = self.pack_string_arrays(context.rules.array_packing, budget)?;
            }
            Rule::PoolStrings => {
                let _ = self.pool_strings(true, budget)?;
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

#[cfg(test)]
mod contract_tests {
    use super::*;

    fn local_context() -> Context<'static> {
        Context { reuse_normalization: true, rules: TargetRules::SEMANTIC, frames_hidden: true,
            strict: true, pristine: false, prunes: true, numeric_lengths: false, year: 2022,
            statements: StatementSpellings::NONE, choices: None, families: OutputFamilies::NONE }
    }

    #[test]
    fn q2_stable_target_rules_invalidate_only_with_complete_dependencies() {
        for rule in [Rule::FoldLiteralOperations, Rule::ElideUndefined, Rule::DropUnreachable] {
            for domain in 0..5 {
                let mut stable = StableRules::new();
                assert!(!stable.stable(rule));
                stable.record(rule, &Change::default());
                assert!(stable.stable(rule));
                stable.record(Rule::InlineSingleCalls, &Change { edited: true, domains: 1 << domain });
                let invalidated = match rule {
                    Rule::FoldLiteralOperations => matches!(domain, 0 | 4),
                    _ => matches!(domain, 0 | 1 | 3),
                };
                assert_eq!(stable.stable(rule), !invalidated, "{rule:?}, domain {domain}");
                assert!(!stable.stable(Rule::InlineSingleCalls));
            }
        }
        let mut module = Module::default();
        let value = module.expression(Expr::Literal(Literal::Number(1.0)), None);
        module.regions[0].statements.push(Statement::Evaluate(value));
        let mut budget = AllocationBudget::new(None);
        module.open_journal_admitted(&mut budget).unwrap();
        module.renumber(&mut budget).unwrap();
        assert_eq!(module.take_journal().changed_domains(), 31);
    }

    #[test]
    fn q2_stable_target_work_replay_matches_cold_success_and_partial_refusals() {
        use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};
        let mut source = Module::default();
        for value in 0..8 {
            let id = source.expression(Expr::Literal(Literal::Number(value as f64)), None);
            source.regions[0].statements.push(Statement::Evaluate(id));
        }
        for rule in [Rule::FoldLiteralOperations, Rule::ElideUndefined, Rule::DropUnreachable] {
            for work in 0..45 {
                for bytes in [1, 16, 1_000_000] {
                    let runs = [false, true].map(|stable| {
                        let mut module = source.clone();
                        let mut ledger = BudgetLedger::new(ResourceLimits::default(), BudgetPlan {
                            baseline_work: work, optional_work: 0, baseline_retained_bytes: 0, retained_bytes: bytes,
                        }).unwrap();
                        let result = {
                            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
                            module.apply_rule(rule, &local_context(), &mut budget, stable)
                        };
                        assert_eq!(ledger.retained_bytes(), 0);
                        assert_eq!(module, source);
                        (result.is_ok(), result.err().map(|error| format!("{error:?}")), ledger.work_used(WorkDomain::Baseline), ledger.peak_retained_bytes())
                    });
                    assert_eq!(runs[0], runs[1], "{rule:?}, work {work}, bytes {bytes}");
                }
            }
        }
    }

    #[test]
    fn q2_dirty_target_schedule_matches_disabled_reuse_after_later_region_edits() {
        let mut source = Module::default();
        let value = source.expression(Expr::Literal(Literal::Undefined), None);
        let binding = source.binding(Binding { source_symbol: None, scope: source.regions[0].scope,
            spelling: "value".into(), pinned: false, class: None, defined: false });
        source.regions[0].statements.push(Statement::Let { binding, value: Some(value) });
        let runs = [false, true].map(|reuse_normalization| {
            let mut module = source.clone();
            let context = Context { reuse_normalization, ..local_context() };
            let rounds = module.run_rules(&[Rule::FoldLiteralOperations, Rule::ElideUndefined], &context, &mut AllocationBudget::new(None)).unwrap();
            (module, rounds)
        });
        assert_eq!(runs[0], runs[1]);
        assert_eq!(runs[0].1, 2);
    }

    #[test]
    fn q2_rule_refusal_closes_and_releases_its_journal() {
        use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};
        let mut module = Module::default();
        for value in 0..64 {
            let expr = module.expression(Expr::Literal(Literal::Number(value as f64)), None);
            module.regions[0].statements.push(Statement::Evaluate(expr));
        }
        let mut ledger = BudgetLedger::new(ResourceLimits::default(), BudgetPlan {
            baseline_work: 20, optional_work: 0, baseline_retained_bytes: 0, retained_bytes: 1_000_000,
        }).unwrap();
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        budget.retain(AllocationClass::Retained, 64).unwrap();
        let context = Context { reuse_normalization: true, rules: TargetRules::SEMANTIC, frames_hidden: true, strict: true,
            pristine: false, prunes: true, numeric_lengths: false, year: 2022,
            statements: StatementSpellings::NONE, choices: None, families: OutputFamilies::NONE };
        assert!(module.apply_rule(Rule::PruneDeclarations, &context, &mut budget, false).is_err());
        assert_eq!(module.take_journal().edits(), 0);
        assert_eq!(budget.retained_bytes(AllocationClass::Retained), 64);
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
    }

    #[test]
    fn shared_host_results_guard_equal_node_operator_progress() {
        for (host, pristine, expected) in [("Object", true, Binary::Equal), ("Object", false, Binary::StrictEqual), ("Foreign", true, Binary::StrictEqual)] {
            let mut module = Module::default();
            module.pristine_builtins = pristine;
            let object = module.expression(Expr::Host(Host::new(host)), None);
            let method = module.expression(Expr::Member { object, property: Property::Named("hasOwn".into()) }, None);
            let input = module.expression(Expr::Host(Host::new("input")), None);
            let key = module.expression(Expr::Literal(Literal::String("key".into())), None);
            let left = module.expression(Expr::Call { callee: method, arguments: vec![input, key], invocation: Invocation::Reference }, None);
            let right = module.expression(Expr::Literal(Literal::Bool(true)), None);
            let comparison = module.expression(Expr::Binary { op: Binary::StrictEqual, left, right }, None);
            module.regions[module.root.index()].statements.push(Statement::Evaluate(comparison));
            let context = Context { reuse_normalization: true, rules: TargetRules::SEMANTIC, frames_hidden: true, strict: true,
                pristine, prunes: false, numeric_lengths: false, year: 2022,
                statements: StatementSpellings::NONE, choices: None, families: OutputFamilies::NONE };
            module.run_rules(&[Rule::SimplifyOperators], &context, &mut AllocationBudget::new(None)).unwrap();
            assert!(matches!(module.expressions[comparison.index()], Expr::Binary { op, .. } if op == expected));
        }
    }
}
