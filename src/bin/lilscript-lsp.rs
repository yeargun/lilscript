use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;

use bumpalo::Bump;
use clap::Parser;
use lilscript::ast::{ClassMember, ExternClassMember, Item, Stmt};
use lilscript::config::load_project_config;
use lilscript::formatter::format_source;
use lilscript::lexer::{lex, lex_lossless, SyntaxElement, TokenKind, TriviaKind};
use lilscript::lint::{lint_path_with_source, DiagnosticSeverity};
use lilscript::semantic::analyze;
use lilscript::span::Span;
use lilscript::{
    compile_path_with_source, compile_path_with_source_configured, compile_source, parse_source,
};
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
                "documentSymbolProvider": true,
                "documentFormattingProvider": true,
                "referencesProvider": true,
                "renameProvider": true,
                "semanticTokensProvider": {
                    "full": true,
                    "legend": {
                        "tokenTypes": [
                            "keyword", "type", "class", "function", "variable",
                            "parameter", "property", "number", "string", "comment"
                        ],
                        "tokenModifiers": []
                    }
                },
                "codeActionProvider": {
                    "codeActionKinds": ["quickfix", "source.organizeImports"]
                }
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
        "textDocument/formatting" => Ok(formatting_result(&request.params, documents)),
        "textDocument/codeAction" => Ok(code_action_result(&request.params, documents)),
        "textDocument/references" => Ok(references_result(&request.params, documents)),
        "textDocument/rename" => rename_result(&request.params, documents),
        "textDocument/semanticTokens/full" => {
            Ok(semantic_tokens_result(&request.params, documents))
        }
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
            "diagnostics": diagnostics(Some(uri), source)
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

