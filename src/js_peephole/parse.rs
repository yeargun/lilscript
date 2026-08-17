use crate::js_peephole::rewrite::Rewrite;
use crate::js_peephole::token::{Token, TokenKind};
use crate::js_peephole::JavaScriptSyntaxMetrics;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Expression<'src> {
    Identifier(&'src str),
    Literal,
    Unary {
        operand: Box<Self>,
    },
    Binary {
        operator: &'src str,
        lhs: Box<Self>,
        rhs: Box<Self>,
    },
    Assignment {
        operator: &'src str,
        lhs: Box<Self>,
        rhs: Box<Self>,
    },
    Conditional {
        condition: Box<Self>,
        then_value: Box<Self>,
        else_value: Box<Self>,
    },
    Call {
        callee: Box<Self>,
        arguments: Vec<Self>,
    },
    Member {
        object: Box<Self>,
        property: Box<Self>,
    },
    Array(Vec<Self>),
    Sequence(Vec<Self>),
}

impl Expression<'_> {
    pub(crate) fn node_count(&self) -> usize {
        match self {
            Self::Identifier(_) | Self::Literal => 1,
            Self::Unary { operand } => 1 + operand.node_count(),
            Self::Binary { lhs, rhs, .. } | Self::Assignment { lhs, rhs, .. } => {
                1 + lhs.node_count() + rhs.node_count()
            }
            Self::Conditional {
                condition,
                then_value,
                else_value,
            } => 1 + condition.node_count() + then_value.node_count() + else_value.node_count(),
            Self::Call { callee, arguments } => {
                1 + callee.node_count() + arguments.iter().map(Self::node_count).sum::<usize>()
            }
            Self::Member { object, property } => 1 + object.node_count() + property.node_count(),
            Self::Array(values) | Self::Sequence(values) => {
                1 + values.iter().map(Self::node_count).sum::<usize>()
            }
        }
    }

    fn max_depth(&self) -> usize {
        match self {
            Self::Identifier(_) | Self::Literal => 1,
            Self::Unary { operand } => 1 + operand.max_depth(),
            Self::Binary { lhs, rhs, .. } | Self::Assignment { lhs, rhs, .. } => {
                1 + lhs.max_depth().max(rhs.max_depth())
            }
            Self::Conditional {
                condition,
                then_value,
                else_value,
            } => {
                1 + condition
                    .max_depth()
                    .max(then_value.max_depth())
                    .max(else_value.max_depth())
            }
            Self::Call { callee, arguments } => {
                1 + arguments
                    .iter()
                    .map(Self::max_depth)
                    .fold(callee.max_depth(), usize::max)
            }
            Self::Member { object, property } => 1 + object.max_depth().max(property.max_depth()),
            Self::Array(values) | Self::Sequence(values) => {
                1 + values.iter().map(Self::max_depth).max().unwrap_or(0)
            }
        }
    }
}

