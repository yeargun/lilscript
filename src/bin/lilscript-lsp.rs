use std::collections::HashMap;
use std::error::Error;

use bumpalo::Bump;
use clap::Parser;
use lilscript::ast::{ClassMember, Item, Stmt};
use lilscript::span::Span;
use lilscript::{compile_source, parse_source};
use lsp_server::{Connection, ErrorCode, Message, Notification, Request, Response};
use serde_json::{json, Value};

#[derive(Debug, Parser)]
#[command(name = "lilscript-lsp")]
#[command(about = "Language Server Protocol implementation for LilScript.")]
struct Args {}

#[derive(Debug, Clone)]
struct Document {
    text: String,
}

fn main() {
    Args::parse();
    if let Err(error) = run() {
        eprintln!("lilscript-lsp: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error + Send + Sync>> {
    let (connection, io_threads) = Connection::stdio();
    let (initialize_id, _) = connection.initialize_start()?;
    connection.initialize_finish(
        initialize_id,
        json!({
            "capabilities": {
                "positionEncoding": "utf-16",
                "textDocumentSync": { "openClose": true, "change": 1 },
                "completionProvider": {
                    "resolveProvider": false,
                    "triggerCharacters": ["."]
                },
                "hoverProvider": true,
                "documentSymbolProvider": true
            },
            "serverInfo": { "name": "lilscript-lsp", "version": env!("CARGO_PKG_VERSION") }
        }),
    )?;

    let mut documents = HashMap::<String, Document>::new();
    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request)? {
                    break;
                }
                let response = handle_request(request, &documents);
                connection.sender.send(Message::Response(response))?;
            }
            Message::Notification(notification) => {
                handle_notification(notification, &mut documents, &connection)?;
            }
            Message::Response(_) => {}
        }
    }

    drop(connection);
    io_threads.join()?;
    Ok(())
}

fn handle_notification(
    notification: Notification,
    documents: &mut HashMap<String, Document>,
    connection: &Connection,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match notification.method.as_str() {
        "textDocument/didOpen" => {
            let Some(uri) = string_at(&notification.params, "/textDocument/uri") else {
                return Ok(());
            };
            let Some(text) = string_at(&notification.params, "/textDocument/text") else {
                return Ok(());
            };
            let version = integer_at(&notification.params, "/textDocument/version");
            documents.insert(
                uri.to_string(),
                Document {
                    text: text.to_string(),
                },
            );
            publish_diagnostics(connection, uri, text, version)?;
        }
        "textDocument/didChange" => {
            let Some(uri) = string_at(&notification.params, "/textDocument/uri") else {
                return Ok(());
            };
            let Some(text) = notification
                .params
                .pointer("/contentChanges")
                .and_then(Value::as_array)
                .and_then(|changes| changes.last())
                .and_then(|change| change.get("text"))
                .and_then(Value::as_str)
            else {
                return Ok(());
            };
            let version = integer_at(&notification.params, "/textDocument/version");
            documents.insert(
                uri.to_string(),
                Document {
                    text: text.to_string(),
                },
            );
            publish_diagnostics(connection, uri, text, version)?;
        }
        "textDocument/didClose" => {
            let Some(uri) = string_at(&notification.params, "/textDocument/uri") else {
                return Ok(());
            };
            documents.remove(uri);
            send_notification(
                connection,
                "textDocument/publishDiagnostics",
                json!({ "uri": uri, "diagnostics": [] }),
            )?;
        }
        _ => {}
    }
    Ok(())
}

fn handle_request(request: Request, documents: &HashMap<String, Document>) -> Response {
    let result = match request.method.as_str() {
        "textDocument/completion" => {
            let source = request_uri(&request.params)
                .and_then(|uri| documents.get(uri))
                .map(|document| document.text.as_str());
            Ok(completion_result(source))
        }
        "textDocument/hover" => Ok(hover_result(&request.params, documents)),
        "textDocument/documentSymbol" => Ok(document_symbol_result(&request.params, documents)),
        _ => Err((
            ErrorCode::MethodNotFound as i32,
            format!("unsupported request `{}`", request.method),
        )),
    };

    match result {
        Ok(value) => Response::new_ok(request.id, value),
        Err((code, message)) => Response::new_err(request.id, code, message),
    }
}

