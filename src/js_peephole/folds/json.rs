use crate::js_peephole::rewrite::apply_token_rewrites;
use crate::js_peephole::token::{is_identifier_continue, is_identifier_start, lex, TokenKind};
use crate::js_peephole::JavaScriptParseError;
use serde_json::Value;

pub(crate) fn fold_constant_json_parse(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut index = 0;
    while index + 5 < tokens.len() {
        if tokens[index].text == "JSON"
            && tokens[index + 1].text == "."
            && tokens[index + 2].text == "parse"
            && tokens[index + 3].text == "("
            && tokens[index + 4].kind == TokenKind::String
            && tokens[index + 5].text == ")"
        {
            if let Some(rendered) = render_json_parse_literal(tokens[index + 4].text) {
                let start = tokens[index].start;
                let end = tokens[index + 5].end;
                if rendered.len() < end - start {
                    replacements.push((start, end, rendered));
                }
            }
            index += 6;
            continue;
        }
        index += 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn render_json_parse_literal(literal: &str) -> Option<String> {
    let json = unescape_js_string(literal)?;
    let value = serde_json::from_str::<Value>(&json).ok()?;
    if !matches!(value, Value::Object(_) | Value::Array(_)) {
        return None;
    }
    Some(render_js_value(&value))
}

fn render_js_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(true) => "!0".to_string(),
        Value::Bool(false) => "!1".to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => quote_js_string(text),
        Value::Array(items) => {
            let mut out = String::from("[");
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&render_js_value(item));
            }
            out.push(']');
            out
        }
        Value::Object(fields) => {
            let mut out = String::from("{");
            for (index, (key, item)) in fields.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                push_js_object_key(&mut out, key);
                out.push(':');
                out.push_str(&render_js_value(item));
            }
            out.push('}');
            out
        }
    }
}

fn push_js_object_key(out: &mut String, key: &str) {
    if key != "__proto__" && is_js_ident_key(key) {
        out.push_str(key);
        return;
    }
    out.push_str(&quote_js_string(key));
}

fn is_js_ident_key(key: &str) -> bool {
    let bytes = key.as_bytes();
    !bytes.is_empty()
        && is_identifier_start(bytes[0])
        && bytes[1..].iter().copied().all(is_identifier_continue)
}

fn quote_js_string(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            ch if (ch as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", ch as u32));
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

pub(crate) fn unescape_js_string(literal: &str) -> Option<String> {
    let bytes = literal.as_bytes();
    if bytes.len() < 2 {
        return None;
    }
    let quote = bytes[0];
    if !matches!(quote, b'"' | b'\'') || bytes[bytes.len() - 1] != quote {
        return None;
    }
    let mut out = String::new();
    let mut chars = literal[1..literal.len() - 1].chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next()? {
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'b' => out.push('\u{0008}'),
            'f' => out.push('\u{000c}'),
            'v' => out.push('\u{000b}'),
            '0' => out.push('\0'),
            '\\' => out.push('\\'),
            '"' => out.push('"'),
            '\'' => out.push('\''),
            '/' => out.push('/'),
            'x' => {
                let hi = chars.next()?.to_digit(16)?;
                let lo = chars.next()?.to_digit(16)?;
                out.push(char::from_u32((hi << 4) | lo)?);
            }
            'u' => {
                if chars.peek() == Some(&'{') {
                    chars.next();
                    let mut value = 0u32;
                    loop {
                        let digit = chars.next()?;
                        if digit == '}' {
                            break;
                        }
                        value = value.checked_shl(4)?.checked_add(digit.to_digit(16)?)?;
                    }
                    out.push(char::from_u32(value)?);
                } else {
                    let mut value = 0u32;
                    for _ in 0..4 {
                        value = (value << 4) | chars.next()?.to_digit(16)?;
                    }
                    out.push(char::from_u32(value)?);
                }
            }
            '\n' => {}
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
            }
            other => out.push(other),
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::fold_constant_json_parse;

    #[test]
    fn folds_constant_json_object_to_a_literal() {
        let source = r#"var table=JSON.parse("{\"AElig\":\"Æ\",\"AMP\":\"&\"}")"#;
        let (out, count) = fold_constant_json_parse(source).unwrap();
        assert_eq!(count, 1, "{out}");
        assert!(out.contains("{AElig:\"Æ\",AMP:\"&\"}"), "{out}");
        assert!(!out.contains("JSON.parse"), "{out}");
    }

    #[test]
    fn keeps_non_object_json_parse() {
        let source = r#"var table=JSON.parse("null")"#;
        let (out, count) = fold_constant_json_parse(source).unwrap();
        assert_eq!(count, 0, "{out}");
        assert!(out.contains("JSON.parse"), "{out}");
    }
}
