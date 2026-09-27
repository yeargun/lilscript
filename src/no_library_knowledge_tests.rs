//! NO3 (architecture §18.3; plan rule 10, task M2.13): no library knowledge in
//! the compiler. Non-test `src/` must not name a port or an upstream library
//! beyond the allowlist ledger `tests/no3-allowlist.json`, where each mention
//! carries a reason and an owner (M8.7 empties it). The ledger is a ratchet: a
//! file may not gain a mention, and one that loses a mention must lower its
//! entry in the same change, so the ledger only ever shrinks.
//!
//! Matching is lexical and deliberately plain: case-insensitive substrings for
//! names that are not English words, and exact spellings for the few that are
//! (`marked`, `motion`, `remark`, `unified`, `solid`, `React` are only counted
//! in their library forms). Test files (`*_tests.rs`, `tests.rs`, fixtures and
//! inline `#[cfg(test)] mod … {` bodies) are not compiler knowledge.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// (ledger name, needle, case-insensitive). Every needle maps to one ledger
/// name, so `katexlil` and `KaTeX` are both `katex`.
const NEEDLES: &[(&str, &str, bool)] = &[
    ("katex", "katex", true),
    ("katex", "mhchem", true),
    ("katex", "fontmetrics", true),
    ("marked", "markedlil", true),
    ("marked", "marked.js", true),
    ("zod", "zod", true),
    ("motion", "motionlil", true),
    ("motion", "framer-motion", true),
    ("motion", "motion.dev", true),
    ("posthog", "posthog", true),
    ("jquery", "jquery", true),
    ("micromark", "micromark", true),
    ("mobx", "mobx", true),
    ("remark", "remark-", true),
    ("remark", "remarklil", true),
    ("rehype", "rehype", true),
    ("mdast", "mdast", true),
    ("hast", "hast-util", true),
    ("unified", "unifiedlil", true),
    ("unified", "unifiedjs", true),
    ("playcanvas", "playcanvas", true),
    ("monaco", "monaco", true),
    ("solid", "solidjs", true),
    ("solid", "solid-js", true),
    ("solid", "solidlil", true),
    ("solid", "lil-solid", true),
    ("vue", "vuelil", true),
    ("cn", "cnlil", true),
    ("react-markdown", "react-markdown", true),
    ("react-markdown", "React", false),
    ("probelil", "probelil", true),
    ("acorn", "acorn", true),
];

const LEDGER: &str = "tests/no3-allowlist.json";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn is_test_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    name.ends_with("_tests.rs")
        || name == "tests.rs"
        || path.components().any(|part| part.as_os_str() == "fixtures")
}

fn sources(directory: &Path, found: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("{}: {error}", directory.display()))
        .map(|entry| entry.expect("directory entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            sources(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs")
            && !is_test_file(&path)
        {
            found.push(path);
        }
    }
}

/// The lines outside inline test modules: from a `#[cfg(test)]` line that is
/// followed by `mod name {` to that module's closing brace at column zero.
fn compiler_lines(text: &str) -> Vec<(usize, &str)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut kept = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        if line.trim_start() == "#[cfg(test)]" {
            let next = (index + 1..lines.len().min(index + 4))
                .find(|&n| !lines[n].trim_start().starts_with("#["));
            if let Some(next) = next {
                let opened = lines[next].trim_start();
                if opened.starts_with("mod ") && opened.trim_end().ends_with('{') {
                    let indent = lines[next].len() - opened.len();
                    let close = (next + 1..lines.len()).find(|&n| {
                        let candidate = lines[n];
                        candidate.trim_end() == "}"
                            && candidate.len() - candidate.trim_start().len() == indent
                    });
                    index = close.map_or(lines.len(), |close| close + 1);
                    continue;
                }
            }
        }
        kept.push((index + 1, line));
        index += 1;
    }
    kept
}

fn occurrences(line: &str, needle: &str, insensitive: bool) -> usize {
    if insensitive {
        line.to_ascii_lowercase().matches(needle).count()
    } else {
        // Exact spelling as a whole word.
        let bytes = line.as_bytes();
        line.match_indices(needle)
            .filter(|&(at, _)| {
                let before = at.checked_sub(1).map(|i| bytes[i]);
                let after = bytes.get(at + needle.len()).copied();
                let word =
                    |byte: Option<u8>| byte.is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_');
                !word(before) && !word(after)
            })
            .count()
    }
}