fn publish_diagnostics(
    connection: &Connection,
    uri: &str,
    source: &str,
    version: Option<i64>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    send_notification(
        connection,
        "textDocument/publishDiagnostics",
        json!({
            "uri": uri,
            "version": version,
            "diagnostics": diagnostics(source)
        }),
    )
}

fn send_notification(
    connection: &Connection,
    method: &str,
    params: Value,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    connection
        .sender
        .send(Message::Notification(Notification::new(
            method.to_string(),
            params,
        )))?;
    Ok(())
}

fn diagnostics(source: &str) -> Vec<Value> {
    match compile_source(source) {
        Ok(_) => Vec::new(),
        Err(error) => vec![json!({
            "range": span_range(source, error.span()),
            "severity": 1,
            "source": "lilscript",
            "message": error.to_string()
        })],
    }
}

fn completion_result(source: Option<&str>) -> Value {
    let mut items = vec![
        keyword("int", "Signed wrapping 32-bit integer type"),
        keyword("float", "IEEE-754 binary64 type"),
        keyword("string", "Immutable UTF-8 string type"),
        keyword("bool", "Boolean type"),
        keyword("auto", "Infer a declaration type from its initializer"),
        keyword("void", "No-value return type"),
        keyword("return", "Return from the current function"),
        keyword("new", "Construct a class value"),
        keyword("extern", "Declare a typed host boundary"),
        snippet("struct", "struct ${1:Name} {\n  ${2:int} ${3:field};\n}", "Struct declaration"),
        snippet("class", "class ${1:Name} {\n  ${2:int} ${3:field};\n\n  init(${2:int} ${3:field}) {\n    this.${3:field} = ${3:field};\n  }\n}", "Class declaration"),
        snippet("function", "${1:int} ${2:name}(${3:int} ${4:value}) {\n  return ${4:value};\n}", "Typed function"),
        snippet("if", "if (${1:condition}) {\n  ${2}\n}", "Conditional block"),
        snippet("for", "for (int ${1:index} = 0; ${1:index} < ${2:length}; ${1:index}++) {\n  ${3}\n}", "Counted loop"),
        snippet("while", "while (${1:condition}) {\n  ${2}\n}", "While loop"),
        snippet("map", "map((${1:int} ${2:value}) => ${3:value})", "Typed array map"),
        snippet("filter", "filter((${1:int} ${2:value}) => ${3:condition})", "Typed array filter"),
        snippet("reduce", "reduce((${1:int} ${2:total}, ${1:int} ${3:value}) => ${2:total} + ${3:value}, ${4:0})", "Typed array reduction"),
        function_item("print", "Portable observable output intrinsic"),
    ];

    if let Some(source) = source {
        append_document_completions(source, &mut items);
    }
    json!({ "isIncomplete": false, "items": items })
}

fn keyword(label: &str, detail: &str) -> Value {
    json!({ "label": label, "kind": 14, "detail": detail })
}

fn function_item(label: &str, detail: &str) -> Value {
    json!({ "label": label, "kind": 3, "detail": detail })
}

fn snippet(label: &str, insert_text: &str, detail: &str) -> Value {
    json!({
        "label": label,
        "kind": 15,
        "detail": detail,
        "insertText": insert_text,
        "insertTextFormat": 2
    })
}

fn append_document_completions(source: &str, items: &mut Vec<Value>) {
    let arena = Bump::new();
    let Ok(program) = parse_source(&arena, source) else {
        return;
    };
    for item in program.items {
        let entry = match item {
            Item::Struct(decl) => {
                json!({ "label": decl.name.name, "kind": 22, "detail": "LilScript struct" })
            }
            Item::Class(decl) => {
                json!({ "label": decl.name.name, "kind": 7, "detail": "LilScript class" })
            }
            Item::Function(decl) => {
                json!({ "label": decl.name.name, "kind": 3, "detail": "LilScript function" })
            }
            Item::Extern(decl) => {
                json!({ "label": decl.name.name, "kind": 3, "detail": "LilScript extern function" })
            }
            Item::Stmt(Stmt::VarDecl(decl)) => {
                json!({ "label": decl.name.name, "kind": 6, "detail": "LilScript binding" })
            }
            Item::Stmt(_) => continue,
        };
        items.push(entry);
    }
}