fn diagnostics(uri: Option<&str>, source: &str) -> Vec<Value> {
    if let Some(path) = uri.and_then(file_uri_path) {
        if path.is_file() {
            let compilation = load_project_config(&path, None).map_or_else(
                |_| compile_path_with_source(&path, source),
                |loaded| compile_path_with_source_configured(&path, source, &loaded.config),
            );
            return match compilation {
                Ok(_) => lint_diagnostics(&path, source),
                Err(error) => {
                    let current = path.canonicalize().ok();
                    let is_current = current.as_ref().is_some_and(|path| *path == error.path);
                    let span = if is_current {
                        error.span
                    } else {
                        Span::empty(0)
                    };
                    let message = if is_current {
                        error.message
                    } else {
                        format!("{}: {}", error.path.display(), error.message)
                    };
                    vec![json!({
                        "range": span_range(source, span),
                        "severity": 1,
                        "source": "lilscript",
                        "message": message
                    })]
                }
            };
        }
    }

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

fn lint_diagnostics(path: &std::path::Path, source: &str) -> Vec<Value> {
    let Ok(loaded) = load_project_config(path, None) else {
        return Vec::new();
    };
    let Ok(diagnostics) = lint_path_with_source(path, source, &loaded.config) else {
        return Vec::new();
    };
    diagnostics
        .into_iter()
        .filter(|diagnostic| {
            diagnostic.path == path
                || diagnostic.path.canonicalize().ok().as_deref()
                    == path.canonicalize().ok().as_deref()
        })
        .map(|diagnostic| {
            let severity = match diagnostic.severity {
                DiagnosticSeverity::Error => 1,
                DiagnosticSeverity::Warning => 2,
                DiagnosticSeverity::Hint => 4,
            };
            json!({
                "range": span_range(source, diagnostic.span),
                "severity": severity,
                "source": "lilscript-lint",
                "code": diagnostic.rule,
                "message": diagnostic.message,
                "data": {
                    "evidence": diagnostic.evidence,
                    "help": diagnostic.help,
                    "fix": diagnostic.fix
                }
            })
        })
        .collect()
}

fn formatting_result(params: &Value, documents: &HashMap<String, Document>) -> Value {
    let Some(uri) = request_uri(params) else {
        return Value::Array(Vec::new());
    };
    let Some(document) = documents.get(uri) else {
        return Value::Array(Vec::new());
    };
    let config = file_uri_path(uri)
        .and_then(|path| load_project_config(&path, None).ok())
        .map(|loaded| loaded.config.format)
        .unwrap_or_default();
    if !config.enabled {
        return Value::Array(Vec::new());
    }
    let Ok(formatted) = format_source(&document.text, &config) else {
        return Value::Array(Vec::new());
    };
    if formatted == document.text {
        return Value::Array(Vec::new());
    }
    json!([{
        "range": span_range(&document.text, Span::new(0, document.text.len())),
        "newText": formatted
    }])
}

fn code_action_result(params: &Value, documents: &HashMap<String, Document>) -> Value {
    let Some(uri) = request_uri(params) else {
        return Value::Array(Vec::new());
    };
    let Some(document) = documents.get(uri) else {
        return Value::Array(Vec::new());
    };
    let mut actions = Vec::new();
    if let Some(diagnostics) = params
        .pointer("/context/diagnostics")
        .and_then(Value::as_array)
    {
        for diagnostic in diagnostics {
            let Some(edits) = diagnostic
                .pointer("/data/fix/edits")
                .and_then(Value::as_array)
            else {
                continue;
            };
            let text_edits = edits
                .iter()
                .filter_map(|edit| {
                    let start = edit.pointer("/span/start")?.as_u64()? as usize;
                    let end = edit.pointer("/span/end")?.as_u64()? as usize;
                    let replacement = edit.get("replacement")?.as_str()?;
                    Some(json!({
                        "range": span_range(&document.text, Span::new(start, end)),
                        "newText": replacement
                    }))
                })
                .collect::<Vec<_>>();
            if text_edits.is_empty() {
                continue;
            }
            let mut changes = serde_json::Map::new();
            changes.insert(uri.to_string(), Value::Array(text_edits));
            let rule = diagnostic
                .get("code")
                .and_then(Value::as_str)
                .unwrap_or("lint");
            actions.push(json!({
                "title": format!("Apply {rule} fix"),
                "kind": "quickfix",
                "diagnostics": [diagnostic],
                "isPreferred": true,
                "edit": { "changes": changes }
            }));
        }
    }
    let edits = formatting_result(params, documents);
    if edits.as_array().is_some_and(|edits| !edits.is_empty()) {
        let mut changes = serde_json::Map::new();
        changes.insert(uri.to_string(), edits);
        actions.push(json!({
            "title": "Organize imports and format document",
            "kind": "source.organizeImports",
            "isPreferred": true,
            "edit": { "changes": changes }
        }));
    }
    Value::Array(actions)
}

fn references_result(params: &Value, documents: &HashMap<String, Document>) -> Value {
    let Some(uri) = request_uri(params) else {
        return Value::Array(Vec::new());
    };
    let Some((document, line, character)) = document_position(params, documents) else {
        return Value::Array(Vec::new());
    };
    let Some(offset) = byte_offset(&document.text, line, character) else {
        return Value::Array(Vec::new());
    };
    let spans = semantic_identifier_spans(&document.text, offset);
    Value::Array(
        spans
            .into_iter()
            .map(|span| json!({ "uri": uri, "range": span_range(&document.text, span) }))
            .collect(),
    )
}

fn rename_result(
    params: &Value,
    documents: &HashMap<String, Document>,
) -> Result<Value, (i32, String)> {
    let new_name = string_at(params, "/newName").ok_or_else(|| {
        (
            ErrorCode::InvalidParams as i32,
            "rename requires `newName`".to_string(),
        )
    })?;
    if !valid_identifier(new_name) || keyword_name(new_name) {
        return Err((
            ErrorCode::InvalidParams as i32,
            format!("`{new_name}` is not a valid LilScript identifier"),
        ));
    }
    let uri = request_uri(params).ok_or_else(|| {
        (
            ErrorCode::InvalidParams as i32,
            "rename requires a document URI".to_string(),
        )
    })?;
    let (document, line, character) = document_position(params, documents).ok_or_else(|| {
        (
            ErrorCode::InvalidParams as i32,
            "rename position is outside the open document".to_string(),
        )
    })?;
    let offset = byte_offset(&document.text, line, character).ok_or_else(|| {
        (
            ErrorCode::InvalidParams as i32,
            "rename position is outside the open document".to_string(),
        )
    })?;
    let spans = semantic_identifier_spans(&document.text, offset);
    if spans.is_empty() {
        return Err((
            ErrorCode::InvalidParams as i32,
            "the selected token does not resolve to a renameable symbol".to_string(),
        ));
    }
    let edits = spans
        .into_iter()
        .map(|span| json!({ "range": span_range(&document.text, span), "newText": new_name }))
        .collect::<Vec<_>>();
    let mut changes = serde_json::Map::new();
    changes.insert(uri.to_string(), Value::Array(edits));
    Ok(json!({ "changes": changes }))
}

fn semantic_identifier_spans(source: &str, offset: usize) -> Vec<Span> {
    let Some((_, selected_span)) = word_at(source, offset) else {
        return Vec::new();
    };
    let arena = Bump::new();
    let Ok(program) = parse_source(&arena, source) else {
        return Vec::new();
    };
    let Ok(semantics) = analyze(&program) else {
        return Vec::new();
    };
    let Some(selected_symbol) = semantics.identifier_symbol(selected_span) else {
        return Vec::new();
    };
    let Ok(tokens) = lex(source) else {
        return Vec::new();
    };
    tokens
        .into_iter()
        .filter_map(|token| match token.kind {
            TokenKind::Ident(_)
                if semantics.identifier_symbol(token.span) == Some(selected_symbol) =>
            {
                Some(token.span)
            }
            _ => None,
        })
        .collect()
}

fn valid_identifier(name: &str) -> bool {
    let mut bytes = name.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || matches!(first, b'_' | b'$'))
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$'))
}

