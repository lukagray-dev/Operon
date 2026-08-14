//! seek_sequence — fuzzy line-sequence matcher for the edit tool.
//!
//! Hey friend! This module locates sequences of pattern lines within a file's line buffer using six
//! passes of decreasing strictness.
//!
//! Matching passes (applied in order, first success wins):
//!   1. Exact match
//!   2. Trailing-whitespace-ignored match (rstrip)
//!   3. Both-sides-trimmed match
//!   4. Unicode-normalised match — normalises fancy dashes, quotes, and
//!      non-breaking spaces to their ASCII equivalents before comparing.
//!   5. Case-insensitive both-sides-trimmed match
//!   6. Case-insensitive Unicode-normalised match

/// Normalise a line: trim both ends then map fancy Unicode punctuation to
/// ASCII equivalents so a plain-ASCII pattern can still locate the region.
pub(crate) fn normalise(s: &str) -> String {
    s.trim()
        .chars()
        .map(|c| match c {
            // Various dash / hyphen codepoints → ASCII '-'
            '\u{2010}' // HYPHEN
            | '\u{2011}' // NON-BREAKING HYPHEN
            | '\u{2012}' // FIGURE DASH
            | '\u{2013}' // EN DASH
            | '\u{2014}' // EM DASH
            | '\u{2015}' // HORIZONTAL BAR
            | '\u{2212}' // MINUS SIGN
            => '-',

            // Fancy single quotes → ASCII apostrophe
            '\u{2018}' // LEFT SINGLE QUOTATION MARK
            | '\u{2019}' // RIGHT SINGLE QUOTATION MARK
            | '\u{201A}' // SINGLE LOW-9 QUOTATION MARK
            | '\u{201B}' // SINGLE HIGH-REVERSED-9 QUOTATION MARK
            => '\'',

            // Fancy double quotes → ASCII double quote
            '\u{201C}' // LEFT DOUBLE QUOTATION MARK
            | '\u{201D}' // RIGHT DOUBLE QUOTATION MARK
            | '\u{201E}' // DOUBLE LOW-9 QUOTATION MARK
            | '\u{201F}' // DOUBLE HIGH-REVERSED-9 QUOTATION MARK
            => '"',

            // Non-breaking and miscellaneous odd spaces → ASCII space
            '\u{00A0}' // NO-BREAK SPACE
            | '\u{2002}' // EN SPACE
            | '\u{2003}' // EM SPACE
            | '\u{2004}' // THREE-PER-EM SPACE
            | '\u{2005}' // FOUR-PER-EM SPACE
            | '\u{2006}' // SIX-PER-EM SPACE
            | '\u{2007}' // FIGURE SPACE
            | '\u{2008}' // PUNCTUATION SPACE
            | '\u{2009}' // THIN SPACE
            | '\u{200A}' // HAIR SPACE
            | '\u{202F}' // NARROW NO-BREAK SPACE
            | '\u{205F}' // MEDIUM MATHEMATICAL SPACE
            | '\u{3000}' // IDEOGRAPHIC SPACE
            => ' ',

            other => other,
        })
        .collect::<String>()
}

/// Result of searching for a pattern sequence across fuzzy matching passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SequenceMatch {
    /// No match found across any pass.
    NotFound,
    /// Exactly one match found at the returned starting line index.
    Unique(usize),
    /// Ambiguous match found multiple times (contains match count >= 2).
    Ambiguous(usize),
}

