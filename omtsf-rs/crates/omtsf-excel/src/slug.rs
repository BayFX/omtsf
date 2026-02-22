//! Stable slug generation for auto-generated node and edge IDs.
//!
//! Generates URL-safe, human-readable identifiers from a prefix and a name.
//! The algorithm: lowercase, replace non-alphanumeric with hyphens, collapse
//! consecutive hyphens, strip leading/trailing hyphens, truncate to 48 chars.

/// Generates a stable slug from a type prefix and entity name.
///
/// For example, `make_slug("org", "Bolt Supplies Ltd")` → `"org-bolt-supplies-ltd"`.
/// If the name is blank, falls back to a counter-based ID: `"org-1"`, `"org-2"`, etc.
pub fn make_slug(prefix: &str, name: &str, counter: usize) -> String {
    let name = name.trim();
    if name.is_empty() {
        return format!("{prefix}-{counter}");
    }

    let slugged: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();

    // Collapse consecutive hyphens.
    let mut result = String::with_capacity(slugged.len());
    let mut last_was_hyphen = false;
    for ch in slugged.chars() {
        if ch == '-' {
            if !last_was_hyphen {
                result.push('-');
            }
            last_was_hyphen = true;
        } else {
            result.push(ch);
            last_was_hyphen = false;
        }
    }

    let result = result.trim_matches('-');
    if result.is_empty() {
        return format!("{prefix}-{counter}");
    }

    let truncated = if result.len() > 48 {
        &result[..48]
    } else {
        result
    };
    let truncated = truncated.trim_end_matches('-');

    format!("{prefix}-{truncated}")
}

/// Generates an edge slug from edge type and source/target IDs.
pub fn make_edge_slug(edge_type: &str, source: &str, target: &str, counter: usize) -> String {
    let src = source.trim_start_matches(|c: char| !c.is_ascii_alphanumeric());
    let tgt = target.trim_start_matches(|c: char| !c.is_ascii_alphanumeric());
    let short_src: String = src.chars().take(12).collect();
    let short_tgt: String = tgt.chars().take(12).collect();
    let candidate = format!("edge-{edge_type}-{short_src}-{short_tgt}");
    if candidate.len() < 64 {
        candidate
    } else {
        format!("edge-{counter}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_basic() {
        assert_eq!(
            make_slug("org", "Bolt Supplies Ltd", 1),
            "org-bolt-supplies-ltd"
        );
    }

    #[test]
    fn slug_empty_name_uses_counter() {
        assert_eq!(make_slug("org", "", 3), "org-3");
    }

    #[test]
    fn slug_collapses_hyphens() {
        assert_eq!(make_slug("org", "A  B  C", 1), "org-a-b-c");
    }

    #[test]
    fn slug_truncates_long_names() {
        let long = "a".repeat(100);
        let result = make_slug("org", &long, 1);
        assert!(result.len() <= 56);
    }
}
