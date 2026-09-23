use std::fmt::Write as _;
use std::fs;
use std::io::{Read, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

use clap::Parser;
use lilscript::config::ProjectConfig;
use lilscript::{
    compile_source_semantic, render_module_diagnostic, ServiceError, ServiceOptions, ServiceTarget,
};

const MAX_REQUEST_BYTES: usize = 1024 * 1024;

#[derive(Debug, Parser)]
#[command(name = "lilscript-playground")]
struct Args {
    #[arg(long, default_value_t = 4173)]
    port: u16,

    #[arg(long, default_value = "web/dist")]
    web_root: PathBuf,
}

fn main() -> Result<(), String> {
    let args = Args::parse();
    let listener = TcpListener::bind(("127.0.0.1", args.port))
        .map_err(|error| format!("failed to bind playground server: {error}"))?;
    let web_root = Arc::new(args.web_root);
    if !web_root.join("index.html").is_file() {
        eprintln!(
            "warning: {} is missing; run `cd web && npm install && npm run build`",
            web_root.join("index.html").display()
        );
    }
    println!("LilScript playground: http://127.0.0.1:{}", args.port);
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let web_root = Arc::clone(&web_root);
                thread::spawn(move || {
                    if let Err(error) = handle(stream, &web_root) {
                        eprintln!("playground request failed: {error}");
                    }
                });
            }
            Err(error) => eprintln!("playground connection failed: {error}"),
        }
    }
    Ok(())
}

fn handle(mut stream: TcpStream, web_root: &Path) -> Result<(), String> {
    let mut request = Vec::new();
    let mut buffer = [0u8; 8192];
    let header_end;
    loop {
        let read = stream
            .read(&mut buffer)
            .map_err(|error| format!("failed to read request: {error}"))?;
        if read == 0 {
            return Ok(());
        }
        request.extend_from_slice(&buffer[..read]);
        if request.len() > MAX_REQUEST_BYTES {
            return respond(
                &mut stream,
                413,
                "text/plain; charset=utf-8",
                "request too large",
            );
        }
        if let Some(index) = find_bytes(&request, b"\r\n\r\n") {
            header_end = index + 4;
            break;
        }
    }

    let headers = std::str::from_utf8(&request[..header_end])
        .map_err(|_| "request headers are not UTF-8".to_string())?;
    let mut lines = headers.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| "request has no request line".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts
        .next()
        .unwrap_or("")
        .split('?')
        .next()
        .unwrap_or("")
        .to_string();
    let content_length = lines
        .find_map(|line| {
            line.split_once(':').and_then(|(name, value)| {
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
        })
        .unwrap_or(0);
    if content_length > MAX_REQUEST_BYTES {
        return respond(
            &mut stream,
            413,
            "text/plain; charset=utf-8",
            "request too large",
        );
    }
    while request.len() < header_end + content_length {
        let read = stream
            .read(&mut buffer)
            .map_err(|error| format!("failed to read request body: {error}"))?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
    }
    let body_end = (header_end + content_length).min(request.len());
    let body = &request[header_end..body_end];

    match (method.as_str(), path.as_str()) {
        ("POST", "/api/compile") => compile(body, &mut stream),
        ("GET", _) => serve_static(&path, web_root, &mut stream),
        _ => respond(&mut stream, 404, "text/plain; charset=utf-8", "not found"),
    }
}

fn serve_static(path: &str, web_root: &Path, stream: &mut TcpStream) -> Result<(), String> {
    let relative = match path {
        "/" | "/index.html" => "index.html",
        "/docs" | "/docs/" => "docs.html",
        "/about" | "/about/" => "about.html",
        value => value.trim_start_matches('/'),
    };
    if relative.is_empty()
        || relative
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
        || relative.contains('\\')
    {
        return respond(stream, 404, "text/plain; charset=utf-8", "not found");
    }

    let path = web_root.join(relative);
    let body = match fs::read(&path) {
        Ok(body) => body,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return respond(stream, 404, "text/plain; charset=utf-8", "not found");
        }
        Err(error) => return Err(format!("failed to read {}: {error}", path.display())),
    };
    respond_bytes(stream, 200, content_type(&path), &body)
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    }
}

fn compile(body: &[u8], stream: &mut TcpStream) -> Result<(), String> {
    let source = std::str::from_utf8(body)
        .map_err(|_| "LilScript source must be valid UTF-8".to_string())?;
    let response = match compile_playground(source) {
        Ok(js) => format!("{{\"ok\":true,\"js\":\"{}\"}}", json_escape(&js)),
        Err(diagnostic) => format!(
            "{{\"ok\":false,\"error\":\"{}\"}}",
            json_escape(&diagnostic)
        ),
    };
    respond(stream, 200, "application/json; charset=utf-8", &response)
}

/// The compiler's JavaScript for the page, or its rendered diagnostic. The page
/// evaluates the output as a classic script and shows what it prints, so the
/// program keeps `print` and executes as a script.
fn compile_playground(source: &str) -> Result<String, String> {
    let mut config = ProjectConfig::default();
    config.javascript.strip_console = false;
    let options = ServiceOptions {
        target: ServiceTarget::JavaScript,
        preserve_root_exports: false,
        ..ServiceOptions::default()
    };
    let compilation = compile_source_semantic(source, &config, options).map_err(render_error)?;
    compilation
        .javascript(config.javascript.cost_model)
        .map(|artifact| artifact.javascript().to_string())
        .ok_or_else(|| "the compiler selected no JavaScript artifact".to_string())
}

/// A source diagnostic renders against the playground's file name, with its span.
fn render_error(error: ServiceError) -> String {
    match error.diagnostic {
        Some(mut diagnostic) => {
            diagnostic.path = PathBuf::from("playground.lil");
            render_module_diagnostic(&diagnostic)
        }
        None => error.to_string(),
    }
}

fn respond(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &str,
) -> Result<(), String> {
    respond_bytes(stream, status, content_type, body.as_bytes())
}

fn respond_bytes(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<(), String> {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        413 => "Payload Too Large",
        _ => "Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self'; frame-src 'self'\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .map_err(|error| format!("failed to write response headers: {error}"))?;
    stream
        .write_all(body)
        .map_err(|error| format!("failed to write response body: {error}"))
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => {
                write!(escaped, "\\u{:04x}", ch as u32).expect("writing to String cannot fail");
            }
            ch => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    /// Evaluate `js` as the page does: a classic script whose `console.log`
    /// lines are the program's output.
    fn run_like_the_page(js: &str) -> String {
        let runner = "const lines=[];console.log=(...values)=>lines.push(values.map(String).join(' '));\
            (0,eval)(require('fs').readFileSync(0,'utf8'));process.stdout.write(lines.join('\\n'));";
        let mut child = Command::new("node")
            .args(["-e", runner])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("node is on PATH");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(js.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{js}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    #[test]
    fn printed_output_reaches_the_page() {
        let js = compile_playground(
            "string greeting=\"hello playground\";int[] values=[1,2,3];print(greeting);print(values.length*14);",
        )
        .unwrap();
        assert_eq!(run_like_the_page(&js), "hello playground\n42");
    }

    #[test]
    fn renders_source_diagnostics_against_the_playground_file() {
        let diagnostic = compile_playground("int value=\"wrong\";print(value);").unwrap_err();
        assert!(diagnostic.contains("playground.lil"), "{diagnostic}");
        assert!(diagnostic.contains("expected `int`"), "{diagnostic}");
        assert!(diagnostic.contains('^'), "{diagnostic}");
    }
}
