use crate::js_peephole::rewrite::{
    apply_token_rewrites, is_property_identifier, rewrite_identifier_span, top_level_stop,
};
use crate::js_peephole::scope::{
    name_is_bound_in_nested_function_between, nested_function_end, parse_function_expression,
};
use crate::js_peephole::token::{
    ascii_identifier_name_string, lex, matching_closers, Token, TokenKind,
};
use crate::js_peephole::JavaScriptParseError;

struct Method {
    name: String,
    params: String,
    body: String,
    computed: bool,
}

struct Accessor {
    name: String,
    get_params: String,
    get_body: String,
    set_params: String,
    set_body: String,
    computed: bool,
}

fn statement_boundary(prev: Option<&str>) -> bool {
    matches!(
        prev,
        None | Some(";") | Some("{") | Some("}") | Some(",") | Some("var") | Some("let") | Some("const")
    )
}

fn skip_group_zero_function<'a>(
    tokens: &'a [Token<'a>],
    at: usize,
) -> Option<(usize, bool)> {
    if tokens.get(at).map(|token| token.text) == Some("function") {
        return Some((at, false));
    }
    if tokens.get(at).map(|token| token.text) == Some("(")
        && tokens.get(at + 1).map(|token| token.text) == Some("0")
        && tokens.get(at + 2).map(|token| token.text) == Some(",")
        && tokens.get(at + 3).map(|token| token.text) == Some("function")
    {
        return Some((at + 3, true));
    }
    None
}

fn constructor_this_aliases<'a>(
    tokens: &'a [Token<'a>],
    body_open: usize,
    body_close: usize,
) -> Vec<&'a str> {
    let mut aliases = Vec::new();
    let mut rejected = Vec::new();
    let mut index = body_open + 1;
    while index + 2 < body_close {
        let name_at = if matches!(tokens[index].text, "var" | "let" | "const")
            && tokens
                .get(index + 1)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
        {
            index + 1
        } else if tokens[index].kind == TokenKind::Identifier
            && statement_boundary(index.checked_sub(1).map(|prev| tokens[prev].text))
        {
            index
        } else {
            index += 1;
            continue;
        };
        if tokens.get(name_at + 1).map(|token| token.text) != Some("=") {
            index += 1;
            continue;
        }
        let name = tokens[name_at].text;
        if tokens.get(name_at + 2).map(|token| token.text) == Some("this")
            && matches!(
                tokens.get(name_at + 3).map(|token| token.text),
                None | Some(";") | Some(",") | Some(")")
            )
        {
            if !rejected.contains(&name) && !aliases.contains(&name) {
                aliases.push(name);
            }
        } else if aliases.contains(&name) {
            rejected.push(name);
            aliases.retain(|alias| *alias != name);
        }
        index += 1;
    }
    aliases
}

fn last_return_this(tokens: &[Token<'_>], body_open: usize, body_close: usize) -> bool {
    let aliases = constructor_this_aliases(tokens, body_open, body_close);
    let mut index = body_close;
    while index > body_open + 1 {
        index -= 1;
        if matches!(tokens[index].text, ";" | "}") {
            continue;
        }
        let wrapped = tokens[index].text == ")";
        if wrapped {
            if index == 0 {
                return false;
            }
            index -= 1;
        }
        let returned_this = tokens[index].text == "this" || aliases.contains(&tokens[index].text);
        if !returned_this {
            return false;
        }
        if wrapped && tokens.get(index.saturating_sub(1)).map(|token| token.text) != Some(",") {
            return false;
        }
        let mut cursor = index;
        let mut depth = 0i32;
        while cursor > body_open + 1 {
            cursor -= 1;
            match tokens[cursor].text {
                ")" | "]" | "}" => depth += 1,
                "(" | "[" | "{" => {
                    if depth == 0 {
                        if tokens[cursor].text == "{" {
                            return false;
                        }
                    } else {
                        depth -= 1;
                    }
                }
                "return" if depth == 0 => return true,
                ";" if depth == 0 => return false,
                _ => {}
            }
        }
        return false;
    }
    false
}

fn strip_trailing_return_this(body: &str, aliases: &[&str]) -> String {
    let trimmed = body.trim_end().trim_end_matches(';').trim_end();
    if let Some(stripped) = trimmed.strip_suffix("return this") {
        return stripped.trim_end_matches(';').to_string();
    }
    for alias in aliases {
        if let Some(stripped) = trimmed.strip_suffix(&format!("return {alias}")) {
            return stripped.trim_end_matches(';').to_string();
        }
        if let Some(stripped) = trimmed.strip_suffix(&format!(",{alias}")) {
            let stripped = stripped.trim_end();
            if let Some(expr) = stripped.strip_prefix("return ") {
                return expr.to_string();
            }
            return stripped.to_string();
        }
    }
    if let Some(stripped) = trimmed.strip_suffix(",this") {
        let stripped = stripped.trim_end();
        if let Some(expr) = stripped.strip_prefix("return ") {
            return expr.to_string();
        }
        return stripped.to_string();
    }
    if let Some(stripped) = trimmed.strip_suffix(",this)") {
        let stripped = stripped.trim_end();
        if let Some(expr) = stripped.strip_prefix("return(") {
            return expr.to_string();
        }
        if let Some(expr) = stripped.strip_prefix("return (") {
            return expr.to_string();
        }
    }
    body.to_string()
}

fn is_ident_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

fn rewrite_super_call(body: &str, base: &str) -> String {
    let needle = format!("{base}.call(this");
    let mut search = 0usize;
    while let Some(rel) = body[search..].find(&needle) {
        let start = search + rel;
        let before_ok = start == 0
            || !is_ident_continue(body.as_bytes()[start - 1]);
        if before_ok {
            let after = start + needle.len();
            let rest = &body[after..];
            if let Some(stripped) = rest.strip_prefix(')') {
                let mut rewritten = String::new();
                rewritten.push_str(&body[..start]);
                rewritten.push_str("super()");
                rewritten.push_str(stripped);
                return rewritten;
            }
            if let Some(stripped) = rest.strip_prefix(',') {
                let mut rewritten = String::new();
                rewritten.push_str(&body[..start]);
                rewritten.push_str("super(");
                rewritten.push_str(stripped);
                return rewritten;
            }
        }
        search = start + needle.len();
    }
    format!("super();{body}")
}

fn parse_params(source: &str, tokens: &[Token<'_>], from: usize, to: usize) -> String {
    if from >= to {
        String::new()
    } else {
        source[tokens[from].start..tokens[to - 1].end].to_string()
    }
}

fn skip_separators(tokens: &[Token<'_>], mut index: usize) -> usize {
    while index < tokens.len() && matches!(tokens[index].text, ";" | ",") {
        index += 1;
    }
    index
}

fn is_method_name(token: &Token<'_>) -> bool {
    if token.kind == TokenKind::Identifier {
        return !is_reserved_method_name(token.text);
    }
    matches!(
        token.text,
        "get" | "set"
            | "delete"
            | "catch"
            | "finally"
            | "throw"
            | "void"
            | "typeof"
            | "instanceof"
            | "in"
            | "of"
    )
}

fn is_reserved_method_name(name: &str) -> bool {
    matches!(
        name,
        "class"
            | "function"
            | "var"
            | "let"
            | "const"
            | "if"
            | "else"
            | "for"
            | "while"
            | "return"
            | "new"
            | "extends"
            | "super"
            | "static"
            | "constructor"
    )
}

fn take_method_function<'a>(
    source: &'a str,
    tokens: &'a [Token<'a>],
    matching_close: &[Option<usize>],
    at: usize,
) -> Option<(String, String, usize)> {
    let method = parse_function_expression(tokens, matching_close, at)?;
    if method.named || method.is_arrow {
        return None;
    }
    let open = method.block_open?;
    Some((
        parse_params(source, tokens, method.params_from, method.params_to),
        source[tokens[open + 1].start..tokens[method.end].start].to_string(),
        method.end,
    ))
}

fn take_class_method_value<'a>(
    source: &'a str,
    tokens: &'a [Token<'a>],
    matching_close: &[Option<usize>],
    at: usize,
) -> Option<(String, String, usize)> {
    take_method_function(source, tokens, matching_close, at)
        .or_else(|| take_bind_this_method(source, tokens, matching_close, at))
}

fn wrapper_returns_receiver_call(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    body_open: usize,
    body_close: usize,
    receiver: &str,
    params: &[&str],
) -> bool {
    let mut index = body_open + 1;
    if tokens.get(index).map(|token| token.text) != Some("return") {
        return false;
    }
    index += 1;
    if tokens.get(index).map(|token| token.text) != Some(receiver)
        || tokens.get(index + 1).map(|token| token.text) != Some("(")
    {
        return false;
    }
    let close = matching_close.get(index + 1).copied().flatten();
    let Some(close) = close else {
        return false;
    };
    if close + 1 != body_close
        && tokens.get(close + 1).map(|token| token.text) != Some(";")
    {
        return false;
    }
    if tokens.get(close + 1).map(|token| token.text) == Some(";") && close + 2 != body_close {
        return false;
    }
    let args = parse_call_args(tokens, matching_close, index + 1);
    let Some(args) = args else {
        return false;
    };
    if args.len() != params.len() + 1 {
        return false;
    }
    tokens[args[0].0].text == "this"
        && args[0].1 == args[0].0 + 1
        && args[1..].iter().enumerate().all(|(offset, (start, end))| {
            *end == *start + 1 && tokens[*start].text == params[offset]
        })
}

fn take_bind_this_method<'a>(
    source: &'a str,
    tokens: &'a [Token<'a>],
    matching_close: &[Option<usize>],
    at: usize,
) -> Option<(String, String, usize)> {
    if tokens.get(at).map(|token| token.text) != Some("(") {
        return None;
    }
    let wrapper_close = matching_close.get(at).copied().flatten()?;
    let receiver_at = at + 1;
    if !tokens
        .get(receiver_at)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
        || tokens.get(receiver_at + 1).map(|token| token.text) != Some("=>")
    {
        return None;
    }
    let receiver = tokens[receiver_at].text;
    let (function_at, _) = skip_group_zero_function(tokens, receiver_at + 2)?;
    let wrapper = parse_function_expression(tokens, matching_close, function_at)?;
    if wrapper.named || wrapper.is_arrow {
        return None;
    }
    let wrapper_params = simple_formals(tokens, wrapper.params_from, wrapper.params_to)?;
    let open = wrapper.block_open?;
    if !wrapper_returns_receiver_call(
        tokens,
        matching_close,
        open,
        wrapper.end,
        receiver,
        &wrapper_params,
    ) {
        return None;
    }
    if tokens.get(wrapper_close + 1).map(|token| token.text) != Some("(") {
        return None;
    }
    let adapted_open = wrapper_close + 1;
    let adapted_close = matching_close.get(adapted_open).copied().flatten()?;
    let adapted = parse_function_expression(tokens, matching_close, adapted_open + 1)?;
    let adapted_params = simple_formals(tokens, adapted.params_from, adapted.params_to)?;
    if adapted_params.len() != wrapper_params.len() + 1 {
        return None;
    }
    let this_param = adapted_params[0];
    let method_params = adapted_params[1..].to_vec();
    let body = if let Some(block) = adapted.block_open {
        rewrite_identifier_span(
            source,
            tokens,
            block + 1,
            adapted.end,
            this_param,
            "this",
        )
    } else {
        let mut body_at = adapted.params_to;
        while tokens.get(body_at).map(|token| token.text) != Some("=>") {
            body_at += 1;
            if body_at >= tokens.len() {
                return None;
            }
        }
        let expr = rewrite_identifier_span(
            source,
            tokens,
            body_at + 1,
            adapted.end + 1,
            this_param,
            "this",
        );
        format!("return {expr}")
    };
    Some((method_params.join(","), body, adapted_close))
}

fn simple_formals<'a>(tokens: &'a [Token<'a>], from: usize, to: usize) -> Option<Vec<&'a str>> {
    if from == to {
        return Some(Vec::new());
    }
    let mut names = Vec::new();
    let mut expect_name = true;
    for token in &tokens[from..to] {
        if expect_name {
            if token.kind != TokenKind::Identifier {
                return None;
            }
            names.push(token.text);
            expect_name = false;
        } else if token.text == "," {
            expect_name = true;
        } else {
            return None;
        }
    }
    if expect_name {
        return None;
    }
    Some(names)
}

fn formals_allowing_defaults<'a>(
    tokens: &'a [Token<'a>],
    from: usize,
    to: usize,
) -> Option<Vec<&'a str>> {
    if from == to {
        return Some(Vec::new());
    }
    let mut names = Vec::new();
    let mut index = from;
    while index < to {
        if tokens[index].kind != TokenKind::Identifier {
            return None;
        }
        names.push(tokens[index].text);
        index += 1;
        if index < to && tokens[index].text == "=" {
            index += 1;
            let mut depth = 0i32;
            while index < to {
                match tokens[index].text {
                    "(" | "[" | "{" => depth += 1,
                    ")" | "]" | "}" => depth -= 1,
                    "," if depth == 0 => break,
                    _ => {}
                }
                index += 1;
            }
        }
        if index >= to {
            break;
        }
        if tokens[index].text != "," {
            return None;
        }
        index += 1;
        if index >= to {
            return None;
        }
    }
    Some(names)
}

fn arguments_used_as_value(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    from: usize,
    to: usize,
) -> bool {
    let mut index = from;
    while index < to {
        if let Some(nested) = nested_function_end(tokens, matching_close, index) {
            index = nested + 1;
            continue;
        }
        if tokens[index].text == "arguments" && !is_property_identifier(tokens, index) {
            let next = tokens.get(index + 1).map(|token| token.text);
            if next != Some("[") && next != Some(".") {
                return true;
            }
        }
        index += 1;
    }
    false
}

fn max_arguments_index(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    from: usize,
    to: usize,
) -> Option<usize> {
    let mut max = None;
    let mut index = from;
    while index < to {
        if let Some(nested) = nested_function_end(tokens, matching_close, index) {
            index = nested + 1;
            continue;
        }
        if tokens[index].text == "arguments"
            && !is_property_identifier(tokens, index)
            && tokens.get(index + 1).map(|token| token.text) == Some("[")
            && tokens
                .get(index + 2)
                .is_some_and(|token| token.kind == TokenKind::Number)
            && tokens.get(index + 3).map(|token| token.text) == Some("]")
        {
            let value = tokens[index + 2].text.parse::<usize>().ok()?;
            if value > 7 {
                return None;
            }
            max = Some(max.unwrap_or(0).max(value));
        }
        index += 1;
    }
    max
}

fn next_formal_name(used: &[&str], index: usize) -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
    let mut candidate = String::new();
    let mut n = index;
    loop {
        candidate.clear();
        let mut value = n;
        candidate.insert(0, ALPHABET[value % 26] as char);
        value /= 26;
        while value > 0 {
            value -= 1;
            candidate.insert(0, ALPHABET[value % 26] as char);
            value /= 26;
        }
        if candidate != "this"
            && candidate != "arguments"
            && !used.iter().any(|name| *name == candidate)
        {
            return candidate;
        }
        n += 1;
    }
}

fn recover_constructor_formals(
    _source: &str,
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    params_from: usize,
    params_to: usize,
    body_open: usize,
    body_close: usize,
    params: &str,
    body: &str,
) -> (String, String) {
    if arguments_used_as_value(tokens, matching_close, body_open + 1, body_close) {
        let Some(existing) = simple_formals(tokens, params_from, params_to) else {
            return (params.to_string(), body.to_string());
        };
        return recover_default_params(params, body, &existing);
    }
    let Some(existing) = simple_formals(tokens, params_from, params_to) else {
        return (params.to_string(), body.to_string());
    };
    let Some(max_index) = max_arguments_index(tokens, matching_close, body_open + 1, body_close)
    else {
        return recover_default_params(params, body, &existing);
    };
    let mut formals = existing.iter().map(|name| (*name).to_string()).collect::<Vec<_>>();
    let used = existing;
    while formals.len() <= max_index {
        formals.push(next_formal_name(&used, formals.len()));
    }
    let formal_refs = formals.iter().map(String::as_str).collect::<Vec<_>>();
    let mut rewritten = body.to_string();
    for (index, formal) in formal_refs.iter().enumerate() {
        rewritten = rewritten.replace(&format!("arguments[{index}]"), formal);
    }
    let param_list = formals.join(",");
    recover_default_params(&param_list, &rewritten, &formal_refs)
}