/// Searches `lines` for `pattern` across the 6 fuzzy matching passes in order of strictness.
///
/// For the first pass that produces at least one match:
/// - If exactly 1 match is found, returns `SequenceMatch::Unique(start_line_idx)`.
/// - If 2 or more non-overlapping matches are found, returns `SequenceMatch::Ambiguous(count)`.
/// If no pass finds any match, returns `SequenceMatch::NotFound`.
pub(crate) fn find_sequence_match(lines: &[String], pattern: &[String]) -> SequenceMatch {
    // Edge cases: empty pattern or pattern longer than the file
    if pattern.is_empty() || pattern.len() > lines.len() {
        return SequenceMatch::NotFound;
    }

    let pattern_len = pattern.len();
    let max_start = lines.len().saturating_sub(pattern_len);

    // ── Pass 1: Exact match ────────────────────────────────────────────────
    let mut matches = Vec::new();
    let mut i = 0;
    while i <= max_start {
        if lines[i..i + pattern_len] == *pattern {
            matches.push(i);
            i += pattern_len.max(1);
        } else {
            i += 1;
        }
    }
    if !matches.is_empty() {
        return if matches.len() == 1 {
            SequenceMatch::Unique(matches[0])
        } else {
            SequenceMatch::Ambiguous(matches.len())
        };
    }

    // ── Pass 2: Trailing-whitespace-ignored (rstrip) match ─────────────────
    matches.clear();
    i = 0;
    while i <= max_start {
        let all_match = pattern.iter().enumerate().all(|(p_idx, pat)| {
            lines[i + p_idx].trim_end() == pat.trim_end()
        });
        if all_match {
            matches.push(i);
            i += pattern_len.max(1);
        } else {
            i += 1;
        }
    }
    if !matches.is_empty() {
        return if matches.len() == 1 {
            SequenceMatch::Unique(matches[0])
        } else {
            SequenceMatch::Ambiguous(matches.len())
        };
    }

    // ── Pass 3: Both-sides-trimmed match ──────────────────────────────────
    matches.clear();
    i = 0;
    while i <= max_start {
        let all_match = pattern.iter().enumerate().all(|(p_idx, pat)| {
            lines[i + p_idx].trim() == pat.trim()
        });
        if all_match {
            matches.push(i);
            i += pattern_len.max(1);
        } else {
            i += 1;
        }
    }
    if !matches.is_empty() {
        return if matches.len() == 1 {
            SequenceMatch::Unique(matches[0])
        } else {
            SequenceMatch::Ambiguous(matches.len())
        };
    }

    // ── Pass 4: Unicode-normalisation pass ────────────────────────────────
    matches.clear();
    i = 0;
    while i <= max_start {
        let all_match = pattern.iter().enumerate().all(|(p_idx, pat)| {
            normalise(&lines[i + p_idx]) == normalise(pat)
        });
        if all_match {
            matches.push(i);
            i += pattern_len.max(1);
        } else {
            i += 1;
        }
    }
    if !matches.is_empty() {
        return if matches.len() == 1 {
            SequenceMatch::Unique(matches[0])
        } else {
            SequenceMatch::Ambiguous(matches.len())
        };
    }

    // ── Pass 5: Case-insensitive both-sides-trimmed match ──────────────────
    matches.clear();
    i = 0;
    while i <= max_start {
        let all_match = pattern.iter().enumerate().all(|(p_idx, pat)| {
            lines[i + p_idx].trim().eq_ignore_ascii_case(pat.trim())
        });
        if all_match {
            matches.push(i);
            i += pattern_len.max(1);
        } else {
            i += 1;
        }
    }
    if !matches.is_empty() {
        return if matches.len() == 1 {
            SequenceMatch::Unique(matches[0])
        } else {
            SequenceMatch::Ambiguous(matches.len())
        };
    }

    // ── Pass 6: Case-insensitive Unicode-normalisation pass ────────────────
    matches.clear();
    i = 0;
    while i <= max_start {
        let all_match = pattern.iter().enumerate().all(|(p_idx, pat)| {
            normalise(&lines[i + p_idx]).to_lowercase() == normalise(pat).to_lowercase()
        });
        if all_match {
            matches.push(i);
            i += pattern_len.max(1);
        } else {
            i += 1;
        }
    }
    if !matches.is_empty() {
        return if matches.len() == 1 {
            SequenceMatch::Unique(matches[0])
        } else {
            SequenceMatch::Ambiguous(matches.len())
        };
    }

    SequenceMatch::NotFound
}