/// (file relative to the repository, ledger name) -> the lines mentioning it.
fn mentions() -> BTreeMap<(String, String), Vec<usize>> {
    let root = root();
    let mut files = Vec::new();
    sources(&root.join("src"), &mut files);
    let mut found: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();
    for path in files {
        let text = std::fs::read_to_string(&path).expect("source is UTF-8");
        let relative = path
            .strip_prefix(&root)
            .expect("under the repository")
            .to_string_lossy()
            .replace('\\', "/");
        for (number, line) in compiler_lines(&text) {
            for &(name, needle, insensitive) in NEEDLES {
                for _ in 0..occurrences(line, needle, insensitive) {
                    found
                        .entry((relative.clone(), name.to_string()))
                        .or_default()
                        .push(number);
                }
            }
        }
    }
    found
}

#[test]
fn no_library_names_in_compiler_source_beyond_the_ledger() {
    let ledger: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root().join(LEDGER)).expect("the NO3 allowlist ledger"),
    )
    .expect("the NO3 ledger is JSON");
    let mut allowed: BTreeMap<(String, String), u64> = BTreeMap::new();
    let mut problems = Vec::new();
    for (index, entry) in ledger["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .enumerate()
    {
        let field = |key: &str| entry[key].as_str().unwrap_or("").trim().to_string();
        let (file, name, reason, owner) = (
            field("file"),
            field("name"),
            field("reason"),
            field("owner"),
        );
        let count = entry["count"].as_u64().unwrap_or(0);
        if file.is_empty() || name.is_empty() || count == 0 {
            problems.push(format!(
                "ledger entry {}: needs file, name and a positive count",
                index + 1
            ));
        }
        if reason.is_empty() || owner.is_empty() {
            problems.push(format!(
                "ledger entry {} ({file}, {name}): needs a reason and an owner",
                index + 1
            ));
        }
        if !NEEDLES.iter().any(|&(known, _, _)| known == name) {
            problems.push(format!(
                "ledger entry {} names an unknown library `{name}`",
                index + 1
            ));
        }
        if allowed
            .insert((file.clone(), name.clone()), count)
            .is_some()
        {
            problems.push(format!(
                "ledger entry {} repeats ({file}, {name})",
                index + 1
            ));
        }
    }
    let found = mentions();
    for ((file, name), lines) in &found {
        let count = lines.len() as u64;
        match allowed.get(&(file.clone(), name.clone())) {
            None => problems.push(format!(
                "{file}: {count} new mention(s) of `{name}` at line(s) {lines:?}; the compiler must not know a library (NO3): state the rule generically, or ledger it with a reason and an owner"
            )),
            Some(&limit) if count > limit => problems.push(format!(
                "{file}: {count} mentions of `{name}` (lines {lines:?}) where the ledger allows {limit}"
            )),
            Some(&limit) if count < limit => problems.push(format!(
                "{file}: {count} mentions of `{name}` where the ledger allows {limit}: lower the entry to {count} (the ledger only shrinks)"
            )),
            _ => {}
        }
    }
    for (file, name) in allowed.keys() {
        if !found.contains_key(&(file.clone(), name.clone())) {
            problems.push(format!(
                "{file}: the ledger allows `{name}` but the file no longer mentions it: remove the entry"
            ));
        }
    }
    assert!(
        problems.is_empty(),
        "NO3 ({LEDGER}):\n{}",
        problems.join("\n")
    );
}

#[test]
fn inline_test_modules_and_exact_words_are_matched_as_documented() {
    let text = "fn a() {} // katexlil\n#[cfg(test)]\nmod tests {\n    // zodlil\n}\nfn b() {}\n";
    let kept: Vec<usize> = compiler_lines(text).iter().map(|(n, _)| *n).collect();
    assert_eq!(kept, vec![1, 6]);
    assert_eq!(
        occurrences("React and reactive, React's", "React", false),
        2
    );
    assert_eq!(occurrences("KaTeX and katexlil", "katex", true), 2);
    assert_eq!(occurrences("a marked lazy chunk", "markedlil", true), 0);
}
