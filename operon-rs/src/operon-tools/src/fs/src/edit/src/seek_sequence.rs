// seek_sequence — fuzzy line-sequence matcher for the edit tool.
//
// Locates a sequence of pattern lines within a file's line buffer using four
// passes of decreasing strictness. Ported from the Codex apply-patch crate.
//
// Matching passes (applied in order, first success wins):
//   1. Exact match
//   2. Trailing-whitespace-ignored match (rstrip)
//   3. Both-sides-trimmed match
//   4. Unicode-normalised match — normalises fancy dashes, quotes, and
//      non-breaking spaces to their ASCII equivalents before comparing.


/// Find the first occurrence of `pattern` lines in `lines` starting at or
/// after `start`. When `eof` is true, first attempt to match at the end of
/// `lines` before falling back to a forward search from `start`.
///
/// Returns the 0-based index into `lines` where the match starts, or None.
/// Empty pattern always returns Some(start). Pattern longer than lines returns None.
///
/// # Arguments
/// - `lines`:   The file split into individual lines (no trailing newline on each element).
/// - `pattern`: The sequence of lines to locate within `lines`.
/// - `start`:   The earliest index in `lines` from which to begin searching.
/// - `eof`:     When true, attempt to match at the very end of `lines` first.
pub(crate) fn seek_sequence(
    lines: &[String],
    pattern: &[String],
    start: usize,
    eof: bool,
) -> Option<usize> {
    // Empty pattern is a no-op — it matches at the current position.
    if pattern.is_empty() {
        return Some(start);
    }

    // If the pattern is longer than the available lines there is no possible
    // match. Guard against out-of-bounds slicing that would otherwise panic.
    if pattern.len() > lines.len() {
        return None;
    }

    // When `eof` is set, start the search from the last position where the
    // pattern could still fit, so we prefer an end-of-file match. If that
    // fails, the loops below continue scanning forward from `start`.
    let search_start = if eof && lines.len() >= pattern.len() {
        lines.len() - pattern.len()
    } else {
        start
    };

    // ── Pass 1: Exact match ────────────────────────────────────────────────
    // Compare line slices byte-for-byte. Cheapest pass — if it succeeds we
    // know the diff matches the file exactly.
    for i in search_start..=lines.len().saturating_sub(pattern.len()) {
        if lines[i..i + pattern.len()] == *pattern {
            return Some(i);
        }
    }

    // ── Pass 2: Trailing-whitespace-ignored (rstrip) match ─────────────────
    // Many editors silently strip trailing spaces/tabs on save. This pass
    // allows mismatched trailing whitespace without failing.
    for i in search_start..=lines.len().saturating_sub(pattern.len()) {
        let all_match = pattern.iter().enumerate().all(|(p_idx, pat)| {
            lines[i + p_idx].trim_end() == pat.trim_end()
        });
        if all_match {
            return Some(i);
        }
    }

    // ── Pass 3: Both-sides-trimmed match ──────────────────────────────────
    // More lenient: strip leading AND trailing whitespace from both sides.
    // Handles indentation drift (e.g. a tab was replaced by spaces).
    for i in search_start..=lines.len().saturating_sub(pattern.len()) {
        let all_match = pattern.iter().enumerate().all(|(p_idx, pat)| {
            lines[i + p_idx].trim() == pat.trim()
        });
        if all_match {
            return Some(i);
        }
    }

    // ── Pass 4: Unicode-normalisation pass ────────────────────────────────
    // Diffs authored with plain ASCII may fail to match source files that
    // contain typographic characters (em-dashes, curly quotes, non-breaking
    // spaces). This pass normalises a curated set of Unicode codepoints to
    // their ASCII equivalents before comparing.
    //
    // Normalised character groups:
    //   Various dash/hyphen codepoints → '-'
    //   Fancy single quotes            → '\''
    //   Fancy double quotes            → '"'
    //   Non-breaking and odd spaces    → ' '

    /// Normalise a line: trim both ends then map fancy Unicode punctuation to
    /// ASCII equivalents so a plain-ASCII diff can still locate the region.
    fn normalise(s: &str) -> String {
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

    for i in search_start..=lines.len().saturating_sub(pattern.len()) {
        let all_match = pattern.iter().enumerate().all(|(p_idx, pat)| {
            normalise(&lines[i + p_idx]) == normalise(pat)
        });
        if all_match {
            return Some(i);
        }
    }

    // No pass found a match.
    None
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::seek_sequence;

    /// Helper: convert a slice of &str literals into Vec<String>.
    fn v(strings: &[&str]) -> Vec<String> {
        strings.iter().map(|s| s.to_string()).collect()
    }

    // ── Pass 1: Exact match ────────────────────────────────────────────────

    #[test]
    fn exact_match_finds_sequence() {
        let lines = v(&["foo", "bar", "baz"]);
        let pattern = v(&["bar", "baz"]);
        assert_eq!(
            seek_sequence(&lines, &pattern, /*start*/ 0, /*eof*/ false),
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
        // Pattern "a" exists at index 0 but start=1, so it should not be found there;
        // the second occurrence at index 2 must be returned.
        let lines = v(&["a", "b", "a"]);
        assert_eq!(seek_sequence(&lines, &v(&["a"]), 1, false), Some(2));
    }

    // ── Pass 2: Rstrip (trailing-whitespace-ignored) match ────────────────

    #[test]
    fn rstrip_match_ignores_trailing_whitespace() {
        let lines = v(&["foo   ", "bar\t\t"]);
        // Pattern omits trailing whitespace — rstrip pass must find it.
        let pattern = v(&["foo", "bar"]);
        assert_eq!(
            seek_sequence(&lines, &pattern, /*start*/ 0, /*eof*/ false),
            Some(0)
        );
    }

    #[test]
    fn rstrip_match_trailing_spaces_on_pattern() {
        // File line has no trailing space but pattern does.
        let lines = v(&["hello", "world"]);
        assert_eq!(
            seek_sequence(&lines, &v(&["hello  ", "world  "]), 0, false),
            Some(0)
        );
    }

    // ── Pass 3: Both-sides-trimmed match ──────────────────────────────────

    #[test]
    fn trim_match_ignores_leading_and_trailing_whitespace() {
        let lines = v(&["    foo   ", "   bar\t"]);
        // Pattern omits any additional whitespace.
        let pattern = v(&["foo", "bar"]);
        assert_eq!(
            seek_sequence(&lines, &pattern, /*start*/ 0, /*eof*/ false),
            Some(0)
        );
    }

    // ── Pass 4: Unicode normalisation ─────────────────────────────────────

    #[test]
    fn unicode_dash_normalised() {
        // File contains an em-dash; pattern uses an ASCII hyphen.
        let lines = v(&["some\u{2014}thing"]);
        assert_eq!(seek_sequence(&lines, &v(&["some-thing"]), 0, false), Some(0));
    }

    #[test]
    fn unicode_curly_quotes_normalised() {
        // File contains fancy double quotes; pattern uses straight quotes.
        let lines = v(&["\u{201C}hello\u{201D}"]);
        assert_eq!(seek_sequence(&lines, &v(&["\"hello\""]), 0, false), Some(0));
    }

    #[test]
    fn unicode_single_quotes_normalised() {
        // File contains a curly apostrophe; pattern uses ASCII apostrophe.
        let lines = v(&["it\u{2019}s fine"]);
        assert_eq!(seek_sequence(&lines, &v(&["it's fine"]), 0, false), Some(0));
    }

    #[test]
    fn unicode_nbsp_normalised() {
        // File contains a non-breaking space; pattern has a regular space.
        let lines = v(&["hello\u{00A0}world"]);
        assert_eq!(seek_sequence(&lines, &v(&["hello world"]), 0, false), Some(0));
    }

    // ── Edge cases ─────────────────────────────────────────────────────────

    #[test]
    fn empty_pattern_returns_start() {
        let lines = v(&["a", "b", "c"]);
        // Empty pattern is always a match at `start`, regardless of content.
        assert_eq!(seek_sequence(&lines, &[], 2, false), Some(2));
    }

    #[test]
    fn pattern_longer_than_input_returns_none() {
        let lines = v(&["just one line"]);
        let pattern = v(&["too", "many", "lines"]);
        // Must NOT panic — returns None when pattern cannot possibly fit.
        assert_eq!(
            seek_sequence(&lines, &pattern, /*start*/ 0, /*eof*/ false),
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
        // "end" appears at index 1 AND index 4. With eof=true the search must
        // prefer the later occurrence (index 4).
        let lines = v(&["start", "end", "middle", "other", "end"]);
        assert_eq!(seek_sequence(&lines, &v(&["end"]), 0, true), Some(4));
    }

    #[test]
    fn eof_true_falls_back_when_end_has_no_match() {
        // Pattern only occurs at the beginning; eof=true must fall back and find it.
        let lines = v(&["unique", "alpha", "beta"]);
        assert_eq!(
            seek_sequence(&lines, &v(&["unique"]), 0, true),
            Some(0)
        );
    }
}
