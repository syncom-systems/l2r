//! Syntax of identifiers, Lean names, paths and free text.

/// Module names that profiles generate themselves and components may not take.
pub const RESERVED_MODULES: &[&str] = &["contract_tests", "imp", "routes"];

const RUST_KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate",
    "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl",
    "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

/// Lower snake case: `[a-z][a-z0-9_]*`, at most 64 bytes, no `__`, no trailing `_`.
pub fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    let starts_lower = matches!(chars.next(), Some('a'..='z'));
    starts_lower
        && s.len() <= 64
        && s.chars().all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_'))
        && !s.contains("__")
        && !s.ends_with('_')
}

/// True for words Rust reserves.
pub fn is_rust_keyword(s: &str) -> bool {
    RUST_KEYWORDS.contains(&s)
}

/// An ASCII, dot-separated Lean name such as `Crossing.Latch.step`.
pub fn is_lean_name(s: &str) -> bool {
    !s.is_empty() && s.split('.').all(is_lean_segment)
}

fn is_lean_segment(segment: &str) -> bool {
    let mut chars = segment.chars();
    let valid_start = matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_');
    valid_start && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '\'' | '!' | '?'))
}

/// A relative path that cannot escape its base; see [`l2r_model::RelPath`].
pub fn is_safe_relative_path(path: &str) -> bool {
    l2r_model::RelPath::parse(path).is_ok()
}

/// Free text without control characters other than newline.
pub fn is_clean_text(text: &str) -> bool {
    text.chars().all(|c| c == '\n' || !c.is_control())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers() {
        assert!(is_identifier("request_latch"));
        assert!(is_identifier("a1"));
        for bad in ["", "Latch", "1a", "a-b", "a__b", "a_", "a b"] {
            assert!(!is_identifier(bad), "{bad}");
        }
    }

    #[test]
    fn lean_names() {
        assert!(is_lean_name("Crossing.Latch.step"));
        assert!(is_lean_name("Foo.bar'"));
        for bad in ["", "Foo.", ".Foo", "Foo..bar", "Foo.1bar", "Foo bar"] {
            assert!(!is_lean_name(bad), "{bad}");
        }
    }

    #[test]
    fn paths() {
        assert!(is_safe_relative_path("scaffold"));
        assert!(is_safe_relative_path("a/b-c/d_e.json"));
        for bad in [
            "", "/abs", "../up", "a/../b", "a//b", ".hidden", "a/.git", "a\\b",
        ] {
            assert!(!is_safe_relative_path(bad), "{bad}");
        }
    }

    #[test]
    fn text() {
        assert!(is_clean_text("line one\nline two"));
        assert!(!is_clean_text("bell\u{7}"));
        assert!(!is_clean_text("carriage\rreturn"));
    }
}