/// Find the first occurrence of `pattern` lines in `lines` starting at or
/// after `start`. When `eof` is true, first attempt to match at the end of
/// `lines` before falling back to a forward search from `start`.
///
/// Returns the 0-based index into `lines` where the match starts, or None.
/// Empty pattern always returns Some(start). Pattern longer than lines returns None.
#[allow(dead_code)]
pub(crate) fn seek_sequence(
    lines: &[String],
    pattern: &[String],
    start: usize,
    eof: bool,
) -> Option<usize> {
    if pattern.is_empty() {
        return Some(start);
    }

    if pattern.len() > lines.len() {
        return None;
    }

    if eof && lines.len() >= pattern.len() {
        let end_idx = lines.len() - pattern.len();

        // Pass 1: Exact match at the end
        if lines[end_idx..end_idx + pattern.len()] == *pattern {
            return Some(end_idx);
        }

        // Pass 2: Trailing-whitespace-ignored match at the end
        let pass2 = pattern.iter().enumerate().all(|(p_idx, pat)| {
            lines[end_idx + p_idx].trim_end() == pat.trim_end()
        });
        if pass2 {
            return Some(end_idx);
        }

        // Pass 3: Both-sides-trimmed match at the end
        let pass3 = pattern.iter().enumerate().all(|(p_idx, pat)| {
            lines[end_idx + p_idx].trim() == pat.trim()
        });
        if pass3 {
            return Some(end_idx);
        }

        // Pass 4: Unicode-normalised match at the end
        let pass4 = pattern.iter().enumerate().all(|(p_idx, pat)| {
            normalise(&lines[end_idx + p_idx]) == normalise(pat)
        });
        if pass4 {
            return Some(end_idx);
        }

        // Pass 5: Case-insensitive both-sides-trimmed match at the end
        let pass5 = pattern.iter().enumerate().all(|(p_idx, pat)| {
            lines[end_idx + p_idx].trim().eq_ignore_ascii_case(pat.trim())
        });
        if pass5 {
            return Some(end_idx);
        }

        // Pass 6: Case-insensitive Unicode-normalised match at the end
        let pass6 = pattern.iter().enumerate().all(|(p_idx, pat)| {
            normalise(&lines[end_idx + p_idx]).to_lowercase() == normalise(pat).to_lowercase()
        });
        if pass6 {
            return Some(end_idx);
        }
    }

    let search_start = start;

    // Pass 1: Exact match
    for i in search_start..=lines.len().saturating_sub(pattern.len()) {
        if lines[i..i + pattern.len()] == *pattern {
            return Some(i);
        }
    }

    // Pass 2: Trailing-whitespace-ignored match
    for i in search_start..=lines.len().saturating_sub(pattern.len()) {
        let all_match = pattern.iter().enumerate().all(|(p_idx, pat)| {
            lines[i + p_idx].trim_end() == pat.trim_end()
        });
        if all_match {
            return Some(i);
        }
    }

    // Pass 3: Both-sides-trimmed match
    for i in search_start..=lines.len().saturating_sub(pattern.len()) {
        let all_match = pattern.iter().enumerate().all(|(p_idx, pat)| {
            lines[i + p_idx].trim() == pat.trim()
        });
        if all_match {
            return Some(i);
        }
    }

    // Pass 4: Unicode normalisation
    for i in search_start..=lines.len().saturating_sub(pattern.len()) {
        let all_match = pattern.iter().enumerate().all(|(p_idx, pat)| {
            normalise(&lines[i + p_idx]) == normalise(pat)
        });
        if all_match {
            return Some(i);
        }
    }

    // Pass 5: Case-insensitive trimmed match
    for i in search_start..=lines.len().saturating_sub(pattern.len()) {
        let all_match = pattern.iter().enumerate().all(|(p_idx, pat)| {
            lines[i + p_idx].trim().eq_ignore_ascii_case(pat.trim())
        });
        if all_match {
            return Some(i);
        }
    }

    // Pass 6: Case-insensitive Unicode normalisation
    for i in search_start..=lines.len().saturating_sub(pattern.len()) {
        let all_match = pattern.iter().enumerate().all(|(p_idx, pat)| {
            normalise(&lines[i + p_idx]).to_lowercase() == normalise(pat).to_lowercase()
        });
        if all_match {
            return Some(i);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(strings: &[&str]) -> Vec<String> {
        strings.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn exact_match_finds_sequence() {
        let lines = v(&["foo", "bar", "baz"]);
        let pattern = v(&["bar", "baz"]);
        assert_eq!(
            seek_sequence(&lines, &pattern, 0, false),
            Some(1)
        );
    }

    #[test]
    fn exact_match_single_line() {
        let lines = v(&["alpha", "beta", "gamma"]);
        assert_eq!(seek_sequence(&lines, &v(&["beta"]), 0, false), Some(1));
    }

    #[test]
    fn exact_match_at_start() {
        let lines = v(&["a", "b", "c"]);
        assert_eq!(seek_sequence(&lines, &v(&["a", "b"]), 0, false), Some(0));
    }

    #[test]
    fn exact_match_honours_start_offset() {
        let lines = v(&["a", "b", "a"]);
        assert_eq!(seek_sequence(&lines, &v(&["a"]), 1, false), Some(2));
    }

    #[test]
    fn rstrip_match_ignores_trailing_whitespace() {
        let lines = v(&["foo   ", "bar\t\t"]);
        let pattern = v(&["foo", "bar"]);
        assert_eq!(
            seek_sequence(&lines, &pattern, 0, false),
            Some(0)
        );
    }

    #[test]
    fn rstrip_match_trailing_spaces_on_pattern() {
        let lines = v(&["hello", "world"]);
        assert_eq!(
            seek_sequence(&lines, &v(&["hello  ", "world  "]), 0, false),
            Some(0)
        );
    }

    #[test]
    fn trim_match_ignores_leading_and_trailing_whitespace() {
        let lines = v(&["    foo   ", "   bar\t"]);
        let pattern = v(&["foo", "bar"]);
        assert_eq!(
            seek_sequence(&lines, &pattern, 0, false),
            Some(0)
        );
    }

    #[test]
    fn unicode_dash_normalised() {
        let lines = v(&["some\u{2014}thing"]);
        assert_eq!(seek_sequence(&lines, &v(&["some-thing"]), 0, false), Some(0));
    }

    #[test]
    fn unicode_curly_quotes_normalised() {
        let lines = v(&["\u{201C}hello\u{201D}"]);
        assert_eq!(seek_sequence(&lines, &v(&["\"hello\""]), 0, false), Some(0));
    }

    #[test]
    fn unicode_single_quotes_normalised() {
        let lines = v(&["it\u{2019}s fine"]);
        assert_eq!(seek_sequence(&lines, &v(&["it's fine"]), 0, false), Some(0));
    }

    #[test]
    fn unicode_nbsp_normalised() {
        let lines = v(&["hello\u{00A0}world"]);
        assert_eq!(seek_sequence(&lines, &v(&["hello world"]), 0, false), Some(0));
    }

    #[test]
    fn empty_pattern_returns_start() {
        let lines = v(&["a", "b", "c"]);
        assert_eq!(seek_sequence(&lines, &[], 2, false), Some(2));
    }

    #[test]
    fn pattern_longer_than_input_returns_none() {
        let lines = v(&["just one line"]);
        let pattern = v(&["too", "many", "lines"]);
        assert_eq!(
            seek_sequence(&lines, &pattern, 0, false),
            None
        );
    }

    #[test]
    fn no_match_returns_none() {
        let lines = v(&["alpha", "beta"]);
        assert_eq!(seek_sequence(&lines, &v(&["gamma"]), 0, false), None);
    }

    #[test]
    fn eof_true_prefers_end_match() {
        let lines = v(&["start", "end", "middle", "other", "end"]);
        assert_eq!(seek_sequence(&lines, &v(&["end"]), 0, true), Some(4));
    }

    #[test]
    fn eof_true_falls_back_when_end_has_no_match() {
        let lines = v(&["unique", "alpha", "beta"]);
        assert_eq!(
            seek_sequence(&lines, &v(&["unique"]), 0, true),
            Some(0)
        );
    }

    #[test]
    fn find_sequence_match_unique_and_ambiguous() {
        let lines = v(&["alpha", "beta", "gamma", "beta"]);
        assert_eq!(
            find_sequence_match(&lines, &v(&["alpha"])),
            SequenceMatch::Unique(0)
        );
        assert_eq!(
            find_sequence_match(&lines, &v(&["beta"])),
            SequenceMatch::Ambiguous(2)
        );
        assert_eq!(
            find_sequence_match(&lines, &v(&["delta"])),
            SequenceMatch::NotFound
        );
    }

    #[test]
    fn find_sequence_match_fuzzy_passes() {
        // Unicode dash + quote
        let lines = v(&["fn foo() {", "    let x = \"some\u{2014}thing\";", "}"]);
        let pattern = v(&["    let x = \"some-thing\";"]);
        assert_eq!(
            find_sequence_match(&lines, &pattern),
            SequenceMatch::Unique(1)
        );

        // Case insensitivity
        let lines = v(&["MAX_LIMIT = 500"]);
        let pattern = v(&["max_limit = 500"]);
        assert_eq!(
            find_sequence_match(&lines, &pattern),
            SequenceMatch::Unique(0)
        );
    }
}