fn keyword_name(name: &str) -> bool {
    matches!(
        name,
        "int"
            | "float"
            | "string"
            | "bool"
            | "void"
            | "auto"
            | "func"
            | "struct"
            | "class"
            | "return"
            | "init"
            | "if"
            | "else"
            | "while"
            | "for"
            | "break"
            | "continue"
            | "extern"
            | "import"
            | "export"
            | "from"
            | "as"
            | "pure"
            | "true"
            | "false"
            | "null"
            | "new"
            | "is"
    )
}

fn semantic_tokens_result(params: &Value, documents: &HashMap<String, Document>) -> Value {
    let Some(uri) = request_uri(params) else {
        return json!({ "data": [] });
    };
    let Some(document) = documents.get(uri) else {
        return json!({ "data": [] });
    };
    let Ok(elements) = lex_lossless(&document.text) else {
        return json!({ "data": [] });
    };
    let mut absolute = Vec::<(u32, u32, u32, u32)>::new();
    for element in elements {
        match element {
            SyntaxElement::Token(token) => {
                let Some(token_type) = semantic_token_type(&token.kind) else {
                    continue;
                };
                append_semantic_span(&document.text, token.span, token_type, &mut absolute);
            }
            SyntaxElement::Trivia(trivia)
                if matches!(
                    trivia.kind,
                    TriviaKind::LineComment | TriviaKind::BlockComment
                ) =>
            {
                append_semantic_span(&document.text, trivia.span, 9, &mut absolute);
            }
            SyntaxElement::Trivia(_) => {}
        }
    }
    absolute.sort_unstable_by_key(|entry| (entry.0, entry.1));
    let mut data = Vec::with_capacity(absolute.len() * 5);
    let mut previous_line = 0;
    let mut previous_start = 0;
    for (line, start, length, token_type) in absolute {
        let delta_line = line - previous_line;
        let delta_start = if delta_line == 0 {
            start - previous_start
        } else {
            start
        };
        data.extend([delta_line, delta_start, length, token_type, 0]);
        previous_line = line;
        previous_start = start;
    }
    json!({ "data": data })
}

