use std::cell::RefCell;
use std::fmt::Write;
use std::path::Path;
use std::sync::Arc;

use ahash::{AHashMap, AHashSet};
use serde::Deserialize;

use crate::codegen_js::CodegenError;
use crate::ir::{
    BlockId, ConstValue, ControlFlowFunction, ControlFlowInstruction, ControlFlowModule,
    ControlFlowOp, ControlShape, ExportBinding, FunctionId, FunctionKind, Intrinsic, IrBinaryOp,
    IrUnaryOp, TemplateOperand, Terminator, ValueId,
};
use crate::semantic::{EscapeState, SymbolId, Type};
use crate::typed_array::{classify_typed_array_intrinsic, TypedArrayIntrinsic, TypedArrayKind};
use crate::value_analysis::{analyze_integer_values, FunctionIntegerFacts, IntegerValueAnalysis};

pub fn emit_optimized_ir_js(module: &ControlFlowModule<'_>) -> Result<String, CodegenError> {
    emit_optimized_ir_js_with_options(module, &IrJsOptions::default())
}

pub fn emit_optimized_ir_js_module(module: &ControlFlowModule<'_>) -> Result<String, CodegenError> {
    emit_optimized_ir_js_module_with_options(module, &IrJsOptions::default())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IrJsOptions {
    pub mangle_identifiers: bool,
    pub mangle_properties: bool,
    pub mangle_exports: bool,
    pub public_aggregate_fields: bool,
    pub pool_strings: bool,
    pub pool_numeric_literals: bool,
    pub elide_safe_integer_coercions: bool,
    pub compact_boolean_literals: bool,
    pub elide_block_terminal_semicolons: bool,
    pub elide_new_parentheses: bool,
    pub elide_call_chain_parentheses: bool,
    pub inline_structured_closures: bool,
    pub struct_method_shorthand: bool,
    pub truthy_nullable_checks: bool,
    pub pack_string_arrays: bool,
    pub scalar_phi_copies: bool,
    pub phi_affinity_mode: PhiAffinityMode,
    pub control_flow_spelling: ControlFlowSpelling,
    pub state_machine_spelling: StateMachineSpelling,
    pub conditional_expressions: bool,
    pub comma_expressions: bool,
    pub update_loop_layout: bool,
    pub cross_scope_name_reuse: bool,
    pub local_name_reserve: usize,
    pub stable_local_names: bool,
    pub entropy_property_names: bool,
    pub function_layout: FunctionLayout,
    pub function_layout_exact_limit: usize,
    pub function_spelling: FunctionSpelling,
    pub public_function_arrows: bool,
    pub loop_spelling: LoopSpelling,
    pub mutation_spelling: MutationSpelling,
    pub identifier_alphabet: IdentifierAlphabet,
    pub string_quote: StringQuote,
}

impl Default for IrJsOptions {
    fn default() -> Self {
        Self {
            mangle_identifiers: true,
            mangle_properties: false,
            mangle_exports: false,
            public_aggregate_fields: true,
            pool_strings: true,
            pool_numeric_literals: false,
            elide_safe_integer_coercions: true,
            compact_boolean_literals: true,
            elide_block_terminal_semicolons: true,
            elide_new_parentheses: true,
            elide_call_chain_parentheses: true,
            inline_structured_closures: true,
            struct_method_shorthand: true,
            truthy_nullable_checks: true,
            pack_string_arrays: true,
            scalar_phi_copies: false,
            phi_affinity_mode: PhiAffinityMode::Grouped,
            control_flow_spelling: ControlFlowSpelling::Auto,
            state_machine_spelling: StateMachineSpelling::Switch,
            conditional_expressions: true,
            comma_expressions: false,
            update_loop_layout: true,
            cross_scope_name_reuse: true,
            local_name_reserve: 0,
            stable_local_names: false,
            entropy_property_names: true,
            function_layout: FunctionLayout::Source,
            function_layout_exact_limit: 13,
            function_spelling: FunctionSpelling::Arrow,
            public_function_arrows: false,
            loop_spelling: LoopSpelling::Auto,
            mutation_spelling: MutationSpelling::Assignment,
            identifier_alphabet: IdentifierAlphabet::canonical(),
            string_quote: StringQuote::Double,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FunctionLayout {
    #[default]
    Source,
    CompressionSimilarity,
    CompressionWindow(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FunctionSpelling {
    #[default]
    Arrow,
    Function,
}

const FUNCTION_LAYOUT_NGRAM_LIMIT: usize = 4_096;

fn compression_similarity_order(segments: &[String], exact_limit: usize) -> Vec<usize> {
    let count = segments.len();
    if count < 3 {
        return (0..count).collect();
    }
    let similarities = compression_similarities(segments);
    let source = (0..count).collect::<Vec<_>>();
    let candidate = if count <= exact_limit {
        maximum_similarity_path(&similarities)
    } else {
        greedy_similarity_path(&similarities)
    };
    if path_similarity(&candidate, &similarities) > path_similarity(&source, &similarities) {
        candidate
    } else {
        source
    }
}

fn compression_similarities(segments: &[String]) -> Vec<Vec<usize>> {
    let count = segments.len();
    let profiles = segments
        .iter()
        .map(|segment| function_ngram_profile(segment.as_bytes()))
        .collect::<Vec<_>>();
    let mut similarities = vec![vec![0_usize; count]; count];
    for left in 0..count {
        for right in left + 1..count {
            let (smaller, larger) = if profiles[left].len() <= profiles[right].len() {
                (&profiles[left], &profiles[right])
            } else {
                (&profiles[right], &profiles[left])
            };
            let score = smaller
                .iter()
                .map(|(ngram, occurrences)| {
                    usize::from((*occurrences).min(larger.get(ngram).copied().unwrap_or_default()))
                })
                .sum();
            similarities[left][right] = score;
            similarities[right][left] = score;
        }
    }
    similarities
}

/// Proposes an order that keeps similar function bodies inside the selected
/// compressor's backward-reference window. The exact compressor still scores
/// this proposal against source and adjacency-only layouts in `compiler.rs`.
fn compression_window_order(
    segments: &[String],
    window_bytes: usize,
    exact_limit: usize,
) -> Vec<usize> {
    let count = segments.len();
    if count < 3 || window_bytes == 0 {
        return (0..count).collect();
    }
    let similarities = compression_similarities(segments);
    let lengths = segments.iter().map(String::len).collect::<Vec<_>>();
    let source = (0..count).collect::<Vec<_>>();

    let mut strongest = None::<(usize, usize, usize)>;
    for left in 0..count {
        for right in left + 1..count {
            let score = window_pair_score(
                similarities[left][right],
                lengths[left] / 2 + lengths[right].div_ceil(2),
                window_bytes,
            );
            let candidate = (score, left, right);
            if strongest.is_none_or(|current| {
                candidate.0 > current.0
                    || (candidate.0 == current.0
                        && (candidate.1, candidate.2) < (current.1, current.2))
            }) {
                strongest = Some(candidate);
            }
        }
    }
    let Some((score, left, right)) = strongest else {
        return source;
    };
    if score == 0 {
        return source;
    }

    // Growing at either endpoint leaves every already-established distance
    // unchanged. This keeps the proposal bounded at O(functions^3) instead of
    // making large-module compilation itself a quadratic-assignment search.
    let mut path = vec![left, right];
    let mut remaining = (0..count)
        .filter(|node| *node != left && *node != right)
        .collect::<Vec<_>>();
    while !remaining.is_empty() {
        let mut best = None::<(usize, usize, bool)>;
        for candidate in &remaining {
            for prepend in [false, true] {
                let added = endpoint_window_score(
                    *candidate,
                    &path,
                    prepend,
                    &similarities,
                    &lengths,
                    window_bytes,
                );
                let choice = (added, *candidate, prepend);
                if best.is_none_or(|current| {
                    choice.0 > current.0
                        || (choice.0 == current.0 && (choice.1, choice.2) < (current.1, current.2))
                }) {
                    best = Some(choice);
                }
            }
        }
        let (_, candidate, prepend) = best.expect("a remaining function has an endpoint");
        if prepend {
            path.insert(0, candidate);
        } else {
            path.push(candidate);
        }
        remaining.retain(|node| *node != candidate);
    }

    let adjacency = if count <= exact_limit {
        maximum_similarity_path(&similarities)
    } else {
        greedy_similarity_path(&similarities)
    };
    [source, adjacency, path]
        .into_iter()
        .max_by(|left, right| {
            window_path_score(left, &similarities, &lengths, window_bytes)
                .cmp(&window_path_score(
                    right,
                    &similarities,
                    &lengths,
                    window_bytes,
                ))
                .then_with(|| right.cmp(left))
        })
        .unwrap_or_default()
}

fn endpoint_window_score(
    candidate: usize,
    path: &[usize],
    prepend: bool,
    similarities: &[Vec<usize>],
    lengths: &[usize],
    window_bytes: usize,
) -> usize {
    let mut gap = 0_usize;
    let mut score = 0_usize;
    let mut add = |other: usize| {
        let distance = lengths[candidate] / 2 + gap + lengths[other].div_ceil(2);
        if distance > window_bytes {
            return false;
        }
        score = score.saturating_add(window_pair_score(
            similarities[candidate][other],
            distance,
            window_bytes,
        ));
        gap = gap.saturating_add(lengths[other]);
        true
    };
    if prepend {
        for other in path {
            if !add(*other) {
                break;
            }
        }
    } else {
        for other in path.iter().rev() {
            if !add(*other) {
                break;
            }
        }
    }
    score
}

fn window_path_score(
    path: &[usize],
    similarities: &[Vec<usize>],
    lengths: &[usize],
    window_bytes: usize,
) -> usize {
    let mut offsets = Vec::with_capacity(path.len());
    let mut offset = 0_usize;
    for node in path {
        offsets.push(offset.saturating_add(lengths[*node] / 2));
        offset = offset.saturating_add(lengths[*node]);
    }
    let mut score = 0_usize;
    for right in 0..path.len() {
        for left in (0..right).rev() {
            let distance = offsets[right].saturating_sub(offsets[left]);
            if distance > window_bytes {
                break;
            }
            score = score.saturating_add(window_pair_score(
                similarities[path[left]][path[right]],
                distance,
                window_bytes,
            ));
        }
    }
    score
}

fn window_pair_score(similarity: usize, distance: usize, window_bytes: usize) -> usize {
    if similarity == 0 || distance > window_bytes {
        return 0;
    }
    let distance_bits = usize::BITS - distance.max(1).leading_zeros();
    let window_bits = usize::BITS - window_bytes.max(1).leading_zeros();
    similarity.saturating_mul(1 + window_bits.saturating_sub(distance_bits) as usize)
}

fn function_ngram_profile(bytes: &[u8]) -> AHashMap<u64, u16> {
    let mut profile = AHashMap::new();
    for window in bytes.windows(8) {
        let ngram = u64::from_le_bytes(window.try_into().expect("an eight-byte window"));
        if profile.len() < FUNCTION_LAYOUT_NGRAM_LIMIT || profile.contains_key(&ngram) {
            let occurrences = profile.entry(ngram).or_insert(0_u16);
            *occurrences = occurrences.saturating_add(1);
        }
    }
    profile
}

fn maximum_similarity_path(similarities: &[Vec<usize>]) -> Vec<usize> {
    let count = similarities.len();
    let state_count = 1_usize << count;
    let mut scores = vec![None::<usize>; state_count * count];
    let mut parents = vec![usize::MAX; state_count * count];
    for node in 0..count {
        scores[((1_usize << node) * count) + node] = Some(0);
    }
    for mask in 1_usize..state_count {
        for (last, similarity_row) in similarities.iter().enumerate() {
            let index = mask * count + last;
            let Some(score) = scores[index] else {
                continue;
            };
            for (next, similarity) in similarity_row.iter().copied().enumerate() {
                let bit = 1_usize << next;
                if mask & bit != 0 {
                    continue;
                }
                let next_index = (mask | bit) * count + next;
                let next_score = score.saturating_add(similarity);
                if scores[next_index].is_none_or(|current| next_score > current)
                    || (scores[next_index] == Some(next_score) && last < parents[next_index])
                {
                    scores[next_index] = Some(next_score);
                    parents[next_index] = last;
                }
            }
        }
    }

    let full = state_count - 1;
    let mut last = (0..count)
        .max_by(|left, right| {
            scores[full * count + *left]
                .cmp(&scores[full * count + *right])
                .then_with(|| right.cmp(left))
        })
        .unwrap_or_default();
    let mut mask = full;
    let mut path = Vec::with_capacity(count);
    loop {
        path.push(last);
        let parent = parents[mask * count + last];
        mask &= !(1_usize << last);
        if parent == usize::MAX {
            break;
        }
        last = parent;
    }
    path.reverse();
    path
}

fn greedy_similarity_path(similarities: &[Vec<usize>]) -> Vec<usize> {
    let count = similarities.len();
    let mut strongest = (0_usize, 1_usize, similarities[0][1]);
    for (left, similarity_row) in similarities.iter().enumerate() {
        for (right, similarity) in similarity_row.iter().copied().enumerate().skip(left + 1) {
            if similarity > strongest.2 {
                strongest = (left, right, similarity);
            }
        }
    }
    let mut path = vec![strongest.0, strongest.1];
    let mut remaining = (0..count)
        .filter(|node| !path.contains(node))
        .collect::<Vec<_>>();
    while !remaining.is_empty() {
        let mut best = (i128::MIN, usize::MAX, usize::MAX);
        for candidate in &remaining {
            for position in 0..=path.len() {
                let added = if position == 0 {
                    similarities[*candidate][path[0]] as i128
                } else if position == path.len() {
                    similarities[path[position - 1]][*candidate] as i128
                } else {
                    similarities[path[position - 1]][*candidate] as i128
                        + similarities[*candidate][path[position]] as i128
                        - similarities[path[position - 1]][path[position]] as i128
                };
                let choice = (added, *candidate, position);
                if choice.0 > best.0
                    || (choice.0 == best.0 && (choice.1, choice.2) < (best.1, best.2))
                {
                    best = choice;
                }
            }
        }
        path.insert(best.2, best.1);
        remaining.retain(|candidate| *candidate != best.1);
    }
    improve_similarity_path(path, similarities)
}

/// Deterministic 2-opt refinement for the bounded large-module layout path.
/// Reversing a segment preserves all of its internal undirected similarities,
/// so each proposal needs to score only the two boundary edges. The insertion
/// pass is not necessarily locally optimal after later nodes are added; this
/// closes that gap without the exponential memory of Held-Karp.
fn improve_similarity_path(mut path: Vec<usize>, similarities: &[Vec<usize>]) -> Vec<usize> {
    loop {
        let mut best = None::<(usize, usize, usize)>;
        for left in 0..path.len() {
            for right in left + 1..path.len() {
                if left == 0 && right + 1 == path.len() {
                    continue;
                }
                let mut removed = 0_usize;
                let mut added = 0_usize;
                if left > 0 {
                    removed = removed.saturating_add(similarities[path[left - 1]][path[left]]);
                    added = added.saturating_add(similarities[path[left - 1]][path[right]]);
                }
                if right + 1 < path.len() {
                    removed = removed.saturating_add(similarities[path[right]][path[right + 1]]);
                    added = added.saturating_add(similarities[path[left]][path[right + 1]]);
                }
                let gain = added.saturating_sub(removed);
                if gain != 0
                    && best.is_none_or(|current| {
                        gain > current.0
                            || (gain == current.0 && (left, right) < (current.1, current.2))
                    })
                {
                    best = Some((gain, left, right));
                }
            }
        }
        let Some((_, left, right)) = best else {
            return path;
        };
        path[left..=right].reverse();
    }
}

fn path_similarity(path: &[usize], similarities: &[Vec<usize>]) -> usize {
    path.windows(2)
        .map(|edge| similarities[edge[0]][edge[1]])
        .sum()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StringQuote {
    #[default]
    Double,
    Single,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PhiAffinityMode {
    Conservative,
    Direct,
    #[default]
    Grouped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoopSpelling {
    #[default]
    Auto,
    While,
    For,
    Do,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MutationSpelling {
    #[default]
    Assignment,
    Prefix,
    Postfix,
    Compound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlFlowSpelling {
    #[default]
    Auto,
    Structured,
    StateMachine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateMachineSpelling {
    #[default]
    Switch,
    Conditional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum JsPrecedence {
    Comma,
    Assignment,
    Conditional,
    LogicalOr,
    LogicalAnd,
    BitOr,
    BitXor,
    BitAnd,
    Equality,
    Relational,
    Shift,
    Additive,
    Multiplicative,
    Unary,
    // `new C` cannot be chained like `new C()` without grouping the receiver.
    NewWithoutArgs,
    Call,
    Member,
    Primary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JsExpressionRoot {
    Atom,
    Unary(&'static str),
    Binary(IrBinaryOp),
    Conditional,
    Call,
    Member,
    IntegerNormalization,
    Raw,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct JsExpression {
    code: String,
    ungrouped: Option<String>,
    precedence: JsPrecedence,
    root: JsExpressionRoot,
    normalization_operand: Option<Box<Self>>,
    binary_operands: Option<(Box<Self>, Box<Self>)>,
    unary_operand: Option<Box<Self>>,
}

impl JsExpression {
    fn atom(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            ungrouped: None,
            precedence: JsPrecedence::Primary,
            root: JsExpressionRoot::Atom,
            normalization_operand: None,
            binary_operands: None,
            unary_operand: None,
        }
    }

    fn raw(code: impl Into<String>, precedence: JsPrecedence) -> Self {
        Self {
            code: code.into(),
            ungrouped: None,
            precedence,
            root: JsExpressionRoot::Raw,
            normalization_operand: None,
            binary_operands: None,
            unary_operand: None,
        }
    }

    fn grouped(ungrouped: String, precedence: JsPrecedence, root: JsExpressionRoot) -> Self {
        Self {
            code: format!("({ungrouped})"),
            ungrouped: Some(ungrouped),
            precedence,
            root,
            normalization_operand: None,
            binary_operands: None,
            unary_operand: None,
        }
    }

    fn unary(operator: &'static str, operand: Self) -> Self {
        let original_operand = Box::new(operand.clone());
        let token_collision = (operator == "-" && operand.code.starts_with('-'))
            || (operator == "+" && operand.code.starts_with('+'));
        let operand = if operand.precedence < JsPrecedence::Unary || token_collision {
            operand.grouped_code()
        } else {
            operand.code
        };
        Self {
            code: format!("{operator}{operand}"),
            ungrouped: None,
            precedence: JsPrecedence::Unary,
            root: JsExpressionRoot::Unary(operator),
            normalization_operand: None,
            binary_operands: None,
            unary_operand: Some(original_operand),
        }
    }

    fn binary(op: IrBinaryOp, lhs: Self, rhs: Self) -> Self {
        let operands = (Box::new(lhs.clone()), Box::new(rhs.clone()));
        let lhs = lhs.binary_operand(op, BinaryOperandSide::Left);
        let rhs = rhs.binary_operand(op, BinaryOperandSide::Right);
        let rhs = token_safe_binary_rhs(op, rhs);
        let mut expression = Self::grouped(
            format!("{lhs}{}{rhs}", binary_operator(op)),
            js_binary_precedence(op),
            JsExpressionRoot::Binary(op),
        );
        expression.binary_operands = Some(operands);
        expression
    }

    fn conditional(condition: Self, then_value: Self, else_value: Self) -> Self {
        let condition = condition.at_least(JsPrecedence::LogicalOr);
        let then_value = then_value.at_least(JsPrecedence::Assignment);
        let else_value = else_value.at_least(JsPrecedence::Assignment);
        Self::grouped(
            format!("{condition}?{then_value}:{else_value}"),
            JsPrecedence::Conditional,
            JsExpressionRoot::Conditional,
        )
    }

    fn comma(expressions: impl IntoIterator<Item = Self>) -> Self {
        let code = expressions
            .into_iter()
            .map(|expression| expression.at_least(JsPrecedence::Assignment))
            .collect::<Vec<_>>()
            .join(",");
        Self::grouped(code, JsPrecedence::Comma, JsExpressionRoot::Raw)
    }

    fn call(callee: Self, args: impl IntoIterator<Item = Self>) -> Self {
        let mut code = callee.at_least(JsPrecedence::Call);
        code.push('(');
        for (index, argument) in args.into_iter().enumerate() {
            if index != 0 {
                code.push(',');
            }
            code.push_str(&argument.at_least(JsPrecedence::Assignment));
        }
        code.push(')');
        Self {
            code,
            ungrouped: None,
            precedence: JsPrecedence::Call,
            root: JsExpressionRoot::Call,
            normalization_operand: None,
            binary_operands: None,
            unary_operand: None,
        }
    }

    fn member(object: Self, property: &str, elide_call_chain_parentheses: bool) -> Self {
        let object = if object.precedence == JsPrecedence::Primary
            && object
                .ungrouped
                .as_deref()
                .unwrap_or(&object.code)
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_digit)
        {
            object.grouped_code()
        } else {
            object.at_least(if elide_call_chain_parentheses {
                JsPrecedence::Call
            } else {
                JsPrecedence::Member
            })
        };
        Self {
            code: format!("{object}.{property}"),
            ungrouped: None,
            precedence: JsPrecedence::Member,
            root: JsExpressionRoot::Member,
            normalization_operand: None,
            binary_operands: None,
            unary_operand: None,
        }
    }

    fn index(object: Self, index: Self, elide_call_chain_parentheses: bool) -> Self {
        Self {
            code: format!(
                "{}[{}]",
                object.at_least(if elide_call_chain_parentheses {
                    JsPrecedence::Call
                } else {
                    JsPrecedence::Member
                }),
                index.at_least(JsPrecedence::Assignment)
            ),
            ungrouped: None,
            precedence: JsPrecedence::Member,
            root: JsExpressionRoot::Member,
            normalization_operand: None,
            binary_operands: None,
            unary_operand: None,
        }
    }

    fn integer_normalization(value: Self) -> Self {
        let rendered = value
            .clone()
            .binary_operand(IrBinaryOp::BitOr, BinaryOperandSide::Left);
        let mut normalized = Self::grouped(
            format!("{rendered}|0"),
            JsPrecedence::BitOr,
            JsExpressionRoot::IntegerNormalization,
        );
        normalized.normalization_operand = Some(Box::new(value));
        normalized
    }

    fn at_least(self, minimum: JsPrecedence) -> String {
        if self.precedence < minimum {
            self.grouped_code()
        } else {
            self.into_minimal()
        }
    }

    fn grouped_code(self) -> String {
        if self.code.starts_with('(') && self.code.ends_with(')') {
            self.code
        } else {
            format!("({})", self.code)
        }
    }

    fn into_minimal(self) -> String {
        self.ungrouped.unwrap_or(self.code)
    }

    /// A branch/loop already applies JavaScript's ToBoolean operation. Keep
    /// explicit negation, but do not spell a redundant double negation at the
    /// point where the value is consumed as a condition.
    fn into_condition(self) -> String {
        if self.root == JsExpressionRoot::Unary("!") {
            if let Some(operand) = self.unary_operand.as_deref() {
                if operand.root == JsExpressionRoot::Unary("!") {
                    return operand
                        .unary_operand
                        .as_deref()
                        .cloned()
                        .map(JsExpression::into_minimal)
                        .expect("unary expressions carry their operand");
                }
            }
        }
        self.into_minimal()
    }

    fn negated(self) -> String {
        if is_true_literal(&self.code) {
            return "!1".to_string();
        }
        if is_false_literal(&self.code) {
            return "!0".to_string();
        }
        if self.root == JsExpressionRoot::Unary("!") {
            return self
                .unary_operand
                .map(|operand| operand.into_minimal())
                .expect("unary expressions carry their operand");
        }
        if let JsExpressionRoot::Binary(operator) = self.root {
            if let Some(inverse) = inverse_comparison(operator) {
                if let Some((lhs, rhs)) = self.binary_operands {
                    return Self::binary(inverse, *lhs, *rhs).into_minimal();
                }
            }
        }
        Self::unary("!", self).into_minimal()
    }

    fn without_integer_normalization(self) -> Self {
        if self.root != JsExpressionRoot::IntegerNormalization {
            return self;
        }
        self.normalization_operand
            .map(|operand| *operand)
            .expect("integer normalization expressions carry their operand")
    }

    fn is_integer_normalization(&self) -> bool {
        self.root == JsExpressionRoot::IntegerNormalization
    }

    fn binary_operand(self, parent: IrBinaryOp, side: BinaryOperandSide) -> String {
        let can_unwrap = match self.root {
            JsExpressionRoot::Binary(child) => {
                let child_precedence = js_binary_precedence(child);
                let parent_precedence = js_binary_precedence(parent);
                child_precedence > parent_precedence
                    || (child_precedence == parent_precedence
                        && match side {
                            BinaryOperandSide::Left => true,
                            BinaryOperandSide::Right => {
                                child == parent
                                    && matches!(
                                        parent,
                                        IrBinaryOp::BitAnd
                                            | IrBinaryOp::BitOr
                                            | IrBinaryOp::Xor
                                            | IrBinaryOp::And
                                            | IrBinaryOp::Or
                                    )
                            }
                        })
            }
            _ => self.precedence > js_binary_precedence(parent),
        };
        if can_unwrap {
            self.into_minimal()
        } else if self.precedence < js_binary_precedence(parent) {
            self.grouped_code()
        } else {
            self.code
        }
    }
}

impl std::fmt::Display for JsExpression {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.code)
    }
}

impl std::ops::Deref for JsExpression {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.code
    }
}

type ExpressionCache = AHashMap<ValueId, JsExpression>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdentifierAlphabet {
    first: [u8; 54],
    rest: [u8; 64],
}

impl IdentifierAlphabet {
    pub const fn canonical() -> Self {
        Self {
            first: *b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_$",
            rest: *b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_$0123456789",
        }
    }

    pub fn for_code(code: &str) -> Self {
        let canonical = Self::canonical();
        let mut counts = [0usize; 128];
        for byte in code.bytes().filter(|byte| byte.is_ascii()) {
            counts[byte as usize] += 1;
        }
        let mut alphabet = canonical;
        alphabet.first.sort_unstable_by(|left, right| {
            counts[*right as usize]
                .cmp(&counts[*left as usize])
                .then_with(|| {
                    canonical_rank(*left, &canonical.first)
                        .cmp(&canonical_rank(*right, &canonical.first))
                })
        });
        alphabet.rest.sort_unstable_by(|left, right| {
            counts[*right as usize]
                .cmp(&counts[*left as usize])
                .then_with(|| {
                    canonical_rank(*left, &canonical.rest)
                        .cmp(&canonical_rank(*right, &canonical.rest))
                })
        });
        alphabet
    }

    pub fn remapped(self, mapping: &[u8; 128]) -> Self {
        let mut remapped = self;
        for byte in &mut remapped.first {
            *byte = mapping[*byte as usize];
        }
        for byte in &mut remapped.rest {
            *byte = mapping[*byte as usize];
        }
        remapped
    }
}

impl Default for IdentifierAlphabet {
    fn default() -> Self {
        Self::canonical()
    }
}

fn canonical_rank(byte: u8, alphabet: &[u8]) -> usize {
    alphabet
        .iter()
        .position(|candidate| *candidate == byte)
        .unwrap_or(usize::MAX)
}

pub fn emit_optimized_ir_js_with_options(
    module: &ControlFlowModule<'_>,
    options: &IrJsOptions,
) -> Result<String, CodegenError> {
    IrJsEmitter::new(module, false, *options).emit()
}

pub(crate) fn emit_optimized_ir_js_with_options_and_analysis(
    module: &ControlFlowModule<'_>,
    options: &IrJsOptions,
    integer_analysis: Arc<IntegerValueAnalysis>,
) -> Result<String, CodegenError> {
    IrJsEmitter::with_integer_analysis(module, false, *options, integer_analysis).emit()
}

pub fn emit_optimized_ir_js_module_with_options(
    module: &ControlFlowModule<'_>,
    options: &IrJsOptions,
) -> Result<String, CodegenError> {
    IrJsEmitter::new(module, true, *options).emit()
}

pub(crate) fn emit_optimized_ir_js_module_with_options_and_analysis(
    module: &ControlFlowModule<'_>,
    options: &IrJsOptions,
    integer_analysis: Arc<IntegerValueAnalysis>,
) -> Result<String, CodegenError> {
    IrJsEmitter::with_integer_analysis(module, true, *options, integer_analysis).emit()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrJsChunkSpec {
    pub file_name: String,
    pub functions: Vec<FunctionId>,
    pub lazy_module: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrJsChunkPlan {
    pub entry_file: String,
    pub chunks: Vec<IrJsChunkSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrJsChunk {
    pub file_name: String,
    pub code: String,
    pub dependencies: Vec<String>,
    pub dynamic_dependencies: Vec<String>,
}

pub fn emit_optimized_ir_js_chunks_with_options(
    module: &ControlFlowModule<'_>,
    options: &IrJsOptions,
    plan: &IrJsChunkPlan,
) -> Result<Vec<IrJsChunk>, CodegenError> {
    IrJsEmitter::new(module, true, *options).emit_chunks(plan)
}

pub fn ir_function_can_move_to_chunk(module: &ControlFlowModule<'_>, function: FunctionId) -> bool {
    module
        .functions
        .get(function.0 as usize)
        .is_some_and(|function| is_emitted_function(function, true))
        && !function_writes_global(module, function, &mut AHashSet::new(), true)
}

struct IrJsEmitter<'module, 'src> {
    module: &'module ControlFlowModule<'src>,
    integer_analysis: Arc<IntegerValueAnalysis>,
    global_names: AHashMap<SymbolId, String>,
    external_export_aliases: AHashMap<SymbolId, String>,
    function_names: AHashMap<FunctionId, String>,
    top_level_mangler: Mangler,
    local_name_reservations: Vec<String>,
    preferred_local_names: AHashMap<String, String>,
    declared_globals: AHashSet<SymbolId>,
    constant_global_strings: AHashMap<SymbolId, String>,
    deferred_global_declarations: AHashSet<SymbolId>,
    string_aliases: AHashMap<String, String>,
    pooled_strings: Vec<(String, String)>,
    numeric_aliases: AHashMap<String, String>,
    pooled_numbers: Vec<(String, String)>,
    property_names: AHashMap<String, String>,
    named_field_aggregates: AHashSet<String>,
    module_output: bool,
    options: IrJsOptions,
    dynamic_chunk_files: AHashMap<u32, String>,
}

impl<'module, 'src> IrJsEmitter<'module, 'src> {
    fn new(
        module: &'module ControlFlowModule<'src>,
        module_output: bool,
        options: IrJsOptions,
    ) -> Self {
        Self::with_integer_analysis(
            module,
            module_output,
            options,
            Arc::new(analyze_integer_values(module)),
        )
    }

    fn with_integer_analysis(
        module: &'module ControlFlowModule<'src>,
        module_output: bool,
        options: IrJsOptions,
        integer_analysis: Arc<IntegerValueAnalysis>,
    ) -> Self {
        Self {
            module,
            integer_analysis,
            global_names: AHashMap::new(),
            external_export_aliases: AHashMap::new(),
            function_names: AHashMap::new(),
            top_level_mangler: Mangler::new(options.identifier_alphabet),
            local_name_reservations: Vec::new(),
            preferred_local_names: AHashMap::new(),
            declared_globals: AHashSet::new(),
            constant_global_strings: AHashMap::new(),
            deferred_global_declarations: AHashSet::new(),
            string_aliases: AHashMap::new(),
            pooled_strings: Vec::new(),
            numeric_aliases: AHashMap::new(),
            pooled_numbers: Vec::new(),
            property_names: AHashMap::new(),
            named_field_aggregates: AHashSet::new(),
            module_output,
            options,
            dynamic_chunk_files: AHashMap::new(),
        }
    }

    fn emit(mut self) -> Result<String, CodegenError> {
        self.prepare();
        let entry = self.function(self.module.entry)?.clone();
        let entry_is_single_block = entry.blocks.len() == 1 && entry.blocks[0].phis.is_empty();
        let entry_can_structure = can_structure(&entry);
        let mut out = String::new();

        if !self.pooled_strings.is_empty() || !self.pooled_numbers.is_empty() {
            out.push_str("let ");
            let mut index = 0;
            for (value, name) in &self.pooled_strings {
                if index != 0 {
                    out.push(',');
                }
                out.push_str(name);
                out.push('=');
                out.push_str(&render_string_literal(value, self.options.string_quote));
                index += 1;
            }
            for (value, name) in &self.pooled_numbers {
                if index != 0 {
                    out.push(',');
                }
                out.push_str(name);
                out.push('=');
                out.push_str(value);
                index += 1;
            }
            out.push(';');
        }

        let owned_globals = self
            .module
            .globals
            .iter()
            .filter(|global| !global.external)
            .collect::<Vec<_>>();
        let predeclared_globals = owned_globals
            .iter()
            .copied()
            .filter(|global| !self.deferred_global_declarations.contains(&global.symbol))
            .collect::<Vec<_>>();
        if !predeclared_globals.is_empty() {
            out.push_str("let ");
            for (index, global) in predeclared_globals.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                out.push_str(self.global_name(global.symbol)?);
                if let Some(value) = self.constant_global_strings.get(&global.symbol) {
                    out.push('=');
                    out.push_str(&render_string_literal(value, self.options.string_quote));
                }
            }
            out.push(';');
        }
        self.declared_globals
            .extend(owned_globals.iter().map(|global| global.symbol));
        self.emit_external_export_aliases(&mut out)?;

        let functions = self
            .module
            .functions
            .iter()
            .filter(|function| {
                function.live
                    && function.kind != FunctionKind::Entry
                    && function.kind != FunctionKind::Extern
                    && !(function.kind == FunctionKind::Closure
                        && can_inline_closure(function, self.options.inline_structured_closures))
            })
            .map(|function| function.id)
            .collect::<Vec<_>>();
        self.emit_function_group(&functions, &mut out)?;

        for global in &self.deferred_global_declarations {
            self.declared_globals.remove(global);
        }

        if entry_is_single_block {
            self.emit_single_block(&entry, false, &mut out)?;
        } else if entry_can_structure {
            self.emit_structured(&entry, false, &mut out)?;
        } else {
            out.push_str("(()=>");
            self.emit_state_machine(&entry, &mut out)?;
            out.push_str(")();");
        }
        if self.module_output {
            self.emit_exports(&mut out)?;
        }
        if self.options.elide_block_terminal_semicolons && out.ends_with(';') {
            out.pop();
        }
        Ok(out)
    }

    fn prepare(&mut self) {
        self.assign_top_level_names();
        self.assign_external_export_aliases();
        self.assign_constant_global_strings();
        self.assign_deferred_global_declarations();
        self.assign_string_aliases();
        self.assign_numeric_aliases();
        self.assign_named_field_aggregates();
        self.assign_property_names();
    }

    fn assign_named_field_aggregates(&mut self) {
        if !self.options.public_aggregate_fields {
            return;
        }
        let layouts = self
            .module
            .structs
            .iter()
            .chain(&self.module.classes)
            .collect::<Vec<_>>();
        let exports = self
            .module
            .exports
            .iter()
            .chain(
                self.module
                    .lazy_modules
                    .iter()
                    .flat_map(|module| module.exports.iter()),
            )
            .collect::<Vec<_>>();
        for layout in &layouts {
            let public = exports.iter().any(|export| match export.binding {
                ExportBinding::TypeOnly => export.name == layout.name,
                ExportBinding::Function(function) => {
                    self.module
                        .functions
                        .get(function.0 as usize)
                        .is_some_and(|function| {
                            type_references_class(&function.return_type, layout.name)
                                || function.params.iter().any(|parameter| {
                                    type_references_class(&parameter.ty, layout.name)
                                })
                        })
                }
                ExportBinding::Global(global) => self
                    .module
                    .globals
                    .iter()
                    .find(|candidate| candidate.symbol == global)
                    .is_some_and(|global| type_references_class(&global.ty, layout.name)),
            }) || self.module.functions.iter().any(|function| {
                function.params.iter().any(|parameter| {
                    function.value_escapes.get(parameter.value.0 as usize)
                        == Some(&EscapeState::EscapesToUntypedBoundary)
                        && type_references_class(&parameter.ty, layout.name)
                }) || function.blocks.iter().any(|block| {
                    block.phis.iter().any(|phi| {
                        function.value_escapes.get(phi.out.0 as usize)
                            == Some(&EscapeState::EscapesToUntypedBoundary)
                            && type_references_class(&phi.ty, layout.name)
                    }) || block.instructions.iter().any(|instruction| {
                        instruction.out.is_some_and(|out| {
                            function.value_escapes.get(out.0 as usize)
                                == Some(&EscapeState::EscapesToUntypedBoundary)
                        }) && instruction
                            .ty
                            .as_ref()
                            .is_some_and(|ty| type_references_class(ty, layout.name))
                    })
                })
            });
            if public {
                self.named_field_aggregates.insert(layout.name.to_string());
            }
        }
        loop {
            let mut changed = false;
            for layout in &layouts {
                if !self.named_field_aggregates.contains(layout.name) {
                    continue;
                }
                for candidate in &layouts {
                    if self.named_field_aggregates.contains(candidate.name) {
                        continue;
                    }
                    if layout
                        .fields
                        .iter()
                        .any(|field| type_references_class(&field.ty, candidate.name))
                    {
                        self.named_field_aggregates
                            .insert(candidate.name.to_string());
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
    }

    fn assign_constant_global_strings(&mut self) {
        let Ok(entry) = self.function(self.module.entry) else {
            return;
        };
        let definitions = entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| instruction.out.map(|out| (out, &instruction.op)))
            .collect::<AHashMap<_, _>>();
        let mut assigned = AHashMap::<SymbolId, Option<String>>::new();
        for instruction in entry.blocks.iter().flat_map(|block| &block.instructions) {
            let ControlFlowOp::StoreGlobal { global, value } = instruction.op else {
                continue;
            };
            let constant = match definitions.get(&value) {
                Some(ControlFlowOp::Const(ConstValue::String(text))) => Some((*text).to_string()),
                _ => None,
            };
            match assigned.get_mut(&global) {
                Some(existing) => {
                    *existing = None;
                }
                None => {
                    assigned.insert(global, constant);
                }
            }
        }
        self.constant_global_strings = assigned
            .into_iter()
            .filter_map(|(global, value)| value.map(|value| (global, value)))
            .collect();
    }

    fn assign_deferred_global_declarations(&mut self) {
        let Ok(entry) = self.function(self.module.entry) else {
            return;
        };
        self.deferred_global_declarations = entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match instruction.op {
                ControlFlowOp::StoreGlobal { global, .. }
                    if !self.constant_global_strings.contains_key(&global) =>
                {
                    Some(global)
                }
                _ => None,
            })
            .collect();
    }

    fn global_string_index_in_bounds(
        &self,
        object: ValueId,
        index: ValueId,
        context: &LocalNames,
    ) -> Option<bool> {
        let range = context.integer_ranges.get(&index)?;
        if range.min < 0 {
            return Some(false);
        }
        let symbol = *context.global_loads.get(&object)?;
        let length = self
            .constant_global_strings
            .get(&symbol)?
            .encode_utf16()
            .count();
        Some(range.max < length as i64)
    }

    fn emit_chunks(mut self, plan: &IrJsChunkPlan) -> Result<Vec<IrJsChunk>, CodegenError> {
        let fallback_span = self.function(self.module.entry)?.span;
        let mut files = AHashSet::new();
        for file in std::iter::once(&plan.entry_file)
            .chain(plan.chunks.iter().map(|chunk| &chunk.file_name))
        {
            if file.is_empty()
                || Path::new(file).file_name().and_then(|name| name.to_str()) != Some(file)
            {
                return Err(CodegenError::new(
                    fallback_span,
                    "chunk file names must not contain directory components",
                ));
            }
            if !files.insert(file) {
                return Err(CodegenError::new(
                    fallback_span,
                    format!("duplicate chunk file name `{file}`"),
                ));
            }
        }
        self.prepare();
        for chunk in &plan.chunks {
            if let Some(module) = chunk.lazy_module {
                if self
                    .dynamic_chunk_files
                    .insert(module, chunk.file_name.clone())
                    .is_some()
                {
                    return Err(CodegenError::new(
                        fallback_span,
                        format!("dynamic module {module} belongs to more than one chunk"),
                    ));
                }
            }
        }
        let emitted = self
            .module
            .functions
            .iter()
            .filter(|function| {
                is_emitted_function(function, self.options.inline_structured_closures)
            })
            .map(|function| function.id)
            .collect::<AHashSet<_>>();
        let mut owners = emitted
            .iter()
            .copied()
            .map(|function| (function, None))
            .collect::<AHashMap<_, Option<usize>>>();
        for (chunk_index, chunk) in plan.chunks.iter().enumerate() {
            for function in &chunk.functions {
                if !emitted.contains(function) {
                    return Err(CodegenError::new(
                        self.module
                            .functions
                            .get(function.0 as usize)
                            .map_or(fallback_span, |item| item.span),
                        format!("function {} cannot be emitted as a chunk", function.0),
                    ));
                }
                if owners
                    .insert(*function, Some(chunk_index))
                    .flatten()
                    .is_some()
                {
                    return Err(CodegenError::new(
                        self.function(*function)?.span,
                        format!("function {} belongs to more than one chunk", function.0),
                    ));
                }
                if function_writes_global(
                    self.module,
                    *function,
                    &mut AHashSet::new(),
                    self.options.inline_structured_closures,
                ) {
                    return Err(CodegenError::new(
                        self.function(*function)?.span,
                        "functions that mutate module globals must remain in the entry chunk",
                    ));
                }
            }
        }

        let mut unit_functions = vec![Vec::new(); plan.chunks.len() + 1];
        for function in emitted {
            let unit = owners[&function].map_or(0, |chunk| chunk + 1);
            unit_functions[unit].push(function);
        }
        for functions in &mut unit_functions {
            functions.sort_unstable_by_key(|function| function.0);
        }

        let unit_files = std::iter::once(plan.entry_file.clone())
            .chain(plan.chunks.iter().map(|chunk| chunk.file_name.clone()))
            .collect::<Vec<_>>();
        let mut imports = vec![AHashMap::<usize, AHashSet<String>>::new(); unit_files.len()];
        let mut dynamic_imports = vec![AHashSet::<u32>::new(); unit_files.len()];
        for (unit, functions) in unit_functions.iter().enumerate() {
            let mut roots = functions.clone();
            if unit == 0 {
                roots.push(self.module.entry);
            }
            let references = collect_chunk_references(
                self.module,
                &roots,
                &self.string_aliases,
                &self.numeric_aliases,
                self.options.inline_structured_closures,
            );
            dynamic_imports[unit].extend(references.dynamic_modules.iter().copied());
            for function in references.functions {
                let Some(owner) = owners.get(&function) else {
                    continue;
                };
                let source = owner.map_or(0, |chunk| chunk + 1);
                if source != unit {
                    imports[unit]
                        .entry(source)
                        .or_default()
                        .insert(self.function_name(function)?.to_string());
                }
            }
            if unit != 0 {
                let entry_imports = imports[unit].entry(0).or_default();
                for global in references.globals {
                    if !self
                        .module
                        .globals
                        .iter()
                        .any(|candidate| candidate.symbol == global && candidate.external)
                    {
                        entry_imports.insert(self.global_name(global)?.to_string());
                    }
                }
                entry_imports.extend(references.strings);
            }
        }
        for export in &self.module.exports {
            if let ExportBinding::Function(function) = export.binding {
                let source = owners
                    .get(&function)
                    .copied()
                    .flatten()
                    .map_or(0, |chunk| chunk + 1);
                if source != 0 {
                    imports[0]
                        .entry(source)
                        .or_default()
                        .insert(self.function_name(function)?.to_string());
                }
            }
        }
        for (chunk_index, chunk) in plan.chunks.iter().enumerate() {
            let Some(module_id) = chunk.lazy_module else {
                continue;
            };
            let module = self
                .module
                .lazy_modules
                .iter()
                .find(|module| module.id == module_id)
                .ok_or_else(|| {
                    CodegenError::new(
                        fallback_span,
                        format!("chunk references unknown dynamic module {module_id}"),
                    )
                })?;
            let unit = chunk_index + 1;
            for export in &module.exports {
                let (source, name) = match export.binding {
                    ExportBinding::Function(function) => (
                        owners
                            .get(&function)
                            .copied()
                            .flatten()
                            .map_or(0, |owner| owner + 1),
                        self.function_name(function)?.to_string(),
                    ),
                    ExportBinding::Global(global) => (0, self.global_name(global)?.to_string()),
                    ExportBinding::TypeOnly => {
                        return Err(CodegenError::new(
                            export.span,
                            format!("dynamic export `{}` has no runtime binding", export.name),
                        ));
                    }
                };
                if source != unit {
                    imports[unit].entry(source).or_default().insert(name);
                }
            }
        }

        let mut internal_exports = vec![AHashSet::<String>::new(); unit_files.len()];
        for dependencies in &imports {
            for (source, names) in dependencies {
                internal_exports[*source].extend(names.iter().cloned());
            }
        }

        let mut output = Vec::with_capacity(unit_files.len());
        for unit in 0..unit_files.len() {
            let mut code = String::new();
            emit_chunk_imports(
                &mut code,
                unit,
                &unit_files,
                &imports[unit],
                self.options.string_quote,
            );
            if unit == 0 {
                self.emit_module_preamble(&mut code)?;
            }
            self.emit_function_group(&unit_functions[unit], &mut code)?;
            if unit == 0 {
                self.emit_entry_body(&mut code)?;
                self.emit_named_exports(&internal_exports[unit], &mut code);
                self.emit_exports_excluding(&internal_exports[unit], &mut code)?;
            } else {
                self.emit_named_exports(&internal_exports[unit], &mut code);
                if let Some(module) = plan.chunks[unit - 1].lazy_module {
                    self.emit_dynamic_module_exports(module, &mut code)?;
                }
            }
            output.push(IrJsChunk {
                file_name: unit_files[unit].clone(),
                code,
                dependencies: {
                    let mut files = imports[unit]
                        .iter()
                        .filter(|(source, names)| **source != unit && !names.is_empty())
                        .map(|(source, _)| unit_files[*source].clone())
                        .collect::<Vec<_>>();
                    files.sort_unstable();
                    files.dedup();
                    files
                },
                dynamic_dependencies: {
                    let mut files = dynamic_imports[unit]
                        .iter()
                        .filter_map(|module| self.dynamic_chunk_files.get(module).cloned())
                        .collect::<Vec<_>>();
                    files.sort_unstable();
                    files.dedup();
                    files
                },
            });
        }
        Ok(output)
    }

    fn emit_dynamic_module_exports(
        &self,
        module_id: u32,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let module = self
            .module
            .lazy_modules
            .iter()
            .find(|module| module.id == module_id)
            .ok_or_else(|| {
                CodegenError::new(
                    self.module.functions[self.module.entry.0 as usize].span,
                    format!("missing dynamic module {module_id}"),
                )
            })?;
        if module.exports.is_empty() {
            return Ok(());
        }
        out.push_str("export{");
        for (index, export) in module.exports.iter().enumerate() {
            if index != 0 {
                out.push(',');
            }
            let binding = match export.binding {
                ExportBinding::Function(function) => self.function_name(function)?,
                ExportBinding::Global(global) => self.global_name(global)?,
                ExportBinding::TypeOnly => {
                    return Err(CodegenError::new(
                        export.span,
                        format!("dynamic export `{}` has no runtime binding", export.name),
                    ));
                }
            };
            out.push_str(binding);
            if binding != export.name {
                out.push_str(" as ");
                out.push_str(export.name);
            }
        }
        out.push_str("};");
        Ok(())
    }

    fn emit_module_preamble(&mut self, out: &mut String) -> Result<(), CodegenError> {
        if !self.pooled_strings.is_empty() || !self.pooled_numbers.is_empty() {
            out.push_str("let ");
            let mut index = 0;
            for (value, name) in &self.pooled_strings {
                if index != 0 {
                    out.push(',');
                }
                out.push_str(name);
                out.push('=');
                out.push_str(&render_string_literal(value, self.options.string_quote));
                index += 1;
            }
            for (value, name) in &self.pooled_numbers {
                if index != 0 {
                    out.push(',');
                }
                out.push_str(name);
                out.push('=');
                out.push_str(value);
                index += 1;
            }
            out.push(';');
        }
        let owned_globals = self
            .module
            .globals
            .iter()
            .filter(|global| !global.external)
            .collect::<Vec<_>>();
        if !owned_globals.is_empty() {
            out.push_str("let ");
            for (index, global) in owned_globals.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                out.push_str(self.global_name(global.symbol)?);
                if let Some(value) = self.constant_global_strings.get(&global.symbol) {
                    out.push('=');
                    out.push_str(&render_string_literal(value, self.options.string_quote));
                }
                self.declared_globals.insert(global.symbol);
            }
            out.push(';');
        }
        self.emit_external_export_aliases(out)?;
        Ok(())
    }

    fn emit_external_export_aliases(&self, out: &mut String) -> Result<(), CodegenError> {
        let mut aliases = self.external_export_aliases.iter().collect::<Vec<_>>();
        aliases.sort_unstable_by_key(|(symbol, _)| symbol.0);
        for (symbol, alias) in aliases {
            out.push_str("const ");
            out.push_str(alias);
            out.push('=');
            out.push_str(self.global_name(*symbol)?);
            out.push(';');
        }
        Ok(())
    }

    fn emit_entry_body(&mut self, out: &mut String) -> Result<(), CodegenError> {
        let entry = self.function(self.module.entry)?.clone();
        if entry.blocks.len() == 1 && entry.blocks[0].phis.is_empty() {
            self.emit_single_block(&entry, false, out)
        } else if can_structure(&entry) {
            self.emit_structured(&entry, false, out)
        } else {
            out.push_str("(()=>");
            self.emit_state_machine(&entry, out)?;
            out.push_str(")();");
            Ok(())
        }
    }

    fn emit_named_exports(&self, names: &AHashSet<String>, out: &mut String) {
        if names.is_empty() {
            return;
        }
        let mut names = names.iter().collect::<Vec<_>>();
        names.sort_unstable();
        if !out.is_empty() && !out.ends_with(';') {
            out.push(';');
        }
        out.push_str("export{");
        for (index, name) in names.iter().enumerate() {
            if index != 0 {
                out.push(',');
            }
            out.push_str(name);
        }
        out.push_str("};");
    }

    fn emit_exports(&self, out: &mut String) -> Result<(), CodegenError> {
        self.emit_exports_excluding(&AHashSet::new(), out)
    }

    fn emit_exports_excluding(
        &self,
        already_exported: &AHashSet<String>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let mut runtime_exports = Vec::<(&str, &str)>::new();
        for export in &self.module.exports {
            let internal = match export.binding {
                ExportBinding::Function(function) => self.function_name(function)?,
                ExportBinding::Global(symbol) => self
                    .external_export_aliases
                    .get(&symbol)
                    .map_or(self.global_name(symbol)?, String::as_str),
                ExportBinding::TypeOnly => continue,
            };
            let public = if self.options.mangle_exports {
                internal
            } else {
                export.name
            };
            if internal != public || !already_exported.contains(internal) {
                runtime_exports.push((internal, public));
            }
        }
        if runtime_exports.is_empty() {
            return Ok(());
        }
        if !out.is_empty() && !out.ends_with(';') {
            out.push(';');
        }
        out.push_str("export{");
        for (index, (internal, public)) in runtime_exports.iter().enumerate() {
            if index != 0 {
                out.push(',');
            }
            out.push_str(internal);
            if internal != public {
                out.push_str(" as ");
                out.push_str(public);
            }
        }
        out.push('}');
        Ok(())
    }

    fn assign_top_level_names(&mut self) {
        for function in &self.module.functions {
            if function.live && function.kind == FunctionKind::Extern {
                if let Some(name) = function.name {
                    self.top_level_mangler.reserve(name);
                    self.function_names.insert(function.id, name.to_string());
                }
            }
        }
        for global in &self.module.globals {
            if global.external {
                self.top_level_mangler.reserve(global.name);
                self.global_names
                    .insert(global.symbol, global.name.to_string());
            }
        }

        if self.options.mangle_identifiers && self.options.cross_scope_name_reuse {
            for _ in 0..self.options.local_name_reserve {
                self.local_name_reservations
                    .push(self.top_level_mangler.next_name());
            }
            self.assign_preferred_local_names();
        }

        if !self.options.mangle_identifiers {
            for function in &self.module.functions {
                if !function.live
                    || matches!(function.kind, FunctionKind::Entry | FunctionKind::Extern)
                    || (function.kind == FunctionKind::Closure
                        && can_inline_closure(function, self.options.inline_structured_closures))
                {
                    continue;
                }
                let source_name = function.name.unwrap_or("closure");
                let preferred = match function.kind {
                    FunctionKind::Method { class } => format!("{class}${source_name}"),
                    FunctionKind::Constructor { class } => format!("{class}$init"),
                    FunctionKind::Closure => format!("closure${}", function.id.0),
                    _ => source_name.to_string(),
                };
                let name = self.top_level_mangler.unique_name(&preferred);
                self.function_names.insert(function.id, name);
            }
            for global in &self.module.globals {
                if global.external {
                    continue;
                }
                let name = self.top_level_mangler.unique_name(global.name);
                self.global_names.insert(global.symbol, name);
            }
            return;
        }

        let mut function_uses = AHashMap::<FunctionId, usize>::new();
        let mut global_uses = AHashMap::<SymbolId, usize>::new();
        for function in self
            .module
            .functions
            .iter()
            .filter(|function| function.live)
        {
            for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
                match &instruction.op {
                    ControlFlowOp::LoadGlobal(global)
                    | ControlFlowOp::StoreGlobal { global, .. } => {
                        *global_uses.entry(*global).or_insert(0) += 1;
                    }
                    ControlFlowOp::NewClass {
                        constructor: Some(function),
                        ..
                    }
                    | ControlFlowOp::Closure { function, .. }
                    | ControlFlowOp::CallDirect { function, .. }
                    | ControlFlowOp::CallMethod { function, .. } => {
                        *function_uses.entry(*function).or_insert(0) += 1;
                    }
                    _ => {}
                }
            }
        }

        let mut bindings = Vec::new();
        for function in &self.module.functions {
            if !function.live
                || matches!(function.kind, FunctionKind::Entry | FunctionKind::Extern)
                || (function.kind == FunctionKind::Closure
                    && can_inline_closure(function, self.options.inline_structured_closures))
            {
                continue;
            }
            bindings.push((
                function_uses.get(&function.id).copied().unwrap_or(0) + 1,
                0_u8,
                function.id.0,
            ));
        }
        for global in &self.module.globals {
            if global.external {
                continue;
            }
            bindings.push((
                global_uses.get(&global.symbol).copied().unwrap_or(0) + 1,
                1_u8,
                global.symbol.0,
            ));
        }
        bindings.sort_unstable_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| left.1.cmp(&right.1))
                .then_with(|| left.2.cmp(&right.2))
        });
        for (_, kind, id) in bindings {
            let name = self.top_level_mangler.next_name();
            if kind == 0 {
                self.function_names.insert(FunctionId(id), name);
            } else {
                self.global_names.insert(SymbolId(id), name);
            }
        }
    }

    fn assign_preferred_local_names(&mut self) {
        let mut frequencies = AHashMap::<String, (usize, usize)>::new();
        for function in self
            .module
            .functions
            .iter()
            .filter(|function| function.live)
        {
            let uses = use_counts(function);
            let mut present = AHashSet::new();
            for (index, local) in function.value_local_hints.iter().enumerate() {
                let Some(local) = *local else {
                    continue;
                };
                let entry = frequencies.entry(local.to_string()).or_insert((0, 0));
                entry.1 += uses.get(&ValueId(index as u32)).copied().unwrap_or(0) + 1;
                if present.insert(local) {
                    entry.0 += 1;
                }
            }
        }
        let mut locals = frequencies.into_iter().collect::<Vec<_>>();
        locals.sort_unstable_by(|left, right| {
            right
                .1
                 .0
                .cmp(&left.1 .0)
                .then_with(|| right.1 .1.cmp(&left.1 .1))
                .then_with(|| left.0.cmp(&right.0))
        });
        for ((name, _), identifier) in locals.into_iter().zip(&self.local_name_reservations) {
            self.preferred_local_names.insert(name, identifier.clone());
        }
    }

    fn assign_external_export_aliases(&mut self) {
        for export in &self.module.exports {
            let ExportBinding::Global(symbol) = export.binding else {
                continue;
            };
            let Some(global) = self
                .module
                .globals
                .iter()
                .find(|global| global.symbol == symbol && global.external)
            else {
                continue;
            };
            let alias = if self.options.mangle_identifiers {
                self.top_level_mangler.next_name()
            } else {
                self.top_level_mangler
                    .unique_name(&format!("$host${}", global.name))
            };
            self.external_export_aliases.insert(symbol, alias);
        }
    }

    fn assign_string_aliases(&mut self) {
        if !self.options.pool_strings {
            return;
        }
        let mut counts = AHashMap::<String, usize>::new();
        for function in self
            .module
            .functions
            .iter()
            .filter(|function| function.live)
        {
            let uses = use_counts(function);
            for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
                if let (Some(out), ControlFlowOp::Const(ConstValue::String(value))) =
                    (instruction.out, &instruction.op)
                {
                    if uses.get(&out).copied().unwrap_or(0) != 0 {
                        *counts.entry(value.clone()).or_insert(0) += 1;
                    }
                }
            }
        }

        let mut candidates = counts
            .into_iter()
            .filter_map(|(value, count)| {
                let literal_length = value.len() + 2;
                let unaliased = count * literal_length;
                let aliased = literal_length + 7 + count;
                (unaliased > aliased).then(|| (unaliased - aliased, count, value))
            })
            .collect::<Vec<_>>();
        candidates.sort_unstable_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| right.1.cmp(&left.1))
                .then_with(|| left.2.cmp(&right.2))
        });
        for (_, _, value) in candidates {
            let name = self.top_level_mangler.next_name();
            self.string_aliases.insert(value.clone(), name.clone());
            self.pooled_strings.push((value, name));
        }
    }

    fn assign_numeric_aliases(&mut self) {
        if !self.options.pool_numeric_literals {
            return;
        }
        let mut counts = AHashMap::<String, usize>::new();
        for function in self
            .module
            .functions
            .iter()
            .filter(|function| function.live)
        {
            let uses = use_counts(function);
            for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
                let (Some(out), ControlFlowOp::Const(value)) = (instruction.out, &instruction.op)
                else {
                    continue;
                };
                if matches!(
                    value,
                    ConstValue::String(_) | ConstValue::Bool(_) | ConstValue::Null
                ) || uses.get(&out).copied().unwrap_or(0) == 0
                {
                    continue;
                }
                let rendered = render_const(
                    value,
                    self.options.compact_boolean_literals,
                    self.options.string_quote,
                );
                *counts.entry(rendered).or_insert(0) += 1;
            }
        }
        let mut candidates = counts.into_iter().collect::<Vec<_>>();
        candidates.sort_unstable_by(|left, right| {
            (right.0.len().saturating_sub(1) * right.1)
                .cmp(&(left.0.len().saturating_sub(1) * left.1))
                .then_with(|| right.1.cmp(&left.1))
                .then_with(|| left.0.cmp(&right.0))
        });
        for (value, count) in candidates {
            let mut trial = self.top_level_mangler.clone();
            let name = trial.next_name();
            let inline_cost = value.len().saturating_mul(count);
            let pooled_cost = value.len() + name.len() + 2 + name.len().saturating_mul(count);
            if pooled_cost >= inline_cost {
                continue;
            }
            self.top_level_mangler = trial;
            self.numeric_aliases.insert(value.clone(), name.clone());
            self.pooled_numbers.push((value, name));
        }
    }

    fn assign_property_names(&mut self) {
        if !self.options.mangle_properties {
            return;
        }
        let stable_public_fields = if self.options.mangle_exports {
            AHashSet::new()
        } else {
            self.module
                .structs
                .iter()
                .chain(&self.module.classes)
                .filter(|layout| self.class_uses_named_fields(layout.name))
                .flat_map(|layout| layout.fields.iter().map(|field| field.name.to_string()))
                .collect::<AHashSet<_>>()
        };
        let mut frequencies = AHashMap::<String, usize>::new();
        for field in self
            .module
            .structs
            .iter()
            .chain(&self.module.classes)
            .flat_map(|layout| &layout.fields)
        {
            if !stable_public_fields.contains(field.name) {
                frequencies.entry(field.name.to_string()).or_insert(0);
            }
        }
        for function in self
            .module
            .functions
            .iter()
            .filter(|function| function.live)
        {
            for block in &function.blocks {
                let loop_weight = 1 + function
                    .shapes
                    .iter()
                    .filter(|shape| matches!(shape, ControlShape::Loop { body, update, .. } if *body == block.id || *update == Some(block.id)))
                    .count()
                    * 3;
                for instruction in &block.instructions {
                    let field = match &instruction.op {
                        ControlFlowOp::FieldGet { field, .. }
                        | ControlFlowOp::FieldSet { field, .. } => Some(*field),
                        _ => None,
                    };
                    if let Some(field) =
                        field.filter(|field| !stable_public_fields.contains(*field))
                    {
                        *frequencies.entry(field.to_string()).or_insert(0) += loop_weight;
                    }
                }
            }
        }
        let mut fields = frequencies.into_iter().collect::<Vec<_>>();
        fields.sort_unstable_by(|left, right| {
            right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0))
        });
        let alphabet = if self.options.entropy_property_names {
            self.options.identifier_alphabet
        } else {
            IdentifierAlphabet::canonical()
        };
        let mut mangler = Mangler::new(alphabet);
        for (field, _) in fields {
            self.property_names.insert(field, mangler.next_name());
        }
    }

    fn local_mangler(&self, function: &ControlFlowFunction<'src>) -> Mangler {
        if !self.options.cross_scope_name_reuse || function.kind == FunctionKind::Entry {
            return self.top_level_mangler.clone();
        }
        let mut referenced = AHashSet::new();
        self.collect_top_level_references(function.id, &mut AHashSet::new(), &mut referenced);
        let mut mangler = self.top_level_mangler.clone();
        for candidate in self
            .module
            .functions
            .iter()
            .filter(|candidate| candidate.live && candidate.kind != FunctionKind::Extern)
        {
            if let Some(name) = self.function_names.get(&candidate.id) {
                if !referenced.contains(name) {
                    mangler.release(name);
                }
            }
        }
        for global in self.module.globals.iter().filter(|global| !global.external) {
            if let Some(name) = self.global_names.get(&global.symbol) {
                if !referenced.contains(name) {
                    mangler.release(name);
                }
            }
        }
        for name in self.string_aliases.values() {
            if !referenced.contains(name) {
                mangler.release(name);
            }
        }
        for name in self.numeric_aliases.values() {
            if !referenced.contains(name) {
                mangler.release(name);
            }
        }
        for name in &self.local_name_reservations {
            mangler.release(name);
        }
        mangler.rewind();
        mangler
    }

    fn collect_top_level_references(
        &self,
        function: FunctionId,
        visited: &mut AHashSet<FunctionId>,
        referenced: &mut AHashSet<String>,
    ) {
        if !visited.insert(function) {
            return;
        }
        let Some(function) = self.module.functions.get(function.0 as usize) else {
            return;
        };
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            match &instruction.op {
                ControlFlowOp::CallDirect { function, .. }
                | ControlFlowOp::NewClass {
                    constructor: Some(function),
                    ..
                } => {
                    if let Some(name) = self.function_names.get(function) {
                        referenced.insert(name.clone());
                    }
                }
                ControlFlowOp::Closure {
                    function: closure, ..
                } => {
                    let inline =
                        self.module
                            .functions
                            .get(closure.0 as usize)
                            .is_some_and(|closure| {
                                can_inline_closure(closure, self.options.inline_structured_closures)
                            });
                    if inline {
                        self.collect_top_level_references(*closure, visited, referenced);
                    } else if let Some(name) = self.function_names.get(closure) {
                        referenced.insert(name.clone());
                    }
                }
                ControlFlowOp::LoadGlobal(global) | ControlFlowOp::StoreGlobal { global, .. } => {
                    if let Some(name) = self.global_names.get(global) {
                        referenced.insert(name.clone());
                    }
                }
                ControlFlowOp::Const(ConstValue::String(value)) => {
                    if let Some(name) = self.string_aliases.get(value) {
                        referenced.insert(name.clone());
                    }
                }
                ControlFlowOp::Const(value) => {
                    let rendered = render_const(
                        value,
                        self.options.compact_boolean_literals,
                        self.options.string_quote,
                    );
                    if let Some(name) = self.numeric_aliases.get(&rendered) {
                        referenced.insert(name.clone());
                    }
                }
                ControlFlowOp::DynamicImport { module } => {
                    if let Some(module) = self
                        .module
                        .lazy_modules
                        .iter()
                        .find(|candidate| candidate.id == *module)
                    {
                        for export in &module.exports {
                            let name = match export.binding {
                                ExportBinding::Function(function) => {
                                    self.function_names.get(&function)
                                }
                                ExportBinding::Global(global) => self.global_names.get(&global),
                                ExportBinding::TypeOnly => None,
                            };
                            if let Some(name) = name {
                                referenced.insert(name.clone());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn property_name<'name>(&'name self, field: &'name str) -> &'name str {
        self.property_names.get(field).map_or(field, String::as_str)
    }

    fn function_has_public_abi(&self, function: FunctionId) -> bool {
        self.module
            .exports
            .iter()
            .chain(
                self.module
                    .lazy_modules
                    .iter()
                    .flat_map(|module| module.exports.iter()),
            )
            .any(|export| export.binding == ExportBinding::Function(function))
    }

    fn function_reads_own_arguments(&self, function: &ControlFlowFunction<'src>) -> bool {
        let arguments = self
            .module
            .globals
            .iter()
            .find(|global| global.external && global.name == "arguments")
            .map(|global| global.symbol);
        arguments.is_some_and(|arguments| {
            function
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .any(|instruction| {
                    matches!(instruction.op, ControlFlowOp::LoadGlobal(global) if global == arguments)
                })
        })
    }

    fn emit_function(
        &mut self,
        function: &ControlFlowFunction<'src>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let name = self.function_name(function.id)?.to_string();
        let public_abi = self.function_has_public_abi(function.id);
        let arrow_binding = self.options.function_spelling == FunctionSpelling::Arrow
            && matches!(function.kind, FunctionKind::Function)
            && function.capture_count == 0
            && (!public_abi || self.options.public_function_arrows)
            // A bare JavaScript `arguments` binding is local to an ordinary
            // function. Emitting an arrow here would silently capture the
            // module wrapper's binding and change the host ABI.
            && !self.function_reads_own_arguments(function)
            && self.options.mangle_identifiers;
        let single_block = function.blocks.len() == 1 && function.blocks[0].phis.is_empty();
        let structure_available = !single_block && can_structure(function);
        let structured = match self.options.control_flow_spelling {
            ControlFlowSpelling::Auto | ControlFlowSpelling::Structured => structure_available,
            ControlFlowSpelling::StateMachine => false,
        };
        let local_mangler = self.local_mangler(function);
        let mut context = LocalNames::new(
            function,
            self.integer_analysis.function(function.id),
            !single_block && !structured,
            &local_mangler,
            &self.preferred_local_names,
            &self.numeric_aliases,
            &self.options,
        );
        context.inline_declarations = structured;
        let uses = &context.use_counts;
        let parameter_count = if public_abi {
            function.params.len()
        } else {
            function
                .params
                .iter()
                .rposition(|param| uses.get(&param.value).copied().unwrap_or(0) != 0)
                .map_or(0, |index| index + 1)
        };
        let mut params = String::new();
        for (index, param) in function.params.iter().take(parameter_count).enumerate() {
            if index != 0 {
                params.push(',');
            }
            params.push_str(context.value_name(param.value)?);
            if public_abi {
                if let Some(default) = &param.default {
                    params.push('=');
                    params.push_str(&render_param_default(
                        default,
                        function,
                        &context,
                        self.options.compact_boolean_literals,
                    )?);
                }
            }
        }
        if arrow_binding {
            out.push_str("let ");
            out.push_str(&name);
            out.push('=');
            if parameter_count != 1
                || function
                    .params
                    .first()
                    .is_some_and(|param| param.default.is_some())
            {
                out.push('(');
                out.push_str(&params);
                out.push(')');
            } else {
                out.push_str(&params);
            }
            out.push_str("=>");
        } else {
            out.push_str("function ");
            out.push_str(&name);
            out.push('(');
            out.push_str(&params);
            out.push(')');
        }
        if self.options.conditional_expressions {
            if let Some(expression) = self.render_conditional_return(function, &context)? {
                let self_default = (parameter_count == 1)
                    .then(|| {
                        rewrite_self_default_conditional(
                            &expression,
                            &name,
                            context.value_name(function.params[0].value).ok()?,
                            is_nullable_with_truthy_value(&function.params[0].ty),
                        )
                    })
                    .flatten();
                if let Some((assignment, returned)) = self_default {
                    if let Some(folded) = fold_default_assignment_into_first_field(
                        &assignment,
                        &returned,
                        context.value_name(function.params[0].value)?,
                    ) {
                        out.push_str("{return ");
                        out.push_str(&folded);
                        out.push('}');
                    } else {
                        out.push('{');
                        out.push_str(&assignment);
                        out.push_str(";return ");
                        out.push_str(&returned);
                        out.push('}');
                    }
                    if arrow_binding {
                        out.push(';');
                    }
                    return Ok(());
                }
                if arrow_binding {
                    out.push_str(&expression);
                    out.push(';');
                } else {
                    out.push_str("{return ");
                    out.push_str(&expression);
                    out.push('}');
                }
                return Ok(());
            }
        }
        let public_int_params = if public_abi {
            function
                .params
                .iter()
                .take(parameter_count)
                .filter(|param| param.ty == Type::Int)
                .filter(|param| param_feeds_loop_phi(function, param.value))
                .filter_map(|param| context.value_name(param.value).ok())
                .map(str::to_string)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let function_start = out.len();
        let body_start = out.len();
        if single_block {
            self.emit_single_block_with_context(function, true, context, out)?;
        } else if structured {
            self.emit_structured_with_context(function, true, context, out)?;
        } else {
            self.emit_state_machine_with_context(function, context, out)?;
        }
        if arrow_binding {
            try_rewrite_arrow_expression_body(out, body_start);
            if !out.ends_with(';') {
                out.push(';');
            }
        }
        if !public_int_params.is_empty() {
            inject_public_int_param_coercions(out, function_start, &public_int_params);
        }
        Ok(())
    }

    fn emit_function_group(
        &mut self,
        functions: &[FunctionId],
        out: &mut String,
    ) -> Result<(), CodegenError> {
        if self.options.function_layout == FunctionLayout::Source {
            for function in functions {
                let function = self.function(*function)?.clone();
                self.emit_function(&function, out)?;
            }
            return Ok(());
        }
        let mut segments = Vec::with_capacity(functions.len());
        for function in functions {
            let function = self.function(*function)?.clone();
            let mut code = String::new();
            self.emit_function(&function, &mut code)?;
            segments.push(code);
        }
        let order = match self.options.function_layout {
            FunctionLayout::Source => unreachable!("source layout returned before buffering"),
            FunctionLayout::CompressionSimilarity => {
                compression_similarity_order(&segments, self.options.function_layout_exact_limit)
            }
            FunctionLayout::CompressionWindow(window_bytes) => compression_window_order(
                &segments,
                window_bytes,
                self.options.function_layout_exact_limit,
            ),
        };
        for index in order {
            out.push_str(&segments[index]);
        }
        Ok(())
    }

    fn render_conditional_return(
        &mut self,
        function: &ControlFlowFunction<'src>,
        context: &LocalNames,
    ) -> Result<Option<String>, CodegenError> {
        // Keep straight-line functions on the statement emitter. Its name
        // coalescing deliberately reuses one JavaScript binding for successive
        // SSA values, which cannot be reconstructed as one nested expression
        // merely by substituting that shared spelling.
        let mut cursor = function.entry;
        let mut visited = AHashSet::new();
        loop {
            if !visited.insert(cursor) {
                return Ok(None);
            }
            match function.blocks[cursor.0 as usize].terminator {
                Some(Terminator::Jump(target)) => cursor = target,
                Some(Terminator::Branch { .. }) => break,
                _ => return Ok(None),
            }
        }
        let uses = &context.use_counts;
        let Some(expression) = self.render_return_path(
            function,
            function.entry,
            context,
            uses,
            AHashMap::new(),
            AHashSet::new(),
            0,
        )?
        else {
            return Ok(None);
        };
        Ok(Some(expression.into_minimal()))
    }

    /// Render a return-only CFG region as one expression.  Unlike the old
    /// linear-arm helper, this follows nested structured branches, allowing a
    /// guard-return ladder to become a right-associated conditional expression.
    /// Every traversed instruction must remain safely deferrable and every
    /// complete path retains the existing one-effectful-call ceiling.
    #[allow(clippy::too_many_arguments)]
    fn render_return_path(
        &mut self,
        function: &ControlFlowFunction<'src>,
        mut block: BlockId,
        context: &LocalNames,
        uses: &AHashMap<ValueId, usize>,
        mut cache: ExpressionCache,
        mut visited: AHashSet<BlockId>,
        mut deferred_effects: usize,
    ) -> Result<Option<JsExpression>, CodegenError> {
        loop {
            if !visited.insert(block) {
                return Ok(None);
            }
            let current = &function.blocks[block.0 as usize];
            if !current.phis.is_empty() {
                return Ok(None);
            }
            for instruction in &current.instructions {
                let Some(out) = instruction.out else {
                    return Ok(None);
                };
                let deferred_effect = matches!(instruction.op, ControlFlowOp::CallDirect { .. });
                if (!expression_only_op(&instruction.op) && !deferred_effect)
                    || (uses.get(&out).copied().unwrap_or(0) > 1 && !op_can_defer(&instruction.op))
                    || (deferred_effect && uses.get(&out).copied().unwrap_or(0) != 1)
                {
                    return Ok(None);
                }
                if deferred_effect {
                    deferred_effects += 1;
                    if deferred_effects > 1 {
                        return Ok(None);
                    }
                }
                let expression = self.render_instruction_op(instruction, context, &mut cache)?;
                cache.insert(out, expression);
            }
            match current.terminator {
                Some(Terminator::Jump(target)) => block = target,
                Some(Terminator::Return(Some(value))) => {
                    return take_value(value, context, &mut cache).map(Some)
                }
                Some(Terminator::Branch { condition, .. }) => {
                    let Some(crate::ir::ControlShape::If {
                        then_block,
                        else_block,
                        ..
                    }) = shape_at(function, block)
                    else {
                        return Ok(None);
                    };
                    let condition = take_value(condition, context, &mut cache)?;
                    let Some(then_value) = self.render_return_path(
                        function,
                        then_block,
                        context,
                        uses,
                        cache.clone(),
                        visited.clone(),
                        deferred_effects,
                    )?
                    else {
                        return Ok(None);
                    };
                    let Some(else_value) = self.render_return_path(
                        function,
                        else_block,
                        context,
                        uses,
                        cache,
                        visited,
                        deferred_effects,
                    )?
                    else {
                        return Ok(None);
                    };
                    return Ok(Some(JsExpression::conditional(
                        condition, then_value, else_value,
                    )));
                }
                _ => return Ok(None),
            }
        }
    }

    fn emit_single_block(
        &mut self,
        function: &ControlFlowFunction<'src>,
        wrapped: bool,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let local_mangler = self.local_mangler(function);
        let context = LocalNames::new(
            function,
            self.integer_analysis.function(function.id),
            false,
            &local_mangler,
            &self.preferred_local_names,
            &self.numeric_aliases,
            &self.options,
        );
        self.emit_single_block_with_context(function, wrapped, context, out)
    }

    fn emit_single_block_with_context(
        &mut self,
        function: &ControlFlowFunction<'src>,
        wrapped: bool,
        mut context: LocalNames,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        context.inline_declarations = true;
        if wrapped {
            out.push('{');
        }
        let block = &function.blocks[0];
        let uses = &context.use_counts;
        let mut cache = ExpressionCache::new();
        let mut previous_binding = false;
        let mut previous_expressions = None::<(usize, Vec<JsExpression>)>;
        for (index, instruction) in block.instructions.iter().enumerate() {
            let fuse_with_next = instruction.out.is_some_and(|value| {
                uses.get(&value).copied().unwrap_or(0) == 1 && can_fuse_value(block, index, value)
            });
            let mut statement = String::new();
            self.emit_linear_instruction(
                instruction,
                uses,
                fuse_with_next,
                false,
                &context,
                &mut cache,
                &mut statement,
            )?;
            if statement.is_empty() {
                continue;
            }
            let binding = is_single_binding_statement(&statement);
            let statement_start = out.len();
            if previous_binding && binding {
                out.pop();
                out.push(',');
                out.push_str(&statement[4..]);
            } else if self.options.comma_expressions
                && previous_expressions.is_some()
                && is_comma_eligible_statement(&statement)
            {
                let (start, mut expressions) = previous_expressions
                    .take()
                    .expect("comma expression candidate was checked");
                expressions.push(JsExpression::raw(
                    statement
                        .strip_suffix(';')
                        .expect("eligible expression statements end in semicolons"),
                    JsPrecedence::Assignment,
                ));
                out.truncate(start);
                out.push_str(&JsExpression::comma(expressions.clone()).into_minimal());
                out.push(';');
                previous_expressions = Some((start, expressions));
            } else {
                out.push_str(&statement);
            }
            previous_binding = binding;
            if !binding && is_comma_eligible_statement(&statement) && previous_expressions.is_none()
            {
                previous_expressions = Some((
                    statement_start,
                    vec![JsExpression::raw(
                        statement
                            .strip_suffix(';')
                            .expect("eligible expression statements end in semicolons"),
                        JsPrecedence::Assignment,
                    )],
                ));
            } else if binding || !is_comma_eligible_statement(&statement) {
                previous_expressions = None;
            }
        }
        match block.terminator.as_ref() {
            Some(Terminator::Return(Some(value))) => {
                out.push_str("return ");
                out.push_str(&strip_outer_parens(take_value(
                    *value, &context, &mut cache,
                )?));
                out.push(';');
            }
            Some(Terminator::Return(None)) if function.kind != FunctionKind::Entry => {
                if function.return_type != Type::Void {
                    return Err(CodegenError::new(
                        function.span,
                        "non-void IR function has no value",
                    ));
                }
            }
            Some(Terminator::Return(None)) => {}
            Some(Terminator::Unreachable) => out.push_str("throw Error();"),
            _ => {
                return Err(CodegenError::new(
                    block.span,
                    "single-block function has a control-flow terminator",
                ));
            }
        }
        if wrapped {
            if self.options.elide_block_terminal_semicolons && out.ends_with(';') {
                out.pop();
            }
            out.push('}');
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_linear_instruction(
        &mut self,
        instruction: &ControlFlowInstruction<'src>,
        uses: &AHashMap<ValueId, usize>,
        fuse_with_next: bool,
        predeclared: bool,
        context: &LocalNames,
        cache: &mut ExpressionCache,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        match &instruction.op {
            ControlFlowOp::StoreGlobal { global, value } => {
                if self.constant_global_strings.contains_key(global) {
                    let _ = take_value(*value, context, cache)?;
                    return Ok(());
                }
                let value = strip_outer_parens(take_value(*value, context, cache)?);
                if self.declared_globals.insert(*global) {
                    out.push_str(if self.deferred_global_declarations.contains(global) {
                        "var "
                    } else {
                        "let "
                    });
                }
                out.push_str(self.global_name(*global)?);
                out.push('=');
                out.push_str(&value);
                out.push(';');
                return Ok(());
            }
            ControlFlowOp::FieldSet {
                object,
                owner,
                field,
                index,
                value,
                ..
            } => {
                out.push_str(&take_value(*object, context, cache)?);
                if (self.options.public_aggregate_fields && context.is_untyped(*object))
                    || self.class_uses_named_fields(owner)
                {
                    write!(out, ".{}=", self.property_name(field))
                        .expect("writing to String cannot fail");
                } else {
                    write!(out, "[{index}]=").expect("writing to String cannot fail");
                }
                out.push_str(&strip_outer_parens(take_value(*value, context, cache)?));
                out.push(';');
                return Ok(());
            }
            ControlFlowOp::IndexSet {
                object,
                index,
                value,
            } => {
                out.push_str(&take_value(*object, context, cache)?);
                out.push('[');
                out.push_str(&strip_outer_parens(take_value(*index, context, cache)?));
                out.push_str("]=");
                out.push_str(&strip_outer_parens(take_value(*value, context, cache)?));
                out.push(';');
                return Ok(());
            }
            ControlFlowOp::NewClass {
                class,
                constructor: Some(constructor),
                args,
            } => {
                let result = instruction.out.ok_or_else(|| {
                    CodegenError::new(instruction.span, "class construction has no result")
                })?;
                let name = context.value_name(result)?;
                emit_binding_prefix(context, result, predeclared, out)?;
                out.push_str(name);
                out.push('=');
                out.push_str(&self.default_class_value(class, context.is_untyped(result))?);
                out.push(';');
                out.push_str(self.function_name(*constructor)?);
                out.push('(');
                out.push_str(name);
                for arg in args {
                    out.push(',');
                    out.push_str(&strip_outer_parens(take_value(*arg, context, cache)?));
                }
                out.push_str(");");
                return Ok(());
            }
            _ => {}
        }

        if instruction
            .out
            .is_some_and(|value| context.inlined_values.contains_key(&value))
        {
            return Ok(());
        }

        if let (Some(output), ControlFlowOp::LoadGlobal(global)) =
            (instruction.out, &instruction.op)
        {
            if uses.get(&output).copied() == Some(1)
                && self.constant_global_strings.contains_key(global)
            {
                let expression = self.render_instruction_op(instruction, context, cache)?;
                cache.insert(output, expression);
                return Ok(());
            }
        }

        if self.emit_in_place_update(instruction, context, cache, out)? {
            return Ok(());
        }

        let expression = self.render_instruction_op(instruction, context, cache)?;
        let Some(out_value) = instruction.out else {
            if !expression.is_empty() {
                out.push_str(&expression);
                out.push(';');
            }
            return Ok(());
        };
        let use_count = uses.get(&out_value).copied().unwrap_or(0);
        if use_count == 0 {
            if op_has_side_effects(&instruction.op) {
                out.push_str(&expression);
                out.push(';');
            }
        } else if use_count == 1
            && !context.is_stored(out_value)
            && (op_can_defer(&instruction.op) || fuse_with_next)
        {
            cache.insert(out_value, expression);
        } else {
            emit_binding_prefix(context, out_value, predeclared, out)?;
            out.push_str(context.value_name(out_value)?);
            out.push('=');
            out.push_str(&strip_outer_parens(expression));
            out.push(';');
        }
        Ok(())
    }

    fn emit_in_place_update(
        &self,
        instruction: &ControlFlowInstruction<'src>,
        context: &LocalNames,
        cache: &mut ExpressionCache,
        out: &mut String,
    ) -> Result<bool, CodegenError> {
        if self.options.mutation_spelling == MutationSpelling::Assignment {
            return Ok(false);
        }
        let Some(output) = instruction.out else {
            return Ok(false);
        };
        let ControlFlowOp::Binary { op, lhs, rhs } = instruction.op else {
            return Ok(false);
        };
        let output_name = context.value_name(output)?;
        if !context.is_name_declared(output) {
            return Ok(false);
        }
        let lhs_is_output = context.value_name(lhs).ok() == Some(output_name)
            && context.is_safe_in_place_update(output, lhs);
        let rhs_is_output = context.value_name(rhs).ok() == Some(output_name)
            && context.is_safe_in_place_update(output, rhs);
        let commutative = matches!(
            op,
            IrBinaryOp::Add
                | IrBinaryOp::Mul
                | IrBinaryOp::BitAnd
                | IrBinaryOp::BitOr
                | IrBinaryOp::Xor
        );
        let operand = if lhs_is_output {
            rhs
        } else if rhs_is_output && commutative {
            lhs
        } else {
            return Ok(false);
        };
        let integer_safe = context.can_elide_i32_coercion(output)
            || matches!(
                op,
                IrBinaryOp::BitAnd
                    | IrBinaryOp::BitOr
                    | IrBinaryOp::Xor
                    | IrBinaryOp::ShiftLeft
                    | IrBinaryOp::ShiftRight
            );
        if !matches!(instruction.ty, Some(Type::Float) | Some(Type::String))
            && !(instruction.ty == Some(Type::Int) && integer_safe)
        {
            return Ok(false);
        }
        let mut trial_cache = cache.clone();
        let operand = strip_outer_parens(take_value(operand, context, &mut trial_cache)?);
        if operand == "1"
            && matches!(op, IrBinaryOp::Add | IrBinaryOp::Sub)
            && matches!(
                self.options.mutation_spelling,
                MutationSpelling::Prefix | MutationSpelling::Postfix
            )
        {
            let operator = if op == IrBinaryOp::Add { "++" } else { "--" };
            if self.options.mutation_spelling == MutationSpelling::Prefix {
                out.push_str(operator);
            }
            out.push_str(output_name);
            if self.options.mutation_spelling == MutationSpelling::Postfix {
                out.push_str(operator);
            }
            out.push(';');
            *cache = trial_cache;
            return Ok(true);
        }
        if self.options.mutation_spelling != MutationSpelling::Compound {
            return Ok(false);
        }
        let operator = match op {
            IrBinaryOp::Add => "+",
            IrBinaryOp::Sub => "-",
            IrBinaryOp::Mul => "*",
            IrBinaryOp::Div => "/",
            IrBinaryOp::Mod => "%",
            IrBinaryOp::BitAnd => "&",
            IrBinaryOp::BitOr => "|",
            IrBinaryOp::Xor => "^",
            IrBinaryOp::ShiftLeft => "<<",
            IrBinaryOp::ShiftRight => ">>",
            _ => return Ok(false),
        };
        out.push_str(output_name);
        out.push_str(operator);
        out.push('=');
        out.push_str(&operand);
        out.push(';');
        *cache = trial_cache;
        Ok(true)
    }

    fn emit_state_machine(
        &mut self,
        function: &ControlFlowFunction<'src>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let local_mangler = self.local_mangler(function);
        let context = LocalNames::new(
            function,
            self.integer_analysis.function(function.id),
            true,
            &local_mangler,
            &self.preferred_local_names,
            &self.numeric_aliases,
            &self.options,
        );
        self.emit_state_machine_with_context(function, context, out)
    }

    fn emit_state_machine_with_context(
        &mut self,
        function: &ControlFlowFunction<'src>,
        context: LocalNames,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        out.push('{');
        let declared = context.non_parameter_names(function);
        if !declared.is_empty() {
            out.push_str("let ");
            for (index, name) in declared.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                out.push_str(name);
            }
            out.push(';');
        }
        let state = context.state_name();
        out.push_str("let ");
        out.push_str(state);
        match self.options.state_machine_spelling {
            StateMachineSpelling::Switch => {
                write!(out, "={};for(;;)switch({state}){{", function.entry.0)
                    .expect("writing to String cannot fail");
            }
            StateMachineSpelling::Conditional => {
                write!(out, "={};for(;;){{", function.entry.0)
                    .expect("writing to String cannot fail");
            }
        }

        let uses = &context.use_counts;
        for block in &function.blocks {
            match self.options.state_machine_spelling {
                StateMachineSpelling::Switch => {
                    write!(out, "case {}:", block.id.0).expect("writing to String cannot fail");
                }
                StateMachineSpelling::Conditional => {
                    write!(out, "if({state}=={}){{", block.id.0)
                        .expect("writing to String cannot fail");
                }
            }
            let mut cache = AHashMap::new();
            for (index, instruction) in block.instructions.iter().enumerate() {
                let fuse_with_next = instruction.out.is_some_and(|value| {
                    uses.get(&value).copied().unwrap_or(0) == 1
                        && can_fuse_value(block, index, value)
                });
                self.emit_linear_instruction(
                    instruction,
                    uses,
                    fuse_with_next,
                    true,
                    &context,
                    &mut cache,
                    out,
                )?;
            }
            match block
                .terminator
                .as_ref()
                .ok_or_else(|| CodegenError::new(block.span, "IR block has no terminator"))?
            {
                Terminator::Jump(target) => {
                    self.emit_phi_edge_cached(
                        function, block.id.0, target.0, &context, &mut cache, out,
                    )?;
                    write!(out, "{state}={};continue;", target.0)
                        .expect("writing to String cannot fail");
                }
                Terminator::Branch {
                    condition,
                    then_block,
                    else_block,
                } => {
                    let condition = take_value(*condition, &context, &mut cache)?;
                    out.push_str("if(");
                    out.push_str(&condition);
                    out.push_str("){");
                    let mut then_cache = cache.clone();
                    self.emit_phi_edge_cached(
                        function,
                        block.id.0,
                        then_block.0,
                        &context,
                        &mut then_cache,
                        out,
                    )?;
                    write!(out, "{state}={}", then_block.0).expect("writing to String cannot fail");
                    out.push_str("}else{");
                    self.emit_phi_edge_cached(
                        function,
                        block.id.0,
                        else_block.0,
                        &context,
                        &mut cache,
                        out,
                    )?;
                    write!(out, "{state}={}", else_block.0).expect("writing to String cannot fail");
                    out.push_str("}continue;");
                }
                Terminator::Return(Some(value)) => {
                    out.push_str("return ");
                    out.push_str(&take_value(*value, &context, &mut cache)?);
                    out.push(';');
                }
                Terminator::Return(None) => out.push_str("return;"),
                Terminator::Unreachable => out.push_str("throw Error();"),
            }
            if self.options.state_machine_spelling == StateMachineSpelling::Conditional {
                if self.options.elide_block_terminal_semicolons && out.ends_with(';') {
                    out.pop();
                }
                out.push('}');
            }
        }
        if self.options.elide_block_terminal_semicolons && out.ends_with(';') {
            out.pop();
        }
        out.push_str("}}");
        Ok(())
    }

    fn emit_structured(
        &mut self,
        function: &ControlFlowFunction<'src>,
        wrapped: bool,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let local_mangler = self.local_mangler(function);
        let mut context = LocalNames::new(
            function,
            self.integer_analysis.function(function.id),
            false,
            &local_mangler,
            &self.preferred_local_names,
            &self.numeric_aliases,
            &self.options,
        );
        context.inline_declarations = true;
        self.emit_structured_with_context(function, wrapped, context, out)
    }

    fn emit_structured_with_context(
        &mut self,
        function: &ControlFlowFunction<'src>,
        wrapped: bool,
        context: LocalNames,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        if wrapped {
            out.push('{');
        }
        let declared = context.non_parameter_names(function);
        if !context.inline_declarations && !declared.is_empty() {
            out.push_str("let ");
            for (index, name) in declared.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                out.push_str(name);
            }
            out.push(';');
        }
        let uses = &context.use_counts;
        let mut visited = AHashSet::new();
        let mut cache = AHashMap::new();
        self.emit_structured_path(
            function,
            function.entry,
            None,
            None,
            &context,
            uses,
            &mut cache,
            &mut visited,
            out,
        )?;
        if out.ends_with("return;") {
            out.truncate(out.len() - "return;".len());
        }
        if wrapped {
            if self.options.elide_block_terminal_semicolons && out.ends_with(';') {
                out.pop();
            }
            out.push('}');
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_structured_path(
        &mut self,
        function: &ControlFlowFunction<'src>,
        mut current: BlockId,
        stop: Option<BlockId>,
        loop_context: Option<LoopContext>,
        context: &LocalNames,
        uses: &AHashMap<ValueId, usize>,
        cache: &mut ExpressionCache,
        visited: &mut AHashSet<BlockId>,
        out: &mut String,
    ) -> Result<PathEnd, CodegenError> {
        loop {
            if Some(current) == stop {
                return Ok(PathEnd::ReachedStop);
            }
            if !visited.insert(current) {
                return Err(CodegenError::new(
                    function.blocks[current.0 as usize].span,
                    "structured CFG traversal encountered an unexpected cycle",
                ));
            }

            if let Some(shape) = shape_at(function, current) {
                let retained_condition = match &shape {
                    ControlShape::If { header, .. } => {
                        let block = &function.blocks[header.0 as usize];
                        if block.instructions.is_empty() {
                            match block.terminator {
                                Some(Terminator::Branch { condition, .. })
                                    if cache.contains_key(&condition) =>
                                {
                                    Some(condition)
                                }
                                _ => None,
                            }
                        } else {
                            None
                        }
                    }
                    ControlShape::Loop { .. } | ControlShape::ForIn { .. } => None,
                };
                self.flush_cache_except(cache, context, out, retained_condition)?;
                match shape {
                    ControlShape::If {
                        header,
                        then_block,
                        else_block,
                        merge_block,
                    } => {
                        let block = &function.blocks[header.0 as usize];
                        self.emit_cached_block(block, uses, context, cache, out)?;
                        let Some(Terminator::Branch { condition, .. }) = block.terminator else {
                            return Err(CodegenError::new(
                                block.span,
                                "if shape header is not a branch",
                            ));
                        };
                        let condition_expression = take_value(condition, context, cache)?;
                        let negated_condition = condition_expression.clone().negated();
                        let condition = condition_expression.into_condition();
                        if is_true_literal(&condition) || is_false_literal(&condition) {
                            let selected = if is_true_literal(&condition) {
                                then_block
                            } else {
                                else_block
                            };
                            let mut selected_visited = visited.clone();
                            let mut selected_cache = cache.clone();
                            let selected_end = self.emit_structured_path(
                                function,
                                selected,
                                Some(merge_block),
                                loop_context,
                                context,
                                uses,
                                &mut selected_cache,
                                &mut selected_visited,
                                out,
                            )?;
                            if selected_end == PathEnd::Terminated {
                                return Ok(PathEnd::Terminated);
                            }
                            cache.clear();
                            current = merge_block;
                            continue;
                        }
                        let mut then_visited = visited.clone();
                        let mut then_cache = cache.clone();
                        let mut then_output = String::new();
                        self.emit_structured_path(
                            function,
                            then_block,
                            Some(merge_block),
                            loop_context,
                            context,
                            uses,
                            &mut then_cache,
                            &mut then_visited,
                            &mut then_output,
                        )?;
                        let mut else_visited = visited.clone();
                        let mut else_cache = cache.clone();
                        let mut else_output = String::new();
                        self.emit_structured_path(
                            function,
                            else_block,
                            Some(merge_block),
                            loop_context,
                            context,
                            uses,
                            &mut else_cache,
                            &mut else_visited,
                            &mut else_output,
                        )?;
                        let mut deferred_merge = None;
                        if let Some((declare, target, then_value, else_value, trailing)) = self
                            .options
                            .conditional_expressions
                            .then(|| merge_conditional_assignments(&then_output, &else_output))
                            .flatten()
                        {
                            let mut value = String::new();
                            if is_true_literal(then_value) && is_false_literal(else_value) {
                                value.push_str(&condition);
                            } else if is_false_literal(then_value) && is_true_literal(else_value) {
                                value.push_str(&negated_condition);
                            } else if is_true_literal(then_value) {
                                push_logical_operand(&mut value, &condition, IrBinaryOp::Or);
                                value.push_str("||");
                                push_logical_operand(&mut value, else_value, IrBinaryOp::Or);
                            } else if is_false_literal(else_value) {
                                push_logical_operand(&mut value, &condition, IrBinaryOp::And);
                                value.push_str("&&");
                                push_logical_operand(&mut value, then_value, IrBinaryOp::And);
                            } else {
                                value.push_str(&condition);
                                value.push('?');
                                value.push_str(then_value);
                                value.push(':');
                                value.push_str(else_value);
                            }
                            let declaration_tail = if trailing.is_empty() {
                                Some(None)
                            } else if declare {
                                uninitialized_declaration_tail(trailing).map(Some)
                            } else {
                                None
                            };
                            let deferred = if declaration_tail.is_some() {
                                function.blocks[merge_block.0 as usize]
                                    .phis
                                    .iter()
                                    .find(|phi| {
                                        context.value_name(phi.out).ok() == Some(target)
                                            && uses.get(&phi.out).copied() == Some(1)
                                            && immediately_branches_on_phi(
                                                function,
                                                merge_block,
                                                phi.out,
                                            )
                                    })
                                    .map(|phi| phi.out)
                            } else {
                                None
                            };
                            if let Some(value_id) = deferred {
                                if declare {
                                    out.push_str("var ");
                                    out.push_str(target);
                                    out.push_str(trailing);
                                    out.push(';');
                                }
                                deferred_merge = Some((value_id, value));
                            } else {
                                if declare {
                                    out.push_str("var ");
                                }
                                out.push_str(target);
                                out.push('=');
                                out.push_str(&value);
                                out.push_str(trailing);
                                out.push(';');
                            }
                        } else if let Some((then_target, then_value, else_target, else_value)) =
                            self.options
                                .conditional_expressions
                                .then(|| {
                                    conditional_assignment_expression(&then_output, &else_output)
                                })
                                .flatten()
                        {
                            out.push_str(&condition);
                            out.push('?');
                            out.push_str(then_target);
                            out.push('=');
                            out.push_str(then_value);
                            out.push(':');
                            out.push_str(else_target);
                            out.push('=');
                            out.push_str(else_value);
                            out.push(';');
                        } else if let Some((then_expression, else_expression)) = self
                            .options
                            .conditional_expressions
                            .then(|| {
                                Some((
                                    compact_branch_expression(&then_output)?,
                                    compact_branch_expression(&else_output)?,
                                ))
                            })
                            .flatten()
                        {
                            out.push_str(&condition);
                            out.push('?');
                            push_conditional_arm(out, then_expression);
                            out.push(':');
                            push_conditional_arm(out, else_expression);
                            out.push(';');
                        } else if else_output.is_empty() {
                            if self.options.conditional_expressions
                                && compact_branch_expression(&then_output).is_some()
                            {
                                push_logical_operand(out, &condition, IrBinaryOp::And);
                                out.push_str("&&");
                                push_logical_operand(
                                    out,
                                    compact_branch_expression(&then_output)
                                        .expect("branch expression was checked"),
                                    IrBinaryOp::And,
                                );
                                out.push(';');
                            } else {
                                out.push_str("if(");
                                out.push_str(&condition);
                                if is_braceless_statement(&then_output) {
                                    out.push(')');
                                    out.push_str(&then_output);
                                } else {
                                    out.push_str("){");
                                    out.push_str(&then_output);
                                    out.push('}');
                                }
                            }
                        } else if then_output.is_empty() {
                            if self.options.conditional_expressions
                                && compact_branch_expression(&else_output).is_some()
                            {
                                push_logical_operand(out, &negated_condition, IrBinaryOp::And);
                                out.push_str("&&");
                                push_logical_operand(
                                    out,
                                    compact_branch_expression(&else_output)
                                        .expect("branch expression was checked"),
                                    IrBinaryOp::And,
                                );
                                out.push(';');
                            } else {
                                out.push_str("if(");
                                out.push_str(&negated_condition);
                                if is_braceless_statement(&else_output) {
                                    out.push(')');
                                    out.push_str(&else_output);
                                } else {
                                    out.push_str("){");
                                    out.push_str(&else_output);
                                    out.push('}');
                                }
                            }
                        } else {
                            out.push_str("if(");
                            out.push_str(&condition);
                            out.push_str("){");
                            out.push_str(&then_output);
                            out.push_str("}else{");
                            out.push_str(&else_output);
                            out.push('}');
                        }
                        cache.clear();
                        if let Some((value, expression)) = deferred_merge {
                            cache.insert(
                                value,
                                JsExpression::raw(expression, JsPrecedence::Conditional),
                            );
                        }
                        current = merge_block;
                        continue;
                    }
                    ControlShape::Loop {
                        header,
                        body,
                        update,
                        exit,
                    } => {
                        let condition_block = loop_condition_branch(function, header, body, exit)
                            .ok_or_else(|| {
                            CodegenError::new(
                                function.blocks[header.0 as usize].span,
                                "loop condition does not branch to its body and exit",
                            )
                        })?;
                        let mut header_output = String::new();
                        if condition_block != header {
                            let outer_shape = ControlShape::Loop {
                                header,
                                body,
                                update,
                                exit,
                            };
                            let mut condition_function = function.clone();
                            condition_function
                                .shapes
                                .retain(|shape| shape != &outer_shape);
                            let mut condition_visited = AHashSet::new();
                            let condition_end = self.emit_structured_path(
                                &condition_function,
                                header,
                                Some(condition_block),
                                None,
                                context,
                                uses,
                                cache,
                                &mut condition_visited,
                                &mut header_output,
                            )?;
                            if condition_end != PathEnd::ReachedStop {
                                return Err(CodegenError::new(
                                    function.blocks[header.0 as usize].span,
                                    "loop short-circuit condition did not reach its final branch",
                                ));
                            }
                        }
                        let block = &function.blocks[condition_block.0 as usize];
                        let Some(Terminator::Branch {
                            condition,
                            then_block,
                            else_block,
                        }) = block.terminator
                        else {
                            return Err(CodegenError::new(
                                block.span,
                                "loop shape header is not a branch",
                            ));
                        };
                        let body_on_true = then_block == body && else_block == exit;
                        let body_on_false = else_block == body && then_block == exit;
                        if !body_on_true && !body_on_false {
                            return Err(CodegenError::new(
                                block.span,
                                "loop branch does not target its body and exit",
                            ));
                        }
                        self.emit_cached_block(block, uses, context, cache, &mut header_output)?;
                        let rotation_counter = positive_counter_condition(function, condition)
                            .filter(|counter| {
                                value_is_unused_until_block(
                                    function,
                                    exit,
                                    header,
                                    *counter,
                                    ValueId(u32::MAX),
                                )
                            });
                        let condition_expression = take_value(condition, context, cache)?;
                        let negated_condition = condition_expression.clone().negated();
                        let condition = condition_expression.into_condition();
                        let loop_condition_is_constant_true = (body_on_true
                            && is_true_literal(&condition))
                            || (body_on_false && is_false_literal(&condition));
                        let mut exit_output = String::new();
                        let mut exit_cache = cache.clone();
                        self.emit_phi_edge_cached(
                            function,
                            condition_block.0,
                            exit.0,
                            context,
                            &mut exit_cache,
                            &mut exit_output,
                        )?;
                        let compact_loop = header_output.is_empty() && exit_output.is_empty();
                        let do_loop =
                            compact_loop && self.options.loop_spelling == LoopSpelling::Do;
                        let update_clause =
                            if compact_loop && self.options.update_loop_layout && !do_loop {
                                if let Some(update_block) = update {
                                    let mut update_visited = AHashSet::new();
                                    let mut update_cache = AHashMap::new();
                                    let mut update_output = String::new();
                                    let update_end = self.emit_structured_path(
                                        function,
                                        update_block,
                                        Some(header),
                                        None,
                                        context,
                                        uses,
                                        &mut update_cache,
                                        &mut update_visited,
                                        &mut update_output,
                                    )?;
                                    (update_end == PathEnd::ReachedStop)
                                        .then(|| for_update_clause(&update_output))
                                        .flatten()
                                } else {
                                    None
                                }
                            } else {
                                None
                            };
                        let reuse_for_spelling = compact_loop
                            && update_clause.is_none()
                            && match self.options.loop_spelling {
                                LoopSpelling::Auto => {
                                    out.matches("for(").count() > out.matches("while(").count()
                                }
                                LoopSpelling::While => false,
                                LoopSpelling::For => true,
                                LoopSpelling::Do => false,
                            };
                        let for_initializer = (!do_loop
                            && (update_clause.is_some() || reuse_for_spelling)
                            && self.options.comma_expressions)
                            .then(|| take_trailing_expression_statements(out))
                            .flatten();
                        let do_condition = if do_loop {
                            let condition = if body_on_true {
                                condition.clone()
                            } else {
                                negated_condition.clone()
                            };
                            out.push_str("if(");
                            out.push_str(&condition);
                            out.push_str(")do{");
                            Some(condition)
                        } else if let Some(update_clause) = &update_clause {
                            out.push_str("for(");
                            if let Some(initializer) = &for_initializer {
                                out.push_str(initializer);
                            }
                            out.push(';');
                            if body_on_true {
                                out.push_str(&condition);
                            } else {
                                out.push_str(&negated_condition);
                            }
                            out.push(';');
                            out.push_str(update_clause);
                            out.push_str("){");
                            None
                        } else if compact_loop {
                            if reuse_for_spelling {
                                out.push_str("for(");
                                if let Some(initializer) = &for_initializer {
                                    out.push_str(initializer);
                                }
                                out.push(';');
                            } else {
                                out.push_str("while(");
                            }
                            if body_on_true {
                                out.push_str(&condition);
                            } else {
                                out.push_str(&negated_condition);
                            }
                            if reuse_for_spelling {
                                out.push(';');
                            }
                            out.push_str("){");
                            None
                        } else {
                            out.push_str("for(;;){");
                            out.push_str(&header_output);
                            out.push_str("if(");
                            if body_on_true {
                                out.push_str(&negated_condition);
                            } else {
                                out.push_str(&condition);
                            }
                            out.push_str("){");
                            out.push_str(&exit_output);
                            out.push_str("break}");
                            None
                        };
                        let loop_body_open = (compact_loop && !do_loop).then_some(out.len() - 1);
                        let loop_body_content_start = compact_loop.then_some(out.len());

                        let continue_target = update.unwrap_or(header);
                        let nested_loop = LoopContext {
                            header,
                            continue_target,
                            update: update_clause.is_none().then_some(update).flatten(),
                            exit,
                        };
                        let mut body_visited = visited.clone();
                        let mut body_cache = cache.clone();
                        let body_end = self.emit_structured_path(
                            function,
                            body,
                            Some(continue_target),
                            Some(nested_loop),
                            context,
                            uses,
                            &mut body_cache,
                            &mut body_visited,
                            out,
                        )?;
                        if body_end == PathEnd::ReachedStop && update_clause.is_none() {
                            if let Some(update_block) = update {
                                let mut update_visited = AHashSet::new();
                                let mut update_cache = body_cache;
                                self.emit_structured_path(
                                    function,
                                    update_block,
                                    Some(header),
                                    None,
                                    context,
                                    uses,
                                    &mut update_cache,
                                    &mut update_visited,
                                    out,
                                )?;
                            }
                        }
                        let mut compacted_loop_body = false;
                        if self.options.comma_expressions {
                            if let Some(body_start) = loop_body_content_start {
                                if let Some(compact) =
                                    compact_top_level_expression_statements(&out[body_start..])
                                {
                                    out.replace_range(body_start.., &compact);
                                    compacted_loop_body = true;
                                }
                            }
                        }
                        if let Some(condition) = do_condition {
                            out.push_str("}while(");
                            out.push_str(&condition);
                            out.push_str(");");
                        } else if loop_body_open.is_some_and(|open| {
                            compacted_loop_body || is_braceless_statement(&out[open + 1..])
                        }) {
                            out.remove(loop_body_open.expect("checked loop body opening"));
                        } else {
                            out.push('}');
                        }
                        if let Some(counter) = rotation_counter {
                            rewrite_guarded_decrement_loop(
                                out,
                                loop_body_open,
                                context.value_name(counter)?,
                            );
                        }
                        cache.clear();
                        if loop_condition_is_constant_true
                            && !loop_body_reaches_exit(function, body, header, exit)
                        {
                            return Ok(PathEnd::Terminated);
                        }
                        current = exit;
                        continue;
                    }
                    ControlShape::ForIn {
                        header,
                        body,
                        exit,
                        object,
                        key,
                    } => {
                        let block = &function.blocks[header.0 as usize];
                        if !matches!(block.terminator, Some(Terminator::Branch {
                            then_block,
                            else_block,
                            ..
                        }) if then_block == body && else_block == exit)
                        {
                            return Err(CodegenError::new(
                                block.span,
                                "for-in shape header does not branch to its body and exit",
                            ));
                        }
                        let object = context.value_name(object)?;
                        let declare_key = context.claim_declaration(key)?;
                        let key = context.value_name(key)?;
                        out.push_str("for(");
                        if declare_key {
                            out.push_str("var ");
                        }
                        out.push_str(key);
                        out.push_str(" in ");
                        out.push_str(object);
                        out.push_str("){");

                        let nested_loop = LoopContext {
                            header,
                            continue_target: header,
                            update: None,
                            exit,
                        };
                        let mut body_visited = visited.clone();
                        let mut body_cache = cache.clone();
                        self.emit_structured_path(
                            function,
                            body,
                            Some(header),
                            Some(nested_loop),
                            context,
                            uses,
                            &mut body_cache,
                            &mut body_visited,
                            out,
                        )?;
                        out.push('}');
                        cache.clear();
                        current = exit;
                        continue;
                    }
                }
            }

            let block = &function.blocks[current.0 as usize];
            self.emit_cached_block(block, uses, context, cache, out)?;
            match block
                .terminator
                .as_ref()
                .ok_or_else(|| CodegenError::new(block.span, "IR block has no terminator"))?
            {
                Terminator::Jump(target) => {
                    self.emit_phi_edge_cached(function, current.0, target.0, context, cache, out)?;
                    if Some(*target) == stop {
                        return Ok(PathEnd::ReachedStop);
                    }
                    if let Some(loop_context) = loop_context {
                        if *target == loop_context.exit {
                            out.push_str("break;");
                            return Ok(PathEnd::Terminated);
                        }
                        if *target == loop_context.continue_target {
                            if let Some(update) = loop_context.update {
                                if current != update {
                                    let mut update_visited = AHashSet::new();
                                    let mut update_cache = AHashMap::new();
                                    self.emit_structured_path(
                                        function,
                                        update,
                                        Some(loop_context.header),
                                        None,
                                        context,
                                        uses,
                                        &mut update_cache,
                                        &mut update_visited,
                                        out,
                                    )?;
                                }
                            }
                            out.push_str("continue;");
                            return Ok(PathEnd::Terminated);
                        }
                    }
                    current = *target;
                }
                Terminator::Return(Some(value)) => {
                    out.push_str("return ");
                    out.push_str(&strip_outer_parens(take_value(*value, context, cache)?));
                    out.push(';');
                    return Ok(PathEnd::Terminated);
                }
                Terminator::Return(None) => {
                    if function.kind != FunctionKind::Entry {
                        out.push_str("return;");
                    }
                    return Ok(PathEnd::Terminated);
                }
                Terminator::Unreachable => {
                    out.push_str("throw Error();");
                    return Ok(PathEnd::Terminated);
                }
                Terminator::Branch { .. } => {
                    return Err(CodegenError::new(
                        block.span,
                        "branch block has no structured shape",
                    ));
                }
            }
        }
    }

    fn emit_cached_block(
        &mut self,
        block: &crate::ir::ControlFlowBlock<'src>,
        uses: &AHashMap<ValueId, usize>,
        context: &LocalNames,
        cache: &mut ExpressionCache,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        for (index, instruction) in block.instructions.iter().enumerate() {
            let fuse_with_next = instruction.out.is_some_and(|value| {
                uses.get(&value).copied().unwrap_or(0) == 1 && can_fuse_value(block, index, value)
            });
            self.emit_linear_instruction(
                instruction,
                uses,
                fuse_with_next,
                true,
                context,
                cache,
                out,
            )?;
        }
        Ok(())
    }

    fn flush_cache_except(
        &self,
        cache: &mut ExpressionCache,
        context: &LocalNames,
        out: &mut String,
        retained: Option<ValueId>,
    ) -> Result<(), CodegenError> {
        let retained =
            retained.and_then(|value| cache.remove(&value).map(|expression| (value, expression)));
        let mut values = cache.drain().collect::<Vec<_>>();
        values.sort_by_key(|(value, _)| value.0);
        for (value, expression) in values {
            if context.claim_declaration(value)? {
                out.push_str("var ");
            }
            out.push_str(context.value_name(value)?);
            out.push('=');
            out.push_str(&strip_outer_parens(expression));
            out.push(';');
        }
        if let Some((value, expression)) = retained {
            cache.insert(value, expression);
        }
        Ok(())
    }

    fn emit_phi_edge_cached(
        &self,
        function: &ControlFlowFunction<'src>,
        from: u32,
        to: u32,
        context: &LocalNames,
        cache: &mut ExpressionCache,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let copies = function.blocks[to as usize]
            .phis
            .iter()
            .filter_map(|phi| {
                phi.incoming
                    .iter()
                    .find(|(block, _)| block.0 == from)
                    .map(|(_, value)| (phi.out, *value))
            })
            .collect::<Vec<_>>();
        let mut sources = Vec::with_capacity(copies.len());
        for (_, source) in &copies {
            sources.push(strip_outer_parens(take_value(*source, context, cache)?));
        }
        let mut assignments = Vec::with_capacity(copies.len());
        let mut single_assignment_copy = None;
        let mut declaration_needed = false;
        for ((target, source_value), source) in copies.iter().zip(sources) {
            let target_value = *target;
            let target = context.value_name(target_value)?.to_string();
            if target != source {
                declaration_needed |= context.claim_declaration(target_value)?;
                single_assignment_copy = assignments
                    .is_empty()
                    .then_some((target_value, *source_value));
                assignments.push((target, source));
            }
        }
        if assignments.len() == 1 {
            let compound_update = (!declaration_needed
                && self.options.mutation_spelling == MutationSpelling::Compound)
                .then(|| {
                    single_assignment_copy.and_then(|(target, source)| {
                        compound_assignment_copy(
                            function,
                            BlockId(from),
                            target,
                            source,
                            &context.use_counts,
                            context,
                        )
                    })
                })
                .flatten();
            if let Some((operator, operand)) = compound_update {
                out.push_str(&assignments[0].0);
                out.push_str(operator);
                out.push('=');
                out.push_str(&operand);
                out.push(';');
                return Ok(());
            }
            let compact_update = (!declaration_needed
                && matches!(
                    self.options.mutation_spelling,
                    MutationSpelling::Prefix | MutationSpelling::Postfix
                ))
            .then(|| {
                single_assignment_copy.and_then(|(target, source)| {
                    one_use_unit_update(
                        function,
                        BlockId(from),
                        target,
                        source,
                        &context.use_counts,
                        context,
                    )
                    .map(|delta| (self.options.mutation_spelling, delta))
                })
            })
            .flatten();
            if let Some((spelling, delta)) = compact_update {
                let operator = if delta > 0 { "++" } else { "--" };
                if spelling == MutationSpelling::Prefix {
                    out.push_str(operator);
                }
                out.push_str(&assignments[0].0);
                if spelling == MutationSpelling::Postfix {
                    out.push_str(operator);
                }
                out.push(';');
                return Ok(());
            }
            if declaration_needed {
                out.push_str("var ");
            }
            out.push_str(&assignments[0].0);
            out.push('=');
            out.push_str(&assignments[0].1);
            if declaration_needed {
                for name in context.claim_remaining_declarations() {
                    out.push(',');
                    out.push_str(&name);
                }
            }
            out.push(';');
        } else if !assignments.is_empty() {
            let targets = assignments
                .iter()
                .map(|(target, _)| target.as_str())
                .collect::<AHashSet<_>>();
            let scalar_declaration = declaration_needed
                && assignments
                    .iter()
                    .all(|(_, source)| !targets.contains(source.as_str()));
            if scalar_declaration {
                out.push_str("var ");
                for (index, (target, source)) in assignments.iter().enumerate() {
                    if index != 0 {
                        out.push(',');
                    }
                    out.push_str(target);
                    out.push('=');
                    out.push_str(source);
                }
                for name in context.claim_remaining_declarations() {
                    out.push(',');
                    out.push_str(&name);
                }
                out.push(';');
                return Ok(());
            }
            if !declaration_needed {
                let reusable_temporary = self
                    .options
                    .scalar_phi_copies
                    .then(|| reusable_parallel_copy_temporary(BlockId(to), context, &assignments));
                let temporary = reusable_temporary
                    .as_ref()
                    .and_then(|name| name.as_deref())
                    .map(|name| (name, false))
                    .or_else(|| {
                        context
                            .parallel_copy_temp
                            .as_deref()
                            .map(|name| (name, true))
                    });
                if let Some(scalar) = scalar_parallel_assignments(&assignments, temporary) {
                    let tuple_size = assignments
                        .iter()
                        .map(|(target, source)| target.len() + source.len())
                        .sum::<usize>()
                        + assignments.len().saturating_sub(1) * 2
                        + 6;
                    if self.options.scalar_phi_copies || scalar.len() < tuple_size {
                        out.push_str(&scalar);
                        return Ok(());
                    }
                }
            }
            if declaration_needed {
                out.push_str("var ");
            }
            out.push('[');
            for (index, (target, _)) in assignments.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                out.push_str(target);
            }
            out.push_str("]=[");
            for (index, (_, source)) in assignments.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                out.push_str(source);
            }
            out.push_str("];");
        }
        Ok(())
    }

    fn render_instruction_op(
        &mut self,
        instruction: &ControlFlowInstruction<'src>,
        context: &LocalNames,
        cache: &mut ExpressionCache,
    ) -> Result<JsExpression, CodegenError> {
        if instruction
            .out
            .is_some_and(|out| context.can_elide_map_get_normalization(out))
        {
            if let ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::MapGet,
                receiver: Some(receiver),
                args,
            } = &instruction.op
            {
                let receiver = take_value(*receiver, context, cache)?;
                return self.render_call(
                    JsExpression::member(
                        receiver,
                        "get",
                        self.options.elide_call_chain_parentheses,
                    ),
                    args,
                    context,
                    cache,
                );
            }
        }
        if instruction.ty.as_ref() == Some(&Type::Int) {
            let coercion_is_elidable = self.options.elide_safe_integer_coercions
                && instruction
                    .out
                    .is_some_and(|out| context.can_elide_i32_coercion(out));
            match &instruction.op {
                ControlFlowOp::Unary {
                    op: IrUnaryOp::Neg,
                    value,
                } => {
                    let value = take_value(*value, context, cache)?;
                    let negated = JsExpression::unary("-", value);
                    return Ok(if coercion_is_elidable {
                        negated
                    } else {
                        JsExpression::integer_normalization(negated)
                    });
                }
                ControlFlowOp::Binary { op, lhs, rhs }
                    if matches!(
                        op,
                        IrBinaryOp::Add
                            | IrBinaryOp::Sub
                            | IrBinaryOp::Mul
                            | IrBinaryOp::Div
                            | IrBinaryOp::Mod
                            | IrBinaryOp::BitAnd
                            | IrBinaryOp::BitOr
                            | IrBinaryOp::Xor
                            | IrBinaryOp::ShiftLeft
                            | IrBinaryOp::ShiftRight
                            | IrBinaryOp::UnsignedShiftRight
                    ) =>
                {
                    let mut lhs = take_value(*lhs, context, cache)?;
                    let rhs_value = *rhs;
                    let mut rhs = take_value(rhs_value, context, cache)?;
                    if matches!(
                        op,
                        IrBinaryOp::BitAnd
                            | IrBinaryOp::BitOr
                            | IrBinaryOp::Xor
                            | IrBinaryOp::ShiftLeft
                            | IrBinaryOp::ShiftRight
                            | IrBinaryOp::UnsignedShiftRight
                    ) {
                        lhs = lhs.without_integer_normalization();
                        rhs = rhs.without_integer_normalization();
                    }
                    let rhs_is_nonzero = is_nonzero_i32_literal(&rhs.code)
                        || context.integer_range_excludes_zero(rhs_value);
                    if *op == IrBinaryOp::Add {
                        let tilde_operand = if rhs.code == "1" && lhs.is_integer_normalization() {
                            Some(lhs.clone().without_integer_normalization())
                        } else if lhs.code == "1" && rhs.is_integer_normalization() {
                            Some(rhs.clone().without_integer_normalization())
                        } else {
                            None
                        };
                        if let Some(operand) = tilde_operand {
                            let incremented =
                                JsExpression::unary("-", JsExpression::unary("~", operand));
                            return Ok(if coercion_is_elidable {
                                incremented
                            } else {
                                JsExpression::integer_normalization(incremented)
                            });
                        }
                    }
                    let bitwise_arithmetic = self.options.elide_safe_integer_coercions
                        && bitwise_arithmetic_elides_coercion(*op, &lhs, &rhs);
                    let expression = JsExpression::binary(*op, lhs, rhs);
                    return Ok(match op {
                        IrBinaryOp::BitAnd
                        | IrBinaryOp::BitOr
                        | IrBinaryOp::Xor
                        | IrBinaryOp::ShiftLeft
                        | IrBinaryOp::ShiftRight
                        | IrBinaryOp::UnsignedShiftRight => expression,
                        IrBinaryOp::Mod if rhs_is_nonzero => expression,
                        _ if coercion_is_elidable || bitwise_arithmetic => expression,
                        _ => JsExpression::integer_normalization(expression),
                    });
                }
                ControlFlowOp::IndexGet { object, index } => {
                    let indexed = JsExpression::index(
                        take_value(*object, context, cache)?,
                        take_value(*index, context, cache)?,
                        self.options.elide_call_chain_parentheses,
                    );
                    return Ok(
                        if coercion_is_elidable || context.can_elide_int_array_read(instruction.out)
                        {
                            indexed
                        } else {
                            JsExpression::integer_normalization(indexed)
                        },
                    );
                }
                _ => {}
            }
        }
        let boundary = instruction.out.is_some_and(|out| context.is_untyped(out));
        match &instruction.op {
            ControlFlowOp::Struct { name, fields }
                if boundary && self.options.public_aggregate_fields =>
            {
                let layout = self
                    .module
                    .structs
                    .iter()
                    .find(|layout| layout.name == *name)
                    .ok_or_else(|| {
                        CodegenError::new(instruction.span, "missing boundary struct layout")
                    })?;
                let mut rendered = String::from("{");
                for (index, (field, value)) in layout.fields.iter().zip(fields).enumerate() {
                    if index != 0 {
                        rendered.push(',');
                    }
                    let property = self.property_name(field.name);
                    let value = take_value(*value, context, cache)?;
                    if self.options.struct_method_shorthand {
                        if let Some(method) = arrow_block_as_object_method(property, &value) {
                            rendered.push_str(&method);
                            continue;
                        }
                    }
                    rendered.push_str(property);
                    rendered.push(':');
                    rendered.push_str(&value);
                }
                rendered.push('}');
                Ok(JsExpression::atom(rendered))
            }
            ControlFlowOp::NewClass {
                class,
                constructor: None,
                args,
            } if boundary && args.is_empty() => self
                .default_class_value(class, true)
                .map(JsExpression::atom),
            ControlFlowOp::IndexGet { object, index }
                if instruction.ty.as_ref() == Some(&Type::String) =>
            {
                let in_bounds = context
                    .string_index_in_bounds(*object, *index)
                    .or_else(|| self.global_string_index_in_bounds(*object, *index, context));
                let indexed = JsExpression::index(
                    take_value(*object, context, cache)?,
                    take_value(*index, context, cache)?,
                    self.options.elide_call_chain_parentheses,
                );
                Ok(if in_bounds == Some(true) {
                    indexed
                } else {
                    JsExpression::binary(IrBinaryOp::Or, indexed, JsExpression::atom("\"\""))
                })
            }
            _ => self.render_op(&instruction.op, context, cache),
        }
    }

    fn render_op(
        &mut self,
        op: &ControlFlowOp<'src>,
        context: &LocalNames,
        cache: &mut ExpressionCache,
    ) -> Result<JsExpression, CodegenError> {
        let value = |id, cache: &mut ExpressionCache| take_value(id, context, cache);
        Ok(match op {
            ControlFlowOp::Const(ConstValue::String(value)) => JsExpression::atom(
                self.string_aliases
                    .get(value)
                    .cloned()
                    .unwrap_or_else(|| render_string_literal(value, self.options.string_quote)),
            ),
            ControlFlowOp::Const(value) => {
                let rendered = render_const(
                    value,
                    self.options.compact_boolean_literals,
                    self.options.string_quote,
                );
                JsExpression::atom(
                    self.numeric_aliases
                        .get(&rendered)
                        .cloned()
                        .unwrap_or(rendered),
                )
            }
            ControlFlowOp::Unary { op, value: operand } => JsExpression::unary(
                match op {
                    IrUnaryOp::Neg => "-",
                    IrUnaryOp::Not => "!",
                },
                value(*operand, cache)?,
            ),
            ControlFlowOp::Binary { op, lhs, rhs } => {
                let truthy_nullable = (self.options.truthy_nullable_checks
                    && matches!(op, IrBinaryOp::Eq | IrBinaryOp::NotEq))
                .then(|| context.truthy_nullable_operand(*lhs, *rhs))
                .flatten();
                let nullable_on_lhs = truthy_nullable == Some(*lhs);
                let lhs = value(*lhs, cache)?;
                let rhs = value(*rhs, cache)?;
                let tilde_increment = if *op == IrBinaryOp::Add {
                    if rhs.code == "1" && lhs.is_integer_normalization() {
                        Some(lhs.clone().without_integer_normalization())
                    } else if lhs.code == "1" && rhs.is_integer_normalization() {
                        Some(rhs.clone().without_integer_normalization())
                    } else {
                        None
                    }
                } else {
                    None
                };
                if truthy_nullable.is_some() {
                    let operand = if nullable_on_lhs { lhs } else { rhs };
                    let boolean = JsExpression::unary("!", operand);
                    if *op == IrBinaryOp::Eq {
                        boolean
                    } else {
                        JsExpression::unary("!", boolean)
                    }
                } else if let Some(operand) = tilde_increment {
                    JsExpression::unary("-", JsExpression::unary("~", operand))
                } else if matches!(op, IrBinaryOp::Eq | IrBinaryOp::NotEq)
                    && is_rendered_string_literal(&lhs.code)
                    && is_rendered_string_literal(&rhs.code)
                {
                    let equal = lhs.code == rhs.code;
                    JsExpression::atom(render_const(
                        &ConstValue::Bool(if *op == IrBinaryOp::Eq { equal } else { !equal }),
                        self.options.compact_boolean_literals,
                        self.options.string_quote,
                    ))
                } else {
                    JsExpression::binary(*op, lhs, rhs)
                }
            }
            ControlFlowOp::TypeCheck {
                value: input,
                target,
            } => JsExpression::raw(
                render_js_type_check(
                    &value(*input, cache)?.code,
                    target,
                    self.options.string_quote,
                )?,
                JsPrecedence::Equality,
            ),
            ControlFlowOp::Array(values) => {
                let mut rendered = String::from("[");
                for (index, item) in values.iter().enumerate() {
                    if index != 0 {
                        rendered.push(',');
                    }
                    rendered.push_str(&strip_outer_parens(value(*item, cache)?));
                }
                rendered.push(']');
                if self.options.pack_string_arrays {
                    JsExpression::atom(
                        packed_string_array(values, context, self.options.string_quote)
                            .filter(|packed| packed.len() < rendered.len())
                            .unwrap_or(rendered),
                    )
                } else {
                    JsExpression::atom(rendered)
                }
            }
            ControlFlowOp::Struct { fields: values, .. } => {
                let mut rendered = String::from("[");
                for (index, item) in values.iter().enumerate() {
                    if index != 0 {
                        rendered.push(',');
                    }
                    rendered.push_str(&strip_outer_parens(value(*item, cache)?));
                }
                rendered.push(']');
                JsExpression::atom(rendered)
            }
            ControlFlowOp::NewClass {
                class,
                constructor: None,
                args,
            } if args.is_empty() => JsExpression::atom(self.default_class_value(class, false)?),
            ControlFlowOp::Closure { function, captures } => {
                let captures = captures
                    .iter()
                    .map(|capture| value(*capture, cache).map(strip_outer_parens))
                    .collect::<Result<Vec<_>, _>>()?;
                let rendered = self.render_closure(*function, &captures)?;
                if rendered.contains("=>") {
                    JsExpression::raw(rendered, JsPrecedence::Assignment)
                } else {
                    JsExpression::atom(rendered)
                }
            }
            ControlFlowOp::LoadGlobal(symbol) => JsExpression::atom(self.global_name(*symbol)?),
            ControlFlowOp::FieldGet {
                object,
                owner,
                field,
                index,
                ..
            } => {
                let object_value = value(*object, cache)?;
                if (self.options.public_aggregate_fields && context.is_untyped(*object))
                    || self.class_uses_named_fields(owner)
                {
                    JsExpression::member(
                        object_value,
                        self.property_name(field),
                        self.options.elide_call_chain_parentheses,
                    )
                } else {
                    JsExpression::index(
                        object_value,
                        JsExpression::atom(index.to_string()),
                        self.options.elide_call_chain_parentheses,
                    )
                }
            }
            ControlFlowOp::HostFieldGet { object, property } => JsExpression::member(
                value(*object, cache)?,
                property,
                self.options.elide_call_chain_parentheses,
            ),
            ControlFlowOp::HostFieldSet {
                object,
                property,
                value: assigned,
            } => JsExpression::raw(
                format!(
                    "{}={}",
                    JsExpression::member(
                        value(*object, cache)?,
                        property,
                        self.options.elide_call_chain_parentheses,
                    ),
                    strip_outer_parens(value(*assigned, cache)?)
                ),
                JsPrecedence::Assignment,
            ),
            ControlFlowOp::IndexGet { object, index } => JsExpression::index(
                value(*object, cache)?,
                value(*index, cache)?,
                self.options.elide_call_chain_parentheses,
            ),
            ControlFlowOp::CallDirect { function, args } => self.render_call(
                JsExpression::atom(self.function_name(*function)?),
                args,
                context,
                cache,
            )?,
            ControlFlowOp::CallValue { callee, args } => {
                self.render_call(value(*callee, cache)?, args, context, cache)?
            }
            ControlFlowOp::HostCall {
                receiver,
                method,
                args,
                ..
            } => {
                let receiver = value(*receiver, cache)?;
                self.render_call(
                    JsExpression::member(
                        receiver,
                        method,
                        self.options.elide_call_chain_parentheses,
                    ),
                    args,
                    context,
                    cache,
                )?
            }
            ControlFlowOp::DynamicImport { module } => self.render_dynamic_import(*module)?,
            ControlFlowOp::Intrinsic {
                intrinsic,
                receiver,
                args,
            } => self.render_intrinsic(*intrinsic, *receiver, args, context, cache)?,
            ControlFlowOp::Template(parts) => {
                let mut rendered = String::from("`");
                for part in parts {
                    match part {
                        TemplateOperand::String(string) => rendered.push_str(string),
                        TemplateOperand::Value(item) => {
                            rendered.push_str("${");
                            rendered.push_str(&strip_outer_parens(value(*item, cache)?));
                            rendered.push('}');
                        }
                    }
                }
                rendered.push('`');
                JsExpression::atom(rendered)
            }
            ControlFlowOp::StoreLocal { .. }
            | ControlFlowOp::LoadLocal(_)
            | ControlFlowOp::StoreGlobal { .. }
            | ControlFlowOp::FieldSet { .. }
            | ControlFlowOp::IndexSet { .. }
            | ControlFlowOp::NewClass { .. }
            | ControlFlowOp::CallMethod { .. } => {
                return Err(CodegenError::new(
                    self.function(self.module.entry)?.span,
                    "IR contains an operation that must be lowered before expression emission",
                ));
            }
        })
    }

    fn render_dynamic_import(&self, module_id: u32) -> Result<JsExpression, CodegenError> {
        let module = self
            .module
            .lazy_modules
            .iter()
            .find(|module| module.id == module_id)
            .ok_or_else(|| {
                CodegenError::new(
                    self.module.functions[self.module.entry.0 as usize].span,
                    format!("missing dynamic module {module_id}"),
                )
            })?;
        if let Some(file) = self.dynamic_chunk_files.get(&module_id) {
            let file = render_string_literal(&format!("./{file}"), self.options.string_quote);
            let source = render_string_literal(module.source, self.options.string_quote);
            let imported =
                JsExpression::call(JsExpression::atom("import"), [JsExpression::atom(file)]);
            return Ok(JsExpression::call(
                JsExpression::member(imported, "catch", self.options.elide_call_chain_parentheses),
                [JsExpression::raw(
                    format!("e=>Promise.reject({{specifier:{source},message:String(e)}})"),
                    JsPrecedence::Assignment,
                )],
            ));
        }

        let mut namespace = String::from("Promise.resolve({");
        for (index, export) in module.exports.iter().enumerate() {
            if index != 0 {
                namespace.push(',');
            }
            namespace.push_str(&render_string_literal(
                export.name,
                self.options.string_quote,
            ));
            namespace.push(':');
            namespace.push_str(match export.binding {
                ExportBinding::Function(function) => self.function_name(function)?,
                ExportBinding::Global(global) => self.global_name(global)?,
                ExportBinding::TypeOnly => {
                    return Err(CodegenError::new(
                        export.span,
                        format!("dynamic export `{}` has no runtime binding", export.name),
                    ));
                }
            });
        }
        namespace.push_str("})");
        Ok(JsExpression::raw(namespace, JsPrecedence::Call))
    }

    fn render_call(
        &self,
        callee: JsExpression,
        args: &[ValueId],
        context: &LocalNames,
        cache: &mut ExpressionCache,
    ) -> Result<JsExpression, CodegenError> {
        let arguments = args
            .iter()
            .map(|argument| take_value(*argument, context, cache))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(JsExpression::call(callee, arguments))
    }

    fn render_intrinsic(
        &self,
        intrinsic: Intrinsic,
        receiver: Option<ValueId>,
        args: &[ValueId],
        context: &LocalNames,
        cache: &mut ExpressionCache,
    ) -> Result<JsExpression, CodegenError> {
        if intrinsic == Intrinsic::Print {
            return self.render_call(
                JsExpression::member(
                    JsExpression::atom("console"),
                    "log",
                    self.options.elide_call_chain_parentheses,
                ),
                args,
                context,
                cache,
            );
        }
        if intrinsic == Intrinsic::IntImul {
            return self.render_call(
                JsExpression::member(
                    JsExpression::atom("Math"),
                    "imul",
                    self.options.elide_call_chain_parentheses,
                ),
                args,
                context,
                cache,
            );
        }
        let constructor = match intrinsic {
            Intrinsic::MapNew => Some("Map"),
            Intrinsic::SetNew => Some("Set"),
            Intrinsic::ArrayBufferNew => Some("ArrayBuffer"),
            Intrinsic::SharedArrayBufferNew => Some("SharedArrayBuffer"),
            Intrinsic::SymbolNew => None,
            other => classify_typed_array_intrinsic(other)
                .filter(|(_, op)| *op == TypedArrayIntrinsic::New)
                .map(|(kind, _)| kind.name()),
        };
        if intrinsic == Intrinsic::SymbolNew {
            let mut rendered = String::from("Symbol(");
            for (index, arg) in args.iter().enumerate() {
                if index != 0 {
                    rendered.push(',');
                }
                rendered.push_str(&strip_outer_parens(take_value(*arg, context, cache)?));
            }
            rendered.push(')');
            return Ok(JsExpression::raw(rendered, JsPrecedence::Call));
        }
        if let Some(constructor) = constructor {
            if args.is_empty() && self.options.elide_new_parentheses {
                return Ok(JsExpression::raw(
                    format!("new {constructor}"),
                    JsPrecedence::NewWithoutArgs,
                ));
            }
            let mut rendered = format!("new {constructor}(");
            for (index, arg) in args.iter().enumerate() {
                if index != 0 {
                    rendered.push(',');
                }
                let argument = take_value(*arg, context, cache)?.without_integer_normalization();
                rendered.push_str(&strip_outer_parens(argument));
            }
            rendered.push(')');
            return Ok(JsExpression::raw(rendered, JsPrecedence::Call));
        }
        let receiver = receiver.ok_or_else(|| {
            CodegenError::new(
                self.function(self.module.entry).unwrap().span,
                "missing receiver",
            )
        })?;
        let receiver = take_value(receiver, context, cache)?;
        match intrinsic {
            Intrinsic::JsTruthy => {
                return Ok(JsExpression::unary("!", JsExpression::unary("!", receiver)));
            }
            Intrinsic::JsIsArray => {
                return Ok(JsExpression::call(
                    JsExpression::atom("Array.isArray"),
                    [receiver],
                ));
            }
            Intrinsic::JsIsObject => {
                return Ok(JsExpression::raw(
                    format!(
                        "typeof({})=={}",
                        receiver.into_minimal(),
                        render_string_literal("object", self.options.string_quote)
                    ),
                    JsPrecedence::Equality,
                ));
            }
            Intrinsic::JsForInKey | Intrinsic::JsForInHasNext => {
                return Err(CodegenError::new(
                    self.function(self.module.entry)?.span,
                    "for-in pseudo-intrinsic escaped structured loop emission",
                ));
            }
            _ => {}
        }
        let property = match intrinsic {
            Intrinsic::UnwrapNullable | Intrinsic::UnwrapUnion => return Ok(receiver),
            Intrinsic::ArrayLength | Intrinsic::StringLength => {
                return Ok(JsExpression::member(
                    receiver,
                    "length",
                    self.options.elide_call_chain_parentheses,
                ));
            }
            Intrinsic::MapSize | Intrinsic::SetSize => {
                return Ok(JsExpression::member(
                    receiver,
                    "size",
                    self.options.elide_call_chain_parentheses,
                ));
            }
            Intrinsic::BufferByteLength => {
                return Ok(JsExpression::member(
                    receiver,
                    "byteLength",
                    self.options.elide_call_chain_parentheses,
                ));
            }
            other
                if matches!(
                    classify_typed_array_intrinsic(other),
                    Some((_, TypedArrayIntrinsic::ByteLength))
                ) =>
            {
                return Ok(JsExpression::member(
                    receiver,
                    "byteLength",
                    self.options.elide_call_chain_parentheses,
                ));
            }
            other
                if matches!(
                    classify_typed_array_intrinsic(other),
                    Some((_, TypedArrayIntrinsic::Length))
                ) =>
            {
                return Ok(JsExpression::member(
                    receiver,
                    "length",
                    self.options.elide_call_chain_parentheses,
                ));
            }
            other
                if matches!(
                    classify_typed_array_intrinsic(other),
                    Some((_, TypedArrayIntrinsic::ByteOffset))
                ) =>
            {
                return Ok(JsExpression::member(
                    receiver,
                    "byteOffset",
                    self.options.elide_call_chain_parentheses,
                ));
            }
            other
                if matches!(
                    classify_typed_array_intrinsic(other),
                    Some((_, TypedArrayIntrinsic::Buffer))
                ) =>
            {
                return Ok(JsExpression::member(
                    receiver,
                    "buffer",
                    self.options.elide_call_chain_parentheses,
                ));
            }
            Intrinsic::MapGet => {
                let call = self.render_call(
                    JsExpression::member(
                        receiver,
                        "get",
                        self.options.elide_call_chain_parentheses,
                    ),
                    args,
                    context,
                    cache,
                )?;
                return Ok(JsExpression::grouped(
                    format!("{call}??null"),
                    JsPrecedence::LogicalOr,
                    JsExpressionRoot::Raw,
                ));
            }
            Intrinsic::StringCharCodeAt => {
                let call = self.render_call(
                    JsExpression::member(
                        receiver,
                        "charCodeAt",
                        self.options.elide_call_chain_parentheses,
                    ),
                    args,
                    context,
                    cache,
                )?;
                return Ok(JsExpression::integer_normalization(call));
            }
            Intrinsic::StringCharAt => {
                return self.render_call(
                    JsExpression::member(
                        receiver,
                        "charAt",
                        self.options.elide_call_chain_parentheses,
                    ),
                    args,
                    context,
                    cache,
                );
            }
            Intrinsic::IntToString | Intrinsic::IntToUnsignedString => {
                let radix = if let Some(radix) = args.first() {
                    take_value(*radix, context, cache)?
                } else {
                    JsExpression::atom("10")
                };
                let receiver = if matches!(intrinsic, Intrinsic::IntToUnsignedString) {
                    JsExpression::grouped(
                        format!(
                            "{}>>>0",
                            receiver.binary_operand(
                                IrBinaryOp::UnsignedShiftRight,
                                BinaryOperandSide::Left,
                            )
                        ),
                        JsPrecedence::Shift,
                        JsExpressionRoot::Binary(IrBinaryOp::UnsignedShiftRight),
                    )
                } else {
                    JsExpression::grouped(
                        receiver.into_minimal(),
                        JsPrecedence::Primary,
                        JsExpressionRoot::Raw,
                    )
                };
                return Ok(JsExpression::call(
                    JsExpression::member(
                        receiver,
                        "toString",
                        self.options.elide_call_chain_parentheses,
                    ),
                    [radix],
                ));
            }
            Intrinsic::FloatToInt => {
                return Ok(JsExpression::integer_normalization(receiver));
            }
            Intrinsic::FloatAbs
            | Intrinsic::FloatFloor
            | Intrinsic::FloatCeil
            | Intrinsic::FloatRound
            | Intrinsic::FloatSqrt
            | Intrinsic::FloatSin
            | Intrinsic::FloatCos
            | Intrinsic::FloatAcos
            | Intrinsic::FloatExp
            | Intrinsic::FloatLog
            | Intrinsic::FloatTan
            | Intrinsic::FloatAtan2
            | Intrinsic::FloatHypot
            | Intrinsic::FloatMin
            | Intrinsic::FloatMax => {
                let method = match intrinsic {
                    Intrinsic::FloatAbs => "abs",
                    Intrinsic::FloatFloor => "floor",
                    Intrinsic::FloatCeil => "ceil",
                    Intrinsic::FloatRound => "round",
                    Intrinsic::FloatSqrt => "sqrt",
                    Intrinsic::FloatSin => "sin",
                    Intrinsic::FloatCos => "cos",
                    Intrinsic::FloatAcos => "acos",
                    Intrinsic::FloatExp => "exp",
                    Intrinsic::FloatLog => "log",
                    Intrinsic::FloatTan => "tan",
                    Intrinsic::FloatAtan2 => "atan2",
                    Intrinsic::FloatHypot => "hypot",
                    Intrinsic::FloatMin => "min",
                    Intrinsic::FloatMax => "max",
                    _ => unreachable!(),
                };
                let mut rendered = format!("Math.{method}({}", strip_outer_parens(receiver));
                for arg in args {
                    rendered.push(',');
                    rendered.push_str(&strip_outer_parens(take_value(*arg, context, cache)?));
                }
                rendered.push(')');
                return Ok(JsExpression::raw(rendered, JsPrecedence::Call));
            }
            Intrinsic::ArrayMap => "map",
            Intrinsic::ArrayFilter => "filter",
            Intrinsic::ArrayReduce => "reduce",
            Intrinsic::ArrayForEach => "forEach",
            Intrinsic::ArrayPush => "push",
            Intrinsic::ArrayPop => "pop",
            Intrinsic::ArrayIndexOf => "indexOf",
            Intrinsic::ArraySlice => "slice",
            Intrinsic::ArraySplice => "splice",
            Intrinsic::MapSet => "set",
            Intrinsic::MapHas => "has",
            Intrinsic::MapDelete => "delete",
            Intrinsic::MapClear => "clear",
            Intrinsic::SetAdd => "add",
            Intrinsic::SetHas => "has",
            Intrinsic::SetDelete => "delete",
            Intrinsic::SetClear => "clear",
            Intrinsic::BufferSlice => "slice",
            other
                if matches!(
                    classify_typed_array_intrinsic(other),
                    Some((_, TypedArrayIntrinsic::Slice))
                ) =>
            {
                "slice"
            }
            other
                if matches!(
                    classify_typed_array_intrinsic(other),
                    Some((_, TypedArrayIntrinsic::Subarray))
                ) =>
            {
                "subarray"
            }
            Intrinsic::StringIncludes => "includes",
            Intrinsic::StringStartsWith => "startsWith",
            Intrinsic::StringEndsWith => "endsWith",
            Intrinsic::StringToUpperCase => "toUpperCase",
            Intrinsic::StringToLowerCase => "toLowerCase",
            Intrinsic::Print
            | Intrinsic::IntImul
            | Intrinsic::MapNew
            | Intrinsic::SetNew
            | Intrinsic::ArrayBufferNew
            | Intrinsic::SharedArrayBufferNew
            | Intrinsic::SymbolNew => unreachable!(),
            other
                if matches!(
                    classify_typed_array_intrinsic(other),
                    Some((_, TypedArrayIntrinsic::New))
                ) =>
            {
                unreachable!()
            }
            _ => unreachable!("unhandled intrinsic in property emit"),
        };
        self.render_call(
            JsExpression::member(
                receiver,
                property,
                self.options.elide_call_chain_parentheses,
            ),
            args,
            context,
            cache,
        )
    }

    fn render_closure(
        &mut self,
        function: FunctionId,
        captures: &[String],
    ) -> Result<String, CodegenError> {
        let function = self.function(function)?.clone();
        if captures.len() != function.capture_count {
            return Err(CodegenError::new(
                function.span,
                "closure capture count does not match its IR function",
            ));
        }
        if !can_inline_closure(&function, self.options.inline_structured_closures) {
            let name = self.function_name(function.id)?;
            if captures.is_empty() {
                return Ok(name.to_string());
            }
            let mut wrapper_mangler = self.local_mangler(&function);
            wrapper_mangler.reserve(name);
            for capture in captures {
                reserve_expression_identifiers(&mut wrapper_mangler, capture);
            }
            let context = LocalNames::new(
                &function,
                self.integer_analysis.function(function.id),
                false,
                &wrapper_mangler,
                &self.preferred_local_names,
                &self.numeric_aliases,
                &self.options,
            );
            let mut rendered = render_arrow_parameters(
                &function,
                &context,
                self.options.compact_boolean_literals,
            )?;
            let mut call = String::new();
            call.push_str(name);
            call.push('(');
            for (index, capture) in captures.iter().enumerate() {
                if index != 0 {
                    call.push(',');
                }
                call.push_str(capture);
            }
            for param in &function.params[function.capture_count..] {
                if !captures.is_empty() || param.value != function.params[0].value {
                    call.push(',');
                }
                call.push_str(context.value_name(param.value)?);
            }
            call.push(')');
            if self.options.function_spelling == FunctionSpelling::Function {
                rendered = function_parameters(&rendered);
                rendered.insert_str(0, "function");
                rendered.push_str("{return ");
                rendered.push_str(&call);
                rendered.push('}');
            } else {
                rendered.push_str("=>");
                rendered.push_str(&call);
            }
            return Ok(rendered);
        }
        let local_mangler = self.local_mangler(&function);
        let mut context = LocalNames::new(
            &function,
            self.integer_analysis.function(function.id),
            false,
            &local_mangler,
            &self.preferred_local_names,
            &self.numeric_aliases,
            &self.options,
        );
        let capture_params = &function.params[..function.capture_count];
        let capture_values = capture_params
            .iter()
            .map(|parameter| parameter.value)
            .collect::<AHashSet<_>>();
        let hidden_names = capture_params
            .iter()
            .filter_map(|parameter| context.value_names.get(&parameter.value))
            .cloned()
            .collect::<AHashSet<_>>();
        let mut mangler = local_mangler;
        for name in context.value_names.values().chain(captures) {
            mangler.reserve(name);
        }
        let mut replacements = AHashMap::<String, String>::new();
        for (value, name) in &context.value_names {
            if !capture_values.contains(value)
                && (hidden_names.contains(name) || captures.contains(name))
            {
                replacements
                    .entry(name.clone())
                    .or_insert_with(|| mangler.next_name());
            }
        }
        for (value, name) in &mut context.value_names {
            if !capture_values.contains(value) {
                if let Some(replacement) = replacements.get(name) {
                    *name = replacement.clone();
                }
            }
        }
        for (param, capture) in function.params.iter().zip(captures) {
            context.value_names.insert(param.value, capture.clone());
        }
        *context.declared_names.borrow_mut() = function.params[function.capture_count..]
            .iter()
            .filter_map(|parameter| context.value_names.get(&parameter.value))
            .cloned()
            .collect();
        if function.blocks.len() > 1 {
            context.inline_declarations = true;
            let parameters = render_arrow_parameters(
                &function,
                &context,
                self.options.compact_boolean_literals,
            )?;
            let mut body = String::new();
            self.emit_structured_with_context(&function, true, context, &mut body)?;
            return Ok(
                if self.options.function_spelling == FunctionSpelling::Function {
                    format!("function{}{body}", function_parameters(&parameters))
                } else {
                    format!("{parameters}=>{body}")
                },
            );
        }
        let expression_closure = matches!(
            function.blocks[0].terminator,
            Some(Terminator::Return(Some(_)))
        ) && function.blocks[0]
            .instructions
            .iter()
            .all(|instruction| op_can_defer(&instruction.op));
        if !expression_closure {
            let parameters = render_arrow_parameters(
                &function,
                &context,
                self.options.compact_boolean_literals,
            )?;
            let mut body = String::new();
            self.emit_single_block_with_context(&function, true, context, &mut body)?;
            return Ok(
                if self.options.function_spelling == FunctionSpelling::Function {
                    format!("function{}{body}", function_parameters(&parameters))
                } else {
                    format!("{parameters}=>{body}")
                },
            );
        }
        let uses = use_counts(&function);
        let mut cache = AHashMap::new();
        let mut prefix = String::new();
        for instruction in &function.blocks[0].instructions {
            if !op_can_defer(&instruction.op) {
                return Err(CodegenError::new(
                    instruction.span,
                    "effectful closure requires named function emission",
                ));
            }
            let expression = self.render_instruction_op(instruction, &context, &mut cache)?;
            let out = instruction.out.ok_or_else(|| {
                CodegenError::new(instruction.span, "closure value has no output")
            })?;
            if uses.get(&out).copied().unwrap_or(0) == 1 {
                cache.insert(out, expression);
            } else {
                prefix.push_str("let ");
                prefix.push_str(context.value_name(out)?);
                prefix.push('=');
                prefix.push_str(&expression);
                prefix.push(';');
            }
        }
        let Some(Terminator::Return(Some(value))) = function.blocks[0].terminator else {
            return Err(CodegenError::new(
                function.span,
                "closure has no returned expression",
            ));
        };
        let returned = strip_outer_parens(take_value(value, &context, &mut cache)?);
        let parameters =
            render_arrow_parameters(&function, &context, self.options.compact_boolean_literals)?;
        let mut rendered = if self.options.function_spelling == FunctionSpelling::Function {
            format!("function{}{{", function_parameters(&parameters))
        } else {
            format!("{parameters}=>")
        };
        if self.options.function_spelling == FunctionSpelling::Function {
            rendered.push_str(&prefix);
            rendered.push_str("return ");
            rendered.push_str(&returned);
            rendered.push('}');
        } else if prefix.is_empty() {
            rendered.push_str(&returned);
        } else {
            rendered.push('{');
            rendered.push_str(&prefix);
            rendered.push_str("return ");
            rendered.push_str(&returned);
            rendered.push('}');
        }
        Ok(rendered)
    }

    fn default_class_value(&self, class: &str, boundary: bool) -> Result<String, CodegenError> {
        let layout = self
            .module
            .classes
            .iter()
            .find(|layout| layout.name == class)
            .ok_or_else(|| {
                CodegenError::new(
                    self.function(self.module.entry).unwrap().span,
                    "missing class layout",
                )
            })?;
        let named = (boundary && self.options.public_aggregate_fields)
            || self.class_uses_named_fields(class);
        let mut value = String::from(if named { "{" } else { "[" });
        for (index, field) in layout.fields.iter().enumerate() {
            if index != 0 {
                value.push(',');
            }
            if named {
                value.push_str(self.property_name(field.name));
                value.push(':');
            }
            value.push_str(default_value(
                &field.ty,
                self.options.compact_boolean_literals,
            ));
        }
        value.push(if named { '}' } else { ']' });
        Ok(value)
    }

    fn class_uses_named_fields(&self, owner: &str) -> bool {
        self.named_field_aggregates.contains(owner)
    }

    fn function(&self, id: FunctionId) -> Result<&ControlFlowFunction<'src>, CodegenError> {
        self.module.functions.get(id.0 as usize).ok_or_else(|| {
            CodegenError::new(
                self.module
                    .functions
                    .first()
                    .map_or(crate::span::Span::empty(0), |function| function.span),
                format!("missing IR function {}", id.0),
            )
        })
    }

    fn function_name(&self, id: FunctionId) -> Result<&str, CodegenError> {
        self.function_names
            .get(&id)
            .map(String::as_str)
            .ok_or_else(|| {
                CodegenError::new(
                    self.function(id)
                        .map_or(crate::span::Span::empty(0), |function| function.span),
                    format!("function {} has no emitted name", id.0),
                )
            })
    }

    fn global_name(&self, symbol: SymbolId) -> Result<&str, CodegenError> {
        self.global_names
            .get(&symbol)
            .map(String::as_str)
            .ok_or_else(|| {
                CodegenError::new(
                    self.function(self.module.entry).unwrap().span,
                    format!("global symbol {} has no emitted name", symbol.0),
                )
            })
    }
}

fn type_references_class(ty: &Type<'_>, owner: &str) -> bool {
    match ty {
        Type::Class(name) => *name == owner,
        Type::ClassInstance { name, args } => {
            *name == owner || args.iter().any(|ty| type_references_class(ty, owner))
        }
        Type::Struct(name) => *name == owner,
        Type::StructInstance { name, args } => {
            *name == owner || args.iter().any(|ty| type_references_class(ty, owner))
        }
        Type::Array(ty) | Type::Set(ty) | Type::Task(ty) | Type::Nullable(ty) => {
            type_references_class(ty, owner)
        }
        Type::Map(key, value) => {
            type_references_class(key, owner) || type_references_class(value, owner)
        }
        Type::Union(members) => members
            .iter()
            .any(|member| type_references_class(member, owner)),
        Type::Function(function) => {
            function
                .params
                .iter()
                .any(|ty| type_references_class(ty, owner))
                || type_references_class(&function.return_type, owner)
        }
        Type::GenericFunction(function) => {
            function
                .signature
                .params
                .iter()
                .any(|ty| type_references_class(ty, owner))
                || type_references_class(&function.signature.return_type, owner)
        }
        Type::Int
        | Type::Float
        | Type::String
        | Type::Bool
        | Type::Null
        | Type::Void
        | Type::ArrayBuffer
        | Type::SharedArrayBuffer
        | Type::Int8Array
        | Type::Uint8Array
        | Type::Uint8ClampedArray
        | Type::Int16Array
        | Type::Uint16Array
        | Type::Int32Array
        | Type::Uint32Array
        | Type::Float32Array
        | Type::Float64Array
        | Type::Symbol
        | Type::ModuleNamespace(_)
        | Type::ModuleLoadError
        | Type::TypeParameter(_) => false,
    }
}

#[derive(Debug, Default)]
struct ChunkReferences {
    functions: AHashSet<FunctionId>,
    globals: AHashSet<SymbolId>,
    strings: AHashSet<String>,
    dynamic_modules: AHashSet<u32>,
}

fn is_emitted_function(function: &ControlFlowFunction<'_>, inline_structured: bool) -> bool {
    function.live
        && !matches!(function.kind, FunctionKind::Entry | FunctionKind::Extern)
        && !(function.kind == FunctionKind::Closure
            && can_inline_closure(function, inline_structured))
}

fn collect_chunk_references(
    module: &ControlFlowModule<'_>,
    roots: &[FunctionId],
    string_aliases: &AHashMap<String, String>,
    numeric_aliases: &AHashMap<String, String>,
    inline_structured: bool,
) -> ChunkReferences {
    let mut references = ChunkReferences::default();
    let mut pending = roots.to_vec();
    let mut visited = AHashSet::new();
    while let Some(function_id) = pending.pop() {
        if !visited.insert(function_id) {
            continue;
        }
        let Some(function) = module.functions.get(function_id.0 as usize) else {
            continue;
        };
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            match &instruction.op {
                ControlFlowOp::LoadGlobal(global) | ControlFlowOp::StoreGlobal { global, .. } => {
                    references.globals.insert(*global);
                }
                ControlFlowOp::Const(ConstValue::String(value)) => {
                    if let Some(alias) = string_aliases.get(value) {
                        references.strings.insert(alias.clone());
                    }
                }
                ControlFlowOp::Const(value) => {
                    let rendered = render_const(value, true, StringQuote::Double);
                    if let Some(alias) = numeric_aliases.get(&rendered) {
                        references.strings.insert(alias.clone());
                    }
                }
                ControlFlowOp::DynamicImport { module } => {
                    references.dynamic_modules.insert(*module);
                }
                ControlFlowOp::NewClass {
                    constructor: Some(target),
                    ..
                }
                | ControlFlowOp::Closure {
                    function: target, ..
                }
                | ControlFlowOp::CallDirect {
                    function: target, ..
                }
                | ControlFlowOp::CallMethod {
                    function: target, ..
                } => {
                    let Some(target_function) = module.functions.get(target.0 as usize) else {
                        continue;
                    };
                    if target_function.kind == FunctionKind::Closure
                        && can_inline_closure(target_function, inline_structured)
                    {
                        pending.push(*target);
                    } else if is_emitted_function(target_function, inline_structured) {
                        references.functions.insert(*target);
                    }
                }
                _ => {}
            }
        }
    }
    references
}

fn function_writes_global(
    module: &ControlFlowModule<'_>,
    function_id: FunctionId,
    visited: &mut AHashSet<FunctionId>,
    inline_structured: bool,
) -> bool {
    if !visited.insert(function_id) {
        return false;
    }
    let Some(function) = module.functions.get(function_id.0 as usize) else {
        return false;
    };
    function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .any(|instruction| match &instruction.op {
            ControlFlowOp::StoreGlobal { .. } => true,
            ControlFlowOp::Closure {
                function: target, ..
            } => module
                .functions
                .get(target.0 as usize)
                .is_some_and(|target_function| {
                    target_function.kind == FunctionKind::Closure
                        && can_inline_closure(target_function, inline_structured)
                        && function_writes_global(module, *target, visited, inline_structured)
                }),
            _ => false,
        })
}

fn emit_chunk_imports(
    out: &mut String,
    current: usize,
    files: &[String],
    imports: &AHashMap<usize, AHashSet<String>>,
    quote: StringQuote,
) {
    let mut sources = imports.iter().collect::<Vec<_>>();
    sources.sort_unstable_by(|(left, _), (right, _)| files[**left].cmp(&files[**right]));
    for (source, names) in sources {
        if *source == current || names.is_empty() {
            continue;
        }
        let mut names = names.iter().collect::<Vec<_>>();
        names.sort_unstable();
        out.push_str("import{");
        for (index, name) in names.iter().enumerate() {
            if index != 0 {
                out.push(',');
            }
            out.push_str(name);
        }
        out.push_str("}from");
        out.push_str(&render_string_literal(
            &format!("./{}", files[*source]),
            quote,
        ));
        out.push(';');
    }
}

fn order_scalar_assignments(assignments: &[(String, String)]) -> Option<Vec<(&str, &str)>> {
    let mut remaining = assignments.iter().collect::<Vec<_>>();
    let mut ordered = Vec::with_capacity(assignments.len());
    while !remaining.is_empty() {
        let index = remaining.iter().position(|(target, _)| {
            remaining.iter().all(|(other_target, source)| {
                other_target == target || !expression_references_name(source, target)
            })
        })?;
        let (target, source) = remaining.remove(index);
        ordered.push((target.as_str(), source.as_str()));
    }
    Some(ordered)
}

fn scalar_parallel_assignments(
    assignments: &[(String, String)],
    temporary: Option<(&str, bool)>,
) -> Option<String> {
    if let Some(ordered) = order_scalar_assignments(assignments) {
        let mut output = String::new();
        for (target, source) in ordered {
            output.push_str(target);
            output.push('=');
            output.push_str(source);
            output.push(';');
        }
        return Some(output);
    }

    let (temporary, declare_temporary) = temporary?;
    let mut remaining = assignments.to_vec();
    let mut output = String::new();
    let mut temporary_declared = false;
    while !remaining.is_empty() {
        if let Some(index) = remaining.iter().position(|(target, _)| {
            remaining.iter().all(|(other_target, source)| {
                other_target == target || !expression_references_name(source, target)
            })
        }) {
            let (target, source) = remaining.remove(index);
            output.push_str(&target);
            output.push('=');
            output.push_str(&source);
            output.push(';');
            continue;
        }

        if remaining
            .iter()
            .any(|(_, source)| expression_references_name(source, temporary))
        {
            return None;
        }
        let saved = remaining[0].0.clone();
        if temporary_declared || !declare_temporary {
            output.push_str(temporary);
        } else {
            output.push_str("var ");
            output.push_str(temporary);
            temporary_declared = true;
        }
        output.push('=');
        output.push_str(&saved);
        output.push(';');
        for (_, source) in &mut remaining {
            *source = replace_identifier(source, &saved, temporary);
        }
    }
    Some(output)
}

fn reusable_parallel_copy_temporary(
    target: BlockId,
    context: &LocalNames,
    assignments: &[(String, String)],
) -> Option<String> {
    let live_names = context.live_in_values[target.0 as usize]
        .iter()
        .filter_map(|value| context.value_names.get(value))
        .collect::<AHashSet<_>>();
    let declared = context.declared_names.borrow();
    let mut candidates = context
        .value_names
        .values()
        .filter(|name| declared.contains(*name) && !live_names.contains(name))
        .filter(|name| {
            assignments.iter().all(|(target, source)| {
                target != *name && !expression_references_name(source, name)
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    candidates
        .sort_unstable_by(|left, right| left.len().cmp(&right.len()).then_with(|| left.cmp(right)));
    candidates.dedup();
    candidates.into_iter().next()
}

fn live_in_values(function: &ControlFlowFunction<'_>) -> Vec<AHashSet<ValueId>> {
    let block_count = function.blocks.len();
    let mut definitions = vec![AHashSet::new(); block_count];
    let mut local_uses = vec![AHashSet::new(); block_count];
    let mut phi_definitions = vec![AHashSet::new(); block_count];
    for block in &function.blocks {
        let index = block.id.0 as usize;
        for phi in &block.phis {
            definitions[index].insert(phi.out);
            phi_definitions[index].insert(phi.out);
        }
        for instruction in &block.instructions {
            for value in op_values(&instruction.op) {
                if !definitions[index].contains(&value) {
                    local_uses[index].insert(value);
                }
            }
            if let Some(out) = instruction.out {
                definitions[index].insert(out);
            }
        }
        for value in block
            .terminator
            .as_ref()
            .into_iter()
            .flat_map(terminator_values)
        {
            if !definitions[index].contains(&value) {
                local_uses[index].insert(value);
            }
        }
    }

    let mut live_in = vec![AHashSet::new(); block_count];
    let mut live_out = vec![AHashSet::new(); block_count];
    loop {
        let mut changed = false;
        for block in function.blocks.iter().rev() {
            let index = block.id.0 as usize;
            let mut output = AHashSet::new();
            for successor in block_successors(block) {
                let successor_index = successor.0 as usize;
                output.extend(
                    live_in[successor_index]
                        .difference(&phi_definitions[successor_index])
                        .copied(),
                );
                for phi in &function.blocks[successor_index].phis {
                    if let Some((_, value)) = phi
                        .incoming
                        .iter()
                        .find(|(predecessor, _)| predecessor == &block.id)
                    {
                        output.insert(*value);
                    }
                }
            }
            let mut input = local_uses[index].clone();
            input.extend(output.difference(&definitions[index]).copied());
            if output != live_out[index] {
                live_out[index] = output;
                changed = true;
            }
            if input != live_in[index] {
                live_in[index] = input;
                changed = true;
            }
        }
        if !changed {
            return live_in;
        }
    }
}

fn replace_identifier(expression: &str, from: &str, to: &str) -> String {
    let mut output = String::with_capacity(expression.len());
    let mut copied_until = 0usize;
    for (start, end) in expression_identifier_spans(expression) {
        if &expression[start..end] == from {
            output.push_str(&expression[copied_until..start]);
            output.push_str(to);
            copied_until = end;
        }
    }
    output.push_str(&expression[copied_until..]);
    output
}

fn merge_conditional_assignments<'a>(
    then_output: &'a str,
    else_output: &'a str,
) -> Option<(bool, &'a str, &'a str, &'a str, &'a str)> {
    let (then_declare, then_target, then_value, then_trailing) =
        parse_single_assignment(then_output)?;
    let (else_declare, else_target, else_value, else_trailing) =
        parse_single_assignment(else_output)?;
    if !then_trailing.is_empty() && !else_trailing.is_empty() && then_trailing != else_trailing {
        return None;
    }
    (then_target == else_target).then_some((
        then_declare || else_declare,
        then_target,
        then_value,
        else_value,
        if then_trailing.is_empty() {
            else_trailing
        } else {
            then_trailing
        },
    ))
}

fn conditional_assignment_expression<'a>(
    then_output: &'a str,
    else_output: &'a str,
) -> Option<(&'a str, &'a str, &'a str, &'a str)> {
    let (then_declare, then_target, then_value, then_trailing) =
        parse_single_assignment(then_output)?;
    let (else_declare, else_target, else_value, else_trailing) =
        parse_single_assignment(else_output)?;
    (!then_declare
        && !else_declare
        && then_trailing.is_empty()
        && else_trailing.is_empty()
        && then_target != else_target)
        .then_some((then_target, then_value, else_target, else_value))
}

fn push_logical_operand(out: &mut String, value: &str, parent: IrBinaryOp) {
    let needs_parentheses = logical_operand_needs_parentheses(value, parent);
    if needs_parentheses {
        out.push('(');
    }
    out.push_str(value);
    if needs_parentheses {
        out.push(')');
    }
}

fn logical_operand_needs_parentheses(value: &str, parent: IrBinaryOp) -> bool {
    let bytes = value.as_bytes();
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(active) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == active {
                quote = None;
            }
            index += 1;
            continue;
        }
        match byte {
            b'\'' | b'"' | b'`' => quote = Some(byte),
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b';' | b',' | b'?' if depth == 0 => return true,
            b'=' if depth == 0 => {
                let previous = index.checked_sub(1).map(|index| bytes[index]);
                let next = bytes.get(index + 1).copied();
                let comparison =
                    matches!(previous, Some(b'=' | b'!' | b'<' | b'>')) || next == Some(b'=');
                if !comparison {
                    return true;
                }
            }
            b'|' if depth == 0
                && parent == IrBinaryOp::And
                && bytes.get(index + 1) == Some(&b'|') =>
            {
                return true;
            }
            _ => {}
        }
        index += 1;
    }
    depth != 0
}

fn param_feeds_loop_phi(function: &ControlFlowFunction<'_>, param: ValueId) -> bool {
    function.blocks.iter().any(|block| {
        block
            .phis
            .iter()
            .any(|phi| phi.incoming.iter().any(|(_, incoming)| *incoming == param))
    })
}

fn is_bitwise_root(expression: &JsExpression) -> bool {
    matches!(
        expression.root,
        JsExpressionRoot::Binary(
            IrBinaryOp::BitAnd
                | IrBinaryOp::BitOr
                | IrBinaryOp::Xor
                | IrBinaryOp::ShiftLeft
                | IrBinaryOp::ShiftRight
                | IrBinaryOp::UnsignedShiftRight
        )
    )
}

fn is_i32_atom_literal(expression: &JsExpression) -> bool {
    expression.root == JsExpressionRoot::Atom && is_i32_literal(&expression.code)
}

fn is_i32_literal(code: &str) -> bool {
    code.parse::<i32>()
        .ok()
        .is_some_and(|value| value.to_string() == code)
}

fn bitwise_arithmetic_elides_coercion(
    op: IrBinaryOp,
    lhs: &JsExpression,
    rhs: &JsExpression,
) -> bool {
    match op {
        IrBinaryOp::Sub => is_bitwise_root(lhs) && is_i32_atom_literal(rhs),
        IrBinaryOp::Add => {
            (is_bitwise_root(lhs) && is_i32_atom_literal(rhs))
                || (is_bitwise_root(rhs) && is_i32_atom_literal(lhs))
        }
        _ => false,
    }
}

fn try_rewrite_arrow_expression_body(out: &mut String, body_start: usize) {
    if body_start >= out.len() || !out[body_start..].starts_with('{') {
        return;
    }
    let body = &out[body_start..];
    let Some(inner) = body
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'))
    else {
        return;
    };
    let Some(expression) = inner.strip_prefix("return ") else {
        return;
    };
    let expression = expression.strip_suffix(';').unwrap_or(expression);
    if expression.is_empty() || expression_has_top_level_statement_break(expression) {
        return;
    }
    let expression = expression.to_string();
    out.truncate(body_start);
    out.push_str(&expression);
}

fn arrow_block_as_object_method(property: &str, expression: &JsExpression) -> Option<String> {
    let code = expression
        .ungrouped
        .as_deref()
        .unwrap_or(expression.code.as_str());
    let arrow = code.find("=>{")?;
    let parameters = &code[..arrow];
    let body = &code[arrow + 2..];
    let parameters = parameters
        .strip_prefix('(')
        .and_then(|parameters| parameters.strip_suffix(')'))
        .unwrap_or(parameters);
    Some(format!("{property}({parameters}){body}"))
}

fn expression_has_top_level_statement_break(expression: &str) -> bool {
    let mut depth = 0i32;
    let mut in_string = None::<char>;
    let bytes = expression.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(quote) = in_string {
            if byte == b'\\' {
                index += 2;
                continue;
            }
            if byte as char == quote {
                in_string = None;
            }
            index += 1;
            continue;
        }
        match byte {
            b'\'' | b'"' | b'`' => in_string = Some(byte as char),
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b';' if depth == 0 => return true,
            b',' if depth == 0 => return true,
            _ => {}
        }
        index += 1;
    }
    depth != 0
}

fn inject_public_int_param_coercions(out: &mut String, function_start: usize, params: &[String]) {
    let body = &out[function_start..];
    let Some(relative) = body.find('{') else {
        return;
    };
    let insert_at = function_start + relative + 1;
    let mut coercion = String::new();
    for param in params {
        coercion.push_str(param);
        coercion.push_str("|=0;");
    }
    out.insert_str(insert_at, &coercion);
}

fn for_update_clause(output: &str) -> Option<String> {
    let clause = output.strip_suffix(';')?;
    if clause.contains(';') {
        return None;
    }
    if parse_single_assignment(output)
        .is_some_and(|(declare, _, _, trailing)| !declare && trailing.is_empty())
    {
        return Some(clause.to_string());
    }
    is_unit_update_clause(clause).then(|| clause.to_string())
}

fn is_unit_update_clause(clause: &str) -> bool {
    let operand = clause
        .strip_prefix("++")
        .or_else(|| clause.strip_prefix("--"))
        .or_else(|| clause.strip_suffix("++"))
        .or_else(|| clause.strip_suffix("--"));
    operand.is_some_and(|name| {
        !name.is_empty()
            && is_js_identifier_start(name.as_bytes()[0])
            && name.bytes().all(is_js_identifier_byte)
    })
}

fn one_use_unit_update(
    function: &ControlFlowFunction<'_>,
    from: BlockId,
    target: ValueId,
    source: ValueId,
    uses: &AHashMap<ValueId, usize>,
    context: &LocalNames,
) -> Option<i32> {
    if uses.get(&source).copied() != Some(1) {
        return None;
    }
    let instruction = function.blocks[from.0 as usize]
        .instructions
        .iter()
        .find(|instruction| instruction.out == Some(source))?;
    if instruction.ty.as_ref() != Some(&Type::Float) && !context.can_elide_i32_coercion(source) {
        return None;
    }
    let ControlFlowOp::Binary { op, lhs, rhs } = instruction.op else {
        return None;
    };
    match op {
        IrBinaryOp::Add
            if (lhs == target && is_int_constant(function, rhs, 1))
                || (rhs == target && is_int_constant(function, lhs, 1)) =>
        {
            Some(1)
        }
        IrBinaryOp::Sub if lhs == target && is_int_constant(function, rhs, 1) => Some(-1),
        _ => None,
    }
}

fn compound_assignment_copy(
    function: &ControlFlowFunction<'_>,
    from: BlockId,
    target: ValueId,
    source: ValueId,
    uses: &AHashMap<ValueId, usize>,
    context: &LocalNames,
) -> Option<(&'static str, String)> {
    if uses.get(&source).copied() != Some(1) {
        return None;
    }
    let instruction = function.blocks[from.0 as usize]
        .instructions
        .iter()
        .find(|instruction| instruction.out == Some(source))?;
    let ControlFlowOp::Binary { op, lhs, rhs } = instruction.op else {
        return None;
    };
    let integer_safe = context.can_elide_i32_coercion(source)
        || matches!(
            op,
            IrBinaryOp::BitAnd
                | IrBinaryOp::BitOr
                | IrBinaryOp::Xor
                | IrBinaryOp::ShiftLeft
                | IrBinaryOp::ShiftRight
        );
    if instruction.ty.as_ref() != Some(&Type::Float)
        && !(instruction.ty.as_ref() == Some(&Type::Int) && integer_safe)
    {
        return None;
    }
    let commutative = matches!(
        op,
        IrBinaryOp::Add
            | IrBinaryOp::Mul
            | IrBinaryOp::BitAnd
            | IrBinaryOp::BitOr
            | IrBinaryOp::Xor
    );
    let operand = if lhs == target {
        rhs
    } else if rhs == target && commutative {
        lhs
    } else {
        return None;
    };
    let operand = context
        .inlined_values
        .get(&operand)
        .cloned()
        .map(JsExpression::into_minimal)
        .or_else(|| context.value_names.get(&operand).cloned())?;
    let operator = match op {
        IrBinaryOp::Add => "+",
        IrBinaryOp::Sub => "-",
        IrBinaryOp::Mul => "*",
        IrBinaryOp::Div => "/",
        IrBinaryOp::Mod => "%",
        IrBinaryOp::BitAnd => "&",
        IrBinaryOp::BitOr => "|",
        IrBinaryOp::Xor => "^",
        IrBinaryOp::ShiftLeft => "<<",
        IrBinaryOp::ShiftRight => ">>",
        _ => return None,
    };
    Some((operator, operand))
}

fn is_int_constant(function: &ControlFlowFunction<'_>, value: ValueId, expected: i64) -> bool {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .any(|instruction| {
            instruction.out == Some(value)
                && matches!(instruction.op, ControlFlowOp::Const(ConstValue::Int(value)) if value == expected)
        })
}

fn parse_single_assignment(output: &str) -> Option<(bool, &str, &str, &str)> {
    let statement = output.strip_suffix(';')?;
    let (declare, statement) = statement
        .strip_prefix("var ")
        .map_or((false, statement), |statement| (true, statement));
    let (assignment_statement, trailing) = if declare {
        split_top_level_comma(statement).map_or((statement, ""), |index| {
            (&statement[..index], &statement[index..])
        })
    } else {
        (statement, "")
    };
    let assignment = assignment_statement.find('=')?;
    let target = &assignment_statement[..assignment];
    let value = &assignment_statement[assignment + 1..];
    (!target.is_empty()
        && target.bytes().all(is_js_identifier_byte)
        && !value.is_empty()
        && !value.contains(';'))
    .then_some((declare, target, value, trailing))
}

fn uninitialized_declaration_tail(trailing: &str) -> Option<&str> {
    let names = trailing.strip_prefix(',')?;
    (!names.is_empty()
        && names.split(',').all(|name| {
            !name.is_empty()
                && is_js_identifier_start(name.as_bytes()[0])
                && name.bytes().all(is_js_identifier_byte)
        }))
    .then_some(names)
}

fn split_top_level_comma(value: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in value.char_indices() {
        if let Some(active) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == active {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' | '`' => quote = Some(character),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => return Some(index),
            _ => {}
        }
    }
    None
}

fn is_nonzero_i32_literal(expression: &str) -> bool {
    expression.parse::<i32>().is_ok_and(|value| value != 0)
}

fn is_braceless_statement(output: &str) -> bool {
    let Some(statement) = output.strip_suffix(';') else {
        return false;
    };
    !statement.is_empty()
        && !expression_has_top_level_statement_break(statement)
        && !statement.starts_with('{')
        && !statement.starts_with("var ")
        && !statement.starts_with("let ")
        && !statement.starts_with("const ")
        && !statement.starts_with("function ")
        && !statement.starts_with("class ")
        && !statement.starts_with("if(")
        && !statement.starts_with("for(")
        && !statement.starts_with("while(")
}

fn is_comma_eligible_statement(output: &str) -> bool {
    is_braceless_statement(output)
        && !output.starts_with("return")
        && !output.starts_with("throw")
        && !output.starts_with("break")
        && !output.starts_with("continue")
}

fn compact_branch_expression(output: &str) -> Option<&str> {
    let expression = output.strip_suffix(';')?;
    is_comma_eligible_statement(output).then_some(expression)
}

fn compact_top_level_expression_statements(output: &str) -> Option<String> {
    let bytes = output.as_bytes();
    let mut statements = Vec::new();
    let mut start = 0usize;
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    for (index, byte) in bytes.iter().copied().enumerate() {
        if let Some(active) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == active {
                quote = None;
            }
            continue;
        }
        match byte {
            b'\'' | b'"' | b'`' => quote = Some(byte),
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b';' if depth == 0 => {
                let statement = &output[start..=index];
                if !is_comma_eligible_statement(statement) {
                    return None;
                }
                statements.push(
                    statement
                        .strip_suffix(';')
                        .expect("statement ends in semicolon"),
                );
                start = index + 1;
            }
            _ => {}
        }
    }
    if start != output.len() || statements.len() < 2 {
        return None;
    }
    let mut compact = statements.join(",");
    compact.push(';');
    Some(compact)
}

fn take_trailing_expression_statements(output: &mut String) -> Option<String> {
    let mut expressions = Vec::new();
    while let Some((start, expression)) = trailing_expression_statement(output) {
        expressions.push(expression.to_string());
        output.truncate(start);
    }
    if expressions.is_empty() {
        return None;
    }
    expressions.reverse();
    Some(expressions.join(","))
}

fn trailing_expression_statement(output: &str) -> Option<(usize, &str)> {
    if !output.ends_with(';') {
        return None;
    }
    let bytes = output.as_bytes();
    let mut statement_start = 0usize;
    let mut candidate = None;
    let mut delimiters = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    for (index, byte) in bytes.iter().copied().enumerate() {
        if let Some(active) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == active {
                quote = None;
            }
            continue;
        }
        match byte {
            b'\'' | b'"' | b'`' => quote = Some(byte),
            b'(' | b'[' => delimiters.push(byte),
            b')' | b']' => {
                delimiters.pop()?;
            }
            b'{' if delimiters.is_empty() => statement_start = index + 1,
            b'}' if delimiters.is_empty() => statement_start = index + 1,
            b';' if delimiters.is_empty() => {
                candidate = Some((statement_start, index + 1));
                statement_start = index + 1;
            }
            _ => {}
        }
    }
    let (start, end) = candidate?;
    if end != output.len() {
        return None;
    }
    let statement = &output[start..end];
    is_comma_eligible_statement(statement).then(|| {
        (
            start,
            statement
                .strip_suffix(';')
                .expect("statement ends in semicolon"),
        )
    })
}

fn push_conditional_arm(out: &mut String, expression: &str) {
    let grouped = split_top_level_comma(expression).is_some();
    if grouped {
        out.push('(');
    }
    out.push_str(expression);
    if grouped {
        out.push(')');
    }
}

fn expression_references_name(expression: &str, name: &str) -> bool {
    expression_identifier_spans(expression)
        .into_iter()
        .any(|(start, end)| &expression[start..end] == name)
}

fn expression_identifier_spans(expression: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut index = 0usize;
    scan_generated_js(expression.as_bytes(), &mut index, false, &mut spans);
    spans
}

fn scan_generated_js(
    bytes: &[u8],
    index: &mut usize,
    stop_at_brace: bool,
    spans: &mut Vec<(usize, usize)>,
) {
    while *index < bytes.len() {
        match bytes[*index] {
            b'\'' | b'"' => skip_generated_js_string(bytes, index),
            b'`' => scan_generated_js_template(bytes, index, spans),
            b'{' => {
                *index += 1;
                scan_generated_js(bytes, index, true, spans);
            }
            b'}' if stop_at_brace => {
                *index += 1;
                return;
            }
            byte if is_js_identifier_start(byte) => {
                let start = *index;
                *index += 1;
                while *index < bytes.len() && is_js_identifier_byte(bytes[*index]) {
                    *index += 1;
                }
                let property = bytes[..start]
                    .iter()
                    .rfind(|byte| !byte.is_ascii_whitespace())
                    .is_some_and(|byte| *byte == b'.');
                if !property {
                    spans.push((start, *index));
                }
            }
            _ => *index += 1,
        }
    }
}

fn skip_generated_js_string(bytes: &[u8], index: &mut usize) {
    let quote = bytes[*index];
    *index += 1;
    while *index < bytes.len() {
        match bytes[*index] {
            b'\\' => *index = (*index + 2).min(bytes.len()),
            byte if byte == quote => {
                *index += 1;
                return;
            }
            _ => *index += 1,
        }
    }
}

fn scan_generated_js_template(bytes: &[u8], index: &mut usize, spans: &mut Vec<(usize, usize)>) {
    *index += 1;
    while *index < bytes.len() {
        match bytes[*index] {
            b'\\' => *index = (*index + 2).min(bytes.len()),
            b'`' => {
                *index += 1;
                return;
            }
            b'$' if bytes.get(*index + 1) == Some(&b'{') => {
                *index += 2;
                scan_generated_js(bytes, index, true, spans);
            }
            _ => *index += 1,
        }
    }
}

fn reserve_expression_identifiers(mangler: &mut Mangler, expression: &str) {
    let bytes = expression.as_bytes();
    let mut start = 0;
    while start < bytes.len() {
        if !is_js_identifier_start(bytes[start]) {
            start += 1;
            continue;
        }
        let mut end = start + 1;
        while end < bytes.len() && is_js_identifier_byte(bytes[end]) {
            end += 1;
        }
        mangler.reserve(&expression[start..end]);
        start = end;
    }
}

const fn is_js_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_' || byte == b'$'
}

const fn is_js_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

struct LocalNames {
    value_names: AHashMap<ValueId, String>,
    parameter_values: AHashSet<ValueId>,
    stored_values: AHashSet<ValueId>,
    untyped_values: AHashSet<ValueId>,
    inlined_values: AHashMap<ValueId, JsExpression>,
    string_constants: AHashMap<ValueId, String>,
    global_loads: AHashMap<ValueId, SymbolId>,
    integer_ranges: AHashMap<ValueId, crate::value_analysis::I32Range>,
    elidable_i32_coercions: AHashSet<ValueId>,
    elidable_map_get_normalizations: AHashSet<ValueId>,
    null_values: AHashSet<ValueId>,
    truthy_nullable_values: AHashSet<ValueId>,
    safe_int_array_reads: AHashSet<ValueId>,
    safe_in_place_updates: AHashSet<(ValueId, ValueId)>,
    parallel_copy_temp: Option<String>,
    live_in_values: Vec<AHashSet<ValueId>>,
    use_counts: AHashMap<ValueId, usize>,
    declared_names: RefCell<AHashSet<String>>,
    inline_declarations: bool,
    state: String,
    function_name: String,
    function_span: crate::span::Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PathEnd {
    ReachedStop,
    Terminated,
}

#[derive(Debug, Clone, Copy)]
struct LoopContext {
    header: BlockId,
    continue_target: BlockId,
    update: Option<BlockId>,
    exit: BlockId,
}

fn can_structure(function: &ControlFlowFunction<'_>) -> bool {
    let mut shaped_headers = function
        .shapes
        .iter()
        .map(ControlShape::header)
        .collect::<AHashSet<_>>();
    for shape in &function.shapes {
        let ControlShape::Loop {
            header, body, exit, ..
        } = shape
        else {
            continue;
        };
        if let Some(condition) = loop_condition_branch(function, *header, *body, *exit) {
            shaped_headers.insert(condition);
        }
    }
    function.blocks.iter().all(|block| {
        !matches!(block.terminator, Some(Terminator::Branch { .. }))
            || shaped_headers.contains(&block.id)
    })
}

fn loop_condition_branch(
    function: &ControlFlowFunction<'_>,
    entry: BlockId,
    body: BlockId,
    exit: BlockId,
) -> Option<BlockId> {
    let mut pending = vec![entry];
    let mut visited = AHashSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current) || current == body || current == exit {
            continue;
        }
        let block = function.blocks.get(current.0 as usize)?;
        if matches!(
            block.terminator,
            Some(Terminator::Branch {
                then_block,
                else_block,
                ..
            }) if (then_block == body && else_block == exit)
                || (then_block == exit && else_block == body)
        ) {
            return Some(current);
        }
        let mut successors = block_successors(block);
        successors.sort_unstable_by_key(|block| std::cmp::Reverse(block.0));
        pending.extend(successors);
    }
    None
}

fn can_inline_closure(function: &ControlFlowFunction<'_>, inline_structured: bool) -> bool {
    if function.kind != FunctionKind::Closure {
        return false;
    }
    if function.blocks.len() == 1 {
        return function.blocks[0].phis.is_empty()
            && matches!(function.blocks[0].terminator, Some(Terminator::Return(_)))
            && function.blocks[0].instructions.len() <= 8;
    }
    inline_structured
        && can_structure(function)
        && function
            .blocks
            .iter()
            .map(|block| block.instructions.len() + block.phis.len())
            .sum::<usize>()
            <= 80
}

fn render_arrow_parameters(
    function: &ControlFlowFunction<'_>,
    context: &LocalNames,
    compact_boolean_literals: bool,
) -> Result<String, CodegenError> {
    let params = &function.params[function.capture_count..];
    let has_default = params.iter().any(|param| param.default.is_some());
    if let [param] = params {
        if !has_default {
            return Ok(context.value_name(param.value)?.to_string());
        }
    }
    let mut rendered = String::from("(");
    for (index, param) in params.iter().enumerate() {
        if index != 0 {
            rendered.push(',');
        }
        rendered.push_str(context.value_name(param.value)?);
        if let Some(default) = &param.default {
            rendered.push('=');
            rendered.push_str(&render_param_default(
                default,
                function,
                context,
                compact_boolean_literals,
            )?);
        }
    }
    rendered.push(')');
    Ok(rendered)
}

fn function_parameters(parameters: &str) -> String {
    if parameters.starts_with('(') {
        parameters.to_string()
    } else {
        format!("({parameters})")
    }
}

fn fold_default_assignment_into_first_field(
    assignment: &str,
    returned: &str,
    parameter: &str,
) -> Option<String> {
    let object = returned.strip_prefix('{')?;
    let colon = object.find(':')?;
    let field = &object[..colon];
    if field.is_empty()
        || field
            .bytes()
            .any(|byte| matches!(byte, b'{' | b'}' | b'[' | b']' | b'(' | b')' | b','))
    {
        return None;
    }
    let value_start = 1 + colon + 1;
    if !returned[value_start..].starts_with(parameter)
        || !matches!(
            returned.as_bytes().get(value_start + parameter.len()),
            Some(b',') | Some(b'}')
        )
        || !assignment.starts_with(parameter)
    {
        return None;
    }
    let mut folded = String::with_capacity(returned.len() + assignment.len() - parameter.len());
    folded.push_str(&returned[..value_start]);
    folded.push_str(assignment);
    folded.push_str(&returned[value_start + parameter.len()..]);
    Some(folded)
}

fn rewrite_self_default_conditional(
    expression: &str,
    function: &str,
    parameter: &str,
    non_null_is_truthy: bool,
) -> Option<(String, String)> {
    let (condition, recursive, returned) = split_top_level_conditional(expression)?;
    let operator = if condition == format!("!{parameter}") {
        "||="
    } else if condition == format!("{parameter}==null") {
        if non_null_is_truthy {
            "||="
        } else {
            "??="
        }
    } else {
        return None;
    };
    let argument = recursive
        .strip_prefix(function)?
        .strip_prefix('(')?
        .strip_suffix(')')?;
    if argument.is_empty() || split_top_level_comma(argument).is_some() {
        return None;
    }
    Some((
        format!("{parameter}{operator}{argument}"),
        returned.to_string(),
    ))
}

fn split_top_level_conditional(expression: &str) -> Option<(&str, &str, &str)> {
    let bytes = expression.as_bytes();
    let mut delimiter_depth = 0_i32;
    let mut quote = None;
    let mut escaped = false;
    let mut question = None;
    for (index, byte) in bytes.iter().copied().enumerate() {
        if let Some(active) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == active {
                quote = None;
            }
            continue;
        }
        match byte {
            b'\'' | b'"' | b'`' => quote = Some(byte),
            b'(' | b'[' | b'{' => delimiter_depth += 1,
            b')' | b']' | b'}' => delimiter_depth -= 1,
            b'?' if delimiter_depth == 0 && question.is_none() => question = Some(index),
            b':' if delimiter_depth == 0 => {
                let question = question?;
                return Some((
                    &expression[..question],
                    &expression[question + 1..index],
                    &expression[index + 1..],
                ));
            }
            _ => {}
        }
    }
    None
}

fn render_param_default(
    default: &crate::ir::IrParamDefault<'_>,
    function: &ControlFlowFunction<'_>,
    context: &LocalNames,
    compact_boolean_literals: bool,
) -> Result<String, CodegenError> {
    match default {
        crate::ir::IrParamDefault::Const(value) => Ok(render_const(
            value,
            compact_boolean_literals,
            StringQuote::Double,
        )),
        crate::ir::IrParamDefault::Value(value) => Ok(context.value_name(*value)?.to_string()),
        crate::ir::IrParamDefault::Name(name) => {
            let source = function
                .params
                .iter()
                .find(|parameter| parameter.name == *name)
                .ok_or_else(|| {
                    CodegenError::new(function.span, format!("missing default parameter `{name}`"))
                })?;
            Ok(context.value_name(source.value)?.to_string())
        }
    }
}

fn shape_at(function: &ControlFlowFunction<'_>, block: BlockId) -> Option<ControlShape> {
    if !matches!(
        function.blocks[block.0 as usize].terminator,
        Some(Terminator::Branch { .. })
    ) {
        return None;
    }
    function
        .shapes
        .iter()
        .find(|shape| shape.header() == block)
        .cloned()
}

fn immediately_branches_on_phi(
    function: &ControlFlowFunction<'_>,
    block: BlockId,
    value: ValueId,
) -> bool {
    let block = &function.blocks[block.0 as usize];
    block.instructions.is_empty()
        && matches!(block.terminator, Some(Terminator::Branch { condition, .. }) if condition == value)
        && function
            .shapes
            .iter()
            .any(|shape| matches!(shape, ControlShape::If { header, .. } if *header == block.id))
}

impl LocalNames {
    fn new(
        function: &ControlFlowFunction<'_>,
        integer_facts: &FunctionIntegerFacts,
        all_values: bool,
        parent: &Mangler,
        preferred_local_names: &AHashMap<String, String>,
        numeric_aliases: &AHashMap<String, String>,
        options: &IrJsOptions,
    ) -> Self {
        let mangle_identifiers = options.mangle_identifiers;
        let compact_boolean_literals = options.compact_boolean_literals;
        let scalar_phi_copies = options.scalar_phi_copies;
        let mut mangler = parent.clone();
        let mut value_names = AHashMap::new();
        let parameter_values = function
            .params
            .iter()
            .map(|param| param.value)
            .collect::<AHashSet<_>>();
        let mut stored_values = AHashSet::new();
        let untyped_values = function
            .value_escapes
            .iter()
            .enumerate()
            .filter_map(|(index, escape)| {
                (*escape == EscapeState::EscapesToUntypedBoundary).then_some(ValueId(index as u32))
            })
            .collect();
        let uses = use_counts(function);
        let unstable_values = unstable_values(function);
        let captured_values = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match &instruction.op {
                ControlFlowOp::Closure { captures, .. } => Some(captures),
                _ => None,
            })
            .flatten()
            .copied()
            .collect::<AHashSet<_>>();
        let inlined_values = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match (instruction.out, &instruction.op) {
                (
                    Some(out),
                    ControlFlowOp::Const(
                        value @ (ConstValue::Int(_) | ConstValue::Float(_) | ConstValue::Bool(_)),
                    ),
                ) => {
                    let rendered =
                        render_const(value, compact_boolean_literals, StringQuote::Double);
                    let use_count = uses.get(&out).copied().unwrap_or(0);
                    let inline_cost = rendered.len() * use_count;
                    let binding_cost = rendered.len() + 7 + use_count;
                    let rendered = numeric_aliases.get(&rendered).cloned().unwrap_or(rendered);
                    (inline_cost <= binding_cost).then_some((out, JsExpression::atom(rendered)))
                }
                _ => None,
            })
            .collect::<AHashMap<_, _>>();
        let string_constants = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match (instruction.out, &instruction.op) {
                (Some(out), ControlFlowOp::Const(ConstValue::String(value))) => {
                    Some((out, value.to_string()))
                }
                _ => None,
            })
            .collect();
        let null_values = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match (&instruction.op, instruction.out) {
                (ControlFlowOp::Const(ConstValue::Null), Some(out)) => Some(out),
                _ => None,
            })
            .collect();
        let mut truthy_nullable_values = function
            .params
            .iter()
            .filter_map(|parameter| {
                is_nullable_with_truthy_value(&parameter.ty).then_some(parameter.value)
            })
            .collect::<AHashSet<_>>();
        for block in &function.blocks {
            truthy_nullable_values.extend(
                block
                    .phis
                    .iter()
                    .filter_map(|phi| is_nullable_with_truthy_value(&phi.ty).then_some(phi.out)),
            );
            truthy_nullable_values.extend(block.instructions.iter().filter_map(|instruction| {
                instruction.out.filter(|_| {
                    instruction
                        .ty
                        .as_ref()
                        .is_some_and(is_nullable_with_truthy_value)
                })
            }));
        }
        let global_loads = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match (instruction.out, &instruction.op) {
                (Some(out), ControlFlowOp::LoadGlobal(symbol)) => Some((out, *symbol)),
                _ => None,
            })
            .collect();
        let safe_int_array_reads = safe_int_array_reads(function, integer_facts);
        let cross_block = cross_block_values(function);
        let mut values = function
            .params
            .iter()
            .map(|param| param.value)
            .collect::<Vec<_>>();
        for block in &function.blocks {
            for phi in &block.phis {
                values.push(phi.out);
                stored_values.insert(phi.out);
            }
            for (index, instruction) in block.instructions.iter().enumerate() {
                if let Some(value) = instruction.out {
                    values.push(value);
                    let use_count = uses.get(&value).copied().unwrap_or(0);
                    let fused = use_count == 1 && can_fuse_value(block, index, value);
                    if (cross_block.contains(&value)
                        || use_count > 1
                        || (captured_values.contains(&value)
                            && !matches!(instruction.op, ControlFlowOp::Const(_)))
                        || (use_count != 0 && unstable_values.contains(&value) && !fused)
                        || matches!(
                            instruction.op,
                            ControlFlowOp::NewClass {
                                constructor: Some(_),
                                ..
                            } | ControlFlowOp::Intrinsic {
                                intrinsic: Intrinsic::JsForInKey,
                                ..
                            }
                        ))
                        && !inlined_values.contains_key(&value)
                    {
                        stored_values.insert(value);
                    }
                }
            }
        }
        let stable_constructor_values = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match &instruction.op {
                ControlFlowOp::Const(_) => instruction.out,
                ControlFlowOp::Closure { captures, .. } if captures.is_empty() => instruction.out,
                _ => None,
            })
            .collect::<AHashSet<_>>();
        for argument in function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match &instruction.op {
                ControlFlowOp::NewClass {
                    constructor: Some(_),
                    args,
                    ..
                } => Some(args),
                _ => None,
            })
            .flatten()
        {
            if !stable_constructor_values.contains(argument)
                && !inlined_values.contains_key(argument)
            {
                stored_values.insert(*argument);
            }
        }
        values.sort_unstable_by_key(|value| value.0);
        values.dedup();
        values.sort_unstable_by(|left, right| {
            let left_emitted = parameter_values.contains(left) || stored_values.contains(left);
            let right_emitted = parameter_values.contains(right) || stored_values.contains(right);
            right_emitted
                .cmp(&left_emitted)
                .then_with(|| {
                    uses.get(right)
                        .copied()
                        .unwrap_or(0)
                        .cmp(&uses.get(left).copied().unwrap_or(0))
                })
                .then_with(|| left.0.cmp(&right.0))
        });
        let state = if all_values {
            if mangle_identifiers {
                mangler.next_name()
            } else {
                mangler.unique_name("$state")
            }
        } else {
            String::new()
        };
        let named_values = stored_values
            .union(&parameter_values)
            .copied()
            .collect::<AHashSet<_>>();
        let safe_in_place_updates = safe_two_address_phi_pairs(function, &named_values, false);
        if mangle_identifiers && !named_values.is_empty() {
            let colors = coalesce_value_names(
                function,
                &stored_values,
                &parameter_values,
                &uses,
                options.phi_affinity_mode,
                preferred_local_names,
            );
            let color_count = colors.values().copied().max().map_or(0, |color| color + 1);
            let mut color_names = vec![String::new(); color_count];
            if options.stable_local_names {
                let mut preferences = vec![AHashMap::<String, usize>::new(); color_count];
                for (value, color) in &colors {
                    let Some(identifier) = function
                        .value_local_hints
                        .get(value.0 as usize)
                        .copied()
                        .flatten()
                        .and_then(|local| preferred_local_names.get(local))
                    else {
                        continue;
                    };
                    *preferences[*color].entry(identifier.clone()).or_insert(0) +=
                        uses.get(value).copied().unwrap_or(0) + 1;
                }
                let first_value = |color| {
                    colors
                        .iter()
                        .filter_map(|(value, candidate)| (*candidate == color).then_some(value.0))
                        .min()
                        .unwrap_or(u32::MAX)
                };
                let best_score =
                    |color: usize| preferences[color].values().copied().max().unwrap_or(0);
                let mut color_order = (0..color_count).collect::<Vec<_>>();
                color_order.sort_unstable_by(|left, right| {
                    best_score(*right)
                        .cmp(&best_score(*left))
                        .then_with(|| first_value(*left).cmp(&first_value(*right)))
                });
                for color in color_order {
                    let mut candidates = preferences[color].iter().collect::<Vec<_>>();
                    candidates.sort_unstable_by(|left, right| {
                        right.1.cmp(left.1).then_with(|| left.0.cmp(right.0))
                    });
                    if let Some((identifier, _)) = candidates
                        .into_iter()
                        .find(|(identifier, _)| mangler.claim_name(identifier))
                    {
                        color_names[color] = identifier.clone();
                    }
                }
            }
            for name in &mut color_names {
                if name.is_empty() {
                    *name = mangler.next_name();
                }
            }
            for value in &values {
                if let Some(color) = colors.get(value) {
                    value_names.insert(*value, color_names[*color].clone());
                }
            }
        }
        for value in values {
            value_names.entry(value).or_insert_with(|| {
                if mangle_identifiers {
                    mangler.next_name()
                } else {
                    let preferred = function
                        .params
                        .iter()
                        .find(|parameter| parameter.value == value)
                        .map_or_else(
                            || format!("v{}", value.0),
                            |parameter| parameter.name.into(),
                        );
                    mangler.unique_name(&preferred)
                }
            });
        }
        let declared_names = function
            .params
            .iter()
            .filter_map(|parameter| value_names.get(&parameter.value))
            .cloned()
            .collect();
        let parallel_copy_temp = scalar_phi_copies.then(|| mangler.next_name());
        let live_in_values = if scalar_phi_copies {
            live_in_values(function)
        } else {
            Vec::new()
        };
        Self {
            value_names,
            parameter_values,
            stored_values,
            untyped_values,
            inlined_values,
            string_constants,
            global_loads,
            integer_ranges: integer_facts.ranges().clone(),
            elidable_i32_coercions: function
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .filter_map(|instruction| instruction.out)
                .filter(|value| integer_facts.can_elide_coercion(*value))
                .collect(),
            elidable_map_get_normalizations: map_get_normalization_elisions(function),
            null_values,
            truthy_nullable_values,
            safe_int_array_reads,
            safe_in_place_updates,
            parallel_copy_temp,
            live_in_values,
            use_counts: uses,
            declared_names: RefCell::new(declared_names),
            inline_declarations: false,
            state,
            function_name: function
                .name
                .unwrap_or(if function.kind == FunctionKind::Entry {
                    "<entry>"
                } else {
                    "<closure>"
                })
                .to_string(),
            function_span: function.span,
        }
    }

    fn value_name(&self, value: ValueId) -> Result<&str, CodegenError> {
        self.value_names
            .get(&value)
            .map(String::as_str)
            .ok_or_else(|| {
                CodegenError::new(
                    self.function_span,
                    format!(
                        "SSA value {} has no emitted name in function `{}`",
                        value.0, self.function_name
                    ),
                )
            })
    }

    fn state_name(&self) -> &str {
        &self.state
    }

    fn is_untyped(&self, value: ValueId) -> bool {
        self.untyped_values.contains(&value)
    }

    fn is_stored(&self, value: ValueId) -> bool {
        self.stored_values.contains(&value)
    }

    fn is_name_declared(&self, value: ValueId) -> bool {
        self.value_names
            .get(&value)
            .is_some_and(|name| self.declared_names.borrow().contains(name))
    }

    fn is_safe_in_place_update(&self, output: ValueId, previous: ValueId) -> bool {
        self.safe_in_place_updates.contains(&(output, previous))
            || self.safe_in_place_updates.contains(&(previous, output))
    }

    fn can_elide_i32_coercion(&self, value: ValueId) -> bool {
        self.elidable_i32_coercions.contains(&value)
    }

    fn integer_range_excludes_zero(&self, value: ValueId) -> bool {
        self.integer_ranges
            .get(&value)
            .is_some_and(|range| range.min > 0 || range.max < 0)
    }

    fn can_elide_map_get_normalization(&self, value: ValueId) -> bool {
        self.elidable_map_get_normalizations.contains(&value)
    }

    fn can_elide_int_array_read(&self, value: Option<ValueId>) -> bool {
        value.is_some_and(|value| self.safe_int_array_reads.contains(&value))
    }

    fn truthy_nullable_operand(&self, lhs: ValueId, rhs: ValueId) -> Option<ValueId> {
        if self.null_values.contains(&lhs) && self.truthy_nullable_values.contains(&rhs) {
            Some(rhs)
        } else if self.null_values.contains(&rhs) && self.truthy_nullable_values.contains(&lhs) {
            Some(lhs)
        } else {
            None
        }
    }

    fn string_index_in_bounds(&self, object: ValueId, index: ValueId) -> Option<bool> {
        let range = self.integer_ranges.get(&index)?;
        if range.min < 0 {
            return Some(false);
        }
        let length = self.string_constants.get(&object)?.encode_utf16().count();
        Some(range.max < length as i64)
    }

    fn claim_declaration(&self, value: ValueId) -> Result<bool, CodegenError> {
        if !self.inline_declarations {
            return Ok(false);
        }
        let name = self.value_name(value)?.to_string();
        Ok(self.declared_names.borrow_mut().insert(name))
    }

    fn claim_remaining_declarations(&self) -> Vec<String> {
        if !self.inline_declarations {
            return Vec::new();
        }
        let mut values = self.stored_values.iter().copied().collect::<Vec<_>>();
        values.sort_unstable_by_key(|value| value.0);
        let mut declared = self.declared_names.borrow_mut();
        values
            .into_iter()
            .filter_map(|value| self.value_names.get(&value))
            .filter(|name| declared.insert((*name).clone()))
            .cloned()
            .collect()
    }

    fn non_parameter_names(&self, function: &ControlFlowFunction<'_>) -> Vec<&str> {
        let parameter_names = function
            .params
            .iter()
            .filter_map(|parameter| self.value_names.get(&parameter.value))
            .cloned()
            .collect::<AHashSet<_>>();
        let mut values = function
            .blocks
            .iter()
            .flat_map(|block| {
                block.phis.iter().map(|phi| phi.out).chain(
                    block
                        .instructions
                        .iter()
                        .filter_map(|instruction| instruction.out),
                )
            })
            .filter(|value| !self.parameter_values.contains(value))
            .filter(|value| self.stored_values.contains(value))
            .collect::<Vec<_>>();
        values.sort_by_key(|value| value.0);
        values.dedup();
        let mut seen = AHashSet::new();
        values
            .into_iter()
            .filter_map(|value| self.value_names.get(&value).map(String::as_str))
            .filter(|name| !parameter_names.contains(*name))
            .filter(|name| seen.insert((*name).to_string()))
            .collect()
    }
}

fn unstable_values(function: &ControlFlowFunction<'_>) -> AHashSet<ValueId> {
    let mut unstable = AHashSet::new();
    loop {
        let mut changed = false;
        for block in &function.blocks {
            for phi in &block.phis {
                if phi
                    .incoming
                    .iter()
                    .any(|(_, value)| unstable.contains(value))
                {
                    changed |= unstable.insert(phi.out);
                }
            }
            for instruction in &block.instructions {
                let Some(out) = instruction.out else {
                    continue;
                };
                if !op_can_defer(&instruction.op)
                    || op_values(&instruction.op)
                        .iter()
                        .any(|value| unstable.contains(value))
                {
                    changed |= unstable.insert(out);
                }
            }
        }
        if !changed {
            return unstable;
        }
    }
}

fn emit_binding_prefix(
    context: &LocalNames,
    value: ValueId,
    predeclared: bool,
    out: &mut String,
) -> Result<(), CodegenError> {
    if context.claim_declaration(value)? {
        out.push_str(if predeclared { "var " } else { "let " });
    }
    Ok(())
}

fn coalesce_value_names(
    function: &ControlFlowFunction<'_>,
    stored_values: &AHashSet<ValueId>,
    parameter_values: &AHashSet<ValueId>,
    uses: &AHashMap<ValueId, usize>,
    phi_affinity_mode: PhiAffinityMode,
    preferred_local_names: &AHashMap<String, String>,
) -> AHashMap<ValueId, usize> {
    let named = stored_values
        .union(parameter_values)
        .copied()
        .collect::<AHashSet<_>>();
    let two_address_phi_pairs = safe_two_address_phi_pairs(function, &named, true);
    let value_definitions = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| instruction.out.map(|out| (out, &instruction.op)))
        .collect::<AHashMap<_, _>>();
    let named_values = &named;
    let deferred_definitions = function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .instructions
                .iter()
                .enumerate()
                .filter_map(move |(index, instruction)| {
                    let out = instruction.out?;
                    (uses.get(&out).copied() == Some(1)
                        && !named_values.contains(&out)
                        && (op_can_defer(&instruction.op) || can_fuse_value(block, index, out)))
                    .then_some(out)
                })
        })
        .collect::<AHashSet<_>>();
    let block_count = function.blocks.len();
    let mut block_definitions = vec![AHashSet::<ValueId>::new(); block_count];
    let mut local_uses = vec![AHashSet::<ValueId>::new(); block_count];
    let mut phi_definitions = vec![AHashSet::<ValueId>::new(); block_count];

    for block in &function.blocks {
        let index = block.id.0 as usize;
        for phi in &block.phis {
            if named.contains(&phi.out) {
                block_definitions[index].insert(phi.out);
                phi_definitions[index].insert(phi.out);
            }
        }
        for instruction in &block.instructions {
            let mut operands = AHashSet::new();
            for value in op_values(&instruction.op) {
                collect_deferred_named_values(
                    value,
                    &named,
                    &value_definitions,
                    &deferred_definitions,
                    &mut AHashSet::new(),
                    &mut operands,
                );
            }
            for value in operands {
                if !block_definitions[index].contains(&value) {
                    local_uses[index].insert(value);
                }
            }
            if let Some(out) = instruction.out.filter(|out| named.contains(out)) {
                block_definitions[index].insert(out);
            }
        }
        for value in block
            .terminator
            .as_ref()
            .into_iter()
            .flat_map(terminator_values)
        {
            let mut operands = AHashSet::new();
            collect_deferred_named_values(
                value,
                &named,
                &value_definitions,
                &deferred_definitions,
                &mut AHashSet::new(),
                &mut operands,
            );
            for operand in operands {
                if !block_definitions[index].contains(&operand) {
                    local_uses[index].insert(operand);
                }
            }
        }
    }

    let mut live_in = vec![AHashSet::<ValueId>::new(); block_count];
    let mut live_out = vec![AHashSet::<ValueId>::new(); block_count];
    loop {
        let mut changed = false;
        for block in function.blocks.iter().rev() {
            let index = block.id.0 as usize;
            let mut out = AHashSet::new();
            for successor in block_successors(block) {
                let successor_index = successor.0 as usize;
                out.extend(
                    live_in[successor_index]
                        .difference(&phi_definitions[successor_index])
                        .copied(),
                );
                for phi in &function.blocks[successor_index].phis {
                    if let Some((_, value)) = phi
                        .incoming
                        .iter()
                        .find(|(predecessor, _)| predecessor == &block.id)
                    {
                        collect_deferred_named_values(
                            *value,
                            &named,
                            &value_definitions,
                            &deferred_definitions,
                            &mut AHashSet::new(),
                            &mut out,
                        );
                    }
                }
            }
            let mut input = local_uses[index].clone();
            input.extend(out.difference(&block_definitions[index]).copied());
            if out != live_out[index] {
                live_out[index] = out;
                changed = true;
            }
            if input != live_in[index] {
                live_in[index] = input;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let mut interference = named
        .iter()
        .map(|value| (*value, AHashSet::<ValueId>::new()))
        .collect::<AHashMap<_, _>>();
    let mut connect = |left: ValueId, right: ValueId| {
        if left != right
            && !two_address_phi_pairs.contains(&(left, right))
            && !two_address_phi_pairs.contains(&(right, left))
        {
            interference.entry(left).or_default().insert(right);
            interference.entry(right).or_default().insert(left);
        }
    };
    for block in &function.blocks {
        let index = block.id.0 as usize;
        let mut live = live_out[index].clone();
        for value in block
            .terminator
            .as_ref()
            .into_iter()
            .flat_map(terminator_values)
        {
            collect_deferred_named_values(
                value,
                &named,
                &value_definitions,
                &deferred_definitions,
                &mut AHashSet::new(),
                &mut live,
            );
        }
        for instruction in block.instructions.iter().rev() {
            let mut operands = AHashSet::new();
            for value in op_values(&instruction.op) {
                collect_deferred_named_values(
                    value,
                    &named,
                    &value_definitions,
                    &deferred_definitions,
                    &mut AHashSet::new(),
                    &mut operands,
                );
            }
            if let Some(out) = instruction.out.filter(|out| named.contains(out)) {
                if matches!(
                    instruction.op,
                    ControlFlowOp::NewClass {
                        constructor: Some(_),
                        ..
                    }
                ) {
                    for operand in &operands {
                        connect(out, *operand);
                    }
                }
                for value in &live {
                    connect(out, *value);
                }
                live.remove(&out);
            }
            for value in operands {
                live.insert(value);
            }
        }
        let phi_values = block
            .phis
            .iter()
            .map(|phi| phi.out)
            .filter(|value| named.contains(value))
            .collect::<Vec<_>>();
        for (position, value) in phi_values.iter().enumerate() {
            for live_value in &live {
                connect(*value, *live_value);
            }
            for other in &phi_values[position + 1..] {
                connect(*value, *other);
            }
            live.remove(value);
        }
    }

    let parameters = function
        .params
        .iter()
        .map(|parameter| parameter.value)
        .filter(|value| named.contains(value))
        .collect::<Vec<_>>();
    for (position, parameter) in parameters.iter().enumerate() {
        for other in &parameters[position + 1..] {
            connect(*parameter, *other);
        }
    }

    let captured = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match &instruction.op {
            ControlFlowOp::Closure { captures, .. } => Some(captures),
            _ => None,
        })
        .flatten()
        .filter(|value| named.contains(value))
        .copied()
        .collect::<AHashSet<_>>();
    for capture in captured {
        for value in &named {
            connect(capture, *value);
        }
    }

    let preferred_values = named
        .iter()
        .filter_map(|value| {
            let identifier = function
                .value_local_hints
                .get(value.0 as usize)
                .copied()
                .flatten()
                .and_then(|local| preferred_local_names.get(local))?;
            Some((*value, identifier))
        })
        .collect::<Vec<_>>();
    for (position, (left, left_name)) in preferred_values.iter().enumerate() {
        for (right, right_name) in &preferred_values[position + 1..] {
            if left_name != right_name {
                connect(*left, *right);
            }
        }
    }

    let mut values = named.into_iter().collect::<Vec<_>>();
    let parameter_order = function
        .params
        .iter()
        .enumerate()
        .map(|(index, parameter)| (parameter.value, index))
        .collect::<AHashMap<_, _>>();
    let mut affinities = AHashMap::<ValueId, Vec<ValueId>>::new();
    for block in &function.blocks {
        for phi in &block.phis {
            if !interference.contains_key(&phi.out) {
                continue;
            }
            for (_, incoming) in &phi.incoming {
                if interference.contains_key(incoming) && !interference[&phi.out].contains(incoming)
                {
                    affinities.entry(phi.out).or_default().push(*incoming);
                    affinities.entry(*incoming).or_default().push(phi.out);
                }
            }
        }
    }
    for (left, right) in &two_address_phi_pairs {
        if interference.contains_key(left)
            && interference.contains_key(right)
            && !interference[left].contains(right)
        {
            affinities.entry(*left).or_default().push(*right);
            affinities.entry(*right).or_default().push(*left);
        }
    }
    values.sort_unstable_by(|left, right| {
        parameter_order
            .contains_key(right)
            .cmp(&parameter_order.contains_key(left))
            .then_with(|| {
                parameter_order
                    .get(left)
                    .unwrap_or(&usize::MAX)
                    .cmp(parameter_order.get(right).unwrap_or(&usize::MAX))
            })
            .then_with(|| {
                uses.get(right)
                    .copied()
                    .unwrap_or(0)
                    .cmp(&uses.get(left).copied().unwrap_or(0))
            })
            .then_with(|| {
                interference
                    .get(right)
                    .map_or(0, |values| values.len())
                    .cmp(&interference.get(left).map_or(0, |values| values.len()))
            })
            .then_with(|| left.0.cmp(&right.0))
    });
    if phi_affinity_mode == PhiAffinityMode::Grouped {
        return color_phi_affinity_groups(
            &values,
            &interference,
            &affinities,
            &parameter_order,
            uses,
        );
    }
    let mut colors = AHashMap::<ValueId, usize>::new();
    for value in values {
        let unavailable = interference
            .get(&value)
            .into_iter()
            .flatten()
            .filter_map(|neighbor| colors.get(neighbor).copied())
            .collect::<AHashSet<_>>();
        let preferred = affinities
            .get(&value)
            .into_iter()
            .flatten()
            .filter_map(|affinity| colors.get(affinity).copied())
            .filter(|color| !unavailable.contains(color))
            .min();
        let color =
            preferred.unwrap_or_else(|| (0..).find(|color| !unavailable.contains(color)).unwrap());
        colors.insert(value, color);
    }
    colors
}

fn safe_two_address_phi_pairs(
    function: &ControlFlowFunction<'_>,
    named: &AHashSet<ValueId>,
    loop_headers_only: bool,
) -> AHashSet<(ValueId, ValueId)> {
    let definitions = function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .instructions
                .iter()
                .enumerate()
                .filter_map(move |(index, instruction)| {
                    instruction
                        .out
                        .map(|out| (out, (block.id, index, instruction)))
                })
        })
        .collect::<AHashMap<_, _>>();
    let phi_definitions = function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .phis
                .iter()
                .map(move |phi| (phi.out, (block.id, phi.local)))
        })
        .collect::<AHashMap<_, _>>();
    let mut pairs = AHashSet::new();
    for block in &function.blocks {
        if loop_headers_only && !block_is_on_cycle(function, block.id) {
            continue;
        }
        for phi in &block.phis {
            if !named.contains(&phi.out) {
                continue;
            }
            for (predecessor, incoming) in &phi.incoming {
                if !named.contains(incoming) {
                    continue;
                }
                if !block_dominates(function, block.id, *predecessor)
                    && value_type(function, *incoming).as_ref() == Some(&phi.ty)
                    && value_is_unused_until_block(function, block.id, block.id, *incoming, phi.out)
                {
                    pairs.insert((phi.out, *incoming));
                    continue;
                }
                if let Some((incoming_block, incoming_local)) = phi_definitions.get(incoming) {
                    if *incoming_local == phi.local
                        && value_is_unused_until_block(
                            function,
                            block.id,
                            *incoming_block,
                            *incoming,
                            phi.out,
                        )
                    {
                        pairs.insert((phi.out, *incoming));
                        continue;
                    }
                }
                let Some((definition_block, definition_index, instruction)) =
                    definitions.get(incoming)
                else {
                    continue;
                };
                let ControlFlowOp::Binary { lhs, rhs, .. } = instruction.op else {
                    continue;
                };
                if lhs != phi.out && rhs != phi.out {
                    continue;
                }
                if instruction.ty.as_ref() != Some(&phi.ty) {
                    continue;
                }
                if target_is_unused_until_phi_redefinition(
                    function,
                    *definition_block,
                    *definition_index,
                    block.id,
                    phi.out,
                ) {
                    pairs.insert((phi.out, *incoming));
                }
            }
        }
    }
    loop {
        let snapshot = pairs.clone();
        let mut changed = false;
        for block in &function.blocks {
            if loop_headers_only && !block_is_on_cycle(function, block.id) {
                continue;
            }
            for phi in &block.phis {
                for (candidate, (definition_block, definition_index, instruction)) in &definitions {
                    if candidate == &phi.out
                        || snapshot.contains(&(phi.out, *candidate))
                        || !values_are_connected(&snapshot, phi.out, *candidate)
                        || instruction.ty.as_ref() != Some(&phi.ty)
                    {
                        continue;
                    }
                    if target_is_unused_until_phi_redefinition(
                        function,
                        *definition_block,
                        *definition_index,
                        block.id,
                        phi.out,
                    ) {
                        changed |= pairs.insert((phi.out, *candidate));
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    let snapshot = pairs.clone();
    for block in &function.blocks {
        for phi in &block.phis {
            for (predecessor, incoming) in &phi.incoming {
                if block_dominates(function, block.id, *predecessor)
                    || !snapshot.contains(&(phi.out, *incoming))
                        && !snapshot.contains(&(*incoming, phi.out))
                {
                    continue;
                }
                for candidate in named {
                    if values_are_connected(&snapshot, phi.out, *candidate) {
                        pairs.insert((*incoming, *candidate));
                    }
                }
            }
        }
    }
    pairs
}

fn value_type<'src>(function: &ControlFlowFunction<'src>, value: ValueId) -> Option<Type<'src>> {
    function
        .params
        .iter()
        .find(|parameter| parameter.value == value)
        .map(|parameter| parameter.ty.clone())
        .or_else(|| {
            function.blocks.iter().find_map(|block| {
                block
                    .phis
                    .iter()
                    .find(|phi| phi.out == value)
                    .map(|phi| phi.ty.clone())
                    .or_else(|| {
                        block
                            .instructions
                            .iter()
                            .find(|instruction| instruction.out == Some(value))
                            .and_then(|instruction| instruction.ty.clone())
                    })
            })
        })
}

fn block_dominates(
    function: &ControlFlowFunction<'_>,
    dominator: BlockId,
    target: BlockId,
) -> bool {
    if dominator == target {
        return true;
    }
    let mut pending = vec![function.entry];
    let mut visited = AHashSet::new();
    while let Some(block) = pending.pop() {
        if block == dominator || !visited.insert(block) {
            continue;
        }
        if block == target {
            return false;
        }
        pending.extend(block_successors(&function.blocks[block.0 as usize]));
    }
    true
}

fn block_is_on_cycle(function: &ControlFlowFunction<'_>, start: BlockId) -> bool {
    let mut pending = block_successors(&function.blocks[start.0 as usize]);
    let mut visited = AHashSet::new();
    while let Some(block) = pending.pop() {
        if block == start {
            return true;
        }
        if visited.insert(block) {
            pending.extend(block_successors(&function.blocks[block.0 as usize]));
        }
    }
    false
}

fn loop_body_reaches_exit(
    function: &ControlFlowFunction<'_>,
    body: BlockId,
    header: BlockId,
    exit: BlockId,
) -> bool {
    let mut pending = vec![body];
    let mut visited = AHashSet::new();
    while let Some(block_id) = pending.pop() {
        if block_id == header || !visited.insert(block_id) {
            continue;
        }
        for successor in block_successors(&function.blocks[block_id.0 as usize]) {
            if successor == exit {
                return true;
            }
            if successor != header {
                pending.push(successor);
            }
        }
    }
    false
}

fn positive_counter_condition(
    function: &ControlFlowFunction<'_>,
    condition: ValueId,
) -> Option<ValueId> {
    let instruction = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find(|instruction| instruction.out == Some(condition))?;
    let ControlFlowOp::Binary { op, lhs, rhs } = instruction.op else {
        return None;
    };
    let counter = match op {
        IrBinaryOp::Greater if is_int_constant(function, rhs, 0) => lhs,
        IrBinaryOp::Less if is_int_constant(function, lhs, 0) => rhs,
        _ => return None,
    };
    (value_type(function, counter).as_ref() == Some(&Type::Int)).then_some(counter)
}

fn rewrite_guarded_decrement_loop(
    out: &mut String,
    loop_body_open: Option<usize>,
    counter: &str,
) -> bool {
    let Some(open) = loop_body_open else {
        return false;
    };
    let body_start = open + usize::from(out.as_bytes().get(open) == Some(&b'{'));
    let prefix = format!("--{counter};");
    let postfix = format!("{counter}--;");
    let decrement_len = if out[body_start..].starts_with(&prefix) {
        prefix.len()
    } else if out[body_start..].starts_with(&postfix) {
        postfix.len()
    } else {
        return false;
    };
    let guarded = format!("{counter}>0");
    let Some(relative_condition) = out[..open].rfind(&guarded) else {
        return false;
    };
    let condition_end = relative_condition + guarded.len();
    if !out[condition_end..open]
        .bytes()
        .all(|byte| matches!(byte, b')' | b';'))
    {
        return false;
    }
    out.replace_range(body_start..body_start + decrement_len, "");
    out.replace_range(relative_condition..condition_end, &format!("{counter}--"));
    true
}

fn values_are_connected(
    pairs: &AHashSet<(ValueId, ValueId)>,
    start: ValueId,
    target: ValueId,
) -> bool {
    let mut pending = vec![start];
    let mut visited = AHashSet::new();
    while let Some(value) = pending.pop() {
        if value == target {
            return true;
        }
        if !visited.insert(value) {
            continue;
        }
        for (left, right) in pairs {
            if *left == value {
                pending.push(*right);
            } else if *right == value {
                pending.push(*left);
            }
        }
    }
    false
}

fn value_is_unused_until_block(
    function: &ControlFlowFunction<'_>,
    start: BlockId,
    stop: BlockId,
    value: ValueId,
    ignored_phi: ValueId,
) -> bool {
    let start_block = &function.blocks[start.0 as usize];
    if start_block.phis.iter().any(|phi| {
        phi.out != ignored_phi && phi.incoming.iter().any(|(_, incoming)| *incoming == value)
    }) || start_block
        .instructions
        .iter()
        .any(|instruction| op_values(&instruction.op).contains(&value))
        || start_block
            .terminator
            .as_ref()
            .is_some_and(|terminator| terminator_values(terminator).contains(&value))
    {
        return false;
    }
    let mut pending = block_successors(start_block)
        .into_iter()
        .map(|successor| (start, successor))
        .collect::<Vec<_>>();
    let mut visited = AHashSet::new();
    while let Some((predecessor, block_id)) = pending.pop() {
        if block_id == stop {
            continue;
        }
        let block = &function.blocks[block_id.0 as usize];
        if phi_edge_uses(block, predecessor, value) {
            return false;
        }
        if !visited.insert(block_id) {
            continue;
        }
        if block
            .instructions
            .iter()
            .any(|instruction| op_values(&instruction.op).contains(&value))
            || block
                .terminator
                .as_ref()
                .is_some_and(|terminator| terminator_values(terminator).contains(&value))
        {
            return false;
        }
        pending.extend(
            block_successors(block)
                .into_iter()
                .map(|successor| (block_id, successor)),
        );
    }
    true
}

fn phi_edge_uses(
    block: &crate::ir::ControlFlowBlock<'_>,
    predecessor: BlockId,
    value: ValueId,
) -> bool {
    block.phis.iter().any(|phi| {
        phi.incoming
            .iter()
            .any(|(incoming_block, incoming)| *incoming_block == predecessor && *incoming == value)
    })
}

fn target_is_unused_until_phi_redefinition(
    function: &ControlFlowFunction<'_>,
    definition_block: BlockId,
    definition_index: usize,
    phi_block: BlockId,
    target: ValueId,
) -> bool {
    let definition = &function.blocks[definition_block.0 as usize];
    if definition.instructions[definition_index + 1..]
        .iter()
        .any(|instruction| op_values(&instruction.op).contains(&target))
        || definition
            .terminator
            .as_ref()
            .is_some_and(|terminator| terminator_values(terminator).contains(&target))
    {
        return false;
    }

    let mut pending = block_successors(definition)
        .into_iter()
        .map(|successor| (definition_block, successor))
        .collect::<Vec<_>>();
    let mut visited = AHashSet::new();
    while let Some((predecessor, block_id)) = pending.pop() {
        if block_id == phi_block {
            let block = &function.blocks[block_id.0 as usize];
            if block.phis.iter().any(|phi| {
                phi.out != target
                    && phi.incoming.iter().any(|(incoming_block, incoming)| {
                        *incoming_block == predecessor && *incoming == target
                    })
            }) {
                return false;
            }
            continue;
        }
        let block = &function.blocks[block_id.0 as usize];
        if phi_edge_uses(block, predecessor, target) {
            return false;
        }
        if !visited.insert(block_id) {
            continue;
        }
        if block
            .instructions
            .iter()
            .any(|instruction| op_values(&instruction.op).contains(&target))
            || block
                .terminator
                .as_ref()
                .is_some_and(|terminator| terminator_values(terminator).contains(&target))
        {
            return false;
        }
        pending.extend(
            block_successors(block)
                .into_iter()
                .map(|successor| (block_id, successor)),
        );
    }
    true
}

fn color_phi_affinity_groups(
    values: &[ValueId],
    interference: &AHashMap<ValueId, AHashSet<ValueId>>,
    affinities: &AHashMap<ValueId, Vec<ValueId>>,
    parameter_order: &AHashMap<ValueId, usize>,
    uses: &AHashMap<ValueId, usize>,
) -> AHashMap<ValueId, usize> {
    let mut groups = values
        .iter()
        .enumerate()
        .map(|(index, value)| (index, vec![*value]))
        .collect::<AHashMap<_, _>>();
    let mut group_of = values
        .iter()
        .enumerate()
        .map(|(index, value)| (*value, index))
        .collect::<AHashMap<_, _>>();
    let mut edges = affinities
        .iter()
        .flat_map(|(left, rights)| {
            rights.iter().map(|right| {
                if left.0 < right.0 {
                    (*left, *right)
                } else {
                    (*right, *left)
                }
            })
        })
        .collect::<Vec<_>>();
    edges.sort_unstable_by_key(|(left, right)| (left.0, right.0));
    edges.dedup();
    for (left, right) in edges {
        let left_group = group_of[&left];
        let right_group = group_of[&right];
        if left_group == right_group {
            continue;
        }
        let left_members = &groups[&left_group];
        let right_members = &groups[&right_group];
        if left_members.iter().any(|value| {
            right_members
                .iter()
                .any(|other| interference[value].contains(other))
        }) {
            continue;
        }
        let retained = left_group.min(right_group);
        let removed = left_group.max(right_group);
        let mut merged = groups.remove(&retained).expect("affinity group exists");
        merged.extend(groups.remove(&removed).expect("affinity group exists"));
        merged.sort_unstable_by_key(|value| value.0);
        for value in &merged {
            group_of.insert(*value, retained);
        }
        groups.insert(retained, merged);
    }

    let mut group_ids = groups.keys().copied().collect::<Vec<_>>();
    group_ids.sort_unstable_by(|left, right| {
        let left_members = &groups[left];
        let right_members = &groups[right];
        let left_parameter = left_members
            .iter()
            .filter_map(|value| parameter_order.get(value))
            .min()
            .copied();
        let right_parameter = right_members
            .iter()
            .filter_map(|value| parameter_order.get(value))
            .min()
            .copied();
        right_parameter
            .is_some()
            .cmp(&left_parameter.is_some())
            .then_with(|| {
                left_parameter
                    .unwrap_or(usize::MAX)
                    .cmp(&right_parameter.unwrap_or(usize::MAX))
            })
            .then_with(|| {
                right_members
                    .iter()
                    .map(|value| uses.get(value).copied().unwrap_or(0))
                    .sum::<usize>()
                    .cmp(
                        &left_members
                            .iter()
                            .map(|value| uses.get(value).copied().unwrap_or(0))
                            .sum::<usize>(),
                    )
            })
            .then_with(|| left_members[0].0.cmp(&right_members[0].0))
    });
    let mut group_colors = AHashMap::<usize, usize>::new();
    let mut colors = AHashMap::<ValueId, usize>::new();
    for group in group_ids {
        let unavailable = groups[&group]
            .iter()
            .flat_map(|value| &interference[value])
            .filter_map(|neighbor| group_colors.get(&group_of[neighbor]).copied())
            .collect::<AHashSet<_>>();
        let color = (0..)
            .find(|color| !unavailable.contains(color))
            .expect("an interference graph always has another color");
        group_colors.insert(group, color);
        for value in &groups[&group] {
            colors.insert(*value, color);
        }
    }
    colors
}

fn collect_deferred_named_values(
    value: ValueId,
    named: &AHashSet<ValueId>,
    definitions: &AHashMap<ValueId, &ControlFlowOp<'_>>,
    deferred_definitions: &AHashSet<ValueId>,
    visited: &mut AHashSet<ValueId>,
    values: &mut AHashSet<ValueId>,
) {
    if named.contains(&value) {
        values.insert(value);
        return;
    }
    if !visited.insert(value) || !deferred_definitions.contains(&value) {
        return;
    }
    let Some(op) = definitions.get(&value) else {
        return;
    };
    for operand in op_values(op) {
        collect_deferred_named_values(
            operand,
            named,
            definitions,
            deferred_definitions,
            visited,
            values,
        );
    }
}

fn block_successors(block: &crate::ir::ControlFlowBlock<'_>) -> Vec<BlockId> {
    match block.terminator {
        Some(Terminator::Jump(target)) => vec![target],
        Some(Terminator::Branch {
            then_block,
            else_block,
            ..
        }) => vec![then_block, else_block],
        _ => Vec::new(),
    }
}

fn terminator_values(terminator: &Terminator) -> Vec<ValueId> {
    match terminator {
        Terminator::Branch { condition, .. } => vec![*condition],
        Terminator::Return(Some(value)) => vec![*value],
        _ => Vec::new(),
    }
}

fn take_value(
    value: ValueId,
    context: &LocalNames,
    cache: &mut ExpressionCache,
) -> Result<JsExpression, CodegenError> {
    if let Some(expression) = context
        .inlined_values
        .get(&value)
        .cloned()
        .or_else(|| cache.remove(&value))
    {
        return Ok(expression);
    }
    Ok(JsExpression::atom(context.value_name(value)?))
}

fn use_counts(function: &ControlFlowFunction<'_>) -> AHashMap<ValueId, usize> {
    let mut counts = AHashMap::new();
    let mut add = |value| *counts.entry(value).or_insert(0) += 1;
    for block in &function.blocks {
        for phi in &block.phis {
            for (_, value) in &phi.incoming {
                add(*value);
            }
        }
        for instruction in &block.instructions {
            for value in op_values(&instruction.op) {
                add(value);
            }
        }
        match block.terminator.as_ref() {
            Some(Terminator::Branch { condition, .. }) => add(*condition),
            Some(Terminator::Return(Some(value))) => add(*value),
            _ => {}
        }
    }
    counts
}

fn map_get_normalization_elisions(function: &ControlFlowFunction<'_>) -> AHashSet<ValueId> {
    let null_values = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match (&instruction.op, instruction.out) {
            (ControlFlowOp::Const(ConstValue::Null), Some(out)) => Some(out),
            _ => None,
        })
        .collect::<AHashSet<_>>();
    let candidates = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match (&instruction.op, instruction.out) {
            (
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::MapGet,
                    ..
                },
                Some(out),
            ) => Some(out),
            _ => None,
        })
        .collect::<Vec<_>>();

    candidates
        .into_iter()
        .filter(|candidate| {
            for block in &function.blocks {
                if block
                    .phis
                    .iter()
                    .any(|phi| phi.incoming.iter().any(|(_, value)| value == candidate))
                {
                    return false;
                }
                for instruction in &block.instructions {
                    if !op_values(&instruction.op).contains(candidate) {
                        continue;
                    }
                    let safe = match &instruction.op {
                        ControlFlowOp::Binary {
                            op: IrBinaryOp::Eq | IrBinaryOp::NotEq,
                            lhs,
                            rhs,
                        } => {
                            (*lhs == *candidate && null_values.contains(rhs))
                                || (*rhs == *candidate && null_values.contains(lhs))
                        }
                        ControlFlowOp::Intrinsic {
                            intrinsic: Intrinsic::UnwrapNullable,
                            receiver: Some(receiver),
                            ..
                        } => receiver == candidate,
                        _ => false,
                    };
                    if !safe {
                        return false;
                    }
                }
                if block
                    .terminator
                    .as_ref()
                    .is_some_and(|terminator| terminator_values(terminator).contains(candidate))
                {
                    return false;
                }
            }
            true
        })
        .collect()
}

fn safe_int_array_reads(
    function: &ControlFlowFunction<'_>,
    integer_facts: &FunctionIntegerFacts,
) -> AHashSet<ValueId> {
    let fixed_typed_lengths = crate::optimizer::fixed_typed_array_lengths(function);
    let definitions = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| instruction.out.map(|out| (out, &instruction.op)))
        .collect::<AHashMap<_, _>>();
    let int_constant = |value: ValueId| match definitions.get(&value) {
        Some(ControlFlowOp::Const(ConstValue::Int(value))) => Some(*value),
        _ => None,
    };
    let int_array = |value: ValueId| {
        value_type(function, value).is_some_and(|ty| {
            matches!(&ty, Type::Array(element) if **element == Type::Int)
                || matches!(
                    TypedArrayKind::from_type(&ty),
                    Some(
                        TypedArrayKind::Int8
                            | TypedArrayKind::Uint8
                            | TypedArrayKind::Uint8Clamped
                            | TypedArrayKind::Int16
                            | TypedArrayKind::Uint16
                            | TypedArrayKind::Int32
                    )
                )
        })
    };
    let index_offset = |value: ValueId, index: ValueId| {
        if value == index {
            return Some(0_i64);
        }
        let Some(ControlFlowOp::Binary { op, lhs, rhs }) = definitions.get(&value) else {
            return None;
        };
        match op {
            IrBinaryOp::Add if *lhs == index => int_constant(*rhs),
            IrBinaryOp::Add if *rhs == index => int_constant(*lhs),
            IrBinaryOp::Sub if *lhs == index => int_constant(*rhs).map(|value| -value),
            _ => None,
        }
    };
    let array_bound = |value: ValueId| {
        let mut length_value = value;
        let mut margin = 0_i64;
        if let Some(ControlFlowOp::Binary {
            op: IrBinaryOp::Sub,
            lhs,
            rhs,
        }) = definitions.get(&value)
        {
            margin = int_constant(*rhs)?;
            length_value = *lhs;
        }
        let Some(ControlFlowOp::Intrinsic {
            intrinsic,
            receiver: Some(array),
            ..
        }) = definitions.get(&length_value)
        else {
            return None;
        };
        let is_index_length = *intrinsic == Intrinsic::ArrayLength
            || matches!(
                classify_typed_array_intrinsic(*intrinsic),
                Some((_, TypedArrayIntrinsic::Length))
            );
        if !is_index_length {
            return None;
        }
        Some((*array, margin))
    };

    let mut safe = AHashSet::new();
    for shape in &function.shapes {
        let ControlShape::Loop {
            header,
            body,
            update,
            exit,
        } = shape
        else {
            continue;
        };
        let Some(Terminator::Branch { condition, .. }) =
            function.blocks[header.0 as usize].terminator
        else {
            continue;
        };
        let Some(ControlFlowOp::Binary {
            op: IrBinaryOp::Less,
            lhs: index,
            rhs: bound,
        }) = definitions.get(&condition)
        else {
            continue;
        };
        let nonnegative_induction = function.blocks[header.0 as usize]
            .phis
            .iter()
            .find(|phi| phi.out == *index)
            .is_some_and(|phi| {
                phi.incoming.iter().all(|(_, incoming)| {
                    int_constant(*incoming).is_some_and(|value| value >= 0)
                        || matches!(definitions.get(incoming), Some(ControlFlowOp::Binary {
                            op: IrBinaryOp::Add,
                            lhs,
                            rhs,
                        }) if (*lhs == *index && int_constant(*rhs).is_some_and(|value| value >= 0))
                            || (*rhs == *index && int_constant(*lhs).is_some_and(|value| value >= 0)))
                })
            });
        if integer_facts
            .range(*index)
            .is_none_or(|range| range.min < 0)
            && !nonnegative_induction
        {
            continue;
        }
        if let Some(bound) = int_constant(*bound).filter(|bound| *bound >= 0) {
            let mut work = vec![*body];
            let mut visited = AHashSet::new();
            while let Some(block_id) = work.pop() {
                if block_id == *header || block_id == *exit || Some(block_id) == *update {
                    continue;
                }
                if !visited.insert(block_id) {
                    continue;
                }
                let block = &function.blocks[block_id.0 as usize];
                for instruction in &block.instructions {
                    let (
                        Some(out),
                        ControlFlowOp::IndexGet {
                            object,
                            index: read_index,
                        },
                    ) = (instruction.out, &instruction.op)
                    else {
                        continue;
                    };
                    let Some(offset) =
                        index_offset(*read_index, *index).filter(|offset| *offset >= 0)
                    else {
                        continue;
                    };
                    if fixed_typed_lengths.get(object).is_some_and(|length| {
                        bound
                            .checked_add(offset)
                            .is_some_and(|exclusive_end| exclusive_end <= *length as i64)
                    }) {
                        safe.insert(out);
                    }
                }
                match block.terminator {
                    Some(Terminator::Jump(target)) => work.push(target),
                    Some(Terminator::Branch {
                        then_block,
                        else_block,
                        ..
                    }) => work.extend([then_block, else_block]),
                    _ => {}
                }
            }
            continue;
        }
        let Some((array, margin)) = array_bound(*bound) else {
            continue;
        };
        if margin < 0
            || !int_array(array)
            || function.value_escapes.get(array.0 as usize)
                == Some(&EscapeState::EscapesToUntypedBoundary)
        {
            continue;
        }

        let mut work = vec![*body];
        let mut visited = AHashSet::new();
        while let Some(block_id) = work.pop() {
            if block_id == *header || block_id == *exit || Some(block_id) == *update {
                continue;
            }
            if !visited.insert(block_id) {
                continue;
            }
            let block = &function.blocks[block_id.0 as usize];
            for instruction in &block.instructions {
                let (
                    Some(out),
                    ControlFlowOp::IndexGet {
                        object,
                        index: read_index,
                    },
                ) = (instruction.out, &instruction.op)
                else {
                    continue;
                };
                if *object == array
                    && index_offset(*read_index, *index)
                        .is_some_and(|offset| offset >= 0 && offset <= margin)
                {
                    safe.insert(out);
                }
            }
            match block.terminator {
                Some(Terminator::Jump(target)) => work.push(target),
                Some(Terminator::Branch {
                    then_block,
                    else_block,
                    ..
                }) => work.extend([then_block, else_block]),
                _ => {}
            }
        }
    }
    safe
}

fn can_fuse_value(
    block: &crate::ir::ControlFlowBlock<'_>,
    definition_index: usize,
    value: ValueId,
) -> bool {
    // JavaScript represents narrowed nullable and union values with the same
    // runtime value. Keeping the identity unwrap deferred is safe even when an
    // effectful operation sits between the narrowing and its sole use; the
    // deferred-operand interference edges keep the receiver's binding live.
    if matches!(
        block.instructions[definition_index].op,
        ControlFlowOp::Intrinsic {
            intrinsic: Intrinsic::UnwrapNullable | Intrinsic::UnwrapUnion,
            ..
        }
    ) {
        return true;
    }
    if can_fuse_global_host_receiver(block, definition_index, value) {
        return true;
    }
    for instruction in &block.instructions[definition_index + 1..] {
        if op_values(&instruction.op).contains(&value) {
            return true;
        }
        // A mutable read may move across other typed reads and pure expression
        // construction as long as its first consumer remains before any call
        // or write. `op_can_defer` is intentionally stricter because it also
        // governs open-ended expression caching; using it here needlessly
        // materialized left-to-right field reads such as `a.x*b.x-c.x`.
        if !expression_only_op(&instruction.op)
            || matches!(instruction.op, ControlFlowOp::HostFieldGet { .. })
        {
            return false;
        }
    }
    block
        .terminator
        .as_ref()
        .is_some_and(|terminator| terminator_values(terminator).contains(&value))
}

fn can_fuse_global_host_receiver(
    block: &crate::ir::ControlFlowBlock<'_>,
    definition_index: usize,
    value: ValueId,
) -> bool {
    if !matches!(
        block.instructions[definition_index].op,
        ControlFlowOp::LoadGlobal(_)
    ) {
        return false;
    }
    let Some((consumer_offset, args)) = block.instructions[definition_index + 1..]
        .iter()
        .enumerate()
        .find_map(|(offset, instruction)| match &instruction.op {
            ControlFlowOp::HostCall { receiver, args, .. } if *receiver == value => {
                Some((offset, args))
            }
            _ => None,
        })
    else {
        return false;
    };
    let consumer_index = definition_index + 1 + consumer_offset;
    let intermediates = &block.instructions[definition_index + 1..consumer_index];
    intermediates.iter().all(|instruction| {
        let Some(output) = instruction.out else {
            return false;
        };
        expression_only_op(&instruction.op)
            && args.contains(&output)
            && block
                .instructions
                .iter()
                .flat_map(|candidate| op_values(&candidate.op))
                .chain(
                    block
                        .terminator
                        .as_ref()
                        .into_iter()
                        .flat_map(terminator_values),
                )
                .filter(|used| *used == output)
                .count()
                == 1
    })
}

fn cross_block_values(function: &ControlFlowFunction<'_>) -> AHashSet<ValueId> {
    let mut definitions = AHashMap::new();
    for block in &function.blocks {
        for phi in &block.phis {
            definitions.insert(phi.out, block.id);
        }
        for instruction in &block.instructions {
            if let Some(value) = instruction.out {
                definitions.insert(value, block.id);
            }
        }
    }

    let mut crossing = AHashSet::new();
    let mut record_use = |value: ValueId, block: BlockId| {
        if definitions
            .get(&value)
            .is_some_and(|definition| *definition != block)
        {
            crossing.insert(value);
        }
    };
    for block in &function.blocks {
        for phi in &block.phis {
            for (incoming, value) in &phi.incoming {
                record_use(*value, *incoming);
            }
        }
        for instruction in &block.instructions {
            for value in op_values(&instruction.op) {
                record_use(value, block.id);
            }
        }
        match block.terminator.as_ref() {
            Some(Terminator::Branch { condition, .. }) => record_use(*condition, block.id),
            Some(Terminator::Return(Some(value))) => record_use(*value, block.id),
            _ => {}
        }
    }
    crossing
}

fn op_values(op: &ControlFlowOp<'_>) -> Vec<ValueId> {
    match op {
        ControlFlowOp::Const(_)
        | ControlFlowOp::LoadLocal(_)
        | ControlFlowOp::LoadGlobal(_)
        | ControlFlowOp::DynamicImport { .. } => Vec::new(),
        ControlFlowOp::Unary { value, .. } | ControlFlowOp::TypeCheck { value, .. } => vec![*value],
        ControlFlowOp::Binary { lhs, rhs, .. } => vec![*lhs, *rhs],
        ControlFlowOp::Array(values) | ControlFlowOp::Struct { fields: values, .. } => {
            values.clone()
        }
        ControlFlowOp::NewClass { args, .. } => args.clone(),
        ControlFlowOp::Closure { captures, .. } => captures.clone(),
        ControlFlowOp::StoreLocal { value, .. } | ControlFlowOp::StoreGlobal { value, .. } => {
            vec![*value]
        }
        ControlFlowOp::FieldGet { object, .. } => vec![*object],
        ControlFlowOp::FieldSet { object, value, .. } => vec![*object, *value],
        ControlFlowOp::HostFieldGet { object, .. } => vec![*object],
        ControlFlowOp::HostFieldSet { object, value, .. } => vec![*object, *value],
        ControlFlowOp::IndexGet { object, index } => vec![*object, *index],
        ControlFlowOp::IndexSet {
            object,
            index,
            value,
        } => vec![*object, *index, *value],
        ControlFlowOp::CallDirect { args, .. } => args.clone(),
        ControlFlowOp::CallValue { callee, args } => {
            let mut values = vec![*callee];
            values.extend(args);
            values
        }
        ControlFlowOp::CallMethod { receiver, args, .. } => {
            let mut values = vec![*receiver];
            values.extend(args);
            values
        }
        ControlFlowOp::HostCall { receiver, args, .. } => {
            let mut values = vec![*receiver];
            values.extend(args);
            values
        }
        ControlFlowOp::Intrinsic { receiver, args, .. } => {
            let mut values = receiver.iter().copied().collect::<Vec<_>>();
            values.extend(args);
            values
        }
        ControlFlowOp::Template(parts) => parts
            .iter()
            .filter_map(|part| match part {
                TemplateOperand::Value(value) => Some(*value),
                TemplateOperand::String(_) => None,
            })
            .collect(),
    }
}

fn op_can_defer(op: &ControlFlowOp<'_>) -> bool {
    matches!(
        op,
        ControlFlowOp::Const(_)
            | ControlFlowOp::Unary { .. }
            | ControlFlowOp::Binary { .. }
            | ControlFlowOp::TypeCheck { .. }
            | ControlFlowOp::Array(_)
            | ControlFlowOp::Struct { .. }
            | ControlFlowOp::Closure { .. }
            | ControlFlowOp::Template(_)
            | ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::UnwrapNullable
                    | Intrinsic::UnwrapUnion
                    | Intrinsic::StringLength
                    | Intrinsic::StringCharAt
                    | Intrinsic::StringCharCodeAt
                    | Intrinsic::IntImul
                    | Intrinsic::IntToString
                    | Intrinsic::IntToUnsignedString,
                ..
            }
    )
}

fn expression_only_op(op: &ControlFlowOp<'_>) -> bool {
    !matches!(
        op,
        ControlFlowOp::StoreLocal { .. }
            | ControlFlowOp::StoreGlobal { .. }
            | ControlFlowOp::FieldSet { .. }
            | ControlFlowOp::HostFieldSet { .. }
            | ControlFlowOp::IndexSet { .. }
            | ControlFlowOp::NewClass {
                constructor: Some(_),
                ..
            }
            | ControlFlowOp::CallDirect { .. }
            | ControlFlowOp::CallValue { .. }
            | ControlFlowOp::CallMethod { .. }
            | ControlFlowOp::HostCall { .. }
            | ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::Print
                    | Intrinsic::ArrayMap
                    | Intrinsic::ArrayFilter
                    | Intrinsic::ArrayReduce
                    | Intrinsic::ArrayForEach
                    | Intrinsic::ArrayPush
                    | Intrinsic::ArrayPop
                    | Intrinsic::ArrayIndexOf
                    | Intrinsic::ArraySplice
                    | Intrinsic::MapSet
                    | Intrinsic::MapDelete
                    | Intrinsic::MapClear
                    | Intrinsic::SetAdd
                    | Intrinsic::SetDelete
                    | Intrinsic::SetClear,
                ..
            }
    )
}

fn op_has_side_effects(op: &ControlFlowOp<'_>) -> bool {
    matches!(
        op,
        ControlFlowOp::StoreLocal { .. }
            | ControlFlowOp::StoreGlobal { .. }
            | ControlFlowOp::FieldSet { .. }
            | ControlFlowOp::HostFieldGet { .. }
            | ControlFlowOp::HostFieldSet { .. }
            | ControlFlowOp::IndexSet { .. }
            | ControlFlowOp::CallDirect { .. }
            | ControlFlowOp::CallValue { .. }
            | ControlFlowOp::CallMethod { .. }
            | ControlFlowOp::HostCall { pure: false, .. }
            | ControlFlowOp::NewClass { .. }
            | ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::Print
                    | Intrinsic::ArrayMap
                    | Intrinsic::ArrayFilter
                    | Intrinsic::ArrayReduce
                    | Intrinsic::ArrayForEach
                    | Intrinsic::ArrayPush
                    | Intrinsic::ArrayPop
                    | Intrinsic::ArrayIndexOf
                    | Intrinsic::ArraySplice
                    | Intrinsic::MapSet
                    | Intrinsic::MapDelete
                    | Intrinsic::MapClear
                    | Intrinsic::SetAdd
                    | Intrinsic::SetDelete
                    | Intrinsic::SetClear,
                ..
            }
    )
}

fn render_const(value: &ConstValue, compact_boolean_literals: bool, quote: StringQuote) -> String {
    match value {
        ConstValue::Int(value) => shortest_integer(*value),
        ConstValue::Float(value) => shortest_float(*value),
        ConstValue::Bool(true) if compact_boolean_literals => "!0".to_string(),
        ConstValue::Bool(false) if compact_boolean_literals => "!1".to_string(),
        ConstValue::Bool(value) => value.to_string(),
        ConstValue::String(value) => render_string_literal(value, quote),
        ConstValue::Null => "null".to_string(),
    }
}

fn shortest_integer(value: i64) -> String {
    let decimal = value.to_string();
    if value == 0 {
        return decimal;
    }
    let negative = value < 0;
    let digits = decimal.trim_start_matches('-');
    let zeros = digits
        .bytes()
        .rev()
        .take_while(|byte| *byte == b'0')
        .count();
    if zeros == 0 {
        return decimal;
    }
    let exponent = format!(
        "{}{}e{zeros}",
        if negative { "-" } else { "" },
        &digits[..digits.len() - zeros]
    );
    if exponent.len() < decimal.len() {
        exponent
    } else {
        decimal
    }
}

fn shortest_float(value: f64) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    if value == 0.0 {
        return if value.is_sign_negative() { "-0" } else { "0" }.to_string();
    }
    let mut candidates = Vec::with_capacity(3);
    if value.fract() == 0.0 && value >= i64::MIN as f64 && value <= i64::MAX as f64 {
        candidates.push(shortest_integer(value as i64));
    }
    let decimal = value.to_string();
    candidates.push(trim_leading_zero(decimal));
    let scientific = normalize_exponent(format!("{value:e}"));
    candidates.push(trim_leading_zero(scientific));
    candidates
        .into_iter()
        .filter(|candidate| candidate.parse::<f64>().ok() == Some(value))
        .min_by_key(String::len)
        .unwrap_or_else(|| value.to_string())
}

fn trim_leading_zero(value: String) -> String {
    if let Some(rest) = value.strip_prefix("0.") {
        format!(".{rest}")
    } else if let Some(rest) = value.strip_prefix("-0.") {
        format!("-.{rest}")
    } else {
        value
    }
}

fn normalize_exponent(value: String) -> String {
    let Some((mantissa, exponent)) = value.split_once('e') else {
        return value;
    };
    let exponent = exponent
        .strip_prefix('+')
        .unwrap_or(exponent)
        .trim_start_matches('0');
    let exponent = if exponent.is_empty() || exponent == "-" {
        "0"
    } else if let Some(rest) = exponent.strip_prefix("-0") {
        if rest.is_empty() {
            "0"
        } else {
            return format!("{mantissa}e-{rest}");
        }
    } else {
        exponent
    };
    format!("{mantissa}e{exponent}")
}

fn render_js_type_check(
    value: &str,
    target: &Type<'_>,
    quote: StringQuote,
) -> Result<String, CodegenError> {
    Ok(match target {
        Type::Int | Type::Float => {
            format!(
                "typeof({value})=={}",
                render_string_literal("number", quote)
            )
        }
        Type::String => {
            format!(
                "typeof({value})=={}",
                render_string_literal("string", quote)
            )
        }
        Type::Bool => {
            format!(
                "typeof({value})=={}",
                render_string_literal("boolean", quote)
            )
        }
        Type::Array(_) => format!("Array.isArray({value})"),
        Type::Function(_) | Type::GenericFunction(_) => {
            format!(
                "typeof({value})=={}",
                render_string_literal("function", quote)
            )
        }
        _ => {
            return Err(CodegenError::new(
                crate::span::Span::empty(0),
                format!("type `{target}` has no JavaScript type guard"),
            ));
        }
    })
}

fn render_string_literal(value: &str, quote: StringQuote) -> String {
    if quote == StringQuote::Double {
        return format!("\"{value}\"");
    }
    let encoded = format!("\"{value}\"");
    let decoded = serde_json::from_str::<String>(&encoded).unwrap_or_else(|_| value.to_string());
    let mut rendered = String::with_capacity(decoded.len() + 2);
    rendered.push('\'');
    for character in decoded.chars() {
        match character {
            '\'' => rendered.push_str("\\'"),
            '\\' => rendered.push_str("\\\\"),
            '\u{0008}' => rendered.push_str("\\b"),
            '\u{000c}' => rendered.push_str("\\f"),
            '\n' => rendered.push_str("\\n"),
            '\r' => rendered.push_str("\\r"),
            '\t' => rendered.push_str("\\t"),
            '\u{2028}' => rendered.push_str("\\u2028"),
            '\u{2029}' => rendered.push_str("\\u2029"),
            control if control <= '\u{001f}' => {
                write!(rendered, "\\u{:04x}", control as u32)
                    .expect("writing to a string cannot fail");
            }
            _ => rendered.push(character),
        }
    }
    rendered.push('\'');
    rendered
}

fn packed_string_array(
    values: &[ValueId],
    context: &LocalNames,
    quote: StringQuote,
) -> Option<String> {
    if values.is_empty() {
        return None;
    }
    let strings = values
        .iter()
        .map(|value| context.string_constants.get(value).map(String::as_str))
        .collect::<Option<Vec<_>>>()?;
    [",", " ", "|", ";", "~", ":"]
        .into_iter()
        .filter(|delimiter| strings.iter().all(|value| !value.contains(delimiter)))
        .map(|delimiter| {
            format!(
                "{}.split({})",
                render_string_literal(&strings.join(delimiter), quote),
                render_string_literal(delimiter, quote)
            )
        })
        .min_by(|left, right| (left.len(), left).cmp(&(right.len(), right)))
}

fn binary_operator(op: IrBinaryOp) -> &'static str {
    match op {
        IrBinaryOp::Add => "+",
        IrBinaryOp::Sub => "-",
        IrBinaryOp::Mul => "*",
        IrBinaryOp::Div => "/",
        IrBinaryOp::Mod => "%",
        IrBinaryOp::BitAnd => "&",
        IrBinaryOp::BitOr => "|",
        IrBinaryOp::Xor => "^",
        IrBinaryOp::ShiftLeft => "<<",
        IrBinaryOp::ShiftRight => ">>",
        IrBinaryOp::UnsignedShiftRight => ">>>",
        IrBinaryOp::Eq => "==",
        IrBinaryOp::NotEq => "!=",
        IrBinaryOp::Less => "<",
        IrBinaryOp::LessEq => "<=",
        IrBinaryOp::Greater => ">",
        IrBinaryOp::GreaterEq => ">=",
        IrBinaryOp::And => "&&",
        IrBinaryOp::Or => "||",
    }
}

fn token_safe_binary_rhs(op: IrBinaryOp, rhs: String) -> String {
    if op == IrBinaryOp::Sub && rhs.starts_with('-') {
        format!("({rhs})")
    } else {
        rhs
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BinaryOperandSide {
    Left,
    Right,
}

#[cfg(test)]
fn render_binary_operand(
    expression: String,
    child: Option<IrBinaryOp>,
    parent: IrBinaryOp,
    side: BinaryOperandSide,
) -> String {
    if expression.ends_with("|0)") {
        return expression;
    }
    let Some(child) = child else {
        return expression;
    };
    let child_precedence = binary_precedence(child);
    let parent_precedence = binary_precedence(parent);
    let can_unwrap = child_precedence > parent_precedence
        || (child_precedence == parent_precedence
            && match side {
                BinaryOperandSide::Left => true,
                BinaryOperandSide::Right => {
                    child == parent
                        && matches!(
                            parent,
                            IrBinaryOp::BitAnd
                                | IrBinaryOp::BitOr
                                | IrBinaryOp::Xor
                                | IrBinaryOp::And
                                | IrBinaryOp::Or
                        )
                }
            });
    if can_unwrap {
        strip_outer_parens(expression)
    } else {
        expression
    }
}

#[cfg(test)]
fn binary_precedence(op: IrBinaryOp) -> u8 {
    match op {
        IrBinaryOp::Or => 1,
        IrBinaryOp::And => 2,
        IrBinaryOp::BitOr => 3,
        IrBinaryOp::Xor => 4,
        IrBinaryOp::BitAnd => 5,
        IrBinaryOp::Eq | IrBinaryOp::NotEq => 6,
        IrBinaryOp::Less | IrBinaryOp::LessEq | IrBinaryOp::Greater | IrBinaryOp::GreaterEq => 7,
        IrBinaryOp::ShiftLeft | IrBinaryOp::ShiftRight | IrBinaryOp::UnsignedShiftRight => 8,
        IrBinaryOp::Add | IrBinaryOp::Sub => 9,
        IrBinaryOp::Mul | IrBinaryOp::Div | IrBinaryOp::Mod => 10,
    }
}

fn js_binary_precedence(op: IrBinaryOp) -> JsPrecedence {
    match op {
        IrBinaryOp::Or => JsPrecedence::LogicalOr,
        IrBinaryOp::And => JsPrecedence::LogicalAnd,
        IrBinaryOp::BitOr => JsPrecedence::BitOr,
        IrBinaryOp::Xor => JsPrecedence::BitXor,
        IrBinaryOp::BitAnd => JsPrecedence::BitAnd,
        IrBinaryOp::Eq | IrBinaryOp::NotEq => JsPrecedence::Equality,
        IrBinaryOp::Less | IrBinaryOp::LessEq | IrBinaryOp::Greater | IrBinaryOp::GreaterEq => {
            JsPrecedence::Relational
        }
        IrBinaryOp::ShiftLeft | IrBinaryOp::ShiftRight | IrBinaryOp::UnsignedShiftRight => {
            JsPrecedence::Shift
        }
        IrBinaryOp::Add | IrBinaryOp::Sub => JsPrecedence::Additive,
        IrBinaryOp::Mul | IrBinaryOp::Div | IrBinaryOp::Mod => JsPrecedence::Multiplicative,
    }
}

const fn inverse_comparison(op: IrBinaryOp) -> Option<IrBinaryOp> {
    match op {
        IrBinaryOp::Eq => Some(IrBinaryOp::NotEq),
        IrBinaryOp::NotEq => Some(IrBinaryOp::Eq),
        IrBinaryOp::Less => Some(IrBinaryOp::GreaterEq),
        IrBinaryOp::LessEq => Some(IrBinaryOp::Greater),
        IrBinaryOp::Greater => Some(IrBinaryOp::LessEq),
        IrBinaryOp::GreaterEq => Some(IrBinaryOp::Less),
        _ => None,
    }
}

fn default_value(ty: &Type<'_>, compact_boolean_literals: bool) -> &'static str {
    match ty {
        Type::Int | Type::Float => "0",
        Type::Bool if compact_boolean_literals => "!1",
        Type::Bool => "false",
        Type::String => "\"\"",
        Type::Array(_) => "[]",
        Type::Map(_, _) => "new Map",
        Type::Set(_) => "new Set",
        Type::ArrayBuffer => "new ArrayBuffer(0)",
        Type::SharedArrayBuffer => "new SharedArrayBuffer(0)",
        Type::Int8Array => "new Int8Array(0)",
        Type::Uint8Array => "new Uint8Array(0)",
        Type::Uint8ClampedArray => "new Uint8ClampedArray(0)",
        Type::Int16Array => "new Int16Array(0)",
        Type::Uint16Array => "new Uint16Array(0)",
        Type::Int32Array => "new Int32Array(0)",
        Type::Uint32Array => "new Uint32Array(0)",
        Type::Float32Array => "new Float32Array(0)",
        Type::Float64Array => "new Float64Array(0)",
        Type::Symbol => "Symbol()",
        Type::Union(members) => members.first().map_or("null", |member| {
            default_value(member, compact_boolean_literals)
        }),
        Type::Null | Type::Nullable(_) => "null",
        Type::Struct(_)
        | Type::Class(_)
        | Type::StructInstance { .. }
        | Type::ClassInstance { .. }
        | Type::TypeParameter(_)
        | Type::Function(_)
        | Type::GenericFunction(_) => "null",
        Type::Void | Type::Task(_) | Type::ModuleNamespace(_) | Type::ModuleLoadError => "void 0",
    }
}

fn is_nullable_with_truthy_value(ty: &Type<'_>) -> bool {
    let Type::Nullable(inner) = ty else {
        return false;
    };
    type_is_always_js_truthy(inner)
}

fn type_is_always_js_truthy(ty: &Type<'_>) -> bool {
    match ty {
        Type::Array(_)
        | Type::Map(_, _)
        | Type::Set(_)
        | Type::ArrayBuffer
        | Type::SharedArrayBuffer
        | Type::Int8Array
        | Type::Uint8Array
        | Type::Uint8ClampedArray
        | Type::Int16Array
        | Type::Uint16Array
        | Type::Int32Array
        | Type::Uint32Array
        | Type::Float32Array
        | Type::Float64Array
        | Type::Symbol
        | Type::Task(_)
        | Type::ModuleNamespace(_)
        | Type::ModuleLoadError
        | Type::Struct(_)
        | Type::Class(_)
        | Type::StructInstance { .. }
        | Type::ClassInstance { .. }
        | Type::Function(_)
        | Type::GenericFunction(_) => true,
        Type::Union(members) => members.iter().all(type_is_always_js_truthy),
        Type::Int
        | Type::Float
        | Type::String
        | Type::Bool
        | Type::Null
        | Type::Void
        | Type::Nullable(_)
        | Type::TypeParameter(_) => false,
    }
}

trait IntoMinimalExpression {
    fn into_minimal_expression(self) -> String;
}

impl IntoMinimalExpression for JsExpression {
    fn into_minimal_expression(self) -> String {
        self.into_minimal()
    }
}

impl IntoMinimalExpression for String {
    fn into_minimal_expression(self) -> String {
        strip_outer_parens_from_string(self)
    }
}

fn strip_outer_parens(value: impl IntoMinimalExpression) -> String {
    value.into_minimal_expression()
}

fn strip_outer_parens_from_string(value: String) -> String {
    if !value.starts_with('(') || !value.ends_with(')') {
        return value;
    }
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in value.char_indices() {
        if let Some(active) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == active {
                quote = None;
            }
            continue;
        }
        if matches!(character, '\'' | '"' | '`') {
            quote = Some(character);
            continue;
        }
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 && index + character.len_utf8() != value.len() {
                    return value;
                }
            }
            _ => {}
        }
    }
    value[1..value.len() - 1].to_string()
}

fn is_true_literal(value: &str) -> bool {
    matches!(value, "true" | "!0")
}

fn is_false_literal(value: &str) -> bool {
    matches!(value, "false" | "!1")
}

fn is_rendered_string_literal(value: &str) -> bool {
    value.len() >= 2
        && matches!(value.as_bytes()[0], b'\'' | b'"')
        && value.as_bytes().last() == Some(&value.as_bytes()[0])
}

fn is_single_binding_statement(statement: &str) -> bool {
    statement.starts_with("let ")
        && statement.ends_with(';')
        && !statement[..statement.len() - 1].contains(';')
}

#[derive(Debug, Clone)]
struct Mangler {
    next: usize,
    reserved: AHashSet<String>,
    alphabet: IdentifierAlphabet,
}

impl Default for Mangler {
    fn default() -> Self {
        Self::new(IdentifierAlphabet::canonical())
    }
}

impl Mangler {
    fn new(alphabet: IdentifierAlphabet) -> Self {
        Self {
            next: 0,
            reserved: AHashSet::new(),
            alphabet,
        }
    }

    fn reserve(&mut self, name: &str) {
        self.reserved.insert(name.to_string());
    }

    fn release(&mut self, name: &str) {
        self.reserved.remove(name);
    }

    fn claim_name(&mut self, name: &str) -> bool {
        !is_js_reserved(name) && self.reserved.insert(name.to_string())
    }

    fn rewind(&mut self) {
        self.next = 0;
    }

    fn next_name(&mut self) -> String {
        loop {
            let name = encode_identifier(self.next, &self.alphabet);
            self.next += 1;
            if !self.reserved.contains(&name) && !is_js_reserved(&name) {
                self.reserved.insert(name.clone());
                return name;
            }
        }
    }

    fn unique_name(&mut self, preferred: &str) -> String {
        let base = if is_js_reserved(preferred) {
            format!("${preferred}")
        } else {
            preferred.to_string()
        };
        if self.reserved.insert(base.clone()) {
            return base;
        }
        let mut suffix = 2;
        loop {
            let candidate = format!("{base}${suffix}");
            if self.reserved.insert(candidate.clone()) {
                return candidate;
            }
            suffix += 1;
        }
    }
}

fn is_js_reserved(name: &str) -> bool {
    matches!(
        name,
        "await"
            | "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "implements"
            | "import"
            | "in"
            | "instanceof"
            | "interface"
            | "let"
            | "new"
            | "null"
            | "package"
            | "private"
            | "protected"
            | "public"
            | "return"
            | "static"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
            | "yield"
    )
}

fn encode_identifier(mut index: usize, alphabet: &IdentifierAlphabet) -> String {
    let mut output = String::new();
    output.push(alphabet.first[index % alphabet.first.len()] as char);
    index /= alphabet.first.len();
    while index > 0 {
        index -= 1;
        output.push(alphabet.rest[index % alphabet.rest.len()] as char);
        index /= alphabet.rest.len();
    }
    output
}

#[cfg(test)]
mod tests {
    use bumpalo::Bump;

    use super::*;
    use crate::{
        analyze, lower_to_control_flow,
        optimizer::{
            optimize_control_flow, optimize_control_flow_for_module,
            optimize_control_flow_with_options, OptimizationOptions,
        },
        parse_source,
    };

    fn compile(source: &str) -> String {
        compile_with_options(source, IrJsOptions::default())
    }

    fn compile_with_options(source: &str, options: IrJsOptions) -> String {
        let arena = Bump::new();
        let program = parse_source(&arena, source).unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut ir).unwrap();
        emit_optimized_ir_js_with_options(&ir, &options).unwrap()
    }

    fn compile_module(source: &str) -> String {
        compile_module_with_options(source, IrJsOptions::default())
    }

    fn compile_module_with_options(source: &str, options: IrJsOptions) -> String {
        let arena = Bump::new();
        let program = parse_source(&arena, source).unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_for_module(&mut ir).unwrap();
        emit_optimized_ir_js_module_with_options(&ir, &options).unwrap()
    }

    fn compile_without_inlining(source: &str, scalar_replacement: bool) -> String {
        compile_without_inlining_with_options(source, scalar_replacement, IrJsOptions::default())
    }

    fn compile_without_inlining_with_options(
        source: &str,
        scalar_replacement: bool,
        js_options: IrJsOptions,
    ) -> String {
        let arena = Bump::new();
        let program = parse_source(&arena, source).unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            scalar_replacement,
            ..OptimizationOptions::default()
        };
        optimize_control_flow_with_options(&mut ir, &options, false).unwrap();
        emit_optimized_ir_js_with_options(&ir, &js_options).unwrap()
    }

    #[test]
    fn emits_compact_straight_line_ir() {
        assert_eq!(compile("print(1+2*3);"), "console.log(7)");
    }

    #[test]
    fn coalesces_nonoverlapping_names_in_a_single_block_function() {
        let output = compile_module(
            "export float pipeline(float value){float first=value+1.0;float second=first*first;float third=second+second;float fourth=third*third;return fourth+fourth;}",
        );

        assert_eq!(output.matches("let ").count(), 0, "{output}");
        assert!(!output.contains("let a="), "{output}");
        assert!(output.contains("a=a+1;a=a*a;a=a+a;a=a*a"), "{output}");
    }

    #[test]
    fn fuses_a_global_host_receiver_with_its_one_use_constructed_argument() {
        let output = compile_module(
            "extern class Crypto{Uint8Array getRandomValues(Uint8Array values);}extern Crypto crypto;export Uint8Array sample(int size){return crypto.getRandomValues(new Uint8Array(size));}",
        );

        assert!(
            output.contains("crypto.getRandomValues(new Uint8Array("),
            "{output}"
        );
        assert!(!output.contains("=crypto"), "{output}");
    }

    #[test]
    fn uses_boolean_coercion_for_null_checks_when_non_null_values_are_always_truthy() {
        let reference = compile_module(
            "export bool present(int[]? value){return value!=null;}export bool missing(Map<string,int>? value){return value==null;}",
        );
        assert!(reference.contains("!!"), "{reference}");
        assert!(reference.contains("return !"), "{reference}");

        let scalar = compile_module(
            "export bool present(string? value){return value!=null;}export bool missing(int? value){return value==null;}",
        );
        assert!(scalar.contains("!=null"), "{scalar}");
        assert!(scalar.contains("==null"), "{scalar}");
    }

    #[test]
    fn defers_identity_unwraps_across_effectful_array_lookups() {
        let output = compile_module(
            "export void remove(int[]? values,int needle){if(values!=null){values.splice(values.indexOf(needle)>>>0,1);}}",
        );

        assert!(output.contains(".splice("), "{output}");
        assert!(output.contains(".indexOf("), "{output}");
        assert!(!output.contains("{var "), "{output}");
    }

    #[test]
    fn clusters_similar_function_declarations_with_dynamic_programming() {
        let segments = vec![
            "function warmA(a){return a*a+a*17+a%11}".to_string(),
            "function coldA(a){return a^a>>>3^987654321}".to_string(),
            "function warmB(a){return a*a+a*19+a%13}".to_string(),
            "function coldB(a){return a^a>>>5^987654323}".to_string(),
        ];

        let order = compression_similarity_order(&segments, 13);

        let position = |function| order.iter().position(|item| *item == function).unwrap();
        assert_eq!(position(0).abs_diff(position(2)), 1, "{order:?}");
        assert_eq!(position(1).abs_diff(position(3)), 1, "{order:?}");
    }

    #[test]
    fn preserves_structured_shapes_for_nested_numeric_loops() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            include_str!("../benchmarks/libraries/ports/robust-predicates/util.lil"),
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_for_module(&mut ir).unwrap();
        let function = ir
            .functions
            .iter()
            .find(|function| function.name == Some("expansionSum"))
            .unwrap();
        assert!(can_structure(function));
        let output = emit_optimized_ir_js_module(&ir).unwrap();
        assert!(!output.contains("switch("), "{output}");
        assert!(output.contains("&&"), "{output}");
    }

    #[test]
    fn keeps_repeated_function_bodies_inside_the_codec_history_window() {
        let repeated_left = format!("function left(a){{return a+{}+a*a}}", "x".repeat(48));
        let unrelated = format!("function filler(a){{return a+{}}}", "q".repeat(160));
        let repeated_right = format!("function right(a){{return a+{}+a*a}}", "x".repeat(48));
        let segments = vec![
            repeated_left,
            unrelated,
            repeated_right,
            "function tail(a){return a^987654321}".to_string(),
        ];

        let order = compression_window_order(&segments, 128, 13);
        let position = |function| order.iter().position(|item| *item == function).unwrap();

        assert_eq!(position(0).abs_diff(position(2)), 1, "{order:?}");
        let similarities = compression_similarities(&segments);
        let lengths = segments.iter().map(String::len).collect::<Vec<_>>();
        assert!(
            window_path_score(&order, &similarities, &lengths, 128)
                > window_path_score(&[0, 1, 2, 3], &similarities, &lengths, 128)
        );
    }

    #[test]
    fn two_opt_refines_a_large_layout_without_exponential_search() {
        let similarities = vec![
            vec![0, 1, 10, 0],
            vec![1, 0, 1, 10],
            vec![10, 1, 0, 1],
            vec![0, 10, 1, 0],
        ];
        let source = vec![0, 1, 2, 3];
        let improved = improve_similarity_path(source.clone(), &similarities);

        assert_eq!(improved, vec![0, 2, 1, 3]);
        assert!(
            path_similarity(&improved, &similarities) > path_similarity(&source, &similarities)
        );
    }

    #[test]
    fn preserves_nested_short_circuit_grouping_after_name_coalescing() {
        let code = compile_module(
            "int depth=0;bool flushing=false;void set(int nextDepth,bool nextFlushing){depth=nextDepth;flushing=nextFlushing;}bool gated(bool user){return user&&(depth>0||flushing);}export{set,gated};",
        );

        assert!(code.contains("&&("), "{code}");
        assert!(!code.contains("&&depth>0||"), "{code}");
    }

    #[test]
    fn defers_one_use_short_circuit_phis_into_their_branch() {
        let code = compile_module(
            "export void report(int left,int right){if(left==0&&right==0){print(1);}print(2);}",
        );

        assert!(code.contains("&&"), "{code}");
        assert!(!code.contains("var "), "{code}");
        assert!(!code.contains(";if("), "{code}");
    }

    #[test]
    fn removes_only_precedence_safe_binary_parentheses() {
        assert_eq!(
            render_binary_operand(
                "(a-b)".to_string(),
                Some(IrBinaryOp::Sub),
                IrBinaryOp::Add,
                BinaryOperandSide::Left,
            ),
            "a-b"
        );
        assert_eq!(
            render_binary_operand(
                "(b*c)".to_string(),
                Some(IrBinaryOp::Mul),
                IrBinaryOp::Sub,
                BinaryOperandSide::Right,
            ),
            "b*c"
        );
        assert_eq!(
            render_binary_operand(
                "(b-c)".to_string(),
                Some(IrBinaryOp::Sub),
                IrBinaryOp::Add,
                BinaryOperandSide::Right,
            ),
            "(b-c)"
        );
        assert_eq!(
            render_binary_operand(
                "(a+b)".to_string(),
                Some(IrBinaryOp::Add),
                IrBinaryOp::Mul,
                BinaryOperandSide::Left,
            ),
            "(a+b)"
        );
        assert_eq!(
            render_binary_operand(
                "(b&&c)".to_string(),
                Some(IrBinaryOp::And),
                IrBinaryOp::And,
                BinaryOperandSide::Right,
            ),
            "b&&c"
        );
    }

    #[test]
    fn removes_outer_parentheses_inside_expression_delimiters() {
        let code = compile(
            "float sample(float[] values,int index,float input){values[index%3]=input-1.0;return values[index%3]+(input-1.0).abs();}print(sample([1.0,2.0,3.0],2,4.0));",
        );

        assert!(!code.contains("[(("), "{code}");
        assert!(!code.contains("Math.abs(("), "{code}");
    }

    #[test]
    fn coalesces_sequential_mutations_across_direct_phi_edges() {
        let source = "export int refine(int hash,int remaining,int byte){if(remaining==3){hash^=byte<<16;}if(remaining>=2){hash^=byte<<8;}if(remaining>=1){hash^=byte;hash=Math.imul(hash,31);}return hash;}";
        let code = compile_module(source);
        let direct = compile_module_with_options(
            source,
            IrJsOptions {
                phi_affinity_mode: PhiAffinityMode::Direct,
                ..IrJsOptions::default()
            },
        );
        let conservative = compile_module_with_options(
            source,
            IrJsOptions {
                phi_affinity_mode: PhiAffinityMode::Conservative,
                ..IrJsOptions::default()
            },
        );

        assert!(!code.contains("else{"), "{code}");
        assert_eq!(direct, conservative);
        assert!(code.len() <= direct.len(), "{code}\n{direct}");
        assert!(direct.contains("a=Math.imul(a^c,31)"), "{direct}");
    }

    #[test]
    fn folds_branches_over_literal_string_captures() {
        let code = compile(
            "func(float)->float choose(string direction){return (float value)=>{if(direction==\"end\"){return value+1.0;}return value-1.0;};}func(float)->float end=choose(\"end\");func(float)->float start=choose(\"start\");float[] values=[1.0,2.0];float total=0.0;for(int i=0;i<values.length;i++){total=total+end(values[i])+start(values[i]);}print(total);",
        );

        assert!(!code.contains("\"end\"==\"end\""), "{code}");
        assert!(!code.contains("\"start\"==\"end\""), "{code}");
    }

    #[test]
    fn packs_literal_string_arrays_when_the_raw_candidate_is_shorter() {
        let source = "extern void consume(string[] values);string[] values=[\"aaaaaa\",\"bbbbbb\",\"cccccc\",\"dddddd\",\"eeeeee\",\"ffffff\",\"gggggg\",\"hhhhhh\"];consume(values);";
        let packed = compile(source);
        let unpacked = compile_with_options(
            source,
            IrJsOptions {
                pack_string_arrays: false,
                ..IrJsOptions::default()
            },
        );

        assert!(packed.contains(".split("), "{packed}");
        assert!(!unpacked.contains(".split("), "{unpacked}");
        assert!(packed.len() < unpacked.len(), "{packed}\n{unpacked}");
    }

    #[test]
    fn coalesces_loop_carried_updates_with_their_header_phi() {
        let code = compile(
            "int state=7;for(int index=0;index<5000;index++){state=Math.imul(state,3)+1;}print(state);",
        );
        assert!(!code.contains(",c;"), "{code}");
    }

    #[test]
    fn coalesces_a_loop_carried_string_update_used_by_an_early_return() {
        let source = "string grow(int count){string result=\"\";while(count>0){count=count-1;string next=result+\"x\";if(next.length>=5){return next;}result=next;}return result;}print(grow(6));";
        let code = compile(source);

        assert!(code.contains("a=a+\"x\""), "{code}");
        assert!(!code.contains(";a=b"), "{code}");
    }

    #[test]
    fn coalesces_one_source_local_across_nested_loop_phis() {
        let source = "extern int next();string grow(int count){string result=\"\";while(true){int index=next();while(index>0){index=index-1;result=result+\"x\";if(result.length>=count){return result;}}}return result;}print(grow(6));";
        let code = compile(source);

        assert!(!code.contains("=a;"), "{code}");
    }

    #[test]
    fn coalesces_a_decrement_used_before_its_loop_phi_copy() {
        let source = "extern string item(int index);string grow(int count){string result=\"\";while(count>0){count=count-1;result=result+item(count);}return result;}print(grow(6));";
        let code = compile(source);
        let postfix = compile_with_options(
            source,
            IrJsOptions {
                phi_affinity_mode: PhiAffinityMode::Direct,
                mutation_spelling: MutationSpelling::Postfix,
                ..IrJsOptions::default()
            },
        );
        let compound = compile_with_options(
            source,
            IrJsOptions {
                mutation_spelling: MutationSpelling::Compound,
                ..IrJsOptions::default()
            },
        );

        assert!(code.contains("a=a-1") || code.contains("b=b-1"), "{code}");
        assert!(
            postfix.contains("a--") || postfix.contains("b--"),
            "{postfix}"
        );
        assert!(!postfix.contains(">0"), "{postfix}");
        assert!(
            compound.contains("a-=1") || compound.contains("b-=1"),
            "{compound}"
        );
        assert!(postfix.len() < code.len(), "{postfix}\n{code}");
    }

    #[test]
    fn does_not_rotate_a_guarded_decrement_when_the_exit_value_is_observed() {
        let output = compile_with_options(
            "extern int read();int countdown(int value){while(value>0){value--;}return value;}print(countdown(read()));",
            IrJsOptions {
                mutation_spelling: MutationSpelling::Postfix,
                ..IrJsOptions::default()
            },
        );

        assert!(output.contains(">0"), "{output}");
    }

    #[test]
    fn coalesces_nested_random_loop_counters_after_postfix_updates() {
        let source = "extern Uint8Array get(float size);extern string alphabet;string grow(float step,float size){string result=\"\";while(true){Uint8Array bytes=get(step);int index=step.toInt();while(index>0){index--;result+=alphabet[bytes[index]&63];if(result.length>=size){return result;}}}return result;}print(grow(8,5));";
        let postfix = compile_with_options(
            source,
            IrJsOptions {
                phi_affinity_mode: PhiAffinityMode::Direct,
                mutation_spelling: MutationSpelling::Postfix,
                ..IrJsOptions::default()
            },
        );

        assert!(postfix.contains("--"), "{postfix}");
    }

    #[test]
    fn keeps_old_and_new_loop_values_separate_when_both_are_observed() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern void observe(string value);string grow(int count){string result=\"o\";while(count>0){count=count-1;string next=result+\"x\";observe(result);result=next;}return result;}",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut ir).unwrap();
        let function = ir
            .functions
            .iter()
            .find(|function| function.name == Some("grow"))
            .unwrap();
        let named = (0..function.value_count).map(ValueId).collect();
        let pairs = safe_two_address_phi_pairs(function, &named, true);
        let string_update = function.blocks.iter().find_map(|block| {
            block.phis.iter().find_map(|phi| {
                if phi.ty != Type::String {
                    return None;
                }
                phi.incoming.iter().find_map(|(_, incoming)| {
                    function
                        .blocks
                        .iter()
                        .flat_map(|candidate| &candidate.instructions)
                        .find(|instruction| {
                            instruction.out == Some(*incoming)
                                && instruction.ty.as_ref() == Some(&Type::String)
                                && matches!(instruction.op, ControlFlowOp::Binary { .. })
                        })
                        .map(|_| (phi.out, *incoming))
                })
            })
        });

        let pair = string_update.expect("fixture must contain a loop-carried string update");
        assert!(!pairs.contains(&pair), "{pairs:?}");
    }

    #[test]
    fn keeps_an_old_loop_value_alive_for_a_sibling_phi_copy() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int fibonacci(int count){int previous=0;int current=1;for(int index=0;index<count;index++){int next=previous+current;previous=current;current=next;}return previous;}",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut ir).unwrap();
        let function = ir
            .functions
            .iter()
            .find(|function| function.name == Some("fibonacci"))
            .unwrap();
        let named = (0..function.value_count).map(ValueId).collect();
        let pairs = safe_two_address_phi_pairs(function, &named, true);
        let definitions = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| instruction.out.map(|out| (out, instruction)))
            .collect::<AHashMap<_, _>>();
        let hazard = function.blocks.iter().find_map(|block| {
            block.phis.iter().find_map(|phi| {
                phi.incoming.iter().find_map(|(predecessor, incoming)| {
                    let instruction = definitions.get(incoming)?;
                    let ControlFlowOp::Binary { lhs, rhs, .. } = instruction.op else {
                        return None;
                    };
                    let is_in_place_candidate = lhs == phi.out || rhs == phi.out;
                    let sibling_needs_old_value = block.phis.iter().any(|other| {
                        other.out != phi.out
                            && other
                                .incoming
                                .iter()
                                .any(|(other_predecessor, other_incoming)| {
                                    other_predecessor == predecessor && *other_incoming == phi.out
                                })
                    });
                    (is_in_place_candidate && sibling_needs_old_value)
                        .then_some((phi.out, *incoming))
                })
            })
        });

        let pair = hazard.expect("fixture must contain the Fibonacci parallel-copy hazard");
        assert!(!pairs.contains(&pair), "{pairs:?}");
    }

    #[test]
    fn coalesces_conditional_loop_updates_with_their_merge_phi() {
        let code = compile(
            "extern bool test(int value);int sum=0;for(int index=0;index<100;index++){if(test(index)){sum+=index;}}print(sum);",
        );

        assert!(!code.contains('?'), "{code}");
    }

    #[test]
    fn phi_affinity_preserves_conditional_loop_index_progress() {
        let output = compile_without_inlining_with_options(
            "int select(bool[] memo){int ready=-1;int user=-1;for(int index=0;index<memo.length;index++){if(memo[index]){if(ready<0){ready=index;}}else if(user<0){user=index;}}if(ready<0){return user;}return ready;}print(select([false,false,true]));",
            false,
            IrJsOptions {
                mangle_identifiers: false,
                phi_affinity_mode: PhiAffinityMode::Grouped,
                ..IrJsOptions::default()
            },
        );

        let loop_counter = output
            .split_once("while(")
            .and_then(|(_, loop_body)| loop_body.split_once('<'))
            .map(|(counter, _)| counter)
            .expect("fixture must emit a named loop counter");
        assert_eq!(
            output.matches(&format!("{loop_counter}=")).count(),
            2,
            "the counter may only be initialized and incremented: {output}"
        );
    }

    #[test]
    fn emits_cross_chunk_imports_and_live_global_exports() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int base=40;void setBase(int value){base=value;}int read(){return base;}int apply(int value){return read()+value;}export{setBase,read,apply};",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_for_module(&mut ir).unwrap();
        let read = ir
            .functions
            .iter()
            .find(|function| function.name == Some("read"))
            .unwrap()
            .id;
        let chunks = emit_optimized_ir_js_chunks_with_options(
            &ir,
            &IrJsOptions::default(),
            &IrJsChunkPlan {
                entry_file: "entry.js".to_string(),
                chunks: vec![IrJsChunkSpec {
                    file_name: "shared.js".to_string(),
                    functions: vec![read],
                    lazy_module: None,
                }],
            },
        )
        .unwrap();

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].file_name, "entry.js");
        assert_eq!(chunks[1].file_name, "shared.js");
        assert!(chunks[0].code.contains("from\"./shared.js\""));
        assert!(chunks[1].code.contains("from\"./entry.js\""));
        assert!(chunks[0].code.contains(" as apply"));
        assert!(chunks[0].code.contains(" as read"));

        let error = emit_optimized_ir_js_chunks_with_options(
            &ir,
            &IrJsOptions::default(),
            &IrJsChunkPlan {
                entry_file: "entry.js".to_string(),
                chunks: vec![IrJsChunkSpec {
                    file_name: "entry.js".to_string(),
                    functions: vec![read],
                    lazy_module: None,
                }],
            },
        )
        .unwrap_err();
        assert!(error.message.contains("duplicate chunk file name"));
    }

    #[test]
    fn declares_pooled_numeric_literals_imported_by_a_chunk() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "export float select(int value){if(value==0){return 134217729.0;}if(value==1){return 134217729.0;}if(value==2){return 134217729.0;}return 134217729.0;}",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_for_module(&mut ir).unwrap();
        let select = ir
            .functions
            .iter()
            .find(|function| function.name == Some("select"))
            .unwrap()
            .id;
        let chunks = emit_optimized_ir_js_chunks_with_options(
            &ir,
            &IrJsOptions {
                pool_numeric_literals: true,
                ..IrJsOptions::default()
            },
            &IrJsChunkPlan {
                entry_file: "entry.js".to_string(),
                chunks: vec![IrJsChunkSpec {
                    file_name: "select.js".to_string(),
                    functions: vec![select],
                    lazy_module: None,
                }],
            },
        )
        .unwrap();

        assert!(chunks[0].code.contains("=134217729"), "{}", chunks[0].code);
        assert!(
            chunks[1].code.contains("from\"./entry.js\""),
            "{}",
            chunks[1].code
        );
    }

    #[test]
    fn constructor_results_do_not_coalesce_with_constructor_arguments() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "class Task{func()->void callback;bool marker;init(func()->void callback,bool marker){this.callback=callback;this.marker=marker;}}int install(func()->void callback,bool marker){Task task=new Task(callback,marker);return 1;}print(install(()=>{print(1);},true));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        crate::optimizer::promote_locals_to_ssa(&mut ir).unwrap();
        let function = ir
            .functions
            .iter()
            .find(|function| function.name == Some("install"))
            .unwrap();
        let callback = function.params[0].value;
        let object = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find_map(|instruction| {
                matches!(instruction.op, ControlFlowOp::NewClass { .. })
                    .then_some(instruction.out)
                    .flatten()
            })
            .unwrap();
        let named = function
            .params
            .iter()
            .map(|parameter| parameter.value)
            .chain(
                function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .filter_map(|instruction| instruction.out),
            )
            .collect::<AHashSet<_>>();
        let parameters = function
            .params
            .iter()
            .map(|parameter| parameter.value)
            .collect::<AHashSet<_>>();
        let colors = coalesce_value_names(
            function,
            &named,
            &parameters,
            &use_counts(function),
            PhiAffinityMode::Grouped,
            &AHashMap::new(),
        );

        assert_ne!(colors[&callback], colors[&object]);

        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "class Holder<T>{T value;func(T,T)->bool equals;init(T value,func(T,T)->bool equals){this.value=value;this.equals=equals;}}Holder<T> holder<T>(T value,(func(T,T)->bool)? equals=null){if(equals==null){return new Holder(value,(T previous,T next)=>previous==next);}return new Holder(value,equals);}bool same(int a,int b){return a==b;}Holder<int> result=holder(1,same);print(result.value);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        crate::optimizer::optimize_control_flow(&mut ir).unwrap();
        let integer_analysis = analyze_integer_values(&ir);
        let function = ir
            .functions
            .iter()
            .find(|function| function.name == Some("holder"))
            .unwrap();
        let named = function
            .params
            .iter()
            .map(|parameter| parameter.value)
            .chain(
                function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .filter_map(|instruction| instruction.out),
            )
            .collect::<AHashSet<_>>();
        let parameters = function
            .params
            .iter()
            .map(|parameter| parameter.value)
            .collect::<AHashSet<_>>();
        let colors = coalesce_value_names(
            function,
            &named,
            &parameters,
            &use_counts(function),
            PhiAffinityMode::Grouped,
            &AHashMap::new(),
        );
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            if let (
                Some(output),
                ControlFlowOp::NewClass {
                    constructor: Some(_),
                    args,
                    ..
                },
            ) = (instruction.out, &instruction.op)
            {
                for argument in args {
                    assert_ne!(colors[&output], colors[argument]);
                }
            }
        }
        let context = LocalNames::new(
            function,
            integer_analysis.function(function.id),
            false,
            &Mangler::default(),
            &AHashMap::new(),
            &AHashMap::new(),
            &IrJsOptions {
                scalar_phi_copies: false,
                ..IrJsOptions::default()
            },
        );
        let mut checked_unwrap = false;
        let mut checked_captureless_closure = false;
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            let (Some(output), ControlFlowOp::NewClass { args, .. }) =
                (instruction.out, &instruction.op)
            else {
                continue;
            };
            for argument in args {
                let is_unwrap = function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .any(|candidate| {
                        candidate.out == Some(*argument)
                            && matches!(
                                &candidate.op,
                                ControlFlowOp::Intrinsic {
                                    intrinsic: Intrinsic::UnwrapNullable,
                                    ..
                                }
                            )
                    });
                if is_unwrap {
                    checked_unwrap = true;
                    assert!(context.is_stored(*argument));
                    assert_ne!(
                        context.value_name(output).unwrap(),
                        context.value_name(*argument).unwrap()
                    );
                }
                let is_captureless_closure = function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .any(|candidate| {
                        candidate.out == Some(*argument)
                            && matches!(
                                &candidate.op,
                                ControlFlowOp::Closure { captures, .. } if captures.is_empty()
                            )
                    });
                if is_captureless_closure {
                    checked_captureless_closure = true;
                    assert!(!context.is_stored(*argument));
                }
            }
        }
        assert!(checked_unwrap);
        assert!(checked_captureless_closure);
    }

    #[test]
    fn captured_values_keep_a_dedicated_color() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern void retain(func()->int callback);void install(int value){func()->int callback=()=>value;retain(callback);int later=value+1;print(later);}install(1);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        crate::optimizer::promote_locals_to_ssa(&mut ir).unwrap();
        let function = ir
            .functions
            .iter()
            .find(|function| function.name == Some("install"))
            .unwrap();
        let captured = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find_map(|instruction| match &instruction.op {
                ControlFlowOp::Closure { captures, .. } => captures.first().copied(),
                _ => None,
            })
            .unwrap();
        let named = function
            .params
            .iter()
            .map(|parameter| parameter.value)
            .chain(
                function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .filter_map(|instruction| instruction.out),
            )
            .collect::<AHashSet<_>>();
        let parameters = function
            .params
            .iter()
            .map(|parameter| parameter.value)
            .collect::<AHashSet<_>>();
        let colors = coalesce_value_names(
            function,
            &named,
            &parameters,
            &use_counts(function),
            PhiAffinityMode::Grouped,
            &AHashMap::new(),
        );

        for (value, color) in &colors {
            if *value != captured {
                assert_ne!(colors[&captured], *color);
            }
        }
    }

    #[test]
    fn closure_wrappers_reserve_capture_expression_identifiers() {
        let mut mangler = Mangler::default();
        reserve_expression_identifiers(&mut mangler, "a[0]+b.c+d");

        assert_eq!(mangler.next_name(), "e");
    }

    #[test]
    fn emits_structs_from_ir() {
        assert_eq!(
            compile("struct Point{int x;int y;}Point p=Point{10,20};print(p.x);"),
            "console.log(10)"
        );
    }

    #[test]
    fn uses_positional_internal_classes_and_named_public_class_abi() {
        let readable = IrJsOptions {
            mangle_identifiers: false,
            mangle_properties: false,
            ..IrJsOptions::default()
        };
        let internal = compile_without_inlining_with_options(
            "int seed=0;int read(){seed+=1;return seed;}class Box{int value;}int update(Box box){box.value=read();return box.value;}Box box=new Box();print(update(box));",
            false,
            readable,
        );
        assert!(internal.contains("[0]"), "{internal}");
        assert!(!internal.contains(".value"), "{internal}");

        let public = compile_module_with_options(
            "export class Box{int value;}export void update(Box box,int value){box.value=value;}",
            readable,
        );
        assert!(public.contains(".value"), "{public}");

        let opaque = compile_module_with_options(
            "export class Box{int value;}export void update(Box box,int value){box.value=value;}",
            IrJsOptions {
                mangle_identifiers: false,
                mangle_properties: false,
                public_aggregate_fields: false,
                ..IrJsOptions::default()
            },
        );
        assert!(opaque.contains("[0]"), "{opaque}");
        assert!(!opaque.contains(".value"), "{opaque}");

        let nested_public_struct = compile_module_with_options(
            "struct Item{string[] items;}export int count(string | Item value){if(value is string){return 0;}return value.items.length;}",
            IrJsOptions {
                mangle_identifiers: false,
                mangle_properties: true,
                mangle_exports: false,
                ..IrJsOptions::default()
            },
        );
        assert!(
            nested_public_struct.contains(".items"),
            "{nested_public_struct}"
        );

        let transitive_public_class = compile_module_with_options(
            "class Leaf{int value;}class Root{Leaf leaf;}export int read(Root root){return root.leaf.value;}",
            readable,
        );
        assert!(
            transitive_public_class.contains(".leaf"),
            "{transitive_public_class}"
        );
        assert!(
            transitive_public_class.contains(".value"),
            "{transitive_public_class}"
        );
    }

    #[test]
    fn rewrites_block_arrows_as_boundary_object_methods() {
        let arrow = JsExpression::raw("(a,b)=>{return a+b}", JsPrecedence::Assignment);
        assert_eq!(
            arrow_block_as_object_method("apply", &arrow).as_deref(),
            Some("apply(a,b){return a+b}")
        );
        let expression = JsExpression::raw("a=>a+1", JsPrecedence::Assignment);
        assert!(arrow_block_as_object_method("apply", &expression).is_none());
    }

    #[test]
    fn emits_invoked_capturing_closures() {
        let output = compile(
            "int apply(int factor){auto callback=(int value)=>value*factor;return callback(4);}print(apply(3));",
        );
        assert!(output.starts_with("console.log(("), "{output}");
        assert!(output.contains("=>"), "{output}");
        assert!(output.ends_with("*3|0)(4))"), "{output}");
    }

    #[test]
    fn folds_signed_i32_overflow() {
        assert_eq!(compile("print(2147483647+1);"), "console.log(-2147483648)");
    }

    #[test]
    fn normalizes_dynamic_tilde_increment_overflow() {
        let output = compile(
            "int[] values=[2147483647];extern int choose();int value=values[choose()]+1;print(value);",
        );
        assert!(output.contains("-~"), "{output}");
        assert!(output.contains("|0"), "{output}");
    }

    #[test]
    fn keeps_tilde_increment_as_a_number_when_the_result_is_float() {
        let output = compile("extern float read();float value=read().toInt()+1.0;print(value);");
        assert!(output.contains("-~read()"), "{output}");
        assert!(!output.contains("-~read()|0"), "{output}");
    }

    #[test]
    fn elides_map_get_null_normalization_behind_a_null_guard() {
        let output = compile_module(
            "export void show(Map<string,int> map){int? value=map.get(\"x\");if(value!=null){print(value);}}",
        );
        assert!(output.contains(".get("), "{output}");
        assert!(!output.contains("??null"), "{output}");
    }

    #[test]
    fn preserves_map_get_null_normalization_at_an_export_boundary() {
        let output =
            compile_module("export int? lookup(Map<string,int> map){return map.get(\"x\");}");
        assert!(output.contains("??null"), "{output}");
    }

    #[test]
    fn preserves_extern_names() {
        assert_eq!(
            compile("extern int hostAdd(int a,int b);int result=hostAdd(1,2);"),
            "hostAdd(1,2)"
        );
    }

    #[test]
    fn entry_bindings_never_reuse_top_level_function_names() {
        let output = compile_without_inlining(
            "int helper(int value){return value+1;}int wrapper(int value){return helper(value);}extern int read();int result=read();print(wrapper(result));",
            true,
        );
        let function_names = output
            .match_indices("function ")
            .filter_map(|(index, _)| {
                output[index + "function ".len()..]
                    .split_once('(')
                    .map(|(name, _)| name)
            })
            .collect::<Vec<_>>();
        let entry = output
            .rsplit_once('}')
            .map_or(output.as_str(), |(_, entry)| entry);
        let declaration = entry
            .strip_prefix("let ")
            .or_else(|| entry.strip_prefix("var "))
            .and_then(|entry| entry.split_once(';'))
            .map_or("", |(declaration, _)| declaration);
        let entry_bindings = declaration
            .split(',')
            .filter_map(|binding| binding.split(['=', ';']).next())
            .collect::<Vec<_>>();

        assert!(
            function_names
                .iter()
                .all(|name| !entry_bindings.contains(name)),
            "{output}"
        );
    }

    #[test]
    fn propagates_shared_constants_through_deep_inlining() {
        assert_eq!(
            compile(
                "int factor=3;int add(int value){return value+factor;}int twice(int value){return add(add(value));}print(twice(4));"
            ),
            "console.log(10)"
        );
    }

    #[test]
    fn eliminates_algebraic_identities_and_pure_calls() {
        assert_eq!(
            compile(
                "extern int read();int square(int value){return value*value;}int value=read();square(9);print((value+0)*1);"
            ),
            "console.log(read())"
        );
    }

    #[test]
    fn eliminates_unused_calls_to_declared_pure_externs() {
        assert_eq!(
            compile("pure extern int stableHash(int value);stableHash(7);print(2);"),
            "console.log(2)"
        );
    }

    #[test]
    fn orders_acyclic_phi_copies_and_preserves_cycles() {
        let assignments = vec![
            ("b".to_string(), "(b+Math.imul(a,2)|0)".to_string()),
            ("a".to_string(), "(a+1|0)".to_string()),
        ];
        assert_eq!(
            order_scalar_assignments(&assignments).unwrap(),
            vec![("b", "(b+Math.imul(a,2)|0)"), ("a", "(a+1|0)")]
        );

        let swap = vec![
            ("a".to_string(), "b".to_string()),
            ("b".to_string(), "a".to_string()),
        ];
        assert!(order_scalar_assignments(&swap).is_none());
        assert_eq!(
            scalar_parallel_assignments(&swap, Some(("c", true))),
            Some("var c=a;a=b;b=c;".to_string())
        );
        assert_eq!(
            replace_identifier("a+(data.a||\"a\")", "a", "b"),
            "b+(data.a||\"a\")"
        );
        assert!(!expression_references_name("data.a+'a'", "a"));
        assert_eq!(
            scalar_parallel_assignments(
                &[
                    ("a".to_string(), "b".to_string()),
                    ("b".to_string(), "data.a".to_string()),
                ],
                Some(("c", true)),
            ),
            Some("a=b;b=data.a;".to_string())
        );
        assert_eq!(
            scalar_parallel_assignments(
                &[
                    ("a".to_string(), "b".to_string()),
                    ("b".to_string(), "`${a}`".to_string()),
                ],
                Some(("c", true)),
            ),
            Some("var c=a;a=b;b=`${c}`;".to_string())
        );
    }

    #[test]
    fn merges_matching_branch_assignments() {
        assert_eq!(
            merge_conditional_assignments("a=(a+1|0);", "a=(a-1|0);"),
            Some((false, "a", "(a+1|0)", "(a-1|0)", ""))
        );
        assert_eq!(
            merge_conditional_assignments("var a=1;", "a=0;"),
            Some((true, "a", "1", "0", ""))
        );
        assert_eq!(
            merge_conditional_assignments("var a=1,b,c;", "a=0;"),
            Some((true, "a", "1", "0", ",b,c"))
        );
        assert!(merge_conditional_assignments("a=1;", "b=2;").is_none());
    }

    #[test]
    fn renders_distinct_branch_assignments_as_an_expression() {
        assert_eq!(
            conditional_assignment_expression("a=b;", "c=d;"),
            Some(("a", "b", "c", "d"))
        );
        assert!(conditional_assignment_expression("var a=b;", "c=d;").is_none());

        let output = compile(
            "extern int read();int left=0;int right=0;void route(int value){if(value>0){left=value;}else{right=value;}}route(read());print(left+right);",
        );
        assert!(output.contains("?"), "{output}");
        assert!(!output.contains("else"), "{output}");
    }

    #[test]
    fn renders_shortest_exact_numeric_literals() {
        assert_eq!(shortest_integer(120_000), "12e4");
        assert_eq!(shortest_float(0.5), ".5");
        assert_eq!(shortest_float(0.0000001), "1e-7");
        assert_eq!(shortest_float(-0.25), "-.25");
    }

    #[test]
    fn derives_identifier_alphabets_from_emitted_character_frequency() {
        let alphabet = IdentifierAlphabet::for_code("nnnnnnneeeeett");
        assert_eq!(encode_identifier(0, &alphabet), "n");
        assert_eq!(encode_identifier(1, &alphabet), "e");
        assert_eq!(encode_identifier(2, &alphabet), "t");
        assert_eq!(
            IdentifierAlphabet::for_code(""),
            IdentifierAlphabet::canonical()
        );
    }

    #[test]
    fn renders_semantically_equivalent_single_quoted_strings() {
        assert_eq!(
            render_string_literal(r#"say \"hi\" and it's\nready"#, StringQuote::Single),
            r#"'say "hi" and it\'s\nready'"#
        );
    }

    #[test]
    fn emits_compact_boolean_constants_and_typed_defaults() {
        let compact = compile(
            "class Flags{bool enabled;}extern void consumeFlags(Flags value);extern void consume(bool value);Flags flags=new Flags();consumeFlags(flags);consume(true);consume(false);",
        );
        assert!(compact.contains("consumeFlags({enabled:!1})"), "{compact}");
        assert!(compact.contains("consume(!0)"), "{compact}");
        assert!(compact.contains("consume(!1)"), "{compact}");

        let keyword = IrJsOptions {
            compact_boolean_literals: false,
            ..IrJsOptions::default()
        };
        let keyword = compile_with_options(
            "extern void consume(bool value);consume(true);consume(false);",
            keyword,
        );
        assert!(keyword.contains("consume(true)"), "{keyword}");
        assert!(keyword.contains("consume(false)"), "{keyword}");
    }

    #[test]
    fn collapses_boolean_phi_identities_without_dropping_effects() {
        let identities = compile(
            "extern int read();int left=read();int right=read();bool first=left == 1 && true;bool second=right == 2 || false;print(first);print(second);",
        );
        assert_eq!(identities.matches("read()").count(), 2, "{identities}");
        assert!(!identities.contains("||"), "{identities}");
        assert!(!identities.contains("&&"), "{identities}");

        let inversion = compile(
            "extern int read();bool value=false;if(read() == 1){value=false;}else{value=true;}print(value);",
        );
        assert!(inversion.contains("read()!=1"), "{inversion}");
        assert!(!inversion.contains('?'), "{inversion}");
    }

    #[test]
    fn hoists_loop_locals_into_the_first_var_group() {
        let output = compile(
            "int total=0;for(int outer=0;outer<12;outer++){if(outer%3==0){continue;}int inner=0;while(inner<4){total+=inner;inner++;}}print(total);",
        );
        assert!(output.starts_with("var "), "{output}");
        assert!(output.contains(";for("), "{output}");
        assert_eq!(output.matches("for(").count(), 2, "{output}");
        assert!(!output.contains("while("), "{output}");
        assert_eq!(output.matches("var ").count(), 1, "{output}");
    }

    #[test]
    fn fuses_deferred_loop_conditions_into_the_header() {
        let output = compile(
            "extern int[] readValues();int[] values=readValues();int total=0;for(int index=0;index<values.length;index++){total+=values[index];}print(total);",
        );
        assert!(output.contains("for(;"), "{output}");
        assert!(!output.contains("for(;;)"), "{output}");
    }

    #[test]
    fn elides_typed_array_length_integer_normalization() {
        let output = compile(
            "extern class Crypto{Uint8Array getRandomValues(Uint8Array values);}extern Crypto crypto;extern float read();Uint8Array random(float bytes){return crypto.getRandomValues(new Uint8Array(bytes.toInt()));}print(random(read()).length);",
        );
        assert!(output.contains("new Uint8Array("), "{output}");
        assert!(
            !output.contains("Uint8Array(") || !output.contains("|0)"),
            "{output}"
        );
        assert!(!output.contains("Uint8Array(a|0)"), "{output}");
        assert!(!output.contains("Uint8Array(bytes|0)"), "{output}");
        assert!(!output.contains("Uint8Array(read()|0)"), "{output}");
    }

    #[test]
    fn elides_in_bounds_fixed_typed_array_read_normalization() {
        let output = compile(
            "Uint8Array values=new Uint8Array(4);for(int index=0;index<values.length;index++){print(values[index]);}",
        );
        assert!(!output.contains("]|0"), "{output}");
    }

    #[test]
    fn elides_only_bounds_proven_owned_int_array_read_coercions() {
        let bounded = compile(
            "extern int read();int count=read()%10+2;int[] values=[];for(int fill=0;fill<count;fill++){values.push(fill);}int end=values.length-1;float total=0.0;for(int index=0;index<end;index++){total+=values[index];total+=values[index+1];}print(total);",
        );
        assert!(!bounded.contains("]|0"), "{bounded}");

        let unchecked = compile("extern int read();int[] values=[1];print(values[read()]);");
        assert!(unchecked.contains("]|0"), "{unchecked}");

        let boundary = compile_module("export int first(int[] values){return values[0];}");
        assert!(boundary.contains("[0]|0"), "{boundary}");
    }

    #[test]
    fn elides_shift_minus_one_mask_coercion() {
        let output = compile(
            "extern class MathHost{pure float log2(float value);}extern MathHost Math;int mask(string alphabet){return (2<<Math.log2(alphabet.length-1.0).toInt())-1;}print(mask(\"abcd\")&3);",
        );
        assert!(
            output.contains(")-1") || output.contains(")-1&") || output.contains("-1)"),
            "{output}"
        );
        assert!(!output.contains("-1|0"), "{output}");
    }

    #[test]
    fn emits_expression_body_for_arrow_bindings() {
        let output = compile_module_with_options(
            "export int double(int value){return value*2;}",
            IrJsOptions {
                function_spelling: FunctionSpelling::Arrow,
                public_function_arrows: true,
                ..IrJsOptions::default()
            },
        );
        assert!(
            output.contains("=>") && !output.contains("{return"),
            "{output}"
        );
    }

    #[test]
    fn emits_javascript_value_guards_and_direct_for_in() {
        let output = compile_module(
            "export string inspect(JsValue value){string out=\"\";if(value is string){out=out+value;}if(value.isArray()){out=out+\"a\";}if(value.isObject()){for(string key in value){if(value[key].truthy()){out=out+key;}}}return out;}",
        );

        assert!(output.contains("typeof"), "{output}");
        assert!(output.contains("Array.isArray("), "{output}");
        assert!(output.contains(" in "), "{output}");
        assert!(!output.contains("Object.keys"), "{output}");
        assert!(!output.contains("JsForIn"), "{output}");
    }

    #[test]
    fn functions_reading_javascript_arguments_cannot_be_arrows() {
        let output = compile_module_with_options(
            "extern JsValue arguments;export float count(){return arguments.length;}",
            IrJsOptions {
                function_spelling: FunctionSpelling::Arrow,
                public_function_arrows: true,
                ..IrJsOptions::default()
            },
        );

        assert!(output.contains("function "), "{output}");
        assert!(!output.contains("=>"), "{output}");
        assert!(output.contains("arguments.length"), "{output}");
    }

    #[test]
    fn renders_effectful_closure_branches_as_expressions() {
        let output = compile_module(
            "struct Ops{func(int[],int)->void choose;func(int[],int)->void append;func(int[],int)->void replace;}export Ops make(){func(int[],int)->void choose=(int[] values,int value)=>{if(values.length!=0){values.push(value);}else{values.push(value+1);}};func(int[],int)->void append=(int[] values,int value)=>{if(values.length!=0){values.push(value);}};func(int[],int)->void replace=(int[] values,int value)=>{if(values.length!=0){values[0]=value;}};return Ops{choose,append,replace};}",
        );

        assert!(output.contains('?'), "{output}");
        assert!(output.contains("&&"), "{output}");
        assert!(
            output.contains("&&(") && output.contains("[0]="),
            "{output}"
        );
        assert!(!output.contains("if("), "{output}");
    }

    #[test]
    fn omits_redundant_integer_remainder_coercions() {
        assert_eq!(
            compile("extern int read();print(read()%7);"),
            "console.log(read()%7)"
        );
        assert!(
            compile("extern int read();print(7%read());").contains("|0"),
            "a runtime zero divisor must still produce LilScript's integer zero"
        );
        let pooled = compile_with_options(
            "extern int read();int a=read()%1000000007;int b=read()%1000000007;print(a);print(b);",
            IrJsOptions {
                pool_numeric_literals: true,
                ..IrJsOptions::default()
            },
        );
        assert!(!pooled.contains("|0"), "{pooled}");
    }

    #[test]
    fn elides_only_range_proven_integer_coercions() {
        let bounded_add = compile("extern int read();print(read()%10+5);");
        assert!(!bounded_add.contains("|0"), "{bounded_add}");

        let bounded_multiply = compile("extern int read();print((read()%10)*(read()%10));");
        assert!(
            !bounded_multiply.contains("Math.imul"),
            "{bounded_multiply}"
        );
        assert!(!bounded_multiply.contains("|0"), "{bounded_multiply}");

        let overflow_capable = compile("extern int read();print(read()+1);");
        assert!(overflow_capable.contains("|0"), "{overflow_capable}");

        let eager = IrJsOptions {
            elide_safe_integer_coercions: false,
            ..IrJsOptions::default()
        };
        let eager = compile_with_options("extern int read();print(read()%10+5);", eager);
        assert!(eager.contains("|0"), "{eager}");
    }

    #[test]
    fn elides_coercions_from_interprocedural_argument_and_return_ranges() {
        let output = compile_without_inlining(
            "extern int read();int digit(int value){return value%10;}int offset(int value){return value+5;}print(offset(digit(read())));",
            true,
        );

        assert!(output.contains("=>a+5"), "{output}");
        assert!(!output.contains("+5|0"), "{output}");
    }

    #[test]
    fn uses_owned_field_ranges_but_invalidates_untyped_owners() {
        let owned = compile_without_inlining(
            "struct Box{int value;}extern int read();int increment(Box box){return box.value+1;}Box box=Box{read()%10};print(increment(box));",
            false,
        );
        assert!(owned.contains("=>a[0]+1"), "{owned}");
        assert!(!owned.contains("+1|0"), "{owned}");

        let exposed = compile_without_inlining(
            "struct Box{int value;}extern int read();extern void mutate(Box box);Box box=Box{read()%10};mutate(box);print(box.value+1);",
            false,
        );
        assert!(exposed.contains("+1|0"), "{exposed}");
    }

    #[test]
    fn never_introduces_math_imul_for_ordinary_multiplication() {
        let small = compile("extern int read();print(read()*3);");
        assert!(small.contains("*3|0"), "{small}");
        assert!(!small.contains("Math.imul"), "{small}");

        let large = compile("extern int read();print(read()*8388608);");
        assert!(large.contains("*8388608|0"), "{large}");
        assert!(!large.contains("Math.imul"), "{large}");
    }

    #[test]
    fn separates_subtraction_from_negative_operands() {
        assert_eq!(
            token_safe_binary_rhs(IrBinaryOp::Sub, "-626380242".to_string()),
            "(-626380242)"
        );
        let output = compile("extern int read();print(read()-(-626380242));");
        assert!(!output.contains("--626380242"), "{output}");
    }

    #[test]
    fn preserves_nested_shift_associativity_after_coercion_elision() {
        let output = compile_with_options(
            "extern int read();int value=read();print(value>>((value%2)>>>18));",
            IrJsOptions {
                elide_safe_integer_coercions: false,
                ..IrJsOptions::default()
            },
        );
        assert!(output.contains(">>(a%2>>>18)"), "{output}");
    }

    #[test]
    fn keeps_integer_coercions_grouped_inside_comparisons() {
        let output = compile("extern int read();print(15>=(read()%0));");
        assert!(output.contains(">=(read()%0|0)"), "{output}");
    }

    #[test]
    fn preserves_explicit_math_imul_calls() {
        let output = compile("extern int read();print(Math.imul(read(),8388608));");
        assert!(output.contains("Math.imul(read(),8388608)"), "{output}");
    }

    #[test]
    fn folds_operator_and_imul_multiplication_with_distinct_semantics() {
        assert_eq!(compile("print(2147483647*2147483647);"), "console.log(0)");
        assert_eq!(
            compile("print(Math.imul(2147483647,2147483647));"),
            "console.log(1)"
        );
    }

    #[test]
    fn emits_simple_branch_bodies_without_braces() {
        let output = compile("extern int read();int value=read();if(value==0){print(1);}print(2);");
        assert!(!output.contains("{console.log"), "{output}");
    }

    #[test]
    fn folds_recursive_guard_returns_into_a_conditional_expression() {
        assert_eq!(
            compile(
                "int factorial(int value){if(value<=1){return 1;}return value*factorial(value-1);}print(factorial(7));"
            ),
            "let a=b=>b<=1?1:b*a(b-1|0)|0;console.log(a(7))"
        );
    }

    #[test]
    fn folds_nested_guard_return_ladders_into_conditional_expressions() {
        let output = compile_without_inlining(
            "extern int read();int clamp(int value,int low,int high){if(low<high){if(value<low){return low;}if(value>high){return high;}return value;}if(value<high){return high;}if(value>low){return low;}return value;}print(clamp(read(),-5,7));",
            false,
        );

        assert!(output.contains("?"), "{output}");
        assert!(!output.contains("if("), "{output}");
    }

    #[test]
    fn eliminates_a_self_recursive_nullable_default_guard() {
        let output = compile_module(
            "struct Box{Map<string,int> all;}export Box make(Map<string,int>? all=null){if(all==null){return make(new Map<string,int>());}return Box{all};}",
        );

        assert!(output.contains("||=new Map"), "{output}");
        assert_eq!(output.matches("new Map").count(), 1, "{output}");
        assert!(output.contains("{return {all:"), "{output}");
        assert!(!output.contains(";return {all:"), "{output}");

        let scalar = compile_module(
            "export string make(string? value=null){if(value==null){return make(\"fallback\");}return value;}",
        );
        assert!(scalar.contains("??="), "{scalar}");
    }

    #[test]
    fn materializes_values_shared_by_conditional_return_arms() {
        let output = compile_without_inlining(
            "extern int read();int classify(int value){int adjusted=value+1;if(adjusted>100){return adjusted-3;}return adjusted*7+11;}print(classify(read()));",
            false,
        );

        assert!(output.contains("+1|0"), "{output}");
        assert_eq!(output.matches("+1|0").count(), 1, "{output}");
    }

    #[test]
    fn emits_simple_loop_bodies_without_braces() {
        let output = compile(
            "extern int read();int count=read();for(int index=0;index<count;index++){print(index);}",
        );
        assert!(!output.contains("{console.log"), "{output}");
    }

    #[test]
    fn preserves_structured_infinite_loops_after_constant_propagation() {
        let output = compile(
            "extern int next();int readPositive(){while(true){int value=next();if(value>0){return value;}}return 0;}print(readPositive());",
        );
        assert!(output.contains("while("), "{output}");
        assert!(!output.contains("switch("), "{output}");
        assert!(output.contains("return "), "{output}");
        assert!(!output.contains("return 0"), "{output}");
    }

    #[test]
    fn obeys_forced_condition_loop_spelling() {
        let source = "extern bool ready();while(ready()){print(1);}";
        let as_while = compile_with_options(
            source,
            IrJsOptions {
                loop_spelling: LoopSpelling::While,
                ..IrJsOptions::default()
            },
        );
        let as_for = compile_with_options(
            source,
            IrJsOptions {
                loop_spelling: LoopSpelling::For,
                ..IrJsOptions::default()
            },
        );

        assert!(as_while.contains("while(ready())"), "{as_while}");
        assert!(as_for.contains("for(;ready();)"), "{as_for}");
    }

    #[test]
    fn emits_forced_do_update_and_conditional_dispatch_variants() {
        let as_do = compile_with_options(
            "extern bool ready();while(ready()){print(1);}",
            IrJsOptions {
                loop_spelling: LoopSpelling::Do,
                ..IrJsOptions::default()
            },
        );
        assert!(as_do.contains("do{"), "{as_do}");
        assert!(as_do.contains("}while(ready())"), "{as_do}");

        let with_update = compile_with_options(
            "extern float read();float total=0;for(float index=0;index<read();index=index+1){total=total+index;}print(total);",
            IrJsOptions {
                loop_spelling: LoopSpelling::For,
                mutation_spelling: MutationSpelling::Compound,
                update_loop_layout: true,
                ..IrJsOptions::default()
            },
        );
        assert!(with_update.contains("+=1"), "{with_update}");
        assert!(with_update.contains("for(;"), "{with_update}");

        let conditional_dispatch = compile_without_inlining_with_options(
            "extern int read();int classify(){int value=read();if(value>0){return value;}return -value;}print(classify());",
            true,
            IrJsOptions {
                control_flow_spelling: ControlFlowSpelling::StateMachine,
                state_machine_spelling: StateMachineSpelling::Conditional,
                ..IrJsOptions::default()
            },
        );
        assert!(
            conditional_dispatch.contains("for(;;){if("),
            "{conditional_dispatch}"
        );
        assert!(
            !conditional_dispatch.contains("switch("),
            "{conditional_dispatch}"
        );
    }

    #[test]
    fn carries_precedence_through_structural_expression_nodes() {
        let comparison = JsExpression::binary(
            IrBinaryOp::Less,
            JsExpression::atom("a"),
            JsExpression::atom("b"),
        );
        let call = JsExpression::call(
            JsExpression::member(JsExpression::atom("Math"), "abs", true),
            [JsExpression::unary("-", JsExpression::atom("a"))],
        );
        let conditional = JsExpression::conditional(
            comparison.clone(),
            call,
            JsExpression::index(JsExpression::atom("values"), JsExpression::atom("a"), true),
        );
        assert_eq!(conditional.into_minimal(), "a<b?Math.abs(-a):values[a]");
        assert_eq!(comparison.negated(), "a>=b");

        let normalized = JsExpression::integer_normalization(JsExpression::binary(
            IrBinaryOp::Add,
            JsExpression::atom("a"),
            JsExpression::atom("b"),
        ));
        assert_eq!(
            normalized.without_integer_normalization().into_minimal(),
            "a+b"
        );
        assert_eq!(
            JsExpression::call(
                JsExpression::member(
                    JsExpression::grouped(
                        "35".to_string(),
                        JsPrecedence::Primary,
                        JsExpressionRoot::Raw,
                    ),
                    "toString",
                    true,
                ),
                [JsExpression::atom("36")],
            )
            .into_minimal(),
            "(35).toString(36)"
        );

        let call = JsExpression::call(JsExpression::atom("factory"), []);
        assert_eq!(
            JsExpression::member(call.clone(), "value", true).into_minimal(),
            "factory().value"
        );
        assert_eq!(
            JsExpression::member(call, "value", false).into_minimal(),
            "(factory()).value"
        );
        assert_eq!(
            JsExpression::member(
                JsExpression::raw("new Map", JsPrecedence::NewWithoutArgs),
                "size",
                true,
            )
            .into_minimal(),
            "(new Map).size"
        );
    }

    #[test]
    fn standard_grammar_elisions_are_independently_configurable() {
        let source = "Map<string,int> values=new Map();values.set(\"x\",1);print(values.size);";
        let compact = compile_with_options(source, IrJsOptions::default());
        let explicit = compile_with_options(
            source,
            IrJsOptions {
                elide_new_parentheses: false,
                elide_call_chain_parentheses: false,
                elide_block_terminal_semicolons: false,
                ..IrJsOptions::default()
            },
        );
        assert!(compact.contains("new Map"), "{compact}");
        assert!(!compact.contains("new Map()"), "{compact}");
        assert!(explicit.contains("new Map()"), "{explicit}");
        assert!(explicit.ends_with(';'), "{explicit}");
        assert!(!compact.ends_with(';'), "{compact}");
    }

    #[test]
    fn compacts_effectful_loop_statement_sequences_only_when_legal() {
        assert_eq!(
            compact_top_level_expression_statements("a=read();b+=a;a>2&&(b+=1);").as_deref(),
            Some("a=read(),b+=a,a>2&&(b+=1);")
        );
        assert_eq!(
            compact_top_level_expression_statements("var a=read();b+=a;"),
            None
        );
        assert_eq!(
            compact_top_level_expression_statements("a=read();return a;"),
            None
        );
        let mut prefix = "function f(){var x;x=read();y=x+1;".to_string();
        assert_eq!(
            take_trailing_expression_statements(&mut prefix).as_deref(),
            Some("x=read(),y=x+1")
        );
        assert_eq!(prefix, "function f(){var x;");
        let mut declaration = "function f(){var x=read();".to_string();
        assert_eq!(take_trailing_expression_statements(&mut declaration), None);
        assert_eq!(declaration, "function f(){var x=read();");

        let output = compile_with_options(
            "extern int read();int value=0;int total=0;for(int index=0;index<4;index++){value=read();total+=value;if(value>2){total+=1;}}print(total);",
            IrJsOptions {
                comma_expressions: true,
                loop_spelling: LoopSpelling::For,
                ..IrJsOptions::default()
            },
        );
        assert!(output.contains(","), "{output}");
        assert!(!output.contains("){"), "{output}");
    }

    #[test]
    fn compacts_only_range_proven_one_use_increments() {
        let proven = compile_with_options(
            "int total=0;for(int index=0;index<4;index++){total+=index;}print(total);",
            IrJsOptions {
                mutation_spelling: MutationSpelling::Postfix,
                ..IrJsOptions::default()
            },
        );
        let unknown = compile_with_options(
            "extern int read();int index=read();while(index<read()){index++;}print(index);",
            IrJsOptions {
                mutation_spelling: MutationSpelling::Postfix,
                ..IrJsOptions::default()
            },
        );

        assert!(proven.contains("++"), "{proven}");
        assert!(!unknown.contains("++"), "{unknown}");
        assert!(unknown.contains("+1|0"), "{unknown}");
    }

    #[test]
    fn emits_constructible_function_expression_variants() {
        let output = compile_module_with_options(
            "struct Ops{func(int)->int apply;}export Ops make(){func(int)->int apply=(int value)=>value+1;return Ops{apply};}",
            IrJsOptions {
                function_spelling: FunctionSpelling::Function,
                ..IrJsOptions::default()
            },
        );

        assert!(output.contains("function "), "{output}");
        assert!(output.contains(":function("), "{output}");
        assert!(!output.contains("=>"), "{output}");
    }

    #[test]
    fn explicit_arrow_mode_can_match_a_nonconstructible_public_abi() {
        let source = "export int increment(int value){return value+1;}";
        let ordinary = compile_module_with_options(source, IrJsOptions::default());
        let arrow = compile_module_with_options(
            source,
            IrJsOptions {
                function_spelling: FunctionSpelling::Arrow,
                public_function_arrows: true,
                ..IrJsOptions::default()
            },
        );

        assert!(ordinary.contains("function "), "{ordinary}");
        assert!(arrow.contains("=>"), "{arrow}");
        assert!(!arrow.contains("function "), "{arrow}");
    }

    #[test]
    fn aliases_repeated_long_strings_when_it_reduces_size() {
        let output = compile(
            "extern void sink(string value);sink(\"a-repeated-application-string\");sink(\"a-repeated-application-string\");sink(\"a-repeated-application-string\");",
        );
        assert!(output.starts_with("let a=\"a-repeated-application-string\";"));
        assert_eq!(output.matches("a-repeated-application-string").count(), 1);
        assert_eq!(output.matches("sink(a)").count(), 3);
    }

    #[test]
    fn aliases_repeated_long_numeric_literals_only_when_profitable() {
        let source = "export float a(){return 134217729.0;}export float b(){return 134217729.0;}export float c(){return 134217729.0;}export float d(){return 134217729.0;}";
        let inline = compile_module_with_options(source, IrJsOptions::default());
        let pooled = compile_module_with_options(
            source,
            IrJsOptions {
                pool_numeric_literals: true,
                ..IrJsOptions::default()
            },
        );

        assert_eq!(inline.matches("134217729").count(), 4, "{inline}");
        assert_eq!(pooled.matches("134217729").count(), 1, "{pooled}");
        assert!(pooled.len() < inline.len(), "{pooled}\n{inline}");
    }

    #[test]
    fn hoists_module_global_initializers_without_repeating_declarations() {
        let output = compile_module(
            "Float64Array left=new Float64Array(4);Float64Array right=new Float64Array(8);void clear(){left=new Float64Array(2);}export int total(){return left.length+right.length;}",
        );

        assert!(!output.starts_with("let "), "{output}");
        assert_eq!(output.matches("var ").count(), 2, "{output}");
        assert!(!output.contains("{let "), "{output}");
    }

    #[test]
    fn materializes_named_structs_at_extern_boundaries() {
        assert_eq!(
            compile(
                "struct Point{int x;int y;}extern void consume(Point p);Point p=Point{1,2};consume(p);"
            ),
            "consume({x:1,y:2})"
        );
    }

    #[test]
    fn inlines_closures_that_read_typed_globals() {
        let output = compile(
            "int factor=2;int[] values=[1,2];auto mapped=values.map((int value)=>value*factor);print(mapped[0]);",
        );
        assert!(output.contains(".map("));
        assert!(output.contains("*2|0"), "{output}");
        assert!(!output.contains("Math.imul"), "{output}");
    }

    #[test]
    fn keeps_loop_counter_init_after_inlined_null_check() {
        let output = compile_module(
            r#"
export extern class Console { void log(string s); }
export extern Console console;
int? CUR = null;
int useState(int initial) {
  if (CUR == null) {
    CUR = initial;
    return initial;
  }
  int cur = CUR;
  return cur;
}
export void run() {
  int n = useState(0);
  string[] list = [];
  list.push("a");
  list.push("b");
  string[] out = [];
  int li = 0;
  while (li < list.length) {
    out.push(list[li]);
    li = li + 1;
  }
  console.log(n.toString() + ":" + out.length.toString());
}
"#,
        );
        let collapsed = output.replace('\n', "");
        let while_idx = collapsed
            .find("while(")
            .unwrap_or_else(|| panic!("expected a while loop: {output}"));
        let counter = collapsed[while_idx + "while(".len()..]
            .split('<')
            .next()
            .expect("while condition");
        assert!(
            !counter.is_empty() && counter.bytes().all(is_js_identifier_byte),
            "could not parse loop counter from: {output}"
        );
        let prefix = &collapsed[..while_idx];
        let assigned = prefix.contains(&format!("{counter}=0"))
            || prefix.contains(&format!("var {counter}=0"))
            || prefix
                .split("var ")
                .any(|chunk| chunk.starts_with(&format!("{counter}=0")));
        assert!(
            assigned,
            "loop counter `{counter}` must be assigned 0 before the loop: {output}"
        );
    }

    #[test]
    fn declares_locals_in_each_inlined_closure_factory_instance() {
        let output = compile(
            r#"
pure func(float)->float make(int count, string direction) {
  return (float value) => {
    float expanded = value * count;
    if (direction == "end") {
      expanded = expanded.floor();
    } else {
      expanded = expanded.ceil();
    }
    return expanded / count;
  };
}
func(float)->float end = make(4, "end");
func(float)->float start = make(4, "start");
print(end(0.5));
print(start(0.5));
"#,
        );

        assert!(
            output.matches("var ").count() >= 2,
            "each sibling closure must declare its own stored values: {output}"
        );
    }
}
