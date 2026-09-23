//! Rendering compiler diagnostics against the source they point at. The
//! compiler reports a [`ModuleError`] (a message at a span of one module) or a
//! [`ServiceError`] that may carry one; anything else with a span renders
//! through [`SourceDiagnostic`].

use std::path::Path;

use crate::compiler_service::ServiceError;
use crate::module::ModuleError;
use crate::parser::ParseError;
use crate::span::Span;

/// An error located at one span of the source it was reported against.
pub trait SourceDiagnostic: std::fmt::Display {
    fn span(&self) -> Span;
}

impl SourceDiagnostic for ParseError {
    fn span(&self) -> Span {
        ParseError::span(self)
    }
}

impl SourceDiagnostic for ModuleError {
    fn span(&self) -> Span {
        self.span
    }
}

/// A module error, rendered against the module it names.
pub fn render_module_diagnostic(error: &ModuleError) -> String {
    render_message_diagnostic(&error.path, &error.source, error.span, &error.message)
}

/// A compilation error: against its source when it has a location, else its
/// message alone.
pub fn render_service_error(error: &ServiceError) -> String {
    error
        .diagnostic
        .as_ref()
        .map(render_module_diagnostic)
        .unwrap_or_else(|| error.to_string())
}

/// An error with a span, rendered against the source it was reported for.
pub fn render_diagnostic(
    path: &Path,
    source: &str,
    error: &(impl SourceDiagnostic + ?Sized),
) -> String {
    render_message_diagnostic(path, source, error.span(), &error.to_string())
}

/// `message` at `span` of `source`, with the source line and a marker.
pub fn render_message_diagnostic(path: &Path, source: &str, span: Span, message: &str) -> String {
    let start = span.start.min(source.len());
    let end = span.end.min(source.len()).max(start);
    let line_start = source[..start].rfind('\n').map_or(0, |index| index + 1);
    let line_end = source[end..]
        .find('\n')
        .map_or(source.len(), |index| end + index);
    let line_number = source[..line_start]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1;
    let column = source[line_start..start].chars().count() + 1;
    let width = source[start..end].chars().count().max(1);
    let source_line = &source[line_start..line_end];
    let padding = " ".repeat(column.saturating_sub(1));
    let marker = "^".repeat(width);

    format!(
        "error: {message}\n --> {}:{line_number}:{column}\n  |\n{line_number:>2} | {source_line}\n  | {padding}{marker}",
        path.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_a_parse_error_under_its_source_line() {
        let source = "int ok=1;\nint = 2;\n";
        let arena = bumpalo::Bump::new();
        let error = crate::parser::parse_source(&arena, source).err().unwrap();
        let rendered = render_diagnostic(Path::new("sample.lil"), source, &error);
        assert!(rendered.starts_with("error: "), "{rendered}");
        assert!(rendered.contains(" --> sample.lil:2:"), "{rendered}");
        assert!(rendered.contains(" 2 | int = 2;"), "{rendered}");
    }

    #[test]
    fn a_marker_spans_the_reported_characters() {
        let rendered = render_message_diagnostic(
            Path::new("m.lil"),
            "print(value);",
            Span { start: 6, end: 11 },
            "unknown identifier `value`",
        );
        assert_eq!(
            rendered,
            "error: unknown identifier `value`\n --> m.lil:1:7\n  |\n 1 | print(value);\n  |       ^^^^^"
        );
    }
}