fn semantic_token_type(kind: &TokenKind<'_>) -> Option<u32> {
    Some(match kind {
        TokenKind::Int
        | TokenKind::Float
        | TokenKind::String
        | TokenKind::Bool
        | TokenKind::Void
        | TokenKind::Auto => 1,
        TokenKind::IntLiteral(_) | TokenKind::FloatLiteral(_) => 7,
        TokenKind::StringLiteral(_) | TokenKind::TemplateLiteral(_) => 8,
        TokenKind::Ident(_) => 4,
        TokenKind::Func
        | TokenKind::Struct
        | TokenKind::Class
        | TokenKind::Return
        | TokenKind::Init
        | TokenKind::If
        | TokenKind::Else
        | TokenKind::While
        | TokenKind::For
        | TokenKind::Break
        | TokenKind::Continue
        | TokenKind::Extern
        | TokenKind::Import
        | TokenKind::Export
        | TokenKind::From
        | TokenKind::As
        | TokenKind::Pure
        | TokenKind::True
        | TokenKind::False
        | TokenKind::Null
        | TokenKind::New
        | TokenKind::Is => 0,
        _ => return None,
    })
}

fn append_semantic_span(
    source: &str,
    span: Span,
    token_type: u32,
    tokens: &mut Vec<(u32, u32, u32, u32)>,
) {
    let mut segment_start = span.start;
    for (relative, ch) in source[span.start..span.end].char_indices() {
        if ch != '\n' {
            continue;
        }
        append_semantic_segment(
            source,
            segment_start,
            span.start + relative,
            token_type,
            tokens,
        );
        segment_start = span.start + relative + 1;
    }
    append_semantic_segment(source, segment_start, span.end, token_type, tokens);
}

fn append_semantic_segment(
    source: &str,
    start: usize,
    end: usize,
    token_type: u32,
    tokens: &mut Vec<(u32, u32, u32, u32)>,
) {
    if start >= end {
        return;
    }
    let (line, character) = position_pair(source, start);
    let length = source[start..end].encode_utf16().count() as u32;
    tokens.push((line, character, length, token_type));
}

