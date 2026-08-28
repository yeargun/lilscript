use crate::js_peephole::rewrite::apply_token_rewrites;
use crate::js_peephole::token::{
    lex, matching_closers, matching_openers, opens_object_literal, Token, TokenKind,
};
use crate::js_peephole::JavaScriptParseError;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Frame {
    ForHeader,
    ControlHeader,
    SwitchHeader,
    FunctionParams { declaration: bool },
    MethodParams,
    Paren,
    Bracket,
    StatementBlock,
    SwitchBody,
    FunctionDeclaration,
    FunctionExpression,
    MethodBody,
    ClassDeclaration,
    ClassExpression,
    Object,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ClosedParen {
    Control,
    For,
    Switch,
    Function { declaration: bool },
    Method,
    Group,
}

/// Drop statement-terminating semicolons that JavaScript will insert again, or
/// that are empty leftovers after a construct that is already a complete
/// statement.
///
/// Compact generated code has no line terminators between statements, so ASI
/// only fires before `}` and at EOF. A semicolon between two same-line
/// statements is therefore kept unless the token before it is a `}` that ends a
/// statement (or a class method), not an expression. `for` header semicolons
/// are grammar and are never candidates. Uncertain `}` kinds fail closed and
/// keep the semicolon.
pub(crate) fn elide_asi_safe_semicolons(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    if tokens.is_empty() {
        return Ok((source.to_string(), 0));
    }
    let matching_close = matching_closers(&tokens);
    let matching_open = matching_openers(&matching_close);
    let analysis = analyze_semicolons(&tokens, &matching_open);
    let mut elide = vec![false; tokens.len()];

    for index in (0..tokens.len()).rev() {
        if tokens[index].text != ";" || !analysis.consider[index] {
            continue;
        }
        if next_significant_after_elision(&tokens, &elide, index + 1)
            .is_none_or(|next| tokens[next].text == "}")
            && !index
                .checked_sub(1)
                .is_some_and(|previous| matches!(tokens[previous].text, ":" | "." | "?."))
        {
            elide[index] = true;
            continue;
        }
        if index.checked_sub(1).is_some_and(|previous| {
            tokens[previous].text == "}" && analysis.leftover_after[previous]
        }) {
            elide[index] = true;
        }
    }

    let replacements = tokens
        .iter()
        .enumerate()
        .filter(|(index, _)| elide[*index])
        .map(|(_, token)| (token.start, token.end, String::new()))
        .collect::<Vec<_>>();
    Ok(apply_token_rewrites(source, replacements))
}

struct SemicolonAnalysis {
    consider: Vec<bool>,
    leftover_after: Vec<bool>,
}

fn analyze_semicolons(tokens: &[Token<'_>], matching_open: &[Option<usize>]) -> SemicolonAnalysis {
    let mut consider = vec![false; tokens.len()];
    let mut leftover_after = vec![false; tokens.len()];
    let mut stack = Vec::<Frame>::new();
    let mut last_closed_paren = None;

    for (index, token) in tokens.iter().enumerate() {
        match token.text {
            "{" => {
                let frame = classify_brace(tokens, index, last_closed_paren, &stack, matching_open);
                stack.push(frame);
                last_closed_paren = None;
            }
            "}" => {
                last_closed_paren = None;
                let closed = stack.pop().unwrap_or(Frame::Object);
                leftover_after[index] = leftover_semicolon_after(closed, stack.last().copied());
            }
            "(" => {
                stack.push(paren_frame(tokens, index, stack.last().copied()));
                last_closed_paren = None;
            }
            ")" => {
                last_closed_paren = Some(closed_paren(stack.pop().unwrap_or(Frame::Paren)));
            }
            "[" => {
                stack.push(Frame::Bracket);
                last_closed_paren = None;
            }
            "]" => {
                stack.pop();
                last_closed_paren = None;
            }
            ";" => {
                last_closed_paren = None;
                consider[index] = semicolon_is_statement_list(&stack);
            }
            _ => last_closed_paren = None,
        }
    }

    SemicolonAnalysis {
        consider,
        leftover_after,
    }
}

fn semicolon_is_statement_list(stack: &[Frame]) -> bool {
    match stack.last() {
        None
        | Some(
            Frame::StatementBlock
            | Frame::SwitchBody
            | Frame::FunctionDeclaration
            | Frame::FunctionExpression
            | Frame::MethodBody
            | Frame::ClassDeclaration
            | Frame::ClassExpression,
        ) => true,
        Some(
            Frame::ForHeader
            | Frame::ControlHeader
            | Frame::SwitchHeader
            | Frame::FunctionParams { .. }
            | Frame::MethodParams
            | Frame::Paren
            | Frame::Bracket
            | Frame::Object,
        ) => false,
    }
}

fn leftover_semicolon_after(closed: Frame, parent: Option<Frame>) -> bool {
    match closed {
        Frame::StatementBlock
        | Frame::SwitchBody
        | Frame::FunctionDeclaration
        | Frame::ClassDeclaration => true,
        Frame::MethodBody
            if matches!(
                parent,
                Some(Frame::ClassDeclaration | Frame::ClassExpression)
            ) =>
        {
            true
        }
        _ => false,
    }
}

fn classify_brace(
    tokens: &[Token<'_>],
    index: usize,
    last_closed_paren: Option<ClosedParen>,
    stack: &[Frame],
    matching_open: &[Option<usize>],
) -> Frame {
    if let Some(closed) = last_closed_paren {
        return frame_after_paren(closed);
    }
    let Some(previous) = index.checked_sub(1) else {
        return Frame::StatementBlock;
    };
    match tokens[previous].text {
        "=>" => Frame::FunctionExpression,
        "else" | "try" | "finally" | "do" | "catch" | "static" => Frame::StatementBlock,
        ";" | "{" | "}" => Frame::StatementBlock,
        ":" => {
            if matches!(stack.last(), Some(Frame::SwitchBody))
                || colon_starts_label(tokens, previous)
            {
                Frame::StatementBlock
            } else {
                Frame::Object
            }
        }
        "class" => class_frame(tokens, previous),
        _ if opens_object_literal(tokens, previous) => Frame::Object,
        _ => class_body_open(tokens, index, matching_open).unwrap_or(Frame::Object),
    }
}

fn frame_after_paren(closed: ClosedParen) -> Frame {
    match closed {
        ClosedParen::Control | ClosedParen::For => Frame::StatementBlock,
        ClosedParen::Switch => Frame::SwitchBody,
        ClosedParen::Function { declaration: true } => Frame::FunctionDeclaration,
        ClosedParen::Function { declaration: false } => Frame::FunctionExpression,
        ClosedParen::Method => Frame::MethodBody,
        ClosedParen::Group => Frame::Object,
    }
}

fn paren_frame(tokens: &[Token<'_>], open: usize, parent: Option<Frame>) -> Frame {
    let Some(previous) = open.checked_sub(1) else {
        return Frame::Paren;
    };
    match tokens[previous].text {
        "if" | "while" | "with" | "catch" => Frame::ControlHeader,
        "for" => Frame::ForHeader,
        "await"
            if previous
                .checked_sub(1)
                .is_some_and(|at| tokens[at].text == "for") =>
        {
            Frame::ForHeader
        }
        "switch" => Frame::SwitchHeader,
        "function" => Frame::FunctionParams {
            declaration: is_declaration_keyword(tokens, previous),
        },
        "*" if previous
            .checked_sub(1)
            .is_some_and(|at| tokens[at].text == "function") =>
        {
            Frame::FunctionParams {
                declaration: is_declaration_keyword(tokens, previous - 1),
            }
        }
        _ => {
            if let Some(function_at) = named_function_before_params(tokens, previous) {
                return Frame::FunctionParams {
                    declaration: is_declaration_keyword(tokens, function_at),
                };
            }
            if matches!(
                parent,
                Some(Frame::ClassDeclaration | Frame::ClassExpression | Frame::Object)
            ) {
                Frame::MethodParams
            } else {
                Frame::Paren
            }
        }
    }
}

fn closed_paren(frame: Frame) -> ClosedParen {
    match frame {
        Frame::ControlHeader => ClosedParen::Control,
        Frame::ForHeader => ClosedParen::For,
        Frame::SwitchHeader => ClosedParen::Switch,
        Frame::FunctionParams { declaration } => ClosedParen::Function { declaration },
        Frame::MethodParams => ClosedParen::Method,
        _ => ClosedParen::Group,
    }
}

fn named_function_before_params(tokens: &[Token<'_>], name_at: usize) -> Option<usize> {
    if !matches!(
        tokens[name_at].kind,
        TokenKind::Identifier | TokenKind::Keyword
    ) {
        return None;
    }
    let before_name = name_at.checked_sub(1)?;
    match tokens[before_name].text {
        "function" => Some(before_name),
        "*" if before_name
            .checked_sub(1)
            .is_some_and(|at| tokens[at].text == "function") =>
        {
            Some(before_name - 1)
        }
        _ => None,
    }
}

fn is_declaration_keyword(tokens: &[Token<'_>], function_or_class_at: usize) -> bool {
    let start = if function_or_class_at > 0
        && tokens[function_or_class_at].text == "function"
        && tokens[function_or_class_at - 1].text == "async"
    {
        function_or_class_at - 1
    } else {
        function_or_class_at
    };
    match start.checked_sub(1) {
        None => true,
        Some(previous) => match tokens[previous].text {
            ";" | "{" | "}" => true,
            "export" => true,
            "default" => previous
                .checked_sub(1)
                .is_some_and(|at| tokens[at].text == "export"),
            _ => false,
        },
    }
}

fn class_frame(tokens: &[Token<'_>], class_at: usize) -> Frame {
    if is_declaration_keyword(tokens, class_at) {
        Frame::ClassDeclaration
    } else {
        Frame::ClassExpression
    }
}

fn class_body_open(
    tokens: &[Token<'_>],
    brace: usize,
    matching_open: &[Option<usize>],
) -> Option<Frame> {
    let class_at = find_class_keyword(tokens, brace, matching_open)?;
    Some(class_frame(tokens, class_at))
}

fn find_class_keyword(
    tokens: &[Token<'_>],
    brace: usize,
    matching_open: &[Option<usize>],
) -> Option<usize> {
    let mut index = brace;
    while index > 0 {
        index -= 1;
        match tokens[index].text {
            "class" => return Some(index),
            ";" => return None,
            "}" | ")" | "]" => {
                let open = matching_open[index]?;
                if open >= index {
                    return None;
                }
                index = open;
            }
            "{" => return None,
            _ => {}
        }
    }
    None
}

fn colon_starts_label(tokens: &[Token<'_>], colon: usize) -> bool {
    let Some(name) = colon.checked_sub(1) else {
        return false;
    };
    if tokens[name].text == "default" {
        return true;
    }
    if !matches!(
        tokens[name].kind,
        TokenKind::Identifier | TokenKind::Keyword
    ) {
        return false;
    }
    match name.checked_sub(1) {
        None => true,
        Some(previous) => matches!(tokens[previous].text, ";" | "{" | "}" | ")"),
    }
}

fn next_significant_after_elision(
    tokens: &[Token<'_>],
    elide: &[bool],
    mut index: usize,
) -> Option<usize> {
    while index < tokens.len() {
        if !elide[index] {
            return Some(index);
        }
        index += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::elide_asi_safe_semicolons;

    fn elide(source: &str) -> String {
        let (out, _) = elide_asi_safe_semicolons(source).unwrap();
        out
    }

    fn asserts_parses(code: &str) {
        let out = std::process::Command::new("node")
            .args(["-e", "new Function(process.argv[1])", code])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "invalid JS: {code}\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn elides_before_block_close_and_eof() {
        assert_eq!(elide("function f(){return x;}"), "function f(){return x}");
        assert_eq!(elide("a();"), "a()");
        assert_eq!(elide("function f(){a();}"), "function f(){a()}");
        assert_eq!(elide("if(x){a();}"), "if(x){a()}");
        asserts_parses(&elide("function f(){return x;}"));
    }

    #[test]
    fn elides_leftover_after_self_terminating_statements() {
        assert_eq!(elide("if(x){a()};b()"), "if(x){a()}b()");
        assert_eq!(elide("while(x){a()};b()"), "while(x){a()}b()");
        assert_eq!(elide("function f(){};g()"), "function f(){}g()");
        assert_eq!(elide("class C{};g()"), "class C{}g()");
        assert_eq!(
            elide("try{a()}catch(e){b()};c()"),
            "try{a()}catch(e){b()}c()"
        );
        assert_eq!(
            elide("function f(){function g(){return 1;};return g()}"),
            "function f(){function g(){return 1}return g()}"
        );
        for source in [
            "if(x){a()};b()",
            "function f(){};g()",
            "class C{};g()",
            "try{a()}catch(e){b()};c()",
        ] {
            asserts_parses(&elide(source));
        }
    }

    #[test]
    fn elides_empty_class_element_after_a_method() {
        assert_eq!(elide("class C{m(){};n(){}}"), "class C{m(){}n(){}}");
        asserts_parses(&elide("class C{m(){return 1;};n(){return 2}}"));
    }

    #[test]
    fn keeps_for_header_and_required_separators() {
        assert_eq!(elide("for(var i=0;i<n;i++)a()"), "for(var i=0;i<n;i++)a()");
        assert_eq!(elide("a();b()"), "a();b()");
        assert_eq!(elide("a();[b]"), "a();[b]");
        assert_eq!(elide("a();(b)"), "a();(b)");
        assert_eq!(elide("a();++b"), "a();++b");
        assert_eq!(elide("return;x"), "return;x");
        assert_eq!(elide("break;x"), "break;x");
        assert_eq!(elide("continue;x"), "continue;x");
        assert_eq!(elide("var f=function(){};g()"), "var f=function(){};g()");
        assert_eq!(elide("var f=class{};g()"), "var f=class{};g()");
        assert_eq!(elide("var o={a:1};g()"), "var o={a:1};g()");
        assert_eq!(elide("if(x);else y"), "if(x);else y");
        assert_eq!(elide("for(;;);g()"), "for(;;);g()");
        assert_eq!(elide("while(x);g()"), "while(x);g()");
        assert_eq!(elide("do x();while(y);g()"), "do x();while(y);g()");
        assert_eq!(elide("if(c)x={a:1};else y"), "if(c)x={a:1};else y");
        assert_eq!(
            elide("class C{x=function(){};m(){}}"),
            "class C{x=function(){};m(){}}"
        );
        assert_eq!(elide("class C{x={a:1};[y]=2}"), "class C{x={a:1};[y]=2}");
        for source in [
            "a();[b]",
            "a();(b)",
            "var f=function(){};g()",
            "var o={a:1};g()",
            "if(x);else y",
            "for(;;);g()",
            "do x();while(y);g()",
            "if(c)x={a:1};else y",
            "class C{x={a:1};[y]=2}",
            "class C{x=function(){};m(){}}",
        ] {
            asserts_parses(source);
            asserts_parses(&elide(source));
            assert_eq!(elide(source), source, "{source}");
        }
    }

    #[test]
    fn elides_chained_terminal_empties_before_a_block_close() {
        assert_eq!(elide("{a();;}"), "{a()}");
        asserts_parses(&elide("function f(){a();;}"));
    }

    #[test]
    fn keeps_semicolon_after_a_colon() {
        assert_eq!(elide("function f(){foo:;}"), "function f(){foo:;}");
        assert_eq!(elide("function f(){a?b:c;}"), "function f(){a?b:c}");
        assert_eq!(elide("function f(){a.;}"), "function f(){a.;}");
        asserts_parses(&elide("function f(){foo:;}"));
        asserts_parses(&elide("function f(){a?b:c;}"));
    }

    #[test]
    fn keeps_continue_separated_from_the_following_statement() {
        let source =
            "function f(a){var i=0,s=0;while(i<a.length){if(a[i]==null)continue;s+=a[i];i++}return s}";
        let out = elide(source);
        assert!(out.contains("continue;s+="), "{out}");
        assert!(out.contains("while(i<a.length){"), "{out}");
    }
}