fn scan_default_literal_end(rest: &str) -> usize {
    let bytes = rest.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'"' | b'\'' => {
                let quote = bytes[index];
                index += 1;
                while index < bytes.len() {
                    if bytes[index] == b'\\' {
                        index = index.saturating_add(2);
                        continue;
                    }
                    if bytes[index] == quote {
                        index += 1;
                        break;
                    }
                    index += 1;
                }
            }
            b'`' => {
                index += 1;
                while index < bytes.len() {
                    if bytes[index] == b'\\' {
                        index = index.saturating_add(2);
                        continue;
                    }
                    if bytes[index] == b'`' {
                        index += 1;
                        break;
                    }
                    index += 1;
                }
            }
            b')' | b',' | b';' | b'?' | b':' => return index,
            _ => index += 1,
        }
    }
    index
}

fn peel_arguments_length_guard(before: &str) -> Option<usize> {
    let mut end = before.len();
    while end > 0 && before.as_bytes()[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    if !before[..end].ends_with("&&") {
        return None;
    }
    end -= 2;
    while end > 0 && before.as_bytes()[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    let bytes = before.as_bytes();
    let mut cursor = end;
    while cursor > 0 && bytes[cursor - 1].is_ascii_digit() {
        cursor -= 1;
    }
    if cursor == end || cursor == 0 || bytes[cursor - 1] != b'>' {
        return None;
    }
    let mut start = cursor - 1;
    const GUARDS: &[&str] = &[
        "(+arguments.length|0)",
        "(arguments.length|0)",
        "(+arguments.length)",
        "+arguments.length|0",
        "arguments.length|0",
        "+arguments.length",
        "(arguments.length)",
        "arguments.length",
    ];
    let prefix = &before[..start];
    for guard in GUARDS {
        if let Some(stripped) = prefix.strip_suffix(*guard) {
            if stripped
                .bytes()
                .last()
                .is_none_or(|byte| !is_ident_continue(byte))
            {
                start = stripped.len();
                return Some(start);
            }
        }
    }
    None
}

fn recover_default_params(params: &str, body: &str, formals: &[&str]) -> (String, String) {
    let mut defaults = vec![None; formals.len()];
    let mut rewritten = body.to_string();
    for (index, formal) in formals.iter().enumerate() {
        let patterns = [
            format!("{formal}!==void 0?{formal}+\"\":"),
            format!("{formal}!==void 0?+{formal}|0:"),
            format!("{formal}!==void 0?{formal}|0:"),
            format!("{formal}!==void 0?+{formal}:"),
            format!("{formal}!==void 0?{formal}:"),
            format!("{formal}!=null?{formal}:"),
        ];
        for pattern in &patterns {
            if let Some(at) = rewritten.find(pattern) {
                let after = at + pattern.len();
                let rest = &rewritten[after..];
                let end = scan_default_literal_end(rest);
                let default = rest[..end].trim();
                if default.is_empty() || default.contains('(') {
                    continue;
                }
                defaults[index] = Some(default.to_string());
                let keep = if pattern.contains("+\"\"") {
                    format!("{formal}+\"\"")
                } else if pattern.contains(&format!("+{formal}|0")) {
                    format!("+{formal}|0")
                } else if pattern.contains(&format!("{formal}|0")) {
                    format!("{formal}|0")
                } else if pattern.contains(&format!("+{formal}")) {
                    format!("+{formal}")
                } else {
                    (*formal).to_string()
                };
                let mut start = at;
                loop {
                    let before = &rewritten[..start];
                    if before.ends_with("&&") {
                        if let Some(guard_start) = peel_arguments_length_guard(before) {
                            start = guard_start;
                            continue;
                        }
                    }
                    break;
                }
                rewritten.replace_range(start..after + end, &keep);
                break;
            }
        }
    }
    let mut param_list = String::new();
    for (index, formal) in formals.iter().enumerate() {
        if !param_list.is_empty() {
            param_list.push(',');
        }
        param_list.push_str(formal);
        if let Some(default) = &defaults[index] {
            param_list.push('=');
            param_list.push_str(default);
        }
    }
    let rewritten = rewrite_optional_length_guards(&rewritten, formals);
    if param_list.is_empty() {
        (params.to_string(), rewritten)
    } else {
        (param_list, rewritten)
    }
}

fn rewrite_optional_length_guards(body: &str, formals: &[&str]) -> String {
    let mut rewritten = body.to_string();
    for (index, formal) in formals.iter().enumerate() {
        let guards = [
            format!("(arguments.length|0)>{index}&&"),
            format!("(+arguments.length|0)>{index}&&"),
            format!("(+arguments.length)>{index}&&"),
            format!("(arguments.length)>{index}&&"),
            format!("arguments.length>{index}&&"),
        ];
        for guard in &guards {
            loop {
                let Some(at) = rewritten.find(guard) else {
                    break;
                };
                let after = at + guard.len();
                let rest = &rewritten[after..];
                if rest.starts_with(&format!("{formal}!==void 0"))
                    || rest.starts_with(&format!("{formal}!=null"))
                    || rest.starts_with(&format!("{formal}&&"))
                    || rest.starts_with(&format!("{formal}?"))
                {
                    rewritten.replace_range(at..after, "");
                    continue;
                }
                if rest.starts_with(&format!("{formal},"))
                    || rest.starts_with(&format!("{formal};"))
                {
                    rewritten.replace_range(at..after, "");
                    continue;
                }
                if rest.starts_with("(this.") || rest.starts_with("this.") {
                    rewritten.replace_range(at..after, &format!("{formal}!==void 0&&"));
                    continue;
                }
                break;
            }
        }
    }
    rewritten
}

fn emit_class(
    name: &str,
    base: Option<&str>,
    params: &str,
    ctor_body: &str,
    methods: &[Method],
    accessors: &[Accessor],
    proto_alias: Option<&str>,
    proto_alias_keyword: Option<&str>,
    declaration: bool,
) -> String {
    let mut class = String::new();
    if !declaration {
        class.push_str(name);
        class.push('=');
    }
        class.push_str("class");
    if declaration {
        class.push(' ');
        class.push_str(name);
    }
    if let Some(parent) = base {
        class.push_str(" extends ");
        class.push_str(parent);
    }
    class.push('{');
    class.push_str("constructor(");
    class.push_str(params);
    class.push_str("){");
    class.push_str(ctor_body);
    class.push('}');
    for method in methods {
        if method.computed {
            class.push('[');
            class.push_str(&method.name);
            class.push(']');
        } else {
            class.push_str(&method.name);
        }
        class.push('(');
        class.push_str(&method.params);
        class.push_str("){");
        class.push_str(&method.body);
        class.push('}');
    }
    for accessor in accessors {
        class.push_str("get ");
        push_class_member_name(&mut class, &accessor.name, accessor.computed);
        class.push('(');
        class.push_str(&accessor.get_params);
        class.push_str("){");
        class.push_str(&accessor.get_body);
        class.push('}');
        if !accessor.set_body.is_empty() || !accessor.set_params.is_empty() {
            class.push_str("set ");
            push_class_member_name(&mut class, &accessor.name, accessor.computed);
            class.push('(');
            class.push_str(&accessor.set_params);
            class.push_str("){");
            class.push_str(&accessor.set_body);
            class.push('}');
        }
    }
    class.push('}');
    if !declaration {
        class.push(';');
    }
    if let Some(alias) = proto_alias {
        if let Some(keyword) = proto_alias_keyword {
            class.push_str(keyword);
            class.push(' ');
        }
        class.push_str(alias);
        class.push('=');
        class.push_str(name);
        class.push_str(".prototype;");
    }
    class
}

fn push_class_member_name(out: &mut String, name: &str, computed: bool) {
    if computed {
        out.push('[');
        out.push_str(name);
        out.push(']');
    } else {
        out.push_str(name);
    }
}

fn emit_class_method(method: &Method) -> String {
    let mut piece = String::new();
    push_class_member_name(&mut piece, &method.name, method.computed);
    piece.push('(');
    piece.push_str(&method.params);
    piece.push_str("){");
    piece.push_str(&method.body);
    piece.push('}');
    piece
}

fn emit_class_accessor(accessor: &Accessor) -> String {
    let mut piece = String::new();
    piece.push_str("get ");
    push_class_member_name(&mut piece, &accessor.name, accessor.computed);
    piece.push('(');
    piece.push_str(&accessor.get_params);
    piece.push_str("){");
    piece.push_str(&accessor.get_body);
    piece.push('}');
    if !accessor.set_body.is_empty() || !accessor.set_params.is_empty() {
        piece.push_str("set ");
        push_class_member_name(&mut piece, &accessor.name, accessor.computed);
        piece.push('(');
        piece.push_str(&accessor.set_params);
        piece.push_str("){");
        piece.push_str(&accessor.set_body);
        piece.push('}');
    }
    piece
}

fn is_quoted_name(token: &Token<'_>, name: &str) -> bool {
    let text = token.text;
    if text == name {
        return true;
    }
    let quoted = (text.starts_with('"') && text.ends_with('"'))
        || (text.starts_with('\'') && text.ends_with('\''));
    quoted && text.len() == name.len() + 2 && &text[1..text.len() - 1] == name
}

fn match_name_prototype<'a>(
    tokens: &'a [Token<'a>],
    at: usize,
    name: Option<&str>,
) -> Option<(&'a str, usize)> {
    if !tokens
        .get(at)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
        || name.is_some_and(|expected| tokens[at].text != expected)
    {
        return None;
    }
    if tokens.get(at + 1).map(|token| token.text) == Some(".")
        && tokens.get(at + 2).map(|token| token.text) == Some("prototype")
    {
        return Some((tokens[at].text, at + 2));
    }
    if tokens.get(at + 1).map(|token| token.text) == Some("[")
        && tokens
            .get(at + 2)
            .is_some_and(|token| is_quoted_name(token, "prototype"))
        && tokens.get(at + 3).map(|token| token.text) == Some("]")
    {
        return Some((tokens[at].text, at + 3));
    }
    None
}

fn match_proto_assign<'a>(
    tokens: &'a [Token<'a>],
    scan: usize,
    name: &str,
) -> Option<(&'a str, usize)> {
    let start = if matches!(tokens.get(scan).map(|token| token.text), Some("var" | "let" | "const"))
    {
        scan + 1
    } else {
        scan
    };
    if tokens
        .get(start)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
        && tokens.get(start + 1).map(|token| token.text) == Some("=")
    {
        if let Some((_, last)) = match_name_prototype(tokens, start + 2, Some(name)) {
            return Some((tokens[start].text, last));
        }
    }
    None
}

fn match_base_proto_decl<'a>(
    tokens: &'a [Token<'a>],
    scan: usize,
) -> Option<((&'a str, &'a str), usize)> {
    let start = if matches!(tokens.get(scan).map(|token| token.text), Some("var" | "let" | "const"))
    {
        scan + 1
    } else {
        scan
    };
    if tokens
        .get(start)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
        && tokens.get(start + 1).map(|token| token.text) == Some("=")
    {
        if let Some((origin, last)) = match_name_prototype(tokens, start + 2, None) {
            return Some(((tokens[start].text, origin), last));
        }
    }
    None
}

fn set_prototype_of_open(
    tokens: &[Token<'_>],
    scan: usize,
    wrappers: &std::collections::HashSet<&str>,
) -> Option<usize> {
    if tokens
        .get(scan)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
        && tokens.get(scan + 1).map(|token| token.text) == Some(".")
        && tokens.get(scan + 2).map(|token| token.text) == Some("setPrototypeOf")
        && tokens.get(scan + 3).map(|token| token.text) == Some("(")
    {
        return Some(scan + 3);
    }
    if tokens
        .get(scan)
        .is_some_and(|token| wrappers.contains(token.text))
        && tokens.get(scan + 1).map(|token| token.text) == Some("(")
    {
        return Some(scan + 1);
    }
    None
}

fn split_two_arg_call(
    tokens: &[Token<'_>],
    open: usize,
) -> Option<(usize, usize, usize, usize)> {
    let mut depth = 0i32;
    let mut comma = None;
    let mut close = None;
    for index in open + 1..tokens.len() {
        match tokens[index].text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => {
                if depth == 0 {
                    if tokens[index].text == ")" {
                        close = Some(index);
                    }
                    break;
                }
                depth -= 1;
            }
            "," if depth == 0 => {
                comma = Some(index);
            }
            _ => {}
        }
    }
    let comma = comma?;
    let close = close?;
    Some((open + 1, comma, comma + 1, close))
}

fn child_is_class_prototype(
    tokens: &[Token<'_>],
    child_at: usize,
    comma: usize,
    name: &str,
    proto_alias: Option<&str>,
) -> bool {
    if match_name_prototype(tokens, child_at, Some(name)).is_some() {
        return true;
    }
    proto_alias.is_some_and(|alias| {
        tokens.get(child_at).map(|token| token.text) == Some(alias) && child_at + 1 == comma
    })
}

fn resolve_set_prototype_parent<'a>(
    tokens: &'a [Token<'a>],
    parent_at: usize,
    close: usize,
    base_alias: Option<(&'a str, &'a str)>,
) -> Option<&'a str> {
    if let Some((origin, last)) = match_name_prototype(tokens, parent_at, None) {
        if last + 1 == close {
            return Some(origin);
        }
    }
    if tokens
        .get(parent_at)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
        && parent_at + 1 == close
    {
        let parent = tokens[parent_at].text;
        return match base_alias {
            Some((alias, origin)) if parent == alias => Some(origin),
            _ => None,
        };
    }
    None
}

fn function_body_is_set_prototype_of(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    at: usize,
) -> bool {
    let Some(function) = parse_function_expression(tokens, matching_close, at) else {
        return false;
    };
    let empty = std::collections::HashSet::new();
    let start = if let Some(open) = function.block_open {
        let mut body = open + 1;
        if tokens.get(body).map(|token| token.text) == Some("return") {
            body += 1;
        }
        body
    } else {
        let mut index = at;
        while index < function.end {
            if tokens.get(index).map(|token| token.text) == Some("=>") {
                return set_prototype_of_open(tokens, index + 1, &empty).is_some();
            }
            index += 1;
        }
        return false;
    };
    set_prototype_of_open(tokens, start, &empty).is_some()
}

fn set_prototype_of_wrappers<'a>(
    tokens: &'a [Token<'a>],
    matching_close: &[Option<usize>],
) -> std::collections::HashSet<&'a str> {
    let mut names = std::collections::HashSet::new();
    let mut index = 0usize;
    while index < tokens.len() {
        if tokens[index].text == "function"
            && tokens
                .get(index + 1)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && function_body_is_set_prototype_of(tokens, matching_close, index)
        {
            names.insert(tokens[index + 1].text);
        }
        let name_at = if matches!(tokens[index].text, "var" | "let" | "const")
            && tokens
                .get(index + 1)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(index + 2).map(|token| token.text) == Some("=")
        {
            Some(index + 1)
        } else if tokens[index].kind == TokenKind::Identifier
            && tokens.get(index + 1).map(|token| token.text) == Some("=")
        {
            Some(index)
        } else {
            None
        };
        if let Some(name_at) = name_at {
            let mut rhs = name_at + 2;
            if tokens.get(rhs).map(|token| token.text) == Some("(")
                && tokens.get(rhs + 1).map(|token| token.text) == Some("0")
                && tokens.get(rhs + 2).map(|token| token.text) == Some(",")
            {
                rhs += 3;
            }
            if function_body_is_set_prototype_of(tokens, matching_close, rhs) {
                names.insert(tokens[name_at].text);
            }
        }
        index += 1;
    }
    names
}

fn match_set_prototype_of<'a>(
    tokens: &'a [Token<'a>],
    scan: usize,
    name: &str,
    proto_alias: Option<&str>,
    base_alias: Option<(&'a str, &'a str)>,
    wrappers: &std::collections::HashSet<&str>,
) -> Option<(&'a str, usize)> {
    let open = set_prototype_of_open(tokens, scan, wrappers)?;
    let (_, comma, parent_at, close) = split_two_arg_call(tokens, open)?;
    if !child_is_class_prototype(tokens, open + 1, comma, name, proto_alias) {
        return None;
    }
    let parent = resolve_set_prototype_parent(tokens, parent_at, close, base_alias)?;
    Some((parent, close))
}