fn file_uri_path(uri: &str) -> Option<PathBuf> {
    let encoded = uri.strip_prefix("file://")?;
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let high = hex_value(bytes[index + 1])?;
            let low = hex_value(bytes[index + 2])?;
            decoded.push(high * 16 + low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok().map(PathBuf::from)
}

const fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn completion_result(source: Option<&str>) -> Value {
    let mut items = vec![
        keyword("int", "Signed 32-bit integer type"),
        keyword("float", "IEEE-754 binary64 type"),
        keyword("string", "Immutable UTF-8 string type"),
        keyword("bool", "Boolean type"),
        keyword("auto", "Infer a declaration type from its initializer"),
        keyword("void", "No-value return type"),
        keyword("Map", "Typed mutable key/value collection"),
        keyword("Set", "Typed mutable unique-value collection"),
        keyword("ArrayBuffer", "Fixed-length byte storage"),
        keyword(
            "SharedArrayBuffer",
            "Fixed-length storage shared by byte views",
        ),
        keyword("Int8Array", "Signed 8-bit buffer view"),
        keyword("Uint8Array", "Unsigned byte buffer view"),
        keyword("Uint8ClampedArray", "Clamped unsigned byte buffer view"),
        keyword("Int16Array", "Signed 16-bit buffer view"),
        keyword("Uint16Array", "Unsigned 16-bit buffer view"),
        keyword("Int32Array", "Signed 32-bit buffer view"),
        keyword("Uint32Array", "Unsigned 32-bit buffer view"),
        keyword("Float32Array", "IEEE-754 single-precision buffer view"),
        keyword("Float64Array", "IEEE-754 double-precision buffer view"),
        keyword("Symbol", "Unique opaque identity value"),
        keyword("Task", "Typed asynchronous value"),
        keyword("null", "Absent value for an explicitly nullable type"),
        keyword("is", "Narrow a union member with a portable runtime check"),
        keyword("return", "Return from the current function"),
        keyword("new", "Construct a class value"),
        keyword("extern", "Declare a typed host boundary"),
        keyword("import", "Import exported bindings from a LilScript module"),
        keyword("export", "Expose a module binding to importers"),
        keyword("pure", "Require a function to be side-effect free"),
        snippet("import", "import { ${1:name} } from \"${2:./module}\";", "Named module import"),
        snippet("export function", "export ${1:int} ${2:name}(${3:int} ${4:value}) {\n  return ${4:value};\n}", "Exported typed function"),
        snippet("pure function", "pure ${1:int} ${2:name}(${3:int} ${4:value}) {\n  return ${4:value};\n}", "Checked side-effect-free function"),
        snippet("struct", "struct ${1:Name} {\n  ${2:int} ${3:field};\n}", "Struct declaration"),
        snippet("class", "class ${1:Name} {\n  ${2:int} ${3:field};\n\n  init(${2:int} ${3:field}) {\n    this.${3:field} = ${3:field};\n  }\n}", "Class declaration"),
        snippet("extern class", "extern class ${1:Document} {\n  ${2:string} ${3:title};\n  ${4:void} ${5:method}(${6:string} ${7:value});\n}", "Typed JavaScript host interface"),
        snippet("extern global", "extern ${1:Document} ${2:document};", "Typed JavaScript host global"),
        snippet("function", "${1:int} ${2:name}(${3:int} ${4:value}) {\n  return ${4:value};\n}", "Typed function"),
        snippet("if", "if (${1:condition}) {\n  ${2}\n}", "Conditional block"),
        snippet("for", "for (int ${1:index} = 0; ${1:index} < ${2:length}; ${1:index}++) {\n  ${3}\n}", "Counted loop"),
        snippet("while", "while (${1:condition}) {\n  ${2}\n}", "While loop"),
        snippet("map", "map((${1:int} ${2:value}) => ${3:value})", "Typed array map"),
        snippet("filter", "filter((${1:int} ${2:value}) => ${3:condition})", "Typed array filter"),
        snippet("reduce", "reduce((${1:int} ${2:total}, ${1:int} ${3:value}) => ${2:total} + ${3:value}, ${4:0})", "Typed array reduction"),
        snippet("Map", "Map<${1:string}, ${2:int}> ${3:values} = new Map();", "Typed map declaration"),
        snippet("Set", "Set<${1:int}> ${2:values} = new Set();", "Typed set declaration"),
        snippet("Uint8Array", "Uint8Array ${1:bytes} = new Uint8Array(${2:length});", "Unsigned byte view declaration"),
        snippet("Float32Array", "Float32Array ${1:values} = new Float32Array(${2:length});", "Single-precision float view declaration"),
        snippet("Int32Array", "Int32Array ${1:values} = new Int32Array(${2:length});", "Signed 32-bit view declaration"),
        snippet(
            "dynamic import",
            "import(\"${1:./feature}\").then((auto ${2:module}) => ${2:module}.${3:run}()).catch((auto error) => print(error.message));",
            "Load a typed lazy module with explicit failure handling",
        ),
        snippet(
            "Math.imul",
            "Math.imul(${1:left}, ${2:right})",
            "Exact low-32-bit integer multiplication",
        ),
        snippet(
            "toUnsignedString",
            "toUnsignedString(${1:36})",
            "Format an integer's unsigned 32-bit bit pattern",
        ),
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
    for import in program.imports {
        for specifier in import.specifiers {
            items.push(json!({
                "label": specifier.local.name,
                "kind": 6,
                "detail": format!("imported from {}", import.source)
            }));
        }
    }
    for item in program.items {
        let entry = match item {
            Item::Struct(decl) => {
                json!({ "label": decl.name.name, "kind": 22, "detail": "LilScript struct" })
            }
            Item::Class(decl) => {
                json!({ "label": decl.name.name, "kind": 7, "detail": "LilScript class" })
            }
            Item::ExternClass(decl) => {
                json!({ "label": decl.name.name, "kind": 7, "detail": "LilScript host interface" })
            }
            Item::Function(decl) => {
                json!({ "label": decl.name.name, "kind": 3, "detail": "LilScript function" })
            }
            Item::Extern(decl) => {
                json!({ "label": decl.name.name, "kind": 3, "detail": "LilScript extern function" })
            }
            Item::ExternGlobal(decl) => {
                json!({ "label": decl.name.name, "kind": 6, "detail": "LilScript host global" })
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
        "int" => "Signed 32-bit integer. Ordinary multiplication follows JavaScript number multiplication followed by i32 normalization.",
        "Math" => "Built-in numeric namespace containing the explicit `Math.imul(int, int)` intrinsic.",
        "imul" => "Returns the exact low 32 bits of two integer operands. The compiler preserves explicit calls and never introduces them for ordinary `*`.",
        "toUnsignedString" => "Formats an int's unsigned 32-bit bit pattern with a radix from 2 through 36.",
        "toString" => "Formats a signed int with a radix from 2 through 36.",
        "float" => "IEEE-754 binary64 floating-point value.",
        "string" => "Immutable UTF-8 text value.",
        "bool" => "Boolean value: `true` or `false`.",
        "Map" => "Invariant typed key/value collection with `get`, `set`, `has`, `delete`, `clear`, and `size`.",
        "Set" => "Invariant typed unique-value collection with `add`, `has`, `delete`, `clear`, and `size`.",
        "ArrayBuffer" => "Fixed-length byte storage. Create typed views such as `Uint8Array` or `Float32Array`.",
        "SharedArrayBuffer" => "Fixed-length storage shared by views; host security policy controls browser availability.",
        "Int8Array" => "Signed 8-bit view with indexed access, copy slices, and zero-copy subarrays.",
        "Uint8Array" => "Unsigned byte view with indexed access, copy slices, and zero-copy subarrays.",
        "Uint8ClampedArray" => "Unsigned byte view that clamps stores into the 0..255 range.",
        "Int16Array" => "Signed 16-bit view with indexed access, copy slices, and zero-copy subarrays.",
        "Uint16Array" => "Unsigned 16-bit view with indexed access, copy slices, and zero-copy subarrays.",
        "Int32Array" => "Signed 32-bit view with indexed access, copy slices, and zero-copy subarrays.",
        "Uint32Array" => "Unsigned 32-bit view exposed as `int` using ToInt32 bit-pattern semantics.",
        "Float32Array" => "IEEE-754 single-precision view with indexed access, copy slices, and zero-copy subarrays.",
        "Float64Array" => "IEEE-754 double-precision view with indexed access, copy slices, and zero-copy subarrays.",
        "Symbol" => "Unique opaque identity value for map keys and event channels.",
        "Task" => "Typed asynchronous value returned by dynamic module imports. Chain `then`, `catch`, and `finally` without an untyped Promise boundary.",
        "auto" => "Infers the binding type from its required initializer.",
        "null" => "Absent value assignable only to an explicitly nullable `T?` type.",
        "is" => "Checks a portable runtime type category and narrows a union binding in the selected branch.",
        "struct" => "Declares a positional value aggregate eligible for scalar replacement.",
        "class" => "Declares a nominal reference type with fields, one `init`, and methods.",
        "init" => "Declares the constructor body for a class.",
        "extern" => "Declares a typed function, host interface, or host global. JavaScript host member names remain exact and are emitted without wrappers.",
        "import" => "Adds a relative `.lil` module to the closed-world compilation graph and binds selected exports.",
        "export" => "Makes a top-level module binding available to named imports without forcing it to remain in the final bundle.",
        "pure" => "Asserts that a function has no observable side effects; the compiler rejects a violated contract and infers purity without it.",
        "map" => "Transforms every array element with a statically typed callback.",
        "filter" => "Returns elements for which a typed callback returns `true`.",
        "reduce" => "Combines array elements into one typed accumulator value.",
        "forEach" => "Invokes a typed callback for each array element.",
        "push" => "Appends one typed element and returns the new array length.",
        "pop" => "Removes and returns the final typed array element.",
        "includes" => "Tests whether a string contains another string.",
        "startsWith" => "Tests a string prefix.",
        "endsWith" => "Tests a string suffix.",
        "charCodeAt" => "Returns the UTF-16 code unit at an integer string index, or zero outside the string.",
        "toUpperCase" => "Returns an uppercase string.",
        "toLowerCase" => "Returns a lowercase string.",
        "abs" => "Returns the absolute value of a float.",
        "floor" => "Rounds a float toward negative infinity.",
        "ceil" => "Rounds a float toward positive infinity.",
        "min" => "Returns the smaller of two floats with JavaScript Math.min semantics.",
        "max" => "Returns the larger of two floats with JavaScript Math.max semantics.",
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
            Item::ExternClass(decl) => {
                let children = decl
                    .members
                    .iter()
                    .map(|member| match member {
                        ExternClassMember::Field(field) => document_symbol(
                            &document.text,
                            field.name.name,
                            8,
                            field.span,
                            field.name.span,
                            Vec::new(),
                        ),
                        ExternClassMember::Method(method) => document_symbol(
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
            Item::ExternGlobal(decl) => symbols.push(document_symbol(
                &document.text,
                decl.name.name,
                13,
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
    let (line, character) = position_pair(source, requested_offset);
    json!({ "line": line, "character": character })
}

fn position_pair(source: &str, requested_offset: usize) -> (u32, u32) {
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
    (line, character)
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
        let result = diagnostics(None, "int value=\"wrong\";");
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
    fn semantic_navigation_respects_shadowed_bindings() {
        let source = "int value=1;int read(){int value=2;return value;}print(value);";
        let global = source.find("value").unwrap() + 1;
        let local = source[source.find("read").unwrap()..]
            .find("value")
            .unwrap()
            + source.find("read").unwrap()
            + 1;

        assert_eq!(semantic_identifier_spans(source, global).len(), 2);
        assert_eq!(semantic_identifier_spans(source, local).len(), 2);
        assert_ne!(
            semantic_identifier_spans(source, global),
            semantic_identifier_spans(source, local)
        );
    }

    #[test]
    fn emits_delta_encoded_semantic_tokens() {
        let uri = "file:///tmp/semantic.lil";
        let mut documents = HashMap::new();
        documents.insert(
            uri.to_string(),
            Document {
                text: "// note\nint value=1;".to_string(),
            },
        );
        let tokens = semantic_tokens_result(&json!({ "textDocument": { "uri": uri } }), &documents);
        let data = tokens["data"].as_array().unwrap();
        assert_eq!(data.len() % 5, 0);
        assert!(data.len() >= 20);
    }

    #[test]
    fn lexer_accepts_documented_completion_keywords() {
        assert!(lilscript::lexer::lex(
            "struct Point{int? x;}Point? point=null;bool text=point is string;"
        )
        .is_ok());
        assert!(completion_result(None)["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["label"] == "Math.imul"));
        assert!(completion_result(None)["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["label"] == "toUnsignedString"));
    }

    #[test]
    fn resolves_modules_for_saved_document_diagnostics() {
        let directory =
            std::env::temp_dir().join(format!("lilscript-lsp-module-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("math.lil"),
            "export pure int square(int value){return value*value;}",
        )
        .unwrap();
        let main = directory.join("main.lil");
        let source = "import {square as sq} from \"./math\";print(sq(4));";
        std::fs::write(&main, source).unwrap();
        let uri = format!("file://{}", main.display());

        assert!(diagnostics(Some(&uri), source).is_empty());
        let completions = completion_result(Some(source));
        assert!(completions["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["label"] == "sq"));
        std::fs::remove_dir_all(directory).unwrap();
    }
}
