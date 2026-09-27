//! Delivered file names (plan M3.3, design §8): templates over `[name]`
//! (the entry), `[index]` (the file's position among the plan's other
//! files), `[hash:N]` (the first N hex digits of a SHA-256 over the file's
//! bytes without its own specifiers and over the hashes of the files it
//! imports), `[path]` (its source module, relative to the common source
//! directory, without extension) and `[ext]`.
//!
//! A file name is delivered bytes: an importer spells it. So `[index]`, the
//! shortest, is the library default, and `[hash:8]` the application default
//! (a changed file never reuses a cached name).
//!
//! Prior art: Rolldown `5c676e5 crates/rolldown_common/src/file_template.rs`
//! (`[name]`, `[hash]`, `[ext]` placeholders); esbuild `f6058f8
//! internal/config/config.go` (`PathTemplate` with `[dir]`, `[name]`,
//! `[hash]`, `[ext]`).

/// One placeholder or literal text of a template.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Part<'a> {
    Text(&'a str),
    Name,
    Index,
    Hash(usize),
    Path,
    Ext,
}

fn parts(template: &str) -> Result<Vec<Part<'_>>, String> {
    let mut parts = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find('[') {
        if open > 0 {
            parts.push(Part::Text(&rest[..open]));
        }
        let close = rest[open..]
            .find(']')
            .map(|close| open + close)
            .ok_or_else(|| format!("unclosed `[` in file name template `{template}`"))?;
        let token = &rest[open + 1..close];
        parts.push(match token {
            "name" => Part::Name,
            "index" => Part::Index,
            "path" => Part::Path,
            "ext" => Part::Ext,
            "hash" => Part::Hash(8),
            _ => match token.strip_prefix("hash:").map(str::parse::<usize>) {
                Some(Ok(length)) if (1..=64).contains(&length) => Part::Hash(length),
                _ => {
                    return Err(format!(
                        "unknown placeholder `[{token}]` in file name template `{template}`; use [name], [index], [hash:N], [path] or [ext]"
                    ))
                }
            },
        });
        rest = &rest[close + 1..];
    }
    if !rest.is_empty() {
        parts.push(Part::Text(rest));
    }
    Ok(parts)
}

/// Refuse a template that cannot name a file: unknown placeholders, an
/// absolute path, `..` segments, or characters a specifier would escape.
pub fn check_template(key: &str, template: &str) -> Result<(), String> {
    let parts = parts(template).map_err(|error| format!("`delivery.{key}`: {error}"))?;
    if parts.is_empty() {
        return Err(format!("`delivery.{key}` is empty"));
    }
    for part in &parts {
        if let Part::Text(text) = part {
            if text
                .chars()
                .any(|c| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-' | '/')))
            {
                return Err(format!(
                    "`delivery.{key}` = `{template}` may hold only letters, digits, `_`, `.`, `-` and `/` besides its placeholders"
                ));
            }
        }
    }
    if template.starts_with('/') || template.split('/').any(|segment| segment == "..") {
        return Err(format!(
            "`delivery.{key}` = `{template}` must stay inside the output directory"
        ));
    }
    // An importer spells the path segment by segment: `.` or an empty
    // segment would count as a directory it is not.
    if template
        .split('/')
        .any(|segment| segment.is_empty() || segment == ".")
    {
        return Err(format!(
            "`delivery.{key}` = `{template}` has an empty or `.` path segment"
        ));
    }
    Ok(())
}

/// Whether a template needs the file's content hash.
pub fn needs_hash(template: &str) -> bool {
    parts(template).is_ok_and(|parts| parts.iter().any(|part| matches!(part, Part::Hash(_))))
}

/// What a template is expanded with.
pub struct Fields<'a> {
    pub name: &'a str,
    pub index: usize,
    pub path: &'a str,
    pub ext: &'a str,
    /// Hex digest; empty when the template needs none.
    pub hash: &'a str,
}

/// Expand a checked template.
pub fn expand(template: &str, fields: &Fields<'_>) -> String {
    let mut out = String::new();
    for part in parts(template).unwrap_or_default() {
        match part {
            Part::Text(text) => out.push_str(text),
            Part::Name => out.push_str(fields.name),
            Part::Index => out.push_str(&fields.index.to_string()),
            Part::Hash(length) => out.push_str(&fields.hash[..length.min(fields.hash.len())]),
            Part::Path => out.push_str(fields.path),
            Part::Ext => out.push_str(fields.ext),
        }
    }
    out
}

/// The specifier one delivered file spells to import another: relative to
/// the importer's directory, always starting with `./` or `../`.
pub fn specifier(from: &str, to: &str) -> String {
    let from_dir: Vec<&str> = {
        let mut segments = from.split('/').collect::<Vec<_>>();
        segments.pop();
        segments
    };
    let target: Vec<&str> = to.split('/').collect();
    let common = from_dir
        .iter()
        .zip(&target)
        .take_while(|(left, right)| left == right)
        .count()
        .min(target.len().saturating_sub(1));
    let mut out = String::new();
    for _ in common..from_dir.len() {
        out.push_str("../");
    }
    if out.is_empty() {
        out.push_str("./");
    }
    out.push_str(&target[common..].join("/"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_expand_and_refuse() {
        let fields = Fields {
            name: "animate",
            index: 3,
            path: "motion-dom/value/index",
            ext: "js",
            hash: "0123456789abcdef",
        };
        assert_eq!(expand("[name].[ext]", &fields), "animate.js");
        assert_eq!(expand("internal/[index].[ext]", &fields), "internal/3.js");
        assert_eq!(expand("[hash:8].[ext]", &fields), "01234567.js");
        assert_eq!(expand("[path].[ext]", &fields), "motion-dom/value/index.js");
        assert!(needs_hash("c-[hash].js") && !needs_hash("[index].js"));
        assert!(check_template("chunk_names", "[nope].js").is_err());
        assert!(check_template("chunk_names", "../x.js").is_err());
        assert!(check_template("chunk_names", "a b.js").is_err());
        assert!(check_template("chunk_names", "[hash:0].js").is_err());
        assert!(check_template("chunk_names", "internal/[index].[ext]").is_ok());
        assert!(check_template("entry_names", "./[name].[ext]").is_err());
        assert!(check_template("entry_names", "a//[name].[ext]").is_err());
        assert!(check_template("chunk_names", "internal/").is_err());
    }

    #[test]
    fn specifiers_are_relative() {
        assert_eq!(specifier("index.js", "internal/0.js"), "./internal/0.js");
        assert_eq!(specifier("internal/0.js", "internal/1.js"), "./1.js");
        assert_eq!(specifier("internal/a/0.js", "b/1.js"), "../../b/1.js");
        assert_eq!(specifier("a.js", "b.js"), "./b.js");
    }
}