fn consume_set_prototype_of(
    tokens: &[Token<'_>],
    scan: usize,
    name: &str,
    proto_alias: Option<&str>,
    wrappers: &std::collections::HashSet<&str>,
) -> Option<usize> {
    let open = set_prototype_of_open(tokens, scan, wrappers)?;
    let (_, comma, _, close) = split_two_arg_call(tokens, open)?;
    if child_is_class_prototype(tokens, open + 1, comma, name, proto_alias) {
        return Some(close);
    }
    None
}

fn match_constructor_restore(
    tokens: &[Token<'_>],
    scan: usize,
    name: &str,
    proto_alias: Option<&str>,
) -> Option<usize> {
    if let Some(alias) = proto_alias {
        if tokens.get(scan).map(|token| token.text) == Some(alias)
            && tokens.get(scan + 1).map(|token| token.text) == Some(".")
            && tokens.get(scan + 2).map(|token| token.text) == Some("constructor")
            && tokens.get(scan + 3).map(|token| token.text) == Some("=")
            && tokens.get(scan + 4).map(|token| token.text) == Some(name)
        {
            return Some(scan + 4);
        }
        if tokens.get(scan).map(|token| token.text) == Some(alias)
            && tokens.get(scan + 1).map(|token| token.text) == Some("[")
            && tokens
                .get(scan + 2)
                .is_some_and(|token| is_quoted_name(token, "constructor"))
            && tokens.get(scan + 3).map(|token| token.text) == Some("]")
            && tokens.get(scan + 4).map(|token| token.text) == Some("=")
            && tokens.get(scan + 5).map(|token| token.text) == Some(name)
        {
            return Some(scan + 5);
        }
    }
    if tokens.get(scan).map(|token| token.text) == Some(name)
        && tokens.get(scan + 1).map(|token| token.text) == Some(".")
        && tokens.get(scan + 2).map(|token| token.text) == Some("prototype")
        && tokens.get(scan + 3).map(|token| token.text) == Some(".")
        && tokens.get(scan + 4).map(|token| token.text) == Some("constructor")
        && tokens.get(scan + 5).map(|token| token.text) == Some("=")
        && tokens.get(scan + 6).map(|token| token.text) == Some(name)
    {
        return Some(scan + 6);
    }
    if tokens.get(scan).map(|token| token.text) == Some(name)
        && tokens.get(scan + 1).map(|token| token.text) == Some("[")
        && tokens
            .get(scan + 2)
            .is_some_and(|token| is_quoted_name(token, "prototype"))
        && tokens.get(scan + 3).map(|token| token.text) == Some("]")
        && tokens.get(scan + 4).map(|token| token.text) == Some("[")
        && tokens
            .get(scan + 5)
            .is_some_and(|token| is_quoted_name(token, "constructor"))
        && tokens.get(scan + 6).map(|token| token.text) == Some("]")
        && tokens.get(scan + 7).map(|token| token.text) == Some("=")
        && tokens.get(scan + 8).map(|token| token.text) == Some(name)
    {
        return Some(scan + 8);
    }
    None
}

fn infer_base_from_ctor_call<'a>(
    tokens: &'a [Token<'a>],
    body_open: usize,
    body_close: usize,
    name: &str,
) -> Option<&'a str> {
    let mut index = body_open + 1;
    let mut depth = 0i32;
    while index + 4 < body_close {
        match tokens[index].text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            _ => {}
        }
        if depth == 0
            && tokens[index].kind == TokenKind::Identifier
            && tokens[index].text != name
            && tokens.get(index + 1).map(|token| token.text) == Some(".")
            && tokens.get(index + 2).map(|token| token.text) == Some("call")
            && tokens.get(index + 3).map(|token| token.text) == Some("(")
            && tokens.get(index + 4).map(|token| token.text) == Some("this")
        {
            return Some(tokens[index].text);
        }
        index += 1;
    }
    None
}

fn extended_class_names<'a>(tokens: &'a [Token<'a>]) -> std::collections::HashSet<&'a str> {
    let mut names = std::collections::HashSet::new();
    let mut index = 0usize;
    while index + 3 < tokens.len() {
        if tokens[index].text == "class"
            && tokens
                .get(index + 1)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(index + 2).map(|token| token.text) == Some("extends")
        {
            names.insert(tokens[index + 1].text);
        } else if tokens[index].kind == TokenKind::Identifier
            && tokens.get(index + 1).map(|token| token.text) == Some("=")
            && tokens.get(index + 2).map(|token| token.text) == Some("class")
            && tokens.get(index + 3).map(|token| token.text) == Some("extends")
        {
            names.insert(tokens[index].text);
        }
        index += 1;
    }
    names
}

fn call_child_class<'a>(
    tokens: &'a [Token<'a>],
    child_at: usize,
    comma: usize,
    proto_aliases: &[(usize, &'a str, &'a str)],
    call_at: usize,
) -> Option<&'a str> {
    if let Some((origin, last)) = match_name_prototype(tokens, child_at, None) {
        if last + 1 == comma {
            return Some(origin);
        }
    }
    if tokens
        .get(child_at)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
        && child_at + 1 == comma
    {
        return class_for_proto_alias(proto_aliases, tokens[child_at].text, call_at);
    }
    None
}