fn hover_result(params: &Value, documents: &HashMap<String, Document>) -> Value {
    let Some((document, line, character)) = document_position(params, documents) else {
        return Value::Null;
    };
    let Some(offset) = byte_offset(&document.text, line, character) else {
        return Value::Null;
    };
    let Some((word, span)) = word_at(&document.text, offset) else {
        return Value::Null;
    };
    let Some(description) = language_help(word) else {
        return Value::Null;
    };
    json!({
        "contents": {
            "kind": "markdown",
            "value": format!("`{word}`\n\n{description}")
        },
        "range": span_range(&document.text, span)
    })
}

fn language_help(word: &str) -> Option<&'static str> {
    Some(match word {
        "int" => "Signed 32-bit integer with two's-complement wrapping semantics.",
        "float" => "IEEE-754 binary64 floating-point value.",
        "string" => "Immutable UTF-8 text value.",
        "bool" => "Boolean value: `true` or `false`.",
        "auto" => "Infers the binding type from its required initializer.",
        "struct" => "Declares a positional value aggregate eligible for scalar replacement.",
        "class" => "Declares a nominal reference type with fields, one `init`, and methods.",
        "init" => "Declares the constructor body for a class.",
        "extern" => "Declares a typed function implemented by the JavaScript or native host.",
        "map" => "Transforms every array element with a statically typed callback.",
        "filter" => "Returns elements for which a typed callback returns `true`.",
        "reduce" => "Combines array elements into one typed accumulator value.",
        "forEach" => "Invokes a typed callback for each array element.",
        "push" => "Appends one typed element and returns the new array length.",
        "pop" => "Removes and returns the final typed array element.",
        "includes" => "Tests whether a string contains another string.",
        "startsWith" => "Tests a string prefix.",
        "endsWith" => "Tests a string suffix.",
        "toUpperCase" => "Returns an uppercase string.",
        "toLowerCase" => "Returns a lowercase string.",
        "print" => "Portable output intrinsic supported by JavaScript and native targets.",
        _ => return None,
    })
}

fn document_symbol_result(params: &Value, documents: &HashMap<String, Document>) -> Value {
    let Some(uri) = request_uri(params) else {
        return json!([]);
    };
    let Some(document) = documents.get(uri) else {
        return json!([]);
    };
    let arena = Bump::new();
    let Ok(program) = parse_source(&arena, &document.text) else {
        return json!([]);
    };
    let mut symbols = Vec::new();
    for item in program.items {
        match item {
            Item::Struct(decl) => symbols.push(document_symbol(
                &document.text,
                decl.name.name,
                23,
                decl.span,
                decl.name.span,
                decl.fields
                    .iter()
                    .map(|field| {
                        document_symbol(
                            &document.text,
                            field.name.name,
                            8,
                            field.span,
                            field.name.span,
                            Vec::new(),
                        )
                    })
                    .collect(),
            )),
            Item::Class(decl) => {
                let children = decl
                    .members
                    .iter()
                    .map(|member| match member {
                        ClassMember::Field(field) => document_symbol(
                            &document.text,
                            field.name.name,
                            8,
                            field.span,
                            field.name.span,
                            Vec::new(),
                        ),
                        ClassMember::Constructor(constructor) => document_symbol(
                            &document.text,
                            "init",
                            9,
                            constructor.span,
                            constructor.span,
                            Vec::new(),
                        ),
                        ClassMember::Method(method) => document_symbol(
                            &document.text,
                            method.name.name,
                            6,
                            method.span,
                            method.name.span,
                            Vec::new(),
                        ),
                    })
                    .collect();
                symbols.push(document_symbol(
                    &document.text,
                    decl.name.name,
                    5,
                    decl.span,
                    decl.name.span,
                    children,
                ));
            }
            Item::Function(decl) => symbols.push(document_symbol(
                &document.text,
                decl.name.name,
                12,
                decl.span,
                decl.name.span,
                Vec::new(),
            )),
            Item::Extern(decl) => symbols.push(document_symbol(
                &document.text,
                decl.name.name,
                12,
                decl.span,
                decl.name.span,
                Vec::new(),
            )),
            Item::Stmt(Stmt::VarDecl(decl)) => symbols.push(document_symbol(
                &document.text,
                decl.name.name,
                13,
                decl.span,
                decl.name.span,
                Vec::new(),
            )),
            Item::Stmt(_) => {}
        }
    }
    Value::Array(symbols)
}