pub(crate) fn syntax_metrics(
    source: &str,
    tokens: &[Token<'_>],
    parsed: &[ParsedRegion<'_>],
    delimiter_nesting: usize,
) -> JavaScriptSyntaxMetrics {
    let functions = tokens
        .iter()
        .filter(|token| token.text == "function" || token.text == "=>")
        .count();
    let branches = tokens
        .iter()
        .filter(|token| matches!(token.text, "if" | "switch" | "case" | "?"))
        .count();
    let loops = tokens
        .iter()
        .filter(|token| matches!(token.text, "for" | "while" | "do"))
        .count();
    let calls = tokens
        .windows(2)
        .filter(|pair| {
            pair[1].text == "("
                && (matches!(
                    pair[0].kind,
                    TokenKind::Identifier | TokenKind::String | TokenKind::Template
                ) || matches!(pair[0].text, ")" | "]"))
                && !matches!(pair[0].text, "if" | "for" | "while" | "switch" | "catch")
        })
        .count();
    let parsed_nodes = non_overlapping_parsed_node_count(parsed);
    let expression_nesting = parsed
        .iter()
        .map(|region| region.expression.max_depth())
        .max()
        .unwrap_or(0);
    let max_nesting = delimiter_nesting.max(expression_nesting);
    let structural_nodes = tokens
        .iter()
        .filter(|token| !matches!(token.text, ";" | "," | "(" | ")" | "[" | "]" | "{" | "}"))
        .count();
    let ast_nodes = structural_nodes.max(parsed_nodes);
    let literal_bytes = tokens
        .iter()
        .filter(|token| {
            matches!(
                token.kind,
                TokenKind::Number | TokenKind::String | TokenKind::Template
            )
        })
        .map(|token| token.text.len())
        .sum::<usize>();

    let parse_cost = (tokens.len() as u64)
        .saturating_mul(8)
        .saturating_add(literal_bytes as u64)
        .saturating_add((max_nesting as u64).saturating_pow(2));
    let compile_cost = (ast_nodes as u64)
        .saturating_mul(12)
        .saturating_add((functions as u64).saturating_mul(64))
        .saturating_add((calls as u64).saturating_mul(12))
        .saturating_add((branches as u64).saturating_mul(32))
        .saturating_add((loops as u64).saturating_mul(48));
    let estimated_memory_bytes = (source.len() as u64)
        .saturating_mul(2)
        .saturating_add((tokens.len() as u64).saturating_mul(24))
        .saturating_add((ast_nodes as u64).saturating_mul(32))
        .saturating_add((max_nesting as u64).saturating_mul(64));

    JavaScriptSyntaxMetrics {
        bytes: source.len(),
        tokens: tokens.len(),
        ast_nodes,
        max_nesting,
        functions,
        calls,
        branches,
        loops,
        parse_cost,
        compile_cost,
        estimated_memory_bytes,
    }
}

pub(crate) fn non_overlapping_parsed_node_count(parsed: &[ParsedRegion<'_>]) -> usize {
    // `parse_expression_regions` deliberately discovers nested suffixes as
    // independent rewrite opportunities (notably every arm after `:` in a
    // conditional chain). Those regions describe the same AST nodes and must
    // not be summed for startup accounting. Regions are produced in ascending
    // start-token order, and successfully parsed overlaps are nested, so the
    // first interval is the maximal expression root; later contained roots are
    // skipped while genuinely disjoint statement expressions remain additive.
    let mut covered_until = 0;
    let mut nodes = 0usize;
    for region in parsed {
        if region.start_token < covered_until {
            continue;
        }
        nodes = nodes.saturating_add(region.expression.node_count());
        covered_until = region.end_token;
    }
    nodes
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedRegion<'src> {
    pub(crate) start_token: usize,
    pub(crate) end_token: usize,
    pub(crate) expression: Expression<'src>,
}

pub(crate) fn parse_expression_regions<'src>(tokens: &[Token<'src>]) -> Vec<ParsedRegion<'src>> {
    let mut regions = Vec::new();
    for start in 0..tokens.len() {
        if !can_start_expression_region(tokens, start) {
            continue;
        }
        let end = expression_region_end(tokens, start);
        if end <= start {
            continue;
        }
        let mut parser = ExpressionParser::new(&tokens[start..end]);
        if let Some(expression) = parser.parse_complete() {
            regions.push(ParsedRegion {
                start_token: start,
                end_token: end,
                expression,
            });
        }
    }
    regions
}