fn strip_set_prototype_of_call(
    tokens: &[Token<'_>],
    index: usize,
    close: usize,
    replacements: &mut Vec<(usize, usize, String)>,
) {
    let start = if index > 0 && matches!(tokens[index - 1].text, ";" | ",") {
        tokens[index - 1].start
    } else {
        tokens[index].start
    };
    replacements.push((start, tokens[close].end, String::new()));
}

fn strip_redundant_set_prototype_of(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let wrappers = set_prototype_of_wrappers(&tokens, &matching_close);
    let extended = extended_class_names(&tokens);
    if extended.is_empty() {
        return Ok((source.to_string(), 0));
    }
    let proto_aliases = prototype_alias_assignments(&tokens);
    let mut replacements = Vec::new();
    let mut index = 0usize;
    while index + 2 < tokens.len() {
        if let Some(open) = set_prototype_of_open(&tokens, index, &wrappers) {
            if let Some((_, comma, _, close)) = split_two_arg_call(&tokens, open) {
                if call_child_class(&tokens, open + 1, comma, &proto_aliases, index)
                    .is_some_and(|name| extended.contains(name))
                {
                    strip_set_prototype_of_call(&tokens, index, close, &mut replacements);
                    index = close + 1;
                    continue;
                }
            }
        }
        index += 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn identifier_is_later_non_prototype(
    tokens: &[Token<'_>],
    name: &str,
    after: usize,
) -> bool {
    let mut index = after;
    while index + 2 < tokens.len() {
        let name_at = if matches!(tokens[index].text, "var" | "let" | "const")
            && tokens
                .get(index + 1)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens[index + 1].text == name
        {
            index + 1
        } else if tokens[index].kind == TokenKind::Identifier && tokens[index].text == name {
            index
        } else {
            index += 1;
            continue;
        };
        if tokens.get(name_at + 1).map(|token| token.text) != Some("=") {
            index += 1;
            continue;
        }
        if tokens.get(name_at + 3).map(|token| token.text) == Some("prototype") {
            return false;
        }
        return true;
    }
    false
}

fn identifier_has_prototype_assign_before(
    tokens: &[Token<'_>],
    name: &str,
    before: usize,
) -> bool {
    let mut index = 0usize;
    while index + 3 < before && index + 3 < tokens.len() {
        let name_at = if matches!(tokens[index].text, "var" | "let" | "const")
            && tokens
                .get(index + 1)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens[index + 1].text == name
        {
            index + 1
        } else if tokens[index].kind == TokenKind::Identifier && tokens[index].text == name {
            index
        } else {
            index += 1;
            continue;
        };
        if tokens.get(name_at + 1).map(|token| token.text) == Some("=")
            && tokens.get(name_at + 3).map(|token| token.text) == Some("prototype")
        {
            return true;
        }
        index += 1;
    }
    false
}

fn identifier_is_locally_declared(tokens: &[Token<'_>], name: &str) -> bool {
    let mut index = 0usize;
    while index + 1 < tokens.len() {
        if matches!(tokens[index].text, "var" | "let" | "const")
            && tokens
                .get(index + 1)
                .is_some_and(|token| token.kind == TokenKind::Identifier && token.text == name)
        {
            return true;
        }
        if tokens[index].kind == TokenKind::Identifier
            && tokens[index].text == name
            && tokens.get(index + 1).map(|token| token.text) == Some("=")
            && matches!(
                tokens.get(index + 2).map(|token| token.text),
                Some("function") | Some("class")
            )
        {
            return true;
        }
        index += 1;
    }
    false
}

fn parent_alias_is_unusable_prototype(
    tokens: &[Token<'_>],
    name: &str,
    call_at: usize,
    after: usize,
) -> bool {
    if identifier_is_later_non_prototype(tokens, name, after) {
        return true;
    }
    identifier_is_locally_declared(tokens, name)
        && !identifier_has_prototype_assign_before(tokens, name, call_at)
}

fn strip_dangling_set_prototype_of(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let wrappers = set_prototype_of_wrappers(&tokens, &matching_close);
    let mut replacements = Vec::new();
    let mut index = 0usize;
    while index + 2 < tokens.len() {
        if let Some(open) = set_prototype_of_open(&tokens, index, &wrappers) {
            if let Some((_, _, parent_at, close)) = split_two_arg_call(&tokens, open) {
                if tokens
                    .get(parent_at)
                    .is_some_and(|token| token.kind == TokenKind::Identifier)
                    && parent_at + 1 == close
                    && parent_alias_is_unusable_prototype(
                        &tokens,
                        tokens[parent_at].text,
                        index,
                        close,
                    )
                {
                    strip_set_prototype_of_call(&tokens, index, close, &mut replacements);
                    index = close + 1;
                    continue;
                }
            }
        }
        index += 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

pub(crate) fn strip_stale_set_prototype_of(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let (source, redundant) = strip_redundant_set_prototype_of(source)?;
    let (source, dangling) = strip_dangling_set_prototype_of(&source)?;
    Ok((source, redundant + dangling))
}

pub(crate) fn terminate_bare_prototype_before_statement(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::new();
    let mut index = 0usize;
    while index + 1 < tokens.len() {
        if tokens[index].text == "prototype"
            && (tokens[index + 1].kind == TokenKind::Identifier
                || matches!(
                    tokens[index + 1].text,
                    "var" | "let" | "const" | "function" | "class" | "export" | "import"
                ))
        {
            replacements.push((tokens[index].end, tokens[index].end, ";".to_string()));
        }
        index += 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

pub(crate) fn fold_grouped_zero_function_expressions(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut index = 0usize;
    while index + 4 < tokens.len() {
        let Some((function_at, grouped)) = skip_group_zero_function(&tokens, index) else {
            index += 1;
            continue;
        };
        if !grouped {
            index += 1;
            continue;
        }
        let Some(function) = parse_function_expression(&tokens, &matching_close, function_at) else {
            index += 1;
            continue;
        };
        if tokens.get(function.end + 1).map(|token| token.text) != Some(")") {
            index += 1;
            continue;
        }
        let close = function.end + 1;
        if tokens.get(close + 1).map(|token| token.text) == Some("(") {
            index = close + 1;
            continue;
        }
        replacements.push((
            tokens[index].start,
            tokens[function_at].start,
            String::new(),
        ));
        replacements.push((tokens[close].start, tokens[close].end, String::new()));
        index = close + 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

pub(crate) fn fold_constructor_prototype_tables_to_classes(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let (expanded, _) = super::fold_object_assign_literal_to_writes(source)?;
    let source = expanded.as_str();
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let set_proto_wrappers = set_prototype_of_wrappers(&tokens, &matching_close);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 6 < tokens.len() {
        let decl_at = cursor;
        let name_at = if matches!(tokens[cursor].text, "var" | "let" | "const") {
            cursor + 1
        } else {
            cursor
        };
        if !statement_boundary(cursor.checked_sub(1).map(|index| tokens[index].text))
            || tokens
                .get(name_at)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(name_at + 1).map(|token| token.text) != Some("=")
        {
            cursor += 1;
            continue;
        }
        let Some((function_at, grouped)) = skip_group_zero_function(&tokens, name_at + 2) else {
            cursor += 1;
            continue;
        };
        let Some(function) = parse_function_expression(&tokens, &matching_close, function_at) else {
            cursor += 1;
            continue;
        };
        let Some(block_open) = function.block_open else {
            cursor += 1;
            continue;
        };
        if function.named || function.is_arrow || !last_return_this(&tokens, block_open, function.end)
        {
            cursor += 1;
            continue;
        }
        let mut end = function.end;
        if grouped {
            if tokens.get(end + 1).map(|token| token.text) != Some(")") {
                cursor += 1;
                continue;
            }
            end += 1;
        }
        let name = tokens[name_at].text;
        let mut scan = skip_separators(&tokens, end + 1);
        let mut methods = Vec::<Method>::new();
        let mut proto_alias: Option<&str> = None;
        let mut proto_alias_keyword: Option<&str> = None;
        let mut base: Option<&str> = None;
        let mut base_alias: Option<(&str, &str)> = None;
        let mut fused_end = tokens[end].end;
        loop {
            if let Some((alias, last)) = match_proto_assign(&tokens, scan, name) {
                proto_alias = Some(alias);
                if matches!(tokens[scan].text, "var" | "let" | "const") {
                    proto_alias_keyword = Some(tokens[scan].text);
                }
                fused_end = tokens[last].end;
                scan = last + 1;
                if tokens.get(scan).map(|token| token.text) == Some(",")
                    && tokens
                        .get(scan + 1)
                        .is_some_and(|token| token.kind == TokenKind::Identifier)
                    && tokens.get(scan + 2).map(|token| token.text) == Some("=")
                {
                    if let Some((origin, parent_last)) =
                        match_name_prototype(&tokens, scan + 3, None)
                    {
                        base_alias = Some((tokens[scan + 1].text, origin));
                        fused_end = tokens[parent_last].end;
                        scan = parent_last + 1;
                    }
                }
                scan = skip_separators(&tokens, scan);
                continue;
            }
            if let Some((alias, last)) = match_base_proto_decl(&tokens, scan) {
                base_alias = Some(alias);
                fused_end = tokens[last].end;
                scan = skip_separators(&tokens, last + 1);
                continue;
            }
            if let Some((parent, last)) = match_set_prototype_of(
                &tokens,
                scan,
                name,
                proto_alias,
                base_alias,
                &set_proto_wrappers,
            ) {
                base = Some(parent);
                fused_end = tokens[last].end;
                scan = skip_separators(&tokens, last + 1);
                continue;
            }
            if let Some(last) = consume_set_prototype_of(
                &tokens,
                scan,
                name,
                proto_alias,
                &set_proto_wrappers,
            ) {
                fused_end = tokens[last].end;
                scan = skip_separators(&tokens, last + 1);
                continue;
            }
            if let Some(last) = match_constructor_restore(&tokens, scan, name, proto_alias) {
                fused_end = tokens[last].end;
                scan = skip_separators(&tokens, last + 1);
                continue;
            }
            let alias = proto_alias.unwrap_or("");
            if !alias.is_empty()
                && tokens.get(scan).map(|token| token.text) == Some(alias)
                && tokens.get(scan + 1).map(|token| token.text) == Some(".")
                && tokens.get(scan + 2).is_some_and(is_method_name)
                && tokens.get(scan + 3).map(|token| token.text) == Some("=")
            {
                let method_name = tokens[scan + 2].text.to_string();
                let Some((params, body, method_end)) =
                    take_class_method_value(source, &tokens, &matching_close, scan + 4)
                else {
                    break;
                };
                methods.push(Method {
                    name: method_name,
                    params,
                    body,
                    computed: false,
                });
                fused_end = tokens[method_end].end;
                scan = skip_separators(&tokens, method_end + 1);
                continue;
            }
            if !alias.is_empty()
                && tokens.get(scan).map(|token| token.text) == Some(alias)
                && tokens.get(scan + 1).map(|token| token.text) == Some("[")
            {
                let close = matching_close.get(scan + 1).copied().flatten();
                if let Some(close) = close {
                    if tokens.get(close + 1).map(|token| token.text) == Some("=") {
                        if let Some((params, body, method_end)) =
                            take_class_method_value(source, &tokens, &matching_close, close + 2)
                        {
                            let key_tokens = &tokens[scan + 2..close];
                            let ident = key_tokens
                                .first()
                                .and_then(|token| ascii_identifier_name_string(token.text));
                            let computed = !(key_tokens.len() == 1
                                && ident.is_some_and(|name| !is_reserved_method_name(name)));
                            let name = if let Some(ident) = ident.filter(|name| {
                                key_tokens.len() == 1 && !is_reserved_method_name(name)
                            }) {
                                ident.to_string()
                            } else {
                                source[tokens[scan + 2].start..tokens[close].start].to_string()
                            };
                            methods.push(Method {
                                name,
                                params,
                                body,
                                computed,
                            });
                            fused_end = tokens[method_end].end;
                            scan = skip_separators(&tokens, method_end + 1);
                            continue;
                        }
                    }
                }
            }
            if tokens.get(scan).map(|token| token.text) == Some(name)
                && tokens.get(scan + 1).map(|token| token.text) == Some(".")
                && tokens.get(scan + 2).map(|token| token.text) == Some("prototype")
                && tokens.get(scan + 3).map(|token| token.text) == Some(".")
                && tokens.get(scan + 4).is_some_and(is_method_name)
                && tokens.get(scan + 5).map(|token| token.text) == Some("=")
            {
                let method_name = tokens[scan + 4].text.to_string();
                let Some((params, body, method_end)) =
                    take_class_method_value(source, &tokens, &matching_close, scan + 6)
                else {
                    break;
                };
                methods.push(Method {
                    name: method_name,
                    params,
                    body,
                    computed: false,
                });
                fused_end = tokens[method_end].end;
                scan = skip_separators(&tokens, method_end + 1);
                continue;
            }
            if tokens.get(scan).map(|token| token.text) == Some(name)
                && tokens.get(scan + 1).map(|token| token.text) == Some("[")
                && matches!(
                    tokens.get(scan + 2).map(|token| token.text),
                    Some("\"prototype\"") | Some("'prototype'")
                )
            {
                if tokens.get(scan + 3).map(|token| token.text) == Some("]")
                    && tokens.get(scan + 4).map(|token| token.text) == Some("[")
                {
                    let close = matching_close.get(scan + 4).copied().flatten();
                    if let Some(close) = close {
                        if tokens.get(close + 1).map(|token| token.text) == Some("=") {
                            if let Some((params, body, method_end)) =
                                take_class_method_value(source, &tokens, &matching_close, close + 2)
                            {
                                let key = ascii_identifier_name_string(tokens[scan + 5].text)
                                    .unwrap_or(tokens[scan + 5].text);
                                methods.push(Method {
                                    name: key.to_string(),
                                    params,
                                    body,
                                    computed: ascii_identifier_name_string(tokens[scan + 5].text)
                                        .is_none(),
                                });
                                fused_end = tokens[method_end].end;
                                scan = skip_separators(&tokens, method_end + 1);
                                continue;
                            }
                        }
                    }
                }
            }
            break;
        }
        let this_aliases = constructor_this_aliases(&tokens, block_open, function.end);
        let ctor_source = strip_trailing_return_this(
            &source[tokens[block_open + 1].start..tokens[function.end].start],
            &this_aliases,
        );
        if methods.is_empty() && ctor_source.trim().is_empty() {
            cursor = name_at + 1;
            continue;
        }
        let (params, ctor_body) = recover_constructor_formals(
            source,
            &tokens,
            &matching_close,
            function.params_from,
            function.params_to,
            block_open,
            function.end,
            &parse_params(source, &tokens, function.params_from, function.params_to),
            &ctor_source,
        );
        let mut ctor_body = ctor_body;
        if base.is_none() {
            if let Some(parent) =
                infer_base_from_ctor_call(&tokens, block_open, function.end, name)
            {
                if let Some(last) = match_set_prototype_of(
                    &tokens,
                    scan,
                    name,
                    proto_alias,
                    base_alias,
                    &set_proto_wrappers,
                )
                .map(|(_, last)| last)
                .or_else(|| {
                    consume_set_prototype_of(
                        &tokens,
                        scan,
                        name,
                        proto_alias,
                        &set_proto_wrappers,
                    )
                })
                {
                    fused_end = tokens[last].end;
                    scan = skip_separators(&tokens, last + 1);
                }
                base = Some(parent);
            }
        }
        if let Some(parent) = base {
            ctor_body = rewrite_super_call(&ctor_body, parent);
        }
        replacements.push((
            tokens[decl_at].start,
            fused_end,
            emit_class(
                name,
                base,
                &params,
                &ctor_body,
                &methods,
                &[],
                proto_alias,
                proto_alias_keyword,
                matches!(tokens[decl_at].text, "var" | "let" | "const"),
            ),
        ));
        cursor = scan.max(name_at + 1);
    }
    let (source, class_count) = apply_token_rewrites(source, replacements);
    let (source, inline_count) = inline_define_property_installers(&source)?;
    let (source, accessor_count) = fold_define_property_accessors_into_classes(&source)?;
    let (source, absorb_count) = absorb_prototype_members_into_classes(&source)?;
    let (source, default_count) = fold_undefined_defaults_into_formals(&source)?;
    let (source, rest_count) = fold_arguments_slice_to_rest(&source)?;
    let (source, strip_count) = strip_redundant_set_prototype_of(&source)?;
    let (source, dangling_count) = strip_dangling_set_prototype_of(&source)?;
    let (source, semi_count) = terminate_bare_prototype_before_statement(&source)?;
    Ok((
        source,
        class_count
            + inline_count
            + accessor_count
            + absorb_count
            + default_count
            + rest_count
            + strip_count
            + dangling_count
            + semi_count,
    ))
}

fn object_aliases<'a>(tokens: &'a [Token<'a>]) -> std::collections::HashSet<&'a str> {
    let mut names = std::collections::HashSet::from(["Object"]);
    let mut index = 0usize;
    while index + 2 < tokens.len() {
        if tokens[index].kind == TokenKind::Identifier
            && tokens.get(index + 1).map(|token| token.text) == Some("=")
            && tokens.get(index + 2).map(|token| token.text) == Some("Object")
        {
            names.insert(tokens[index].text);
        }
        if matches!(tokens[index].text, "var" | "let" | "const")
            && tokens
                .get(index + 1)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(index + 2).map(|token| token.text) == Some("=")
            && tokens.get(index + 3).map(|token| token.text) == Some("Object")
        {
            names.insert(tokens[index + 1].text);
        }
        index += 1;
    }
    names
}

fn looks_like_define_property_callee(tokens: &[Token<'_>], at: usize, objects: &std::collections::HashSet<&str>) -> bool {
    (tokens.get(at).map(|token| token.text) == Some("Object")
        && tokens.get(at + 1).map(|token| token.text) == Some(".")
        && tokens.get(at + 2).map(|token| token.text) == Some("defineProperty"))
        || (tokens
            .get(at)
            .is_some_and(|token| objects.contains(token.text))
            && tokens.get(at + 1).map(|token| token.text) == Some(".")
            && tokens.get(at + 2).map(|token| token.text) == Some("defineProperty"))
}

fn assignment_name_and_value<'a>(tokens: &'a [Token<'a>], index: usize) -> Option<(&'a str, usize)> {
    if tokens
        .get(index)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
        && tokens.get(index + 1).map(|token| token.text) == Some("=")
    {
        return Some((tokens[index].text, index + 2));
    }
    None
}

fn function_body_call_span(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    value_at: usize,
) -> Option<(usize, usize)> {
    let start = skip_group_zero_function(tokens, value_at)
        .map(|(at, _)| at)
        .unwrap_or(value_at);
    let function = parse_function_expression(tokens, matching_close, start)?;
    let (body, limit) = if let Some(open) = function.block_open {
        let mut body = open + 1;
        if tokens.get(body).map(|token| token.text) == Some("return") {
            body += 1;
        }
        (body, function.end)
    } else {
        let mut body = function.params_to;
        while tokens.get(body).map(|token| token.text) != Some("=>") {
            body += 1;
            if body >= tokens.len() {
                return None;
            }
        }
        (body + 1, function.end + 1)
    };
    Some((body, limit))
}

fn define_property_aliases<'a>(
    tokens: &'a [Token<'a>],
    matching_close: &[Option<usize>],
) -> Vec<&'a str> {
    let objects = object_aliases(tokens);
    let mut names = vec!["Object.defineProperty"];
    let mut assigned = std::collections::HashSet::new();
    let mut index = 0usize;
    while index + 2 < tokens.len() {
        if let Some((name, value_at)) = assignment_name_and_value(tokens, index) {
            if looks_like_define_property_callee(tokens, value_at, &objects)
                && assigned.insert(name)
            {
                names.push(name);
            } else if let Some((body, limit)) =
                function_body_call_span(tokens, matching_close, value_at)
            {
                if looks_like_define_property_callee(tokens, body, &objects)
                    && tokens
                        .get(limit.saturating_sub(1))
                        .is_some_and(|token| token.text == ")" || token.text == ";")
                    && assigned.insert(name)
                {
                    names.push(name);
                }
            }
        }
        if tokens[index].text == "function"
            && tokens
                .get(index + 1)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(index + 2).map(|token| token.text) == Some("(")
        {
            if let Some((body, _)) = function_body_call_span(tokens, matching_close, index) {
                if looks_like_define_property_callee(tokens, body, &objects)
                    && assigned.insert(tokens[index + 1].text)
                {
                    names.push(tokens[index + 1].text);
                }
            }
        }
        index += 1;
    }
    names
}

fn parse_call_args(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    open: usize,
) -> Option<Vec<(usize, usize)>> {
    let close = matching_close.get(open).copied().flatten()?;
    let mut args = Vec::new();
    let mut start = open + 1;
    let mut depth = 0usize;
    let mut index = open + 1;
    while index < close {
        match tokens[index].text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            "," if depth == 0 => {
                if start < index {
                    args.push((start, index));
                }
                start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    if start < close {
        args.push((start, close));
    }
    Some(args)
}

fn installer_call_open(
    tokens: &[Token<'_>],
    aliases: &[&str],
    body: usize,
) -> Option<usize> {
    if aliases.contains(&tokens.get(body).map(|token| token.text).unwrap_or(""))
        && tokens.get(body + 1).map(|token| token.text) == Some("(")
    {
        Some(body + 1)
    } else if tokens.get(body).map(|token| token.text) == Some("Object")
        && tokens.get(body + 1).map(|token| token.text) == Some(".")
        && tokens.get(body + 2).map(|token| token.text) == Some("defineProperty")
        && tokens.get(body + 3).map(|token| token.text) == Some("(")
    {
        Some(body + 3)
    } else {
        None
    }
}

fn installer_from_value<'a>(
    tokens: &'a [Token<'a>],
    matching_close: &[Option<usize>],
    aliases: &[&str],
    name: &'a str,
    value_at: usize,
) -> Option<(&'a str, Vec<&'a str>, usize, usize)> {
    let start = skip_group_zero_function(tokens, value_at)
        .map(|(at, _)| at)
        .unwrap_or(value_at);
    let function = parse_function_expression(tokens, matching_close, start)?;
    let params = simple_formals(tokens, function.params_from, function.params_to)?;
    let (body, limit) = function_body_call_span(tokens, matching_close, start)?;
    let call_open = installer_call_open(tokens, aliases, body)?;
    let call_close = matching_close.get(call_open).copied().flatten()?;
    let after = call_close + 1;
    if tokens.get(after).map(|token| token.text) == Some(";") {
        if after + 1 != limit && after != limit {
            return None;
        }
    } else if after != limit && after + 1 != limit {
        return None;
    }
    let call_args = parse_call_args(tokens, matching_close, call_open)?;
    if call_args.is_empty()
        || tokens[call_args[0].0].text != params.first().copied().unwrap_or("")
    {
        return None;
    }
    Some((name, params, body, call_close))
}

fn installer_from_function<'a>(
    tokens: &'a [Token<'a>],
    matching_close: &[Option<usize>],
    aliases: &[&str],
    function_at: usize,
    name: &'a str,
) -> Option<(&'a str, Vec<&'a str>, usize, usize)> {
    installer_from_value(tokens, matching_close, aliases, name, function_at)
}

fn assigned_function_name<'a>(tokens: &'a [Token<'a>], function_at: usize) -> Option<&'a str> {
    if function_at < 2 {
        return None;
    }
    if tokens.get(function_at - 1).map(|token| token.text) != Some("=") {
        return None;
    }
    let name_at = function_at - 2;
    if !tokens
        .get(name_at)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
    {
        return None;
    }
    Some(tokens[name_at].text)
}

fn inline_define_property_installers(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let aliases = define_property_aliases(&tokens, &matching_close);
    let mut installers = Vec::<(&str, Vec<&str>, usize, usize)>::new();
    let mut seen = std::collections::HashSet::new();
    let mut index = 0usize;
    while index + 2 < tokens.len() {
        if tokens[index].text == "function" {
            let name = if tokens
                .get(index + 1)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
                && tokens.get(index + 2).map(|token| token.text) == Some("(")
            {
                tokens[index + 1].text
            } else if let Some(name) = assigned_function_name(&tokens, index) {
                name
            } else {
                index += 1;
                continue;
            };
            if seen.insert(name) {
                if let Some(installer) =
                    installer_from_function(&tokens, &matching_close, &aliases, index, name)
                {
                    installers.push(installer);
                }
            }
            index += 1;
            continue;
        }
        if let Some((name, value_at)) = assignment_name_and_value(&tokens, index) {
            if seen.insert(name) {
                if let Some(installer) =
                    installer_from_value(&tokens, &matching_close, &aliases, name, value_at)
                {
                    installers.push(installer);
                }
            }
        }
        index += 1;
    }
    if installers.is_empty() {
        return Ok((source.to_string(), 0));
    }
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut inlined_names = std::collections::HashSet::new();
    let mut cursor = 0usize;
    while cursor + 2 < tokens.len() {
        if let Some((name, params, body, call_close)) = installers.iter().find_map(|(name, params, body, call_close)| {
            (tokens[cursor].text == *name
                && tokens.get(cursor + 1).map(|token| token.text) == Some("(")
                && !is_property_identifier(&tokens, cursor)
                && !tokens
                    .get(cursor.saturating_sub(1))
                    .is_some_and(|token| token.text == "function")
                && !call_is_method_or_function_definition(&tokens, &matching_close, cursor))
            .then_some((*name, params, *body, *call_close))
        }) {
            let open = cursor + 1;
            if let Some(args) = parse_call_args(&tokens, &matching_close, open) {
                if args.len() == params.len() {
                    let mut inlined = String::new();
                    let mut copy = body;
                    let mut last = tokens[body].start;
                    while copy <= call_close {
                        if tokens[copy].kind == TokenKind::Identifier
                            && !is_property_identifier(&tokens, copy)
                            && !name_is_bound_in_nested_function_between(
                                &tokens,
                                &matching_close,
                                body,
                                copy,
                                tokens[copy].text,
                            )
                        {
                            if let Some(arg_index) =
                                params.iter().position(|param| *param == tokens[copy].text)
                            {
                                let (start, end) = args[arg_index];
                                inlined.push_str(&source[last..tokens[copy].start]);
                                inlined.push_str(&source[tokens[start].start..tokens[end].start]);
                                last = tokens[copy].end;
                            }
                        }
                        copy += 1;
                    }
                    inlined.push_str(&source[last..tokens[call_close].end]);
                    let end = matching_close
                        .get(open)
                        .copied()
                        .flatten()
                        .unwrap_or(open);
                    replacements.push((tokens[cursor].start, tokens[end].end, inlined));
                    inlined_names.insert(name);
                    cursor = end + 1;
                    continue;
                }
            }
        }
        cursor += 1;
    }
    if !inlined_names.is_empty() {
        let mut index = 0usize;
        while index + 2 < tokens.len() {
            if let Some((name, value_at)) = assignment_name_and_value(&tokens, index) {
                if inlined_names.contains(name) {
                    if let Some(function) = skip_group_zero_function(&tokens, value_at)
                        .and_then(|(at, _)| parse_function_expression(&tokens, &matching_close, at))
                        .or_else(|| parse_function_expression(&tokens, &matching_close, value_at))
                    {
                        if !installer_name_still_called(
                            &tokens,
                            &matching_close,
                            name,
                            &replacements,
                        ) {
                            let (start, end) = super::assignment_span_to_remove(
                                &tokens,
                                index,
                                function.end,
                            );
                            replacements.push((start, end, String::new()));
                        }
                    }
                }
            }
            if tokens[index].text == "function"
                && tokens
                    .get(index + 1)
                    .is_some_and(|token| token.kind == TokenKind::Identifier)
                && inlined_names.contains(tokens[index + 1].text)
            {
                let name = tokens[index + 1].text;
                if let Some(function) =
                    parse_function_expression(&tokens, &matching_close, index)
                {
                    if !installer_name_still_called(
                        &tokens,
                        &matching_close,
                        name,
                        &replacements,
                    ) && !replacements.iter().any(|(_, _, text)| {
                        text.contains(&format!("{name}("))
                    }) {
                        replacements.push((
                            tokens[index].start,
                            tokens[function.end].end,
                            String::new(),
                        ));
                    }
                }
            }
            index += 1;
        }
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn call_is_method_or_function_definition(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    name_at: usize,
) -> bool {
    if tokens.get(name_at + 1).map(|token| token.text) != Some("(") {
        return false;
    }
    let Some(close) = matching_close.get(name_at + 1).copied().flatten() else {
        return false;
    };
    tokens.get(close + 1).map(|token| token.text) == Some("{")
}

fn is_class_method_head(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    name_at: usize,
) -> bool {
    if is_property_identifier(tokens, name_at) {
        return false;
    }
    if !(tokens
        .get(name_at)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
        || tokens.get(name_at).is_some_and(is_method_name))
    {
        return false;
    }
    if matches!(
        name_at
            .checked_sub(1)
            .and_then(|index| tokens.get(index).map(|token| token.text)),
        Some("get" | "set" | "static" | "async" | "function" | "." | "?." | "*")
    ) {
        return false;
    }
    if !matches!(
        name_at
            .checked_sub(1)
            .and_then(|index| tokens.get(index).map(|token| token.text)),
        Some("{" | "}" | ";")
    ) {
        return false;
    }
    call_is_method_or_function_definition(tokens, matching_close, name_at)
}

fn installer_name_still_called(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    name: &str,
    replacements: &[(usize, usize, String)],
) -> bool {
    let mut index = 0usize;
    while index + 1 < tokens.len() {
        if tokens[index].text == name
            && tokens.get(index + 1).map(|token| token.text) == Some("(")
            && !is_property_identifier(tokens, index)
            && !tokens
                .get(index.saturating_sub(1))
                .is_some_and(|token| token.text == "function")
            && !call_is_method_or_function_definition(tokens, matching_close, index)
            && !replacements
                .iter()
                .any(|(start, end, _)| tokens[index].start >= *start && tokens[index].start < *end)
        {
            return true;
        }
        index += 1;
    }
    false
}

fn class_body_close(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    class_at: usize,
) -> Option<usize> {
    let mut body = class_at + 1;
    if tokens
        .get(body)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
    {
        body += 1;
    }
    if tokens.get(body).map(|token| token.text) == Some("extends") {
        body += 2;
    }
    if tokens.get(body).map(|token| token.text) == Some("{") {
        return matching_close.get(body).copied().flatten();
    }
    None
}

fn class_spans<'a>(
    tokens: &'a [Token<'a>],
    matching_close: &[Option<usize>],
) -> Vec<(&'a str, usize)> {
    let mut spans = Vec::new();
    let mut index = 0usize;
    while index + 3 < tokens.len() {
        if tokens[index].text == "class" {
            if tokens
                .get(index + 1)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            {
                if let Some(close) = class_body_close(tokens, matching_close, index) {
                    spans.push((tokens[index + 1].text, close));
                }
            }
        } else if tokens[index].kind == TokenKind::Identifier
            && tokens.get(index + 1).map(|token| token.text) == Some("=")
            && tokens.get(index + 2).map(|token| token.text) == Some("class")
        {
            if let Some(close) = class_body_close(tokens, matching_close, index + 2) {
                spans.push((tokens[index].text, close));
            }
        }
        index += 1;
    }
    spans
}

fn prototype_alias_assignments<'a>(
    tokens: &'a [Token<'a>],
) -> Vec<(usize, &'a str, &'a str)> {
    let mut aliases = Vec::new();
    let mut index = 0usize;
    while index + 4 < tokens.len() {
        let name_at = if tokens[index].kind == TokenKind::Identifier {
            index
        } else if matches!(tokens[index].text, "let" | "var" | "const")
            && tokens
                .get(index + 1)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
        {
            index + 1
        } else {
            index += 1;
            continue;
        };
        if tokens.get(name_at + 1).map(|token| token.text) == Some("=")
            && tokens
                .get(name_at + 2)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(name_at + 3).map(|token| token.text) == Some(".")
            && tokens.get(name_at + 4).map(|token| token.text) == Some("prototype")
        {
            aliases.push((name_at, tokens[name_at].text, tokens[name_at + 2].text));
        }
        index += 1;
    }
    aliases
}

fn identifier_bound_before(tokens: &[Token<'_>], name: &str, before: usize) -> bool {
    let mut index = 0usize;
    while index + 1 < before && index + 1 < tokens.len() {
        if matches!(tokens[index].text, "var" | "let" | "const" | "function")
            && tokens.get(index + 1).map(|token| token.text) == Some(name)
        {
            return true;
        }
        if tokens[index].text == name
            && tokens.get(index + 1).map(|token| token.text) == Some("=")
            && !is_property_identifier(tokens, index)
        {
            return true;
        }
        index += 1;
    }
    false
}

fn class_for_proto_alias<'a>(
    assignments: &[(usize, &'a str, &'a str)],
    alias: &str,
    before: usize,
) -> Option<&'a str> {
    assignments
        .iter()
        .rev()
        .find(|(at, name, _)| *at < before && *name == alias)
        .map(|(_, _, class)| *class)
}

fn fold_define_property_accessors_into_classes(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let classes = class_spans(&tokens, &matching_close);
    if classes.is_empty() {
        return Ok((source.to_string(), 0));
    }
    let aliases = define_property_aliases(&tokens, &matching_close);
    let proto_aliases = prototype_alias_assignments(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut class_inserts = std::collections::HashMap::<usize, String>::new();
    let mut cursor = 0usize;
    while cursor + 8 < tokens.len() {
        let call_at = if tokens.get(cursor).map(|token| token.text) == Some("Object")
            && tokens.get(cursor + 1).map(|token| token.text) == Some(".")
            && tokens.get(cursor + 2).map(|token| token.text) == Some("defineProperty")
            && tokens.get(cursor + 3).map(|token| token.text) == Some("(")
        {
            Some(cursor + 3)
        } else if tokens
            .get(cursor)
            .is_some_and(|token| aliases.contains(&token.text))
            && tokens.get(cursor + 1).map(|token| token.text) == Some("(")
        {
            Some(cursor + 1)
        } else {
            None
        };
        let Some(open) = call_at else {
            cursor += 1;
            continue;
        };
        let Some(close) = matching_close.get(open).copied().flatten() else {
            cursor += 1;
            continue;
        };
        let (class_name, key_at) = if tokens.get(open + 1).is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(open + 2).map(|token| token.text) == Some(".")
            && tokens.get(open + 3).map(|token| token.text) == Some("prototype")
            && tokens.get(open + 4).map(|token| token.text) == Some(",")
        {
            (tokens[open + 1].text, open + 5)
        } else if tokens.get(open + 1).is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(open + 2).map(|token| token.text) == Some(",")
        {
            let Some(class_name) = class_for_proto_alias(&proto_aliases, tokens[open + 1].text, open)
            else {
                cursor += 1;
                continue;
            };
            (class_name, open + 3)
        } else {
            cursor += 1;
            continue;
        };
        let Some((_, class_close)) = classes.iter().find(|(name, _)| *name == class_name) else {
            cursor += 1;
            continue;
        };
        let Some(key_token) = tokens.get(key_at) else {
            cursor += 1;
            continue;
        };
        let Some((key, computed)) =
            computed_key_name(&tokens, key_token, key_at, *class_close)
        else {
            cursor += 1;
            continue;
        };
        if tokens.get(key_at + 1).map(|token| token.text) != Some(",")
            || tokens.get(key_at + 2).map(|token| token.text) != Some("{")
        {
            cursor += 1;
            continue;
        }
        let Some(desc_close) = matching_close.get(key_at + 2).copied().flatten() else {
            cursor += 1;
            continue;
        };
        let mut get_params = String::new();
        let mut get_body = String::new();
        let mut set_params = String::new();
        let mut set_body = String::new();
        let mut scan = key_at + 3;
        while scan < desc_close {
            let field = tokens[scan].text;
            if (field == "get" || field == "set") && tokens.get(scan + 1).map(|token| token.text) == Some(":")
            {
                if let Some((params, body, end)) =
                    take_method_function(source, &tokens, &matching_close, scan + 2)
                {
                    if field == "get" {
                        get_params = params;
                        get_body = body;
                    } else {
                        set_params = params;
                        set_body = body;
                    }
                    scan = end + 1;
                    continue;
                }
            }
            scan += 1;
        }
        if get_body.is_empty() {
            cursor += 1;
            continue;
        }
        let accessor = Accessor {
            name: key,
            get_params,
            get_body,
            set_params,
            set_body,
            computed,
        };
        class_inserts
            .entry(*class_close)
            .or_default()
            .push_str(&emit_class_accessor(&accessor));
        let start = if cursor > 0 && matches!(tokens[cursor.saturating_sub(1)].text, ";" | ",") {
            tokens[cursor.saturating_sub(1)].start
        } else {
            tokens[cursor].start
        };
        replacements.push((start, tokens[close].end, String::new()));
        cursor = close + 1;
    }
    for (close, insert) in class_inserts {
        replacements.push((tokens[close].start, tokens[close].start, insert));
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn skip_call_statement(tokens: &[Token<'_>], matching_close: &[Option<usize>], at: usize) -> Option<usize> {
    if tokens
        .get(at)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
        && tokens.get(at + 1).map(|token| token.text) == Some("(")
    {
        return matching_close.get(at + 1).copied().flatten();
    }
    if tokens.get(at).map(|token| token.text) == Some("Object")
        && tokens.get(at + 1).map(|token| token.text) == Some(".")
        && tokens.get(at + 2).map(|token| token.text) == Some("defineProperty")
        && tokens.get(at + 3).map(|token| token.text) == Some("(")
    {
        return matching_close.get(at + 3).copied().flatten();
    }
    None
}

fn is_symbol_binding(tokens: &[Token<'_>], at: usize) -> Option<usize> {
    let start = if matches!(tokens.get(at).map(|token| token.text), Some("var" | "let" | "const")) {
        at + 1
    } else {
        at
    };
    if tokens
        .get(start)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
        && tokens.get(start + 1).map(|token| token.text) == Some("=")
        && tokens.get(start + 2).map(|token| token.text) == Some("Symbol")
        && tokens.get(start + 3).map(|token| token.text) == Some(".")
        && tokens
            .get(start + 4)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
    {
        return Some(start + 4);
    }
    None
}

fn reaching_constant_key_expr(
    tokens: &[Token<'_>],
    name: &str,
    before: usize,
) -> Option<String> {
    let mut last = None;
    let mut index = 0usize;
    while index + 2 < before && index + 2 < tokens.len() {
        let name_at = if tokens[index].kind == TokenKind::Identifier
            && tokens[index].text == name
            && tokens.get(index + 1).map(|token| token.text) == Some("=")
            && !is_property_identifier(tokens, index)
        {
            index
        } else if matches!(tokens[index].text, "var" | "let" | "const")
            && tokens.get(index + 1).map(|token| token.text) == Some(name)
            && tokens.get(index + 2).map(|token| token.text) == Some("=")
        {
            index + 1
        } else {
            index += 1;
            continue;
        };
        let value_at = name_at + 2;
        if tokens.get(value_at).map(|token| token.text) == Some("Symbol")
            && tokens.get(value_at + 1).map(|token| token.text) == Some(".")
            && tokens
                .get(value_at + 2)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
        {
            last = Some(format!("Symbol.{}", tokens[value_at + 2].text));
        } else if tokens
            .get(value_at)
            .is_some_and(|token| token.kind == TokenKind::String)
        {
            last = Some(tokens[value_at].text.to_string());
        } else {
            last = None;
        }
        index = name_at + 1;
    }
    last
}

fn identifier_assign_count(tokens: &[Token<'_>], name: &str, before: usize) -> usize {
    let mut count = 0usize;
    let mut index = 0usize;
    while index + 1 < before && index + 1 < tokens.len() {
        if tokens[index].kind == TokenKind::Identifier
            && tokens[index].text == name
            && tokens.get(index + 1).map(|token| token.text) == Some("=")
            && !is_property_identifier(tokens, index)
        {
            count += 1;
        }
        index += 1;
    }
    count
}

fn computed_key_name(
    tokens: &[Token<'_>],
    key_token: &Token<'_>,
    key_at: usize,
    class_close: usize,
) -> Option<(String, bool)> {
    if let Some(name) = ascii_identifier_name_string(key_token.text) {
        return Some((name.to_string(), false));
    }
    if key_token.kind != TokenKind::Identifier {
        return None;
    }
    if let Some(expr) = reaching_constant_key_expr(tokens, key_token.text, key_at) {
        return Some((expr, true));
    }
    if identifier_assign_count(tokens, key_token.text, key_at) <= 1
        && identifier_bound_before(tokens, key_token.text, class_close)
    {
        return Some((key_token.text.to_string(), true));
    }
    None
}

fn skip_non_ctor_declaration(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    at: usize,
) -> Option<usize> {
    if !matches!(tokens.get(at).map(|token| token.text), Some("var" | "let" | "const")) {
        return None;
    }
    let mut cursor = at + 1;
    loop {
        if !tokens
            .get(cursor)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
        {
            return None;
        }
        if tokens.get(cursor + 1).map(|token| token.text) == Some("=") {
            let value_at = cursor + 2;
            if is_new_function_or_class_value(tokens, matching_close, value_at) {
                return None;
            }
            let stop = top_level_stop(tokens, value_at, &[",", ";"])?;
            if tokens[stop].text == ";" {
                return Some(stop);
            }
            cursor = stop + 1;
            continue;
        }
        if tokens.get(cursor + 1).map(|token| token.text) == Some(",") {
            cursor += 2;
            continue;
        }
        if tokens.get(cursor + 1).map(|token| token.text) == Some(";") {
            return Some(cursor + 1);
        }
        return None;
    }
}

fn is_new_function_or_class_value(tokens: &[Token<'_>], matching_close: &[Option<usize>], at: usize) -> bool {
    if matches!(tokens.get(at).map(|token| token.text), Some("class" | "function")) {
        return true;
    }
    skip_group_zero_function(tokens, at).is_some()
        || parse_function_expression(tokens, matching_close, at).is_some()
}

fn absorb_prototype_members_into_classes(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let classes = class_spans(&tokens, &matching_close);
    if classes.is_empty() {
        return Ok((source.to_string(), 0));
    }
    let proto_aliases = prototype_alias_assignments(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut class_inserts = std::collections::HashMap::<usize, String>::new();
    for (class_name, class_close) in &classes {
        let mut scan = class_close + 1;
        while scan < tokens.len() {
            scan = skip_separators(&tokens, scan);
            if scan >= tokens.len() {
                break;
            }
            if matches!(tokens[scan].text, "class" | "function") {
                break;
            }
            if let Some((_, last)) = match_proto_assign(&tokens, scan, class_name) {
                scan = last + 1;
                continue;
            }
            if let Some(last) = is_symbol_binding(&tokens, scan) {
                scan = last + 1;
                continue;
            }
            if let Some((name, value_at)) = assignment_name_and_value(&tokens, scan) {
                if is_new_function_or_class_value(&tokens, &matching_close, value_at) {
                    break;
                }
                if proto_aliases
                    .iter()
                    .any(|(at, alias, owner)| *at < scan && *alias == name && *owner == *class_name)
                    && tokens.get(value_at).map(|token| token.text) != Some(*class_name)
                {
                    break;
                }
            }
            if let Some(close) = skip_call_statement(&tokens, &matching_close, scan) {
                scan = close + 1;
                continue;
            }
            let deleted_start = |start: usize| {
                if start > 0 && matches!(tokens[start - 1].text, ";" | ",") {
                    tokens[start - 1].start
                } else {
                    tokens[start].start
                }
            };
            if tokens
                .get(scan)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
                && tokens.get(scan + 1).map(|token| token.text) == Some(".")
                && tokens.get(scan + 2).is_some_and(is_method_name)
                && tokens.get(scan + 3).map(|token| token.text) == Some("=")
            {
                let alias = tokens[scan].text;
                let method_name = tokens[scan + 2].text;
                if method_name == "constructor" {
                    if let Some((_, _, end)) =
                        take_class_method_value(source, &tokens, &matching_close, scan + 4)
                    {
                        replacements.push((deleted_start(scan), tokens[end].end, String::new()));
                        scan = end + 1;
                        continue;
                    }
                    if tokens
                        .get(scan + 4)
                        .is_some_and(|token| token.kind == TokenKind::Identifier)
                        && tokens[scan + 4].text == *class_name
                    {
                        replacements.push((
                            deleted_start(scan),
                            tokens[scan + 4].end,
                            String::new(),
                        ));
                        scan += 5;
                        continue;
                    }
                    break;
                }
                let Some(owner) = class_for_proto_alias(&proto_aliases, alias, scan) else {
                    break;
                };
                if owner != *class_name {
                    break;
                }
                let Some((params, body, method_end)) =
                    take_class_method_value(source, &tokens, &matching_close, scan + 4)
                else {
                    if let Some(stop) = top_level_stop(&tokens, scan + 4, &[",", ";"]) {
                        scan = stop + 1;
                        continue;
                    }
                    break;
                };
                class_inserts.entry(*class_close).or_default().push_str(&emit_class_method(
                    &Method {
                        name: method_name.to_string(),
                        params,
                        body,
                        computed: false,
                    },
                ));
                replacements.push((deleted_start(scan), tokens[method_end].end, String::new()));
                scan = method_end + 1;
                continue;
            }
            if tokens.get(scan).map(|token| token.text) == Some(*class_name)
                && tokens.get(scan + 1).map(|token| token.text) == Some(".")
                && tokens.get(scan + 2).map(|token| token.text) == Some("prototype")
                && tokens.get(scan + 3).map(|token| token.text) == Some(".")
                && tokens.get(scan + 4).is_some_and(is_method_name)
                && tokens.get(scan + 5).map(|token| token.text) == Some("=")
            {
                let method_name = tokens[scan + 4].text;
                if method_name == "constructor" {
                    replacements.push((deleted_start(scan), tokens[scan + 6].end, String::new()));
                    scan += 7;
                    continue;
                }
                let Some((params, body, method_end)) =
                    take_class_method_value(source, &tokens, &matching_close, scan + 6)
                else {
                    break;
                };
                class_inserts.entry(*class_close).or_default().push_str(&emit_class_method(
                    &Method {
                        name: method_name.to_string(),
                        params,
                        body,
                        computed: false,
                    },
                ));
                replacements.push((deleted_start(scan), tokens[method_end].end, String::new()));
                scan = method_end + 1;
                continue;
            }
            if tokens
                .get(scan)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
                && tokens.get(scan + 1).map(|token| token.text) == Some("[")
            {
                let Some(close) = matching_close.get(scan + 1).copied().flatten() else {
                    break;
                };
                if tokens.get(close + 1).map(|token| token.text) != Some("=") {
                    break;
                }
                let alias = tokens[scan].text;
                let Some(owner) = class_for_proto_alias(&proto_aliases, alias, scan) else {
                    break;
                };
                if owner != *class_name {
                    break;
                }
                let key_tokens = &tokens[scan + 2..close];
                let ident = key_tokens
                    .first()
                    .and_then(|token| ascii_identifier_name_string(token.text));
                let (name, computed) = if let Some(ident) =
                    ident.filter(|name| key_tokens.len() == 1 && !is_reserved_method_name(name))
                {
                    (ident.to_string(), false)
                } else if key_tokens.len() == 1 {
                    let Some((name, computed)) =
                        computed_key_name(&tokens, &key_tokens[0], scan + 2, *class_close)
                    else {
                        if let Some((_, _, method_end)) =
                            take_class_method_value(source, &tokens, &matching_close, close + 2)
                        {
                            scan = method_end + 1;
                            continue;
                        }
                        break;
                    };
                    (name, computed)
                } else {
                    if let Some((_, _, method_end)) =
                        take_class_method_value(source, &tokens, &matching_close, close + 2)
                    {
                        scan = method_end + 1;
                        continue;
                    }
                    break;
                };
                let Some((params, body, method_end)) =
                    take_class_method_value(source, &tokens, &matching_close, close + 2)
                else {
                    break;
                };
                class_inserts.entry(*class_close).or_default().push_str(&emit_class_method(
                    &Method {
                        name,
                        params,
                        body,
                        computed,
                    },
                ));
                replacements.push((deleted_start(scan), tokens[method_end].end, String::new()));
                scan = method_end + 1;
                continue;
            }
            if let Some(last) = skip_non_ctor_declaration(&tokens, &matching_close, scan) {
                scan = last + 1;
                continue;
            }
            if matches!(tokens[scan].text, "var" | "let" | "const") {
                break;
            }
            break;
        }
    }
    for (close, insert) in class_inserts {
        replacements.push((tokens[close].start, tokens[close].start, insert));
    }
    Ok(apply_token_rewrites(source, replacements))
}

pub(crate) fn fold_undefined_defaults_into_formals(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor < tokens.len() {
        let constructor = tokens[cursor].text == "constructor"
            && tokens.get(cursor + 1).map(|token| token.text) == Some("(");
        let function_at = if constructor {
            None
        } else if tokens[cursor].text == "function" {
            Some(cursor)
        } else {
            None
        };
        let method = !constructor
            && function_at.is_none()
            && is_class_method_head(&tokens, &matching_close, cursor);
        let (params_open, block_open, block_close, params_from, params_to) = if constructor
            || method
        {
            let open = cursor + 1;
            let Some(close) = matching_close.get(open).copied().flatten() else {
                cursor += 1;
                continue;
            };
            if tokens.get(close + 1).map(|token| token.text) != Some("{") {
                cursor += 1;
                continue;
            }
            let Some(end) = matching_close.get(close + 1).copied().flatten() else {
                cursor += 1;
                continue;
            };
            (open, close + 1, end, open + 1, close)
        } else if let Some(at) = function_at {
            let Some(function) = parse_function_expression(&tokens, &matching_close, at) else {
                cursor += 1;
                continue;
            };
            if function.is_arrow {
                cursor += 1;
                continue;
            }
            let Some(open) = function.block_open else {
                cursor += 1;
                continue;
            };
            (
                at,
                open,
                function.end,
                function.params_from,
                function.params_to,
            )
        } else {
            cursor += 1;
            continue;
        };
        if simple_formals(&tokens, params_from, params_to).is_none() {
            cursor = block_close + 1;
            continue;
        }
        let old_params = parse_params(source, &tokens, params_from, params_to);
        let old_body = source[tokens[block_open + 1].start..tokens[block_close].start].to_string();
        let (params, body) = if method {
            match simple_formals(&tokens, params_from, params_to) {
                Some(existing) => recover_default_params(&old_params, &old_body, &existing),
                None => (old_params.clone(), old_body.clone()),
            }
        } else {
            recover_constructor_formals(
                source,
                &tokens,
                &matching_close,
                params_from,
                params_to,
                block_open,
                block_close,
                &old_params,
                &old_body,
            )
        };
        if params == old_params && body == old_body {
            cursor = block_close + 1;
            continue;
        }
        if constructor {
            replacements.push((
                tokens[params_open].start,
                tokens[block_close].end,
                format!("({params}){{{body}}}"),
            ));
        } else if method {
            replacements.push((
                tokens[params_open].start,
                tokens[block_close].end,
                format!("({params}){{{body}}}"),
            ));
        } else {
            let name = if tokens
                .get(cursor + 1)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
                && tokens.get(cursor + 2).map(|token| token.text) == Some("(")
            {
                tokens[cursor + 1].text
            } else {
                ""
            };
            replacements.push((
                tokens[cursor].start,
                tokens[block_close].end,
                if name.is_empty() {
                    format!("function({params}){{{body}}}")
                } else {
                    format!("function {name}({params}){{{body}}}")
                },
            ));
        }
        cursor = block_close + 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

pub(crate) fn fold_arguments_length_formal_copies(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor < tokens.len() {
        let (params_from, params_to, block_open, block_close) = if tokens[cursor].text
            == "function"
        {
            let Some(function) = parse_function_expression(&tokens, &matching_close, cursor) else {
                cursor += 1;
                continue;
            };
            let Some(open) = function.block_open else {
                cursor += 1;
                continue;
            };
            let Some(close) = matching_close.get(open).copied().flatten() else {
                cursor += 1;
                continue;
            };
            (function.params_from, function.params_to, open, close)
        } else if (tokens[cursor].text == "constructor"
            || is_class_method_head(&tokens, &matching_close, cursor))
            && tokens.get(cursor + 1).map(|token| token.text) == Some("(")
        {
            let Some(close) = matching_close.get(cursor + 1).copied().flatten() else {
                cursor += 1;
                continue;
            };
            if tokens.get(close + 1).map(|token| token.text) != Some("{") {
                cursor += 1;
                continue;
            }
            let Some(end) = matching_close.get(close + 1).copied().flatten() else {
                cursor += 1;
                continue;
            };
            (cursor + 2, close, close + 1, end)
        } else {
            cursor += 1;
            continue;
        };
        let Some(formals) = formals_allowing_defaults(&tokens, params_from, params_to) else {
            cursor += 1;
            continue;
        };
        let mut scan = block_open + 1;
        while scan + 8 < block_close {
            if let Some(nested) = nested_function_end(&tokens, &matching_close, scan) {
                scan = nested + 1;
                continue;
            }
            if tokens[scan].text != "arguments"
                || tokens.get(scan + 1).map(|token| token.text) != Some(".")
                || tokens.get(scan + 2).map(|token| token.text) != Some("length")
                || tokens.get(scan + 3).map(|token| token.text) != Some(">")
                || !tokens
                    .get(scan + 4)
                    .is_some_and(|token| token.kind == TokenKind::Number)
                || tokens.get(scan + 5).map(|token| token.text) != Some("&&")
                || tokens.get(scan + 6).map(|token| token.text) != Some("(")
            {
                scan += 1;
                continue;
            }
            let Some(close) = matching_close.get(scan + 6).copied().flatten() else {
                scan += 1;
                continue;
            };
            let Some(index) = tokens[scan + 4].text.parse::<usize>().ok() else {
                scan += 1;
                continue;
            };
            let Some(formal) = formals.get(index).copied() else {
                scan += 1;
                continue;
            };
            if tokens
                .get(scan + 7)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
                || tokens.get(scan + 8).map(|token| token.text) != Some("=")
            {
                scan += 1;
                continue;
            }
            let temp = tokens[scan + 7].text;
            let rhs_ok = if tokens.get(scan + 9).map(|token| token.text) == Some(formal)
                && scan + 10 == close
            {
                true
            } else if tokens.get(scan + 9).map(|token| token.text) == Some("!!")
                || (tokens.get(scan + 9).map(|token| token.text) == Some("!")
                    && tokens.get(scan + 10).map(|token| token.text) == Some("!"))
            {
                false
            } else {
                false
            };
            let (rhs_start, bangs) = if tokens.get(scan + 9).map(|token| token.text) == Some("!")
                && tokens.get(scan + 10).map(|token| token.text) == Some("!")
                && tokens.get(scan + 11).map(|token| token.text) == Some(formal)
                && scan + 12 == close
            {
                (scan + 9, true)
            } else if rhs_ok {
                (scan + 9, false)
            } else {
                scan += 1;
                continue;
            };
            let _ = (rhs_start, bangs);
            let rewritten = if tokens.get(scan + 9).map(|token| token.text) == Some("!") {
                format!("{temp}=!!{formal}")
            } else {
                format!("{temp}={formal}")
            };
            replacements.push((tokens[scan].start, tokens[close].end, rewritten));
            scan = close + 1;
        }
        cursor += 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn array_prototype_slice_aliases<'a>(tokens: &'a [Token<'a>]) -> std::collections::HashSet<&'a str> {
    let mut names = std::collections::HashSet::new();
    names.insert("Array.prototype.slice");
    let mut index = 0usize;
    while index + 6 < tokens.len() {
        if let Some((name, value_at)) = assignment_name_and_value(tokens, index) {
            if tokens.get(value_at).map(|token| token.text) == Some("Array")
                && tokens.get(value_at + 1).map(|token| token.text) == Some(".")
                && tokens.get(value_at + 2).map(|token| token.text) == Some("prototype")
                && tokens.get(value_at + 3).map(|token| token.text) == Some(".")
                && tokens.get(value_at + 4).map(|token| token.text) == Some("slice")
            {
                names.insert(name);
            }
        }
        index += 1;
    }
    names
}

fn slice_arguments_call_end(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    aliases: &std::collections::HashSet<&str>,
    at: usize,
    formal_count: usize,
) -> Option<usize> {
    let open = if aliases.contains(tokens.get(at).map(|token| token.text).unwrap_or(""))
        && tokens.get(at + 1).map(|token| token.text) == Some(".")
        && tokens.get(at + 2).map(|token| token.text) == Some("call")
        && tokens.get(at + 3).map(|token| token.text) == Some("(")
    {
        at + 3
    } else if tokens.get(at).map(|token| token.text) == Some("Array")
        && tokens.get(at + 1).map(|token| token.text) == Some(".")
        && tokens.get(at + 2).map(|token| token.text) == Some("prototype")
        && tokens.get(at + 3).map(|token| token.text) == Some(".")
        && tokens.get(at + 4).map(|token| token.text) == Some("slice")
        && tokens.get(at + 5).map(|token| token.text) == Some(".")
        && tokens.get(at + 6).map(|token| token.text) == Some("call")
        && tokens.get(at + 7).map(|token| token.text) == Some("(")
    {
        at + 7
    } else {
        return None;
    };
    let close = matching_close.get(open).copied().flatten()?;
    if tokens.get(open + 1).map(|token| token.text) != Some("arguments")
        || tokens.get(open + 2).map(|token| token.text) != Some(",")
        || tokens
            .get(open + 3)
            .and_then(|token| token.text.parse::<usize>().ok())
            != Some(formal_count)
    {
        return None;
    }
    if close == open + 4 {
        return Some(close);
    }
    if tokens.get(open + 4).map(|token| token.text) == Some(",")
        && tokens.get(open + 5).map(|token| token.text) == Some("arguments")
        && tokens.get(open + 6).map(|token| token.text) == Some(".")
        && tokens.get(open + 7).map(|token| token.text) == Some("length")
    {
        if close == open + 8 {
            return Some(close);
        }
        if tokens.get(open + 8).map(|token| token.text) == Some("|")
            && tokens.get(open + 9).map(|token| token.text) == Some("0")
            && close == open + 10
        {
            return Some(close);
        }
    }
    None
}

pub(crate) fn fold_arguments_slice_to_rest(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let aliases = array_prototype_slice_aliases(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor < tokens.len() {
        let (params_from, params_to, params_open, block_open, block_close) =
            if tokens[cursor].text == "function" {
                let Some(function) = parse_function_expression(&tokens, &matching_close, cursor)
                else {
                    cursor += 1;
                    continue;
                };
                if function.is_arrow {
                    cursor += 1;
                    continue;
                }
                let Some(open) = function.block_open else {
                    cursor += 1;
                    continue;
                };
                (
                    function.params_from,
                    function.params_to,
                    if tokens.get(cursor + 1).map(|token| token.text) == Some("(") {
                        cursor + 1
                    } else {
                        cursor + 2
                    },
                    open,
                    function.end,
                )
            } else if (tokens[cursor].text == "constructor"
                && tokens.get(cursor + 1).map(|token| token.text) == Some("("))
                || is_class_method_head(&tokens, &matching_close, cursor)
            {
                let open = cursor + 1;
                let Some(close) = matching_close.get(open).copied().flatten() else {
                    cursor += 1;
                    continue;
                };
                if tokens.get(close + 1).map(|token| token.text) != Some("{") {
                    cursor += 1;
                    continue;
                }
                let Some(end) = matching_close.get(close + 1).copied().flatten() else {
                    cursor += 1;
                    continue;
                };
                (open + 1, close, open, close + 1, end)
            } else {
                cursor += 1;
                continue;
            };
        let Some(formals) = simple_formals(&tokens, params_from, params_to) else {
            cursor = block_close + 1;
            continue;
        };
        if formals.iter().any(|name| name.starts_with("...")) {
            cursor = block_close + 1;
            continue;
        }
        let formal_count = formals.len();
        let mut calls = Vec::new();
        let mut index = block_open + 1;
        while index < block_close {
            if let Some(nested) = nested_function_end(&tokens, &matching_close, index) {
                index = nested + 1;
                continue;
            }
            if let Some(close) =
                slice_arguments_call_end(&tokens, &matching_close, &aliases, index, formal_count)
            {
                calls.push((index, close));
                index = close + 1;
                continue;
            }
            index += 1;
        }
        if calls.is_empty() {
            cursor = block_close + 1;
            continue;
        }
        let used: Vec<&str> = formals.iter().copied().collect();
        let rest = next_formal_name(&used, used.len());
        let old_params = parse_params(source, &tokens, params_from, params_to);
        let params = if old_params.is_empty() {
            format!("...{rest}")
        } else {
            format!("{old_params},...{rest}")
        };
        let body_start = tokens[block_open + 1].start;
        let mut body = source[body_start..tokens[block_close].start].to_string();
        for (start, close) in calls.iter().rev() {
            let from = tokens[*start].start - body_start;
            let to = tokens[*close].end - body_start;
            body.replace_range(from..to, &rest);
        }
        replacements.push((
            tokens[params_open].start,
            tokens[block_close].end,
            format!("({params}){{{body}}}"),
        ));
        cursor = block_close + 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

pub(crate) fn fold_indexed_arguments_to_formals(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor < tokens.len() {
        let (head_start, params_from, params_to, block_open, block_close, named) =
            if tokens[cursor].text == "function" {
                let Some(function) = parse_function_expression(&tokens, &matching_close, cursor)
                else {
                    cursor += 1;
                    continue;
                };
                let Some(block_open) = function.block_open else {
                    cursor += 1;
                    continue;
                };
                if function.is_arrow {
                    cursor += 1;
                    continue;
                }
                let named = if function.named
                    && tokens
                        .get(cursor + 1)
                        .is_some_and(|token| token.kind == TokenKind::Identifier)
                {
                    Some(tokens[cursor + 1].text)
                } else {
                    None
                };
                (
                    tokens[cursor].start,
                    function.params_from,
                    function.params_to,
                    block_open,
                    function.end,
                    named,
                )
            } else if (tokens[cursor].text == "constructor"
                || is_class_method_head(&tokens, &matching_close, cursor))
                && tokens.get(cursor + 1).map(|token| token.text) == Some("(")
            {
                let Some(close) = matching_close.get(cursor + 1).copied().flatten() else {
                    cursor += 1;
                    continue;
                };
                if tokens.get(close + 1).map(|token| token.text) != Some("{") {
                    cursor += 1;
                    continue;
                }
                let Some(end) = matching_close.get(close + 1).copied().flatten() else {
                    cursor += 1;
                    continue;
                };
                (
                    tokens[cursor].start,
                    cursor + 2,
                    close,
                    close + 1,
                    end,
                    Some(tokens[cursor].text),
                )
            } else {
                cursor += 1;
                continue;
            };
        if arguments_used_as_value(&tokens, &matching_close, block_open + 1, block_close) {
            cursor += 1;
            continue;
        }
        let Some(max_index) =
            max_arguments_index(&tokens, &matching_close, block_open + 1, block_close)
        else {
            cursor += 1;
            continue;
        };
        let Some(existing) = formals_allowing_defaults(&tokens, params_from, params_to) else {
            cursor += 1;
            continue;
        };
        let mut used = existing.clone();
        for token in &tokens[block_open + 1..block_close] {
            if token.kind == TokenKind::Identifier && !used.contains(&token.text) {
                used.push(token.text);
            }
        }
        let mut formals = existing.iter().map(|name| (*name).to_string()).collect::<Vec<_>>();
        while formals.len() <= max_index {
            let mut seen = used.clone();
            seen.extend(formals.iter().map(String::as_str));
            formals.push(next_formal_name(&seen, formals.len()));
        }
        let rewritten = formals.iter().enumerate().any(|(index, formal)| {
            existing.get(index).copied() != Some(formal.as_str())
                || source[tokens[block_open + 1].start..tokens[block_close].start]
                    .contains(&format!("arguments[{index}]"))
        });
        if !rewritten {
            cursor += 1;
            continue;
        }
        let mut body = source[tokens[block_open + 1].start..tokens[block_close].start].to_string();
        for (index, formal) in formals.iter().enumerate() {
            body = body.replace(&format!("arguments[{index}]"), formal);
        }
        let formal_refs = formals.iter().map(String::as_str).collect::<Vec<_>>();
        let (params, body) = recover_default_params(&formals.join(","), &body, &formal_refs);
        let replacement = match named {
            Some(name) if tokens[cursor].text == "function" => {
                format!("function {name}({params}){{{body}}}")
            }
            Some(name) => format!("{name}({params}){{{body}}}"),
            None => format!("function({params}){{{body}}}"),
        };
        replacements.push((head_start, tokens[block_close].end, replacement));
        cursor = block_close + 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

#[cfg(test)]
mod tests {
    use super::{
        fold_constructor_prototype_tables_to_classes, fold_grouped_zero_function_expressions,
        fold_indexed_arguments_to_formals,
    };

    #[test]
    fn unwraps_grouped_zero_function_values_but_keeps_iifes() {
        let source = r#"var i=(0,function(){return 1}),j=(0,function(a){return a});(0,function(){return 2})();k=(0,function(){return this})"#;
        let (out, count) = fold_grouped_zero_function_expressions(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("var i=function(){return 1}"), "{out}");
        assert!(out.contains("j=function(a){return a}"), "{out}");
        assert!(out.contains("k=function(){return this}"), "{out}");
        assert!(out.contains("(0,function(){return 2})()"), "{out}");
    }

    #[test]
    fn leftover_proto_alias_is_semicolon_terminated() {
        let source = r#"var R=(0,function(c){this.a=c;return this});P=R.prototype,P.m=function(){return this.a};var gb=(0,function(){return 1})"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(
            out.contains(".prototype;") || out.contains(".prototype;var") || !out.contains("prototypevar"),
            "{out}"
        );
        assert!(!out.contains("prototypevar"), "{out}");
    }

    #[test]
    fn leftover_proto_after_predicate_does_not_glue_onto_var() {
        let source = r#"var M=(0,function(n){this.b=n;this.s=0;return this});a=M.prototype,a.toString=function(){return this.b};da("Atom",M),a=M.prototype;var gb=(0,function(){return 1})"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(!out.contains("prototypevar"), "{out}");
        assert!(!out.contains("prototypea="), "{out}");
    }

    #[test]
    fn flag_installers_keep_nested_setter_params_and_stay_on_owner_class() {
        let source = r#"function C(e,r,a){return Object.defineProperty(e,r,a)}function K(a,b,c){C(a,b,{configurable:!0,get:function(){return 0!=(this.s&c)},set:function(b){var d=+this.s|0;b?this.s=d|c:this.s=d&(c^-1)}})}var M=(0,function(){this.s=0;return this});a=M.prototype,a.m=function(){return this.s};da("Atom",M),a=M.prototype,K(a,"isBeingObserved",1);var D=(0,function(){this.s=0;return this});a=D.prototype,a.get=function(){return this.s};K(a,"isComputing",1)"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("class M{"), "{out}");
        assert!(out.contains("class D{"), "{out}");
        let atom = out.split("class D{").next().unwrap_or(&out);
        let computed = out.split("class D{").nth(1).unwrap_or("");
        assert!(atom.contains("get isBeingObserved("), "{out}");
        assert!(atom.contains("set isBeingObserved("), "{out}");
        assert!(!atom.contains("set isBeingObserved(\"isBeingObserved\")"), "{out}");
        assert!(computed.contains("get isComputing("), "{out}");
        assert!(!computed.contains("get isBeingObserved("), "{out}");
        assert!(!out.contains("prototypevar"), "{out}");
    }

    #[test]
    fn infers_extends_from_parent_call_this() {
        let source = r#"var M=(0,function(n){this.b=n;return this});a=M.prototype,a.toString=function(){return this.b};var x=(0,function(v,n){M.call(this,n),this.g=v;return this});k=x.prototype;Object.setPrototypeOf(k,l);k=x.prototype,k.get=function(){return this.g}"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("class x extends M{") || out.contains("x=class extends M{"), "{out}");
        assert!(out.contains("super("), "{out}");
        assert!(!out.contains("setPrototypeOf"), "{out}");
        assert!(out.contains("get(){return this.g}"), "{out}");
    }

    #[test]
    fn strips_set_prototype_of_whose_parent_is_rebound_later() {
        let source = r#"var M=(0,function(n){this.b=n;return this});a=M.prototype,a.toString=function(){return this.b};var x=(0,function(v,n){M.call(this,n),this.g=v;return this});k=x.prototype,k.get=function(){return this.g};Object.setPrototypeOf(k,l);var l;l=Symbol.toPrimitive"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(!out.contains("setPrototypeOf"), "{out}");
    }

    #[test]
    fn fuses_set_prototype_of_on_class_prototype() {
        let source = r#"var M=(0,function(n){this.b=n;return this});a=M.prototype,a.toString=function(){return this.b};var x=(0,function(v,n){M.call(this,n),this.g=v;return this});Object.setPrototypeOf(x.prototype,M.prototype);x.prototype.get=function(){return this.g}"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("class x extends M{") || out.contains("x=class extends M{"), "{out}");
        assert!(out.contains("get(){return this.g}"), "{out}");
        assert!(!out.contains("setPrototypeOf"), "{out}");
    }

    #[test]
    fn fuses_set_prototype_of_via_object_alias() {
        let source = r#"var f=Object;var M=(0,function(n){this.b=n;return this});a=M.prototype,a.toString=function(){return this.b};var x=(0,function(v,n){M.call(this,n),this.g=v;return this});k=x.prototype;f.setPrototypeOf(k,M.prototype);k.constructor=x;k.get=function(){return this.g}"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("class x extends M{") || out.contains("x=class extends M{"), "{out}");
        assert!(out.contains("get(){return this.g}"), "{out}");
        assert!(!out.contains("setPrototypeOf"), "{out}");
    }

    #[test]
    fn fuses_set_prototype_of_via_helper_wrapper() {
        let source = r#"function N(a,b){return Object.setPrototypeOf(a,b)}var M=(0,function(n){this.b=n;return this});a=M.prototype,a.toString=function(){return this.b};var x=(0,function(v,n){M.call(this,n),this.g=v;return this});k=x.prototype;N(k,M.prototype);k.constructor=x;k.get=function(){return this.g}"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("class x extends M{") || out.contains("x=class extends M{"), "{out}");
        assert!(out.contains("get(){return this.g}"), "{out}");
        assert!(!out.contains("N(k"), "{out}");
    }

    #[test]
    fn skips_constructor_restore_on_class_prototype() {
        let source = r#"var M=(0,function(n){this.b=n;return this});a=M.prototype,a.toString=function(){return this.b};var x=(0,function(v,n){M.call(this,n),this.g=v;return this});Object.setPrototypeOf(x.prototype,M.prototype);x.prototype.constructor=x;x.prototype.get=function(){return this.g}"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("class x extends M{") || out.contains("x=class extends M{"), "{out}");
        assert!(out.contains("get(){return this.g}"), "{out}");
        assert!(!out.contains("setPrototypeOf"), "{out}");
        assert!(!out.contains("prototype.constructor"), "{out}");
    }

    #[test]
    fn full_pipeline_strips_rebound_helper_set_prototype_of() {
        let source = r#"function N(a,b){return Object.setPrototypeOf(a,b)}var M=(0,function(n){this.b=n;return this});a=M.prototype,a.toString=function(){return this.b};var x=(0,function(v,n){M.call(this,n),this.g=v;return this});k=x.prototype;N(k,l);k=x.prototype;k.get=function(){return this.g};var l;l=Symbol.toPrimitive"#;
        let optimized = crate::js_peephole::optimize_generated_javascript(source).unwrap();
        assert!(
            optimized.code.contains("class x extends M{")
                || optimized.code.contains("x=class extends M{"),
            "{}",
            optimized.code
        );
        assert!(optimized.code.contains("get(){return this.g}"), "{}", optimized.code);
        assert!(!optimized.code.contains("setPrototypeOf"), "{}", optimized.code);
    }

    #[test]
    fn recovers_constructor_default_from_length_guard() {
        let source = r#"var R=(0,function(c){var a=(+arguments.length)>0&&c!==void 0?c+"":"Atom";this.a=a;return this});P=R.prototype,P.m=function(){return this.a}"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("constructor(c=\"Atom\")") || out.contains("constructor(c=\"Atom\"){"), "{out}");
        assert!(!out.contains("arguments.length"), "{out}");
        assert!(out.contains("this.a=a") || out.contains("this.a=c"), "{out}");
    }

    #[test]
    fn recovers_constructor_default_from_int32_length_guard() {
        let source = r#"var R=(0,function(q,k,B,R){var _=(arguments.length|0)>0&&q!==void 0?q+"":`Reaction`;this.n=_;return this});P=R.prototype,P.m=function(){return this.n}"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("constructor(q=`Reaction`") || out.contains("constructor(q=\"Reaction\""), "{out}");
        assert!(!out.contains("arguments.length"), "{out}");
    }

    #[test]
    fn fuses_parenthesized_assign_return_this() {
        let source = r#"var $=(0,function(){return(Object.assign(this,{message:`FLOW_CANCELLED`,name:`FlowCancellationError`}),this)});var as=$.prototype;as.toString=function(){return this.message}"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("class $") || out.contains("$=class"), "{out}");
        assert!(out.contains("toString"), "{out}");
        assert!(!out.contains("(0,function"), "{out}");
    }

    #[test]
    fn fuses_method_table_into_class() {
        let source = "var j=(0,function(s){this.t=s;return this});Ei=j.prototype,Ei.onBO=function(){Q(this)},Ei=j.prototype,Ei.reportChanged=function(){M(this)};ie(j)";
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("class j{"), "{out}");
        assert!(out.contains("constructor(s){this.t=s}"), "{out}");
        assert!(out.contains("onBO(){Q(this)}"), "{out}");
        assert!(out.contains("reportChanged(){M(this)}"), "{out}");
        assert!(!out.contains("(0,function"), "{out}");
    }

    #[test]
    fn fuses_subclass_set_prototype_of() {
        let source = "var w=(0,function(a,n){j.call(this,n),this.a=a;return this});Ei=w.prototype;var Ui=j.prototype;Object.setPrototypeOf(Ei,Ui),Ei=w.prototype,Ei.constructor=w,Ei=w.prototype,Ei.get=function(){return this.a}";
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("class w extends j{"), "{out}");
        assert!(out.contains("super(n)"), "{out}");
        assert!(out.contains("get(){return this.a}"), "{out}");
    }

    #[test]
    fn fuses_returned_this_alias() {
        let source = "var d=(0,function(A){var t=this;t.k=A;return t});a=d.prototype,a.has=function(){return 1}";
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("class d{"), "{out}");
        assert!(out.contains("constructor(A){var t=this;t.k=A}"), "{out}");
        assert!(out.contains("has(){return 1}"), "{out}");
        assert!(!out.contains("return t"), "{out}");
    }

    #[test]
    fn skips_empty_return_this_without_members() {
        let source = "var st=(0,function(){return this});var de=(0,function(){})";
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert_eq!(count, 0, "{out}");
        assert!(!out.contains("class"), "{out}");
    }

    #[test]
    fn fuses_comma_return_this_without_methods() {
        let source = "var ke=(0,function(t){return this.cause=t,this});ke.prototype.x=1";
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("class ke{"), "{out}");
        assert!(out.contains("constructor(t){this.cause=t}"), "{out}");
    }

    #[test]
    fn fuses_quoted_prototype_methods() {
        let source = "var C=(0,function(){return this});P=C.prototype,P[\"get\"]=function(){return 1}";
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("get(){return 1}"), "{out}");
    }

    #[test]
    fn fuses_define_property_accessors() {
        let source = "var C=(0,function(){this.f=1;return this});P=C.prototype,P.m=function(){return this.f};Object.defineProperty(C.prototype,\"on\",{configurable:!0,get:function(){return this.f},set:function(v){this.f=v}})";
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("get on(){return this.f}"), "{out}");
        assert!(out.contains("set on(v){this.f=v}"), "{out}");
        assert!(!out.contains("defineProperty"), "{out}");
    }

    #[test]
    fn fuses_define_property_via_alias_and_proto_temp() {
        let source = "function d(a,b,c){return Object.defineProperty(a,b,c)}var C=(0,function(){this.f=1;return this});P=C.prototype;P.m=function(){return this.f};d(P,\"on\",{configurable:!0,get:function(){return this.f},set:function(v){this.f=v}})";
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("get on(){return this.f}"), "{out}");
        assert!(out.contains("set on(v){this.f=v}"), "{out}");
        assert!(!out.contains("d(P,"), "{out}");
    }

    #[test]
    fn inlines_flag_accessor_installers_into_class_getters() {
        let source = "function C(e,r,a){return Object.defineProperty(e,r,a)}function K(e,c,a){C(e,c,{configurable:!0,get:function(){return 0!=(this.y&a)},set:function(i){i?this.y|=a:this.y&=a^-1}})}var D=(0,function(){this.y=0;return this});P=D.prototype;P.m=function(){return this.y};K(D.prototype,\"on\",1)";
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("get on(){"), "{out}");
        assert!(out.contains("set on(i){"), "{out}");
        assert!(!out.contains("K(D.prototype"), "{out}");
    }

    #[test]
    fn inlines_assigned_flag_accessor_installers_into_class_getters() {
        let source = "function C(e,r,a){return Object.defineProperty(e,r,a)}var K=function(e,c,a){C(e,c,{configurable:!0,get:function(){return 0!=(this.y&a)},set:function(i){i?this.y|=a:this.y&=a^-1}})};var D=(0,function(){this.y=0;return this});P=D.prototype;P.m=function(){return this.y};K(D.prototype,\"on\",1)";
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("get on(){"), "{out}");
        assert!(out.contains("set on(i){"), "{out}");
        assert!(!out.contains("K(D.prototype"), "{out}");
    }

    #[test]
    fn fuses_define_property_via_object_alias_wrapper() {
        let source = "var f=Object;function C(e,r,a){f.defineProperty(e,r,a)}var D=(0,function(){this.f=1;return this});P=D.prototype;P.m=function(){return this.f};C(D.prototype,\"on\",{configurable:!0,get:function(){return this.f},set:function(v){this.f=v}})";
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("get on(){return this.f}"), "{out}");
        assert!(out.contains("set on(v){this.f=v}"), "{out}");
        assert!(!out.contains("C(D.prototype"), "{out}");
    }

    #[test]
    fn fuses_reused_let_prototype_alias_without_redeclaring() {
        let source = r#"let b="value_";var a=(0,function(e){this.name_=e;return this});let c=a.prototype;c.report=function(){return this.name_};c=a.prototype;c.kind=function(){return "atom"};c=(0,function(g,e){a.call(this,e);this.value_=g;return this});let d=c.prototype,e=a.prototype;Object.setPrototypeOf(d,e);d=c.prototype;d.get=function(){return this.value_};d=c.prototype;d.set=function(d){this.value_=d;return d};c=new c(7,"n")"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("class a{") || out.contains("class a "), "{out}");
        assert!(
            out.contains("c=class") || out.contains("class c"),
            "{out}"
        );
        assert!(out.contains("let c=a.prototype"), "{out}");
        assert!(out.contains("c=class extends a{"), "{out}");
    }

    #[test]
    fn skips_factories_that_return_another_object() {
        let source = "var Ye=(0,function(o){var e=new j(o);return e});Ei=Ye.prototype,Ei.x=function(){return 1}";
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert_eq!(count, 0, "{out}");
        assert_eq!(out, source);
    }

    #[test]
    fn fused_classes_survive_later_generated_folds() {
        let source = r#"let b="value_";var a=(0,function(e){this.name_=e;return this});let c=a.prototype;c.report=function(){return this.name_};c=a.prototype;c.kind=function(){return "atom"};c=(0,function(g,e){a.call(this,e);this.value_=g;return this});let d=c.prototype,e=a.prototype;Object.setPrototypeOf(d,e);d=c.prototype;d.get=function(){return this.value_};d=c.prototype;d.set=function(d){this.value_=d;return d};c=new c(7,"n")"#;
        let (fused, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{fused}");
        let optimized = crate::js_peephole::optimize_generated_javascript(&fused)
            .expect("class fusion must remain valid for later folds");
        assert!(
            optimized.code.contains("class a") || optimized.code.contains("class a{"),
            "{}",
            optimized.code
        );
        assert!(
            optimized.code.contains("class") && optimized.code.contains("extends"),
            "{}",
            optimized.code
        );
    }

    #[test]
    fn promotes_indexed_arguments_to_formals() {
        let source = "function f(){var e=arguments[0];var r=arguments[1];return e+r}";
        let (out, count) = fold_indexed_arguments_to_formals(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("function f(a,b){"), "{out}");
        assert!(out.contains("var e=a"), "{out}");
        assert!(!out.contains("arguments"), "{out}");
    }

    #[test]
    fn inlines_function_declare_flag_installers_after_object_alias() {
        let source = r#"function V(t,e,r){m.defineProperty(t,e,r)}var m=Object;function Z(t,e,r){V(t,e,{configurable:!0,get:function(){return 0!=(+this.y&r)},set:function(e){var n=+this.y|0;e?this.y=n|r:this.y=n&(r^-1)}})}class q{constructor(e){this.y=0}onBO(){Ze(this)}}t=q.prototype;;ct("Atom",q),t=q.prototype,Z(t,"isBeingObserved",1),Z(t,"isPendingUnobservation",2);var oe=(0,function(){return new q})"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("get isBeingObserved(){"), "{out}");
        assert!(out.contains("get isPendingUnobservation(){"), "{out}");
        assert!(!out.contains("Z(t,\""), "{out}");
        assert!(!out.contains("function Z("), "{out}");
    }

    #[test]
    fn absorbs_computed_setter_after_symbol_method() {
        let source = r#"class C{constructor(e){this.e=e}get(){return this.e}valueOf(){return this.get()}[Symbol.toPrimitive](){return this.valueOf()}}t=C.prototype;;t.set=(e=>function(a){return e(this,a)})(function(t,e){if(t.Q){t.a=e}}),t=C.prototype,t=C.prototype;var o=Symbol.toPrimitive;ct("ComputedValue",C),t=C.prototype,Z(t,"isComputing",1)"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("set(e){"), "{out}");
        assert!(!out.contains("t.set="), "{out}");
    }

    #[test]
    fn inlines_arrow_define_property_installers_into_class_getters() {
        let source = r#"var f=Object;q=(e,t,a)=>{f.defineProperty(e,t,a)};G=(e,c,a)=>{q(e,c,{configurable:!0,get:function(){return 0!=(this.y&a)},set:function(i){i?this.y|=a:this.y&=a^-1}})};var D=(0,function(){this.y=0;return this});P=D.prototype;P.m=function(){return this.y};G(P,"on",1)"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("get on(){"), "{out}");
        assert!(out.contains("set on(i){"), "{out}");
        assert!(!out.contains("G(P"), "{out}");
    }

    #[test]
    fn installer_inliner_keeps_space_after_var() {
        let source = r#"var f=Object;q=(e,t,a)=>{f.defineProperty(e,t,a)};G=(e,c,a)=>{q(e,c,{configurable:!0,get:function(){return 0!=(this.y&a)},set:function(i){var t=+this.y|0;i?this.y=t|a:this.y=t&(a^-1)}})};var D=(0,function(){this.y=0;return this});P=D.prototype;P.m=function(){return this.y};G(P,"on",1)"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("var t=+this.y") || out.contains("var t =+this.y"), "{out}");
        assert!(!out.contains("vart"), "{out}");
    }

    #[test]
    fn installer_does_not_eat_same_named_class_method() {
        let source = r#"var f=Object;q=(e,t,a)=>{f.defineProperty(e,t,a)};G=(e,c,a)=>{q(e,c,{configurable:!0,get:function(){return this.y},set:function(i){this.y=i}})};var D=(0,function(){this.y=0;return this});P=D.prototype;P.m=function(){return this.y};G(P,"on",1);class j{constructor(){this.y=0}G(t,i){this.y=t+i}k(t){this.G(t,1)}}"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("get on(){"), "{out}");
        assert!(out.contains("G(t,i){this.y=t+i}"), "{out}");
        assert!(out.contains("this.G(t,1)"), "{out}");
        assert!(!out.contains("((e,c,a)=>"), "{out}");
    }

    #[test]
    fn full_pipeline_keeps_call_parens() {
        let source = r#"var k={};k.splice=function(u,h){var n=this[b];if(0==(arguments.length|0))return[];if(1==(arguments.length|0))return n.j(u);if(2==(arguments.length|0))return n.j(u,h);return n.j(u,h,ne.call(arguments,2,arguments.length|0))};var jt=()=>{e.splice(0,e.length)};var x=new Array(a-r.length|0)"#;
        let optimized = crate::js_peephole::optimize_generated_javascript(source).unwrap();
        assert!(optimized.code.contains("ne.call("), "{}", optimized.code);
        assert!(optimized.code.contains("e.splice("), "{}", optimized.code);
        assert!(optimized.code.contains("new Array("), "{}", optimized.code);
        assert!(!optimized.code.contains("var 2,"), "{}", optimized.code);
    }

    #[test]
    fn absorbs_exact_bound_setter_leftover() {
        let source = r#"class B{constructor(a){this.a=a}get(){return this.a}}mn=B.prototype;;mn.set=(e=>function(a){return e(this,a)})((e,i)=>{if(e.$){e.a=i}else e.a=0})"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("set(i){"), "{out}");
        assert!(!out.contains("mn.set="), "{out}");
    }

    #[test]
    fn absorbs_leftover_proto_methods_after_name_register() {
        let source = r#"class C{constructor(c){this.a=c}m(){return this.a}}mn=C.prototype;re("Atom",C),mn=C.prototype,mn.pe=function(){this.a=1},mn.toString=function(){return this.a}"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("pe(){this.a=1}"), "{out}");
        assert!(out.contains("toString(){return this.a}"), "{out}");
        assert!(!out.contains("mn.pe="), "{out}");
        assert!(out.contains("re(\"Atom\",C)"), "{out}");
    }

    #[test]
    fn second_pass_absorbs_artifact_leftovers() {
        let Ok(src) = std::fs::read_to_string("/tmp/mobx-typesplit/full.mjs") else {
            return;
        };
        if src.len() < 10_000 {
            return;
        }
        let out = crate::js_peephole::optimize_generated_javascript(&src).unwrap();
        eprintln!(
            "reopt {} -> {} t.set={} Z(t,={}",
            src.len(),
            out.code.len(),
            out.code.matches("t.set=").count(),
            out.code.matches("Z(t,").count()
        );
        std::fs::write("/tmp/mobx-typesplit/full-reopt.mjs", &out.code).ok();
        let _ = std::process::Command::new("node")
            .args(["--check", "/tmp/mobx-typesplit/full-reopt.mjs"])
            .status();
    }

    #[test]
    fn absorbs_exact_computed_setter_adapter() {
        let source = r#"class C{constructor(e){this.e=e}get(){return this.e}get diffValue(){return 1}bt(){this.z=1}}t=C.prototype;;t.set=(e=>function(a){return e(this,a)})(function(t,e){if(t.Q){!t.isRunningSetter||G(33,t.e),t.isRunningSetter=!0;try{var r=t.Q;r.call(t.L,e)}finally{t.isRunningSetter=!1}}else G(34,t.e)})"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("set(e){"), "{out}");
        assert!(!out.contains("t.set="), "{out}");
        let full = crate::js_peephole::optimize_generated_javascript(source).unwrap();
        assert!(full.code.contains("set(e){") || full.code.contains("set(e){"), "{}", full.code);
        assert!(!full.code.contains("t.set="), "{}", full.code);
    }

    #[test]
    fn absorbs_bound_this_adapter_with_renamed_impl_receiver() {
        let source = r#"class C{constructor(a){this.a=a}get(){return this.a}}t=C.prototype;t.set=(e=>function(a){return e(this,a)})(function(t,e){if(t.Q){t.a=e}})"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("set(e){"), "{out}");
        assert!(out.contains("this.Q") || out.contains("this.a=e"), "{out}");
        assert!(!out.contains("t.set="), "{out}");
    }

    #[test]
    fn absorbs_bound_this_adapter_into_class_method() {
        let source = r#"class j{constructor(a){this.a=a}get(){return this.a}}mn=j.prototype;mn.set=(e=>function(a){return e(this,a)})((e,i)=>{e.a=i})"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("set(i){this.a=i}") || out.contains("set(i){this.a=i;}"), "{out}");
        assert!(!out.contains("mn.set="), "{out}");
    }

    #[test]
    fn expands_object_assign_this_and_lifts_constructor_default() {
        let source = r#"class C{constructor(c){c=c!==void 0?c+"":"Atom",Object.assign(this,{a:c,f:new Set,K:0})}}"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("constructor(c=\"Atom\")"), "{out}");
        assert!(out.contains("this.a="), "{out}");
        assert!(!out.contains("Object.assign"), "{out}");
    }

    #[test]
    fn rewrites_optional_argument_length_guards() {
        let source = r#"class S{constructor(i,s){var c=(arguments.length|0)>0&&i!==void 0?i+"":"Reaction";this.a=c;(arguments.length|0)>1&&(this.ee=s)}}"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("constructor(i=\"Reaction\""), "{out}");
        assert!(!out.contains("arguments.length"), "{out}");
        assert!(out.contains("s!==void 0&&(this.ee=s)") || out.contains("this.ee=s"), "{out}");
    }

    #[test]
    fn inlines_reused_symbol_temp_into_class_computed_key() {
        let source = r#"class l{constructor(){this.r=1}get size(){return this.r.size}}Cn=l.prototype;xn=Symbol.iterator;Cn[xn]=function(){return this.entries()};xn=Symbol.toStringTag;q=Object.defineProperty;q(Cn,xn,{enumerable:!1,configurable:!0,get:function(){return"Map"}})"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("[Symbol.iterator](){"), "{out}");
        assert!(out.contains("get [Symbol.toStringTag](){") || out.contains("get[Symbol.toStringTag](){"), "{out}");
        assert!(!out.contains("get[xn]()"), "{out}");
        assert!(!out.contains("q(Cn,xn"), "{out}");
    }

    #[test]
    fn lifts_defaults_on_class_methods_and_rest_from_slice_call() {
        let source = r#"var ne=Array.prototype.slice;class W{j(b,v,w){var t=arguments.length>0&&b!==void 0?+b|0:0;return t}}var k={};k.splice=function(u,h){var n=this[b];if(0==arguments.length)return[];return n.j(u,h,ne.call(arguments,2,arguments.length))}"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("j(b=0") || out.contains("constructor") && out.contains("b=0"), "{out}");
        assert!(out.contains("..."), "{out}");
        assert!(!out.contains("ne.call(arguments"), "{out}");
    }

    #[test]
    fn length_guard_before_assigned_formal_keeps_rhs() {
        let source = r#"class D{s(t,a,o,r){var e;e=arguments.length>3&&r;!0===o&&(o=this.V);return e}}"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("e=r") || out.contains("return r"), "{out}");
        assert!(!out.contains("e=;"), "{out}");
        assert!(!out.contains("var e=;"), "{out}");
    }

    #[test]
    fn recovers_reaction_constructor_default_without_int32_guard() {
        let source = r#"class S{constructor(i,s,u,h){var c=arguments.length>0&&i!==void 0?i+"":"Reaction";this.a=c;s!==void 0&&(this.ee=s);arguments.length>2&&u&&(this.Y=u);arguments.length>3&&h!==void 0&&(this.Oe=h)}}"#;
        let (out, count) = fold_constructor_prototype_tables_to_classes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("constructor(i=\"Reaction\""), "{out}");
        assert!(!out.contains("arguments.length>0"), "{out}");
        assert!(out.contains("u&&(this.Y=u)"), "{out}");
        assert!(out.contains("h!==void 0&&(this.Oe=h)"), "{out}");
    }
}