fn document_symbol(
    source: &str,
    name: &str,
    kind: u8,
    span: Span,
    selection: Span,
    children: Vec<Value>,
) -> Value {
    json!({
        "name": name,
        "kind": kind,
        "range": span_range(source, span),
        "selectionRange": span_range(source, selection),
        "children": children
    })
}

fn request_uri(params: &Value) -> Option<&str> {
    string_at(params, "/textDocument/uri")
}

fn document_position<'a>(
    params: &Value,
    documents: &'a HashMap<String, Document>,
) -> Option<(&'a Document, u32, u32)> {
    let uri = request_uri(params)?;
    let document = documents.get(uri)?;
    let line = params.pointer("/position/line")?.as_u64()? as u32;
    let character = params.pointer("/position/character")?.as_u64()? as u32;
    Some((document, line, character))
}

fn string_at<'a>(value: &'a Value, pointer: &str) -> Option<&'a str> {
    value.pointer(pointer)?.as_str()
}

fn integer_at(value: &Value, pointer: &str) -> Option<i64> {
    value.pointer(pointer)?.as_i64()
}

fn span_range(source: &str, span: Span) -> Value {
    json!({
        "start": position_at(source, span.start),
        "end": position_at(source, span.end.max(span.start + usize::from(span.start < source.len())))
    })
}

fn position_at(source: &str, requested_offset: usize) -> Value {
    let mut offset = requested_offset.min(source.len());
    while !source.is_char_boundary(offset) {
        offset -= 1;
    }
    let mut line = 0_u32;
    let mut character = 0_u32;
    for ch in source[..offset].chars() {
        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += ch.len_utf16() as u32;
        }
    }
    json!({ "line": line, "character": character })
}

fn byte_offset(source: &str, requested_line: u32, requested_character: u32) -> Option<usize> {
    let mut line = 0_u32;
    let mut character = 0_u32;
    for (offset, ch) in source.char_indices() {
        if line == requested_line && character >= requested_character {
            return Some(offset);
        }
        if ch == '\n' {
            if line == requested_line {
                return Some(offset);
            }
            line += 1;
            character = 0;
        } else if line == requested_line {
            character += ch.len_utf16() as u32;
        }
    }
    (line == requested_line).then_some(source.len())
}

fn word_at(source: &str, offset: usize) -> Option<(&str, Span)> {
    let bytes = source.as_bytes();
    let mut start = offset.min(bytes.len());
    while start > 0 && is_identifier_byte(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = offset.min(bytes.len());
    while end < bytes.len() && is_identifier_byte(bytes[end]) {
        end += 1;
    }
    (start < end).then_some((&source[start..end], Span::new(start, end)))
}

const fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_byte_spans_to_utf16_positions() {
        let source = "string value=\"😀\";\nint bad=1;";
        assert_eq!(
            position_at(source, source.find("bad").unwrap()),
            json!({ "line": 1, "character": 4 })
        );
    }

    #[test]
    fn returns_compiler_diagnostics() {
        let result = diagnostics("int value=\"wrong\";");
        assert_eq!(result.len(), 1);
        assert!(result[0]["message"]
            .as_str()
            .unwrap()
            .contains("expected `int`"));
    }

    #[test]
    fn identifies_hover_words() {
        let source = "int[] values=[1];values.map((int value)=>value);";
        let offset = source.find("map").unwrap() + 1;
        assert_eq!(word_at(source, offset).unwrap().0, "map");
        assert!(language_help("map").unwrap().contains("array element"));
    }

    #[test]
    fn lexer_accepts_documented_completion_keywords() {
        assert!(lilscript::lexer::lex("struct Point{int x;}").is_ok());
    }
}
