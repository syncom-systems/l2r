//! A minimal line diff for previews.

/// Renders a line diff of `old` → `new` with `-`/`+`/` ` prefixes.
///
/// Uses a longest-common-subsequence table, which is ample for scaffold-sized
/// files; very large inputs fall back to a whole-file replacement.
pub fn line_diff(old: &str, new: &str) -> String {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    if a.len().saturating_mul(b.len()) > 4_000_000 {
        return a
            .iter()
            .map(|l| format!("-{l}\n"))
            .chain(b.iter().map(|l| format!("+{l}\n")))
            .collect();
    }
    let lcs = lcs_table(&a, &b);
    let (mut i, mut j) = (0, 0);
    let mut out = String::new();
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            out.push_str(&format!(" {}\n", a[i]));
            i += 1;
            j += 1;
        } else if j < b.len() && (i == a.len() || lcs[i][j + 1] >= lcs[i + 1][j]) {
            out.push_str(&format!("+{}\n", b[j]));
            j += 1;
        } else {
            out.push_str(&format!("-{}\n", a[i]));
            i += 1;
        }
    }
    out
}

/// `table[i][j]` is the LCS length of `a[i..]` and `b[j..]`.
fn lcs_table(a: &[&str], b: &[&str]) -> Vec<Vec<usize>> {
    let mut table = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            table[i][j] = if a[i] == b[j] {
                table[i + 1][j + 1] + 1
            } else {
                table[i + 1][j].max(table[i][j + 1])
            };
        }
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_changed_lines_only_as_changes() {
        assert_eq!(line_diff("a\nb\nc\n", "a\nx\nc\n"), " a\n+x\n-b\n c\n");
        assert_eq!(line_diff("", "new\n"), "+new\n");
        assert_eq!(line_diff("same\n", "same\n"), " same\n");
    }
}