pub(crate) fn can_start_expression_region(tokens: &[Token<'_>], index: usize) -> bool {
    if index == 0 {
        return true;
    }
    matches!(tokens[index - 1].text, "{" | "}" | ";" | ":" | ")")
        && !matches!(
            tokens[index].text,
            "let"
                | "var"
                | "const"
                | "return"
                | "throw"
                | "break"
                | "continue"
                | "case"
                | "default"
                | "else"
        )
}

pub(crate) fn expression_region_end(tokens: &[Token<'_>], start: usize) -> usize {
    let mut stack = Vec::new();
    for (index, token) in tokens.iter().enumerate().skip(start) {
        match token.text {
            "(" | "[" | "{" => stack.push(token.text),
            ")" => {
                if stack.last() == Some(&"(") {
                    stack.pop();
                } else if stack.is_empty() {
                    return index;
                } else {
                    return start;
                }
            }
            "]" => {
                if stack.last() == Some(&"[") {
                    stack.pop();
                } else if stack.is_empty() {
                    return index;
                } else {
                    return start;
                }
            }
            "}" => {
                if stack.last() == Some(&"{") {
                    stack.pop();
                } else if stack.is_empty() {
                    return index;
                } else {
                    return start;
                }
            }
            ";" if stack.is_empty() => return index,
            _ => {}
        }
    }
    tokens.len()
}

pub(crate) fn compound_assignment_rewrite(
    tokens: &[Token<'_>],
    region: &ParsedRegion<'_>,
) -> Option<Rewrite> {
    let Expression::Assignment { operator, lhs, rhs } = &region.expression else {
        return None;
    };
    if *operator != "=" {
        return None;
    }
    let Expression::Identifier(assigned) = lhs.as_ref() else {
        return None;
    };
    let Expression::Binary {
        operator,
        lhs: binary_lhs,
        ..
    } = rhs.as_ref()
    else {
        return None;
    };
    let Expression::Identifier(read) = binary_lhs.as_ref() else {
        return None;
    };
    if assigned != read {
        return None;
    }
    let operator = match *operator {
        "+" => "+",
        "-" => "-",
        "*" => "*",
        "/" => "/",
        "%" => "%",
        "&" => "&",
        "|" => "|",
        "^" => "^",
        "<<" => "<<",
        ">>" => ">>",
        ">>>" => ">>>",
        _ => return None,
    };

    let region_tokens = &tokens[region.start_token..region.end_token];
    let assignment_index = region_tokens.iter().position(|token| token.text == "=")?;
    let binary_index = region_tokens
        .iter()
        .enumerate()
        .skip(assignment_index + 1)
        .find(|(_, token)| token.text == operator)
        .map(|(index, _)| index)?;
    let identifier = region_tokens.first()?;
    let rhs = region_tokens.get(binary_index + 1)?;
    let last = region_tokens.last()?;
    Some(Rewrite {
        start: identifier.start,
        end: last.end,
        rhs_start: rhs.start,
        rhs_end: last.end,
        identifier_start: identifier.start,
        identifier_end: identifier.end,
        operator,
    })
}

pub(crate) struct ExpressionParser<'tokens, 'src> {
    tokens: &'tokens [Token<'src>],
    cursor: usize,
}

impl<'tokens, 'src> ExpressionParser<'tokens, 'src> {
    pub(crate) const fn new(tokens: &'tokens [Token<'src>]) -> Self {
        Self { tokens, cursor: 0 }
    }

    pub(crate) fn parse_complete(&mut self) -> Option<Expression<'src>> {
        let expression = self.parse_expression(1)?;
        (self.cursor == self.tokens.len()).then_some(expression)
    }

    fn parse_expression(&mut self, minimum_precedence: u8) -> Option<Expression<'src>> {
        let mut lhs = self.parse_prefix()?;
        loop {
            lhs = self.parse_postfix(lhs)?;
            let Some(token) = self.peek().copied() else {
                break;
            };
            if token.text == "?" {
                if 3 < minimum_precedence {
                    break;
                }
                self.cursor += 1;
                let then_value = self.parse_expression(1)?;
                self.consume(":")?;
                let else_value = self.parse_expression(3)?;
                lhs = Expression::Conditional {
                    condition: Box::new(lhs),
                    then_value: Box::new(then_value),
                    else_value: Box::new(else_value),
                };
                continue;
            }
            let Some((precedence, right_associative, assignment)) = infix_precedence(token.text)
            else {
                break;
            };
            if precedence < minimum_precedence {
                break;
            }
            self.cursor += 1;
            let rhs = self.parse_expression(if right_associative {
                precedence
            } else {
                precedence + 1
            })?;
            lhs = if token.text == "," {
                match lhs {
                    Expression::Sequence(mut values) => {
                        values.push(rhs);
                        Expression::Sequence(values)
                    }
                    value => Expression::Sequence(vec![value, rhs]),
                }
            } else if assignment {
                Expression::Assignment {
                    operator: token.text,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                }
            } else {
                Expression::Binary {
                    operator: token.text,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                }
            };
        }
        Some(lhs)
    }

    fn parse_prefix(&mut self) -> Option<Expression<'src>> {
        let token = *self.peek()?;
        if matches!(
            token.text,
            "!" | "~" | "+" | "-" | "typeof" | "void" | "delete" | "++" | "--"
        ) {
            self.cursor += 1;
            return Some(Expression::Unary {
                operand: Box::new(self.parse_expression(15)?),
            });
        }
        self.cursor += 1;
        match token.kind {
            TokenKind::Identifier => Some(Expression::Identifier(token.text)),
            TokenKind::Number | TokenKind::String | TokenKind::Template => {
                Some(Expression::Literal)
            }
            TokenKind::Keyword
                if matches!(token.text, "true" | "false" | "null" | "this" | "undefined") =>
            {
                Some(Expression::Literal)
            }
            TokenKind::Punct if token.text == "(" => {
                let expression = self.parse_expression(1)?;
                self.consume(")")?;
                Some(expression)
            }
            TokenKind::Punct if token.text == "[" => {
                let mut values = Vec::new();
                if self.peek().is_some_and(|token| token.text == "]") {
                    self.cursor += 1;
                    return Some(Expression::Array(values));
                }
                loop {
                    values.push(self.parse_expression(2)?);
                    if self.peek().is_some_and(|token| token.text == "]") {
                        self.cursor += 1;
                        break;
                    }
                    self.consume(",")?;
                }
                Some(Expression::Array(values))
            }
            _ => None,
        }
    }

    fn parse_postfix(&mut self, mut expression: Expression<'src>) -> Option<Expression<'src>> {
        loop {
            match self.peek().map(|token| token.text) {
                Some("(") => {
                    self.cursor += 1;
                    let mut arguments = Vec::new();
                    if self.peek().is_some_and(|token| token.text == ")") {
                        self.cursor += 1;
                    } else {
                        loop {
                            arguments.push(self.parse_expression(2)?);
                            if self.peek().is_some_and(|token| token.text == ")") {
                                self.cursor += 1;
                                break;
                            }
                            self.consume(",")?;
                        }
                    }
                    expression = Expression::Call {
                        callee: Box::new(expression),
                        arguments,
                    };
                }
                Some(".") => {
                    self.cursor += 1;
                    let property = *self.peek()?;
                    if !matches!(property.kind, TokenKind::Identifier | TokenKind::Keyword) {
                        return None;
                    }
                    self.cursor += 1;
                    expression = Expression::Member {
                        object: Box::new(expression),
                        property: Box::new(Expression::Identifier(property.text)),
                    };
                }
                Some("[") => {
                    self.cursor += 1;
                    let property = self.parse_expression(1)?;
                    self.consume("]")?;
                    expression = Expression::Member {
                        object: Box::new(expression),
                        property: Box::new(property),
                    };
                }
                Some("++" | "--") => {
                    self.cursor += 1;
                    expression = Expression::Unary {
                        operand: Box::new(expression),
                    };
                }
                _ => break,
            }
        }
        Some(expression)
    }

    fn consume(&mut self, expected: &str) -> Option<()> {
        if self.peek()?.text != expected {
            return None;
        }
        self.cursor += 1;
        Some(())
    }

    fn peek(&self) -> Option<&Token<'src>> {
        self.tokens.get(self.cursor)
    }
}

pub(crate) const fn infix_precedence(operator: &str) -> Option<(u8, bool, bool)> {
    match operator.as_bytes() {
        b"," => Some((1, false, false)),
        b"=" | b"+=" | b"-=" | b"*=" | b"/=" | b"%=" | b"&=" | b"|=" | b"^=" | b"<<=" | b">>="
        | b">>>=" | b"&&=" | b"||=" | b"??=" => Some((2, true, true)),
        b"??" => Some((4, false, false)),
        b"||" => Some((5, false, false)),
        b"&&" => Some((6, false, false)),
        b"|" => Some((7, false, false)),
        b"^" => Some((8, false, false)),
        b"&" => Some((9, false, false)),
        b"==" | b"!=" | b"===" | b"!==" => Some((10, false, false)),
        b"<" | b"<=" | b">" | b">=" | b"in" | b"instanceof" => Some((11, false, false)),
        b"<<" | b">>" | b">>>" => Some((12, false, false)),
        b"+" | b"-" => Some((13, false, false)),
        b"*" | b"/" | b"%" => Some((14, false, false)),
        b"**" => Some((15, true, false)),
        _ => None,
    }
}
